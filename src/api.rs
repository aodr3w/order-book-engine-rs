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
    orderbook::BookSnapshot,
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
pub async fn get_trade_log(
    Path(pair): Path<Pair>,
    State(state): State<AppState>,
) -> Result<Json<Vec<Trade>>, StatusCode> {
    let symbol = pair.code().clone();
    let rows = sqlx::query!(
        r#"SELECT symbol, price, quantity, maker_id as "maker_id!", taker_id as "taker_id!", timestamp_utc
           FROM trades
           WHERE symbol = $1
           ORDER BY timestamp_utc DESC
           LIMIT 100"#,
           symbol,
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
                symbol: r.symbol,
            }
        })
        .collect();
    Ok(Json(trades))
}

/// `GET /book`
/// Returns a JSON snapshot of the current order‐book.
pub async fn get_order_book(
    Path(pair): Path<Pair>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if !Pair::supported().contains(&pair) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "unsupported pair",
                "supported": Pair::supported().iter().map(|p|p.code()).collect::<Vec<_>>()
            })),
        ));
    };
    let books = state.order_books.lock().unwrap();
    let snapshot = books
        .get(&pair)
        .map(|book| BookSnapshot::for_pair(pair.clone(), book))
        .unwrap_or_else(|| BookSnapshot::empty(pair));
    Ok(Json(snapshot).into_response())
}

/// `POST /orders`  
/// Creates a new order.
///
/// *Success:*  
///   • 200, JSON `OrderAck`  
/// *Bad Request:*  
///   • 400, JSON `{ "error": "unsupported pair", "supported": ["BTC-USD","ETH-USD",…] }`  
/// *Failure:*  
///   • 500, JSON `{ "error": "internal server error" }`
pub async fn create_order(
    State(state): State<AppState>,
    Json(payload): Json<NewOrder>,
) -> Result<Json<OrderAck>, (StatusCode, Json<serde_json::Value>)> {
    if !Pair::supported().contains(&payload.pair) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "unsupported pair",
                "supported": Pair::supported().iter().map(|p|p.code()).collect::<Vec<_>>()
            })),
        ));
    };

    let (order_id, trades) = {
        let mut books = state.order_books.lock().unwrap();
        let book = books.get_mut(&payload.pair).unwrap();
        let mut log = state.trade_log.lock().unwrap();
        let order = Order {
            id: Uuid::new_v4().as_u128() as u64,
            side: payload.side,
            order_type: payload.order_type,
            price: payload.price,
            quantity: payload.quantity,
            timestamp: SystemTime::now(),
            pair: payload.pair.clone(),
        };
        let order_id = order.id;
        let trades = book.match_order(order);
        log.extend(trades.clone());
        (order_id, trades)
    };

    //persist all trades in a single db tx;
    let mut tx = state.db_pool.begin().await.map_err(|e| {
        error!("DB begin failed: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "internal server error"})),
        )
    })?;

    for trade in &trades {
        sqlx::query!(
            r#"
            INSERT INTO TRADES (symbol, price, quantity, maker_id, taker_id, timestamp_utc)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            payload.pair.code(),
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
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to persist trades"})),
            )
        })?;
    }
    tx.commit().await.map_err(|e| {
        tracing::error!("DB commit failed: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "DB Transaction commit failed"})),
        )
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
pub async fn ws_handler(
    Path(pair): Path<Pair>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    if !Pair::supported().contains(&pair) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "unsupported pair"})),
        )
            .into_response();
    }
    ws.on_upgrade(move |socket| handle_socket(socket, state, pair))
}

/// Once the socket connection is upgraded from HTTP to WebSocket, drives the message loop:
///  - Sends an initial `BookSnapshot`  
///  - Listens for trade and book‐update broadcasts and forwards them
pub async fn handle_socket(mut socket: WebSocket, state: AppState, pair: Pair) {
    let mut trade_rx = state.trade_tx.subscribe();
    let mut book_rx = state.book_tx.subscribe();

    //initial snapshot
    let initial = {
        let books = state.order_books.lock().unwrap();
        let book = &books[&pair];
        BookSnapshot::for_pair(pair, book)
    };
    let data = serde_json::to_string(&WsFrame::BookSnapshot(initial)).unwrap();
    if let Err(e) = socket.send(Message::Text(data.into())).await {
        error!("Failed to send initial snapshot: {:?}", e);
        return;
    }

    loop {
        tokio::select! {
            Ok(trade) = trade_rx.recv() => {
                if trade.symbol == pair.code() {
                if let Err(e) = socket.send(Message::Text(serde_json::to_string(&WsFrame::Trade(trade)).unwrap().into())).await {
                    error!("WebSocket send trade failed: {:?}", e);
                    break;
                }
                }

            }
            Ok(_) = book_rx.recv() => {
                //we need to check if this book snap shot is for the pair we are handling
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
        .route("/trades/:pair", get(get_trade_log))
        .route("/orders/{id}", delete(cancel_order))
        .route("/ws/:pair", get(ws_handler))
        .with_state(state)
}
