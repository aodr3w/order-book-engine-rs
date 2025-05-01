use errors::MarketMakerError;
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::sync::watch;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMsg};

use crate::{
    api::{BookSnapshot, WsFrame},
    errors,
    orders::{OrderType, Side},
};

const SPREAD: u64 = 2;
const PACE_MS: u64 = 500; //cancel & requote every 500ms

#[derive(Deserialize)]
struct NewOrder {
    side: Side,
    order_type: OrderType,
    price: Option<u64>,
    quantity: u64,
}

pub async fn run_market_maker(api_base: &str) -> Result<(), errors::MarketMakerError> {
    // 1) Subscribe to /ws
    let (ws_stream, _) = connect_async(format!("{}/ws", api_base))
        .await
        .map_err(|e| MarketMakerError::ConnectError(e.to_string()))?;
    let (mut write, mut read) = ws_stream.split();

    //Track the latest mid-price via  watch channel
    let (mid_tx, mut mid_rx) = watch::channel(None::<u64>);

    //Spawn a task to parse snapshots + trades
    tokio::spawn(async move {
        while let Some(Ok(msg)) = read.next().await {
            if let WsMsg::Text(txt) = msg {
                if let Ok(frame) = serde_json::from_str::<WsFrame>(&txt) {
                    if let WsFrame::BookSnapshot(BookSnapshot { bids, asks }) = frame {
                        if let (Some((best_bid, _)), Some((best_ask, _))) =
                            (bids.first(), asks.first())
                        {
                            let mid = (best_bid + best_ask) / 2;
                            let _ = mid_tx.send(Some(mid));
                        }
                    }
                }
            }
        }
    });
    Ok(())
}
