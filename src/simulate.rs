//! Simulation harness for testing the Market Maker under load.
//!
//! Continuously sends random market orders against the API to:
//! 1. Measure the Market Maker’s performance (P&L, inventory).
//! 2. Stress-test quoting logic under varying order arrival rates.
//!
//! ## Components
//!
//! - `SimConfig` holds the simulation parameters:
//!   - `api_base`: base URL of the REST API (e.g. `http://127.0.0.1:3000`).
//!   - `run_secs`: total duration of the simulation in seconds.
//!   - `attack_rate_hz`: rate (orders per second) at which to send market orders.
//!
//! - `run_simulation(cfg)`: the main async function that:
//!   1. Creates an HTTP client.
//!   2. Tracks a simulated trader’s **inventory** (`iv`) and **realized P&L** (`realized_pnl`).
//!   3. Sets up a Tokio interval to pace market orders at `attack_rate_hz`.
//!   4. For each tick until `run_secs` elapse:
//!      - Randomly choose a side (`"Buy"` or `"Sell"`).
//!      - Send a market order of size 1 via `POST /orders`.
//!      - Parse the response trades, and update inventory/P&L:
//!        - If the sim side is **Buy**, the MM sold: sim inventory--, sim receives price → realzied_pnl += price.
//!        - If **Sell**, MM bought: sim inventory++, sim pays price → realized_pnl -= price.
//!   5. After completion, prints summary of realized P&L and ending inventory.
//!
//! ## Rationale
//!
//! - **Random aggression** models external market flow against which the MM must provide liquidity.
//! - **Market orders** ensure the MM’s quotes are tested: aggressors hit the best bid/ask.
//! - Tracking **inventory** and **realized P&L** provides key metrics to evaluate the MM’s profitability
//!   and risk exposure over time.
//! - Adjustable **attack_rate_hz** allows us to simulate both low-frequency and high-frequency
//!   market environments.

//! # Simulation Harness
//!
//! This module provides a simple **adversarial simulation** that attacks the market maker
//! with randomized, aggressive market orders to measure its realized profit and inventory risk.
//!
//! ## What it does
//! 1. Sends market orders of size 1 at a configurable **attack rate** (`attack_rate_hz`) for a total
//!    duration (`run_secs`).
//! 2. Randomly chooses **Buy** or **Sell** side for each order to probe both sides of the MM’s quotes.
//! 3. Parses the MM’s response (the `trades` array) to determine fills: if any trades occur, the
//!    simulator was the taker and the MM was the maker.
//! 4. Updates simple **P&L** and **inventory** counters:
//!    - **Buy** market order → simulator buys 1 unit (MM sells), so inventory ↓ by 1,
//!      P&L ↑ by `price * 1`.
//!    - **Sell** market order → simulator sells 1 unit (MM buys), so inventory ↑ by 1,
//!      P&L ↓ by `price * 1`.
//!
//! ## Why size = 1?
//! - **Fine‑grained probing:** unit‐sized orders isolate single‐tick fills, making it easy to see
//!   which side of the MM’s two‑sided quote was hit without crossing multiple levels.
//! - **Simple accounting:** each trade moves inventory by exactly one unit, letting P&L be computed
//!   as `±price` per trade with no need for aggregation or partial‐fill logic.

use rand::Rng;
use rand_distr::{Distribution, Exp, Exp1, Normal};
use reqwest::Client;
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::instrument::{self, Pair};

#[derive(Clone)]
pub struct SimConfig {
    pub api_base: String,
    pub run_secs: Option<u64>,
    pub attack_rate_hz: f64,
    pub noise_sigma: f64,
    pub mean_qty: f64,
}
pub async fn send_one_order(
    client: &Client,
    api_base: &str,
    iv: &mut i64,
    pnl: &mut f64,
) -> anyhow::Result<()> {
    let side = if rand::rng().random_bool(0.5) {
        "Buy"
    } else {
        "Sell"
    };
    //post
    let resp = client
        .post(format!("{}/orders", api_base))
        .json(&json!(
            {
                "side": side,
                "order_type": "Market",
                "quantity": 1,
                "symbol": Pair::crypto_usd(instrument::Asset::BTC).code(),
            }
        ))
        .send()
        .await?
        .error_for_status()?;

    let ack = resp.json::<serde_json::Value>().await?;
    //update metrics
    if let Some(trades) = ack.get("trades").and_then(|t| t.as_array()) {
        for tr in trades {
            let price = tr["price"].as_f64().unwrap();
            let qty = tr["quantity"].as_f64().unwrap();
            //metrics for our market-maker
            if side == "Buy" {
                //mm's inventory drops
                *iv -= qty as i64;
                *pnl += price * qty;
            } else {
                //mm's inventory goes up and pnl goes down
                *iv += qty as i64;
                *pnl -= price * qty;
            }
        }
    }
    Ok(())
}

