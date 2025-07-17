use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Asset {
    BTC,
    USD,
    ETH,
}

//A Trading pair: base/quote
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
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
    pub fn supported() -> &'static [Pair] {
        &[BTC_USD, ETH_USD]
    }
}
impl FromStr for Pair {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Pair::supported()
            .iter()
            .find(|p| p.code() == s)
            .cloned()
            .ok_or_else(|| format!("unsupported symbol: `{}`", s))
    }
}

// TODO define pairs here
// BTC_USD = Pair {base: ..., quote: ....}
pub const BTC_USD: Pair = Pair {
    base: Asset::BTC,
    quote: Asset::USD,
};
pub const ETH_USD: Pair = Pair {
    base: Asset::ETH,
    quote: Asset::USD,
};
