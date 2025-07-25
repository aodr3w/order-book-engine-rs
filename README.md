Order Book Engine

A high-performance, Rust-based limit order book engine with REST and WebSocket APIs, in-memory matching, persistent trade storage via ParityDB, a market maker bot, and a simulation harness for stress-testing and benchmarking.

Features

	•	Limit & Market Orders: FIFO, price-time priority, partial fills, and crossing.
 
	•	In-Memory Book: Fast matching engine using BTreeMap and VecDeque.
 
	•	Persistence: Trades serialized with Bincode and stored in ParityDB.
 
	•	REST API: Submit orders, query order book & trade history.
 
	•	WebSocket API: Stream live book snapshots and trade events.
 
	•	Market Maker Bot: Two-sided quoting around mid-price via REST+WS.
 
	•	Simulation Harness: Adversarial load testing with random market orders.
 
	•	CLI Tool: Command-line interface for manual interaction.
 
	•	Benchmarking: Criterion benchmarks for matching performance.

Prerequisites

	•	Rust (1.70+)
 
	•	Cargo
 
	•	gnuplot (for Criterion plots, or use plotters fallback)
 
	•	Linux/macOS/Windows

Repository Layout

```

├── benches/benchmark.rs      # Criterion benchmarks
├── src/
│   ├── api.rs                # HTTP & WS handlers
│   ├── cli.rs                # Command-line interface
│   ├── instrument.rs         # Asset & Pair types
│   ├── orderbook.rs          # Matching engine
│   ├── orders.rs             # Order definitions
│   ├── simulate.rs           # Simulation harness
│   ├── state.rs              # Shared AppState
│   ├── store.rs              # ParityDB-backed store
│   ├── trade.rs              # Trade struct
│   ├── market_maker.rs       # Market maker bot
│   ├── errors.rs             # Error types
│   └── main.rs               # Entry point
└── README.md

```

Getting Started

Clone & build:

```
git clone https://github.com/aodr3w/order_book-engine-rs.git
cd order_book-engine-rs
cargo build --release
```

Run the Server

**server only**

Launch HTTP & WS server with a ParityDB store at ./trade_store:

```
cargo run --release -- server 3000

```
**Full Simulation (server + market-maker + attacker)**

```
cargo run --release -- simulate 3000

```


Server listens on 0.0.0.0:3000 by default.

Environment variables:

	•	RUST_LOG – enable tracing (e.g. export RUST_LOG=trace).

REST API

Submit a Limit Order

```
curl -X POST http://127.0.0.1:3000/orders \
     -H "Content-Type: application/json" \
     -d '{
       "side": "Buy",
       "order_type": "Limit",
       "price": 50,
       "quantity": 1,
       "symbol": "BTC-USD"
     }'
```

Response:

```
{
  "order_id": 12345,
  "trades": []
}
```

Submit a Market Order

```
curl -X POST http://127.0.0.1:3000/orders \
     -H "Content-Type: application/json" \
     -d '{
       "side": "Sell",
       "order_type": "Market",
       "quantity": 5,
       "symbol": "BTC-USD"
     }'

```

Response:

```

{
  "order_id": 12346,
  "trades": [
    { "price": 52, "quantity": 5, "maker_id": 67890, "taker_id": 12346, "timestamp": {...}, "symbol": "BTC-USD" }
  ]
}

```

Query the Book


```
curl http://127.0.0.1:3000/book/BTC-USD

```

Query Recent Trades

```
curl http://127.0.0.1:3000/trades/BTC-USD

```

WebSocket API

Stream live updates (snapshots & trades):

```
websocat ws://127.0.0.1:3000/ws/BTC-USD

```

Benchmarking

```
cargo bench

```

License

MIT
