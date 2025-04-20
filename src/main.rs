use cli::run_cli;

pub mod cli;
pub mod orderbook;
pub mod orders;
pub mod state;
pub mod trade;
fn main() {
    run_cli();
}
