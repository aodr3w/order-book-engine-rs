Order Book Engine

A high-performance limit order book engine written in Rust, complete with:
	•	Core Matching Engine (orderbook.rs): FIFO price-time priority matching for limit and market orders.
	•	REST & WebSocket API (api.rs): Axum-powered HTTP server with endpoints to submit and cancel orders, fetch book snapshots, and stream trades.
	•	CLI Interface (cli.rs): Command-line tool for submitting and matching orders, and viewing the current book.
	•	Market Maker Bot (market_maker.rs): A simple two-sided quoting bot that connects via WebSocket and REST to provide liquidity.
	•	Simulation Harness (simulate.rs): Adversarial simulator that sends random market orders to measure P&L and inventory risk.
	•	Benchmarks (benchmarks.rs): Criterion benchmarks for order matching under configurable book depth and load.

⸻

Architecture

┌─────────────────────┐        HTTP/WebSocket        ┌────────────────────────┐
│  Order Book Engine  │<--------------------------->│  Market Maker Bot       │
│ (server: api.rs)    │        REST: POST/DELETE    │ (market_maker.rs)       │
│                     │        WS: BookSnapshot,    │                         │
│  • orderbook.rs     │            Trade            │  • listens to WS feed   │
│  • state.rs         │                             │  • computes mid-price   │
│  • trade logging    │                             │  • cancels & reposts    │
└─────────────────────┘                             └─────────────────────────┘
         ▲  ^                                              ▲        
         │  |                                              |         
         |  | WS feed                                    | WS feed
    REST |  |                                          REST|        
         |  |                                          API |        
         |  └───────────────────────────────▶────────────┘        
         │                                                 
┌─────────────────────┐                                       
│ Simulation Harness  │                                       
│  (simulate.rs)      │                                       
│                     │                                       
│  • sends market     │ REST: POST /orders                     
│    orders at a      │                                       
│    configured rate  │                                       
│  • records P&L,     │                                       
│    inventory risk   │                                       
└─────────────────────┘                                       

	•	Order Book Engine exposes:
	•	POST /orders to place limit or market orders
	•	DELETE /orders/{id} to cancel orders
	•	GET /book to retrieve a snapshot of bids and asks
	•	GET /ws to stream live snapshots and trade events
	•	GET /trades to fetch recent trades
	•	Market Maker connects to the WebSocket feed to get live book snapshots, computes the mid-price, and uses the REST API to place and cancel its own quotes in a loop.
	•	Simulator bombards the engine with randomized market orders via REST to stress-test the matching logic and measure realized P&L and inventory changes.

This modular architecture ensures:
	•	Clear separation between core matching logic and strategy clients
	•	Realistic testing of quoting strategies under live market conditions
	•	Easy extension for additional bots or simulation scenarios

⸻

Features
	•	Limit & Market Orders: Supports both limit and market orders, with partial fills and price-level matching.
	•	Order Cancels: Cancel orders by ID and clean up empty price levels.
	•	Thread-safe State: Uses Arc<Mutex<OrderBook>> for shared state, and tokio::sync::broadcast for event notifications.
	•	Persistence: Trades logged to PostgreSQL via sqlx migrations (in state.rs).
	•	Streaming: WebSocket feed for live book snapshots and trade events.
	•	Extensible: Modular design for plugging in custom strategies, simulations, and I/O layers.

⸻

Getting Started

Prerequisites
	•	Rust (latest stable)
	•	PostgreSQL database
	•	DATABASE_URL environment variable pointing to your Postgres instance
	•	cargo, git, and optionally docker for local testing

Clone & Build

git clone https://github.com/aodr3w/order_book-engine-rs.git
cd order_book-engine-rs
cargo build --release

Database Setup

Create a Postgres database and run migrations:

dotenvy -e .env.sample -- sqlx migrate run

Ensure your .env contains:

DATABASE_URL=postgres://user:password@localhost:5432/orderbook

Running the Server

Starts the Axum HTTP & WS server on port 3000, seeds the book, market maker, and simulation:

cargo run --release

	•	HTTP API base: http://localhost:3000
	•	WS feed: ws://localhost:3000/ws

⸻

REST Endpoints

Method	Path	Description
GET	/book	Returns current book snapshot
POST	/orders	Create a new limit or market order
DELETE	/orders/{id}	Cancel an existing order
GET	/trades	Fetch latest trades (limit 100)

WebSocket Frames
	•	BookSnapshot: Full snapshot with bids and asks.
	•	Trade: Individual trade events.

CLI Usage

# Add a limit buy order at price=100, qty=5
o2 book-cli add buy limit 5 100

# Send a market sell order qty=2
o2 book-cli match sell 2

# View current book
o2 book-cli book

Benchmarks

Run Criterion benchmarks:

cargo bench -- --nocapture

Adjust depth and orders per price level in benchmarks.rs.

Simulation

Attack the engine with random market orders:

cargo run --release --bin simulate

Tune run_secs and attack_rate_hz in simulate.rs.

⸻

Project Structure

├── benches/benchmarks.rs         # Criterion benchmarks
├── src/
│   ├── api.rs                   # Axum REST & WS API
│   ├── cli.rs                   # Command-line interface
│   ├── market_maker.rs          # Market maker bot
│   ├── simulate.rs              # Simulation harness
│   ├── state.rs                 # Shared application state & DB pool
│   ├── orderbook.rs             # Core matching engine
│   ├── orders.rs                # Order and side/type definitions
│   ├── trade.rs                 # Trade struct definition
│   ├── errors.rs                # Custom error types
│   └── lib.rs                   # Module declarations
├── migrations/                  # SQLx database migrations
├── Cargo.toml
└── README.md                    # This file


⸻

Contributing
	1.	Fork the repo
	2.	Create a feature branch (git checkout -b feat/my-feature)
	3.	Run tests (cargo test) and benchmarks (cargo bench)
	4.	Submit a pull request

⸻

License

MIT © Andrew Odiit