use thiserror::Error;

#[derive(Error, Debug)]
pub enum MarketMakerError {
    #[error("connection error")]
    ConnectError(String),
}
