use std::{
    collections::{BTreeMap, VecDeque},
    time::SystemTime,
};

use crate::{
    orders::{Order, Side},
    trade::Trade,
};

/// An [`OrderBook`] stores **active** buy and sell orders in two separate
/// [`BTreeMap`]s:
/// - `bids` (buy orders)  
/// - `asks` (sell orders)
///
/// Each price level (key) has a FIFO queue of orders stored in a [`VecDeque`]
/// to maintain **price-time** priority.
pub struct OrderBook {
    /// Buy orders, keyed by price in ascending order.
    ///
    /// For matching, we'll iterate **in reverse** to find the highest bid first.
    pub bids: BTreeMap<u64, VecDeque<Order>>,

    /// Sell orders, keyed by price in ascending order.
    ///
    /// For matching, we iterate **forwards** to find the lowest ask first.
    pub asks: BTreeMap<u64, VecDeque<Order>>,
}

/// Internal enum to unify forward (`IterMut`) and reverse (`Rev<IterMut>`) BTreeMap iteration.
///
/// - [`EitherIter::Fwd`] handles ascending iteration over prices.
/// - [`EitherIter::Rev`] handles descending iteration (used for matching sells against the highest bids).
enum EitherIter<'a> {
    /// Forward (ascending) iteration over the price levels.
    Fwd(std::collections::btree_map::IterMut<'a, u64, VecDeque<Order>>),
    /// Reverse (descending) iteration over the price levels.
    Rev(std::iter::Rev<std::collections::btree_map::IterMut<'a, u64, VecDeque<Order>>>),
}

impl<'a> Iterator for EitherIter<'a> {
    type Item = (&'a u64, &'a mut VecDeque<Order>);

    /// Retrieves the **next** `(price, VecDeque<Order>)` pair from the underlying iterator.
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            EitherIter::Fwd(iter) => iter.next(),
            EitherIter::Rev(iter) => iter.next(),
        }
    }
}

/// Matches an **incoming order** against one side of the order book,
/// potentially producing a series of [`Trade`]s.
///
/// # Parameters
/// - `incoming`: the incoming [`Order`] to be matched.
/// - `book_side`: a mutable reference to the [`BTreeMap`] representing the relevant side
///   of the book (e.g., `asks` for a buy, `bids` for a sell).
/// - `reversed`: indicates whether to iterate in descending (`true`) or ascending (`false`) order.
///
/// # Returns
/// A [`Vec<Trade>`] describing all the partial or full matches that occurred.
///
/// # Notes
/// - This function supports **partial fills**: if the resting order or the incoming order
///   cannot fully satisfy the other, a partial match is made.
/// - The fill quantity is determined using `min(incoming.quantity, resting.quantity)` to
///   ensure the trade does not overfill either order. This is essential for:
///   - Correct matching (only fill what’s available on both sides)
///   - Preventing negative quantities or overflows
///   - Supporting realistic order book behavior (e.g., partial matches over multiple price levels)
///
/// # Example
/// - A market buy for 10 units encounters a sell (ask) order for 6 units.
/// - The engine fills 6 units, then proceeds to match the remaining 4 against the next best ask.
fn match_incoming_side(
    incoming: &mut Order,
    book_side: &mut BTreeMap<u64, VecDeque<Order>>,
    reversed: bool,
) -> Vec<Trade> {
    let mut trades = Vec::new();
    let mut levels_to_remove = Vec::new();

    // Choose iterator direction based on `reversed`
    let iter = if reversed {
        EitherIter::Rev(book_side.iter_mut().rev())
    } else {
        EitherIter::Fwd(book_side.iter_mut())
    };

    // Labeled loop to break out early if `incoming.quantity` becomes zero.
    'outer: for (&price, orders_at_price) in iter {
        while let Some(order) = orders_at_price.front_mut() {
            // Determine how many units to fill in this match
            let trade_qty = incoming.quantity.min(order.quantity);

            trades.push(Trade {
                price,
                quantity: trade_qty,
                maker_id: order.id,
                taker_id: incoming.id,
                timestamp: SystemTime::now(),
            });

            // Update the quantities on both orders
            incoming.quantity -= trade_qty;
            order.quantity -= trade_qty;

            // Remove the fully filled resting order from the queue front
            if order.quantity == 0 {
                orders_at_price.pop_front();
            }

            // If all orders at this price were consumed, mark the level for cleanup
            if orders_at_price.is_empty() {
                levels_to_remove.push(price);
            }
            // If the incoming order is fully satisfied, break out of both loops
            if incoming.quantity == 0 {
                break 'outer;
            }
        }
    }

    // Remove empty price levels
    for price in levels_to_remove {
        book_side.remove(&price);
    }

    trades
}

