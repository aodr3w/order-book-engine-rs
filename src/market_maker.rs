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
// 1. Connects to your order-book engine’s WebSocket feed (`/ws`) to receive live `BookSnapshot` frames.
// 2. Extracts the best bid and best ask, and computes the mid-price as `(best_bid + best_ask) / 2`.
// 3. Every `PACE_MS` milliseconds **only if the mid-price has changed**:
//    - Cancels its prior bid and ask orders to avoid stale quotes.
//    - Posts a fresh **buy** at `mid_price - SPREAD` and an **ask** at `mid_price + SPREAD` (size = 1).
// 4. Remembers the `order_id`s it just placed so it can cancel them cleanly on the next cycle.
//
// ## Why it works
// - **Two-sided quoting** tightens the market by always offering both sides, narrowing spreads and
//   providing immediate liquidity.
// - Re-quoting only when the mid shifts avoids unnecessary churn and fee overhead when the market is
//   static.
// - A fixed **SPREAD** balances profitability per trade against competitiveness (fill probability).
// - Using a **watch channel** for the mid ensures the quoting loop always has the latest value without
//   blocking the WebSocket read task.
//
// ## Key Constants
// - `SPREAD`: fixed distance from mid-price for quotes. Larger spreads yield more profit but may be
//   less competitive.
// - `PACE_MS`: how frequently (ms) to check for mid-price changes and re-quote.

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

    // 3) Every PACE_MS: if the mid‐price has changed since our last quote,
    //    cancel the old bid/ask and post fresh ones around the new mid.
    let client = reqwest::Client::new();
    let mut outstanding: Vec<u64> = Vec::new();
    let mut interval = time::interval(time::Duration::from_millis(PACE_MS));
    let mut last_mid = None;
    loop {
        interval.tick().await;

        // Only quote once we have a mid-price
        if let Some(mid_price) = *mid_rx.borrow() {
            if Some(mid_price) != last_mid {
                //market has moved, cancel & place new orders, and update mid price
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
                last_mid = Some(mid_price);
            }
        }
    }
}
