use axum::Router;
use clap::{Parser, Subcommand};
use order_book_engine::instrument::{Asset, Pair};
use order_book_engine::utils::shutdown_token;
use order_book_engine::{api, instrument, market_maker, simulate, state::AppState};
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use tokio::net::TcpListener;
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
    Simulate { port: u16, secs: u64 },
    Server { port: u16 },
}

async fn wait_for_server(api_base: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    loop {
        match client
            .get(format!(
                "{}/book/{}",
                api_base,
                Pair::crypto_usd(Asset::BTC).code()
            ))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => break,
            _ => tokio::time::sleep(Duration::from_millis(25)).await,
        };
    }
    Ok(())
}
async fn seed_book(ep: &str) -> anyhow::Result<()> {
    // Seed the book with a resting bid @48 and ask @52
    let client = reqwest::Client::new();
    for (side, price) in &[("Buy", 48), ("Sell", 52)] {
        client
            .post(format!("{}/orders", ep))
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
        tracing::info!(side, price, "seeded resting orders");
    }
    Ok(())
}

async fn get_app_listener(port: u16, state: AppState) -> anyhow::Result<(TcpListener, Router)> {
    let app = api::router(state);
    let ep = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(ep.clone()).await?;
    Ok((listener, app))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let state = AppState::new(Path::new("trade_store")).await.unwrap();
    let token = shutdown_token();
    let server_token = token.clone();
    let mm_token = token.clone();
    let sim_token = token.clone();
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
        Commands::Simulate { port, secs } => {
            let mut handlers = tokio::task::JoinSet::new();
            let (listener, app) = get_app_listener(port, state.clone()).await.unwrap();
            tracing::warn!("spawning the server task, port: {}, {}", port, secs);
            handlers.spawn(async move {
                tracing::info!(
                    "HTTP/WS server listening on {}",
                    format!("0.0.0.0:{}", port)
                );
                // this will serve forever
                axum::serve(listener, app)
                    .with_graceful_shutdown(server_token.cancelled_owned())
                    .await
                    .unwrap();
            });
            let ep = format!("{}:{}", base.clone(), port);
            tracing::info!("end_point: {}", ep);
            wait_for_server(&ep).await?;
            seed_book(&ep).await.unwrap();
            let pair = Pair::crypto_usd(instrument::Asset::BTC);
            //start market maker
            let mmb = base.clone();
            handlers.spawn(async move {
                if let Err(e) = market_maker::run_market_maker(&mmb, pair, mm_token).await {
                    tracing::error!("Market maker exited: {:?}", e);
                }
            });
            //start simulator
            handlers.spawn(async move {
                if let Err(e) = simulate::run_simulation(
                    simulate::SimConfig {
                        api_base: base,
                        run_secs: if secs == 0 { None } else { Some(secs) },
                        attack_rate_hz: 5,
                    },
                    sim_token,
                )
                .await
                {
                    tracing::error!("Simulation error: {:?}", e);
                }
            });
            handlers.join_all().await;
        }
        Commands::Server { port } => {
            let (listener, app) = get_app_listener(port, state.clone()).await.unwrap();
            let svh = tokio::spawn(async move {
                tracing::info!(
                    "HTTP/WS server listening on {}",
                    format!("0.0.0.0:{}", port)
                );
                axum::serve(listener, app)
                    .with_graceful_shutdown(server_token.cancelled_owned())
                    .await
                    .unwrap();
            });
            svh.await?;
        }
    };
    Ok(())
}
