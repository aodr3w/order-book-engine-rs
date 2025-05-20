🦀 Rust Orderbook Engine

A simple matching engine and limit order book written in Rust, developed for educational purposes.

⸻

🎯 Learning Outcomes
	•	Rust mastery: ownership, concurrency (async/await, tokio), channels, locking.
	•	Orderbook internals: price-time priority, limit vs. market orders, partial fills.
	•	Web APIs & WebSocket: axum for REST endpoints, real-time broadcast.
	•	Database integration: sqlx migrations, Postgres persistence for trade history.
	•	Benchmarking & simulation: criterion benches, synthetic market simulation, P&L tracking.

⸻

🧱 Core Features
	•	Order Matching Engine (orderbook.rs)
	•	Bid/Ask book as BTreeMap<u64, VecDeque<Order>>
	•	Price-time FIFO matching, market & limit order support
	•	Partial fills and book maintenance
	•	REST & WebSocket API (api.rs)
	•	POST /orders: submit orders, returns OrderAck with any immediate trades
	•	GET /book: snapshot of aggregated depth
	•	GET /trades: recent trade log from DB
	•	/ws: real-time stream of trades & book snapshots
	•	Market Maker Bot (market_maker.rs)
	•	Two‐sided quoting around mid-price
	•	WebSocket subscription for live book updates
	•	Requotes when mid shifts: posts new bid/ask at mid ± SPREAD
	•	Cleans up stale quotes to manage risk
	•	Simulator (simulate.rs)
	•	Random aggressor market orders at configurable rate
	•	Tracks realized P&L and inventory of the market maker
	•	Uses REST API to drive synthetic trading activity
	•	Persistence
	•	Postgres via sqlx migrations for trades table
	•	Each executed trade is recorded with timestamp
	•	Benchmarking
	•	benches/benchmark.rs with criterion measuring matching latency

⸻

🏛 Architecture Overview

       ┌────────────┐      REST/WebSocket      ┌───────────────┐
       │   HTTP     │   ┌──────────────────┐   │               │
       │   Clients  │──▶│  Axum API Layer  │──▶│   Order Book  │
       │  (Browser, │   └──────────────────┘   │   Matching    │
       │   Bot, CLI)│                           │   Engine      │
       └────────────┘                           └───────────────┘
                ▲                                        │
                │                 WebSocket              │
                │                (Broadcast)             ▼
       ┌────────────┐      ┌──────────────────┐   ┌───────────────┐
       │ Market     │◀─────│  Broadcast Layer │◀──│   Trade Log   │
       │ Maker Bot  │      └──────────────────┘   │   (Postgres)  │
       └────────────┘                             └───────────────┘
                ▲                                          ▲
                │                                          │
       ┌────────────┐      REST API Calls        ┌───────────────┐
       │ Simulator  │───▶│  `POST /orders`  ────▶│ Trade Storage │
       └────────────┘      (Market Orders)       └───────────────┘

	•	API Layer (axum): handles HTTP & WebSocket, routes to AppState.
	•	Order Book Engine: in-memory book + matching logic, protected by Mutex.
	•	Broadcast Layer: two tokio::broadcast channels for distributing events.
	•	Market Maker & Simulator: standalone async tasks driving and responding to book.
	•	Database: persists executed trades for history and analytics.

⸻

🚀 Getting Started
	1.	Clone & configure

git clone https://github.com/your/repo.git
cd order_book-engine
cp .env.example .env
# set DATABASE_URL=postgres://trader:secret@localhost:5432/orderbook


	2.	Start Postgres with Docker

docker-compose up -d


	3.	Run migrations

cargo install sqlx-cli
sqlx migrate run


	4.	Build & run

cargo run --release


	5.	Seed initial book (optional)

# resting bid & ask so market maker has a mid
curl -X POST http://localhost:3000/orders \
  -H 'Content-Type: application/json' \
  -d '{"side":"Buy","order_type":"Limit","price":48,"quantity":10}'
curl -X POST http://localhost:3000/orders \
  -H 'Content-Type: application/json' \
  -d '{"side":"Sell","order_type":"Limit","price":52,"quantity":10}'


	6.	Market maker & simulation run automatically on startup.

⸻

🛠 Benchmarks & Simulation
	•	Benchmark matching performance:

cargo bench


	•	Simulation output P&L & inventory:

# built into main; see startup logs for results



⸻

📈 Next Steps
	•	📊 Interactive Web UI (React + Charting)
	•	⚙️ Order persistence & query endpoints
	•	🔒 Concurrency safety improvements (lock-free, sharded books)
	•	🔄 Backtesting framework over historical data

⸻

Built with ❤️ and Rust 🦀.