# DuckDB Implementation Guide for FlowSurface

## Executive Summary

This document provides a comprehensive guide for implementing DuckDB as a persistent storage solution for the FlowSurface trading terminal. Currently, the application uses entirely in-memory data structures with minimal file-based persistence (JSON configuration files). This implementation plan enables persistent market data storage, historical analysis capabilities, and reduced memory footprint while maintaining real-time performance.

**Current State:**
- In-memory storage using `BTreeMap`, `FxHashMap`, `VecDeque`
- JSON-based application state (`saved-state.json`)
- ZIP archives for Binance historical trades (4-day retention)
- No database layer

**Target State:**
- Hybrid architecture: Hot data in-memory, warm/cold data in DuckDB
- Persistent market data (trades, klines, order book history)
- SQL-based analytics and historical queries
- Reduced memory footprint for long-running sessions

---

## Table of Contents

1. [Current Architecture Analysis](#1-current-architecture-analysis)
2. [Data Types and Volume Analysis](#2-data-types-and-volume-analysis)
3. [DuckDB Schema Design](#3-duckdb-schema-design)
4. [Integration Points](#4-integration-points)
5. [CRUD Operations Implementation](#5-crud-operations-implementation)
6. [Data Migration Strategy](#6-data-migration-strategy)
7. [Performance Optimization](#7-performance-optimization)
8. [Testing Strategy](#8-testing-strategy)
9. [Implementation Timeline](#9-implementation-timeline)
10. [Rollback and Risk Mitigation](#10-rollback-and-risk-mitigation)

---

## 1. Current Architecture Analysis

### 1.1 Storage Mechanisms

**In-Memory Structures:**
- `TimeSeries<D>` - BTreeMap<u64, DataPoint> for time-indexed data
- `TickAggr` - Vec<TickAccumulation> for trade-based aggregation
- `Depth` - BTreeMap<Price, f32> for order book (bids/asks)
- `KlineTrades` - FxHashMap<Price, GroupedTrades> for footprint data
- `HistoricalDepth` - BTreeMap<Price, Vec<OrderRun>> for depth history

**File-Based Storage:**
- `saved-state.json` at `~/.local/share/flowsurface/`
- Market data ZIP archives at `market_data/binance/data/futures/{um,cm}/daily/aggTrades/`
- Log files: `flowsurface-current.log`, `flowsurface-previous.log`

### 1.2 Data Flow Architecture

```
WebSocket → Event → Dashboard → Pane Content → In-Memory Storage
                                              ↓
                                        [Future: DuckDB]
```

**Current Event Flow (src/main.rs:136-175):**
1. Exchange WebSocket receives data
2. Event enum dispatched (DepthReceived, KlineReceived, TradeReceived)
3. Dashboard distributes to matching panes
4. Panes update their in-memory data structures
5. On shutdown: Save JSON state only

### 1.3 Cleanup Mechanisms

| Component | Retention | Cleanup Location |
|-----------|-----------|------------------|
| Heatmap | 4800 datapoints max | src/chart/heatmap.rs:235-253 |
| Ladder | 8 minutes | data/src/chart/ladder.rs:4 |
| Time & Sales | 2 minutes (1-60 configurable) | Configurable retention |
| Market Data ZIPs | 4 days | data/src/lib.rs:176-188 |

---

## 2. Data Types and Volume Analysis

### 2.1 Real-Time Market Data (WebSocket Feeds)

#### Trade Data (`exchange/src/lib.rs:574`)
```rust
struct Trade {
    time: u64,        // millisecond timestamp
    is_sell: bool,    // order side
    price: Price,     // custom fixed-point type
    qty: f32,         // trade quantity
}
```
- **Source:** Exchange WebSocket streams
- **Frequency:** 10-1000+ trades/second per ticker
- **Volume:** ~100 KB - 10 MB per ticker in memory
- **Persistence Need:** HIGH - Historical analysis, footprint reconstruction

#### Order Book Depth (`exchange/src/depth.rs`)
```rust
struct Depth {
    bids: BTreeMap<Price, f32>,
    asks: BTreeMap<Price, f32>,
}
```
- **Source:** WebSocket L2 orderbook updates
- **Frequency:** 10-100 updates/second
- **Volume:** ~50-500 KB per ticker
- **Persistence Need:** MEDIUM - Heatmap reconstruction, market replay

#### Kline/Candlestick Data (`exchange/src/lib.rs:583`)
```rust
struct Kline {
    time: u64,
    open: Price,
    high: Price,
    low: Price,
    close: Price,
    volume: (f32, f32),  // buy/sell volume tuple
}
```
- **Timeframes:** 1m, 3m, 5m, 15m, 30m, 1h, 2h, 4h, 6h, 12h, 1d
- **Volume:** ~1000-5000 candles per visible range
- **Persistence Need:** HIGH - Chart continuity across sessions

### 2.2 Aggregated/Processed Data

#### Footprint Chart Data (`data/src/chart/kline.rs`)
```rust
struct KlineDataPoint {
    kline: Kline,
    footprint: KlineTrades,  // FxHashMap<Price, GroupedTrades>
}
```
- **Structure:** Price-level trade aggregation within each kline
- **Volume:** 100-500 klines × 10-100 price levels each = ~2-10 MB
- **Persistence Need:** MEDIUM - Can be rebuilt from trades, but expensive

#### Heatmap Data (`data/src/chart/heatmap.rs`)
```rust
struct HeatmapDataPoint {
    grouped_trades: Box<[GroupedTrade]>,
    buy_sell: (f32, f32),
}
```
- **Aggregation:** By time intervals (100ms, 200ms, 300ms, 500ms, 1s)
- **Volume:** ~5-20 MB per ticker
- **Persistence Need:** LOW - Primarily real-time visualization

### 2.3 Configuration & State (Currently Persisted)

#### Application State (`data/src/config/state.rs`)
```rust
struct State {
    layout_manager: Layouts,
    selected_theme: Theme,
    custom_theme: Option<Theme>,
    main_window: Option<WindowSpec>,
    timezone: UserTimezone,
    sidebar: Sidebar,
    scale_factor: ScaleFactor,
    audio_cfg: AudioStream,
    trade_fetch_enabled: bool,
    size_in_quote_currency: bool,
}
```
- **File:** `saved-state.json`
- **Size:** ~10-50 KB
- **Update Frequency:** On user action
- **Persistence Need:** ALREADY HANDLED - Keep JSON for now

### 2.4 Volume Estimates

| Data Type | In-Memory Size | Update Frequency | Recommended Persistence |
|-----------|---------------|------------------|------------------------|
| Trades (per ticker) | 100 KB - 10 MB | Continuous (ms) | DuckDB with time partitioning |
| Depth (per ticker) | 50-500 KB | 10-100x/sec | DuckDB (snapshots every 100ms) |
| Klines (per chart) | 1-5 MB | Per timeframe | DuckDB (indexed by timeframe) |
| Footprint data | 2-10 MB | Continuous | Rebuild from trades (optional cache) |
| Heatmap data | 5-20 MB | Per interval | In-memory only |
| App state | N/A | On save | Keep JSON (migrate to DuckDB later) |
| Bulk trade ZIPs | 10-500 MB/symbol/day | N/A | Convert to Parquet, query via DuckDB |

---

## 3. DuckDB Schema Design

### 3.1 Core Entity Tables

```sql
-- Exchanges registry
CREATE TABLE exchanges (
    exchange_id TINYINT PRIMARY KEY,
    exchange_name VARCHAR(30) NOT NULL,
    market_type VARCHAR(20) NOT NULL,
    CONSTRAINT chk_market_type CHECK (market_type IN ('Spot', 'LinearPerps', 'InversePerps'))
);

-- Tickers (instruments/symbols)
CREATE TABLE tickers (
    ticker_id INTEGER PRIMARY KEY,
    exchange_id TINYINT NOT NULL,
    symbol VARCHAR(28) NOT NULL,
    display_symbol VARCHAR(28),
    min_ticksize_power TINYINT NOT NULL,
    min_qty FLOAT NOT NULL,
    contract_size FLOAT,
    market_type VARCHAR(20) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (exchange_id) REFERENCES exchanges(exchange_id),
    UNIQUE (exchange_id, symbol)
);

CREATE INDEX idx_tickers_exchange ON tickers(exchange_id);
CREATE INDEX idx_tickers_symbol ON tickers(symbol);
```

### 3.2 Market Data Tables (Time-Series Optimized)

```sql
-- Trades table (high-frequency inserts)
CREATE TABLE trades (
    trade_id BIGINT PRIMARY KEY,
    ticker_id INTEGER NOT NULL,
    trade_time TIMESTAMP_NS NOT NULL,
    price DECIMAL(18, 8) NOT NULL,
    quantity FLOAT NOT NULL,
    is_sell BOOLEAN NOT NULL,
    FOREIGN KEY (ticker_id) REFERENCES tickers(ticker_id)
) PARTITION BY RANGE (trade_time);

CREATE INDEX idx_trades_time ON trades(trade_time);
CREATE INDEX idx_trades_ticker_time ON trades(ticker_id, trade_time);
CREATE INDEX idx_trades_price ON trades(price);

-- Klines/Candles (OHLCV data)
CREATE TABLE klines (
    kline_id BIGINT PRIMARY KEY,
    ticker_id INTEGER NOT NULL,
    timeframe VARCHAR(10) NOT NULL,
    candle_time TIMESTAMP NOT NULL,
    open_price DECIMAL(18, 8) NOT NULL,
    high_price DECIMAL(18, 8) NOT NULL,
    low_price DECIMAL(18, 8) NOT NULL,
    close_price DECIMAL(18, 8) NOT NULL,
    base_volume FLOAT NOT NULL,
    quote_volume FLOAT NOT NULL,
    FOREIGN KEY (ticker_id) REFERENCES tickers(ticker_id),
    UNIQUE (ticker_id, timeframe, candle_time)
) PARTITION BY RANGE (candle_time);

CREATE INDEX idx_klines_ticker_tf_time ON klines(ticker_id, timeframe, candle_time);

-- Order Book Depth Snapshots
CREATE TABLE depth_snapshots (
    snapshot_id BIGINT PRIMARY KEY,
    ticker_id INTEGER NOT NULL,
    snapshot_time TIMESTAMP_NS NOT NULL,
    last_update_id BIGINT NOT NULL,
    side VARCHAR(3) NOT NULL,
    price DECIMAL(18, 8) NOT NULL,
    quantity FLOAT NOT NULL,
    FOREIGN KEY (ticker_id) REFERENCES tickers(ticker_id),
    CONSTRAINT chk_side CHECK (side IN ('BID', 'ASK'))
) PARTITION BY RANGE (snapshot_time);

CREATE INDEX idx_depth_ticker_time ON depth_snapshots(ticker_id, snapshot_time);
CREATE INDEX idx_depth_price ON depth_snapshots(price);

-- Open Interest (for perpetuals/futures)
CREATE TABLE open_interest (
    oi_id BIGINT PRIMARY KEY,
    ticker_id INTEGER NOT NULL,
    oi_time TIMESTAMP NOT NULL,
    oi_value FLOAT NOT NULL,
    FOREIGN KEY (ticker_id) REFERENCES tickers(ticker_id),
    UNIQUE (ticker_id, oi_time)
) PARTITION BY RANGE (oi_time);

CREATE INDEX idx_oi_ticker_time ON open_interest(ticker_id, oi_time);
```

### 3.3 Aggregated Analytics Tables

```sql
-- Footprint data (price-level aggregation within candles)
CREATE TABLE footprint_data (
    footprint_id BIGINT PRIMARY KEY,
    kline_id BIGINT NOT NULL,
    price DECIMAL(18, 8) NOT NULL,
    buy_quantity FLOAT NOT NULL DEFAULT 0,
    sell_quantity FLOAT NOT NULL DEFAULT 0,
    buy_count INTEGER NOT NULL DEFAULT 0,
    sell_count INTEGER NOT NULL DEFAULT 0,
    first_trade_time TIMESTAMP_NS,
    last_trade_time TIMESTAMP_NS,
    FOREIGN KEY (kline_id) REFERENCES klines(kline_id),
    UNIQUE (kline_id, price)
);

CREATE INDEX idx_footprint_kline ON footprint_data(kline_id);
CREATE INDEX idx_footprint_price ON footprint_data(price);

-- Historical Depth (Order runs for heatmap)
CREATE TABLE order_runs (
    run_id BIGINT PRIMARY KEY,
    ticker_id INTEGER NOT NULL,
    price DECIMAL(18, 8) NOT NULL,
    start_time TIMESTAMP_NS NOT NULL,
    until_time TIMESTAMP_NS NOT NULL,
    quantity FLOAT NOT NULL,
    is_bid BOOLEAN NOT NULL,
    FOREIGN KEY (ticker_id) REFERENCES tickers(ticker_id)
) PARTITION BY RANGE (start_time);

CREATE INDEX idx_order_runs_ticker_time ON order_runs(ticker_id, start_time, until_time);
CREATE INDEX idx_order_runs_price_time ON order_runs(price, start_time, until_time);

-- Volume Profile data
CREATE TABLE volume_profiles (
    profile_id BIGINT PRIMARY KEY,
    ticker_id INTEGER NOT NULL,
    window_start TIMESTAMP NOT NULL,
    window_end TIMESTAMP NOT NULL,
    price DECIMAL(18, 8) NOT NULL,
    total_volume FLOAT NOT NULL,
    buy_volume FLOAT NOT NULL,
    sell_volume FLOAT NOT NULL,
    is_poc BOOLEAN DEFAULT FALSE,
    FOREIGN KEY (ticker_id) REFERENCES tickers(ticker_id)
);

CREATE INDEX idx_volume_profiles_ticker_window ON volume_profiles(ticker_id, window_start, window_end);
CREATE INDEX idx_volume_profiles_poc ON volume_profiles(is_poc) WHERE is_poc = TRUE;
```

### 3.4 Analytical Views

```sql
-- Time and Sales view
CREATE VIEW time_and_sales AS
SELECT
    t.trade_time,
    tk.symbol,
    tk.display_symbol,
    e.exchange_name,
    t.price,
    t.quantity,
    t.is_sell,
    t.price * t.quantity AS notional_value
FROM trades t
JOIN tickers tk ON t.ticker_id = tk.ticker_id
JOIN exchanges e ON tk.exchange_id = e.exchange_id;

-- Heatmap aggregation view
CREATE VIEW heatmap_view AS
SELECT
    ticker_id,
    time_bucket(INTERVAL '100 milliseconds', trade_time) AS bucket_time,
    price,
    SUM(CASE WHEN NOT is_sell THEN quantity ELSE 0 END) AS buy_qty,
    SUM(CASE WHEN is_sell THEN quantity ELSE 0 END) AS sell_qty,
    COUNT(*) AS trade_count
FROM trades
GROUP BY ticker_id, bucket_time, price;
```

### 3.5 Schema Design Rationale

**Columnar Storage Benefits:**
- **Compression:** Timestamp columns compress 10-20x, price/quantity 5-10x
- **Scan Performance:** Only read columns needed for queries
- **Vectorized Execution:** Process thousands of rows per CPU cycle

**Partitioning Strategy:**
- **Monthly partitions** for trades (high volume)
- **Partition pruning** for time-range queries
- **Parallel processing** across partitions

**Data Type Choices:**
- `TIMESTAMP_NS` - Sub-millisecond precision (100ms, 200ms, 300ms timeframes)
- `DECIMAL(18,8)` - Exact price representation (avoid floating-point errors)
- `FLOAT` - Quantities (acceptable precision loss, saves space)
- `TINYINT/SMALLINT` - Enums and small ranges

**Query Performance Estimates:**
- Time-range trade queries: <100ms for 1M rows
- Order book reconstruction: <50ms
- Heatmap aggregations: <200ms for 1-day window
- Footprint calculations: <500ms for 80-100 candles

---

## 4. Integration Points

### 4.1 Database Manager Module

**Location:** `data/src/db/mod.rs`

```rust
use duckdb::{Connection, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct DatabaseManager {
    conn: Arc<Mutex<Connection>>,
    path: PathBuf,
}

impl DatabaseManager {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let conn = Connection::open(&db_path)?;

        // Initialize schema
        conn.execute_batch(include_str!("schema.sql"))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path: db_path,
        })
    }

    pub fn with_conn<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>
    {
        let conn = self.conn.lock().unwrap();
        f(&*conn)
    }
}
```

### 4.2 Application Initialization

**Location:** `src/main.rs:33-36`

```rust
// Add before app initialization
let db_path = data::data_path(Some("flowsurface.duckdb"));
let db_manager = data::db::DatabaseManager::new(db_path)
    .expect("Failed to initialize database");

let flowsurface = Flowsurface::new(db_manager)?;
```

### 4.3 Data Distribution Layer

**Location:** `src/screen/dashboard.rs:1192-1244`

Current flow:
```rust
fn distribute_fetched_data(&mut self, fetched_data: FetchedData) {
    // Distribute to matching panes
}
```

Enhanced flow with DuckDB:
```rust
fn distribute_fetched_data(&mut self, fetched_data: FetchedData) {
    // 1. Persist to DuckDB
    self.persist_fetched_data(&fetched_data);

    // 2. Distribute to in-memory panes (existing logic)
    // ...existing code...
}

fn persist_fetched_data(&self, data: &FetchedData) {
    match data {
        FetchedData::Trades { batch, ticker_info, .. } => {
            self.db.insert_trades(ticker_info, batch)?;
        }
        FetchedData::Klines { klines, ticker_info, timeframe, .. } => {
            self.db.insert_klines(ticker_info, timeframe, klines)?;
        }
        FetchedData::OpenInterest { oi_data, ticker_info, .. } => {
            self.db.insert_open_interest(ticker_info, oi_data)?;
        }
    }
}
```

### 4.4 Trade Fetching Enhancement

**Location:** `exchange/src/adapter/binance.rs:1657-1683`

```rust
// Current: Reads from ZIP files
pub fn fetch_trades_batched(...) -> Result<Vec<Trade>> {
    // Parse CSV from ZIP archives
}

// Enhanced: Check DuckDB first, then fetch
pub fn fetch_trades_batched(
    db: &DatabaseManager,
    ticker_info: &TickerInfo,
    start_time: u64,
    end_time: u64,
) -> Result<Vec<Trade>> {
    // 1. Try database first
    if let Ok(trades) = db.query_trades(ticker_info, start_time, end_time) {
        if !trades.is_empty() {
            return Ok(trades);
        }
    }

    // 2. Fallback to existing ZIP parsing or API fetch
    let trades = fetch_from_source(ticker_info, start_time, end_time)?;

    // 3. Persist newly fetched trades
    db.insert_trades(ticker_info, &trades)?;

    Ok(trades)
}
```

### 4.5 Cleanup Replacement

**Location:** `data/src/lib.rs:176-188`

Current:
```rust
pub fn cleanup_old_market_data() {
    // Delete ZIP files older than 4 days
}
```

Enhanced:
```rust
pub fn cleanup_old_market_data(db: &DatabaseManager) {
    // 1. Archive old data to Parquet
    db.export_old_data_to_parquet(4.days_ago())?;

    // 2. Delete from database
    db.delete_trades_older_than(4.days_ago())?;

    // 3. Vacuum to reclaim space
    db.vacuum()?;
}
```

### 4.6 Integration Summary

| Component | File | Line Range | Operation |
|-----------|------|------------|-----------|
| DB Manager | `data/src/db/mod.rs` | New | Connection, schema, queries |
| App Init | `src/main.rs` | 33-36 | Initialize DuckDB |
| Data Distribution | `src/screen/dashboard.rs` | 1192-1577 | Persist + distribute |
| Trade Fetching | `exchange/src/adapter/binance.rs` | 1657-1683 | DB-first fetch |
| Cleanup | `data/src/lib.rs` | 176-188 | Archive to Parquet |

---

## 5. CRUD Operations Implementation

### 5.1 Module Structure

```
data/src/
├── db/
│   ├── mod.rs              # DatabaseManager, connection management
│   ├── schema.sql          # DDL statements
│   ├── migrations.rs       # Schema versioning
│   ├── error.rs            # Custom error types
│   └── crud/
│       ├── mod.rs          # Re-exports
│       ├── trades.rs       # Trade CRUD operations
│       ├── klines.rs       # Kline CRUD operations
│       ├── depth.rs        # Depth snapshot CRUD
│       ├── footprint.rs    # Footprint data CRUD
│       └── settings.rs     # Settings key-value store
```

### 5.2 Trade CRUD Operations

**File:** `data/src/db/crud/trades.rs`

```rust
use duckdb::{params, Connection, Result};
use crate::exchange::{Trade, TickerInfo};

impl DatabaseManager {
    /// Insert batch of trades (optimized with Appender)
    pub fn insert_trades(&self, ticker_info: &TickerInfo, trades: &[Trade]) -> Result<()> {
        self.with_conn(|conn| {
            let ticker_id = self.get_or_create_ticker_id(ticker_info)?;

            let mut app = conn.appender("trades")?;
            for trade in trades {
                app.append_row(params![
                    generate_trade_id(),
                    ticker_id,
                    trade.time,
                    trade.price.to_f64(),
                    trade.qty,
                    trade.is_sell
                ])?;
            }
            Ok(())
        })
    }

    /// Query trades by time range
    pub fn query_trades(
        &self,
        ticker_info: &TickerInfo,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<Trade>> {
        self.with_conn(|conn| {
            let ticker_id = self.get_ticker_id(ticker_info)?;

            let mut stmt = conn.prepare(
                "SELECT trade_time, price, quantity, is_sell
                 FROM trades
                 WHERE ticker_id = ? AND trade_time BETWEEN ? AND ?
                 ORDER BY trade_time"
            )?;

            let rows = stmt.query_map(
                params![ticker_id, start_time, end_time],
                |row| {
                    Ok(Trade {
                        time: row.get(0)?,
                        price: Price::from_f64(row.get(1)?),
                        qty: row.get(2)?,
                        is_sell: row.get(3)?,
                    })
                }
            )?;

            rows.collect()
        })
    }

    /// Delete trades older than timestamp
    pub fn delete_trades_older_than(&self, cutoff_time: u64) -> Result<usize> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM trades WHERE trade_time < ?",
                params![cutoff_time]
            )
        })
    }

    /// Count trades for a ticker
    pub fn count_trades(&self, ticker_info: &TickerInfo) -> Result<i64> {
        self.with_conn(|conn| {
            let ticker_id = self.get_ticker_id(ticker_info)?;
            conn.query_row(
                "SELECT COUNT(*) FROM trades WHERE ticker_id = ?",
                params![ticker_id],
                |row| row.get(0)
            )
        })
    }
}
```

### 5.3 Kline CRUD Operations

**File:** `data/src/db/crud/klines.rs`

```rust
impl DatabaseManager {
    /// Insert klines for a specific timeframe
    pub fn insert_klines(
        &self,
        ticker_info: &TickerInfo,
        timeframe: &Timeframe,
        klines: &[Kline],
    ) -> Result<()> {
        self.with_conn(|conn| {
            let ticker_id = self.get_or_create_ticker_id(ticker_info)?;
            let tf_str = timeframe.to_string();

            let mut stmt = conn.prepare(
                "INSERT OR REPLACE INTO klines
                 (kline_id, ticker_id, timeframe, candle_time, open_price,
                  high_price, low_price, close_price, base_volume, quote_volume)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )?;

            for kline in klines {
                stmt.execute(params![
                    generate_kline_id(ticker_id, &tf_str, kline.time),
                    ticker_id,
                    tf_str,
                    kline.time,
                    kline.open.to_f64(),
                    kline.high.to_f64(),
                    kline.low.to_f64(),
                    kline.close.to_f64(),
                    kline.volume.0,
                    kline.volume.1,
                ])?;
            }

            Ok(())
        })
    }

    /// Query klines by time range and timeframe
    pub fn query_klines(
        &self,
        ticker_info: &TickerInfo,
        timeframe: &Timeframe,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<Kline>> {
        self.with_conn(|conn| {
            let ticker_id = self.get_ticker_id(ticker_info)?;

            let mut stmt = conn.prepare(
                "SELECT candle_time, open_price, high_price, low_price,
                        close_price, base_volume, quote_volume
                 FROM klines
                 WHERE ticker_id = ? AND timeframe = ?
                   AND candle_time BETWEEN ? AND ?
                 ORDER BY candle_time"
            )?;

            let rows = stmt.query_map(
                params![ticker_id, timeframe.to_string(), start_time, end_time],
                |row| {
                    Ok(Kline {
                        time: row.get(0)?,
                        open: Price::from_f64(row.get(1)?),
                        high: Price::from_f64(row.get(2)?),
                        low: Price::from_f64(row.get(3)?),
                        close: Price::from_f64(row.get(4)?),
                        volume: (row.get(5)?, row.get(6)?),
                    })
                }
            )?;

            rows.collect()
        })
    }

    /// Load TimeSeries from database
    pub fn load_timeseries(
        &self,
        ticker_info: &TickerInfo,
        timeframe: &Timeframe,
        start_time: u64,
        end_time: u64,
    ) -> Result<TimeSeries<KlineDataPoint>> {
        let klines = self.query_klines(ticker_info, timeframe, start_time, end_time)?;

        let mut datapoints = BTreeMap::new();
        for kline in klines {
            let dp = KlineDataPoint {
                kline,
                footprint: KlineTrades::default(),
                // Load footprint separately if needed
            };
            datapoints.insert(dp.kline.time, dp);
        }

        Ok(TimeSeries {
            datapoints,
            interval: *timeframe,
            tick_size: ticker_info.min_ticksize.to_price_step(),
        })
    }
}
```

### 5.4 Depth Snapshot CRUD

**File:** `data/src/db/crud/depth.rs`

```rust
impl DatabaseManager {
    /// Insert depth snapshot
    pub fn insert_depth_snapshot(
        &self,
        ticker_info: &TickerInfo,
        snapshot_time: u64,
        depth: &Depth,
        last_update_id: u64,
    ) -> Result<()> {
        self.with_conn(|conn| {
            let ticker_id = self.get_or_create_ticker_id(ticker_info)?;

            let mut app = conn.appender("depth_snapshots")?;

            // Insert bids
            for (price, qty) in &depth.bids {
                app.append_row(params![
                    generate_snapshot_id(),
                    ticker_id,
                    snapshot_time,
                    last_update_id,
                    "BID",
                    price.to_f64(),
                    qty
                ])?;
            }

            // Insert asks
            for (price, qty) in &depth.asks {
                app.append_row(params![
                    generate_snapshot_id(),
                    ticker_id,
                    snapshot_time,
                    last_update_id,
                    "ASK",
                    price.to_f64(),
                    qty
                ])?;
            }

            Ok(())
        })
    }

    /// Query depth snapshot at specific time
    pub fn query_depth_snapshot(
        &self,
        ticker_info: &TickerInfo,
        snapshot_time: u64,
    ) -> Result<Depth> {
        self.with_conn(|conn| {
            let ticker_id = self.get_ticker_id(ticker_info)?;

            let mut stmt = conn.prepare(
                "SELECT side, price, quantity
                 FROM depth_snapshots
                 WHERE ticker_id = ? AND snapshot_time = ?
                 ORDER BY price"
            )?;

            let mut bids = BTreeMap::new();
            let mut asks = BTreeMap::new();

            let rows = stmt.query_map(params![ticker_id, snapshot_time], |row| {
                let side: String = row.get(0)?;
                let price = Price::from_f64(row.get(1)?);
                let qty: f32 = row.get(2)?;
                Ok((side, price, qty))
            })?;

            for row in rows {
                let (side, price, qty) = row?;
                match side.as_str() {
                    "BID" => bids.insert(price, qty),
                    "ASK" => asks.insert(price, qty),
                    _ => None,
                };
            }

            Ok(Depth { bids, asks })
        })
    }
}
```

### 5.5 Helper Methods

```rust
impl DatabaseManager {
    /// Get or create ticker ID
    fn get_or_create_ticker_id(&self, ticker_info: &TickerInfo) -> Result<i32> {
        if let Ok(id) = self.get_ticker_id(ticker_info) {
            return Ok(id);
        }

        self.with_conn(|conn| {
            // Get exchange_id
            let exchange_id = self.get_or_create_exchange_id(&ticker_info.exchange())?;

            // Insert ticker
            conn.execute(
                "INSERT INTO tickers
                 (ticker_id, exchange_id, symbol, min_ticksize_power, min_qty, market_type)
                 VALUES (?, ?, ?, ?, ?, ?)",
                params![
                    generate_ticker_id(),
                    exchange_id,
                    ticker_info.symbol(),
                    ticker_info.min_ticksize.power(),
                    ticker_info.min_qty,
                    ticker_info.market_type().to_string()
                ]
            )?;

            self.get_ticker_id(ticker_info)
        })
    }

    /// Get existing ticker ID
    fn get_ticker_id(&self, ticker_info: &TickerInfo) -> Result<i32> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT ticker_id FROM tickers
                 WHERE exchange_id = (SELECT exchange_id FROM exchanges WHERE exchange_name = ?)
                   AND symbol = ?",
                params![ticker_info.exchange().to_string(), ticker_info.symbol()],
                |row| row.get(0)
            )
        })
    }
}
```

---

## 6. Data Migration Strategy

### 6.1 Migration Phases

**Phase 1: Dual-Write System (v0.9.0)**
- Write to both in-memory structures AND DuckDB
- Read from in-memory (existing behavior)
- Validate data consistency
- Opt-in via environment variable: `FLOWSURFACE_USE_DUCKDB=1`

**Phase 2: Dual-Read with DB Priority (v0.10.0)**
- Try reading from DuckDB first
- Fallback to in-memory on errors
- Continue dual writes
- Auto-migrate existing data on first run

**Phase 3: Full Migration (v0.11.0)**
- Read exclusively from DuckDB
- Keep minimal in-memory cache for hot data
- Remove legacy file-based storage code

**Phase 4: DuckDB-Only (v1.0.0)**
- Remove all legacy storage code
- Optimize for DuckDB-first architecture

### 6.2 Migration Module

**File:** `data/src/db/migration.rs`

```rust
use duckdb::Connection;
use crate::aggr::time::TimeSeries;
use crate::chart::kline::KlineDataPoint;
use crate::chart::heatmap::HistoricalDepth;

pub struct TimeSeriesMigrator;

impl TimeSeriesMigrator {
    /// Migrate TimeSeries<KlineDataPoint> to DuckDB
    pub fn migrate_klines(
        conn: &Connection,
        timeseries: &TimeSeries<KlineDataPoint>,
        ticker_info: &TickerInfo,
        timeframe: &str,
    ) -> Result<usize> {
        let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;

        let mut stmt = conn.prepare(
            "INSERT INTO klines
             (kline_id, ticker_id, timeframe, candle_time, open_price,
              high_price, low_price, close_price, base_volume, quote_volume)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )?;

        let mut count = 0;
        for (timestamp, dp) in &timeseries.datapoints {
            stmt.execute(params![
                generate_kline_id(ticker_id, timeframe, *timestamp),
                ticker_id,
                timeframe,
                timestamp,
                dp.kline.open.to_f64(),
                dp.kline.high.to_f64(),
                dp.kline.low.to_f64(),
                dp.kline.close.to_f64(),
                dp.kline.volume.0,
                dp.kline.volume.1
            ])?;
            count += 1;
        }

        Ok(count)
    }

    /// Migrate footprint data
    pub fn migrate_footprints(
        conn: &Connection,
        timeseries: &TimeSeries<KlineDataPoint>,
        ticker_info: &TickerInfo,
        timeframe: &str,
    ) -> Result<usize> {
        let ticker_id = get_ticker_id(conn, ticker_info)?;

        let mut stmt = conn.prepare(
            "INSERT INTO footprint_data
             (footprint_id, kline_id, price, buy_quantity, sell_quantity,
              buy_count, sell_count, first_trade_time, last_trade_time)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )?;

        let mut count = 0;
        for (kline_ts, dp) in &timeseries.datapoints {
            let kline_id = generate_kline_id(ticker_id, timeframe, *kline_ts);

            for (price, group) in &dp.footprint.trades {
                stmt.execute(params![
                    generate_footprint_id(),
                    kline_id,
                    price.to_f64(),
                    group.buy_qty,
                    group.sell_qty,
                    group.buy_count,
                    group.sell_count,
                    group.first_time,
                    group.last_time
                ])?;
                count += 1;
            }
        }

        Ok(count)
    }
}

pub struct DepthMigrator;

impl DepthMigrator {
    pub fn migrate_historical_depth(
        conn: &Connection,
        depth: &HistoricalDepth,
        ticker_info: &TickerInfo,
    ) -> Result<usize> {
        let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;

        let mut stmt = conn.prepare(
            "INSERT INTO order_runs
             (run_id, ticker_id, price, start_time, until_time, quantity, is_bid)
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )?;

        let mut count = 0;
        for (price, runs) in &depth.price_levels {
            for run in runs {
                stmt.execute(params![
                    generate_run_id(),
                    ticker_id,
                    price.to_f64(),
                    run.start_time,
                    run.until_time,
                    run.qty(),
                    run.is_bid
                ])?;
                count += 1;
            }
        }

        Ok(count)
    }
}
```

### 6.3 ZIP Archive Migration

**File:** `data/src/db/archive_migration.rs`

```rust
use zip::ZipArchive;
use std::fs::File;
use walkdir::WalkDir;

pub struct ArchiveMigrator;

#[derive(Default, Debug)]
pub struct MigrationStats {
    pub files_processed: usize,
    pub trades_migrated: usize,
    pub errors: Vec<String>,
}

impl ArchiveMigrator {
    /// Migrate Binance aggTrades ZIP files to DuckDB
    pub fn migrate_zip_archives(
        conn: &Connection,
        market_data_path: &Path,
    ) -> Result<MigrationStats> {
        let mut stats = MigrationStats::default();

        // Find all ZIP files
        for entry in WalkDir::new(market_data_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension() == Some("zip".as_ref()))
        {
            match Self::migrate_single_archive(conn, entry.path()) {
                Ok(count) => {
                    stats.trades_migrated += count;
                    stats.files_processed += 1;
                }
                Err(e) => {
                    stats.errors.push(format!("{:?}: {}", entry.path(), e));
                }
            }
        }

        Ok(stats)
    }

    fn migrate_single_archive(conn: &Connection, zip_path: &Path) -> Result<usize> {
        let file = File::open(zip_path)?;
        let mut archive = ZipArchive::new(file)?;

        // Extract symbol and date from path
        let (ticker_info, date) = Self::parse_archive_path(zip_path)?;
        let ticker_id = get_or_create_ticker_id(conn, &ticker_info)?;

        let mut total = 0;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let mut reader = csv::ReaderBuilder::new()
                .has_headers(false)
                .from_reader(&mut file);

            // Batch insert trades
            let mut app = conn.appender("trades")?;

            for result in reader.records() {
                let record = result?;
                // Binance aggTrades CSV format:
                // agg_trade_id, price, quantity, first_trade_id, last_trade_id, timestamp, is_buyer_maker

                let trade_id: i64 = record[0].parse()?;
                let price: f64 = record[1].parse()?;
                let quantity: f32 = record[2].parse()?;
                let timestamp: u64 = record[5].parse()?;
                let is_sell: bool = record[6].parse::<bool>()?;

                app.append_row(params![
                    trade_id,
                    ticker_id,
                    timestamp,
                    price,
                    quantity,
                    is_sell
                ])?;

                total += 1;
            }
        }

        Ok(total)
    }

    fn parse_archive_path(path: &Path) -> Result<(TickerInfo, String)> {
        // Parse path like: market_data/binance/data/futures/um/daily/aggTrades/BTCUSDT/BTCUSDT-aggTrades-2024-01-15.zip
        // Extract BTCUSDT and date
        // Return TickerInfo and date string
        todo!("Implement path parsing based on Binance archive structure")
    }
}
```

### 6.4 One-Time Migration Command

**File:** `src/cli/migrate.rs`

```rust
use clap::Parser;

#[derive(Parser, Debug)]
pub struct MigrateCommand {
    /// Perform dry run without writing to database
    #[arg(long)]
    dry_run: bool,

    /// Create backup before migration
    #[arg(long, default_value = "true")]
    backup: bool,

    /// Migration source
    #[arg(long, default_value = "archives")]
    source: MigrationSource,
}

#[derive(Debug, Clone)]
pub enum MigrationSource {
    Archives,    // ZIP files
    Memory,      // Current in-memory state
    Both,
}

impl MigrateCommand {
    pub fn execute(&self) -> Result<()> {
        println!("Starting DuckDB migration...");

        // 1. Create backup
        if self.backup {
            println!("Creating backup...");
            let backup_mgr = BackupManager::new(data_path(Some("backups")))?;
            let backup = backup_mgr.create_pre_migration_backup()?;
            println!("Backup created at: {:?}", backup.path);
        }

        // 2. Initialize database
        let db_path = data_path(Some("flowsurface.duckdb"));
        let db = DatabaseManager::new(db_path)?;

        // 3. Run migrations
        let mut total_trades = 0;
        let mut total_klines = 0;

        match self.source {
            MigrationSource::Archives => {
                println!("Migrating ZIP archives...");
                let market_data = data_path(Some("market_data"));
                let stats = ArchiveMigrator::migrate_zip_archives(&db, &market_data)?;
                total_trades = stats.trades_migrated;

                if !stats.errors.is_empty() {
                    eprintln!("Errors encountered:");
                    for error in &stats.errors {
                        eprintln!("  - {}", error);
                    }
                }
            }
            _ => {
                // Handle other sources
            }
        }

        // 4. Verify migration
        println!("Verifying migration...");
        let verification = db.verify_data_integrity()?;

        if verification.is_valid() {
            println!("✓ Migration completed successfully!");
            println!("  Trades migrated: {}", total_trades);
            println!("  Klines migrated: {}", total_klines);
        } else {
            eprintln!("✗ Migration verification failed!");
            for error in verification.errors {
                eprintln!("  - {}", error);
            }
        }

        Ok(())
    }
}
```

### 6.5 State Manager with Dual-Write

**File:** `data/src/lib.rs`

```rust
pub struct StateManager {
    db: Option<DatabaseManager>,
    use_db: bool,
}

impl StateManager {
    pub fn new() -> Result<Self> {
        let use_db = std::env::var("FLOWSURFACE_USE_DUCKDB")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);

        let db = if use_db {
            let db_path = data_path(Some("flowsurface.duckdb"));
            Some(DatabaseManager::new(db_path)?)
        } else {
            None
        };

        Ok(Self { db, use_db })
    }

    pub fn save_trades(&self, ticker_info: &TickerInfo, trades: &[Trade]) -> Result<()> {
        // Always save to in-memory (existing behavior)
        // ... existing code ...

        // Additionally save to DuckDB if enabled
        if let Some(db) = &self.db {
            db.insert_trades(ticker_info, trades)?;
        }

        Ok(())
    }

    pub fn load_trades(
        &self,
        ticker_info: &TickerInfo,
        start: u64,
        end: u64,
    ) -> Result<Vec<Trade>> {
        // Try DuckDB first if enabled
        if let Some(db) = &self.db {
            match db.query_trades(ticker_info, start, end) {
                Ok(trades) if !trades.is_empty() => {
                    log::debug!("Loaded {} trades from DuckDB", trades.len());
                    return Ok(trades);
                }
                Err(e) => {
                    log::warn!("Failed to load from DuckDB, falling back: {}", e);
                }
                _ => {}
            }
        }

        // Fallback to in-memory or file-based loading
        // ... existing code ...
    }
}
```

---

## 7. Performance Optimization

### 7.1 Bulk Insert Optimization

**Use Appender API (Recommended)**

```rust
use duckdb::Appender;

pub fn bulk_insert_trades(conn: &Connection, trades: &[Trade]) -> Result<()> {
    let mut app = conn.appender("trades")?;

    for trade in trades {
        app.append_row(params![
            trade.id,
            trade.ticker_id,
            trade.time,
            trade.price.to_f64(),
            trade.qty,
            trade.is_sell
        ])?;
    }

    // Appender automatically flushes on drop
    Ok(())
}
```

**Performance Comparison:**
- **Row-by-row INSERT:** ~1,000 rows/sec
- **Batched INSERT in transaction:** ~10,000 rows/sec
- **Appender API:** ~100,000+ rows/sec
- **COPY FROM CSV:** ~500,000+ rows/sec

**Recommendation for FlowSurface:**
- Use **Appender** for real-time trade ingestion
- Use **COPY FROM** for bulk historical data imports
- Always wrap multiple operations in transactions

### 7.2 Query Optimization Strategies

**1. Index Optimization**

```sql
-- Composite indexes for common query patterns
CREATE INDEX idx_trades_ticker_time ON trades(ticker_id, trade_time);
CREATE INDEX idx_klines_ticker_tf_time ON klines(ticker_id, timeframe, candle_time);

-- Partial indexes for filtered queries
CREATE INDEX idx_large_trades ON trades(quantity) WHERE quantity > 1000;
```

**2. Use Prepared Statements**

```rust
// Bad: Re-parsing SQL for each query
for ticker in tickers {
    conn.execute(&format!("SELECT * FROM trades WHERE ticker_id = {}", ticker.id), [])?;
}

// Good: Prepare once, execute many
let mut stmt = conn.prepare("SELECT * FROM trades WHERE ticker_id = ?")?;
for ticker in tickers {
    let trades = stmt.query_map([ticker.id], |row| { /* ... */ })?;
}
```

**3. Leverage Columnar Storage**

```rust
// Only select columns you need
// Bad: SELECT * (reads all columns)
let query = "SELECT * FROM trades WHERE ticker_id = ?";

// Good: Explicit columns (only reads necessary data)
let query = "SELECT trade_time, price, quantity FROM trades WHERE ticker_id = ?";
```

**4. Use Time-Based Partitioning**

```sql
-- Partition trades by month
CREATE TABLE trades (
    ...
) PARTITION BY RANGE (trade_time);

-- DuckDB will only scan relevant partitions
SELECT * FROM trades WHERE trade_time BETWEEN '2024-01-01' AND '2024-01-31';
```

### 7.3 Connection Management

**Single Connection Pattern (Recommended)**

```rust
use std::sync::{Arc, Mutex};

pub struct DatabaseManager {
    conn: Arc<Mutex<Connection>>,
}

impl DatabaseManager {
    pub fn with_conn<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>
    {
        let conn = self.conn.lock().unwrap();
        f(&*conn)
    }
}

// Usage
let db = DatabaseManager::new(path)?;

// All operations reuse the same connection
db.with_conn(|conn| {
    conn.execute("INSERT INTO trades ...", params![])?;
    Ok(())
})?;
```

**Why Single Connection?**
- DuckDB caches data/metadata (lost when connection closes)
- MVCC allows concurrent reads
- Single writer process is optimal
- Connection pooling adds overhead without benefits

**Read-Only Connections (Optional)**

```rust
// For analytics/queries that should not modify data
let ro_conn = Connection::open_with_flags(
    path,
    OpenFlags::SQLITE_OPEN_READ_ONLY
)?;
```

### 7.4 Memory Management

**Configure Memory Limits**

```rust
impl DatabaseManager {
    pub fn new(path: PathBuf) -> Result<Self> {
        let conn = Connection::open(&path)?;

        // Set memory limit (default: 80% of RAM)
        conn.execute("SET memory_limit='8GB'", [])?;

        // Set temp directory for spilling
        let temp_dir = path.parent().unwrap().join("duckdb_temp");
        std::fs::create_dir_all(&temp_dir)?;
        conn.execute(
            &format!("SET temp_directory='{}'", temp_dir.display()),
            []
        )?;

        // Configure temp size limit (default: 90% of remaining disk)
        conn.execute("SET max_temp_directory_size='50GB'", [])?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path,
        })
    }
}
```

**Streaming Query Results**

```rust
// Bad: Materialize all results in memory
let trades: Vec<Trade> = stmt.query_map(params![], |row| { /* ... */ })?
    .collect()?;

// Good: Process results in streaming fashion
let rows = stmt.query_map(params![], |row| { /* ... */ })?;
for trade in rows {
    let trade = trade?;
    process_trade(trade);  // Process one at a time
}
```

### 7.5 Backup and Export Strategies

**1. Parquet Export for Archival**

```rust
impl DatabaseManager {
    /// Export old data to Parquet for long-term storage
    pub fn export_to_parquet(&self, cutoff_date: &str) -> Result<PathBuf> {
        let export_path = self.path.parent().unwrap()
            .join(format!("trades_archive_{}.parquet", cutoff_date));

        self.with_conn(|conn| {
            conn.execute(
                &format!(
                    "COPY (SELECT * FROM trades WHERE trade_time < ?)
                     TO '{}' (FORMAT PARQUET, COMPRESSION ZSTD)",
                    export_path.display()
                ),
                params![cutoff_date]
            )?;
            Ok(export_path)
        })
    }

    /// Query Parquet files directly without importing
    pub fn query_parquet(&self, parquet_path: &Path) -> Result<Vec<Trade>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT trade_time, price, quantity, is_sell
                 FROM read_parquet('{}')",
                parquet_path.display()
            ))?;

            // Process results...
        })
    }
}
```

**2. Incremental Backup**

```rust
impl DatabaseManager {
    /// Create incremental backup
    pub fn create_backup(&self, backup_dir: &Path) -> Result<()> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_file = backup_dir.join(format!("backup_{}.db", timestamp));

        self.with_conn(|conn| {
            conn.execute(
                &format!("EXPORT DATABASE '{}'", backup_file.display()),
                []
            )?;
            Ok(())
        })
    }
}
```

**3. Vacuum and Optimization**

```rust
impl DatabaseManager {
    /// Reclaim space and optimize database
    pub fn vacuum(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute_batch("VACUUM; ANALYZE;")?;
            Ok(())
        })
    }
}
```

### 7.6 Performance Monitoring

```rust
impl DatabaseManager {
    /// Get database statistics
    pub fn get_stats(&self) -> Result<DbStats> {
        self.with_conn(|conn| {
            let trade_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM trades",
                [],
                |row| row.get(0)
            )?;

            let db_size: i64 = conn.query_row(
                "SELECT SUM(total_blocks * block_size) FROM pragma_database_size()",
                [],
                |row| row.get(0)
            )?;

            Ok(DbStats {
                trade_count,
                db_size_bytes: db_size as usize,
            })
        })
    }
}
```

---

## 8. Testing Strategy

### 8.1 Unit Testing Structure

**File:** `data/src/db/crud/trades.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use duckdb::Connection;

    fn setup_test_db() -> Result<DatabaseManager> {
        let conn = Connection::open_in_memory()?;

        // Initialize schema
        conn.execute_batch(include_str!("../schema.sql"))?;

        Ok(DatabaseManager {
            conn: Arc::new(Mutex::new(conn)),
            path: PathBuf::new(),
        })
    }

    #[test]
    fn test_insert_and_query_trades() -> Result<()> {
        let db = setup_test_db()?;

        let ticker_info = create_test_ticker_info();
        let trades = vec![
            Trade {
                time: 1000,
                price: Price::from_f64(50000.0),
                qty: 1.5,
                is_sell: false,
            },
            Trade {
                time: 2000,
                price: Price::from_f64(50100.0),
                qty: 2.0,
                is_sell: true,
            },
        ];

        // Insert
        db.insert_trades(&ticker_info, &trades)?;

        // Query
        let result = db.query_trades(&ticker_info, 0, 3000)?;

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].time, 1000);
        assert_eq!(result[1].time, 2000);

        Ok(())
    }

    #[test]
    fn test_delete_old_trades() -> Result<()> {
        let db = setup_test_db()?;

        let ticker_info = create_test_ticker_info();
        let trades = vec![
            Trade { time: 1000, /* ... */ },
            Trade { time: 5000, /* ... */ },
        ];

        db.insert_trades(&ticker_info, &trades)?;

        // Delete trades older than 3000
        let deleted = db.delete_trades_older_than(3000)?;
        assert_eq!(deleted, 1);

        // Verify only one remains
        let remaining = db.query_trades(&ticker_info, 0, 10000)?;
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].time, 5000);

        Ok(())
    }

    #[test]
    fn test_bulk_insert_performance() -> Result<()> {
        let db = setup_test_db()?;

        let ticker_info = create_test_ticker_info();
        let trades: Vec<Trade> = (0..10000)
            .map(|i| Trade {
                time: i * 1000,
                price: Price::from_f64(50000.0 + i as f64),
                qty: 1.0,
                is_sell: i % 2 == 0,
            })
            .collect();

        let start = std::time::Instant::now();
        db.insert_trades(&ticker_info, &trades)?;
        let duration = start.elapsed();

        println!("Inserted {} trades in {:?}", trades.len(), duration);

        // Should be <1 second for 10k trades
        assert!(duration.as_secs() < 1);

        Ok(())
    }
}
```

### 8.2 Integration Testing Structure

**File:** `tests/db_integration.rs`

```rust
use flowsurface::data::db::DatabaseManager;
use flowsurface::exchange::{Trade, TickerInfo};

fn setup_test_environment() -> (DatabaseManager, tempfile::TempDir) {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.duckdb");
    let db = DatabaseManager::new(db_path).unwrap();
    (db, temp_dir)
}

#[test]
fn test_full_trade_pipeline() {
    let (db, _temp) = setup_test_environment();

    // 1. Insert trades
    let ticker = create_btc_ticker();
    let trades = generate_test_trades(1000);
    db.insert_trades(&ticker, &trades).unwrap();

    // 2. Query back
    let result = db.query_trades(&ticker, 0, u64::MAX).unwrap();
    assert_eq!(result.len(), 1000);

    // 3. Build klines from trades
    let klines = db.build_klines_from_trades(&ticker, Timeframe::M1).unwrap();
    assert!(!klines.is_empty());

    // 4. Insert klines
    db.insert_klines(&ticker, &Timeframe::M1, &klines).unwrap();

    // 5. Query klines
    let loaded = db.query_klines(&ticker, &Timeframe::M1, 0, u64::MAX).unwrap();
    assert_eq!(loaded.len(), klines.len());
}

#[test]
fn test_concurrent_read_access() {
    use std::thread;

    let (db, _temp) = setup_test_environment();

    // Insert data
    let ticker = create_btc_ticker();
    let trades = generate_test_trades(10000);
    db.insert_trades(&ticker, &trades).unwrap();

    // Spawn multiple reader threads
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let db_clone = db.clone();
            let ticker_clone = ticker.clone();

            thread::spawn(move || {
                let result = db_clone.query_trades(
                    &ticker_clone,
                    i * 2500,
                    (i + 1) * 2500
                ).unwrap();

                assert_eq!(result.len(), 2500);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_migration_from_zip() {
    let (db, temp) = setup_test_environment();

    // Create mock ZIP archive
    let zip_path = temp.path().join("BTCUSDT-aggTrades-2024-01-01.zip");
    create_mock_zip_archive(&zip_path, 10000);

    // Migrate
    let stats = ArchiveMigrator::migrate_single_archive(&db, &zip_path).unwrap();

    assert_eq!(stats.trades_migrated, 10000);
    assert_eq!(stats.errors.len(), 0);

    // Verify data
    let ticker = create_btc_ticker();
    let trades = db.query_trades(&ticker, 0, u64::MAX).unwrap();
    assert_eq!(trades.len(), 10000);
}
```

### 8.3 Test Organization

```
flowsurface/
├── data/
│   └── src/
│       └── db/
│           ├── mod.rs
│           ├── crud/
│           │   ├── trades.rs      (unit tests here)
│           │   ├── klines.rs      (unit tests here)
│           │   └── depth.rs       (unit tests here)
│           └── migration.rs       (unit tests here)
├── tests/
│   ├── common/
│   │   └── mod.rs                 (shared test utilities)
│   ├── db_integration.rs          (integration tests)
│   ├── migration_test.rs          (migration tests)
│   └── performance_bench.rs       (benchmarks)
└── benches/
    └── db_benchmarks.rs            (criterion benchmarks)
```

### 8.4 Running Tests

```bash
# Run all tests
cargo test --workspace --all-features

# Run only database tests
cargo test --package flowsurface-data --lib db

# Run integration tests
cargo test --test db_integration

# Run with output
cargo test -- --nocapture

# Run benchmarks
cargo bench --bench db_benchmarks

# Test with memory leak detection (AddressSanitizer)
RUSTFLAGS="-Z sanitizer=address" \
cargo +nightly test --target x86_64-unknown-linux-gnu
```

### 8.5 Benchmark Example

**File:** `benches/db_benchmarks.rs`

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use flowsurface::data::db::DatabaseManager;

fn bench_insert_trades(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_trades");

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                let db = setup_bench_db();
                let trades = generate_test_trades(size);
                let ticker = create_test_ticker();

                b.iter(|| {
                    db.insert_trades(black_box(&ticker), black_box(&trades)).unwrap();
                });
            }
        );
    }

    group.finish();
}

fn bench_query_trades(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_trades");

    let db = setup_bench_db();
    let ticker = create_test_ticker();

    // Pre-populate
    let trades = generate_test_trades(100000);
    db.insert_trades(&ticker, &trades).unwrap();

    for range_size in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(range_size),
            range_size,
            |b, &range_size| {
                b.iter(|| {
                    db.query_trades(
                        black_box(&ticker),
                        black_box(0),
                        black_box(range_size * 1000)
                    ).unwrap();
                });
            }
        );
    }

    group.finish();
}

criterion_group!(benches, bench_insert_trades, bench_query_trades);
criterion_main!(benches);
```

---

## 9. Implementation Timeline

### Week 1-2: Foundation Setup

**Tasks:**
- [ ] Add DuckDB dependency to `data/Cargo.toml`
  ```toml
  duckdb = { version = "1.1", features = ["bundled", "json"] }
  ```
- [ ] Create `data/src/db/` module structure
- [ ] Implement `DatabaseManager` with connection management
- [ ] Write `schema.sql` with all table definitions
- [ ] Implement schema initialization and migrations module
- [ ] Write unit tests for database initialization

**Deliverables:**
- Working `DatabaseManager::new()` that creates database
- Schema properly initialized on first run
- Tests passing for basic connection management

### Week 3-4: CRUD Implementation

**Tasks:**
- [ ] Implement trade CRUD operations (`data/src/db/crud/trades.rs`)
- [ ] Implement kline CRUD operations (`data/src/db/crud/klines.rs`)
- [ ] Implement depth snapshot CRUD (`data/src/db/crud/depth.rs`)
- [ ] Implement footprint CRUD (`data/src/db/crud/footprint.rs`)
- [ ] Add helper methods (ticker ID lookups, etc.)
- [ ] Write comprehensive unit tests for each CRUD module
- [ ] Write integration tests for full data pipeline

**Deliverables:**
- All CRUD operations working and tested
- >80% test coverage on CRUD modules
- Benchmarks showing acceptable performance

### Week 5: Migration Logic

**Tasks:**
- [ ] Implement `TimeSeriesMigrator` for in-memory → DuckDB
- [ ] Implement `DepthMigrator` for historical depth
- [ ] Implement `ArchiveMigrator` for ZIP files
- [ ] Create CLI migration command (`src/cli/migrate.rs`)
- [ ] Implement backup creation before migration
- [ ] Write migration verification logic
- [ ] Test migration with sample data

**Deliverables:**
- Working migration from ZIP archives to DuckDB
- CLI tool: `flowsurface migrate --source archives`
- Backup mechanism in place

### Week 6: Query Layer and Optimization

**Tasks:**
- [ ] Implement optimized bulk insert using Appender
- [ ] Add query methods that return Rust data structures
- [ ] Implement `load_timeseries()` to reconstruct `TimeSeries<T>`
- [ ] Add connection pooling (if needed)
- [ ] Configure memory limits and temp directory
- [ ] Implement Parquet export for archival
- [ ] Add query performance monitoring
- [ ] Run benchmarks and optimize slow queries

**Deliverables:**
- Optimized insert: >10k trades/sec
- Query performance: <100ms for typical range queries
- Memory usage stable under load

### Week 7-8: Application Integration

**Tasks:**
- [ ] Integrate `DatabaseManager` into `Flowsurface` struct
- [ ] Modify `Dashboard::distribute_fetched_data()` for dual-write
- [ ] Update trade fetching to check database first
- [ ] Add environment variable: `FLOWSURFACE_USE_DUCKDB=1`
- [ ] Implement `StateManager` with fallback logic
- [ ] Update cleanup mechanism to use database
- [ ] Add logging for database operations
- [ ] Test dual-write mode end-to-end

**Deliverables:**
- Application works with DuckDB opt-in
- Dual-write successfully saving to both memory and DB
- No regressions in existing functionality

### Week 9: Validation and Testing

**Tasks:**
- [ ] Load testing with realistic data volumes
- [ ] Memory profiling (valgrind/heaptrack)
- [ ] Query performance profiling
- [ ] Test recovery from database corruption
- [ ] Test rollback mechanism
- [ ] Stress test with multiple tickers
- [ ] Document performance characteristics
- [ ] Fix any discovered bugs

**Deliverables:**
- Performance report documenting benchmarks
- No memory leaks detected
- Application stable with large datasets

### Week 10: Documentation and Release

**Tasks:**
- [ ] Write user documentation for DuckDB feature
- [ ] Write migration guide for existing users
- [ ] Update README with database information
- [ ] Create example configurations
- [ ] Write developer documentation
- [ ] Prepare release notes
- [ ] Tag release: v0.9.0 (DuckDB opt-in)
- [ ] Monitor for issues

**Deliverables:**
- Comprehensive documentation
- Release published with opt-in DuckDB support
- Migration guide for users

---

## 10. Rollback and Risk Mitigation

### 10.1 Backup Strategy

**Pre-Migration Backup**

```rust
pub struct BackupManager {
    backup_root: PathBuf,
}

impl BackupManager {
    pub fn create_pre_migration_backup(&self) -> Result<BackupMetadata> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_dir = self.backup_root.join(format!("backup_{}", timestamp));
        std::fs::create_dir_all(&backup_dir)?;

        // 1. Backup JSON state
        let state_src = data_path(Some(SAVED_STATE_PATH));
        let state_dst = backup_dir.join("saved-state.json");
        std::fs::copy(&state_src, &state_dst)?;

        // 2. Backup market data (optional - can be large)
        let market_src = data_path(Some("market_data"));
        let market_dst = backup_dir.join("market_data");
        copy_dir_all(&market_src, &market_dst)?;

        // 3. Create manifest
        let manifest = BackupMetadata {
            timestamp: timestamp.to_string(),
            path: backup_dir.clone(),
            state_file: state_dst,
            market_data: Some(market_dst),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        // 4. Write manifest JSON
        let manifest_json = serde_json::to_string_pretty(&manifest)?;
        std::fs::write(backup_dir.join("manifest.json"), manifest_json)?;

        info!("Backup created at {:?}", backup_dir);
        Ok(manifest)
    }

    pub fn restore_from_backup(&self, backup_dir: &Path) -> Result<()> {
        // Read manifest
        let manifest: BackupMetadata = serde_json::from_str(
            &std::fs::read_to_string(backup_dir.join("manifest.json"))?
        )?;

        // Restore state file
        std::fs::copy(
            &manifest.state_file,
            data_path(Some(SAVED_STATE_PATH))
        )?;

        // Restore market data
        if let Some(market_backup) = manifest.market_data {
            let market_dst = data_path(Some("market_data"));
            std::fs::remove_dir_all(&market_dst).ok();
            copy_dir_all(&market_backup, &market_dst)?;
        }

        info!("Restored backup from {}", manifest.timestamp);
        Ok(())
    }
}
```

### 10.2 Migration Verification

```rust
pub struct MigrationGuard {
    backup: BackupMetadata,
    db_path: PathBuf,
}

impl MigrationGuard {
    pub fn verify_migration(&self) -> Result<HealthCheck> {
        let conn = Connection::open(&self.db_path)?;

        // Check 1: Tables exist
        let tables: Vec<String> = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type='table'"
        )?
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

        let required_tables = vec![
            "trades", "klines", "footprint_data",
            "depth_snapshots", "order_runs", "tickers", "exchanges"
        ];

        for table in &required_tables {
            if !tables.contains(&table.to_string()) {
                return Ok(HealthCheck::Failed(
                    format!("Missing table: {}", table)
                ));
            }
        }

        // Check 2: Row counts reasonable
        let trade_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM trades",
            [],
            |row| row.get(0)
        )?;

        if trade_count == 0 {
            return Ok(HealthCheck::Warning(
                "No trades found after migration".to_string()
            ));
        }

        // Check 3: Data integrity
        let invalid_prices: i64 = conn.query_row(
            "SELECT COUNT(*) FROM trades WHERE price <= 0",
            [],
            |row| row.get(0)
        )?;

        if invalid_prices > 0 {
            return Ok(HealthCheck::Failed(
                format!("Found {} trades with invalid prices", invalid_prices)
            ));
        }

        Ok(HealthCheck::Passed)
    }

    pub fn rollback_if_failed(self) -> Result<()> {
        match self.verify_migration() {
            Ok(HealthCheck::Passed) => {
                info!("✓ Migration verified successfully");
                Ok(())
            }
            Ok(HealthCheck::Warning(msg)) => {
                warn!("⚠ Migration warning: {}", msg);
                Ok(())
            }
            Ok(HealthCheck::Failed(reason)) | Err(reason) => {
                error!("✗ Migration failed: {}. Rolling back...", reason);

                // Delete corrupted database
                std::fs::remove_file(&self.db_path).ok();

                // Restore backup
                let backup_mgr = BackupManager {
                    backup_root: self.backup.path.parent().unwrap().to_path_buf(),
                };
                backup_mgr.restore_from_backup(&self.backup.path)?;

                Err(anyhow!("Migration failed and rolled back: {}", reason))
            }
        }
    }
}

// Usage in migration
let backup = backup_mgr.create_pre_migration_backup()?;
let guard = MigrationGuard::new(backup, db_path.clone());

// Perform migration...
migrate_all_data(&db)?;

// Verify and rollback if needed
guard.rollback_if_failed()?;
```

### 10.3 Feature Flags

```toml
# Cargo.toml
[features]
default = []
duckdb = ["duckdb"]
legacy-storage = []
```

```rust
// Runtime toggle
pub static USE_DUCKDB: AtomicBool = AtomicBool::new(false);

pub fn toggle_duckdb(enabled: bool) {
    USE_DUCKDB.store(enabled, Ordering::Relaxed);
}

// Conditional logic
if USE_DUCKDB.load(Ordering::Relaxed) {
    // Use DuckDB path
    db.insert_trades(&ticker, &trades)?;
} else {
    // Use legacy in-memory path
    self.trades.extend(trades);
}
```

### 10.4 Risk Matrix

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Data loss during migration | Low | **Critical** | Mandatory backups + verification + rollback |
| Database corruption | Low | **High** | Write-ahead logging, checksums, backups |
| Performance regression | Medium | **High** | Benchmarking, indexes, caching layer |
| Breaking API changes | Medium | **High** | Feature flags, dual-write period, versioning |
| Large migration time | Medium | Medium | Background migration, progress reporting |
| Disk space increase | High | Medium | Cleanup old files, compression, Parquet export |
| Memory leaks | Low | Medium | Valgrind testing, proper Connection management |
| DuckDB bugs | Low | Low | Version pinning, fallback to legacy storage |

### 10.5 Monitoring and Alerts

```rust
pub struct DbHealthMonitor {
    db: Arc<DatabaseManager>,
}

impl DbHealthMonitor {
    pub fn check_health(&self) -> HealthReport {
        let mut report = HealthReport::default();

        // Check 1: Database file exists and is readable
        if !self.db.path.exists() {
            report.errors.push("Database file not found".to_string());
            return report;
        }

        // Check 2: Connection works
        match self.db.with_conn(|conn| {
            conn.execute("SELECT 1", [])?;
            Ok(())
        }) {
            Ok(_) => report.connection_ok = true,
            Err(e) => report.errors.push(format!("Connection failed: {}", e)),
        }

        // Check 3: Query performance
        let start = std::time::Instant::now();
        let count = self.db.with_conn(|conn| {
            conn.query_row("SELECT COUNT(*) FROM trades", [], |row| row.get::<_, i64>(0))
        }).ok();
        let query_time = start.elapsed();

        if query_time > Duration::from_secs(5) {
            report.warnings.push(format!(
                "Slow query: COUNT(*) took {:?}",
                query_time
            ));
        }

        // Check 4: Disk space
        let db_size = std::fs::metadata(&self.db.path).unwrap().len();
        if db_size > 50 * 1024 * 1024 * 1024 {  // 50 GB
            report.warnings.push(format!(
                "Database large: {} GB",
                db_size / (1024 * 1024 * 1024)
            ));
        }

        report.trade_count = count;
        report
    }
}
```

### 10.6 Emergency Rollback Procedure

**If DuckDB causes issues in production:**

1. **Immediate Disable:**
   ```bash
   # Set environment variable to disable DuckDB
   export FLOWSURFACE_USE_DUCKDB=0

   # Restart application
   flowsurface
   ```

2. **Restore from Backup:**
   ```bash
   flowsurface restore --backup-dir ~/.local/share/flowsurface/backups/backup_20240115_143022
   ```

3. **Downgrade to Previous Version:**
   ```bash
   # If DuckDB is causing crashes
   cargo install flowsurface --version 0.8.5
   ```

4. **Manual Data Export:**
   ```bash
   # Export critical data to CSV before rollback
   flowsurface export --format csv --output ./export/
   ```

---

## Appendix

### A. Dependencies

```toml
# data/Cargo.toml
[dependencies]
duckdb = { version = "1.1", features = ["bundled", "json"] }
uuid = { workspace = true, features = ["v4", "serde"] }
chrono = { workspace = true }
thiserror = "1.0"
anyhow = "1.0"
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
csv = "1.3"
zip = "0.6"
walkdir = "2.4"

[dev-dependencies]
tempfile = "3.10"
criterion = "0.5"
```

### B. Useful DuckDB Commands

```sql
-- Show all tables
SELECT * FROM information_schema.tables;

-- Show table schema
DESCRIBE trades;

-- Database size
SELECT * FROM pragma_database_size();

-- Query planner
EXPLAIN SELECT * FROM trades WHERE ticker_id = 1;

-- Export to Parquet
COPY trades TO 'trades.parquet' (FORMAT PARQUET, COMPRESSION ZSTD);

-- Import from Parquet
CREATE TABLE trades AS SELECT * FROM read_parquet('trades.parquet');

-- Vacuum and optimize
VACUUM;
ANALYZE;
```

### C. Resources

- **DuckDB Documentation:** https://duckdb.org/docs/
- **duckdb-rs GitHub:** https://github.com/duckdb/duckdb-rs
- **DuckDB Performance Guide:** https://duckdb.org/docs/guides/performance/
- **Rust duckdb Examples:** https://docs.rs/duckdb/latest/duckdb/

---

## Conclusion

This implementation guide provides a comprehensive roadmap for integrating DuckDB into the FlowSurface trading terminal. The hybrid architecture approach maintains real-time performance for hot data while enabling persistent storage and historical analysis capabilities.

**Key Takeaways:**
1. **Phased Implementation** - Gradual rollout with feature flags minimizes risk
2. **Hybrid Architecture** - Hot data in-memory, warm/cold in DuckDB
3. **Performance Optimized** - Columnar storage, bulk inserts, time partitioning
4. **Safe Migration** - Backups, verification, rollback mechanisms
5. **Comprehensive Testing** - Unit, integration, benchmarks, stress tests

**Next Steps:**
1. Review this document with the team
2. Set up development environment
3. Begin Week 1 tasks (Foundation Setup)
4. Iterate based on findings and performance testing

---

**Document Version:** 1.0
**Last Updated:** 2025-10-09
**Author:** Claude (Anthropic)
