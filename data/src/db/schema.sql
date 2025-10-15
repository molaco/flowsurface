-- FlowSurface Database Schema
-- Version: 1
-- Embedded at compile time via include_str!

-- Schema version tracking table
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    description VARCHAR
);

-- Exchanges table: Maps exchange enum variants to IDs
CREATE TABLE IF NOT EXISTS exchanges (
    exchange_id TINYINT PRIMARY KEY,
    name VARCHAR NOT NULL UNIQUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Tickers table: All trading pairs/instruments across exchanges
CREATE TABLE IF NOT EXISTS tickers (
    ticker_id INTEGER PRIMARY KEY,
    exchange_id TINYINT NOT NULL,
    symbol VARCHAR NOT NULL,
    min_ticksize DECIMAL(18, 8),
    min_qty DECIMAL(18, 8),
    contract_size DECIMAL(18, 8),
    market_type VARCHAR,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (exchange_id) REFERENCES exchanges(exchange_id),
    UNIQUE (exchange_id, symbol)
);
CREATE INDEX IF NOT EXISTS idx_tickers_symbol ON tickers(symbol);
CREATE INDEX IF NOT EXISTS idx_tickers_exchange ON tickers(exchange_id);

-- Trades table: Individual trade executions (time-series data)
CREATE TABLE IF NOT EXISTS trades (
    trade_id BIGINT PRIMARY KEY,
    ticker_id INTEGER NOT NULL,
    timestamp BIGINT NOT NULL,
    price DECIMAL(18, 8) NOT NULL,
    quantity DECIMAL(18, 8) NOT NULL,
    is_buyer_maker BOOLEAN NOT NULL,
    FOREIGN KEY (ticker_id) REFERENCES tickers(ticker_id)
);
CREATE INDEX IF NOT EXISTS idx_trades_ticker_time ON trades(ticker_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_trades_timestamp ON trades(timestamp);

-- Klines/Candlesticks table: OHLCV data for multiple timeframes
CREATE TABLE IF NOT EXISTS klines (
    kline_id BIGINT PRIMARY KEY,
    ticker_id INTEGER NOT NULL,
    timeframe VARCHAR NOT NULL,
    candle_time BIGINT NOT NULL,
    open_price DECIMAL(18, 8) NOT NULL,
    high_price DECIMAL(18, 8) NOT NULL,
    low_price DECIMAL(18, 8) NOT NULL,
    close_price DECIMAL(18, 8) NOT NULL,
    volume DECIMAL(18, 8) NOT NULL,
    num_trades INTEGER,
    FOREIGN KEY (ticker_id) REFERENCES tickers(ticker_id),
    UNIQUE (ticker_id, timeframe, candle_time)
);
CREATE INDEX IF NOT EXISTS idx_klines_ticker_time ON klines(ticker_id, timeframe, candle_time);

-- Depth snapshots table: Order book snapshots at specific timestamps
CREATE TABLE IF NOT EXISTS depth_snapshots (
    snapshot_id BIGINT PRIMARY KEY,
    ticker_id INTEGER NOT NULL,
    timestamp BIGINT NOT NULL,
    bids VARCHAR NOT NULL,  -- JSON array of [price, quantity] arrays
    asks VARCHAR NOT NULL,  -- JSON array of [price, quantity] arrays
    FOREIGN KEY (ticker_id) REFERENCES tickers(ticker_id)
);
CREATE INDEX IF NOT EXISTS idx_depth_ticker_time ON depth_snapshots(ticker_id, timestamp);

-- Open Interest table: Funding rates and open interest for derivatives
CREATE TABLE IF NOT EXISTS open_interest (
    oi_id BIGINT PRIMARY KEY,
    ticker_id INTEGER NOT NULL,
    timestamp BIGINT NOT NULL,
    open_interest DECIMAL(18, 8),
    funding_rate DECIMAL(18, 8),
    FOREIGN KEY (ticker_id) REFERENCES tickers(ticker_id),
    UNIQUE (ticker_id, timestamp)
);
CREATE INDEX IF NOT EXISTS idx_oi_ticker_time ON open_interest(ticker_id, timestamp);

-- Footprint data table: Price levels with volume breakdown
CREATE TABLE IF NOT EXISTS footprint_data (
    footprint_id BIGINT PRIMARY KEY,
    ticker_id INTEGER NOT NULL,
    candle_time BIGINT NOT NULL,
    timeframe VARCHAR NOT NULL,
    price_level DECIMAL(18, 8) NOT NULL,
    buy_volume DECIMAL(18, 8) NOT NULL,
    sell_volume DECIMAL(18, 8) NOT NULL,
    delta DECIMAL(18, 8) NOT NULL,
    num_trades INTEGER NOT NULL,
    FOREIGN KEY (ticker_id) REFERENCES tickers(ticker_id)
);
CREATE INDEX IF NOT EXISTS idx_footprint_ticker_candle ON footprint_data(ticker_id, candle_time, timeframe);
CREATE INDEX IF NOT EXISTS idx_footprint_price ON footprint_data(ticker_id, price_level);

-- Order runs table: Consecutive orders at same price level
CREATE TABLE IF NOT EXISTS order_runs (
    run_id BIGINT PRIMARY KEY,
    ticker_id INTEGER NOT NULL,
    start_time TIMESTAMP NOT NULL,
    end_time TIMESTAMP NOT NULL,
    price_level DECIMAL(18, 8) NOT NULL,
    total_volume DECIMAL(18, 8) NOT NULL,
    num_orders INTEGER NOT NULL,
    is_buy BOOLEAN NOT NULL,
    FOREIGN KEY (ticker_id) REFERENCES tickers(ticker_id)
);
CREATE INDEX IF NOT EXISTS idx_runs_ticker_time ON order_runs(ticker_id, start_time);

-- Volume profiles table: Volume distribution at price levels
CREATE TABLE IF NOT EXISTS volume_profiles (
    profile_id BIGINT PRIMARY KEY,
    ticker_id INTEGER NOT NULL,
    start_time TIMESTAMP NOT NULL,
    end_time TIMESTAMP NOT NULL,
    price_level DECIMAL(18, 8) NOT NULL,
    total_volume DECIMAL(18, 8) NOT NULL,
    buy_volume DECIMAL(18, 8) NOT NULL,
    sell_volume DECIMAL(18, 8) NOT NULL,
    FOREIGN KEY (ticker_id) REFERENCES tickers(ticker_id)
);
CREATE INDEX IF NOT EXISTS idx_vprofile_ticker_time ON volume_profiles(ticker_id, start_time, end_time);
CREATE INDEX IF NOT EXISTS idx_vprofile_price ON volume_profiles(ticker_id, price_level);

-- Insert initial schema version
INSERT INTO schema_version (version, description)
VALUES (1, 'Initial schema with all core tables')
ON CONFLICT DO NOTHING;
