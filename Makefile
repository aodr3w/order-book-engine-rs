# Use bash for nicer loops
SHELL := /bin/bash

# ---- Config (override on the command line) ----
HOST  ?= http://127.0.0.1
PORT  ?= 3000
PAIR  ?= BTC-USD
LIMIT ?= 5
AFTER ?=           # base64-ish cursor from the API (optional)
PAGES ?= 5

BASE := $(HOST):$(PORT)

# ---- Convenience targets ----
.PHONY: help server simulate wait seed trades trades-pages fmt clippy bench

help:
	@echo "Targets:"
	@echo "  make simulate [PORT=3000] [SECS=]      # full stack: server+MM+sim (SECS empty = run until Ctrl-C)"
	@echo "  make server   [PORT=3000]              # server only"
	@echo "  make wait     [PAIR=BTC-USD]           # wait until /book is live"
	@echo "  make seed                               # seed resting orders @48/@52"
	@echo "  make trades  [PAIR][LIMIT][AFTER]      # single page of trades"
	@echo "  make trades-pages [PAGES][LIMIT][AFTER]# paginate trades with jq"
	@echo "  make fmt | clippy | bench              # dev helpers"

# Runs only the API server (use Ctrl-C to stop)
serve:
	cargo run --release -- serve $(PORT)

# Runs server + market-maker + attacker; SECS empty => infinite until Ctrl-C
simulate:
	@if [ -z "$(SECS)" ]; then \
		cargo run --release -- simulate $(PORT); \
	else \
		cargo run --release -- simulate $(PORT) $(SECS); \
	fi

# Wait until the server answers /book
wait:
	@echo "Waiting for $(BASE)/book/$(PAIR) ..."
	@until curl -s "$(BASE)/book/$(PAIR)" | grep -q '"bids"'; do sleep 0.05; done
	@echo "OK"

# Seed two resting limits @48/@52
seed:
	@curl -s -X POST "$(BASE)/orders" -H 'content-type: application/json' \
	  -d '{"side":"Buy","order_type":"Limit","price":48,"quantity":10,"symbol":"BTC-USD"}' >/dev/null
	@curl -s -X POST "$(BASE)/orders" -H 'content-type: application/json' \
	  -d '{"side":"Sell","order_type":"Limit","price":52,"quantity":10,"symbol":"BTC-USD"}' >/dev/null
	@echo "Seeded @48/@52"

# One page of trades; --data-urlencode avoids zsh globbing/quoting issues
trades:
	@curl -s -G "$(BASE)/trades/$(PAIR)" \
	  --data-urlencode limit=$(LIMIT) \
	  $(if $(AFTER),--data-urlencode after=$(AFTER),)

# Paginate trades; prints .items and walks .next using jq
trades-pages:
	@command -v jq >/dev/null || { echo "jq is required (brew install jq)"; exit 1; }
	@AFTER='$(AFTER)'; \
	for ((i=1;i<=$(PAGES);i++)); do \
	  echo "=== Page $$i ==="; \
	  if [ -n "$$AFTER" ]; then \
	    RESP=$$(curl -s -G "$(BASE)/trades/$(PAIR)" --data-urlencode limit=$(LIMIT) --data-urlencode after="$$AFTER"); \
	  else \
	    RESP=$$(curl -s -G "$(BASE)/trades/$(PAIR)" --data-urlencode limit=$(LIMIT)); \
	  fi; \
	  echo "$$RESP" | jq '.items'; \
	  AFTER=$$(echo "$$RESP" | jq -r '.next // empty'); \
	  [ -z "$$AFTER" ] && break; \
	done

# Dev helpers
fmt:
	cargo fmt --all
clippy:
	cargo clippy --all-targets -- -D warnings
bench:
	cargo bench