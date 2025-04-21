use std::time::SystemTime;

use axum::{Json, Router, debug_handler, extract::State, http::StatusCode, routing::post};
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

#[debug_handler]
pub async fn create_order(
    State(state): State<AppState>,
    Json(payload): Json<NewOrder>,
) -> Result<Json<Vec<Trade>>, StatusCode> {
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
    let trades = book.match_order(order);
    log.extend(trades.clone());
    Ok(Json(trades))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/orders", post(create_order))
        .with_state(state)
}
