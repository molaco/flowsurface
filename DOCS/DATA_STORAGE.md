# FlowSurface Data Storage Architecture

A comprehensive guide to how FlowSurface stores and manages all fetched market data.

---

## Table of Contents

1. [Core Data Structures](#1-core-data-structures)
2. [Real-Time WebSocket Data Storage](#2-real-time-websocket-data-storage)
3. [Historical Data Fetching and Storage](#3-historical-data-fetching-and-storage)
4. [Heatmap Data Storage](#4-heatmap-data-storage)
5. [Footprint/Kline Data Storage](#5-footprintkline-data-storage)
6. [Order Book (Ladder/DOM) Data Storage](#6-order-book-ladderdom-data-storage)
7. [Time & Sales Data Storage](#7-time--sales-data-storage)
8. [Price Type and Fixed-Point Storage](#8-price-type-and-fixed-point-storage)
9. [Ticker and Exchange Metadata Storage](#9-ticker-and-exchange-metadata-storage)
10. [Stream State Management](#10-stream-state-management)
11. [In-Memory vs Persistent Storage](#11-in-memory-vs-persistent-storage)
12. [Data Aggregation and Transformation](#12-data-aggregation-and-transformation)
13. [Performance-Critical Storage Patterns](#13-performance-critical-storage-patterns)
14. [Indicator Data Storage](#14-indicator-data-storage)
15. [Complete Data Flow Example](#15-complete-data-flow-example)

---

## 1. Core Data Structures

### TimeSeries<D> Structure

**Location:** `data/src/aggr/time.rs:28-32`

```rust
pub struct TimeSeries<D: DataPoint> {
    pub datapoints: BTreeMap<u64, D>,  // Timestamp → DataPoint
    pub interval: Timeframe,
    pub tick_size: PriceStep,
}
```

**Purpose:** Generic container for time-based market data aggregation where each datapoint represents a fixed time interval.

**Key Characteristics:**
- Generic over `DataPoint` trait - can store any type implementing DataPoint
- Uses `BTreeMap` for timestamp-based indexing
- Enables efficient range queries for visible data culling
- `interval` stores the timeframe (1m, 5m, 1h, etc.)
- `tick_size` defines price step granularity

**Concrete Types:**
- `TimeSeries<KlineDataPoint>` - Candlestick/OHLC data with footprint analysis
- `TimeSeries<HeatmapDataPoint>` - Heatmap visualization data

### TickAggr Structure

**Location:** `data/src/aggr/ticks.rs:88-92`

```rust
pub struct TickAggr {
    pub datapoints: Vec<TickAccumulation>,
    pub interval: aggr::TickCount,
    pub tick_size: PriceStep,
}
```

**Purpose:** Container for tick/trade-based aggregation where each datapoint represents a fixed number of trades.

**Key Characteristics:**
- Uses `Vec` for sequential indexing (not time-keyed)
- `interval` is number of trades per bucket (e.g., 100 trades)
- `datapoints` stores `TickAccumulation` structures

**Supporting Structure - TickAccumulation** (lines 8-86):
```rust
pub struct TickAccumulation {
    pub tick_count: usize,
    pub kline: Kline,
    pub footprint: KlineTrades,
}
```

### Indexing Strategy Comparison

| Aspect | TimeSeries (BTreeMap) | TickAggr (Vec) |
|--------|----------------------|----------------|
| **Key Type** | `u64` (Unix timestamp) | Sequential indices (0, 1, 2...) |
| **Use Case** | Time-based charts | Tick-based charts |
| **Lookup** | O(log n) by timestamp | O(1) by index |
| **Range Queries** | Efficient via `range()` | Manual iteration |
| **Gaps** | Handled naturally | N/A (sequential) |

### DataPoint Trait

**Location:** `data/src/aggr/time.rs:10-26`

```rust
pub trait DataPoint {
    fn add_trade(&mut self, trade: &Trade, step: PriceStep);
    fn clear_trades(&mut self);
    fn last_trade_time(&self) -> Option<u64>;
    fn first_trade_time(&self) -> Option<u64>;
    fn last_price(&self) -> Price;
    fn kline(&self) -> Option<&Kline>;
    fn value_high(&self) -> Price;
    fn value_low(&self) -> Price;
}
```

**Purpose:** Common interface for all aggregated market data types.

**Implementations:**

#### KlineDataPoint
**Location:** `data/src/chart/kline.rs:10-90`

```rust
pub struct KlineDataPoint {
    pub kline: Kline,
    pub footprint: KlineTrades,
}
```

- Stores OHLC candlestick data + detailed footprint
- Supports Point of Control (POC) calculation
- Used for candlestick and footprint charts

#### HeatmapDataPoint
**Location:** `data/src/chart/heatmap.rs:36-109`

```rust
pub struct HeatmapDataPoint {
    pub grouped_trades: Box<[GroupedTrade]>,
    pub buy_sell: (f32, f32),
}
```

- Stores aggregated trade data for heatmap visualization
- `grouped_trades` is a sorted array (binary search for insertion)
- `buy_sell` is total (buy_volume, sell_volume) tuple

---

## 2. Real-Time WebSocket Data Storage

### Event Types

**Location:** `exchange/src/adapter.rs:566`

```rust
pub enum Event {
    Connected(Exchange),
    Disconnected(Exchange, String),
    DepthReceived(StreamKind, u64, Depth, Box<[Trade]>),
    KlineReceived(StreamKind, Kline),
}
```

### Event::DepthReceived Storage

**Handler:** `src/main.rs:147-168`

When depth + trades arrive, the dashboard routes them to matching panes:

**Storage Destinations:**

| Chart Type | Storage Method | Structure |
|-----------|---------------|-----------|
| **Heatmap** | `chart.insert_datapoint()` | `TimeSeries<HeatmapDataPoint>` + `HistoricalDepth` |
| **Kline** | `chart.insert_trades_buffer()` | `raw_trades: Vec<Trade>` + footprint in klines |
| **Time & Sales** | `panel.insert_buffer()` | `recent_trades: VecDeque<TradeEntry>` |
| **Ladder** | `panel.insert_buffers()` | `raw_trades: VecDeque<Trade>` + current depth |

### Depth Structure

**Location:** `exchange/src/depth.rs:66-79`

```rust
#[derive(Clone, Default)]
pub struct Depth {
    pub bids: BTreeMap<Price, f32>,  // Price → Quantity
    pub asks: BTreeMap<Price, f32>,  // Price → Quantity
}
```

**Why BTreeMap:**
- Maintains sorted price levels
- Best bid = `bids.last_key_value()` (highest bid)
- Best ask = `asks.first_key_value()` (lowest ask)
- Efficient iteration for ladder rendering

**Update Mechanism:**
- `update()`: Differential updates (insert/remove price levels)
- `replace_all()`: Full snapshot replacement
- Rounds prices to minimum tick size

### Event::KlineReceived Storage

**Handler:** `src/main.rs:170-174`

Routes to `dashboard.update_latest_klines()` which:
1. Finds matching Kline chart panes
2. Calls `chart.update_latest_kline(kline)`
3. Updates `TimeSeries.insert_klines(&[*kline])`
4. Updates all indicators with new kline

### Kline Structure

**Location:** `exchange/src/lib.rs:580-609`

```rust
#[derive(Debug, Clone, Copy)]
pub struct Kline {
    pub time: u64,          // Timestamp
    pub open: Price,        // Opening price
    pub high: Price,        // Highest price
    pub low: Price,         // Lowest price
    pub close: Price,       // Closing price
    pub volume: (f32, f32), // (base_volume, quote_volume)
}
```

### Trade Buffering

**Location:** `exchange/src/adapter/binance.rs:345-494`

**Lifecycle:**
1. **Initialization:** `let mut trades_buffer: Vec<Trade> = Vec::new();`
2. **Accumulation:** Each trade message appends to buffer
3. **Emission:** On depth update, send accumulated trades as `Box<[Trade]>`

**Why Buffer:**
- Depth updates are throttled (100-300ms intervals)
- Multiple trades occur between snapshots
- Bundling reduces message overhead
- Ensures temporal consistency

---

## 3. Historical Data Fetching and Storage

### fetch_klines() Implementation

**Location:** `exchange/src/adapter.rs:637-656`

```rust
pub async fn fetch_klines(
    ticker_info: TickerInfo,
    timeframe: Timeframe,
    range: Option<(u64, u64)>,
) -> Result<Vec<Kline>, AdapterError>
```

**Returns:** `Vec<Kline>` with OHLCV + volume data

**Exchange-Specific Implementations:**
- **Binance:** `exchange/src/adapter/binance.rs:921-1019`
- **Bybit:** `exchange/src/adapter/bybit.rs:718+`
- **Hyperliquid:** `exchange/src/adapter/hyperliquid.rs:768+`
- **OKX:** `exchange/src/adapter/okex.rs:728+`

### Kline Storage After Fetch

**Storage Structure:** `TimeSeries<KlineDataPoint>`

**Insertion Method:** `data/src/aggr/time.rs:185-199`

```rust
pub fn insert_klines(&mut self, klines: &[Kline]) {
    for kline in klines {
        let entry = self.datapoints
            .entry(kline.time)
            .or_insert_with(|| KlineDataPoint {
                kline: *kline,
                footprint: KlineTrades::new(),
            });
        entry.kline = *kline;
    }
    self.update_poc_status();
}
```

### fetch_historical_oi() and Storage

**Location:** `exchange/src/adapter.rs:658-675`

```rust
pub async fn fetch_open_interest(
    ticker: Ticker,
    timeframe: Timeframe,
    range: Option<(u64, u64)>,
) -> Result<Vec<OpenInterest>, AdapterError>
```

**Storage Structure:** `src/chart/indicator/kline/open_interest.rs:18-29`

```rust
pub struct OpenInterestIndicator {
    cache: Caches,
    pub data: BTreeMap<u64, f32>,  // timestamp → OI value
}
```

**Supported Exchanges:**
- Binance (Linear & Inverse)
- Bybit (Linear & Inverse)
- OKX (Linear & Inverse)
- **NOT** Hyperliquid or Spot markets

### Historical Trades for Footprint

**KlineTrades Structure:** `data/src/chart/kline.rs:134-230`

```rust
pub struct KlineTrades {
    pub trades: FxHashMap<Price, GroupedTrades>,
    pub poc: Option<PointOfControl>,
}

pub struct GroupedTrades {
    pub buy_qty: f32,
    pub sell_qty: f32,
    pub first_time: u64,
    pub last_time: u64,
    pub buy_count: usize,
    pub sell_count: usize,
}
```

**Storage Location:** Inside each `KlineDataPoint.footprint`

**Raw Trades:** `src/chart/kline.rs:166-169`
```rust
pub struct KlineChart {
    raw_trades: Vec<Trade>,  // All raw trades for rebuilding
}
```

### File-Based Caching

**Note:** There is **NO** file-based caching for chart data (klines, trades, OI). All chart data is **in-memory only**.

**What IS Cached to Disk:**
- Application state only (`saved-state.json`)
- Layout configuration
- Theme settings
- Window specifications
- **NOT** chart data, klines, trades, or indicators

**Market Data Cleanup:** `data/src/lib.rs:176-188`
- Cleans up old Binance aggTrades ZIP files (>4 days old)
- Path: `market_data/binance/data/futures/{um|cm}/daily/aggTrades`

---

## 4. Heatmap Data Storage

### HeatmapDataPoint Structure

**Location:** `data/src/chart/heatmap.rs:36-39`

```rust
pub struct HeatmapDataPoint {
    pub grouped_trades: Box<[GroupedTrade]>,
    pub buy_sell: (f32, f32),
}
```

**Fields:**
- `grouped_trades`: Boxed slice (sorted array) of trades by price level
- `buy_sell`: Tuple of (total_buy_volume, total_sell_volume)

### grouped_trades Storage

**Type:** `Box<[GroupedTrade]>` - NOT a HashMap, but a **sorted array**

**GroupedTrade Structure:**
```rust
pub struct GroupedTrade {
    pub is_sell: bool,
    pub price: Price,
    pub qty: f32,
}
```

**Implementation:** Uses **binary search** for insertion
- Find/insert trades at correct sorted position
- Convert to Vec when inserting, then back to boxed slice

### buy_sell Totals Storage

**Type:** `(f32, f32)` - Simple tuple

**Update Logic:**
```rust
if trade.is_sell {
    self.buy_sell.1 += trade.qty;  // Sell volume
} else {
    self.buy_sell.0 += trade.qty;  // Buy volume
}
```

### BTreeMap Indexing Scheme

**Structure:** `TimeSeries<HeatmapDataPoint>`
- **Key:** `u64` (rounded timestamp in milliseconds)
- **Value:** `HeatmapDataPoint`

**Timestamp Rounding:**
```rust
let aggregate_time: u64 = interval.into();
let rounded_depth_update = (depth_update / aggregate_time) * aggregate_time;
```

**Example:** If interval = 1000ms, timestamp 1523ms → key 1000

### Cleanup Mechanism

**Threshold:** `pub const CLEANUP_THRESHOLD: usize = 4800;`

**Implementation:** `src/chart/heatmap.rs:235-253`

```rust
fn cleanup_old_data(&mut self) {
    if self.trades.datapoints.len() > CLEANUP_THRESHOLD {
        // Remove oldest 10% (480 datapoints)
        let keys_to_remove = self.trades.datapoints.keys()
            .take(CLEANUP_THRESHOLD / 10)
            .copied()
            .collect::<Vec<u64>>();

        for key in keys_to_remove {
            self.trades.datapoints.remove(&key);
        }

        // Cleanup corresponding historical depth
        if let Some(oldest_time) = self.trades.datapoints.keys().next().copied() {
            self.heatmap.cleanup_old_price_levels(oldest_time);
        }
    }
}
```

**Trigger:** When datapoints exceed 4800, remove oldest 480

---

## 5. Footprint/Kline Data Storage

### KlineDataPoint Structure

**Location:** `data/src/chart/kline.rs:10-13`

```rust
pub struct KlineDataPoint {
    pub kline: Kline,
    pub footprint: KlineTrades,
}
```

Combines OHLC candle data with trade-level footprint analysis.

### GroupedTrades Structure

**Location:** `data/src/chart/kline.rs:92-132`

```rust
pub struct GroupedTrades {
    pub buy_qty: f32,
    pub sell_qty: f32,
    pub first_time: u64,
    pub last_time: u64,
    pub buy_count: usize,
    pub sell_count: usize,
}
```

**Methods:**
- `total_qty()`: Returns `buy_qty + sell_qty`
- `delta_qty()`: Returns `buy_qty - sell_qty` (buying pressure)

### FxHashMap for Trade Clustering

**Location:** `data/src/chart/kline.rs:134-138`

```rust
pub struct KlineTrades {
    pub trades: FxHashMap<Price, GroupedTrades>,
    pub poc: Option<PointOfControl>,
}
```

**Why FxHashMap:**
- Fast O(1) lookups for price-level aggregation
- FxHasher is faster than default SipHash for small keys
- Order doesn't matter (only queried by exact price)

### Trade Binning by Price Level

**Location:** `data/src/chart/kline.rs:168-178`

```rust
pub fn add_trade_to_nearest_bin(&mut self, trade: &Trade, step: PriceStep) {
    let price = trade.price.round_to_step(step);

    self.trades
        .entry(price)
        .and_modify(|group| group.add_trade(trade))
        .or_insert_with(|| GroupedTrades::new(trade));
}
```

**Process:**
1. Round trade price to nearest tick step
2. Check if GroupedTrades exists for that price
3. If exists: Add quantities to existing entry
4. If new: Create new GroupedTrades entry

### NPoc (Naked Point of Control) Storage

**Location:** `data/src/chart/kline.rs:461-496`

```rust
#[derive(Debug, Clone, Copy)]
pub struct PointOfControl {
    pub price: Price,
    pub volume: f32,
    pub status: NPoc,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum NPoc {
    #[default]
    None,          // PoC not yet calculated
    Naked,         // PoC not touched by later price action
    Filled {       // PoC was touched by a later candle
        at: u64,   // Timestamp when it was filled
    },
}
```

**Calculation Logic:** `data/src/aggr/time.rs:256-274`

For each candle's PoC:
1. Iterate through all subsequent candles
2. If a later candle's high/low includes the PoC price → Mark as `Filled`
3. If no subsequent candle touches it → Mark as `Naked`

---

## 6. Order Book (Ladder/DOM) Data Storage

### Depth Structure

**Location:** `exchange/src/depth.rs:66-70`

```rust
#[derive(Clone, Default)]
pub struct Depth {
    pub bids: BTreeMap<Price, f32>,
    pub asks: BTreeMap<Price, f32>,
}
```

### Why BTreeMap Instead of HashMap

**Reasons:**

1. **Ordered Traversal:** Maintains sorted price levels
   - Best bid: `self.bids.last_key_value()` (highest)
   - Best ask: `self.asks.first_key_value()` (lowest)

2. **Price Type is Ord-Compatible:** `Price` implements `Ord` and `PartialOrd`
   - Stored as `i64` atomic units (10^-8 precision)
   - Natural ordering for BTreeMap

3. **Efficient Range Queries:** Ladder needs to iterate price levels in order

### Trade Overlay Storage

**Location:** `src/screen/dashboard/panel/ladder.rs:48-61`

```rust
pub struct Ladder {
    depth: Depth,
    raw_trades: VecDeque<Trade>,         // Individual trades
    grouped_trades: KlineTrades,         // Aggregated by price
    ticker_info: TickerInfo,
    config: Config,
    tick_size: PriceStep,
    grouped_asks: BTreeMap<Price, f32>,
    grouped_bids: BTreeMap<Price, f32>,
}
```

**Two Storage Structures:**
1. `raw_trades: VecDeque<Trade>` - Chronological order, FIFO cleanup
2. `grouped_trades: KlineTrades` - Aggregated by price level

**Trade Overlay Process:**
```rust
for trade in trades_buffer {
    self.grouped_trades.add_trade_to_side_bin(trade, tick_size);
    self.raw_trades.push_back(*trade);
}
```

### Trade Retention Mechanism

**Default:** `const TRADE_RETENTION_MS: u64 = 8 * 60_000;` (8 minutes, not 2)

**Location:** `data/src/chart/ladder.rs:4`

**Cleanup:** `src/screen/dashboard/panel/ladder.rs:96-134`

```rust
fn maybe_cleanup_trades(&mut self, now_ms: u64) {
    let retention_ms = self.config.trade_retention.as_millis() as u64;
    let cleanup_step_ms = (retention_ms / 10).max(5_000);
    let keep_from_ms = now_ms.saturating_sub(retention_ms);

    while let Some(trade) = self.raw_trades.front() {
        if trade.time < keep_from_ms {
            self.raw_trades.pop_front();
        } else {
            break;
        }
    }

    // Rebuild grouped_trades from remaining raw_trades
}
```

**Strategy:**
- Time-based retention
- Cleanup every `retention/10` (min 5 seconds)
- Rebuilds aggregated trades after cleanup

### Spread Calculation

**Location:** `src/screen/dashboard/panel/ladder.rs:184-193`

```rust
fn calculate_spread(&self) -> Option<Price> {
    if let (Some((best_ask, _)), Some((best_bid, _))) = (
        self.depth.asks.first_key_value(),
        self.depth.bids.last_key_value(),
    ) {
        Some(*best_ask - *best_bid)
    } else {
        None
    }
}
```

**Storage:** NOT stored - calculated on-demand when needed

---

## 7. Time & Sales Data Storage

### Collection Type for Trade List

**Location:** `src/screen/dashboard/panel/timeandsales.rs:73-74`

```rust
recent_trades: VecDeque<TradeEntry>,
paused_trades_buffer: VecDeque<TradeEntry>,
```

**Two VecDeque Collections:**
- `recent_trades`: Active trades visible when not paused
- `paused_trades_buffer`: Trades accumulated while scrolling

**TradeEntry Structure:** `data/src/chart/timeandsales.rs:42-45`
```rust
pub struct TradeEntry {
    pub ts_ms: u64,
    pub display: TradeDisplay,
}
```

### Trade Retention (Time-Based)

**Default:** `const TRADE_RETENTION_MS: u64 = 120_000;` (2 minutes)

**Configuration:** `data/src/chart/timeandsales.rs:11-17`
```rust
pub struct Config {
    pub trade_size_filter: f32,
    pub trade_retention: Duration,
    pub stacked_bar: Option<StackedBar>,
}
```

**Pruning:** `src/screen/dashboard/panel/timeandsales.rs:181-241`
- Remove trades older than `now - retention - slack`
- Slack = 10% of retention for buffer

**User Configuration:** Configurable 1-60 minutes via UI

### Stacked Bar Metrics Storage

**Structure:** `data/src/chart/timeandsales.rs:94-175`

```rust
pub struct HistAgg {
    buy_count: u64,
    sell_count: u64,
    buy_sum: f64,
    sell_sum: f64,
}
```

**Three Metric Types:**
```rust
pub enum StackedBarRatio {
    Count,        // Number of trades
    Volume,       // Total volume
    AverageSize,  // Average trade size
}
```

**Calculation:**
- **Count:** `buy_count` vs `sell_count`
- **Volume:** `buy_sum` vs `sell_sum`
- **AverageSize:** `(buy_sum/buy_count)` vs `(sell_sum/sell_count)`

### Pause Buffer Implementation

**Buffer:** `paused_trades_buffer: VecDeque<TradeEntry>`

**Pause Trigger:**
```rust
if self.scroll_offset > stacked_bar_h + TRADE_ROW_HEIGHT {
    self.is_paused = true;
}
```

**Routing Logic:**
```rust
let target_trades = if self.is_paused {
    &mut self.paused_trades_buffer
} else {
    &mut self.recent_trades
};
```

**Unpause & Merge:**
```rust
self.is_paused = false;
for trade in self.paused_trades_buffer.iter() {
    self.hist_agg.add(&trade.display);
}
self.recent_trades.extend(self.paused_trades_buffer.drain(..));
```

### Size Filtering

**Threshold:** `pub trade_size_filter: f32` (default: 0.0)

**Filter Application:**
- All trades stored regardless of filter
- Filter affects display and max quantity tracking

**During Rendering:**
```rust
let trades_to_draw = self.recent_trades.iter()
    .filter(|t| trade_size >= self.config.trade_size_filter)
    .rev()
    .skip(start_index)
    .take(visible_rows + 2);
```

**User Configuration:** 0 to 50,000 with 500 step size

---

## 8. Price Type and Fixed-Point Storage

### Price Struct with i64 Units

**Location:** `exchange/src/util.rs:94-98`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Deserialize, Serialize)]
pub struct Price {
    /// number of atomic units (atomic unit = 10^-PRICE_SCALE)
    pub units: i64,
}
```

**Key Characteristics:**
- Uses `i64` to store price in atomic units
- Implements `Copy`, `Eq`, `Ord` - enables HashMap keys
- Fully serializable

### PRICE_SCALE Constant

**Location:** `exchange/src/util.rs:101-102`

```rust
pub const PRICE_SCALE: i32 = 8;
```

**Precision:** 10^-8 = 0.00000001

**Examples:**
- Price 1.0 → stored as 100,000,000 units
- Price 0.5 → stored as 50,000,000 units
- Price 0.00000001 → stored as 1 unit

### Price Rounding Functions

**round_to_step** (lines 179-187):
```rust
pub fn round_to_step(self, step: PriceStep) -> Self {
    let unit = step.units;
    if unit <= 1 { return self; }
    let half = unit / 2;
    let rounded = ((self.units + half).div_euclid(unit)) * unit;
    Self { units: rounded }
}
```
Rounds to nearest multiple of step.

**floor_to_step** (lines 189-197):
```rust
fn floor_to_step(self, step: PriceStep) -> Self {
    let unit = step.units;
    if unit <= 1 { return self; }
    let floored = (self.units.div_euclid(unit)) * unit;
    Self { units: floored }
}
```
Rounds DOWN (used for sell orders).

**ceil_to_step** (lines 199-215):
```rust
fn ceil_to_step(self, step: PriceStep) -> Self {
    let unit = step.units;
    if unit <= 1 { return self; }
    let added = self.units.checked_add(unit - 1).unwrap_or(...);
    let ceiled = (added.div_euclid(unit)) * unit;
    Self { units: ceiled }
}
```
Rounds UP (used for buy orders).

**round_to_side_step** (lines 217-224):
```rust
pub fn round_to_side_step(self, is_sell_or_bid: bool, step: PriceStep) -> Self {
    if is_sell_or_bid {
        self.floor_to_step(step)
    } else {
        self.ceil_to_step(step)
    }
}
```
Side-aware rounding for order book aggregation.

### PriceStep Type

**Location:** `exchange/src/util.rs:65-69`

```rust
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct PriceStep {
    pub units: i64,  // step size in atomic units
}
```

**Conversion:**
- `from_f32_lossy()`: Converts f32 step to atomic units
- `to_f32_lossy()`: Converts back to f32 for UI

### Why Fixed-Point Instead of f64

**Reasons:**

1. **Exact Decimal Arithmetic:** Avoids floating-point errors
2. **Hash and Equality:** Can implement `Eq` and `Hash` (impossible with f64 due to NaN)
3. **HashMap Keys:** Prices can be used as map keys (critical for trade aggregation)
4. **Deterministic Rounding:** Integer division ensures consistent results
5. **Binary Representation:** No issues like `0.1 + 0.2 ≠ 0.3`
6. **Performance:** Integer operations are fast and predictable
7. **Range:** i64 with 8 decimals handles all crypto prices

**Example Use Case:**
```rust
pub trades: FxHashMap<Price, GroupedTrades>  // Impossible with f64
```

---

## 9. Ticker and Exchange Metadata Storage

### Ticker Struct - Stack Allocation

**Location:** `exchange/src/lib.rs:288-296`

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ticker {
    bytes: [u8; 28],           // Stack-allocated symbol
    pub exchange: Exchange,
    display_bytes: [u8; 28],   // Optional display symbol
    has_display_symbol: bool,
}
```

**Key Details:**
- `bytes: [u8; 28]` - First 28-byte array for ticker symbol
- `display_bytes: [u8; 28]` - Second array for display symbol
- Stack allocation avoids heap overhead
- Total size: 58 bytes (28 + 1 + 28 + 1)

### TickerInfo Structure

**Location:** `exchange/src/lib.rs:533-540`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize, Hash, Eq)]
pub struct TickerInfo {
    pub ticker: Ticker,
    pub min_ticksize: MinTicksize,
    pub min_qty: MinQtySize,
    pub contract_size: Option<ContractSize>,
}
```

**Type Definitions:**
```rust
pub type ContractSize = Power10<-4, 6>;   // 10^-4 to 10^6
pub type MinTicksize = Power10<-8, 2>;    // 10^-8 to 10^2
pub type MinQtySize = Power10<-6, 8>;     // 10^-6 to 10^8
```

**Power10 Efficiency:** `exchange/src/util.rs:7-24`
```rust
pub struct Power10<const MIN: i8, const MAX: i8> {
    pub power: i8,  // Only stores exponent, not full float
}
```

### Tickers Table Storage

**Location:** `data/src/tickers_table.rs:7-64`

```rust
pub struct Settings {
    pub favorited_tickers: Vec<Ticker>,
    pub show_favorites: bool,
    pub selected_sort_option: SortOptions,
    pub selected_exchanges: Vec<ExchangeInclusive>,
    pub selected_markets: Vec<MarketKind>,
}

pub struct TickerRowData {
    pub exchange: Exchange,
    pub ticker: Ticker,
    pub stats: TickerStats,
    pub previous_stats: Option<TickerStats>,
    pub is_favorited: bool,
}
```

**Main Storage:** `src/screen/dashboard/tickers_table.rs:81-95`

```rust
pub struct TickersTable {
    ticker_rows: Vec<TickerRowData>,
    pub favorited_tickers: FxHashSet<Ticker>,
    display_cache: FxHashMap<Ticker, TickerDisplayData>,
    pub tickers_info: FxHashMap<Ticker, Option<TickerInfo>>,
    // ...
}
```

### Ticker Stats Fetching and Caching

**Fetch Functions:** `exchange/src/adapter.rs:507-530`

**Fetch Ticker Info (metadata):**
```rust
pub async fn fetch_ticker_info(
    exchange: Exchange,
) -> Result<HashMap<Ticker, Option<TickerInfo>>, AdapterError>
```
- Fetches tickSize, minQty, contractSize
- Called once at startup
- Stored in `tickers_info: FxHashMap<Ticker, Option<TickerInfo>>`

**Fetch Ticker Prices (live stats):**
```rust
pub async fn fetch_ticker_prices(
    exchange: Exchange,
) -> Result<HashMap<Ticker, TickerStats>, AdapterError>
```
- Fetches mark_price, daily_price_chg, daily_volume
- Called periodically

**Caching Strategy:** `src/screen/dashboard/tickers_table.rs:133-170`
1. Clear old cache entries for the exchange
2. Insert computed display data into cache
3. Clear entire cache when table is toggled

### Update Intervals

**Constants:** `src/screen/dashboard/tickers_table.rs:29-30`

```rust
const ACTIVE_UPDATE_INTERVAL: u64 = 13;    // 13 seconds when visible
const INACTIVE_UPDATE_INTERVAL: u64 = 300; // 300 seconds when hidden
```

**Subscription:**
```rust
pub fn subscription(&self) -> Subscription<Message> {
    iced::time::every(std::time::Duration::from_secs(
        if self.is_shown { 13 } else { 300 }
    ))
    .map(|_| Message::FetchForTickerStats(None))
}
```

**How it Works:**
- Visible: Fetches every 13 seconds
- Hidden: Fetches every 300 seconds (5 minutes)
- Reduces API load when table not visible

---

## 10. Stream State Management

### UniqueStreams Deduplication System

**Location:** `exchange/src/adapter.rs:212-300`

```rust
pub struct UniqueStreams {
    streams: EnumMap<Exchange, Option<FxHashMap<TickerInfo, FxHashSet<StreamKind>>>>,
    specs: EnumMap<Exchange, Option<StreamSpecs>>,
}
```

**Hierarchical Organization:**
- First level: Organized by Exchange (Binance, Bybit, etc.)
- Second level: Grouped by TickerInfo (trading pair)
- Third level: Deduplicated StreamKind variants (using FxHashSet)

**Deduplication Methods:**
- `add()`: Inserts stream, automatic dedup via HashSet
- `extend()`: Batch addition
- `from()`: Creates from iterator of streams

### StreamKind to Data Storage Mapping

**Location:** `exchange/src/adapter.rs:168-210`

```rust
pub enum StreamKind {
    Kline {
        ticker_info: TickerInfo,
        timeframe: Timeframe,
    },
    DepthAndTrades {
        ticker_info: TickerInfo,
        depth_aggr: StreamTicksize,
        push_freq: PushFrequency,
    },
}
```

**Data Distribution Flow:**

**Entry Point:** `src/main.rs:136-175`

1. **DepthReceived Events:**
   - Calls `dashboard.update_depth_and_trades()`
   - Distributes to Heatmap, Kline, TimeAndSales, Ladder

2. **KlineReceived Events:**
   - Calls `dashboard.update_latest_klines()`
   - Updates candle data in matching panes

### Pane State and Data Storage

**Location:** `src/screen/dashboard/pane.rs:106-115`

```rust
pub struct State {
    id: uuid::Uuid,
    pub content: Content,
    pub streams: ResolvedStream,  // Stream association
    pub status: Status,
    // ...
}
```

**Content Variants:** (lines 1078-1096)
```rust
pub enum Content {
    Heatmap { chart: Option<HeatmapChart>, ... },
    Kline { chart: Option<KlineChart>, ... },
    TimeAndSales(Option<TimeAndSales>),
    Ladder(Option<Ladder>),
}
```

Each content type owns its data storage.

### Multiple Panes Sharing Streams

**Dashboard Stream Collection:** `src/screen/dashboard.rs:58`

```rust
pub struct Dashboard {
    pub panes: pane_grid::State<pane::State>,
    pub streams: UniqueStreams,  // Central deduplication
    // ...
}
```

**Stream Refresh:** (lines 1467-1474)
```rust
fn refresh_streams(&mut self, main_window: window::Id) -> Task<Message> {
    let all_pane_streams = self.iter_all_panes(main_window)
        .flat_map(|(_, _, pane_state)| pane_state.streams.ready_iter());
    self.streams = UniqueStreams::from(all_pane_streams);
    Task::none()
}
```

**Process:**
1. Collects all streams from all panes
2. Rebuilds UniqueStreams (automatic deduplication)
3. Creates subscriptions from deduplicated streams

**Data Broadcasting:** (lines 1331-1361)
```rust
self.iter_all_panes_mut(main_window).for_each(|(_, _, pane_state)| {
    if pane_state.matches_stream(stream) {
        // Insert data into pane's content
    }
});
```

### Cleanup Mechanism for Unused Streams

**Trigger Points:**
1. Pane replaced
2. Stream configuration changed
3. No matching panes found for incoming data
4. Layout loaded
5. Streams resolved

**Cleanup Process:**
1. Collect streams from existing panes only
2. Rebuild UniqueStreams from current pane states
3. Old streams automatically excluded
4. Iced's subscription system closes unused WebSocket connections

**Automatic Garbage Collection:**
- Rust's ownership: old `self.streams` is dropped
- Subscriptions managed by Iced's subscription system
- No manual cleanup needed

---

## 11. In-Memory vs Persistent Storage

### Persisted State - saved-state.json

**Path:** `~/.local/share/flowsurface/saved-state.json`

**Structure:** `data/src/config/state.rs:15-28`

```rust
pub struct State {
    pub layout_manager: Layouts,
    pub selected_theme: Theme,
    pub custom_theme: Option<Theme>,
    pub main_window: Option<WindowSpec>,
    pub timezone: UserTimezone,
    pub sidebar: Sidebar,
    pub scale_factor: ScaleFactor,
    pub audio_cfg: AudioStream,
    pub trade_fetch_enabled: bool,
    pub size_in_quote_currency: bool,
}
```

**Persistence Mechanism:**
- Write: `data/src/lib.rs:34-39`
- Read: `data/src/lib.rs:41-87`
- Format: JSON via serde_json
- Corrupted files backed up as `saved-state_old.json`

### Layout Persistence

**Structure:** `data/src/layout/dashboard.rs:1-13`

```rust
pub struct Dashboard {
    pub pane: Pane,                       // Recursive pane tree
    pub popout: Vec<(Pane, WindowSpec)>,  // Pop-out windows
}
```

**What's Saved:**
- Chart type (Heatmap, Kline, TimeAndSales, Ladder)
- View configuration (zoom, pan, autoscale)
- Stream types (exchange/ticker/timeframe)
- Visual settings
- Studies/indicators enabled
- Link groups

**NOT Saved:**
- Actual chart data
- WebSocket connections
- Canvas caches

### Theme and Audio Persistence

**Theme:** `data/src/config/theme.rs:1-49`
- `selected_theme`: Active theme name
- `custom_theme`: Full color palette if custom

**Audio:** `data/src/audio.rs:231-238`
```rust
pub struct AudioStream {
    pub streams: FxHashMap<SerTicker, StreamCfg>,
    pub volume: Option<f32>,
}
```

### Memory-Only Chart Data

**All chart data is ephemeral:**

| Chart Type | Storage | Cleanup |
|-----------|---------|---------|
| **Heatmap** | `TimeSeries<HeatmapDataPoint>` | 4800 datapoints max |
| **Kline** | `TimeSeries<KlineDataPoint>` | Unbounded until pane closed |
| **Ladder** | `VecDeque<Trade>` | 8 minutes retention |
| **Time & Sales** | `VecDeque<TradeEntry>` | 2 minutes retention |

### Historical Data File Caching

**Location:** `data/src/lib.rs:123-188`

```rust
pub fn cleanup_old_market_data() -> usize {
    // Deletes Binance aggTrades ZIP files > 4 days old
}
```

**Details:**
- Path: `market_data/binance/data/futures/{um|cm}/daily/aggTrades/`
- Pattern: `*-YYYY-MM-DD.zip`
- Retention: 4 days
- Trigger: Background thread on startup

### Summary Table

| Data Type | Persistence | Location | Retention |
|-----------|------------|----------|-----------|
| **Layouts** | Persisted | `saved-state.json` | Permanent |
| **Theme** | Persisted | `saved-state.json` | Permanent |
| **Audio Config** | Persisted | `saved-state.json` | Permanent |
| **Window Specs** | Persisted | `saved-state.json` | Permanent |
| **Heatmap Data** | Memory | `TimeSeries<HeatmapDataPoint>` | 4800 datapoints |
| **Kline Data** | Memory | `TimeSeries<KlineDataPoint>` | Until pane closed |
| **Ladder Trades** | Memory | `VecDeque<Trade>` | 8 minutes |
| **Time & Sales** | Memory | `VecDeque<TradeEntry>` | 2 minutes |
| **Historical Files** | File cache | `.zip` files | 4 days |

---

## 12. Data Aggregation and Transformation

### Time-Based Aggregation

**Implementation:** `data/src/aggr/time.rs:201-238`

**Process:**
```rust
pub fn insert_trades(&mut self, buffer: &[Trade]) {
    let aggr_time = self.interval.to_milliseconds();

    buffer.iter().for_each(|trade| {
        // Round timestamp to interval boundary
        let rounded_time = (trade.time / aggr_time) * aggr_time;

        let entry = self.datapoints.entry(rounded_time)
            .or_insert_with(|| KlineDataPoint { ... });

        entry.add_trade(trade, self.tick_size);
    });
}
```

**Supported Intervals:**
- Heatmap: 100ms, 200ms, 300ms, 500ms, 1s
- Kline: 1m, 3m, 5m, 15m, 30m, 1h, 2h, 4h, 6h, 12h, 1d

**Rounding:** `(trade.time / aggr_time) * aggr_time` floors to interval start

### Tick-Based Aggregation

**Implementation:** `data/src/aggr/ticks.rs:130-161`

**TickCount:** `data/src/aggr.rs:6-23`
```rust
pub struct TickCount(pub u16);

impl TickCount {
    pub const ALL: [TickCount; 7] = [
        TickCount(10), TickCount(20), TickCount(50),
        TickCount(100), TickCount(200), TickCount(500),
        TickCount(1000),
    ];
}
```

**Process:**
```rust
pub fn insert_trades(&mut self, buffer: &[Trade]) {
    for trade in buffer {
        if self.datapoints.is_empty() {
            self.datapoints.push(TickAccumulation::new(trade));
        } else {
            let last_idx = self.datapoints.len() - 1;

            if self.datapoints[last_idx].is_full(self.interval) {
                // Start new accumulation
                self.datapoints.push(TickAccumulation::new(trade));
            } else {
                // Add to current
                self.datapoints[last_idx].update_with_trade(trade);
            }
        }
    }
}
```

### Trade Clustering by Price

**Implementation:** `data/src/chart/kline.rs:140-178`

**Two Rounding Methods:**

1. **Side-Based:**
```rust
pub fn add_trade_to_side_bin(&mut self, trade: &Trade, step: PriceStep) {
    let price = trade.price.round_to_side_step(trade.is_sell, step);
    // Floor for sells, ceil for buys
}
```

2. **Nearest:**
```rust
pub fn add_trade_to_nearest_bin(&mut self, trade: &Trade, step: PriceStep) {
    let price = trade.price.round_to_step(step);
    // Side-agnostic rounding
}
```

**Storage:** `FxHashMap<Price, GroupedTrades>`

### Coalescing Strategies for Heatmap

**Location:** `data/src/chart/heatmap.rs:482-503`

```rust
pub enum CoalesceKind {
    First(f32),    // Use first order quantity
    Average(f32),  // Use average quantity
    Max(f32),      // Use max quantity
}
```

**Default:** `CoalesceKind::Average(0.15)` (15% threshold)

**Order Run:** (lines 111-140)
```rust
pub struct OrderRun {
    pub start_time: u64,
    pub until_time: u64,
    qty: f32,
    pub is_bid: bool,
}
```

**Coalescing Logic:** (lines 272-350)
- Merge runs if: overlapping time, same side, within threshold
- Threshold: percentage difference in quantity
- Reduces visual noise

### Volume Profile Calculation

**Location:** `src/chart/heatmap.rs:918-1018`

**ProfileKind:**
```rust
pub enum ProfileKind {
    FixedWindow(usize),  // Last N datapoints
    VisibleRange,        // Visible time range
}
```

**Algorithm:**
1. Determine time range (visible or fixed lookback)
2. Determine price range and create bins
3. Initialize profile array: `vec![(0.0f32, 0.0f32); num_ticks]`
4. Accumulate volume at each price level
5. Track max volume for scaling
6. Render bars

**Storage:** Computed on-demand, not persisted

**Point of Control (POC):** `data/src/chart/kline.rs:193-214`
```rust
pub fn calculate_poc(&mut self) {
    let mut max_volume = 0.0;
    let mut poc_price = Price::from_f32(0.0);

    for (price, group) in &self.trades {
        let total_volume = group.buy_qty + group.sell_qty;
        if total_volume > max_volume {
            max_volume = total_volume;
            poc_price = *price;
        }
    }
}
```

---

## 13. Performance-Critical Storage Patterns

### FxHashMap vs BTreeMap Usage

**FxHashMap - Fast, Unordered Lookups:**

| Use Case | Location | Reason |
|----------|----------|--------|
| Price-level aggregation | `data/src/chart/kline.rs:136` | O(1) insertion/lookup for trades |
| Heatmap grid | `data/src/chart/heatmap.rs:417` | Pre-allocated for grid construction |
| Stream management | `exchange/src/adapter.rs:214` | Fast ticker lookups |
| Display cache | `src/screen/dashboard/tickers_table.rs:84` | UI cache needs speed |

**BTreeMap - Ordered, Range-Queryable:**

| Use Case | Location | Reason |
|----------|----------|--------|
| Time-series data | `data/src/aggr/time.rs:29` | Range queries for visible window |
| Order book depth | `exchange/src/depth.rs:68` | Price-sorted for best bid/ask |
| Historical depth | `data/src/chart/heatmap.rs:144` | Range queries by price |
| Indicator data | `src/chart/indicator/kline/volume.rs:21` | Time-based iteration |

### Buffer Pre-allocation Strategies

**with_capacity_and_hasher:**
```rust
let capacity = time_interval_offsets.len() * price_tick_offsets.len();
let mut grid_quantities: FxHashMap<(u64, Price), (f32, bool)> =
    FxHashMap::with_capacity_and_hasher(capacity, FxBuildHasher);
```
**Location:** `data/src/chart/heatmap.rs:416-418`
**Benefit:** Single allocation, no rehashing

**with_capacity (Vec):**
```rust
let mut missing_keys = Vec::with_capacity(((latest - earliest) / interval) as usize);
```
**Location:** `data/src/aggr/time.rs:142`
**Benefit:** Pre-calculate exact capacity

### VecDeque Usage for Circular Buffers

**Time and Sales:** `src/screen/dashboard/panel/timeandsales.rs:73-74`
```rust
recent_trades: VecDeque<TradeEntry>,
paused_trades_buffer: VecDeque<TradeEntry>,
```

**Ladder Panel:** `src/screen/dashboard/panel/ladder.rs:50`
```rust
raw_trades: VecDeque<Trade>
```

**Pattern:**
- `push_back()` - Add new trades
- `pop_front()` - Remove old trades
- O(1) operations at both ends

### Visible Range Culling

**Time-Series Range Queries:** `data/src/aggr/time.rs`

```rust
// Line 93: Min/max price in visible range
let mut it = self.datapoints.range(earliest..=latest);

// Line 259: PoC status update
for (&next_time, next_dp) in self.datapoints.range((current_time + 1)..) {

// Line 345: Max cluster quantity
.range(earliest..=latest)
```

**Heatmap Range Queries:** `data/src/chart/heatmap.rs`
```rust
self.price_levels.range(lowest..=highest)
```

**Performance:** O(log n + k) where k = visible points

### Copy Trait for Zero-Cost Abstractions

**Price and Trading Types:**
- `Price` - Fixed-point price (64-bit integer)
- `Trade` - Individual trade (24 bytes)
- `Kline` - OHLCV candle data
- `TickerInfo` - Market metadata

**Configuration Types:**
- `Config` - Chart settings
- `CoalesceKind` - Aggregation strategy
- `NPoc` - PoC state

**Benefits:**
- Stack allocation
- No pointer indirection
- No heap allocations
- No reference counting

**Example:**
```rust
self.raw_trades.push_back(*trade)  // Copies 24 bytes
```

---

## 14. Indicator Data Storage

### Volume Indicator Storage

**Location:** `src/chart/indicator/kline/volume.rs:20-31`

```rust
pub struct VolumeIndicator {
    cache: Caches,
    data: BTreeMap<u64, (f32, f32)>,  // timestamp → (buy, sell)
}
```

**Rebuild from Source:** (lines 87-97)
```rust
fn rebuild_from_source(&mut self, source: &PlotData<KlineDataPoint>) {
    match source {
        PlotData::TimeBased(timeseries) => {
            self.data = timeseries.volume_data();
        }
        PlotData::TickBased(tickseries) => {
            self.data = tickseries.volume_data();
        }
    }
    self.clear_all_caches();
}
```

**on_insert_klines Hook:** (lines 99-105)
```rust
fn on_insert_klines(&mut self, klines: &[Kline]) {
    for kline in klines {
        self.data.insert(kline.time, (kline.volume.0, kline.volume.1));
    }
    self.clear_all_caches();
}
```

### Open Interest Indicator Storage

**Location:** `src/chart/indicator/kline/open_interest.rs:18-29`

```rust
pub struct OpenInterestIndicator {
    cache: Caches,
    pub data: BTreeMap<u64, f32>,  // timestamp → OI value
}
```

**on_open_interest Hook:** (lines 169-172)
```rust
fn on_open_interest(&mut self, data: &[exchange::OpenInterest]) {
    self.data.extend(data.iter().map(|oi| (oi.time, oi.value)));
    self.clear_all_caches();
}
```

**Note:** OI data comes from external API, NOT from klines

### Volume Profile Storage

**Location:** `data/src/chart/heatmap.rs:620-623`

```rust
pub enum ProfileKind {
    FixedWindow(usize),    // Fixed lookback
    VisibleRange,          // Dynamic
}
```

**Calculation:** `src/chart/heatmap.rs:918-1035`

```rust
let mut profile = vec![(0.0f32, 0.0f32); num_ticks];
```

**Difference:**
- **VisibleRange:** Recalculates on every pan/zoom
- **FixedWindow:** Anchored to latest candle

### Indicator Trait and Hooks

**Base Trait:** `src/chart/indicator/kline.rs:12-50`

```rust
pub trait KlineIndicatorImpl {
    fn rebuild_from_source(&mut self, _source: &PlotData<KlineDataPoint>) {}
    fn on_insert_klines(&mut self, _klines: &[Kline]) {}
    fn on_insert_trades(&mut self, _trades: &[Trade], ...) {}
    fn on_ticksize_change(&mut self, _source: &PlotData<KlineDataPoint>) {}
    fn on_basis_change(&mut self, _source: &PlotData<KlineDataPoint>) {}
    fn on_open_interest(&mut self, _pairs: &[exchange::OpenInterest]) {}
}
```

**Hook Invocation:** `src/chart/kline.rs`
- on_insert_klines: Lines 331-350, 735-754
- on_insert_trades: Lines 688-716

---

## 15. Complete Data Flow Example

### Example: Binance Perpetual BTC/USDT 1-minute Kline with Footprint

#### Step 1: fetch_klines()

**Location:** `exchange/src/adapter/binance.rs:921-1019`

```rust
pub async fn fetch_klines(
    ticker_info: TickerInfo,
    timeframe: Timeframe,
    range: Option<(u64, u64)>,
) -> Result<Vec<Kline>, AdapterError>
```

**Returns:** `Vec<Kline>`

**API:** `https://fapi.binance.com/fapi/v1/klines?symbol=BTCUSDT&interval=1m`

**Called From:** `src/screen/dashboard.rs:1634`

#### Step 2: WebSocket Stream Setup

**Kline Stream:**
- URL: `wss://fstream.binance.com/stream?streams=btcusdt@kline_1m`
- Connection: `exchange/src/adapter/binance.rs:604-729`

**Trade Stream:**
- URL: `wss://fstream.binance.com/stream?streams=btcusdt@aggTrade/btcusdt@depth@100ms`
- Connection: `exchange/src/adapter/binance.rs:332-602`

**Subscription:** `src/screen/dashboard.rs:1421-1464`

#### Step 3: Kline Storage

**Structure:** `data/src/aggr/time.rs:28-32`

```rust
pub struct TimeSeries<D: DataPoint> {
    pub datapoints: BTreeMap<u64, D>,
    pub interval: Timeframe,
    pub tick_size: PriceStep,
}
```

**KlineDataPoint:** `data/src/chart/kline.rs:10-13`
```rust
pub struct KlineDataPoint {
    pub kline: Kline,
    pub footprint: KlineTrades,
}
```

**Insert:** `data/src/aggr/time.rs:185-199`

#### Step 4: Trade Storage for Footprint

**Structure:** `data/src/chart/kline.rs:134-230`

```rust
pub struct KlineTrades {
    pub trades: FxHashMap<Price, GroupedTrades>,
    pub poc: Option<PointOfControl>,
}

pub struct GroupedTrades {
    pub buy_qty: f32,
    pub sell_qty: f32,
    pub first_time: u64,
    pub last_time: u64,
    pub buy_count: usize,
    pub sell_count: usize,
}
```

**Insert Trades:** `data/src/aggr/time.rs:201-238`

```rust
let rounded_time = (trade.time / aggr_time) * aggr_time;
let entry = self.datapoints.entry(rounded_time).or_insert_with(...);
entry.add_trade(trade, self.tick_size);
```

#### Step 5: UI Access

**Event Flow:**

1. **Receive:** `src/main.rs:136-175` - `Message::MarketWsEvent(event)`

2. **Route:**
   - KlineReceived: `dashboard.update_latest_klines()` (line 170)
   - DepthReceived: `dashboard.update_depth_and_trades()` (line 147)

3. **Update Kline:** `src/screen/dashboard.rs:1292-1319`
```rust
if let pane::Content::Kline { chart, .. } = &mut pane_state.content {
    chart.update_latest_kline(kline);
}
```

4. **Update Trades:** `src/screen/dashboard.rs:1321-1369`
```rust
match &mut pane_state.content {
    pane::Content::Kline { chart, .. } => {
        chart.insert_trades_buffer(trades_buffer);
    }
}
```

5. **Chart Updates:** `src/chart/kline.rs:331-351, 688-716`

6. **Rendering:** Chart renders from `self.data_source.datapoints`

### Data Flow Diagram

```
1. FETCH INITIAL KLINES
   ├─ User opens chart → kline_fetch_task()
   ├─ Calls adapter::fetch_klines()
   ├─ Returns Vec<Kline>
   └─ Stored in TimeSeries<KlineDataPoint>.datapoints

2. WEBSOCKET STREAMS
   ├─ Kline: wss://fstream.binance.com/stream?streams=btcusdt@kline_1m
   │  ├─ Emits Event::KlineReceived
   │  └─ Updates kline in BTreeMap
   │
   └─ Trade: wss://fstream.binance.com/stream?streams=btcusdt@aggTrade
      ├─ Emits Event::DepthReceived with trades_buffer
      └─ Aggregated into footprint

3. STORAGE STRUCTURE
   TimeSeries<KlineDataPoint>.datapoints: BTreeMap<u64, KlineDataPoint>
   └─ Key: timestamp (60000ms for 1m)
      └─ Value: KlineDataPoint
         ├─ kline: Kline { time, OHLCV, volume }
         └─ footprint: KlineTrades
            ├─ trades: FxHashMap<Price, GroupedTrades>
            │  └─ GroupedTrades { buy_qty, sell_qty, counts, times }
            └─ poc: PointOfControl { price, volume, status }

4. UI ACCESS
   ├─ Event in main.rs (MarketWsEvent)
   ├─ Dashboard routes to update methods
   ├─ Chart updates TimeSeries.datapoints
   └─ Chart renders from data_source
```

---

## Summary

FlowSurface implements a sophisticated multi-layered data storage architecture:

1. **Core Structures:** TimeSeries (BTreeMap) for time-based, TickAggr (Vec) for tick-based
2. **Real-Time Data:** WebSocket events → Dashboard routing → Pane-specific storage
3. **Historical Data:** API fetches → In-memory storage, no file persistence
4. **Aggregation:** Time-based, tick-based, price clustering, order book coalescing
5. **Performance:** FxHashMap for speed, BTreeMap for range queries, VecDeque for FIFO, Copy trait for zero-cost
6. **Indicators:** Trait-based with hooks for incremental updates
7. **Persistence:** Only UI state persisted, all market data is ephemeral

This design enables efficient real-time updates, flexible visualization, and minimal memory overhead through aggressive cleanup mechanisms and smart data structure choices.