/// Drive a “noisy” adversarial simulation against the order-book engine.
///
/// Spawns a background task that:
/// 1. Draws inter-arrival delays from an Exponential(rate = `cfg.attack_rate_hz`), modelling
///    a Poisson stream of incoming orders.
/// 2. On each “tick”:
///    - Draws a random order size by sampling `Exp1 * cfg.mean_qty`.
///    - Applies Gaussian drift `N(0, cfg.noise_sigma)` to a local `mid_price`.
///    - Places a **Limit** order at `mid_price ± spread` (you can adjust spread).
///    - Parses any fills in the engine’s response, updating P&L and inventory counters.
/// 3. Stops after `cfg.run_secs` elapse (if set), or immediately if the provided
///    `cancel_token` is triggered (e.g. on Ctrl–C).
///
/// # Parameters
/// - `cfg`: simulation parameters (API endpoint, duration, arrival rate, noise, average size).
/// - `cancel_token`: a `CancellationToken` whose cancellation immediately terminates the loop.
///
/// # Side Effects
/// Continuously issues HTTP requests against `cfg.api_base`, logging inventory/P&L to stdout.
/// When the loop exits, prints a final summary.
///
/// # Errors
/// Returns an error if any HTTP request fails or if distribution setup (e.g. negative σ) is invalid.
///
pub async fn run_simulation(cfg: SimConfig, cancel_token: CancellationToken) -> anyhow::Result<()> {
    let client = Client::new();
    //1) Exponential inter-arrival times with rate = attack_rate_hz
    let ia_dist = Exp::new(cfg.attack_rate_hz).expect("attack_rate_hz must be > 0");

    //2 Gaussian drift on the mid-price
    let drift = Normal::new(0.0, cfg.noise_sigma).expect("noise sigma >= 0");

    //3 unit expontential for sizing
    let size_dist = Exp1;

    let mut iv = 0i64;
    let mut pnl = 0.0f64;
    let mut mid_price = 50.0;
    let start = Instant::now();

    loop {
        //check overall time-limit
        if let Some(max_secs) = cfg.run_secs {
            if start.elapsed().as_secs() >= max_secs {
                break;
            }
        }
        //draw the next wait
        let wait_secs = ia_dist.sample(&mut rand::rng());
        let sleep_fut = sleep(Duration::from_secs_f64(wait_secs));
        tokio::select! {
                //user hits ctrl-c
        _ = cancel_token.cancelled() => {
                    tracing::info!("👍 received shutdown, exiting noisy sim…");
                break;
                }
        // our dynamically-drawn elapsed
        _ = sleep_fut => {
            // 4) Send the market order
            let raw: f64 = <Exp1 as Distribution<f64>>::sample(&size_dist, &mut rand::rng());
            let qty = raw * cfg.mean_qty;
            //drift mid price
            mid_price += drift.sample(&mut rand::rng());
            // now place a limit order around that drifted price ± spread
            let spread = 1.0;
            let (price, side) = if rand::rng().random_bool(0.5) {
                (mid_price - spread, "Buy")
            } else {
                (mid_price + spread, "Sell")
            };
            let resp = client.post(format!("{}/orders", cfg.api_base))
                  .json(&json!({
                      "side": side,
                      "order_type": "Limit",
                      "price": price as u64,
                      "quantity": qty as u64,
                      "symbol": "BTC-USD",
                  }))
              .send()
                .await?
                .error_for_status()?
                .json::<serde_json::Value>()
                .await?;
            // 6) Update P&L & inventory
            if let Some(trades) = resp.get("trades").and_then(|t| t.as_array()) {
                for tr in trades {
                    let price = tr["price"].as_f64().unwrap();
                    let q     = tr["quantity"].as_f64().unwrap();
                    if side == "Buy" {
                        iv  -= q as i64;
                        pnl += price * q;
                    } else {
                        iv  += q as i64;
                        pnl -= price * q;
                    }
                }
            }

            println!(
                "[{:.1}s] side={} qty={:.2} mid={:.2} inv={} pnl={:.2}",
                start.elapsed().as_secs_f64(),
                side, qty, mid_price, iv, pnl
            );
                }
            }
    }
    println!("--- done --- final inv={} final pnl={:.2}", iv, pnl);
    Ok(())
}
