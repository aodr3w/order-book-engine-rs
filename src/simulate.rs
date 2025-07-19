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

use crate::instrument::{self, Pair};

#[derive(Clone)]
pub struct SimConfig {
    pub api_base: String,
    pub run_secs: u64,
    pub attack_rate_hz: u64,
}

pub async fn run_simulation(cfg: SimConfig) -> anyhow::Result<()> {
    let client = Client::new();
    let mut iv = 0i64; //inventory
    let mut realized_pnl = 0.0f64;
    let mut attack_int = interval(Duration::from_millis(1000 / cfg.attack_rate_hz));
    let start = Instant::now();

    while start.elapsed().as_secs() < cfg.run_secs {
        attack_int.tick().await;
        //Random side
        let side = if rand::rng().random_bool(0.5) {
            "Buy"
        } else {
            "Sell"
        };
        //Aggressive market order of size=1
        let resp = client
            .post(format!("{}/orders", cfg.api_base))
            .json(&json!({"side": side, "order_type": "Market", "quantity": 1, "symbol": Pair::crypto_usd(instrument::Asset::BTC).code()}))
            .send()
            .await?;
        let ack: serde_json::Value = resp.json().await?;
        //if we got a trade , record inventory & pnl
        if let Some(trades) = ack.get("trades").and_then(|t| t.as_array()) {
            for tr in trades {
                let price = tr.get("price").unwrap().as_f64().unwrap();
                let qty = tr.get("quantity").unwrap().as_f64().unwrap();
                //MM was maker if we hit its quote
                if side == "Buy" {
                    // you bought → MM sold
                    iv -= qty as i64;
                    realized_pnl += price * qty;
                } else {
                    // you sold → MM bought
                    iv += qty as i64;
                    realized_pnl -= price * qty;
                }
            }
        }
    }

    println!("--- Simulation complete ---");
    println!("Realized P&L: {:.4}", realized_pnl);
    println!("Ending Inv.:  {}", iv);
    Ok(())
}
