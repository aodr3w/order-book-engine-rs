use std::time::SystemTime;

/// Represents which side of the market the order is on.
///
/// # Intuition
/// - `Buy` (Bid): The trader wants to purchase the asset. Buy orders are sorted from **highest to lowest price**
///   because a higher price means more willingness to buy — i.e., more aggressive.
/// - `Sell` (Ask): The trader wants to sell the asset. Sell orders are sorted from **lowest to highest price**
///   because a lower price means more willingness to sell — i.e., more aggressive.
///
/// This sorting ensures the matching engine always finds the **best price first**:
/// - Buyers match with the **lowest ask**
/// - Sellers match with the **highest bid**
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub enum Side {
    Buy,  // Bid
    Sell, // Ask
}

/// Specifies whether an order is a Limit or Market order.
///
/// - `Limit`: Executes at a specific price or better
/// - `Market`: Executes immediately at the best available price
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub enum OrderType {
    Limit,
    Market,
}

/// An order submitted by a trader.
///
/// - `price` is optional for market orders
/// - `timestamp` is used for time-priority (FIFO within price level)
#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<u64>,
    pub quantity: u64,
    pub timestamp: SystemTime,
}
