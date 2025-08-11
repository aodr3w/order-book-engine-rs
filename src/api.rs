use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, DeserializeOwned},
};
use serde_json::json;
use std::{str::FromStr, time::SystemTime};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{error, info, warn};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{
        FromRequest, Path, Query, Request, State, WebSocketUpgrade,
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

type ApiErr = (StatusCode, Json<serde_json::Value>);
fn err(status: StatusCode, msg: &str) -> ApiErr {
    (status, Json(json!({ "error": msg })))
}

fn log_rejected(payload: &NewOrder, reason: &str) {
    warn!(
        reason,
        side = ?payload.side,
        order_type = ?payload.order_type,
        price = ?payload.price,
        quantity = payload.quantity,
        pair = %payload.pair.code(),
        "order rejected"
    );
}

pub struct LoggedJson<T>(pub T);

impl<S, T> FromRequest<S> for LoggedJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiErr;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        //capture request detais for logging
        let method = req.method().clone();
        let uri = req.uri().clone();
        // read full body
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|e| (err(StatusCode::BAD_REQUEST, &e.to_string())))?;

        match serde_json::from_slice::<T>(&bytes) {
            Ok(val) => Ok(LoggedJson(val)),
            Err(e) => {
                //cap body preview to avoid giant logs
                let preview = String::from_utf8_lossy(&bytes);
                let preview = &preview[..preview.len().min(4096)];
                warn!(
                    error = %e,
                    %method,
                    uri=%uri,
                    body_preview = %preview,
                    "order rejected: JSON deserialization failed"
                );
                Err(err(StatusCode::UNPROCESSABLE_ENTITY, &e.to_string()))
            }
        }
    }
}

fn default_limit() -> usize {
    100
}
#[derive(Deserialize)]
pub struct TradesQuery {
    #[serde(default = "default_limit")]
    limit: usize,
    after: Option<String>,
}

#[derive(Serialize)]
pub struct TradesPage {
    items: Vec<Trade>,
    next: Option<String>,
}
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
/// A websocket message, either a snapshot of the order book or
/// a single trade event.
///
/// Serialized as an internally-tagged enum:
/// ```
/// {"type": "BookSnapshot", "data": { /* snapshot fields */}}
/// {"type": "Trade", "data": { /* trade fields */}}
/// ```
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsFrame {
    BookSnapshot(BookSnapshot),
    Trade(Trade),
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

/// `GET /trades/{pair}`
///
/// Returns the historical trades for the given trading pair.
///
/// # Path Parameters
/// - `pair`: Symbol of the trading pair to query (e.g. `"BTC-USD"`, `"ETH-USD"`).
///
/// # Success
/// - `200 OK` with a JSON array of [`Trade`] objects whose `symbol` matches `pair`.
///
/// # Errors
/// - `500 INTERNAL SERVER ERROR` if the trade store cannot be queried.
///
pub async fn get_trade_log(
    Path(pair): Path<Pair>,
    State(state): State<AppState>,
    Query(q): Query<TradesQuery>,
) -> Result<Json<TradesPage>, StatusCode> {
    let limit = q.limit.min(1000);
    let (items, next) = {
        let store = state.store.read().await;
        store
            .page_trade_asc(&pair.code(), q.after.as_deref(), limit)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };
    Ok(Json(TradesPage { items, next }))
}

/// `GET /book`
/// Returns a JSON snapshot of the current order‐book.
pub async fn get_order_book(
    Path(pair): Path<Pair>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let books = state.order_books.read().await;
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
    LoggedJson(payload): LoggedJson<NewOrder>,
) -> Result<Json<OrderAck>, ApiErr> {
    if payload.quantity == 0 {
        log_rejected(&payload, "quantity must be > 0");
        return Err(err(StatusCode::BAD_REQUEST, "quantity must be > 0"));
    }
    let (order_id, trades) = {
        let mut books = state.order_books.write().await;

        let Some(book) = books.get_mut(&payload.pair) else {
            log_rejected(&payload, "unsupported pair");
            return Err(err(StatusCode::BAD_REQUEST, "unsupported pair"));
        };
        let mut log = state.trade_log.write().await;
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
    let mut store = state.store.write().await;
    for trade in &trades {
        store
            .insert_trade(trade)
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    }

    //broadcast trades after successfull persistence
    for trade in &trades {
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
    let mut books = state.order_books.write().await;

    let Some(book) = books.get_mut(&pair) else {
        return err(StatusCode::BAD_REQUEST, "unsupported pair");
    };
    if book.cancel_order(order_id) {
        info!("Order {} cancelled successfully.", order_id);
        let _ = state.book_tx.send(pair);
        (StatusCode::OK, Json(json!({"status": "cancelled"})))
    } else {
        warn!("Cancel failed: Order {} not found.", order_id);
        err(StatusCode::NOT_FOUND, "order not found")
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
        let books = state.order_books.read().await; //TODO consider a RWLock
        match books.get(&pair) {
            Some(book) => BookSnapshot::for_pair(pair.clone(), book),
            None => BookSnapshot::empty(pair.clone()),
        }
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

                if trade.symbol.cmp(&pair.code()).is_eq() {
                if let Err(e) = socket.send(Message::Text(serde_json::to_string(&WsFrame::Trade(trade)).unwrap().into())).await {
                    error!("WebSocket send trade failed: {:?}", e);
                    break;
                }
                }

            }
            Ok(updated_pair) = book_rx.recv() => {
                if updated_pair.code().cmp(&pair.code()).is_eq(){
                    //get related book
                    let book = {
                         state.order_books.read().await[&pair].clone()
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
