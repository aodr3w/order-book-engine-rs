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
use reqwest::Client;
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::time::interval;
use tokio_util::sync::CancellationToken;

use crate::instrument::{self, Pair};

#[derive(Clone)]
pub struct SimConfig {
    pub api_base: String,
    pub run_secs: Option<u64>,
    pub attack_rate_hz: u64,
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
pub async fn run_simulation(cfg: SimConfig, token: CancellationToken) -> anyhow::Result<()> {
    let client = Client::new();
    let mut iv = 0i64; //inventory
    //ticker for pacing
    let mut ticker = interval(Duration::from_millis(1000 / cfg.attack_rate_hz));
    let mut realized_pnl = 0.0f64;
    // //pin signal feature to singal thread
    // let sigint = tokio::signal::ctrl_c();
    // tokio::pin!(sigint);
    let start = Instant::now();
    loop {
        tokio::select! {
            //your paced tick
            _ = ticker.tick() => {
                if let Some(max_secs) = cfg.run_secs {
                    if start.elapsed().as_secs() >= max_secs {
                        break;
                    }
                }
            send_one_order(
                &client,
                &cfg.api_base,
                &mut iv,
                &mut realized_pnl
            ).await?;
            }
            //handle termination signal
            _ = token.cancelled()  => {
                tracing::info!("👍 caught Ctrl-C, exiting simulation…");
                break
            }
        }
    }
    // end of loop
    println!("--- Simulation complete ---");
    println!("Final inventory: {}", iv);
    println!("Final realized P&L: {:.4}", realized_pnl);
    Ok(())
}
