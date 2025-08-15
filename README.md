# Order Book Engine

A high‑performance matching engine for multiple trading pairs, with REST & WebSocket APIs, persistent trade storage, a lightweight market‑maker, and a noisy traffic simulator.

## Features

- **Limit & Market Orders:** FIFO, price‑time priority, partial fills, and crossing.
- **In‑Memory Books:** One order book per trading pair (e.g. `BTC-USD`, `ETH-USD`) using price‑level queues.
- **Persistence:** Trades serialized with Bincode and stored in ParityDB for durable, per‑pair history.
- **REST API:** Submit orders, cancel, query order book & trade history.
- **WebSocket API:** Stream live book snapshots and trade events.
- **Market Maker Bot:** Two‑sided quoting around mid‑price via REST+WS.
- **Simulation Harness:** Adversarial load testing with random orders.
- **Benchmarking:** Criterion benchmarks for matching performance.

## Prerequisites

- Rust (1.70+)
- Cargo
- gnuplot (for Criterion plots, or use plotters fallback)
- Linux/macOS/Windows

## Repository Layout

```
├── benches/benchmark.rs      # Criterion benchmarks
├── src/
│   ├── api.rs                # HTTP & WS handlers
│   ├── instrument.rs         # Asset & Pair types
│   ├── market_maker.rs       # Market maker bot
│   ├── orderbook.rs          # Matching engine
│   ├── orders.rs             # Order definitions
│   ├── simulate.rs           # Simulation harness
│   ├── state.rs              # Shared AppState
│   ├── store.rs              # ParityDB-backed store
│   ├── trade.rs              # Trade struct
│   ├── errors.rs             # Error types
│   └── main.rs               # Entry point
└── README.md
```

# Getting Started

### Clone & build

**SSH**
```bash
git clone git@github.com:aodr3w/order-book-engine-rs.git
```
**HTTPS**
```bash
git clone https://github.com/aodr3w/order-book-engine-rs.git
```

```bash
cd order-book-engine-rs
cargo build --release
```

## Running

### Server only
Launch HTTP & WS server with a ParityDB store at `./trade_store`:
```bash
cargo run --release -- serve 3000
```

### Full simulation (server + market‑maker + simulator)
Run indefinitely (Ctrl+C to stop):
```bash
cargo run --release -- simulate 3000
```
Run for 5 seconds:
```bash
cargo run --release -- simulate 3000 5
```

---

## API Quickstart

### POST /orders — create an order
Limit order:
```bash
curl -s -X POST http://127.0.0.1:3000/orders   -H "Content-Type: application/json"   -d '{
    "side": "Buy",
    "order_type": "Limit",
    "price": 50,
    "quantity": 1,
    "symbol": "BTC-USD"
  }'
```
Example response (note: `order_id` is a **string**):
```json
{
  "order_id": "153952592511588313400735895982354310302",
  "trades": []
}
```

Market order:
```bash
curl -s -X POST http://127.0.0.1:3000/orders   -H "Content-Type: application/json"   -d '{
    "side": "Sell",
    "order_type": "Market",
    "quantity": 5,
    "symbol": "BTC-USD"
  }'
```
Example response:
```json
{
  "order_id": "78628407350254234892786983129279276375",
  "trades": [
    {
      "price": 52,
      "quantity": 5,
      "maker_id": "…",
      "taker_id": "…",
      "timestamp": "...",
      "symbol": "BTC-USD"
    }
  ]
}
```

#### Notes
- `symbol` must be a supported pair (see **Supported Symbols** below). Invalid symbols return `400` with a `supported` list.
- `quantity` must be > 0 (else `400`).

### NEW: DELETE /orders/{pair}/{id} — cancel an order
Cancels a previously posted order. `id` is the order ID returned by `POST /orders`.
```bash
OID=$(curl -s -X POST http://127.0.0.1:3000/orders   -H "Content-Type: application/json"   -d '{"side":"Buy","order_type":"Limit","price":48,"quantity":1,"symbol":"BTC-USD"}'   | jq -r .order_id)

curl -s -X DELETE "http://127.0.0.1:3000/orders/BTC-USD/$OID"
```
Response on success:
```json
{"status":"cancelled"}
```
Errors:
- `400` — unsupported pair
- `404` — order not found

### GET /book/{pair} — current order book snapshot
```bash
curl -s http://127.0.0.1:3000/book/BTC-USD | jq
```

### GET /trades/{pair}?limit=&after= — paginated trade history
```bash
curl -i "http://127.0.0.1:3000/trades/BTC-USD?limit=5000"
```
Headers include the soft cap you actually got:
```
x-effective-limit: 1000
```
Body:
```json
{
  "items": [ /* trades oldest→newest */ ],
  "next": "opaque-cursor-or-null"
}
```
- `limit`: soft‑capped at 1000; `limit=0` → `400`.
- `after`: opaque cursor string from a previous page. An invalid or cross‑pair cursor returns `400`.

### WebSocket — live snapshots & trades
```bash
websocat ws://127.0.0.1:3000/ws/BTC-USD
```
Frames are internally tagged:
```json
{"type":"BookSnapshot","data":{"pair":"BTC-USD","bids":[[48,10],…],"asks":[[52,10],…]}}
{"type":"Trade","data":{"price":50,"quantity":2,"maker_id":"…","taker_id":"…","timestamp":"…","symbol":"BTC-USD"}}
```

### Errors
All errors are JSON:
```json
{ "error": "…" }
```
Examples:
- Unsupported pair on path:
```json
{ "error": "unsupported symbol `BTC-EUR`", "supported": ["BTC-USD","ETH-USD"] }
```
- Bad pagination cursor:
```json
{ "error": "invalid `after` cursor" }
```

### Supported Symbols
Current spot pairs:
```
BTC-USD
ETH-USD
```

---

## Development & Testing

Run unit and integration tests:
```bash
cargo test
```

Run benchmarks:
```bash
cargo bench
```

---

## Notes for JavaScript Clients

- **IDs are big.** Order IDs (`order_id`) and trade IDs (`maker_id`, `taker_id`) are 128‑bit values. We serialize IDs as **strings** in JSON to avoid precision loss in JS (`Number.MAX_SAFE_INTEGER` is only 53 bits).
- Treat all IDs as opaque strings. Do not parse or assume numeric range.

---

## License

MIT
