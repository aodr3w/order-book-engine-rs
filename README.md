Order Book Engine

A high-performance, Rust-based limit order book engine with REST and WebSocket APIs, in-memory matching, persistent trade storage via ParityDB, a market maker bot, and a simulation harness for stress-testing and benchmarking.

Features

	•	Limit & Market Orders: Supports FIFO, price-time priority, partial fills, and crossing.
 
	•	In-Memory Book: Fast matching engine using BTreeMap and VecDeque.
 
	•	Persistence: Trades serialized with Bincode and stored in ParityDB.
 
	•	REST API: Submit orders, query order book & trade history.
 
	•	WebSocket API: Stream live book snapshots and trade events.
 
	•	Market Maker Bot: Two-sided quoting around mid-price via REST+WS.
 
	•	Simulation Harness: Adversarial load testing with random market orders.
 
	•	CLI Tool: Simple command-line interface for manual interaction.
 
	•	Benchmarking: Criterion benchmarks for matching performance.

Prerequisites

	•	Rust (1.70+)
	•	Cargo
	•	gnuplot (for Criterion plots, or use plotters fallback)
	•	Linux/macOS/Windows

Getting Started

Clone & Build

```
git clone git@github.com:aodr3w/order_book-engine-rs.git
cd order-book-engine
cargo build --release
```

Run the Server

# Launch HTTP & WS server with a ParityDB store at ./trade_store

```
cargo run --release --bin order-book-engine

```

Server listens on 0.0.0.0:3000 by default.

Environment Variables

	•	RUST_LOG – enable tracing (e.g. export RUST_LOG=trace).

REST API

POST /orders – Submit a new order

```

{
  "side": "Buy",         // "Buy" or "Sell"
  "order_type": "Limit", // "Limit" or "Market"
  "price": 50,            // optional for market
  "quantity": 1,
  "symbol": "BTC-USD"
}

```

Response:

```

{
  "order_id": 12345,
  "trades": [ /* any matches */ ]
}

```

GET /book/{symbol} – Get current book snapshot

```
curl http://localhost:3000/book/BTC-USD
```

GET /trades/{symbol} – Get recent trades

```
curl http://localhost:3000/trades/BTC-USD
```

WebSocket API

Connect to ws://localhost:3000/ws/{symbol} for live BookSnapshot and Trade frames.

```
order_book-engine-rs % websocat ws://127.0.0.1:3000/ws/BTC-USD
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[[48,6]],"asks":[[52,7]]}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[[48,5]],"asks":[[52,7]]}}
{"Trade":{"price":48,"quantity":1,"maker_id":12675247018488016793,"taker_id":12256232967323569628,"timestamp":{"secs_since_epoch":1753122755,"nanos_since_epoch":46061000},"symbol":"BTC-USD"}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[[48,5]],"asks":[[52,6]]}}
{"Trade":{"price":52,"quantity":1,"maker_id":12079357885989867747,"taker_id":13591937647730433539,"timestamp":{"secs_since_epoch":1753122755,"nanos_since_epoch":244592000},"symbol":"BTC-USD"}}
{"Trade":{"price":52,"quantity":1,"maker_id":12079357885989867747,"taker_id":10698009476770110052,"timestamp":{"secs_since_epoch":1753122755,"nanos_since_epoch":444650000},"symbol":"BTC-USD"}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[[48,5]],"asks":[[52,5]]}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[[48,4]],"asks":[[52,5]]}}
{"Trade":{"price":48,"quantity":1,"maker_id":12675247018488016793,"taker_id":12199289940956064951,"timestamp":{"secs_since_epoch":1753122755,"nanos_since_epoch":645060000},"symbol":"BTC-USD"}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[[48,4]],"asks":[[52,4]]}}
{"Trade":{"price":52,"quantity":1,"maker_id":12079357885989867747,"taker_id":10532309852038529817,"timestamp":{"secs_since_epoch":1753122755,"nanos_since_epoch":844360000},"symbol":"BTC-USD"}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[[48,4]],"asks":[[52,3]]}}
{"Trade":{"price":52,"quantity":1,"maker_id":12079357885989867747,"taker_id":13453026739183454039,"timestamp":{"secs_since_epoch":1753122756,"nanos_since_epoch":45486000},"symbol":"BTC-USD"}}
{"Trade":{"price":52,"quantity":1,"maker_id":12079357885989867747,"taker_id":10075532142094481215,"timestamp":{"secs_since_epoch":1753122756,"nanos_since_epoch":244474000},"symbol":"BTC-USD"}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[[48,4]],"asks":[[52,2]]}}
{"Trade":{"price":48,"quantity":1,"maker_id":12675247018488016793,"taker_id":13707341538759834384,"timestamp":{"secs_since_epoch":1753122756,"nanos_since_epoch":444773000},"symbol":"BTC-USD"}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[[48,3]],"asks":[[52,2]]}}
{"Trade":{"price":48,"quantity":1,"maker_id":12675247018488016793,"taker_id":11400427895993397181,"timestamp":{"secs_since_epoch":1753122756,"nanos_since_epoch":644381000},"symbol":"BTC-USD"}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[[48,2]],"asks":[[52,2]]}}
{"Trade":{"price":48,"quantity":1,"maker_id":12675247018488016793,"taker_id":12217184210333058439,"timestamp":{"secs_since_epoch":1753122756,"nanos_since_epoch":845628000},"symbol":"BTC-USD"}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[[48,1]],"asks":[[52,2]]}}
{"Trade":{"price":52,"quantity":1,"maker_id":12079357885989867747,"taker_id":12628008327160511755,"timestamp":{"secs_since_epoch":1753122757,"nanos_since_epoch":44896000},"symbol":"BTC-USD"}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[[48,1]],"asks":[[52,1]]}}
{"Trade":{"price":48,"quantity":1,"maker_id":12675247018488016793,"taker_id":10283696296790073204,"timestamp":{"secs_since_epoch":1753122757,"nanos_since_epoch":244497000},"symbol":"BTC-USD"}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[],"asks":[[52,1]]}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[],"asks":[[52,1]]}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[],"asks":[]}}
{"Trade":{"price":52,"quantity":1,"maker_id":12079357885989867747,"taker_id":12903652176689684741,"timestamp":{"secs_since_epoch":1753122757,"nanos_since_epoch":644587000},"symbol":"BTC-USD"}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[],"asks":[]}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[],"asks":[]}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[],"asks":[]}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[],"asks":[]}}
{"BookSnapshot":{"pair":{"base":"BTC","quote":"USD"},"bids":[],"asks":[]}}

```

CLI Usage

# Add limit order

```
cargo run --bin order-book-engine -- cli add buy limit 10 --price 50
```

# Match a market order

```
cargo run --bin order-book-engine -- cli match sell 5
```

# Show book

```
cargo run --bin order-book-engine -- cli book
```

Market Maker Bot

Runs alongside the engine to provide two-sided quotes.

```
cargo run --release --bin order-book-engine -- market-maker
```

Simulation Harness

Stress-test with random market orders:

```
cargo run --release --bin order-book-engine -- simulate --run-secs 10 --attack-rate-hz 5
```

Benchmarking

```
cargo bench
```

License

MIT
