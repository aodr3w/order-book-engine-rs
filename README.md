Rust Orderbook Engine

A high-performance matching engine, limit order book, market maker, and simulator written in Rust, built for low-latency electronic trading systems.

⸻

🧑‍🎓 Learning Outcomes

	•	Rust mastery: ownership/borrowing, async/await with Tokio, synchronization (Mutex, watch channel).

	•	Systems design: architecting a modular engine, REST+WebSocket APIs with Axum.

	•	Database integration: schema migrations and persistence using SQLx+Postgres.

	•	Trading concepts: limit vs market orders, price-time priority matching, two-sided quoting market making.
      
	•	Performance benchmarking: using Criterion to measure matching engine throughput and latency.

⸻

⚡ Features
	•	Order Matching Engine
	•	Price-time priority with FIFO queues per price level.
	•	Supports limit and market orders, partial fills, crossing orders.
	•	REST & WebSocket API (Axum)
	•	POST /orders to create orders, DELETE /orders/{id} to cancel.
	•	GET /book and live WsFrame::BookSnapshot updates.
	•	GET /trades and live WsFrame::Trade feeds.
	•	Market Maker Bot
	•	Two-sided quoting around mid-price with adjustable spread & cadence.
	•	Reacts to book snapshots over WebSocket and cancels/reposts quotes.
	•	Simulator & P&L
	•	Simulates aggressive market orders against the engine to measure realized P&L and inventory.
	•	Persistence
	•	Persists all trade events to Postgres via SQLx migrations.
	•	Benchmark Suite
	•	Criterion benchmarks for matching engine under various book depths.

⸻

🧱 Architecture Overview

    +---------------------+          +---------------------------+
    |  REST API (Axum)    |   HTTP   |       WebSocket Server    |
    +---------------------+ <------> +---------------------------+
             |                                    |
             v                                    v
    +----------------------------------------------+
    |         Orderbook & Matching Engine         |
    |  - BTreeMap price levels with VecDeque queues|
    |  - market/limit match_incoming_side() logic  |
    +----------------------------------------------+
             |                   |
             v                   v
   +------------------+    +-----------------+
   | Trade Log in-Mem |    |  Postgres DB    |
   +------------------+    +-----------------+


⸻

🚀 Getting Started

Prerequisites
	•	Rust (1.66+)
	•	Docker & Docker Compose
	•	cargo, docker-compose, psql

Clone & Build

git clone https://github.com/your/repo.git
cd order_book-engine
cargo build --release

Database & Migrations
	1.	Launch Postgres via Docker:





docker-compose up -d

2. Create `.env` with:

   ```ini
DATABASE_URL=postgres://trader:secret@localhost:5432/orderbook

	3.	Run migrations (automatically on startup):
SQLx discovers ./migrations/ and applies:





cargo run

### Run Service + Market Maker + Simulator

```bash
cargo run --release

This will:
	1.	Start HTTP+WS server on localhost:3000.
	2.	Seed initial book levels in main.rs (48 bid / 52 ask).
	3.	Spawn the Market Maker to quote at [mid-2, mid+2] every 500ms.
	4.	Spawn the Simulator issuing random market orders (5 Hz) for 10s.

Logs will display matching events, P&L summary at simulation end.

⸻

⚙️ Modules
	•	orderbook.rs: core BTreeMap+VecDeque book, matching logic match_incoming_side().
	•	api.rs: Axum routes (/orders, /book, /trades, /ws).
	•	market_maker.rs: two-sided quoting bot using WebSocket & REST.
	•	simulate.rs: random market order injector, computes realized P&L.
	•	state.rs: AppState holds in-memory book, channels, and PgPool.
	•	trade.rs & orders.rs: domain structs.

⸻

📊 Benchmarking

cargo bench

Runs Criterion benchmarks under benches/benchmark.rs, measuring matching times for:
	•	Single market order fill across many price levels.
	•	Single crossing limit order.

Sample output:

match 1 market order    time:   [26.5 ns 26.7 ns ...]
match 1 limit crossing  time:   [37.8 ns 38.0 ns ...]


⸻

✅ Next Steps
	•	Expose metrics (Prometheus) for latencies & P&L.
	•	Add order book persistence / recovery from DB.
	•	Advanced market making strategies (inventory skew, dynamic spread).
	•	Web UI (React) for live book and trades visualization.

⸻

Crafted by [Andrew Odiit] — powered by Rust & SQLx & Tokio & Axum.