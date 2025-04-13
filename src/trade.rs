use std::time::SystemTime;

/// A trade represents a matched transaction between two orders
///
/// - The price comes from the makers order (i.e resting order)
/// - Quantity is the amount filled
/// - the taker is the incoming order that triggered the trade.

#[derive(Debug, Clone)]
pub struct Trade {
    pub price: u64,
    pub quantity: u64,
    pub maker_id: u64,
    pub taker_ud: u64,
    pub timestamp: SystemTime,
}
