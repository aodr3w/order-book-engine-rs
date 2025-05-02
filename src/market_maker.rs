use errors::MarketMakerError;
use futures_util::StreamExt;
use reqwest;
use serde::{Deserialize, Serialize};
use tokio::{sync::watch, time};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMsg};

use crate::{
    api::{BookSnapshot, OrderAck, WsFrame},
    errors,
    orders::{OrderType, Side},
};

const SPREAD: u64 = 2;
const PACE_MS: u64 = 500; //cancel & requote every 500ms

#[derive(Deserialize, Serialize)]
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
    let (write, mut read) = ws_stream.split();

    //Track the latest mid-price via  watch channel
    let (mid_tx, mid_rx) = watch::channel(None::<u64>);

    //2 Spawn a task to parse snapshots + trades
    tokio::spawn(async move {
        while let Some(Ok(WsMsg::Text(txt))) = read.next().await {
            if let Ok(WsFrame::BookSnapshot(BookSnapshot { bids, asks })) =
                serde_json::from_str::<WsFrame>(&txt)
            {
                if let (Some((best_bid, _)), Some((best_ask, _))) = (bids.first(), asks.first()) {
                    let mid = (best_bid + best_ask) / 2;
                    let _ = mid_tx.send(Some(mid));
                }
            }
        }
    });

    //3 Every PACE_MS: cancel old quotes and post new ones
    let client = reqwest::Client::new();
    let mut current_orders: Vec<u64> = Vec::new();

    let mut interval = time::interval(time::Duration::from_millis(PACE_MS));
    loop {
        interval.tick().await;
        if let Some(mid_price) = *mid_rx.borrow() {
            //cancel existing
            for id in current_orders.drain(..) {
                let _ = client
                    .delete(format!("{}/orders/{}", api_base, id))
                    .send()
                    .await;
            }
            //Place new bid
            if let Ok(resp) = client
                .post(format!("{}/orders", api_base))
                .json(&NewOrder {
                    side: Side::Buy,
                    order_type: OrderType::Limit,
                    price: Some(mid_price.saturating_sub(SPREAD)),
                    quantity: 1,
                })
                .send()
                .await
            {
                if let Ok(ack) = resp.json::<OrderAck>().await {
                    current_orders.push(ack.order_id);
                }
            }
            //Place new ask
            if let Ok(resp) = client
                .post(format!("{}/orders", api_base))
                .json(&NewOrder {
                    side: Side::Sell,
                    order_type: OrderType::Limit,
                    price: Some(mid_price + SPREAD),
                    quantity: 1,
                })
                .send()
                .await
            {
                if let Ok(ack) = resp.json::<OrderAck>().await {
                    current_orders.push(ack.order_id);
                }
            }
        }
    }
}
