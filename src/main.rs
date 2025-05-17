use state::AppState;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

pub mod api;
pub mod cli;
pub mod errors;
pub mod market_maker;
pub mod orderbook;
pub mod orders;
pub mod state;
pub mod trade;

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::TRACE)
        // completes the builder.
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    let state = AppState::new();
    {
        let api_base = "http://127.0.0.1:3000".to_string();

        tokio::spawn(async move {
            if let Err(e) = market_maker::run_market_maker(&api_base).await {
                tracing::error!("Market maker exited: {:?}", e);
            }
        });
    }
    let app = api::router(state);
    tracing::info!("running on http://localhost:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap()
}
