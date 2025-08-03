use serde::{Deserialize, Deserializer, Serialize, de};
use serde_json::json;
use std::{str::FromStr, time::SystemTime};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{error, info, warn};

use axum::{
    Json, Router,
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::StatusCode,
    middleware,
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
/// - `pair`: trading pair, e.g. `"BTC-USD"` or `"ETH-USD"`
#[derive(serde::Deserialize)]
pub struct NewOrder {
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<u64>,
    pub quantity: u64,
    #[serde(rename = "symbol", deserialize_with = "parse_pair")]
    pub pair: Pair,
}
fn parse_pair<'de, D>(deserializer: D) -> Result<Pair, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Pair::from_str(&s).map_err(|_| de::Error::custom(format!("unsupported symbol `{}`", s)))
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
    //TODO this needs pagination logic
    let symbol = pair.code().clone();
    let store = state.store.lock().unwrap();
    let trades = store.iter_trades().unwrap();
    Ok(Json(
        trades.into_iter().filter(|t| t.symbol == symbol).collect(),
    ))
}

/// `GET /book`
/// Returns a JSON snapshot of the current order‐book.
pub async fn get_order_book(
    Path(pair): Path<Pair>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let books = state.order_books.lock().unwrap();
    let snapshot = books
        .get(&pair)
        .map(|book| BookSnapshot::for_pair(pair.clone(), book))
        .unwrap_or_else(|| BookSnapshot::empty(pair));
    Json(snapshot).into_response()
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

    //persist all trades in store
    let mut store = state.store.lock().unwrap();
    for trade in &trades {
        store.insert_trade(trade).map_err(|e| {
            error!("DB insert failed: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to persist trade"})),
            )
        })?;
    }

    //broadcast trades after successfull persistence
    for trade in trades.iter() {
        let _ = state.trade_tx.send(trade.clone());
    }
    let _ = state.book_tx.send(payload.pair);
    Ok(Json(OrderAck { order_id, trades }))
}

/// `DELETE /orders/{id}`
/// Path parameter:
/// - `id` – the UUID of the order to cancel.
///
/// Cancels the order with the given ID.
/// *Success:* 200, JSON `{ "status": "cancelled" }`
/// *Failure:* 404, JSON `{ "error": "Order not found", "status": 404 }`
pub async fn cancel_order(
    State(state): State<AppState>,
    Path((pair, order_id)): Path<(Pair, u64)>,
) -> impl IntoResponse {
    //TODO confirm pair is valid
    //this is incomplete
    let mut books = state.order_books.lock().unwrap();
    let book = books.get_mut(&pair).unwrap();
    if book.cancel_order(order_id) {
        info!("Order {} cancelled successfully.", order_id);
        let _ = state.book_tx.send(pair);
        (StatusCode::OK, Json(json!({"status": "cancelled"})))
    } else {
        warn!("Cancel failed: Order {} not found.", order_id);
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
        BookSnapshot::for_pair(pair.clone(), book)
    };
    if let Err(e) = socket
        .send(Message::Text(
            serde_json::to_string(&WsFrame::BookSnapshot(initial))
                .unwrap()
                .into(),
        ))
        .await
    {
        error!("Failed to send initial snapshot: {:?}", e);
        return;
    }

    loop {
        tokio::select! {
            Ok(trade) = trade_rx.recv() => {
                if trade.symbol == pair.clone().code() {
                if let Err(e) = socket.send(Message::Text(serde_json::to_string(&WsFrame::Trade(trade)).unwrap().into())).await {
                    error!("WebSocket send trade failed: {:?}", e);
                    break;
                }
                }

            }
            Ok(updated_pair) = book_rx.recv() => {
                if updated_pair == pair.clone() {
                    //get related book
                    let book = {
                         state.order_books.lock().unwrap()[&pair].clone()
                    };

                    let snap = BookSnapshot::for_pair(pair.clone(), &book);
                    if let Err(e) = socket.send(Message::Text(serde_json::to_string(&WsFrame::BookSnapshot(snap)).unwrap().into())).await {
                        error!("WebSocket send snapshot failed: {:?}", e);
                        break;
                    }
                }
            } else => break
        }
    }
}

/// Constructs the application’s `Router` with all routes and shared state.
pub fn router(state: AppState) -> Router {
    //all routes that require pair will pass throught the middleware that validates the pair argument
    let root = Router::new().route("/orders", post(create_order));

    let pair_router = Router::new()
        .route("/orders/{pair}/{id}", delete(cancel_order))
        .route("/trades/{pair}", get(get_trade_log))
        .route("/book/{pair}", get(get_order_book))
        .route("/ws/{pair}", get(ws_handler))
        .layer(middleware::from_extractor::<Path<Pair>>());

    root.merge(pair_router)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(
                    DefaultMakeSpan::new()
                        .include_headers(false)
                        .level(tracing::Level::TRACE),
                )
                .on_response(DefaultOnResponse::new().level(tracing::Level::TRACE)),
        )
        .with_state(state)
}
