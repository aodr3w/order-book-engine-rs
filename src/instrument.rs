use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Asset {
    BTC,
    USD,
    ETH,
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Asset::BTC => "BTC",
            Asset::USD => "USD",
            Asset::ETH => "ETH",
        })
    }
}
impl FromStr for Asset {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "BTC" => Ok(Asset::BTC),
            "ETH" => Ok(Asset::ETH),
            "USD" => Ok(Asset::USD),
            _ => Err(format!("unknown asset `{s}`")),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(try_from = "&'de str", into = "String")]
pub struct Pair {
    pub base: Asset,
    pub quote: Asset,
}

impl Pair {
    pub fn code(&self) -> String {
        self.to_string()
    }
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

impl fmt::Display for Pair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.base, self.quote)
    }
}

// Fast, allocation-free FromStr that *also* enforces your whitelist.
impl FromStr for Pair {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "BTC-USD" => Ok(BTC_USD),
            "ETH-USD" => Ok(ETH_USD),
            _ => Err(format!("unsupported symbol: `{}`", s)),
        }
    }
}

// Glue for #[serde(try_from, into)]
impl<'a> TryFrom<&'a str> for Pair {
    type Error = String;
    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        s.parse()
    }
}
impl From<Pair> for String {
    fn from(p: Pair) -> Self {
        p.to_string()
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
