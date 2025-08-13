use std::time::Duration;

use axum::Router;
use futures_util::StreamExt;
use order_book_engine::{
    api::{WsFrame, router},
    state::AppState,
};
use serde_json::json;
use tempfile::tempdir;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;

async fn spawn_server() -> (String, tokio::task::JoinHandle<()>, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let state = AppState::new(dir.path()).await.unwrap();
    let app: Router = router(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{}", addr);

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();
    let ok = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(r) = client.get(format!("{}/book/BTC-USD", base)).send().await {
                if r.status().is_success() {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .is_ok();
    assert!(ok, "server did not become ready in time");

    (base.to_string(), handle, dir)
}

#[tokio::test]
async fn websocket_snapshot_and_trade_flow() {
    let (http_base, server, _tmpdir) = spawn_server().await;
    let ws_url = http_base.replace("http://", "ws://") + "/ws/BTC-USD";

    let (mut ws, _resp) = connect_async(&ws_url).await.expect("ws connect");

    let first = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("ws first recv timeout")
        .expect("ws closed")
        .expect("ws error");
    let text = match first {
        tokio_tungstenite::tungstenite::Message::Text(t) => t,
        other => panic!("expected text frame, got {:?}", other),
    };
    let init: WsFrame = serde_json::from_str(&text).expect("parse WsFrame");
    match init {
        WsFrame::BookSnapshot(_snap) => { /* good */ }
        _ => panic!("expected initial BookSnapshot, got {:?}", init),
    }

    let client = reqwest::Client::new();
    let body = json!({
        "side": "Buy",
        "order_type": "Limit",
        "price": 48,
        "quantity": 5,
        "symbol": "BTC-USD"
    });
    let r = client
        .post(format!("{}/orders", http_base))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());

    let next = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("ws recv timeout after add_order")
        .expect("ws closed")
        .expect("ws error");
    let text = match next {
        tokio_tungstenite::tungstenite::Message::Text(t) => t,
        other => panic!("expected text frame, got {:?}", other),
    };
    let snap: WsFrame = serde_json::from_str(&text).expect("parse WsFrame");
    let (bids, asks) = match snap {
        WsFrame::BookSnapshot(s) => (s.bids, s.asks),
        other => panic!("expected BookSnapshot, got {:?}", other),
    };
    assert!(asks.is_empty(), "should not have asks yet");
    assert!(!bids.is_empty(), "bids should not be empty");
    assert_eq!(bids[0].0, 48, "top bid price should be 48");
    assert_eq!(bids[0].1, 5, "top bid qty should be 5");

    let market = json!({
        "side": "Sell",
        "order_type": "Market",
        "quantity": 2,
        "symbol": "BTC-USD"
    });
    let r = client
        .post(format!("{}/orders", http_base))
        .json(&market)
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());

    let trade_frame = loop {
        let msg = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("ws recv timeout after market order")
            .expect("ws closed")
            .expect("ws error");
        let text = match msg {
            tokio_tungstenite::tungstenite::Message::Text(t) => t,
            other => panic!("expected text frame, got {:?}", other),
        };
        let frame: WsFrame = serde_json::from_str(&text).expect("parse WsFrame");
        match frame {
            WsFrame::Trade(t) => break t,
            WsFrame::BookSnapshot(_) => continue, // keep reading until the trade arrives
        }
    };

    assert_eq!(
        trade_frame.price, 48,
        "trade should execute at maker price 48"
    );
    assert_eq!(trade_frame.quantity, 2, "trade should be for quantity 2");

    server.abort();
}
