-- Add migration script here
-- +migrate Up
ALTER TABLE trades
ADD COLUMN symbol TEXT NOT NULL DEFAULT '';

-- +migrate Down
ALTER TABLE trades
DROP COLUMN symbol;