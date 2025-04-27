use tokio::sync::broadcast;

use crate::{orderbook::OrderBook, trade::Trade};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub order_book: Arc<Mutex<OrderBook>>,
    pub trade_log: Arc<Mutex<Vec<Trade>>>,
    pub trade_tx: broadcast::Sender<Trade>,
    pub book_tx: broadcast::Sender<()>,
}

impl AppState {
    pub fn new() -> Self {
        let (trade_tx, _) = broadcast::channel(1024); //size ??
        let (book_tx, _) = broadcast::channel(16); //size ??
        Self {
            order_book: Arc::new(Mutex::new(OrderBook::new())),
            trade_log: Arc::new(Mutex::new(Vec::new())),
            trade_tx,
            book_tx,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        AppState::new()
    }
}
