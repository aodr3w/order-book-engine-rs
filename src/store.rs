use bincode::{
    config,
    error::{DecodeError, EncodeError},
};
use parity_db::{BTreeIterator, ColId, Db, Options};
use serde_json;
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

use crate::trade::Trade;

/// Errors from the key/value store
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("ParityDB error: {0}")]
    Parity(#[from] parity_db::Error),
    #[error("Serialization/Deserialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("UTF-8 conversion error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Bincode encode error: {0}")]
    BincodeEncode(#[from] EncodeError),

    #[error("Bincode decode error: {0}")]
    BincodeDecode(#[from] DecodeError),

    #[error("Invalid cursor")]
    BadCursor,
}

pub type StoreResult<T> = Result<T, StoreError>;

/// A simple ParityDB-backed store for trades, keyed by "symbol:timestamp".
pub struct Store {
    db: Db,
}

impl Store {
    /// Open (or create) a ParityDB at `path`, with a single column and B-tree index.
    pub fn open(path: impl AsRef<Path>) -> StoreResult<Self> {
        let mut opts = Options::with_columns(path.as_ref(), 1);
        // enable B-tree index on column 0 for prefix scans
        opts.columns[0].btree_index = true;
        let db = Db::open_or_create(&opts)?;
        Ok(Store { db })
    }

    #[inline]
    fn to_nanos(ts: SystemTime) -> u128 {
        ts.duration_since(UNIX_EPOCH).unwrap().as_nanos()
    }

    #[inline]
    fn prefix(symbol: &str) -> Vec<u8> {
        let mut k = Vec::with_capacity(symbol.len() + 1);
        k.extend_from_slice(symbol.as_bytes());
        k.push(b':');
        k
    }

    #[inline]
    fn encode_key(symbol: &str, trade: &Trade) -> Vec<u8> {
        let mut key = Self::prefix(symbol);
        let ts = Self::to_nanos(trade.timestamp);
        key.extend_from_slice(&ts.to_be_bytes());
        key.extend_from_slice(&trade.maker_id.to_be_bytes());
        key.extend_from_slice(&trade.taker_id.to_be_bytes());
        key.extend_from_slice(&trade.price.to_be_bytes());
        key.extend_from_slice(&trade.quantity.to_be_bytes());
        key
    }

    /// Insert a trade into the store under key "{symbol}:{timestamp_ms}".
    pub fn insert_trade(&mut self, trade: &Trade) -> StoreResult<()> {
        let config = config::standard();
        // column 0 for trades
        let col: ColId = 0;
        let symbol = &trade.symbol;
        // timestamp in milliseconds since UNIX_EPOCH

        let ts: u64 = trade
            .timestamp
            .duration_since(UNIX_EPOCH)
            .expect("system clock before UNIX_EPOCH")
            .as_millis() as u64;
        // build key = symbol + ':' + big-endian timestamp
        let mut key = Vec::with_capacity(symbol.len() + 1 + 8);
        key.extend(symbol.as_bytes());
        key.push(b':');
        key.extend(&ts.to_be_bytes());
        // serialize the trade as JSON
        let value = bincode::encode_to_vec(trade, config)?;
        // commit in a single-entry batch
        self.db.commit(vec![(col, key, Some(value))])?;
        Ok(())
    }

    /// Retrieve up to `limit` most-recent trades for a given symbol.
    pub fn get_trades(&self, symbol: &str, limit: usize) -> StoreResult<Vec<Trade>> {
        let col: ColId = 0;
        let mut iter: BTreeIterator<'_> = self.db.iter(col)?;
        // seek to the first key >= symbol
        iter.seek(symbol.as_bytes())?;

        // collect all trades with prefix "symbol:"
        let prefix = symbol.as_bytes();
        let mut trades = Vec::new();
        while let Some((key, raw)) = iter.next()? {
            if !key.starts_with(prefix) {
                break;
            }
            // parse JSON back into Trade
            let trade: Trade = serde_json::from_slice(&raw)?;
            trades.push(trade);
        }

        // if more than `limit`, return the last `limit` entries
        let len = trades.len();
        if len > limit {
            Ok(trades[len - limit..].to_vec())
        } else {
            Ok(trades)
        }
    }

    /// Delete all trades for a given symbol.
    pub fn delete_trades(&mut self, symbol: &str) -> StoreResult<()> {
        let col: ColId = 0;
        let mut iter = self.db.iter(col)?;
        iter.seek(symbol.as_bytes())?;

        let prefix = symbol.as_bytes();
        let mut batch = Vec::new();
        while let Some((key, _)) = iter.next()? {
            if !key.starts_with(prefix) {
                break;
            }
            batch.push((col, key.to_vec(), None));
        }
        if !batch.is_empty() {
            self.db.commit(batch)?;
        }
        Ok(())
    }
    pub fn iter_trades(&self) -> Result<impl Iterator<Item = Trade>, StoreError> {
        let config = config::standard();
        let mut iter = self.db.iter(0).map_err(StoreError::Parity)?;

        iter.seek_to_first().map_err(StoreError::Parity)?;
        Ok(std::iter::from_fn(move || match iter.next() {
            Ok(Some((_key, raw))) => {
                let (decoded, _): (Trade, usize) =
                    bincode::decode_from_slice(&raw[..], config).unwrap();
                Some(decoded)
            }
            _ => None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade::Trade;
    use chrono::Utc;
    use tempfile::tempdir;

    #[test]
    fn test_store_roundtrip() {
        let dir = tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let t1 = Trade {
            symbol: "BTC-USD".to_string(),
            price: 42,
            quantity: 3,
            maker_id: 1,
            taker_id: 2,
            timestamp: Utc::now().into(),
        };
        store.insert_trade(&t1).unwrap();
        let res = store.get_trades("BTC-USD", 10).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].symbol, t1.symbol);
    }
}
