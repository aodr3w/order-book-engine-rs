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

git clone https://github.com/your_org/order-book-engine.git
cd order-book-engine
cargo build --release

Run the Server

# Launch HTTP & WS server with a ParityDB store at ./trade_store
cargo run --release --bin order-book-engine

Server listens on 0.0.0.0:3000 by default.

Environment Variables
	•	RUST_LOG – enable tracing (e.g. export RUST_LOG=trace).

REST API

POST /orders – Submit a new order

{
  "side": "Buy",         // "Buy" or "Sell"
  "order_type": "Limit", // "Limit" or "Market"
  "price": 50,            // optional for market
  "quantity": 1,
  "symbol": "BTC-USD"
}

Response:

{
  "order_id": 12345,
  "trades": [ /* any matches */ ]
}

GET /book/{symbol} – Get current book snapshot

curl http://localhost:3000/book/BTC-USD

GET /trades/{symbol} – Get recent trades

curl http://localhost:3000/trades/BTC-USD

WebSocket API

Connect to ws://localhost:3000/ws/{symbol} for live BookSnapshot and Trade frames.

const socket = new WebSocket("ws://localhost:3000/ws/BTC-USD");
socket.onmessage = (msg) => console.log(JSON.parse(msg.data));

CLI Usage

# Add limit order
cargo run --bin order-book-engine -- cli add buy limit 10 --price 50

# Match a market order
cargo run --bin order-book-engine -- cli match sell 5

# Show book
cargo run --bin order-book-engine -- cli book

Market Maker Bot

Runs alongside the engine to provide two-sided quotes.

cargo run --release --bin order-book-engine -- market-maker

Simulation Harness

Stress-test with random market orders:

cargo run --release --bin order-book-engine -- simulate --run-secs 10 --attack-rate-hz 5

Benchmarking

cargo bench

License

MIT