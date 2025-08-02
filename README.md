Order Book Engine

A Rust-based limit order book engine with REST and WebSocket APIs, in-memory matching, persistent trade storage via ParityDB, a market maker bot, and a simulation harness for stress-testing and benchmarking.

Features

- Limit & Market Orders: FIFO, price-time priority, partial fills, and crossing.

- **In-Memory Books:** One order-book per trading pair (e.g. BTC-USD, ETH-USD), backed by a BTreeMap of price levels with VecDeque queues for orders.

- Persistence: Trades serialized with Bincode and stored in ParityDB.

- REST API: Submit orders, query order book & trade history.

- WebSocket API: Stream live book snapshots and trade events.

- Market Maker Bot: Two-sided quoting around mid-price via REST+WS.

- Simulation Harness: Adversarial load testing with random market orders.

- CLI Tool: Command-line interface for manual interaction.

- Benchmarking: Criterion benchmarks for matching performance.

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

**ssh**

```
git clone git@github.com:aodr3w/order-book-engine-rs.git
```
**http**

```
https://github.com/aodr3w/order-book-engine-rs.git
```

```
cd order-book-engine-rs

cargo build --release
```

Run the Server

**server only**

Launch HTTP & WS server with a ParityDB store at ./trade_store:

```
cargo run --release -- serve 3000

```
**Full Simulation (server + market-maker + attacker)**

Specify port only to run indefinitely (hit Ctrl+C to stop).

```
cargo run --release -- simulate 3000
```

Specify both port and seconds to run for a fixed duration

```
cargo run --release -- simulate 3000 5
```

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
       "quantity": 5,xr
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
