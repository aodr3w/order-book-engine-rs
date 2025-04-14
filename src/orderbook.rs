use std::collections::{BTreeMap, VecDeque};

use crate::orders::{Order, Side};

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
