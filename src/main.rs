use clap::{Parser, Subcommand};
use order_book_engine::instrument::Pair;
use order_book_engine::{api, instrument, market_maker, simulate, state::AppState};
use serde_json::json;
use std::path::Path;
use tokio::task::JoinHandle;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "engine-cli")]
#[command(
    author = "Andrew Odiit",
    version = "0.1",
    about = "A demo of a limit-order-book-engine"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Simulate { port: i32 },
    Server { port: i32 },
}

async fn run_simulation(api_base: String) -> anyhow::Result<Vec<JoinHandle<()>>> {
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
    Ok(vec![sim_handle, mm_handler])
}

async fn run_server(port: i32) -> anyhow::Result<JoinHandle<()>> {
    // Launch our Axum server in the background
    let state = AppState::new(Path::new("trade_store")).await.unwrap();
    let app = api::router(state.clone());
    let ep = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(ep.clone()).await?;
    let server_handle = tokio::spawn(async move {
        tracing::info!("HTTP/WS server listening on {}", ep);
        // this will serve forever
        axum::serve(listener, app).await.unwrap();
    });

    // small delay to let the server finish its bind
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    Ok(server_handle)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Setup tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    // The base URL our clients (Market Maker & Simulator) will use

    let cli = Cli::parse();
    let base = "http://127.0.0.1".to_string();
    match cli.command {
        //runs system with market_maker bot && client
        Commands::Simulate { port } => {
            let svh = run_server(port).await?;
            let mut sh = run_simulation(format!("{}:{}", base, port)).await?;
            sh.push(svh);
            for j in sh {
                j.await?;
            }
        }
        Commands::Server { port } => {
            let svh = run_server(port).await?;
            svh.await?;
        }
    };
    Ok(())
}
