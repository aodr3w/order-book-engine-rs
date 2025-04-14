use std::{
    collections::{BTreeMap, VecDeque},
    time::SystemTime,
};

use crate::{
    orders::{Order, Side},
    trade::{self, Trade},
};

pub struct OrderBook {
    pub bids: BTreeMap<u64, VecDeque<Order>>,
    pub asks: BTreeMap<u64, VecDeque<Order>>,
}

//unify iterator types
enum EitherIter<'a> {
    Fwd(std::collections::btree_map::IterMut<'a, u64, VecDeque<Order>>),
    Rev(std::iter::Rev<std::collections::btree_map::IterMut<'a, u64, VecDeque<Order>>>),
}

impl<'a> Iterator for EitherIter<'a> {
    type Item = (&'a u64, &'a mut VecDeque<Order>);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            EitherIter::Fwd(iter) => iter.next(),
            EitherIter::Rev(iter) => iter.next(),
        }
    }
}
//helper function
fn match_incoming_side(
    incoming: &mut Order,
    book_side: &mut BTreeMap<u64, VecDeque<Order>>,
    reversed: bool,
) -> Vec<Trade> {
    let mut trades = Vec::new();
    let mut levels_to_remove = Vec::new();
    //choose iterator
    let iter = if reversed {
        EitherIter::Rev(book_side.iter_mut().rev())
    } else {
        EitherIter::Fwd(book_side.iter_mut())
    };
    //Labeled loop so we can break out from inner while loop
    'outer: for (&price, orders_at_price) in iter {
        while let Some(order) = orders_at_price.front_mut() {
            let trade_qty = incoming.quantity.min(order.quantity);
            //Record the trade
            trades.push(Trade {
                price,
                quantity: trade_qty,
                maker_id: order.id,
                taker_id: incoming.id,
                timestamp: SystemTime::now(),
            });
            //Adjust quantities
            incoming.quantity -= trade_qty;
            order.quantity -= trade_qty;

            //if the resting order is fully filled, remove it
            if order.quantity == 0 {
                orders_at_price.pop_front();
            }
            //if incoming order is fully filled , break out of both loops
            if incoming.quantity == 0 {
                break 'outer;
            }
        }
        // if this price level is fully consumed, mark for removal
        if orders_at_price.is_empty() {
            levels_to_remove.push(price);
        }
    }
    //Remove the emptied price levels
    for price in levels_to_remove {
        book_side.remove(&price);
    }
    trades
}
impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    pub fn add_order(&mut self, order: Order) {
        if let Some(price) = order.price {
            let book_side = match order.side {
                Side::Buy => &mut self.bids,
                Side::Sell => &mut self.asks,
            };
            book_side
                .entry(price)
                .or_insert(VecDeque::new())
                .push_back(order);
        } else {
            eprint!("Cannot add market order to order book");
        }
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}
