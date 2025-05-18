use rand::Rng;
use reqwest::Client;
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::time::interval;

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
            .json(&json!({"side": side, "order_type": "Market", "quantity": 1}))
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
