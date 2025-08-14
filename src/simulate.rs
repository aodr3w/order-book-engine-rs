//! Simulation harness for noisy order flow against the engine.

use rand::Rng; // for rng().random_bool()
use rand_distr::{Distribution, Exp, Exp1, Normal};
use reqwest::{Client, ClientBuilder};
use serde::Deserialize;
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::instrument::Pair;

#[derive(Clone)]
pub struct SimConfig {
    pub api_base: String,
    pub pair: Pair, // <— no more hard-coded symbol
    pub run_secs: Option<u64>,
    pub attack_rate_hz: f64, // Poisson rate λ
    pub noise_sigma: f64,    // N(0, σ) drift applied to mid each tick
    pub mean_qty: f64,       // average order size (unit-exp * mean_qty)
                             // optional tweaks you can expose later:
                             // pub timeout_secs: Option<u64>,
                             // pub spread: f64,
}

#[derive(Deserialize)]
struct Ack {
    trades: Vec<AckTrade>,
}

#[derive(Deserialize)]
struct AckTrade {
    price: u64,
    quantity: u64,
}

/// Fire a single **market** order of size 1, update inventory/P&L.
/// Kept close to your original helper but now typed.
pub async fn send_one_order(
    client: &Client,
    api_base: &str,
    pair: &Pair,
    iv: &mut i64,
    pnl: &mut f64,
) -> anyhow::Result<()> {
    let side = if rand::rng().random_bool(0.5) {
        "Buy"
    } else {
        "Sell"
    };

    let resp = client
        .post(format!("{}/orders", api_base))
        .json(&json!({
            "side": side,
            "order_type": "Market",
            "quantity": 1u64,
            "symbol": pair.code(),
        }))
        .send()
        .await?
        .error_for_status()?;

    let ack: Ack = resp.json().await?;

    for t in ack.trades {
        let price = t.price as f64;
        let qty = t.quantity as f64;
        if side == "Buy" {
            *iv -= qty as i64; // maker sold to us
            *pnl += price * qty;
        } else {
            *iv += qty as i64; // maker bought from us
            *pnl -= price * qty;
        }
    }
    Ok(())
}

/// Noisy limit-order simulation loop.
pub async fn run_simulation(cfg: SimConfig, cancel_token: CancellationToken) -> anyhow::Result<()> {
    // A small client timeout is helpful under load; tweak as desired.
    let client: Client = ClientBuilder::new()
        .timeout(Duration::from_secs(5))
        .build()?;

    let ia = Exp::new(cfg.attack_rate_hz).expect("attack_rate_hz must be > 0");
    let drift = Normal::new(0.0, cfg.noise_sigma).expect("noise_sigma >= 0");
    let size = Exp1;

    let mut iv: i64 = 0;
    let mut pnl: f64 = 0.0;
    let mut mid: f64 = 50.0;
    let start = Instant::now();

    // Choose your quoting spread here
    let spread = 1.0_f64;

    loop {
        // hard stop
        if let Some(max_secs) = cfg.run_secs {
            if start.elapsed().as_secs() >= max_secs {
                break;
            }
        }

        // exponential inter-arrival
        let wait_secs: f64 = ia.sample(&mut rand::rng());
        tokio::select! {
            _ = cancel_token.cancelled() => {
                info!("received shutdown; exiting simulation loop");
                break;
            }
            _ = sleep(Duration::from_secs_f64(wait_secs)) => {}
        }

        // size ~ Exp1 * mean_qty  (and round to >= 1)
        let unit: f64 = size.sample(&mut rand::rng());
        let qty_u64 = (unit * cfg.mean_qty).max(1.0).round() as u64;

        // mid drift
        mid += drift.sample(&mut rand::rng());

        // quote around mid
        let (price_u64, side) = if rand::rng().random_bool(0.5) {
            (mid - spread, "Buy")
        } else {
            (mid + spread, "Sell")
        };
        // sanitize price for the engine
        let price_u64 = price_u64.max(1.0).round() as u64;

        // place the order; on failure, warn and continue
        match client
            .post(format!("{}/orders", cfg.api_base))
            .json(&json!({
                "side": side,
                "order_type": "Limit",
                "price": price_u64,
                "quantity": qty_u64,
                "symbol": cfg.pair.code(),
            }))
            .send()
            .await
        {
            Ok(resp) => {
                if let Err(e) = resp.error_for_status_ref() {
                    warn!(error = %e, "order post returned non-success");
                    continue;
                }
                match resp.json::<Ack>().await {
                    Ok(ack) => {
                        for t in ack.trades {
                            let px = t.price as f64;
                            let q = t.quantity as f64;
                            if side == "Buy" {
                                iv -= q as i64;
                                pnl += px * q;
                            } else {
                                iv += q as i64;
                                pnl -= px * q;
                            }
                        }
                        info!(
                            elapsed = format_args!("{:.1}s", start.elapsed().as_secs_f64()),
                            side,
                            qty = qty_u64,
                            price = price_u64,
                            mid = format_args!("{:.2}", mid),
                            inventory = iv,
                            pnl = format_args!("{:.2}", pnl),
                            "sim tick"
                        );
                    }
                    Err(e) => warn!(error = %e, "failed to parse Ack JSON"),
                }
            }
            Err(e) => {
                warn!(error = %e, "HTTP request failed");
                continue;
            }
        }
    }

    info!(
        inventory = iv,
        pnl = format_args!("{:.2}", pnl),
        "simulation done"
    );
    Ok(())
}
