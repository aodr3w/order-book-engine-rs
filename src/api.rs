use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::SystemTime;
use tracing::{error, info, warn};

use axum::{
    Json, Router,
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use uuid::Uuid;

use crate::{
    instrument::Pair,
    orders::{Order, OrderType, Side},
    state::AppState,
    trade::Trade,
};

/// Request payload for `POST /orders`.
///
/// - `side`: buy or sell  
/// - `order_type`: limit or market  
/// - `price`: limit price (ignored for market)  
/// - `quantity`: how many units to trade
#[derive(serde::Deserialize)]
pub struct NewOrder {
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<u64>,
    pub quantity: u64,
    pub pair: Pair,
}

/// Response payload for `GET /book`.
///
/// - `bids`: list of `(price, total_quantity)` in descending order  
/// - `asks`: list of `(price, total_quantity)` in ascending order
#[derive(Serialize, Deserialize)]
pub struct BookSnapshot {
    pub bids: Vec<(u64, u64)>,
    pub asks: Vec<(u64, u64)>,
}

#[derive(Serialize, Deserialize)]
//#[serde(tag = "type", content = "data")]
pub enum WsFrame {
    Trade(Trade),
    BookSnapshot(BookSnapshot),
}
/// Response for `POST /orders`.
///
/// - `order_id`: the newly generated order ID  
/// - `trades`: any matched trades resulting from this order
#[derive(serde::Serialize, serde::Deserialize)]
pub struct OrderAck {
    pub order_id: u64,
    trades: Vec<Trade>,
}
/// `GET /trades`  
/// *Success:* 200, JSON `Vec<Trade>`
/// *Failure:* 500 if the database query fails
pub async fn get_trade_log(State(state): State<AppState>) -> Result<Json<Vec<Trade>>, StatusCode> {
    let rows = sqlx::query!(
        r#"SELECT price, quantity, maker_id as "maker_id!", taker_id as "taker_id!", timestamp_utc
           FROM trades
           ORDER BY timestamp_utc DESC
           LIMIT 100"#
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let trades = rows
        .into_iter()
        .map(|r| {
            let price_u64 = r.price.to_string().parse::<u64>().unwrap_or(0);
            Trade {
                price: price_u64,
                quantity: r.quantity as u64,
                maker_id: r.maker_id as u64,
                taker_id: r.taker_id as u64,
                timestamp: r.timestamp_utc.into(),
            }
        })
        .collect();
    Ok(Json(trades))
}

/// `GET /book`
/// Returns a JSON snapshot of the current order‐book.
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

/// `POST /orders`  
/// Creates a new order.
/// *Success:* 200, JSON `OrderAck`
/// *Failure:* 500, JSON `{ "error": "internal server error" }`
pub async fn create_order(
    State(state): State<AppState>,
    Json(payload): Json<NewOrder>,
) -> Result<Json<OrderAck>, StatusCode> {
    let (order_id, trades) = {
        let mut book = state.order_book.lock().unwrap();
        let mut log = state.trade_log.lock().unwrap();
        let order = Order {
            id: Uuid::new_v4().as_u128() as u64,
            side: payload.side,
            order_type: payload.order_type,
            price: payload.price,
            quantity: payload.quantity,
            timestamp: SystemTime::now(),
            pair: payload.pair,
        };
        let order_id = order.id;
        let trades = book.match_order(order);
        log.extend(trades.clone());
        (order_id, trades)
    };

    //persist all trades in a single db tx;
    let mut tx = state
        .db_pool
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for trade in &trades {
        sqlx::query!(
            r#"
            INSERT INTO TRADES (price, quantity, maker_id, taker_id, timestamp_utc)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            trade.price as f64,
            trade.quantity as i64,
            trade.maker_id as i64,
            trade.taker_id as i64,
            chrono::Utc::now(),
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("DB insert failed: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }
    tx.commit().await.map_err(|e| {
        tracing::error!("DB commit failed: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    //broadcast trades after successfull persistence
    for trade in trades.iter() {
        let _ = state.trade_tx.send(trade.clone());
    }
    let _ = state.book_tx.send(());
    Ok(Json(OrderAck { order_id, trades }))
}

/// `DELETE /orders/{id}`
/// Path parameter:
/// - `id` – the UUID of the order to cancel.
///
/// Cancels the order with the given ID.
/// *Success:* 200, JSON `{ "status": "cancelled" }`
/// *Failure:* 404, JSON `{ "error": "Order not found", "status": 404 }`
pub async fn cancel_order(State(state): State<AppState>, Path(id): Path<u64>) -> impl IntoResponse {
    let mut book = state.order_book.lock().unwrap();
    if book.cancel_order(id) {
        info!("Order {} cancelled successfully.", id);
        let _ = state.book_tx.send(());
        (StatusCode::OK, Json(json!({"status": "cancelled"})))
    } else {
        warn!("Cancel failed: Order {} not found.", id);
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Order not found", "status": 404})),
        )
    }
}

/// `GET /ws`  
/// Upgrades the HTTP connection to a WebSocket and then  
/// streams order‐book snapshots and trade events to the client.
pub async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Once the socket connection is upgraded from HTTP to WebSocket, drives the message loop:
///  - Sends an initial `BookSnapshot`  
///  - Listens for trade and book‐update broadcasts and forwards them
pub async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut trade_rx = state.trade_tx.subscribe();
    let mut book_rx = state.book_tx.subscribe();

    //initial snapshot
    let initial = {
        let book = state.order_book.lock().unwrap();
        BookSnapshot {
            bids: book
                .bids
                .iter()
                .rev()
                .map(|(p, o)| (*p, o.iter().map(|o| o.quantity).sum()))
                .collect(),
            asks: book
                .asks
                .iter()
                .map(|(p, o)| (*p, o.iter().map(|o| o.quantity).sum()))
                .collect(),
        }
    };
    let data = serde_json::to_string(&WsFrame::BookSnapshot(initial)).unwrap();
    if let Err(e) = socket.send(Message::Text(data.into())).await {
        error!("Failed to send initial snapshot: {:?}", e);
        return;
    }

    loop {
        tokio::select! {
            Ok(trade) = trade_rx.recv() => {
                if let Err(e) = socket.send(Message::Text(serde_json::to_string(&WsFrame::Trade(trade)).unwrap().into())).await {
                    error!("WebSocket send trade failed: {:?}", e);
                    break;
                }
            }
            Ok(_) = book_rx.recv() => {
                let snap = {
                    let book = state.order_book.lock().unwrap();
                    BookSnapshot {
                        bids: book.bids.iter().rev().map(|(p,o)| (*p,o.iter().map(|o|o.quantity).sum())).collect(),
                        asks: book.asks.iter().map(|(p,o)| (*p,o.iter().map(|o|o.quantity).sum())).collect()
                    }
                };
                if let Err(e) =  socket.send(Message::Text(serde_json::to_string(&WsFrame::BookSnapshot(snap)).unwrap().into())).await {
                    error!("WebSocket send snapshot failed: {:?}", e);
                    return;
                }
            } else => break
        }
    }
}

/// Constructs the application’s `Router` with all routes and shared state.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/book", get(get_order_book))
        .route("/orders", post(create_order))
        .route("/trades", get(get_trade_log))
        .route("/orders/{id}", delete(cancel_order))
        .route("/ws", get(ws_handler))
        .with_state(state)
}
