use serde_json::json;
use state::AppState;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

pub mod api;
pub mod cli;
pub mod errors;
pub mod market_maker;
pub mod orderbook;
pub mod orders;
pub mod simulate;
pub mod state;
pub mod trade;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // --- 0) Setup tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // the base URL our clients (MM & sim) will use
    let api_base = "http://127.0.0.1:3000".to_string();

    // --- 1) Launch our Axum server in the background
    let state = AppState::new().await;
    let app = api::router(state.clone());
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    tokio::spawn(async move {
        tracing::info!("HTTP/WS server listening on http://0.0.0.0:3000");
        // this will serve forever
        axum::serve(listener, app).await.unwrap();
    });

    // small delay to let the server finish its bind
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // --- 2) Seed the book with a resting bid @48 and ask @52
    let client = reqwest::Client::new();
    for (side, price) in &[("Buy", 48), ("Sell", 52)] {
        client
            .post(format!("{}/orders", api_base))
            .json(&json!({
                "side": side,
                "order_type": "Limit",
                "price": price,
                "quantity": 10
            }))
            .send()
            .await?
            .error_for_status()?;
        tracing::info!(side, price, "seeded resting order");
    }

    // --- 3) Spawn the market maker
    {
        let b = api_base.clone();
        tokio::spawn(async move {
            if let Err(e) = market_maker::run_market_maker(&b).await {
                tracing::error!("Market maker exited: {:?}", e);
            }
        });
    }

    // --- 4) Spawn the attacker simulation
    let sim_cfg = simulate::SimConfig {
        api_base,
        run_secs: 10,
        attack_rate_hz: 5,
    };
    tokio::spawn(async move {
        if let Err(e) = simulate::run_simulation(sim_cfg).await {
            tracing::error!("Simulation error: {:?}", e);
        }
    });

    // Prevent main from exiting
    futures::future::pending::<()>().await;
    Ok(())
}
