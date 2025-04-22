use state::AppState;

pub mod api;
pub mod cli;
pub mod orderbook;
pub mod orders;
pub mod state;
pub mod trade;

#[tokio::main]
async fn main() {
    let state = AppState::new();
    let app = api::router(state);
    println!("running on http://localhost:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap()
}
