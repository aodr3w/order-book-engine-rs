use errors::MarketMakerError;
use futures_util::StreamExt;
use reqwest;
use serde::{Deserialize, Serialize};
use tokio::{sync::watch, time};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMsg};

use crate::{
    api::{BookSnapshot, OrderAck, WsFrame},
    errors,
    orders::{OrderType, Side},
};

// # Market Maker Bot
//
// Continuously provides liquidity by posting a two-sided quote around the current mid-price.
//
// ## What it does
// 1. Connects to your order‐book engine’s WebSocket feed (`/ws`) to receive live `BookSnapshot` frames.
// 2. Extracts the best bid and best ask, and computes the mid-price as `(best_bid + best_ask) / 2`.
// 3. Every `PACE_MS` milliseconds:
//    - Cancels its prior bid and ask orders to avoid stale quotes.
//    - Posts a fresh **buy** at `mid_price - SPREAD` and an **ask** at `mid_price + SPREAD` (size = 1).
// 4. Remembers the `order_id`s it just placed so it can cancel them cleanly on the next cycle.
//
// ## Why it works
// - **Two‐sided quoting** tightens the market: by always offering to buy and sell, the market maker
//   narrows the spread and provides immediate liquidity to other traders.
// - Re‐quoting at a fixed **PACE** avoids resting stale orders if the market moves:
//   narrow spreads can quickly become unprofitable if the mid shifts—so we cancel and repost.
// - A small constant **SPREAD** (in ticks) balances the trade‐off between capturing the spread
//   (profit per trade) and being competitive (higher fill probability).
// - Using a **watch channel** for the mid ensures the quoting loop always has the latest value
//   without blocking on the WebSocket read task.
//
// ## Key Constants
// - `SPREAD`: fixed distance from mid-price for quotes. A larger spread gives more profit per fill
//   but may be quoted less often by others.
// - `PACE_MS`: how frequently (ms) to cancel & re-quote. A faster pace tracks a rapidly moving market
//   more closely but generates more traffic and fees.

// fixed distance from mid-price for quotes
const SPREAD: u64 = 2;

// cadence for quote updates: cancel + repost every 500ms
const PACE_MS: u64 = 500;

// A small helper to serialize outgoing orders
#[derive(Deserialize, Serialize)]
struct NewOrder {
    side: Side,
    order_type: OrderType,
    price: Option<u64>,
    quantity: u64,
}

/// Starts the market maker loop against a REST+WS API at `api_base`.
///
/// 1. Establishes a WebSocket connection to `ws://{api_base}/ws`.
/// 2. Spawns a background task that listens for `BookSnapshot` frames:
///    - Parses best bid & best ask from each snapshot
///    - Computes and broadcasts the mid-price via a `tokio::watch` channel
/// 3. Enters a loop, ticking every `PACE_MS` ms:
///    - If we have a mid-price, cancel all currently outstanding quotes
///      via `DELETE /orders/{id}`
///    - Sends two new limit orders (size=1):
///      - **Buy** at `(mid_price - SPREAD)`
///      - **Sell** at `(mid_price + SPREAD)`
///    - Records the returned `order_id`s so they can be cancelled on the
///      next iteration.
///
/// Errors from the WebSocket connection or HTTP client are wrapped in
/// `MarketMakerError` for upstream handling.
pub async fn run_market_maker(api_base: &str) -> Result<(), MarketMakerError> {
    // 1) Subscribe to /ws
    let (ws_stream, _) = connect_async(format!("{}/ws", api_base))
        .await
        .map_err(|e| MarketMakerError::ConnectError(e.to_string()))?;
    let (_write, mut read) = ws_stream.split();

    // Channel to hold the latest computed mid-price
    let (mid_tx, mid_rx) = watch::channel(None::<u64>);

    // 2) Spawn task: parse snapshots → update `mid_tx`
    tokio::spawn(async move {
        while let Some(Ok(WsMsg::Text(txt))) = read.next().await {
            // Only care about BookSnapshot frames
            if let Ok(WsFrame::BookSnapshot(BookSnapshot { bids, asks })) =
                serde_json::from_str::<WsFrame>(&txt)
            {
                if let (Some((best_bid, _)), Some((best_ask, _))) = (bids.first(), asks.first()) {
                    let mid = (best_bid + best_ask) / 2;
                    let _ = mid_tx.send(Some(mid));
                }
            }
        }
    });

    // 3) Every PACE_MS: cancel & repost quotes
    let client = reqwest::Client::new();
    let mut outstanding: Vec<u64> = Vec::new();
    let mut interval = time::interval(time::Duration::from_millis(PACE_MS));

    loop {
        interval.tick().await;

        // Only quote once we have a mid-price
        if let Some(mid_price) = *mid_rx.borrow() {
            // Cancel all previous orders
            for id in outstanding.drain(..) {
                let _ = client
                    .delete(format!("{}/orders/{}", api_base, id))
                    .send()
                    .await;
            }

            // Post a new bid
            if let Ok(resp) = client
                .post(format!("{}/orders", api_base))
                .json(&NewOrder {
                    side: Side::Buy,
                    order_type: OrderType::Limit,
                    price: Some(mid_price.saturating_sub(SPREAD)),
                    quantity: 1,
                })
                .send()
                .await
            {
                if let Ok(ack) = resp.json::<OrderAck>().await {
                    outstanding.push(ack.order_id);
                }
            }

            // Post a new ask
            if let Ok(resp) = client
                .post(format!("{}/orders", api_base))
                .json(&NewOrder {
                    side: Side::Sell,
                    order_type: OrderType::Limit,
                    price: Some(mid_price + SPREAD),
                    quantity: 1,
                })
                .send()
                .await
            {
                if let Ok(ack) = resp.json::<OrderAck>().await {
                    outstanding.push(ack.order_id);
                }
            }
        }
    }
}