impl OrderBook {
    /// Creates a new, empty [`OrderBook`], with no active bids or asks.
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    /// Adds a **limit** order to the order book (buy or sell).  
    ///
    /// If it's a market order (`price == None`), we print a warning and do not add it
    /// since market orders match immediately and do not rest in the book.
    pub fn add_order(&mut self, order: Order) {
        if let Some(price) = order.price {
            let book_side = match order.side {
                Side::Buy => &mut self.bids,
                Side::Sell => &mut self.asks,
            };
            book_side
                .entry(price)
                .or_insert_with(VecDeque::new)
                .push_back(order);
        } else {
            eprintln!("Warning: Attempting to add a market order to the book. Ignoring...");
        }
    }

    /// Matches an incoming **market** order against the order book.
    ///
    /// # Behavior
    /// - If `incoming.side` is `Buy`, we match against the `asks` from lowest to highest.
    /// - If `incoming.side` is `Sell`, we match against the `bids` from highest to lowest.
    ///
    /// Returns a [`Vec<Trade>`] describing all executed trades.
    ///
    /// *Note:* For **limit** orders, you’d typically match what can be matched, then
    /// rest the remainder in the book.  
    /// Currently, this function is specialized for market orders or the "matching" portion
    /// of a limit order.
    pub fn match_order(&mut self, mut incoming: Order) -> Vec<Trade> {
        match incoming.side {
            Side::Buy => {
                // Market Buy => match asks (lowest first)
                match_incoming_side(&mut incoming, &mut self.asks, false)
            }
            Side::Sell => {
                // Market Sell => match bids (highest first)
                match_incoming_side(&mut incoming, &mut self.bids, true)
            }
        }
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

//tests

#[cfg(test)]
mod tests {
    use crate::orders::OrderType;

    use super::*;

    fn sample_limit_order(id: u64, side: Side, price: u64, quantity: u64) -> Order {
        Order {
            id,
            side,
            order_type: OrderType::Limit,
            price: Some(price),
            quantity,
            timestamp: SystemTime::now(),
        }
    }

    fn sample_market_order(id: u64, side: Side, quantity: u64) -> Order {
        Order {
            id,
            side,
            order_type: OrderType::Market,
            price: None,
            quantity,
            timestamp: SystemTime::now(),
        }
    }

    #[test]
    fn test_partial_fill_market_buy() {
        let mut ob = OrderBook::new();

        ob.add_order(sample_limit_order(1, Side::Sell, 101, 5));
        ob.add_order(sample_limit_order(2, Side::Sell, 102, 3));

        let market_buy = sample_market_order(100, Side::Buy, 6);
        let trades = ob.match_order(market_buy);

        assert_eq!(trades.len(), 2);
        assert_eq!(trades[0].quantity, 5);
        assert_eq!(trades[0].price, 101);
        assert_eq!(trades[1].quantity, 1);
        assert_eq!(trades[1].price, 102);

        // Remaining ask at 102 should have 2 units left
        let remaining = ob.asks.get(&102).unwrap();
        assert_eq!(remaining[0].quantity, 2);
    }

    #[test]
    fn test_partial_fill_market_sell() {
        let mut ob = OrderBook::new();

        ob.add_order(sample_limit_order(1, Side::Buy, 100, 4));

        let market_sell = sample_market_order(200, Side::Sell, 10);
        let trades = ob.match_order(market_sell);

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, 4);
        assert_eq!(trades[0].price, 100);
        assert!(!ob.bids.contains_key(&100));
    }

    #[test]
    fn test_no_match_for_market_order() {
        let mut ob = OrderBook::new();

        let market_buy = sample_market_order(300, Side::Buy, 10);
        let trades = ob.match_order(market_buy);

        assert!(trades.is_empty());
        assert!(ob.asks.is_empty());
    }

    #[test]
    fn test_exact_match_market_order() {
        let mut ob = OrderBook::new();

        ob.add_order(sample_limit_order(1, Side::Sell, 100, 5));
        let market_buy = sample_market_order(400, Side::Buy, 5);
        assert!(ob.asks.len() == 1);
        let trades = ob.match_order(market_buy);

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, 5);
        assert!(ob.asks.is_empty());
    }
}
