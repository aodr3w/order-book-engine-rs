use crate::{orderbook::OrderBook, trade::Trade};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub order_book: Arc<Mutex<OrderBook>>,
    pub trade_log: Arc<Mutex<Vec<Trade>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            order_book: Arc::new(Mutex::new(OrderBook::new())),
            trade_log: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        AppState::new()
    }
}
