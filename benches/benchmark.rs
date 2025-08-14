use criterion::{Criterion, criterion_group, criterion_main};
use order_book_engine::instrument::BTC_USD;
use order_book_engine::orderbook::OrderBook;
use order_book_engine::orders::{Order, OrderType, Side};
use std::time::SystemTime;

fn setup_order_book(depth: u64, orders_per_level: u64) -> OrderBook {
    let mut ob = OrderBook::new();
    //populate asks
    for price in 1..=depth {
        for i in 0..orders_per_level {
            // Sell side
            ob.add_order(Order {
                id: (price as u128) * 1_000u128 + (i as u128),
                side: Side::Sell,
                order_type: OrderType::Limit,
                price: Some(price),
                quantity: 1,
                timestamp: SystemTime::now(),
                pair: BTC_USD,
            });
            ob.add_order(Order {
                id: ((depth as u128 + price as u128) * 1_000u128) + (i as u128),
                side: Side::Buy,
                order_type: OrderType::Limit,
                price: Some(price),
                quantity: 1,
                timestamp: SystemTime::now(),
                pair: BTC_USD,
            });
        }
    }
    ob
}

fn bench_match_order(c: &mut Criterion) {
    let depth = 100;
    let orders_per_level = 10;
    let mut ob = setup_order_book(depth, orders_per_level);
    c.bench_function("match 1 market order", |b| {
        b.iter(|| {
            let market_buy = Order {
                id: 0,
                side: Side::Buy,
                order_type: OrderType::Market,
                price: None,
                quantity: depth * orders_per_level / 2,
                timestamp: SystemTime::now(),
                pair: BTC_USD,
            };
            ob.match_order(market_buy);
        })
    });

    c.bench_function("match 1 limit crossing order", |b| {
        b.iter(|| {
            let limit_sell = Order {
                id: 1,
                side: Side::Sell,
                order_type: OrderType::Limit,
                price: Some(depth / 2),
                quantity: depth * orders_per_level,
                timestamp: SystemTime::now(),
                pair: BTC_USD,
            };
            ob.match_order(limit_sell)
        })
    });
}
criterion_group!(benches, bench_match_order);
criterion_main!(benches);
