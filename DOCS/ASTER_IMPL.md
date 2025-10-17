# Aster DEX Provider Implementation Plan

## Table of Contents

1. [Understanding the Aster DEX API](#1-understanding-the-aster-dex-api)
2. [Flowsurface Provider Architecture](#2-flowsurface-provider-architecture)
3. [Implementation Plan](#3-implementation-plan)
4. [File Modifications](#4-file-modifications)
5. [Function Implementations](#5-function-implementations)

---

## 1. Understanding the Aster DEX API

### 1.1 API Architecture

**Base Endpoints:**
- REST API: `https://fapi.asterdex.com`
- WebSocket: `wss://fstream.asterdex.com`

**Supported Markets:**
- ✅ Spot markets (`/api/v1/*` endpoints)
- ✅ Linear perpetuals - USDT-margined (`/fapi/v1/*` endpoints)
- ❌ Inverse perpetuals (not supported)

### 1.2 Authentication

**Market Data**: ✅ **No authentication required**
- All REST market data endpoints are public (security type: NONE)
- All public WebSocket streams require no authentication
- Trading/account endpoints require Web3 wallet signatures (not needed for our use case)

### 1.3 Rate Limiting

**Limits:**
- IP-based: **2400 weight per minute**
- Order-based: **1200 orders per minute** (not relevant for market data)
- Response headers: `X-MBX-USED-WEIGHT-1M`, `X-MBX-ORDER-COUNT-1M`
- Error codes: `429` (rate limit exceeded), `418` (IP auto-banned)

**Strategy:** Use `FixedWindowBucket` with 5% safety buffer

### 1.4 REST API Endpoints (Public Market Data)

| Endpoint | Weight | Purpose | Flowsurface Mapping |
|----------|--------|---------|---------------------|
| `GET /fapi/v1/exchangeInfo` | 1 | Symbol information, filters, tick sizes | `fetch_ticksize` |
| `GET /fapi/v1/ticker/24hr` | 1-40 | 24h ticker statistics | `fetch_ticker_prices` |
| `GET /fapi/v1/klines` | 1-10 | Historical OHLCV data | `fetch_klines` |
| `GET /fapi/v1/openInterest/hist` | 1 | Historical open interest | `fetch_historical_oi` |
| `GET /fapi/v1/depth` | 2-20 | Orderbook snapshot | Orderbook sync helper |

### 1.5 WebSocket Streams

**Connection Limits:**
- 24-hour maximum connection validity
- 10 messages/second incoming limit
- Max 200 stream subscriptions per connection
- Ping/pong: Server sends ping every 5 minutes, 15-minute timeout

**Available Streams:**

| Stream | Format | Update Speed | Purpose |
|--------|--------|--------------|---------|
| Aggregate Trades | `<symbol>@aggTrade` | 100ms | Real-time trades |
| Depth Updates | `<symbol>@depth` | 250ms/500ms/100ms | Orderbook updates |
| Kline/Candlestick | `<symbol>@kline_<interval>` | 250ms | Real-time candles |
| Combined Streams | `/stream?streams=<s1>/<s2>` | Varies | Multiple subscriptions |

**Depth Stream Critical Details:**
- Uses **incremental updates** (not snapshots)
- Requires sequence validation with `U`, `u`, `pu` fields
- Must sync with REST snapshot on connection
- Update format: `{"U": firstId, "u": finalId, "pu": prevFinalId, "b": [[price, qty]], "a": [[price, qty]]}`

### 1.6 Data Formats

**Klines (Array format):**
```json
[
  1499040000000,      // [0] Open time
  "0.01634790",       // [1] Open
  "0.80000000",       // [2] High
  "0.01575800",       // [3] Low
  "0.01577100",       // [4] Close
  "148976.11427815",  // [5] Volume
  1499644799999,      // [6] Close time
  "2434.19055334",    // [7] Quote asset volume
  308,                // [8] Number of trades
  "1756.87402397",    // [9] Taker buy base volume
  "28.46694368",      // [10] Taker buy quote volume
  "17928899.62484339" // [11] Ignore
]
```

**Symbol Info (Nested filters):**
```json
{
  "symbol": "BTCUSDT",
  "status": "TRADING",
  "filters": [
    {"filterType": "PRICE_FILTER", "tickSize": "0.01"},
    {"filterType": "LOT_SIZE", "minQty": "0.001", "stepSize": "0.001"}
  ]
}
```

**Depth Updates (Incremental):**
```json
{
  "e": "depthUpdate",
  "U": 157,  // First update ID in event
  "u": 160,  // Final update ID in event
  "pu": 149, // Previous final update ID
  "b": [["9168.86", "0.100"], ["9168.50", "0"]],  // 0 qty = remove
  "a": [["9169.14", "0.258"]]
}
```

**Timeframe Mapping:**
- Minutes: `1m, 3m, 5m, 15m, 30m`
- Hours: `1h, 2h, 4h, 6h, 8h, 12h`
- Days: `1d, 3d`
- Week: `1w`
- Month: `1M`

### 1.7 Key Implementation Considerations

1. **No Server-Side Depth Aggregation**: Must use client-side aggregation via `LocalDepthCache`
2. **Orderbook Sync Required**: Follow Binance pattern for sequence validation
3. **Array Deserialization**: Klines come as arrays, not objects
4. **Filter Parsing**: Symbol info has nested filters that need extraction
5. **Volume Currency**: Handle `SIZE_IN_QUOTE_CURRENCY` flag for proper volume display
6. **Open Interest**: Actually supported (bonus feature vs other DEXs)

---

## 2. Flowsurface Provider Architecture

### 2.1 Modular Adapter Pattern

Flowsurface uses a **modular adapter pattern** where each exchange is a self-contained module implementing a standard interface.

**Core Components:**
```
exchange/src/
├── adapter.rs          # Central dispatch & Exchange enum
├── adapter/
│   ├── binance.rs      # Binance implementation
│   ├── bybit.rs        # Bybit implementation
│   ├── hyperliquid.rs  # Hyperliquid (reference for new adapters)
│   ├── okex.rs         # OKX implementation
│   └── aster.rs        # ← NEW: Aster implementation
├── connect.rs          # WebSocket utilities
├── depth.rs            # LocalDepthCache for orderbook
├── limiter.rs          # Rate limiting (FixedWindowBucket, DynamicBucket)
└── lib.rs              # Public API (Ticker, Kline, Trade types)
```

### 2.2 Exchange Enum System

Each exchange has enum variants for different market types:

```rust
pub enum Exchange {
    BinanceLinear,    // Binance USDT perpetuals
    BinanceSpot,      // Binance spot markets
    BybitLinear,      // Bybit linear perps
    // ... etc
    AsterLinear,      // ← NEW
    AsterSpot,        // ← NEW
}
```

**Configuration Methods:**
- `market_type()` → Returns `MarketKind::{Spot, LinearPerps, InversePerps}`
- `is_depth_client_aggr()` → `true` if client-side orderbook aggregation needed
- `is_custom_push_freq()` → `true` if custom update frequencies supported
- `allowed_push_freqs()` → List of supported push frequencies
- `supports_heatmap_timeframe(tf)` → Whether timeframe is supported

### 2.3 Core Data Types

**From `exchange/src/lib.rs`:**

```rust
// Fixed-size ticker representation
pub struct Ticker {
    pub symbol: String,
    pub exchange: Exchange,
}

// Ticker metadata
pub struct TickerInfo {
    pub ticker: Ticker,
    pub min_ticksize: f32,      // Price tick size
    pub min_qty: f32,            // Minimum order quantity
    pub contract_size: Option<f32>, // For inverse perps
}

// Candlestick data
pub struct Kline {
    pub time: u64,              // Open time (ms)
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
    pub taker_buy_volume: f32,
}

// Individual trade
pub struct Trade {
    pub price: f32,
    pub qty: f32,
    pub side: String,           // "buy" or "sell"
    pub time: u64,              // Timestamp (ms)
}

// 24h ticker statistics
pub struct TickerStats {
    pub mark_price: f32,
    pub daily_price_chg: f32,   // Percentage change
    pub daily_volume: f32,
}

// Open interest (for perpetuals)
pub struct OpenInterest {
    pub time: u64,
    pub value: f32,
}
```

### 2.4 Event-Driven WebSocket Model

All WebSocket streams emit events through a unified `Event` enum:

```rust
pub enum Event {
    Connected(Exchange),
    Disconnected(Exchange, String),
    DepthReceived(StreamKind, u64, Depth, Box<[Trade]>),
    KlineReceived(StreamKind, Kline),
}

pub enum StreamKind {
    Market(TickerInfo, Option<TickMultiplier>),
    Kline(TickerInfo, Timeframe),
}
```

**State Machine Pattern:**
```rust
enum State {
    Disconnected,
    Connected(WebSocketConnection),
}

// Streams automatically reconnect on errors
loop {
    match &mut state {
        State::Disconnected => { /* reconnect */ }
        State::Connected(ws) => { /* process messages */ }
    }
}
```

### 2.5 Orderbook Management

**LocalDepthCache** (`exchange/src/depth.rs`):
- Maintains sorted bid/ask levels
- Handles incremental updates
- Supports client-side price aggregation
- Removes zero-quantity levels automatically

```rust
pub struct LocalDepthCache {
    bids: BTreeMap<Price, f32>,  // Price → Quantity
    asks: BTreeMap<Price, f32>,
}

pub enum DepthUpdate {
    Snapshot(DepthPayload),      // Full orderbook
    Diff(DepthPayload),          // Incremental update
}

impl LocalDepthCache {
    pub fn update(&mut self, update: DepthUpdate, tick_size: f32);
    pub fn get_depth(&self) -> Depth;
}
```

### 2.6 Required Functions per Adapter

Each adapter module must implement:

**REST API Functions:**
1. `fetch_ticksize(MarketKind) -> HashMap<Ticker, Option<TickerInfo>>`
   - Fetches all available symbols with tick sizes and constraints

2. `fetch_ticker_prices(MarketKind) -> HashMap<Ticker, TickerStats>`
   - Fetches 24h ticker statistics for all symbols

3. `fetch_klines(TickerInfo, Timeframe, Option<(u64, u64)>) -> Vec<Kline>`
   - Fetches historical candlestick data

4. `fetch_historical_oi(Ticker, Option<(u64, u64)>, Timeframe) -> Vec<OpenInterest>` *(optional)*
   - Fetches historical open interest data

**WebSocket Functions:**
5. `connect_market_stream(TickerInfo, Option<TickMultiplier>, PushFrequency) -> impl Stream<Item = Event>`
   - Connects to depth + trades stream
   - Manages orderbook via LocalDepthCache
   - Emits DepthReceived events with buffered trades

6. `connect_kline_stream(Vec<(TickerInfo, Timeframe)>, MarketKind) -> impl Stream<Item = Event>`
   - Connects to kline/candlestick stream
   - Supports multiple (ticker, timeframe) subscriptions
   - Emits KlineReceived events

**Helper Functions:**
7. Rate limiter implementation (`RateLimiter` trait)
8. Serde deserialization structs for API responses
9. WebSocket message parsing logic

### 2.7 Central Dispatch System

**File: `exchange/src/adapter.rs`**

All adapter functions are called through central dispatch functions:

```rust
pub async fn fetch_ticker_info(exchange: Exchange) -> Result<...> {
    let market_type = exchange.market_type();
    match exchange {
        Exchange::BinanceLinear | Exchange::BinanceSpot => binance::fetch_ticksize(market_type).await,
        Exchange::AsterLinear | Exchange::AsterSpot => aster::fetch_ticksize(market_type).await,
        // ...
    }
}

// Similar pattern for:
// - fetch_ticker_prices
// - fetch_klines
// - fetch_open_interest
// - connect_market_stream
// - connect_kline_stream
```

This allows the rest of the application to be exchange-agnostic.

---

## 3. Implementation Plan

### 3.1 Overview

We will implement Aster DEX support by:
1. Adding Aster variants to the `Exchange` enum
2. Creating the `aster.rs` adapter module
3. Implementing all required REST and WebSocket functions
4. Wiring up the central dispatch
5. Testing the integration

**Estimated Effort:** 1-2 days for experienced Rust developer

**Reference Implementation:** Use `exchange/src/adapter/hyperliquid.rs` as the primary template (most similar architecture)

### 3.2 Implementation Phases

#### Phase 1: Enum Setup (30 minutes)
- Add `AsterLinear` and `AsterSpot` to `Exchange` enum
- Implement all enum traits (Display, FromStr)
- Configure exchange properties (market type, depth aggregation, timeframes)
- Add to `ExchangeInclusive` enum

#### Phase 2: Module Structure (1 hour)
- Create `exchange/src/adapter/aster.rs`
- Define constants (API URLs, rate limits)
- Implement rate limiter (FixedWindowBucket)
- Define all serde deserialization structs

#### Phase 3: REST API Implementation (3-4 hours)
- Implement `fetch_ticksize` with filter parsing
- Implement `fetch_ticker_prices`
- Implement `fetch_klines` with array deserialization
- Implement `fetch_historical_oi` (bonus feature)
- Write unit tests for each function

#### Phase 4: WebSocket Implementation (4-5 hours)
- Implement WebSocket connection helpers
- Implement `connect_market_stream` with proper orderbook sync
- Implement `connect_kline_stream`
- Handle ping/pong and reconnection logic
- Write integration tests

#### Phase 5: Integration & Testing (2-3 hours)
- Wire up all dispatch functions in `adapter.rs`
- Run compilation tests
- Run unit tests
- Manual WebSocket testing
- End-to-end UI testing

#### Phase 6: Documentation (1 hour)
- Add module documentation
- Update README.md
- Document known limitations
- Create troubleshooting notes

**Total Estimated Time:** 12-15 hours

---

## 4. File Modifications

### Files to Modify

| File Path | Type | Description |
|-----------|------|-------------|
| `exchange/src/adapter.rs` | **MODIFY** | Add Aster enum variants, update dispatch functions |
| `exchange/src/adapter/aster.rs` | **CREATE** | New adapter module implementation |
| `README.md` | **MODIFY** | Add Aster to supported exchanges list |

### Detailed File Changes

#### 4.1 `exchange/src/adapter.rs` (MODIFY)

**Purpose:** Central dispatch and enum definitions

**Changes Required:**
1. Add module declaration for `aster`
2. Add `AsterLinear` and `AsterSpot` to `Exchange` enum
3. Add to `Exchange::ALL` constant array
4. Implement `Display` trait for new variants
5. Implement `FromStr` trait for new variants
6. Update `market_type()` method
7. Update `is_depth_client_aggr()` method
8. Update `is_custom_push_freq()` method
9. Update `allowed_push_freqs()` method
10. Update `supports_heatmap_timeframe()` method
11. Add `Aster` to `ExchangeInclusive` enum
12. Add to `ExchangeInclusive::ALL` array
13. Update `ExchangeInclusive::of()` method
14. Update dispatch functions (6 functions total)

**Lines to modify:** ~20 locations across the file

#### 4.2 `exchange/src/adapter/aster.rs` (CREATE)

**Purpose:** Complete Aster DEX adapter implementation

**Sections to implement:**
1. Module documentation and imports
2. Constants (API URLs, rate limits)
3. Rate limiter implementation
4. Serde deserialization structs
5. REST API functions (4 functions)
6. WebSocket helper functions (3 functions)
7. WebSocket stream functions (2 functions)
8. Unit tests module

**Estimated lines:** ~800-1000 lines

#### 4.3 `README.md` (MODIFY)

**Purpose:** Update documentation

**Changes Required:**
1. Add "Aster DEX" to supported exchanges list
2. Update feature matrix table
3. Add any Aster-specific notes or limitations

**Lines to modify:** ~10 lines

---

## 5. Function Implementations

### 5.1 `exchange/src/adapter.rs` - Enum Updates

#### Function: `Exchange` enum definition
**Location:** ~line 420
**Action:** Add variants
```rust
pub enum Exchange {
    // ... existing variants
    AsterLinear,
    AsterSpot,
}
```

#### Constant: `Exchange::ALL`
**Location:** ~line 450
**Action:** Add to array
```rust
pub const ALL: &'static [Exchange] = &[
    // ... existing
    Exchange::AsterLinear,
    Exchange::AsterSpot,
];
```

#### Function: `Display::fmt`
**Location:** ~line 470
**Action:** Add match arms
```rust
impl Display for Exchange {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            // ... existing
            Exchange::AsterLinear => write!(f, "Aster Linear"),
            Exchange::AsterSpot => write!(f, "Aster Spot"),
        }
    }
}
```

#### Function: `FromStr::from_str`
**Location:** ~line 490
**Action:** Add match arms
```rust
impl FromStr for Exchange {
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            // ... existing
            "Aster Linear" => Ok(Exchange::AsterLinear),
            "Aster Spot" => Ok(Exchange::AsterSpot),
            _ => Err(()),
        }
    }
}
```

#### Function: `market_type`
**Location:** ~line 510
**Action:** Add match arms
```rust
pub fn market_type(&self) -> MarketKind {
    match self {
        // ... existing
        Exchange::AsterLinear => MarketKind::LinearPerps,
        Exchange::AsterSpot => MarketKind::Spot,
    }
}
```

#### Function: `is_depth_client_aggr`
**Location:** ~line 530
**Action:** Add match arm
```rust
pub fn is_depth_client_aggr(&self) -> bool {
    match self {
        // ... existing
        Exchange::AsterLinear | Exchange::AsterSpot => true,
    }
}
```

#### Function: `is_custom_push_freq`
**Location:** ~line 550
**Action:** Add match arm
```rust
pub fn is_custom_push_freq(&self) -> bool {
    match self {
        // ... existing
        Exchange::AsterLinear | Exchange::AsterSpot => false,
    }
}
```

#### Function: `allowed_push_freqs`
**Location:** ~line 570
**Action:** Add match arm
```rust
pub fn allowed_push_freqs(&self) -> &[PushFrequency] {
    match self {
        // ... existing
        Exchange::AsterLinear | Exchange::AsterSpot => &[PushFrequency::ServerDefault],
    }
}
```

#### Function: `supports_heatmap_timeframe`
**Location:** ~line 590
**Action:** Add match arm
```rust
pub fn supports_heatmap_timeframe(&self, tf: Timeframe) -> bool {
    match self {
        // ... existing
        Exchange::AsterLinear | Exchange::AsterSpot => {
            matches!(tf,
                Timeframe::M1 | Timeframe::M3 | Timeframe::M5 | Timeframe::M15 | Timeframe::M30 |
                Timeframe::H1 | Timeframe::H2 | Timeframe::H4 | Timeframe::H6 |
                Timeframe::H8 | Timeframe::H12 | Timeframe::D1 | Timeframe::D3 |
                Timeframe::W1 | Timeframe::MN
            )
        }
    }
}
```

#### Enum: `ExchangeInclusive` definition
**Location:** ~line 610
**Action:** Add variant
```rust
pub enum ExchangeInclusive {
    // ... existing
    Aster,
}
```

#### Constant: `ExchangeInclusive::ALL`
**Location:** ~line 630
**Action:** Add to array
```rust
pub const ALL: &'static [ExchangeInclusive] = &[
    // ... existing
    ExchangeInclusive::Aster,
];
```

#### Function: `ExchangeInclusive::of`
**Location:** ~line 650
**Action:** Add match arm
```rust
pub fn of(exchange: Exchange) -> Self {
    match exchange {
        // ... existing
        Exchange::AsterLinear | Exchange::AsterSpot => ExchangeInclusive::Aster,
    }
}
```

#### Function: `fetch_ticker_info`
**Location:** ~line 595
**Action:** Add dispatch arm
```rust
pub async fn fetch_ticker_info(exchange: Exchange) -> Result<HashMap<Ticker, Option<TickerInfo>>, AdapterError> {
    let market_type = exchange.market_type();
    match exchange {
        // ... existing
        Exchange::AsterLinear | Exchange::AsterSpot => {
            aster::fetch_ticksize(market_type).await
        }
    }
}
```

#### Function: `fetch_ticker_prices`
**Location:** ~line 616
**Action:** Add dispatch arm
```rust
pub async fn fetch_ticker_prices(exchange: Exchange) -> Result<HashMap<Ticker, TickerStats>, AdapterError> {
    let market_type = exchange.market_type();
    match exchange {
        // ... existing
        Exchange::AsterLinear | Exchange::AsterSpot => {
            aster::fetch_ticker_prices(market_type).await
        }
    }
}
```

#### Function: `fetch_klines`
**Location:** ~line 637
**Action:** Add dispatch arm
```rust
pub async fn fetch_klines(ticker_info: TickerInfo, timeframe: Timeframe, range: Option<(u64, u64)>) -> Result<Vec<Kline>, AdapterError> {
    match ticker_info.ticker.exchange {
        // ... existing
        Exchange::AsterLinear | Exchange::AsterSpot => {
            aster::fetch_klines(ticker_info, timeframe, range).await
        }
    }
}
```

#### Function: `fetch_open_interest`
**Location:** ~line 658
**Action:** Add dispatch arm
```rust
pub async fn fetch_open_interest(ticker: Ticker, range: Option<(u64, u64)>, timeframe: Timeframe) -> Result<Vec<OpenInterest>, AdapterError> {
    match ticker.exchange {
        // ... existing
        Exchange::AsterLinear => {
            aster::fetch_historical_oi(ticker, range, timeframe).await
        }
        Exchange::AsterSpot => {
            Err(AdapterError::InvalidRequest("OI not available for spot".into()))
        }
        _ => Err(AdapterError::InvalidRequest("OI not supported".into())),
    }
}
```

#### Function: `connect_market_stream`
**Location:** ~line 680
**Action:** Add dispatch arm
```rust
pub fn connect_market_stream(ticker_info: TickerInfo, tick_multiplier: Option<TickMultiplier>, push_freq: PushFrequency) -> impl Stream<Item = Event> {
    match ticker_info.ticker.exchange {
        // ... existing
        Exchange::AsterLinear | Exchange::AsterSpot => {
            aster::connect_market_stream(ticker_info, tick_multiplier, push_freq)
        }
    }
}
```

#### Function: `connect_kline_stream`
**Location:** ~line 700
**Action:** Add dispatch arm
```rust
pub fn connect_kline_stream(streams: Vec<(TickerInfo, Timeframe)>, market: MarketKind) -> impl Stream<Item = Event> {
    if streams.is_empty() {
        return stream::channel(1, |_| async {});
    }

    let exchange = streams[0].0.ticker.exchange;
    match exchange {
        // ... existing
        Exchange::AsterLinear | Exchange::AsterSpot => {
            aster::connect_kline_stream(streams, market)
        }
    }
}
```

#### Module Declaration
**Location:** Top of file (~line 12)
**Action:** Add module
```rust
pub mod aster;
```

---

### 5.2 `exchange/src/adapter/aster.rs` - New Implementation

This is a new file with complete implementation. Functions listed in implementation order:

#### 1. Module Documentation
**Purpose:** Comprehensive module-level documentation
**Lines:** 1-50

#### 2. Imports Section
**Purpose:** All required dependencies
**Lines:** 51-80

#### 3. Constants
**Purpose:** API configuration
```rust
const API_DOMAIN: &str = "https://fapi.asterdex.com";
const WS_DOMAIN: &str = "wss://fstream.asterdex.com";
const LIMIT: usize = 2400;  // Weight limit per minute
const REFILL_RATE: Duration = Duration::from_secs(60);
const LIMITER_BUFFER_PCT: f32 = 0.05;
```
**Lines:** 81-90

#### 4. Rate Limiter: `AsterLimiter` struct
**Purpose:** Rate limiting implementation
```rust
pub struct AsterLimiter {
    bucket: limiter::FixedWindowBucket,
}
```
**Lines:** 91-110

#### 5. Function: `AsterLimiter::new`
**Purpose:** Initialize rate limiter
```rust
pub fn new() -> Self
```
**Lines:** 111-115

#### 6. Function: `RateLimiter::prepare_request`
**Purpose:** Check rate limit before request
```rust
fn prepare_request(&mut self, weight: usize) -> Option<Duration>
```
**Lines:** 116-120

#### 7. Function: `RateLimiter::update_from_response`
**Purpose:** Update rate limiter after response
```rust
fn update_from_response(&mut self, response: &reqwest::Response, weight: usize)
```
**Lines:** 121-125

#### 8. Function: `RateLimiter::should_exit_on_response`
**Purpose:** Check for rate limit errors
```rust
fn should_exit_on_response(&self, response: &reqwest::Response) -> bool
```
**Lines:** 126-130

#### 9. Static: `ASTER_LIMITER`
**Purpose:** Global rate limiter instance
```rust
static ASTER_LIMITER: LazyLock<Mutex<AsterLimiter>> = LazyLock::new(|| Mutex::new(AsterLimiter::new()));
```
**Lines:** 131-135

#### 10. Struct: `ExchangeInfoResponse`
**Purpose:** Deserialize exchangeInfo response
```rust
#[derive(Deserialize)]
struct ExchangeInfoResponse {
    symbols: Vec<AsterSymbolInfo>,
}
```
**Lines:** 136-145

#### 11. Struct: `AsterSymbolInfo`
**Purpose:** Deserialize symbol information
```rust
#[derive(Deserialize)]
struct AsterSymbolInfo {
    symbol: String,
    status: String,
    #[serde(rename = "contractType")]
    contract_type: Option<String>,
    filters: Vec<Value>,
}
```
**Lines:** 146-160

#### 12. Struct: `AsterTickerStats`
**Purpose:** Deserialize 24h ticker stats
```rust
#[derive(Deserialize)]
struct AsterTickerStats {
    symbol: String,
    #[serde(deserialize_with = "de_string_to_f32")]
    #[serde(rename = "lastPrice")]
    last_price: f32,
    #[serde(deserialize_with = "de_string_to_f32")]
    #[serde(rename = "priceChangePercent")]
    price_change_percent: f32,
    #[serde(deserialize_with = "de_string_to_f32")]
    volume: f32,
}
```
**Lines:** 161-180

#### 13. Struct: `AsterKline` (tuple struct for array deserialization)
**Purpose:** Deserialize kline array format
```rust
#[derive(Deserialize)]
struct AsterKline(
    #[serde(deserialize_with = "de_string_to_u64")] u64,  // open_time
    #[serde(deserialize_with = "de_string_to_f32")] f32,  // open
    #[serde(deserialize_with = "de_string_to_f32")] f32,  // high
    #[serde(deserialize_with = "de_string_to_f32")] f32,  // low
    #[serde(deserialize_with = "de_string_to_f32")] f32,  // close
    #[serde(deserialize_with = "de_string_to_f32")] f32,  // volume
    #[serde(deserialize_with = "de_string_to_u64")] u64,  // close_time
    #[serde(deserialize_with = "de_string_to_f32")] f32,  // quote_volume
    u64,  // num_trades
    #[serde(deserialize_with = "de_string_to_f32")] f32,  // taker_buy_base
    #[serde(deserialize_with = "de_string_to_f32")] f32,  // taker_buy_quote
    String, // ignore
);
```
**Lines:** 181-200

#### 14. Struct: `AsterOIData`
**Purpose:** Deserialize open interest data
```rust
#[derive(Deserialize)]
struct AsterOIData {
    #[serde(deserialize_with = "de_string_to_u64")]
    timestamp: u64,
    #[serde(rename = "sumOpenInterest", deserialize_with = "de_string_to_f32")]
    open_interest: f32,
}
```
**Lines:** 201-215

#### 15. Struct: `AsterDepthSnapshot`
**Purpose:** Deserialize REST depth snapshot
```rust
#[derive(Deserialize)]
struct AsterDepthSnapshot {
    #[serde(rename = "lastUpdateId")]
    last_update_id: u64,
    bids: Vec<Vec<String>>,
    asks: Vec<Vec<String>>,
}
```
**Lines:** 216-230

#### 16. Struct: `AsterWSMessage`
**Purpose:** Deserialize WebSocket wrapper
```rust
#[derive(Deserialize)]
struct AsterWSMessage {
    stream: String,
    data: Value,
}
```
**Lines:** 231-240

#### 17. Struct: `AsterDepthUpdate`
**Purpose:** Deserialize depth update from WebSocket
```rust
#[derive(Deserialize)]
struct AsterDepthUpdate {
    #[serde(rename = "U")]
    first_update_id: u64,
    #[serde(rename = "u")]
    final_update_id: u64,
    #[serde(rename = "pu")]
    prev_final_update_id: u64,
    #[serde(rename = "b")]
    bids: Vec<Vec<String>>,
    #[serde(rename = "a")]
    asks: Vec<Vec<String>>,
}
```
**Lines:** 241-260

#### 18. Struct: `AsterTrade`
**Purpose:** Deserialize aggregate trade
```rust
#[derive(Deserialize)]
struct AsterTrade {
    #[serde(rename = "p", deserialize_with = "de_string_to_f32")]
    price: f32,
    #[serde(rename = "q", deserialize_with = "de_string_to_f32")]
    qty: f32,
    #[serde(rename = "m")]
    is_buyer_maker: bool,
    #[serde(rename = "T", deserialize_with = "de_string_to_u64")]
    timestamp: u64,
}
```
**Lines:** 261-280

#### 19. Struct: `AsterWSKline`
**Purpose:** Deserialize WebSocket kline
```rust
#[derive(Deserialize)]
struct AsterWSKline {
    #[serde(rename = "t", deserialize_with = "de_string_to_u64")]
    time: u64,
    #[serde(rename = "o", deserialize_with = "de_string_to_f32")]
    open: f32,
    #[serde(rename = "h", deserialize_with = "de_string_to_f32")]
    high: f32,
    #[serde(rename = "l", deserialize_with = "de_string_to_f32")]
    low: f32,
    #[serde(rename = "c", deserialize_with = "de_string_to_f32")]
    close: f32,
    #[serde(rename = "v", deserialize_with = "de_string_to_f32")]
    volume: f32,
    #[serde(rename = "x")]
    is_closed: bool,
}
```
**Lines:** 281-310

#### 20. Enum: `StreamData`
**Purpose:** Internal WebSocket data enum
```rust
enum StreamData {
    Trade(Vec<AsterTrade>),
    Depth(AsterDepthUpdate),
    Kline(AsterWSKline),
}
```
**Lines:** 311-320

#### 21. Function: `extract_tick_size`
**Purpose:** Extract tick size from filters array
```rust
fn extract_tick_size(filters: &[Value]) -> Option<f32>
```
**Lines:** 321-335

#### 22. Function: `extract_min_qty`
**Purpose:** Extract min quantity from filters array
```rust
fn extract_min_qty(filters: &[Value]) -> Option<f32>
```
**Lines:** 336-350

#### 23. Function: `timeframe_to_interval`
**Purpose:** Map Timeframe enum to Aster interval string
```rust
fn timeframe_to_interval(tf: Timeframe) -> Option<&'static str>
```
**Lines:** 351-375

#### 24. Function: `parse_price_qty_array`
**Purpose:** Parse [[price, qty]] arrays to DeOrder
```rust
fn parse_price_qty_array(arr: Vec<Vec<String>>, tick_size: f32) -> Vec<DeOrder>
```
**Lines:** 376-395

#### 25. Function: `fetch_ticksize`
**Purpose:** Fetch all ticker information (PUBLIC FUNCTION)
```rust
pub async fn fetch_ticksize(market: MarketKind) -> Result<HashMap<Ticker, Option<TickerInfo>>, AdapterError>
```
**Implementation:**
- Determine endpoint based on market type (spot vs linear)
- Make HTTP request with rate limiting
- Parse response into `ExchangeInfoResponse`
- Extract tick_size and min_qty from filters
- Filter by status == "TRADING"
- Build HashMap of Ticker → TickerInfo
**Lines:** 396-460

#### 26. Function: `fetch_ticker_prices`
**Purpose:** Fetch 24h ticker statistics (PUBLIC FUNCTION)
```rust
pub async fn fetch_ticker_prices(market: MarketKind) -> Result<HashMap<Ticker, TickerStats>, AdapterError>
```
**Implementation:**
- Determine endpoint based on market type
- Make HTTP request with rate limiting
- Parse response into Vec<AsterTickerStats>
- Map to HashMap of Ticker → TickerStats
- Calculate percentage change from priceChangePercent field
**Lines:** 461-510

#### 27. Function: `fetch_klines`
**Purpose:** Fetch historical candlestick data (PUBLIC FUNCTION)
```rust
pub async fn fetch_klines(ticker_info: TickerInfo, timeframe: Timeframe, range: Option<(u64, u64)>) -> Result<Vec<Kline>, AdapterError>
```
**Implementation:**
- Map timeframe to interval string
- Determine time range (default 500 candles if None)
- Build URL with query parameters
- Make HTTP request with rate limiting
- Deserialize as Vec<AsterKline> (array format)
- Convert to Vec<Kline>
- Handle SIZE_IN_QUOTE_CURRENCY flag for volume
- Round prices with min_ticksize
**Lines:** 511-590

#### 28. Function: `fetch_historical_oi`
**Purpose:** Fetch historical open interest (PUBLIC FUNCTION)
```rust
pub async fn fetch_historical_oi(ticker: Ticker, range: Option<(u64, u64)>, timeframe: Timeframe) -> Result<Vec<OpenInterest>, AdapterError>
```
**Implementation:**
- Validate market type (perps only)
- Map timeframe to period string
- Determine time range (default 500 points if None)
- Build URL with parameters
- Make HTTP request
- Parse into Vec<AsterOIData>
- Map to Vec<OpenInterest>
**Lines:** 591-650

#### 29. Function: `fetch_depth_snapshot`
**Purpose:** Fetch REST orderbook snapshot for sync
```rust
async fn fetch_depth_snapshot(symbol: &str, market: MarketKind) -> Result<AsterDepthSnapshot, AdapterError>
```
**Implementation:**
- Determine endpoint based on market type
- Build URL with symbol and limit=1000
- Make HTTP request
- Parse into AsterDepthSnapshot
- Return for orderbook synchronization
**Lines:** 651-680

#### 30. Function: `connect_websocket`
**Purpose:** Establish WebSocket connection
```rust
async fn connect_websocket(path: &str) -> Result<fastwebsockets::FragmentCollector<...>, AdapterError>
```
**Implementation:**
- Use connect_ws utility from crate::connect
- Connect to WS_DOMAIN with path
- Return FragmentCollector for reading frames
**Lines:** 681-695

#### 31. Function: `parse_websocket_message`
**Purpose:** Parse WebSocket frame payload
```rust
fn parse_websocket_message(payload: &[u8]) -> Result<StreamData, AdapterError>
```
**Implementation:**
- Deserialize as AsterWSMessage (wrapper)
- Check stream field to determine type
- Parse data field as appropriate type:
  - Contains "depth" → AsterDepthUpdate
  - Contains "aggTrade" → Vec<AsterTrade>
  - Contains "kline" → AsterWSKline
- Return StreamData enum
**Lines:** 696-730

#### 32. Function: `connect_market_stream`
**Purpose:** Connect to depth + trades stream (PUBLIC FUNCTION)
```rust
pub fn connect_market_stream(ticker_info: TickerInfo, tick_multiplier: Option<TickMultiplier>, push_freq: PushFrequency) -> impl Stream<Item = Event>
```
**Implementation:**
- Create channel stream with buffer
- Initialize state machine (Disconnected)
- Initialize LocalDepthCache
- Create trades buffer
- Loop:
  - **Disconnected state:**
    - Build WebSocket path: `/stream?streams={symbol}@depth/{symbol}@aggTrade`
    - Connect to WebSocket
    - Fetch depth snapshot from REST
    - Initialize orderbook with snapshot
    - Transition to Connected
  - **Connected state:**
    - Read WebSocket frame with timeout
    - Handle OpCode::Text:
      - Parse message
      - If Depth update:
        - Validate sequence (U, u, pu)
        - If sequence break → re-sync (go to Disconnected)
        - Convert to DepthUpdate::Diff
        - Update LocalDepthCache
        - Get current depth snapshot
        - Drain trades buffer
        - Emit DepthReceived event
      - If Trade:
        - Convert to Trade struct
        - Buffer in trades_buffer
    - Handle OpCode::Ping → respond with Pong
    - Handle OpCode::Close → go to Disconnected
    - Handle errors → go to Disconnected
**Lines:** 731-890

#### 33. Function: `connect_kline_stream`
**Purpose:** Connect to kline stream (PUBLIC FUNCTION)
```rust
pub fn connect_kline_stream(streams: Vec<(TickerInfo, Timeframe)>, market: MarketKind) -> impl Stream<Item = Event>
```
**Implementation:**
- Determine exchange from first stream
- Build subscription path with all (ticker, timeframe) pairs
- Format: `/stream?streams={symbol1}@kline_{interval1}/{symbol2}@kline_{interval2}`
- Create channel stream
- State machine loop:
  - **Disconnected:**
    - Connect to WebSocket
    - Transition to Connected
  - **Connected:**
    - Read frame
    - Parse kline message
    - Match symbol from stream name to find TickerInfo
    - Handle SIZE_IN_QUOTE_CURRENCY for volume
    - Create Kline struct
    - Emit KlineReceived event
    - Handle ping/pong
    - Handle errors → reconnect
**Lines:** 891-1020

#### 34. Test Module: `tests`
**Purpose:** Unit and integration tests
```rust
#[cfg(test)]
mod tests { ... }
```
**Tests to include:**
- `test_fetch_ticksize_spot` - Verify spot ticker fetching
- `test_fetch_ticksize_perps` - Verify perps ticker fetching
- `test_fetch_ticker_prices` - Verify 24h stats
- `test_fetch_klines` - Verify kline fetching
- `test_fetch_historical_oi` - Verify OI data
- `test_timeframe_mapping` - Verify interval mapping
- `manual_websocket_market_test` - Manual WS depth+trades test (ignored)
- `manual_websocket_kline_test` - Manual WS kline test (ignored)
**Lines:** 1021-1150

---

### 5.3 `README.md` - Documentation Updates

#### Section: Supported Exchanges
**Location:** Near top of README
**Action:** Add to list
```markdown
## Supported Exchanges

- Binance (Spot, Linear Perpetuals, Inverse Perpetuals)
- Bybit (Spot, Linear Perpetuals, Inverse Perpetuals)
- Hyperliquid (Spot, Linear Perpetuals)
- OKX (Spot, Linear Perpetuals, Inverse Perpetuals)
- **Aster DEX (Spot, Linear Perpetuals)** ← NEW
```

#### Section: Feature Matrix
**Location:** Features section
**Action:** Add row to table
```markdown
| Exchange | Spot | Linear Perps | Inverse Perps | Open Interest | Server-side Depth Aggregation |
|----------|------|--------------|---------------|---------------|-------------------------------|
| Binance  | ✅   | ✅           | ✅            | ✅            | ❌                            |
| Bybit    | ✅   | ✅           | ✅            | ✅            | ✅                            |
| Hyperliquid | ✅ | ✅         | ❌            | ❌            | ✅                            |
| OKX      | ✅   | ✅           | ✅            | ✅            | ❌                            |
| Aster DEX | ✅   | ✅          | ❌            | ✅            | ❌                            |
```

---

## Implementation Summary

### Total Changes:
- **1 file created:** `exchange/src/adapter/aster.rs` (~1000 lines)
- **2 files modified:** `exchange/src/adapter.rs` (~20 locations), `README.md` (~10 lines)
- **34 functions implemented** (20 core functions + 14 helpers/tests)
- **14 serde structs defined**
- **1 rate limiter implementation**

### Critical Implementation Details:

1. **No authentication required** - All market data endpoints are public
2. **Client-side depth aggregation** - Use LocalDepthCache
3. **Incremental orderbook updates** - Must implement sequence validation
4. **Array-based kline format** - Use tuple struct deserialization
5. **Nested filters parsing** - Extract tick_size and min_qty from filters array
6. **Open interest supported** - Bonus feature for linear perpetuals
7. **Binance-compatible** - Can reference Binance adapter patterns

### Testing Strategy:

1. **Unit tests:** Run `cargo test aster::tests --nocapture`
2. **Manual WebSocket tests:** Run with `--ignored` flag
3. **Integration test:** Launch UI, select Aster, create streams
4. **Performance test:** Monitor for 30+ minutes to check memory/reconnection

### Expected Outcome:

After implementation, users will be able to:
- ✅ Select "Aster Linear" or "Aster Spot" from exchange dropdown
- ✅ View all available trading pairs with correct tick sizes
- ✅ See real-time orderbook depth visualization
- ✅ View time & sales (trades) feed
- ✅ Display candlestick charts with multiple timeframes
- ✅ View open interest charts (linear perpetuals only)
- ✅ Experience automatic reconnection on network issues

---

**End of Implementation Plan**
