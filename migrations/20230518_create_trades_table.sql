CREATE TABLE IF NOT EXISTS trades (
  id            BIGSERIAL PRIMARY KEY,
  price         NUMERIC(20,6) NOT NULL,
  quantity      BIGINT      NOT NULL,
  maker_id      BIGINT      NOT NULL,
  taker_id      BIGINT      NOT NULL,
  timestamp_utc TIMESTAMPTZ NOT NULL
);