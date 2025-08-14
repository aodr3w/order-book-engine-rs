use crate::instrument::Pair;
use errors::MarketMakerError;
use futures_util::StreamExt;
use reqwest;
use serde::{Deserialize, Serialize};
use tokio::{sync::watch, time};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMsg};
use tokio_util::sync::CancellationToken;

use crate::{
    api::{OrderAck, WsFrame},
    errors,
    orderbook::BookSnapshot,
    orders::{OrderType, Side},
};

// # Market Maker Bot
//
// Think of this bot as a friendly shopkeeper who always posts both a buy-price and a sell-price
// for a given market pair. It listens to live order-book updates, finds the midpoint between the
// best bid and ask, and continually refreshes its own two-sided quotes just above and below that
// midpoint—providing liquidity for traders and capturing small spreads as profit.
//
// ## At a Glance (Non-Technical)
// - **Always visible:** Posts a buy order a little below the market mid-price, and a sell order a
//   little above it, so anyone can trade immediately.
// - **Lightweight:** Only updates its quotes when the midpoint actually moves, avoiding extra work
//   or fees.
// - **Steady profit:** The difference between its buy and sell prices (the **spread**) is how it
//   earns a tiny bit each time someone hits its quote.
//
// ## How It Works (Technical)
// 1. **Connect** to your engine’s WebSocket feed (`/ws`) and receive `BookSnapshot { pair, bids, asks }`.
// 2. **Compute** the mid-price:
//    ```text
//    mid = (best_bid + best_ask) / 2
//    ```
// 3. **Every PACE_MS milliseconds** (default 500 ms), *if* the midpoint has changed since last time:
//    - **Cancel** previously posted buy & sell orders to avoid stale quotes.
//    - **Place** two fresh **limit** orders via REST:
//      - **Buy** at `(mid_price - SPREAD)`
//      - **Sell** at `(mid_price + SPREAD)`
//    - **Remember** their order IDs so you can cancel them cleanly on the next cycle.
//
// ## Key Parameters
// - `SPREAD: u64` — how far from the midpoint to quote.
//   - Larger → greater profit per fill, but fewer fills.
//   - Smaller → tighter market, but slimmer profit.
// - `PACE_MS: u64` — how often (ms) to refresh quotes.
//   - Faster → ultra-fresh quotes, but more cancellations/posts (and API calls).
//   - Slower → less chatter, but you may miss rapid market moves.
//
// ## Why It Works
// - **Two-Sided Liquidity:** Always having both bid and ask visible narrows spreads and attracts flow.
// - **Efficient Churn:** Only react to real mid-price moves, avoiding needless cancel/post cycles.
// - **Simple Model:** Fixed spread and interval make P&L predictable and coding straightforward.
//
// ## Under the Hood
// - A **WebSocket** task parses `BookSnapshot` frames and sends midpoint updates into a
//   `tokio::watch` channel.
// - A **Quoting** loop ticks on a `tokio::time::interval`; it reads the latest mid-price, cancels
//   old orders, and posts new ones with `reqwest`.
// - All HTTP and WS errors are wrapped in `MarketMakerError` for clean upstream handling.
//

// // how far from mid to quote
const SPREAD: u64 = 2;
// // how many milliseconds between quote updates
const PACE_MS: u64 = 500;

