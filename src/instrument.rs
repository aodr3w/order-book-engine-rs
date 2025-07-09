use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Asset {
    BTC,
    USD,
}

//A Trading pair: base/quote
pub struct Pair {
    /// The asset you buy or sell
    pub base: Asset,
    /// The asset you pay or receive
    pub quote: Asset,
}
impl Pair {
    /// Returns the usual string code, e.g "BTC-USD"
    pub fn code(&self) -> String {
        format!("{:?}-{:?}", self.base, self.quote)
    }
    ///crypto-USD factory spot pairs
    pub fn crypto_usd(base: Asset) -> Self {
        Pair {
            base,
            quote: Asset::USD,
        }
    }
}

// TODO define pairs here
// BTC_USD = Pair {base: ..., quote: ....}
