use std::time::SystemTime;

/// A trade represents a matched transaction between two orders.
///
/// # Terminology
/// - **Maker**: The order that was already resting in the order book (providing liquidity).
///   - Can be either a Buy (bid) or Sell (ask) order.
/// - **Taker**: The incoming order that triggered the trade (taking liquidity).
///   - Can also be a Buy or Sell order.
///
/// # Behavior
/// - The trade always executes at the **maker's price** (book price).
/// - Partial fills may occur: multiple trades can be generated from one order.
///
/// Example:
/// - A market buy order (taker) matches a limit sell at 102 (maker).
/// - A trade is created at price 102.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, bincode::Encode, bincode::Decode)]
pub struct Trade {
    pub price: u64,
    pub quantity: u64,
    pub maker_id: u128,
    pub taker_id: u128,
    pub timestamp: SystemTime,
    pub symbol: String,
}
