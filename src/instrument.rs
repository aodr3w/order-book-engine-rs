use std::str::FromStr;

use serde::{Deserialize, Serialize, Serializer, de};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Asset {
    BTC,
    USD,
    ETH,
}

//A Trading pair: base/quote
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
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

//allows for deserialization of path variable into Pair
impl<'de> Deserialize<'de> for Pair {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(de)?;
        Pair::from_str(&s).map_err(de::Error::custom)
    }
}

// String *serialization* for Pair (e.g., "BTC-USD")
impl Serialize for Pair {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.code())
    }
}

pub const BTC_USD: Pair = Pair {
    base: Asset::BTC,
    quote: Asset::USD,
};
pub const ETH_USD: Pair = Pair {
    base: Asset::ETH,
    quote: Asset::USD,
};
