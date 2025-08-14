use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;

use order_book_engine::{
    api::{OrderAck, router},
    state::AppState,
};
use serde_json::{Value, json};
use tempfile::tempdir;
use tower::ServiceExt;
use urlencoding::encode;

async fn test_app() -> (Router, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let state = AppState::new(dir.path()).await.unwrap();
    (router(state), dir)
}

async fn body_json(res: axum::response::Response) -> Value {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn test_effective_limit_on_get_trade_log() {
    let (app, _tmp) = test_app().await;
    let res = app
        .oneshot(
            Request::builder()
                .uri("/trades/BTC-USD?limit=5000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers().get("x-effective-limit").unwrap(), "1000");
}
#[tokio::test]
async fn pairguard_rejects_bad_pair_on_book() {
    let (app, _tmp) = test_app().await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/book/BTC-EUR")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let v = body_json(res).await;
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("unsupported")
    );
}

#[tokio::test]
async fn pairguard_rejects_bad_pair_on_trades_and_cancel() {
    let (app, _tmp) = test_app().await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/trades/FOO-BAR")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/orders/FOO-BAR/123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_order_rejects_zero_qty() {
    let (app, _tmp) = test_app().await;

    let body = json!({
        "side": "Buy",
        "order_type": "Limit",
        "price": 50,
        "quantity": 0,
        "symbol": "BTC-USD"
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/orders")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let v = body_json(res).await;
    assert_eq!(v["error"], "quantity must be > 0");
}

#[tokio::test]
async fn create_order_invalid_symbol_yields_422_from_loggedjson() {
    let (app, _tmp) = test_app().await;

    let body = json!({
        "side": "Buy",
        "order_type": "Limit",
        "price": 50,
        "quantity": 1,
        "symbol": "BTC-LOL"
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/orders")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let v = body_json(res).await;
    assert!(v["error"].as_str().unwrap().contains("unsupported symbol"));
}

async fn json<T: serde::de::DeserializeOwned>(res: Response) -> T {
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn limit_order_rests_then_cancel_removes_it() {
    let (app, _tmp) = test_app().await;

    let create = json!({
        "side": "Buy",
        "order_type": "Limit",
        "price": 48,
        "quantity": 10,
        "symbol": "BTC-USD"
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/orders")
                .header("content-type", "application/json")
                .body(Body::from(create.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let ack: OrderAck = json(res).await;
    let order_id = ack.order_id;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/book/BTC-USD")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let snap = body_json(res).await;
    assert_eq!(snap["bids"][0][0].as_u64(), Some(48));

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/orders/BTC-USD/{}", order_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/book/BTC-USD")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let snap = body_json(res).await;
    assert!(
        snap["bids"]
            .as_array()
            .map(|arr| arr
                .first()
                .map(|lvl| lvl[0].as_u64().unwrap() != 48)
                .unwrap_or(true))
            .unwrap_or(true)
    );
}

#[tokio::test]
async fn trades_endpoint_paginates_forward() {
    let (app, _tmp) = test_app().await;

    let seed = json!({
        "side": "Sell",
        "order_type": "Limit",
        "price": 52,
        "quantity": 3,
        "symbol": "BTC-USD"
    });
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/orders")
                .header("content-type", "application/json")
                .body(Body::from(seed.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let market_buy = |qty| {
        json!({
            "side": "Buy",
            "order_type": "Market",
            "quantity": qty,
            "symbol": "BTC-USD"
        })
    };

    for _ in 0..2 {
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/orders")
                    .header("content-type", "application/json")
                    .body(Body::from(market_buy(1).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/trades/BTC-USD?limit=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let page1 = body_json(res).await;
    assert_eq!(page1["items"].as_array().unwrap().len(), 1);
    let next = page1["next"].as_str().unwrap();

    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/trades/BTC-USD?limit=1&after={}", encode(next)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let page2 = body_json(res).await;
    assert_eq!(page2["items"].as_array().unwrap().len(), 1);
}
