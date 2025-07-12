use sqlx::PgPool;
use tokio::sync::broadcast;

use crate::{instrument::Pair, orderbook::OrderBook, trade::Trade};
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
    pub book_tx: broadcast::Sender<()>,

    /// Connection pool to the database.
    pub db_pool: PgPool,
}

impl AppState {
    pub async fn new() -> Self {
        dotenvy::dotenv().ok();
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be st in .env");
        let db_pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to Postgres");
        //migrate
        sqlx::migrate!("./migrations").run(&db_pool).await.unwrap();
        let (trade_tx, _) = broadcast::channel(1024); //size ??
        let (book_tx, _) = broadcast::channel(16); //size ??
        Self {
            order_books: Arc::new(Mutex::new(HashMap::new())),
            order_book: Arc::new(Mutex::new(OrderBook::new())),
            trade_log: Arc::new(Mutex::new(Vec::new())),
            trade_tx,
            book_tx,
            db_pool,
        }
    }
}
