# 🦀 Rust Orderbook Engine

A high-performance matching engine and limit order book written in Rust, built for low-latency electronic trading systems.

---

### ⚡ Features
- ✅ Price-time priority matching
- ✅ Limit and market order support
- ✅ Real-time trade feeds (WebSocket)
- ✅ Fast, safe, and memory-efficient
- ✅ Modular core for integration or simulation

> Designed for speed. Built with safety. Powered by Rust.

---

### 🧱 Architecture Overview


                  +---------------------+
                  |   Web UI (TS/React) |
                  |   Order Book Chart  |
                  +---------------------+
                            |
                      WebSocket (trades/quotes)
                            |
                    +-----------------+
                    | WebSocket Server|
                    +-----------------+
                            |
                +-------------------------+
                | Order Matching Engine   |
                |   - Limit Orders        |
                |   - Market Orders       |
                |   - Trade Matching      |
                |   - Partial Fills       |
                +-------------------------+
                  ↑               ↑
     Submit Order API / HTTP       Trade Log (Disk or In-Mem)
                  ↑
                  +------------------+
                  | REST API (Axum)  |
                  +------------------+
