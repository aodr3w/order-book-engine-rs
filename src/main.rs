use order_book_engine::{api, instrument, market_maker, simulate, state::AppState};
use serde_json::json;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use order_book_engine::instrument::Pair;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Setup tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // The base URL our clients (Market Maker & Simulator) will use
    let api_base = "http://127.0.0.1:3000".to_string();

    // Launch our Axum server in the background
    let state = AppState::new().await;
    let app = api::router(state.clone());
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    let server_handle = tokio::spawn(async move {
        tracing::info!("HTTP/WS server listening on http://0.0.0.0:3000");
        // this will serve forever
        axum::serve(listener, app).await.unwrap();
    });

    // small delay to let the server finish its bind
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Seed the book with a resting bid @48 and ask @52
    let client = reqwest::Client::new();
    for (side, price) in &[("Buy", 48), ("Sell", 52)] {
        client
            .post(format!("{}/orders", api_base))
            .json(&json!({
                "side": side,
                "order_type": "Limit",
                "price": price,
                "quantity": 10,
                "symbol": Pair::crypto_usd(instrument::Asset::BTC).code()
            }))
            .send()
            .await?
            .error_for_status()?;
        tracing::info!(side, price, "seeded resting order");
    }

    // Spawn the market maker
    let pair = Pair::crypto_usd(instrument::Asset::BTC);
    let b = api_base.clone();
    let mm_handler = tokio::spawn(async move {
        if let Err(e) = market_maker::run_market_maker(&b, pair).await {
            tracing::error!("Market maker exited: {:?}", e);
        }
    });

    // Spawn the attacker simulation
    let sim_cfg = simulate::SimConfig {
        api_base,
        run_secs: 10,
        attack_rate_hz: 5,
    };
    let sim_handle = tokio::spawn(async move {
        if let Err(e) = simulate::run_simulation(sim_cfg).await {
            tracing::error!("Simulation error: {:?}", e);
        }
    });

    let _ = tokio::join!(server_handle, sim_handle, mm_handler);
    Ok(())
}