// A small helper to serialize outgoing orders
#[derive(Deserialize, Serialize)]
struct NewOrder {
    side: Side,
    order_type: OrderType,
    price: Option<u64>,
    quantity: u64,
    symbol: String,
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
///      - **Buy** at `(mid_price - SPREAD)` buy low
///      - **Sell** at `(mid_price + SPREAD)` sell high
///    - Records the returned `order_id`s so they can be cancelled on the
///      next iteration.
///
/// Errors from the WebSocket connection or HTTP client are wrapped in
/// `MarketMakerError` for upstream handling.
pub async fn run_market_maker(
    api_base: &str,
    target_pair: Pair,
    token: CancellationToken,
) -> Result<(), MarketMakerError> {
    //use pair-specific websocket URL
    let ws_url = format!(
        "ws://{host}/ws/{pair}",
        host = api_base.trim_start_matches("http://"),
        pair = target_pair.code()
    );
    tracing::warn!("market maker: connecting to: {:?}", ws_url);
    // 1) Subscribe to /ws
    let ws_stream = loop {
        match connect_async(&ws_url).await {
            Ok((stream, _)) => {
                tracing::info!("market maker: ws connected successfully");
                break stream;
            }
            Err(e) => {
                tracing::warn!("market maker: ws connect failed: {}; retrying...", e);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await
            }
        }
    };

    let (_write, read) = ws_stream.split();

    // watch channel for mid_price
    let (mid_tx, mid_rx) = watch::channel(None::<u64>);

    // 2) Spawn task: parse snapshots → update `mid_tx`
    let v = target_pair.clone();

    let frames = read.filter_map(|msg| async move {
        match msg {
            Ok(WsMsg::Text(txt)) => match serde_json::from_str::<WsFrame>(&txt) {
                Ok(frame) => Some(frame),
                Err(err) => {
                    tracing::warn!("invalid WS frame: {err}");
                    None
                }
            },
            _ => None,
        }
    });
    tokio::spawn(async move {
        tokio::pin!(frames);
        while let Some(frame) = frames.next().await {
            if let WsFrame::BookSnapshot(BookSnapshot { pair, bids, asks }) = frame {
                if pair != v {
                    continue;
                }

                if let (Some((bb, _)), Some((aa, _))) = (bids.first(), asks.first()) {
                    let mid = (bb + aa) / 2;
                    let _ = mid_tx.send(Some(mid));
                }
            };
        }
    });

    // 3) Every PACE_MS: if the mid‐price has changed since our last quote,
    //    cancel the old bid/ask and post fresh ones around the new mid.
    let client = reqwest::Client::new();
    let mut outstanding: Vec<u128> = Vec::new();
    let mut interval = time::interval(time::Duration::from_millis(PACE_MS));
    let mut last_mid = None;
    loop {
        tokio::select! {
                //cancellation wins instantly
                _ = token.cancelled() => {
                    tracing::info!("market makerL shutdown requested, tearing down...");
                    break;
                }
                _ = interval.tick() => {
                            // Only quote once we have a mid-price

            let mid_opt: Option<u64> = *mid_rx.borrow();
            if let Some(mid_price) = mid_opt {
                if Some(mid_price) != last_mid {
                    //market has moved, cancel & place new orders, and update mid price
                    // Cancel all previous orders
                    for id in outstanding.drain(..) {
                        let _ = client
                            .delete(format!("{}/orders/{}/{}", api_base, target_pair.code(), id))
                            .send()
                            .await;
                    }
                    tracing::info!(bid_price = mid_price.saturating_sub(SPREAD), "placing bid");
                    // Post a new bid
                    if let Ok(resp) = client
                        .post(format!("{}/orders", api_base))
                        .json(&NewOrder {
                            side: Side::Buy,
                            order_type: OrderType::Limit,
                            price: Some(mid_price.saturating_sub(SPREAD)),
                            quantity: 1,
                            symbol: target_pair.code(),
                        })
                        .send()
                        .await
                    {
                        if let Ok(ack) = resp.json::<OrderAck>().await {
                            outstanding.push(ack.order_id);
                        }
                    }
                    tracing::info!(bid_price = mid_price.saturating_add(SPREAD), "placing ask");
                    // Post a new ask
                    if let Ok(resp) = client
                        .post(format!("{}/orders", api_base))
                        .json(&NewOrder {
                            side: Side::Sell,
                            order_type: OrderType::Limit,
                            price: Some(mid_price.saturating_add(SPREAD)),
                            quantity: 1,
                            symbol: target_pair.code(),
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
    }
    Ok(())
}
