use serde_json::json;
use std::time::SystemTime;
use tracing::{info, warn};

use axum::{
    Json, Router, debug_handler,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use uuid::Uuid;

use crate::{
    orders::{Order, OrderType, Side},
    state::AppState,
    trade::Trade,
};

#[derive(serde::Deserialize)]
pub struct NewOrder {
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<u64>,
    pub quantity: u64,
}

#[derive(serde::Serialize)]
pub struct BookSnapshot {
    pub bids: Vec<(u64, u64)>,
    pub asks: Vec<(u64, u64)>,
}

#[derive(serde::Serialize)]
pub struct OrderAck {
    order_id: u64,
    trades: Vec<Trade>,
}
#[debug_handler]
pub async fn get_trade_log(State(state): State<AppState>) -> Json<Vec<Trade>> {
    let log = state.trade_log.lock().unwrap();
    Json(log.to_vec())
}
#[debug_handler]
pub async fn get_order_book(State(state): State<AppState>) -> Json<BookSnapshot> {
    let book = state.order_book.lock().unwrap();
    let bids: Vec<(u64, u64)> = book
        .bids
        .iter()
        .rev()
        .map(|(price, orders)| (*price, orders.iter().map(|o| o.quantity).sum()))
        .collect();

    let asks = book
        .asks
        .iter()
        .map(|(price, orders)| (*price, orders.iter().map(|o| o.quantity).sum()))
        .collect();

    Json(BookSnapshot { bids, asks })
}

#[debug_handler]
pub async fn create_order(
    State(state): State<AppState>,
    Json(payload): Json<NewOrder>,
) -> Result<Json<OrderAck>, StatusCode> {
    let mut book = state.order_book.lock().unwrap();
    let mut log = state.trade_log.lock().unwrap();

    let order = Order {
        id: Uuid::new_v4().as_u128() as u64,
        side: payload.side,
        order_type: payload.order_type,
        price: payload.price,
        quantity: payload.quantity,
        timestamp: SystemTime::now(),
    };
    let order_id = order.id;
    tracing::info!("order with id: {} created", order_id);
    let trades = book.match_order(order);
    log.extend(trades.clone());
    let resp = OrderAck { order_id, trades };
    Ok(axum::Json(resp))
}

pub async fn cancel_order(State(state): State<AppState>, Path(id): Path<u64>) -> impl IntoResponse {
    let mut book = state.order_book.lock().unwrap();
    if book.cancel_order(id) {
        info!("Order {} cancelled successfully.", id);
        (StatusCode::OK, Json(json!({"status": "cancelled"})))
    } else {
        warn!("Cancel failed: Order {} not found.", id);
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Order not found", "status": 404})),
        )
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/book", get(get_order_book))
        .route("/orders", post(create_order))
        .route("/trades", get(get_trade_log))
        .route("/orders/{id}", delete(cancel_order))
        .with_state(state)
}
