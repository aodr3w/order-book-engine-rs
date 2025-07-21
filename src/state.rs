use tokio::sync::broadcast;

use crate::{
    instrument::Pair,
    orderbook::OrderBook,
    store::{Store, StoreResult},
    trade::Trade,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// Shared application state.
///
/// Holds:
///  - `order_book` and `trade_log` behind `Arc<Mutex<…>>` for safe concurrent access  
///  - `trade_tx` and `book_tx` broadcast channels to notify subscribers of new trades
///    and order‐book updates  
///  - `db_pool` for PostgreSQL connections
#[derive(Clone)]
pub struct AppState {
    ///in-memory map of books, with an order-book per pair
    pub order_books: Arc<Mutex<HashMap<Pair, OrderBook>>>,
    /// The in‐memory order‐book.
    pub order_book: Arc<Mutex<OrderBook>>,

    /// The in‐memory trade history.
    pub trade_log: Arc<Mutex<Vec<Trade>>>,

    /// Broadcast channel for new trades.
    pub trade_tx: broadcast::Sender<Trade>,

    /// Broadcast channel for order‐book updates.
    pub book_tx: broadcast::Sender<Pair>,

    /// store
    pub store: Arc<Mutex<Store>>,
}

impl AppState {
    pub async fn new(store_path: impl AsRef<std::path::Path>) -> StoreResult<Self> {
        let store = Store::open(store_path)?;
        let (trade_tx, _) = broadcast::channel(1024);
        let (book_tx, _) = broadcast::channel(16);
        let mut books = HashMap::new();

        for pair in Pair::supported() {
            books.insert(pair.clone(), OrderBook::new());
        }
        Ok(Self {
            order_books: Arc::new(Mutex::new(books)),
            order_book: Arc::new(Mutex::new(OrderBook::new())),
            trade_log: Arc::new(Mutex::new(Vec::new())),
            trade_tx,
            book_tx,
            store: Arc::new(Mutex::new(store)),
        })
    }
}
