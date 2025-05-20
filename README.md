Rust Orderbook Engine

An basic implementation of an orderbook engine for educational purposes.

🧑‍🎓 Learning Outcomes

	•	Rust mastery: ownership/borrowing, async/await with Tokio, synchronization (Mutex, watch channel).

	•	Systems design: architecting a modular engine, REST+WebSocket APIs with Axum.

	•	Database integration: schema migrations and persistence using SQLx+Postgres.

	•	Trading concepts: limit vs market orders, price-time priority matching, two-sided quoting market making.

	•	Performance benchmarking: using Criterion to measure matching engine throughput and latency.

⸻

⚡ Features
	•	Order Matching Engine.

	•	Price-time priority with FIFO queues per price level.

	•	Supports limit and market orders, partial fills, crossing orders.

	•	REST & WebSocket API (Axum).

	•	POST /orders to create orders, DELETE /orders/{id} to cancel.

	•	GET /book and live WsFrame::BookSnapshot updates.

	•	GET /trades and live WsFrame::Trade feeds.

	•	Market Maker Bot.

	•	Two-sided quoting around mid-price with adjustable spread & cadence.

	•	Reacts to book snapshots over WebSocket and cancels/reposts quotes.

	•	Simulator & P&L.

	•	Simulates aggressive market orders against the engine to measure realized P&L and inventory.

	•	Persistence.

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

#TODO add installation steps, copy keiji