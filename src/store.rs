use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use bincode::{
    config::{self, standard},
    error::{DecodeError, EncodeError},
};
use parity_db::{BTreeIterator, ColId, Db, Options};
use serde_json::{self};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

use crate::trade::Trade;

//Cursor (opaque to clients)
#[derive(serde::Serialize, serde::Deserialize)]
struct Cursor {
    ts_nanos: u128,
    maker_id: u64,
    taker_id: u64,
    price: u64,
    quantity: u64,
}

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

    #[inline]
    fn cursor_from_trade(t: &Trade) -> Cursor {
        Cursor {
            ts_nanos: Self::to_nanos(t.timestamp),
            maker_id: t.maker_id,
            taker_id: t.taker_id,
            price: t.price,
            quantity: t.quantity,
        }
    }

    #[inline]
    fn encode_cursor(c: &Cursor) -> String {
        B64.encode(serde_json::to_vec(c).unwrap())
    }

    #[inline]
    fn decode_cursor(s: &str) -> StoreResult<Cursor> {
        let bytes = B64.decode(s).map_err(|_| StoreError::BadCursor)?;
        serde_json::from_slice(&bytes).map_err(|_| StoreError::BadCursor)
    }

    #[inline]
    fn start_key_from(symbol: &str, after: Option<&Cursor>) -> Vec<u8> {
        match after {
            None => Self::prefix(symbol),
            Some(c) => {
                let mut k = Self::prefix(symbol);
                k.extend_from_slice(&c.ts_nanos.to_be_bytes());
                k.extend_from_slice(&c.maker_id.to_be_bytes());
                k.extend_from_slice(&c.taker_id.to_be_bytes());
                k.extend_from_slice(&c.price.to_be_bytes());
                k.extend_from_slice(&c.quantity.to_be_bytes());
                k
            }
        }
    }

    /// Insert a trade into the store under key "{symbol}:{timestamp_ms}".
    pub fn insert_trade(&mut self, trade: &Trade) -> StoreResult<()> {
        let config = config::standard();
        let col: ColId = 0;
        let key = Self::encode_key(&trade.symbol, trade);
        let value = bincode::encode_to_vec(trade, config)?;
        self.db.commit(vec![(col, key, Some(value))])?;
        // // column 0 for trades
        // let col: ColId = 0;
        // let symbol = &trade.symbol;
        // // timestamp in milliseconds since UNIX_EPOCH

        // let ts: u64 = trade
        //     .timestamp
        //     .duration_since(UNIX_EPOCH)
        //     .expect("system clock before UNIX_EPOCH")
        //     .as_millis() as u64;
        // // build key = symbol + ':' + big-endian timestamp
        // let mut key = Vec::with_capacity(symbol.len() + 1 + 8);
        // key.extend(symbol.as_bytes());
        // key.push(b':');
        // key.extend(&ts.to_be_bytes());
        // // serialize the trade as JSON
        // let value = bincode::encode_to_vec(trade, config)?;
        // // commit in a single-entry batch
        // self.db.commit(vec![(col, key, Some(value))])?;
        Ok(())
    }

    /// Page forward (ascending time) for a symbol, starting *strictly after* `after`.
    ///
    /// Returns (uitem , next_cursor). `next_cursor` is None when there are no more items.
    pub fn page_trade_asc(
        &self,
        symbol: &str,
        after: Option<&str>,
        limit: usize,
    ) -> StoreResult<(Vec<Trade>, Option<String>)> {
        let col: ColId = 0;
        let mut it: BTreeIterator<'_> = self.db.iter(col)?;
        let prefix = Self::prefix(symbol);

        let after_decoded = match after {
            None => None,
            Some(s) => Some(Self::decode_cursor(s)?),
        };
        let start_key = Self::start_key_from(symbol, after_decoded.as_ref());
        it.seek(&start_key)?;

        //if we started exactly at the "after" key, advance one so we do "strictly after".
        if after_decoded.is_some() {
            if let Some((k, _)) = it.next()? {
                if k.starts_with(&prefix) && k == start_key {
                    //skip equal
                } else {
                    // we landed past the "after" position; rewind iterator by re-seeking
                    it.seek(&start_key)?;
                }
            }
        } else {
            //no after; we are at first >= prefix; but we already consumed one via preview
            //so re-seek to prefix for a clean loop
            it.seek(&prefix)?;
        }
        let mut items = Vec::with_capacity(limit.min(256));
        let mut last_cursor: Option<String> = None;

        while items.len() < limit {
            match it.next()? {
                Some((k, v)) if k.starts_with(&prefix) => {
                    let (trade, _): (Trade, usize) = bincode::decode_from_slice(&v, standard())?;
                    last_cursor = Some(Self::encode_cursor(&Self::cursor_from_trade(&trade)));
                    items.push(trade);
                }
                _ => break,
            }
        }
        Ok((items, last_cursor))
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
    use std::time::Duration;

    use super::*;
    use crate::trade::Trade;
    use tempfile::tempdir;

    #[test]
    fn test_paging_two_items_limit_one() {
        let dir = tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();

        let t_old = Trade {
            symbol: "BTC-USD".into(),
            price: 50,
            quantity: 1,
            maker_id: 10,
            taker_id: 20,
            timestamp: SystemTime::UNIX_EPOCH + Duration::from_nanos(1),
        };
        let t_new = Trade {
            symbol: "BTC-USD".into(),
            price: 51,
            quantity: 2,
            maker_id: 11,
            taker_id: 21,
            timestamp: SystemTime::UNIX_EPOCH + Duration::from_nanos(2),
        };
        store.insert_trade(&t_old).unwrap();
        store.insert_trade(&t_new).unwrap();

        let (p1, c1) = store.page_trade_asc("BTC-USD", None, 1).unwrap();
        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0].price, 50); // ascending by time

        let (p2, c2) = store.page_trade_asc("BTC-USD", c1.as_deref(), 1).unwrap();
        assert_eq!(p2.len(), 1);
        assert_eq!(p2[0].price, 51);

        let (p3, c3) = store.page_trade_asc("BTC-USD", c2.as_deref(), 1).unwrap();
        assert!(p3.is_empty());
        assert!(c3.is_none());
    }
}
