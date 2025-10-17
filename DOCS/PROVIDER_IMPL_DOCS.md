# Flowsurface Exchange Provider Implementation Guide

This document provides a comprehensive guide for implementing a new exchange provider (adapter) in Flowsurface, with specific focus on implementing Aster DEX as an example.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Phase 1: Research & Discovery](#phase-1-research--discovery)
3. [Phase 2: Type Definitions & Setup](#phase-2-type-definitions--setup)
4. [Phase 3: REST API Implementation](#phase-3-rest-api-implementation)
5. [Phase 4: WebSocket Implementation](#phase-4-websocket-implementation)
6. [Phase 5: Integration & Testing](#phase-5-integration--testing)
7. [Phase 6: Frontend Integration](#phase-6-frontend-integration)
8. [Phase 7: Documentation & Cleanup](#phase-7-documentation--cleanup)
9. [Troubleshooting](#troubleshooting)

---

## Architecture Overview

### Modular Adapter Pattern

Flowsurface uses a **modular adapter pattern** where each exchange is implemented as a separate module within `exchange/src/adapter/`. All adapters follow a consistent structure and implement the same core functionality through shared types and traits.

### Directory Structure

```
exchange/src/
├── adapter.rs          # Main adapter module with shared types and dispatch
├── adapter/
│   ├── binance.rs      # Binance implementation
│   ├── bybit.rs        # Bybit implementation
│   ├── hyperliquid.rs  # Hyperliquid implementation (recommended reference)
│   └── okex.rs         # OKX implementation
├── connect.rs          # WebSocket connection utilities
├── depth.rs            # Orderbook depth management
├── fetcher.rs          # Data fetching coordinator
├── limiter.rs          # Rate limiting
└── lib.rs              # Public API
```

### Core Types and Enums

**Exchange enum** (`adapter.rs`):
- Variants: `BinanceLinear`, `BinanceSpot`, `BybitLinear`, `HyperliquidSpot`, etc.
- Methods: `market_type()`, `is_depth_client_aggr()`, `allowed_push_freqs()`, etc.

**Event enum** (WebSocket stream events):
- `Connected(Exchange)`
- `Disconnected(Exchange, String)`
- `DepthReceived(StreamKind, u64, Depth, Box<[Trade]>)`
- `KlineReceived(StreamKind, Kline)`

**Core data types** (`lib.rs`):
- `Ticker` - Fixed-size ticker representation
- `TickerInfo` - Ticker metadata with tick size and quantity
- `Trade` - Individual trade data
- `Kline` - Candlestick/OHLCV data
- `TickerStats` - 24h ticker statistics
- `OpenInterest` - OI data for perpetuals
- `Timeframe` - Time intervals for klines

### Common Dependencies

Every adapter module imports:

```rust
// Core types from parent module
use super::super::{
    Exchange, Kline, MarketKind, Ticker, TickerInfo, TickerStats,
    Timeframe, Trade, OpenInterest, SIZE_IN_QUOTE_CURRENCY,
};

// Adapter-specific types
use super::{AdapterError, Event, StreamKind};

// WebSocket and streaming
use crate::connect::{State, connect_ws};
use fastwebsockets::{OpCode, Frame};
use iced_futures::{futures::Stream, stream};

// Depth/orderbook management
use crate::depth::{DeOrder, DepthPayload, DepthUpdate, LocalDepthCache};

// Rate limiting
use crate::limiter::{self, RateLimiter, http_request_with_limiter};

// Parsing utilities
use crate::{de_string_to_f32, de_string_to_u64};

// External libraries
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::HashMap, sync::LazyLock, time::Duration};
use tokio::sync::Mutex;
```

---

## Phase 1: Research & Discovery

### 1.1 API Documentation Research

**Objective**: Locate and document the exchange's API specifications.

**Required information to gather**:

- ✅ Official API documentation URL
- ✅ REST API base URLs (mainnet/testnet)
- ✅ WebSocket endpoints (mainnet/testnet)
- ✅ Rate limiting policies and headers
- ✅ Supported market types:
  - Spot markets
  - Linear perpetuals (USDT-margined)
  - Inverse perpetuals (coin-margined)
- ✅ Available data streams:
  - Order book / depth feed
  - Trade feed
  - Kline/candlestick feed
  - Open interest (if applicable)
- ✅ Authentication requirements (if any for public data)

**Action**: Create a reference document with links to specific API documentation sections.

### 1.2 Market Data Structures

**Document the response formats for**:

1. **Ticker/Symbol Information Endpoint**
   - Endpoint URL and HTTP method
   - Request parameters
   - Response format (JSON structure)
   - Fields: symbol name, tick size, min quantity, price decimals, qty decimals

2. **24h Ticker Statistics Endpoint**
   - Endpoint URL and method
   - Response format
   - Fields: mark price, 24h volume, 24h price change %

3. **Historical Klines Endpoint**
   - Endpoint URL and method
   - Supported timeframes
   - Time range parameters (start/end time format)
   - Response format (OHLCV data structure)

4. **Open Interest Endpoint** (if available for perps)
   - Endpoint URL
   - Response format
   - Timeframe support

**Action**: Collect and save example JSON responses for each endpoint.

### 1.3 WebSocket Specification

**Document**:

1. **Connection Details**
   - WebSocket URL format
   - Connection upgrade requirements
   - Heartbeat/ping-pong mechanism

2. **Subscription Format**
   - How to subscribe to channels (message format)
   - How to unsubscribe
   - Channel naming conventions

3. **Available Streams**
   - **Order book/depth stream**:
     - Snapshot vs incremental updates
     - Update frequency options
     - Depth aggregation/precision options
   - **Trade stream**:
     - Real-time trades format
     - Fields: price, quantity, side, timestamp
   - **Kline stream**:
     - Supported timeframes
     - Update mechanism

4. **Message Formats**
   - Subscription response
   - Data update messages
   - Error messages

**Action**: Collect example WebSocket messages for each stream type.

### 1.4 Rate Limiting Strategy

**Analyze**:

1. **Rate Limit Details**
   - Limits per endpoint type (REST)
   - Time window (per minute/second)
   - Weight system (if applicable)
   - Response headers for tracking usage

2. **Recommended Strategy**
   - Choose limiter type:
     - `FixedWindowBucket`: Simple fixed-window limiting (Hyperliquid, Bybit, OKX)
     - `DynamicBucket`: Header-based dynamic limiting (Binance)
   - Buffer percentage (safety margin) - typically 5% (0.05)
   - Initial capacity
   - Refill rate

3. **Error Handling**
   - Status codes for rate limit errors (429, 418, etc.)
   - Retry strategy
   - Backoff mechanism

**References**:
- `exchange/src/adapter/binance.rs` - Uses DynamicBucket with headers
- `exchange/src/adapter/hyperliquid.rs` - Uses FixedWindowBucket

---

## Phase 2: Type Definitions & Setup

### 2.1 Add Exchange Enum Variants

**File**: `exchange/src/adapter.rs`

**Updates required**:

1. **Add to `Exchange` enum** (around line 420):
   ```rust
   pub enum Exchange {
       // ... existing variants
       AsterLinear,
       AsterSpot,
   }
   ```

2. **Update `Exchange::ALL` array**:
   ```rust
   pub const ALL: &'static [Exchange] = &[
       // ... existing exchanges
       Exchange::AsterLinear,
       Exchange::AsterSpot,
   ];
   ```

3. **Add `Display` impl match arms**:
   ```rust
   impl Display for Exchange {
       fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
           match self {
               // ... existing cases
               Exchange::AsterLinear => write!(f, "Aster Linear"),
               Exchange::AsterSpot => write!(f, "Aster Spot"),
           }
       }
   }
   ```

4. **Add `FromStr` impl match arms**:
   ```rust
   impl FromStr for Exchange {
       fn from_str(s: &str) -> Result<Self, Self::Err> {
           match s {
               // ... existing cases
               "Aster Linear" => Ok(Exchange::AsterLinear),
               "Aster Spot" => Ok(Exchange::AsterSpot),
               _ => Err(()),
           }
       }
   }
   ```

5. **Update `market_type()` method**:
   ```rust
   pub fn market_type(&self) -> MarketKind {
       match self {
           // ... existing cases
           Exchange::AsterLinear => MarketKind::LinearPerps,
           Exchange::AsterSpot => MarketKind::Spot,
       }
   }
   ```

6. **Update configuration methods**:
   ```rust
   pub fn is_depth_client_aggr(&self) -> bool {
       match self {
           // ... existing cases
           Exchange::AsterLinear | Exchange::AsterSpot => true, // or false if server-side supported
       }
   }

   pub fn is_custom_push_freq(&self) -> bool {
       match self {
           // ... existing cases
           Exchange::AsterLinear | Exchange::AsterSpot => false, // adjust as needed
       }
   }

   pub fn allowed_push_freqs(&self) -> &[PushFrequency] {
       match self {
           // ... existing cases
           Exchange::AsterLinear | Exchange::AsterSpot => &[PushFrequency::ServerDefault],
       }
   }

   pub fn supports_heatmap_timeframe(&self, tf: Timeframe) -> bool {
       match self {
           // ... existing cases
           Exchange::AsterLinear | Exchange::AsterSpot => {
               matches!(tf, Timeframe::M1 | Timeframe::M5 | Timeframe::M15 | /* ... */)
           }
       }
   }
   ```

7. **Update `ExchangeInclusive` enum**:
   ```rust
   pub enum ExchangeInclusive {
       // ... existing variants
       Aster,
   }
   ```

8. **Update `ExchangeInclusive::ALL`**:
   ```rust
   pub const ALL: &'static [ExchangeInclusive] = &[
       // ... existing
       ExchangeInclusive::Aster,
   ];
   ```

9. **Update `ExchangeInclusive::of()` method**:
   ```rust
   pub fn of(exchange: Exchange) -> Self {
       match exchange {
           // ... existing cases
           Exchange::AsterLinear | Exchange::AsterSpot => ExchangeInclusive::Aster,
       }
   }
   ```

### 2.2 Create Provider Module File

**File**: `exchange/src/adapter/aster.rs`

**Basic structure**:

```rust
//! Aster DEX Exchange Adapter
//!
//! This module provides integration with the Aster DEX API.

use super::super::{
    Exchange, Kline, MarketKind, Ticker, TickerInfo, TickerStats,
    Timeframe, Trade, OpenInterest, SIZE_IN_QUOTE_CURRENCY,
};
use super::{AdapterError, Event, StreamKind};
use crate::connect::{State, connect_ws};
use crate::depth::{DeOrder, DepthPayload, DepthUpdate, LocalDepthCache};
use crate::limiter::{self, RateLimiter, http_request_with_limiter};
use crate::{de_string_to_f32, de_string_to_u64};

use fastwebsockets::{OpCode, Frame};
use iced_futures::{futures::Stream, stream};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::HashMap, sync::LazyLock, time::Duration};
use tokio::sync::Mutex;

// ========== Constants ==========

const API_DOMAIN: &str = "https://api.asterdex.com"; // Replace with actual
const WS_DOMAIN: &str = "wss://stream.asterdex.com"; // Replace with actual

const LIMIT: usize = 1200; // Adjust based on exchange limits
const REFILL_RATE: Duration = Duration::from_secs(60);
const LIMITER_BUFFER_PCT: f32 = 0.05;

// ========== Rate Limiter ==========

static ASTER_LIMITER: LazyLock<Mutex<AsterLimiter>> =
    LazyLock::new(|| Mutex::new(AsterLimiter::new()));

pub struct AsterLimiter {
    bucket: limiter::FixedWindowBucket,
}

impl AsterLimiter {
    pub fn new() -> Self {
        Self {
            bucket: limiter::FixedWindowBucket::new(LIMIT, REFILL_RATE, LIMITER_BUFFER_PCT),
        }
    }
}

impl RateLimiter for AsterLimiter {
    fn prepare_request(&mut self, weight: usize) -> Option<Duration> {
        self.bucket.prepare_request(weight)
    }

    fn update_from_response(&mut self, _response: &reqwest::Response, weight: usize) {
        self.bucket.update_from_response(weight);
    }

    fn should_exit_on_response(&self, response: &reqwest::Response) -> bool {
        response.status().as_u16() == 429 || response.status().as_u16() == 418
    }
}

// ========== Response Structs (to be filled in) ==========

#[derive(Debug, Deserialize)]
struct AsterSymbolInfo {
    // Fields from API
}

#[derive(Debug, Deserialize)]
struct AsterTickerStats {
    // Fields from API
}

#[derive(Debug, Deserialize)]
struct AsterKline {
    // Fields from API
}

#[derive(Debug, Deserialize)]
struct AsterDepth {
    // Fields from API
}

#[derive(Debug, Deserialize)]
struct AsterTrade {
    // Fields from API
}

// ========== Core Functions (to be implemented) ==========

// pub async fn fetch_ticksize(market: MarketKind) -> Result<HashMap<Ticker, Option<TickerInfo>>, AdapterError>
// pub async fn fetch_ticker_prices(market: MarketKind) -> Result<HashMap<Ticker, TickerStats>, AdapterError>
// pub async fn fetch_klines(ticker_info: TickerInfo, timeframe: Timeframe, range: Option<(u64, u64)>) -> Result<Vec<Kline>, AdapterError>
// pub fn connect_market_stream(ticker_info: TickerInfo, tick_multiplier: Option<TickMultiplier>, push_freq: PushFrequency) -> impl Stream<Item = Event>
// pub fn connect_kline_stream(streams: Vec<(TickerInfo, Timeframe)>, market: MarketKind) -> impl Stream<Item = Event>
```

**Reference template**: `exchange/src/adapter/hyperliquid.rs`

### 2.3 Define Serde Structs

Based on the API documentation, implement deserialization structs with proper field mappings:

```rust
#[derive(Debug, Deserialize)]
struct AsterSymbolInfo {
    #[serde(rename = "symbol")]
    symbol: String,

    #[serde(rename = "tickSize", deserialize_with = "de_string_to_f32")]
    tick_size: f32,

    #[serde(rename = "minQuantity", deserialize_with = "de_string_to_f32")]
    min_qty: f32,

    // Add other fields as needed
}

#[derive(Debug, Deserialize)]
struct AsterTickerStats {
    #[serde(rename = "symbol")]
    symbol: String,

    #[serde(rename = "lastPrice", deserialize_with = "de_string_to_f32")]
    last_price: f32,

    #[serde(rename = "priceChangePercent", deserialize_with = "de_string_to_f32")]
    price_change_pct: f32,

    #[serde(rename = "volume", deserialize_with = "de_string_to_f32")]
    volume: f32,
}

#[derive(Debug, Deserialize)]
struct AsterKline {
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
}

#[derive(Debug, Deserialize)]
struct AsterWSMessage {
    #[serde(rename = "stream")]
    stream: String,

    #[serde(rename = "data")]
    data: Value,
}

#[derive(Debug, Deserialize)]
struct AsterDepth {
    #[serde(rename = "bids")]
    bids: Vec<(String, String)>, // price, quantity

    #[serde(rename = "asks")]
    asks: Vec<(String, String)>,

    #[serde(rename = "timestamp", deserialize_with = "de_string_to_u64")]
    timestamp: u64,
}

#[derive(Debug, Deserialize)]
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

**Key considerations**:
- Use `de_string_to_f32` for string-formatted numbers
- Use `de_string_to_u64` for string-formatted timestamps
- Add `#[serde(rename = "...")]` for field name mapping
- Ensure all structs properly deserialize test responses

### 2.4 Add Module Declaration

**File**: `exchange/src/adapter.rs`

At the top of the file (around line 12-15), add:

```rust
pub mod aster;
```

---

## Phase 3: REST API Implementation

### 3.1 Implement `fetch_ticksize`

**Function signature**:
```rust
pub async fn fetch_ticksize(
    market: MarketKind,
) -> Result<HashMap<Ticker, Option<TickerInfo>>, AdapterError>
```

**Implementation pattern**:

```rust
pub async fn fetch_ticksize(
    market: MarketKind,
) -> Result<HashMap<Ticker, Option<TickerInfo>>, AdapterError> {
    // Determine endpoint based on market type
    let url = match market {
        MarketKind::Spot => format!("{}/api/v1/exchangeInfo", API_DOMAIN),
        MarketKind::LinearPerps => format!("{}/fapi/v1/exchangeInfo", API_DOMAIN),
        _ => return Err(AdapterError::InvalidRequest("Unsupported market type".into())),
    };

    // Fetch with rate limiting
    let response = http_request_with_limiter(
        &url,
        &ASTER_LIMITER,
        1, // weight
        None, // method (defaults to GET)
        None, // json body
    ).await?;

    // Parse response
    let data: Vec<AsterSymbolInfo> = serde_json::from_str(&response)
        .map_err(|e| AdapterError::ParseError(e.to_string()))?;

    // Convert to HashMap<Ticker, Option<TickerInfo>>
    let mut result = HashMap::new();

    for symbol_info in data {
        // Filter by trading status, market type, etc.
        if !is_symbol_supported(&symbol_info.symbol) {
            continue;
        }

        let exchange = match market {
            MarketKind::Spot => Exchange::AsterSpot,
            MarketKind::LinearPerps => Exchange::AsterLinear,
            _ => continue,
        };

        let ticker = Ticker::new(&symbol_info.symbol, exchange);
        let ticker_info = TickerInfo::new(
            ticker.clone(),
            symbol_info.tick_size,
            symbol_info.min_qty,
            None, // contract_size (for inverse perps)
        );

        result.insert(ticker, Some(ticker_info));
    }

    Ok(result)
}
```

**Edge cases to handle**:
- Different decimal precision formats
- Display symbols vs internal symbols
- Contract size for inverse perps
- Filtering inactive/delisted symbols

**References**:
- `exchange/src/adapter/binance.rs::fetch_ticksize`
- `exchange/src/adapter/hyperliquid.rs::fetch_ticksize`

### 3.2 Implement `fetch_ticker_prices`

**Function signature**:
```rust
pub async fn fetch_ticker_prices(
    market: MarketKind,
) -> Result<HashMap<Ticker, TickerStats>, AdapterError>
```

**Implementation pattern**:

```rust
pub async fn fetch_ticker_prices(
    market: MarketKind,
) -> Result<HashMap<Ticker, TickerStats>, AdapterError> {
    let url = match market {
        MarketKind::Spot => format!("{}/api/v1/ticker/24hr", API_DOMAIN),
        MarketKind::LinearPerps => format!("{}/fapi/v1/ticker/24hr", API_DOMAIN),
        _ => return Err(AdapterError::InvalidRequest("Unsupported market type".into())),
    };

    let response = http_request_with_limiter(&url, &ASTER_LIMITER, 1, None, None).await?;
    let data: Vec<AsterTickerStats> = serde_json::from_str(&response)
        .map_err(|e| AdapterError::ParseError(e.to_string()))?;

    let mut result = HashMap::new();
    let exchange = match market {
        MarketKind::Spot => Exchange::AsterSpot,
        MarketKind::LinearPerps => Exchange::AsterLinear,
        _ => return Ok(result),
    };

    for stats in data {
        let ticker = Ticker::new(&stats.symbol, exchange);
        let ticker_stats = TickerStats {
            mark_price: stats.last_price,
            daily_price_chg: stats.price_change_pct,
            daily_volume: stats.volume,
        };
        result.insert(ticker, ticker_stats);
    }

    Ok(result)
}
```

**Key points**:
- Calculate `daily_price_chg` as percentage: `((current - prev) / prev) * 100.0`
- Handle missing or zero previous prices gracefully
- Return HashMap<Ticker, TickerStats>

**Reference**: `exchange/src/adapter/hyperliquid.rs::fetch_ticker_prices`

### 3.3 Implement `fetch_klines`

**Function signature**:
```rust
pub async fn fetch_klines(
    ticker_info: TickerInfo,
    timeframe: Timeframe,
    range: Option<(u64, u64)>,
) -> Result<Vec<Kline>, AdapterError>
```

**Implementation pattern**:

```rust
pub async fn fetch_klines(
    ticker_info: TickerInfo,
    timeframe: Timeframe,
    range: Option<(u64, u64)>,
) -> Result<Vec<Kline>, AdapterError> {
    // Map timeframe to exchange-specific interval
    let interval = match timeframe {
        Timeframe::M1 => "1m",
        Timeframe::M5 => "5m",
        Timeframe::M15 => "15m",
        Timeframe::H1 => "1h",
        Timeframe::H4 => "4h",
        Timeframe::D1 => "1d",
        _ => return Err(AdapterError::InvalidRequest("Unsupported timeframe".into())),
    };

    // Determine time range
    let (start_time, end_time) = match range {
        Some((start, end)) => (start, end),
        None => {
            let end = chrono::Utc::now().timestamp_millis() as u64;
            let start = end - (timeframe.to_milliseconds() * 500);
            (start, end)
        }
    };

    // Build URL with query parameters
    let url = format!(
        "{}/api/v1/klines?symbol={}&interval={}&startTime={}&endTime={}",
        API_DOMAIN,
        ticker_info.ticker.symbol,
        interval,
        start_time,
        end_time
    );

    let response = http_request_with_limiter(&url, &ASTER_LIMITER, 1, None, None).await?;
    let data: Vec<AsterKline> = serde_json::from_str(&response)
        .map_err(|e| AdapterError::ParseError(e.to_string()))?;

    // Convert to Kline structs
    let mut klines = Vec::new();
    let size_in_quote = SIZE_IN_QUOTE_CURRENCY.get() == Some(&true);

    for k in data {
        let volume = if size_in_quote {
            k.volume * k.close // Convert to quote currency
        } else {
            k.volume
        };

        let kline = Kline::new(
            k.time,
            k.open,
            k.high,
            k.low,
            k.close,
            (-1.0, volume), // (taker_buy_volume, total_volume)
            ticker_info.min_ticksize,
        );
        klines.push(kline);
    }

    Ok(klines)
}
```

**Key points**:
- Handle timeframe mapping to exchange format
- Handle time range (use default 500 candles if None)
- Handle volume currency conversion (`SIZE_IN_QUOTE_CURRENCY` flag)
- Round prices with min_ticksize

**Reference**: `exchange/src/adapter/hyperliquid.rs::fetch_klines`

### 3.4 Implement `fetch_historical_oi` (Optional)

**Only if the exchange supports open interest for perpetuals.**

**Function signature**:
```rust
pub async fn fetch_historical_oi(
    ticker: Ticker,
    range: Option<(u64, u64)>,
    timeframe: Timeframe,
) -> Result<Vec<OpenInterest>, AdapterError>
```

**Implementation pattern**:

```rust
pub async fn fetch_historical_oi(
    ticker: Ticker,
    range: Option<(u64, u64)>,
    timeframe: Timeframe,
) -> Result<Vec<OpenInterest>, AdapterError> {
    // Check market type
    if !matches!(ticker.exchange.market_type(), MarketKind::LinearPerps | MarketKind::InversePerps) {
        return Err(AdapterError::InvalidRequest("OI only available for perps".into()));
    }

    // Map timeframe
    let interval = match timeframe {
        Timeframe::M5 => "5m",
        Timeframe::M15 => "15m",
        Timeframe::H1 => "1h",
        _ => return Err(AdapterError::InvalidRequest("Unsupported OI timeframe".into())),
    };

    // Determine time range (default to last 500 points)
    let (start_time, end_time) = match range {
        Some((start, end)) => (start, end),
        None => {
            let end = chrono::Utc::now().timestamp_millis() as u64;
            let start = end - (timeframe.to_milliseconds() * 500);
            (start, end)
        }
    };

    let url = format!(
        "{}/fapi/v1/openInterest/hist?symbol={}&period={}&startTime={}&endTime={}",
        API_DOMAIN,
        ticker.symbol,
        interval,
        start_time,
        end_time
    );

    let response = http_request_with_limiter(&url, &ASTER_LIMITER, 1, None, None).await?;

    #[derive(Deserialize)]
    struct OIData {
        #[serde(deserialize_with = "de_string_to_u64")]
        timestamp: u64,
        #[serde(deserialize_with = "de_string_to_f32")]
        open_interest: f32,
    }

    let data: Vec<OIData> = serde_json::from_str(&response)
        .map_err(|e| AdapterError::ParseError(e.to_string()))?;

    let result = data
        .into_iter()
        .map(|oi| OpenInterest {
            time: oi.timestamp,
            value: oi.open_interest,
        })
        .collect();

    Ok(result)
}
```

**Reference**: `exchange/src/adapter/okex.rs::fetch_historical_oi`

---

## Phase 4: WebSocket Implementation

### 4.1 WebSocket Connection Infrastructure

**Helper functions**:

```rust
// Internal stream data enum
enum StreamData {
    Trade(Vec<AsterTrade>),
    Depth(AsterDepth),
    Kline(AsterKline),
}

// Connect to WebSocket
async fn connect_websocket(
    path: &str,
) -> Result<fastwebsockets::FragmentCollector<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>>, AdapterError> {
    connect_ws(WS_DOMAIN, path)
        .await
        .map_err(|e| AdapterError::WebsocketError(e.to_string()))
}

// Parse WebSocket message
fn parse_websocket_message(payload: &[u8]) -> Result<StreamData, AdapterError> {
    let msg: AsterWSMessage = serde_json::from_slice(payload)
        .map_err(|e| AdapterError::ParseError(e.to_string()))?;

    // Identify stream type from 'stream' field
    if msg.stream.contains("depth") {
        let depth: AsterDepth = serde_json::from_value(msg.data)
            .map_err(|e| AdapterError::ParseError(e.to_string()))?;
        Ok(StreamData::Depth(depth))
    } else if msg.stream.contains("trade") {
        let trades: Vec<AsterTrade> = serde_json::from_value(msg.data)
            .map_err(|e| AdapterError::ParseError(e.to_string()))?;
        Ok(StreamData::Trade(trades))
    } else if msg.stream.contains("kline") {
        let kline: AsterKline = serde_json::from_value(msg.data)
            .map_err(|e| AdapterError::ParseError(e.to_string()))?;
        Ok(StreamData::Kline(kline))
    } else {
        Err(AdapterError::ParseError(format!("Unknown stream: {}", msg.stream)))
    }
}
```

**Reference**: `exchange/src/adapter/hyperliquid.rs::connect_websocket` and `parse_websocket_message`

### 4.2 Implement `connect_market_stream`

**Function signature**:
```rust
pub fn connect_market_stream(
    ticker_info: TickerInfo,
    tick_multiplier: Option<TickMultiplier>,
    push_freq: PushFrequency,
) -> impl Stream<Item = Event>
```

**Implementation pattern** (state machine):

```rust
pub fn connect_market_stream(
    ticker_info: TickerInfo,
    tick_multiplier: Option<TickMultiplier>,
    push_freq: PushFrequency,
) -> impl Stream<Item = Event> {
    stream::channel(100, |mut output| async move {
        let mut state = State::Disconnected;
        let mut orderbook = LocalDepthCache::default();
        let mut trades_buffer = Vec::new();
        let exchange = ticker_info.ticker.exchange;
        let stream_kind = StreamKind::Market(ticker_info.clone(), tick_multiplier);

        loop {
            match &mut state {
                State::Disconnected => {
                    // Build WebSocket path with subscriptions
                    let path = format!(
                        "/ws/{}@depth/{}@trade",
                        ticker_info.ticker.symbol.to_lowercase(),
                        ticker_info.ticker.symbol.to_lowercase()
                    );

                    // Connect
                    match connect_websocket(&path).await {
                        Ok(ws) => {
                            orderbook = LocalDepthCache::default();
                            trades_buffer.clear();
                            state = State::Connected(ws);
                            let _ = output.send(Event::Connected(exchange)).await;
                        }
                        Err(e) => {
                            let _ = output.send(Event::Disconnected(exchange, e.to_string())).await;
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                    }
                }

                State::Connected(ws) => {
                    // Read WebSocket frame
                    match tokio::time::timeout(Duration::from_secs(30), ws.read_frame()).await {
                        Ok(Ok(frame)) => {
                            match frame.opcode {
                                OpCode::Text => {
                                    // Parse message
                                    match parse_websocket_message(frame.payload) {
                                        Ok(StreamData::Depth(depth)) => {
                                            // Update orderbook
                                            let depth_update = convert_depth_to_update(depth, &ticker_info);
                                            orderbook.update(depth_update, ticker_info.min_ticksize);

                                            // Get current orderbook snapshot
                                            let current_depth = orderbook.get_depth();

                                            // Consume trades buffer
                                            let trades: Box<[Trade]> = trades_buffer.drain(..).collect();

                                            // Send event
                                            let _ = output.send(Event::DepthReceived(
                                                stream_kind.clone(),
                                                chrono::Utc::now().timestamp_millis() as u64,
                                                current_depth,
                                                trades,
                                            )).await;
                                        }
                                        Ok(StreamData::Trade(aster_trades)) => {
                                            // Convert and buffer trades
                                            for t in aster_trades {
                                                let trade = Trade {
                                                    price: t.price,
                                                    qty: t.qty,
                                                    side: if t.is_buyer_maker { "sell" } else { "buy" }.into(),
                                                    time: t.timestamp,
                                                };
                                                trades_buffer.push(trade);
                                            }
                                        }
                                        Ok(StreamData::Kline(_)) => {
                                            // Ignore klines in market stream
                                        }
                                        Err(e) => {
                                            eprintln!("Parse error: {}", e);
                                        }
                                    }
                                }
                                OpCode::Close => {
                                    let _ = output.send(Event::Disconnected(exchange, "WebSocket closed".into())).await;
                                    state = State::Disconnected;
                                }
                                OpCode::Ping => {
                                    let pong = Frame::pong(frame.payload);
                                    let _ = ws.write_frame(pong).await;
                                }
                                _ => {}
                            }
                        }
                        Ok(Err(e)) => {
                            let _ = output.send(Event::Disconnected(exchange, e.to_string())).await;
                            state = State::Disconnected;
                        }
                        Err(_) => {
                            let _ = output.send(Event::Disconnected(exchange, "Timeout".into())).await;
                            state = State::Disconnected;
                        }
                    }
                }
            }
        }
    })
}

// Helper function to convert exchange depth format to internal format
fn convert_depth_to_update(depth: AsterDepth, ticker_info: &TickerInfo) -> DepthUpdate {
    let bids: Vec<DeOrder> = depth.bids
        .into_iter()
        .map(|(price, qty)| {
            let p = price.parse::<f32>().unwrap_or(0.0);
            let q = qty.parse::<f32>().unwrap_or(0.0);
            DeOrder {
                price: Price::from_f32(p).round_to_min_tick(ticker_info.min_ticksize),
                qty: q,
            }
        })
        .collect();

    let asks: Vec<DeOrder> = depth.asks
        .into_iter()
        .map(|(price, qty)| {
            let p = price.parse::<f32>().unwrap_or(0.0);
            let q = qty.parse::<f32>().unwrap_or(0.0);
            DeOrder {
                price: Price::from_f32(p).round_to_min_tick(ticker_info.min_ticksize),
                qty: q,
            }
        })
        .collect();

    // Determine if this is a snapshot or incremental update
    // (depends on exchange - some send snapshots, others incremental)
    DepthUpdate::Snapshot(DepthPayload {
        bids,
        asks,
        first_id: None,
        final_id: None,
    })
}
```

**Key considerations**:
- Start in `State::Disconnected`
- Handle reconnection on errors
- Buffer trades until depth update
- Use `LocalDepthCache::update()` for orderbook management
- Round prices with `Price::from_f32().round_to_min_tick()`
- Handle ping/pong for heartbeat
- Handle `SIZE_IN_QUOTE_CURRENCY` flag for volume conversion

**Reference**: `exchange/src/adapter/hyperliquid.rs::connect_market_stream`

### 4.3 Implement `connect_kline_stream`

**Function signature**:
```rust
pub fn connect_kline_stream(
    streams: Vec<(TickerInfo, Timeframe)>,
    market: MarketKind,
) -> impl Stream<Item = Event>
```

**Implementation pattern**:

```rust
pub fn connect_kline_stream(
    streams: Vec<(TickerInfo, Timeframe)>,
    market: MarketKind,
) -> impl Stream<Item = Event> {
    stream::channel(100, |mut output| async move {
        let exchange = match market {
            MarketKind::Spot => Exchange::AsterSpot,
            MarketKind::LinearPerps => Exchange::AsterLinear,
            _ => return,
        };

        let mut state = State::Disconnected;

        // Build subscription path for multiple streams
        let mut stream_paths = Vec::new();
        for (ticker_info, timeframe) in &streams {
            let interval = match timeframe {
                Timeframe::M1 => "1m",
                Timeframe::M5 => "5m",
                Timeframe::M15 => "15m",
                Timeframe::H1 => "1h",
                _ => continue,
            };
            stream_paths.push(format!(
                "{}@kline_{}",
                ticker_info.ticker.symbol.to_lowercase(),
                interval
            ));
        }
        let path = format!("/ws/{}", stream_paths.join("/"));

        loop {
            match &mut state {
                State::Disconnected => {
                    match connect_websocket(&path).await {
                        Ok(ws) => {
                            state = State::Connected(ws);
                            let _ = output.send(Event::Connected(exchange)).await;
                        }
                        Err(e) => {
                            let _ = output.send(Event::Disconnected(exchange, e.to_string())).await;
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                    }
                }

                State::Connected(ws) => {
                    match tokio::time::timeout(Duration::from_secs(30), ws.read_frame()).await {
                        Ok(Ok(frame)) => {
                            match frame.opcode {
                                OpCode::Text => {
                                    match parse_websocket_message(frame.payload) {
                                        Ok(StreamData::Kline(k)) => {
                                            // Find matching ticker_info from stream name
                                            // (you'll need to parse symbol from the stream field)
                                            for (ticker_info, timeframe) in &streams {
                                                // Match logic here...
                                                let size_in_quote = SIZE_IN_QUOTE_CURRENCY.get() == Some(&true);
                                                let volume = if size_in_quote {
                                                    k.volume * k.close
                                                } else {
                                                    k.volume
                                                };

                                                let kline = Kline::new(
                                                    k.time,
                                                    k.open,
                                                    k.high,
                                                    k.low,
                                                    k.close,
                                                    (-1.0, volume),
                                                    ticker_info.min_ticksize,
                                                );

                                                let stream_kind = StreamKind::Kline(
                                                    ticker_info.clone(),
                                                    *timeframe,
                                                );

                                                let _ = output.send(Event::KlineReceived(
                                                    stream_kind,
                                                    kline,
                                                )).await;
                                                break;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                OpCode::Close => {
                                    let _ = output.send(Event::Disconnected(exchange, "Closed".into())).await;
                                    state = State::Disconnected;
                                }
                                OpCode::Ping => {
                                    let _ = ws.write_frame(Frame::pong(frame.payload)).await;
                                }
                                _ => {}
                            }
                        }
                        Ok(Err(e)) => {
                            let _ = output.send(Event::Disconnected(exchange, e.to_string())).await;
                            state = State::Disconnected;
                        }
                        Err(_) => {
                            let _ = output.send(Event::Disconnected(exchange, "Timeout".into())).await;
                            state = State::Disconnected;
                        }
                    }
                }
            }
        }
    })
}
```

**Key points**:
- Support multiple (ticker, timeframe) pairs
- Subscribe to all channels at once
- Match received kline to correct ticker_info
- Handle volume conversion (`SIZE_IN_QUOTE_CURRENCY`)
- Emit `Event::KlineReceived`

**Reference**: `exchange/src/adapter/hyperliquid.rs::connect_kline_stream`

### 4.4 Server-Side Depth Aggregation (If Supported)

**Only if the exchange supports server-side depth aggregation.**

If the exchange allows specifying orderbook aggregation/precision levels in the WebSocket subscription:

```rust
// Configuration type for depth aggregation
#[derive(Debug, Clone, Copy)]
enum DepthConfig {
    Precision0,
    Precision1,
    Precision2,
    // ... etc
}

// Map tick multiplier to exchange config
fn config_from_multiplier(price: f32, multiplier: u16) -> DepthConfig {
    // Logic depends on exchange API
    // Example: smaller multipliers = higher precision
    match multiplier {
        1 => DepthConfig::Precision0,
        5 => DepthConfig::Precision1,
        10 => DepthConfig::Precision2,
        _ => DepthConfig::Precision0,
    }
}

// Calculate expected tick size from config
fn depth_tick_from_config(price: f32, config: DepthConfig) -> f32 {
    // Exchange-specific calculation
    match config {
        DepthConfig::Precision0 => 0.01,
        DepthConfig::Precision1 => 0.1,
        DepthConfig::Precision2 => 1.0,
    }
}
```

Then in the subscription, include the precision parameter:

```rust
// In connect_market_stream, when building the path:
let config = tick_multiplier.map(|m| config_from_multiplier(ticker_info.last_price, m.0));
let precision_param = match config {
    Some(DepthConfig::Precision0) => "@depth@0",
    Some(DepthConfig::Precision1) => "@depth@1",
    _ => "@depth",
};

let path = format!(
    "/ws/{}{}",
    ticker_info.ticker.symbol.to_lowercase(),
    precision_param
);
```

**If NOT supported**: Set `is_depth_client_aggr()` to `true` in the `Exchange` enum configuration. This will use client-side aggregation via `LocalDepthCache`.

**Reference**: `exchange/src/adapter/hyperliquid.rs` lines 374-531

---

## Phase 5: Integration & Testing

### 5.1 Wire Up Dispatch Functions

**File**: `exchange/src/adapter.rs`

Update the central dispatch functions to route to your new implementation:

**1. fetch_ticker_info** (around line 595):
```rust
pub async fn fetch_ticker_info(
    exchange: Exchange,
) -> Result<HashMap<Ticker, Option<TickerInfo>>, AdapterError> {
    let market_type = exchange.market_type();
    match exchange {
        // ... existing cases
        Exchange::AsterLinear | Exchange::AsterSpot => {
            aster::fetch_ticksize(market_type).await
        }
    }
}
```

**2. fetch_ticker_prices** (around line 616):
```rust
pub async fn fetch_ticker_prices(
    exchange: Exchange,
) -> Result<HashMap<Ticker, TickerStats>, AdapterError> {
    let market_type = exchange.market_type();
    match exchange {
        // ... existing cases
        Exchange::AsterLinear | Exchange::AsterSpot => {
            aster::fetch_ticker_prices(market_type).await
        }
    }
}
```

**3. fetch_klines** (around line 637):
```rust
pub async fn fetch_klines(
    ticker_info: TickerInfo,
    timeframe: Timeframe,
    range: Option<(u64, u64)>,
) -> Result<Vec<Kline>, AdapterError> {
    match ticker_info.ticker.exchange {
        // ... existing cases
        Exchange::AsterLinear | Exchange::AsterSpot => {
            aster::fetch_klines(ticker_info, timeframe, range).await
        }
    }
}
```

**4. fetch_open_interest** (around line 658) - if supported:
```rust
pub async fn fetch_open_interest(
    ticker: Ticker,
    range: Option<(u64, u64)>,
    timeframe: Timeframe,
) -> Result<Vec<OpenInterest>, AdapterError> {
    match ticker.exchange {
        // ... existing cases
        Exchange::AsterLinear => {
            aster::fetch_historical_oi(ticker, range, timeframe).await
        }
        _ => Err(AdapterError::InvalidRequest("OI not supported".into())),
    }
}
```

**5. connect_market_stream** (find the appropriate function):
```rust
pub fn connect_market_stream(
    ticker_info: TickerInfo,
    tick_multiplier: Option<TickMultiplier>,
    push_freq: PushFrequency,
) -> impl Stream<Item = Event> {
    match ticker_info.ticker.exchange {
        // ... existing cases
        Exchange::AsterLinear | Exchange::AsterSpot => {
            aster::connect_market_stream(ticker_info, tick_multiplier, push_freq)
        }
    }
}
```

**6. connect_kline_stream**:
```rust
pub fn connect_kline_stream(
    streams: Vec<(TickerInfo, Timeframe)>,
    market: MarketKind,
) -> impl Stream<Item = Event> {
    if streams.is_empty() {
        return stream::channel(1, |_| async {});
    }

    let exchange = streams[0].0.ticker.exchange;
    match exchange {
        // ... existing cases
        Exchange::AsterLinear | Exchange::AsterSpot => {
            aster::connect_kline_stream(streams, market)
        }
    }
}
```

### 5.2 Create Integration Tests

**File**: `exchange/src/adapter/aster.rs`

Add a test module at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_ticksize_spot() {
        let result = fetch_ticksize(MarketKind::Spot).await;
        assert!(result.is_ok());
        let tickers = result.unwrap();
        assert!(!tickers.is_empty());

        // Print first few tickers for verification
        for (ticker, info) in tickers.iter().take(3) {
            println!("{:?}: {:?}", ticker, info);
        }
    }

    #[tokio::test]
    async fn test_fetch_ticksize_perps() {
        let result = fetch_ticksize(MarketKind::LinearPerps).await;
        assert!(result.is_ok());
        let tickers = result.unwrap();
        assert!(!tickers.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_ticker_prices() {
        let result = fetch_ticker_prices(MarketKind::Spot).await;
        assert!(result.is_ok());
        let prices = result.unwrap();
        assert!(!prices.is_empty());

        // Verify data structure
        for (ticker, stats) in prices.iter().take(3) {
            println!("{:?}: price={}, chg={}%, vol={}",
                ticker, stats.mark_price, stats.daily_price_chg, stats.daily_volume);
        }
    }

    #[tokio::test]
    async fn test_fetch_klines() {
        // First get a ticker
        let tickers = fetch_ticksize(MarketKind::Spot).await.unwrap();
        let (_, info) = tickers.iter().next().unwrap();
        let ticker_info = info.clone().unwrap();

        let result = fetch_klines(ticker_info, Timeframe::M1, None).await;
        assert!(result.is_ok());
        let klines = result.unwrap();
        assert!(!klines.is_empty());
        println!("Fetched {} klines", klines.len());

        // Verify kline data
        for kline in klines.iter().take(3) {
            println!("{:?}", kline);
        }
    }

    #[tokio::test]
    #[ignore] // Run manually
    async fn manual_websocket_test() {
        use iced_futures::futures::StreamExt;

        // Get ticker info
        let tickers = fetch_ticksize(MarketKind::Spot).await.unwrap();
        let (_, ticker_info) = tickers.iter().next().unwrap();
        let ticker_info = ticker_info.clone().unwrap();

        println!("Testing WebSocket for {:?}", ticker_info.ticker);

        // Create market stream
        let mut stream = connect_market_stream(
            ticker_info,
            None,
            PushFrequency::ServerDefault,
        );

        // Receive first few events
        for i in 0..10 {
            if let Some(event) = stream.next().await {
                println!("Event {}: {:?}", i, event);
            }
        }
    }
}
```

**Run tests**:
```bash
cd exchange
cargo test aster::tests --nocapture
```

**Run manual WebSocket test**:
```bash
cd exchange
cargo test aster::tests::manual_websocket_test --nocapture --ignored
```

**Verify**:
- ✅ Successful REST API calls
- ✅ Proper data parsing
- ✅ WebSocket connection succeeds
- ✅ Depth updates arrive
- ✅ Trades arrive
- ✅ Correct data formats

### 5.3 Build and Verify Compilation

```bash
cd exchange
cargo build
```

Fix any compilation errors. Common issues:
- Missing trait implementations
- Type mismatches
- Lifetime issues
- Import errors

---

## Phase 6: Frontend Integration

### 6.1 Update UI Exchange List

**Files to check**:
- `src/screen/dashboard.rs`
- `src/screen/dashboard/tickers_table.rs`

**Verification**:
- Launch the application
- Check that Aster appears in the exchange dropdown
- Verify the exchange label displays correctly
- Test selecting Aster from the dropdown

The `Exchange::ALL` array inclusion should automatically add it to the UI.

### 6.2 Stream Configuration UI

**File**: `src/modal/pane/stream.rs`

**Verify**:

1. **Depth Aggregation**:
   - If server-side supported: UI shows tick multiplier options
   - If client-side: UI shows client-side aggregation indicator
   - Check that `exchange.is_depth_client_aggr()` is used correctly

2. **Push Frequency**:
   - Verify `exchange.is_custom_push_freq()` returns correct value
   - Check that `exchange.allowed_push_freqs()` provides correct options
   - Test UI displays appropriate frequency options

3. **Timeframe Support**:
   - Verify `exchange.supports_heatmap_timeframe()` returns correct values
   - Test that unsupported timeframes are disabled in UI

### 6.3 End-to-End Test

**Manual testing checklist**:

**1. Ticker Selection**:
- [ ] Select Aster in exchange dropdown
- [ ] Verify tickers load correctly
- [ ] Check tick sizes display properly
- [ ] Verify ticker search works

**2. Stream Creation - Depth & Trades**:
- [ ] Create a depth+trades stream for a ticker
- [ ] Verify WebSocket connects (check status indicator)
- [ ] Check depth visualization updates in real-time
- [ ] Verify trades appear in time & sales panel
- [ ] Test bid/ask spread displays correctly
- [ ] Check orderbook levels update

**3. Kline Stream**:
- [ ] Create a kline stream
- [ ] Check candlestick chart renders
- [ ] Verify chart updates with new candles
- [ ] Test different timeframes (M1, M5, H1, etc.)
- [ ] Check volume bars display

**4. Performance**:
- [ ] Monitor memory usage (should be stable)
- [ ] Check CPU usage (should be reasonable)
- [ ] Verify reconnection works after disconnect
- [ ] Test handling of network interruptions
- [ ] Check for memory leaks (run for extended period)

**5. Error Handling**:
- [ ] Test with invalid ticker
- [ ] Test with unsupported timeframe
- [ ] Disconnect network and verify error handling
- [ ] Reconnect network and verify recovery

**Document any issues found**.

---

## Phase 7: Documentation & Cleanup

### 7.1 Add Provider Documentation

**File**: `exchange/src/adapter/aster.rs`

Add comprehensive module documentation at the top:

```rust
//! Aster DEX Exchange Adapter
//!
//! This module provides integration with the Aster DEX API.
//!
//! ## Supported Features
//! - ✅ Spot markets
//! - ✅ Linear perpetuals (USDT-margined)
//! - ❌ Inverse perpetuals
//! - ❌ Open interest data
//!
//! ## API Details
//! - REST API: https://api.asterdex.com
//! - WebSocket: wss://stream.asterdex.com
//! - API Documentation: https://docs.asterdex.com/api
//! - Rate Limits: 1200 requests per minute (with 5% buffer)
//!
//! ## Depth Aggregation
//! - Server-side: No
//! - Client-side: Yes (via LocalDepthCache)
//! - Custom push frequency: No (uses server default)
//!
//! ## WebSocket Streams
//! - Depth updates: Snapshot-based, updates every 100ms
//! - Trade stream: Real-time individual trades
//! - Kline stream: Updates on candle close + real-time current candle
//!
//! ## Known Limitations
//! - No historical open interest data available
//! - Inverse perpetuals not supported
//! - Maximum 500 historical klines per request
//! - No sub-second kline timeframes
//!
//! ## Implementation Notes
//! - Uses FixedWindowBucket rate limiter
//! - Volumes are in quote currency (USDT) for perps, base currency for spot
//! - Timestamps are in milliseconds UTC
//! - Price precision varies by symbol (use tick_size from symbol info)
```

### 7.2 Update Main Documentation

**README.md**:

Find the "Supported Exchanges" section and add:

```markdown
## Supported Exchanges

- Binance (Spot, Linear Perps, Inverse Perps)
- Bybit (Spot, Linear Perps, Inverse Perps)
- Hyperliquid (Spot, Linear Perps)
- OKX (Spot, Linear Perps, Inverse Perps)
- **Aster DEX (Spot, Linear Perps)** ← NEW

### Feature Support Matrix

| Exchange | Spot | Linear Perps | Inverse Perps | Open Interest | Server-side Depth Aggregation |
|----------|------|--------------|---------------|---------------|-------------------------------|
| Binance  | ✅   | ✅           | ✅            | ✅            | ❌                            |
| Bybit    | ✅   | ✅           | ✅            | ✅            | ✅                            |
| Hyperliquid | ✅ | ✅         | ❌            | ❌            | ✅                            |
| OKX      | ✅   | ✅           | ✅            | ✅            | ❌                            |
| Aster DEX | ✅   | ✅          | ❌            | ❌            | ❌                            |
```

**CHANGELOG** (if exists):

```markdown
## [Unreleased]

### Added
- Aster DEX exchange support
  - Spot markets
  - Linear perpetuals (USDT-margined)
  - Real-time depth and trade streams
  - Historical kline data
  - Client-side depth aggregation
```

### 7.3 Code Review Checklist

**Type Safety**:
- [ ] All serde structs properly deserialize test responses
- [ ] Error handling covers all edge cases
- [ ] No `unwrap()` calls in production code paths
- [ ] All `Result` types properly propagated
- [ ] Proper use of `Option` vs `Result`

**Performance**:
- [ ] Rate limiter configured correctly with appropriate limits
- [ ] No unnecessary allocations in hot paths (WebSocket message processing)
- [ ] WebSocket message parsing is efficient (consider using `sonic_rs` if needed)
- [ ] LocalDepthCache updates are efficient
- [ ] No blocking operations in async functions

**Correctness**:
- [ ] Tick sizes calculated correctly
- [ ] Volume conversions handle `SIZE_IN_QUOTE_CURRENCY` flag
- [ ] Price rounding uses correct `min_ticksize`
- [ ] Timestamps are in milliseconds (UTC)
- [ ] Trade side mapping is correct (buy/sell, maker/taker)
- [ ] Orderbook bid/ask ordering is correct
- [ ] Kline OHLCV data is correctly mapped

**Consistency**:
- [ ] Follows same patterns as other adapters (especially Hyperliquid)
- [ ] Error types match `AdapterError` enum
- [ ] Event emissions match expected format
- [ ] Function signatures match trait/interface expectations
- [ ] Naming conventions consistent with codebase

**Testing**:
- [ ] Unit tests pass: `cargo test aster::tests`
- [ ] Integration tests pass
- [ ] Manual WebSocket test successful
- [ ] End-to-end UI test successful
- [ ] Tested with multiple tickers
- [ ] Tested reconnection logic
- [ ] Tested error scenarios

**Documentation**:
- [ ] Module-level documentation complete
- [ ] Complex functions have doc comments
- [ ] README updated
- [ ] CHANGELOG updated (if exists)
- [ ] Code comments explain non-obvious logic

**Security**:
- [ ] No hardcoded credentials
- [ ] Proper TLS for WebSocket connections
- [ ] Input validation on external data
- [ ] No arbitrary code execution vulnerabilities

---

## Troubleshooting

### Rate Limiting Errors

**Symptoms**:
- 429 or 418 status codes
- Requests failing with rate limit errors
- `AdapterError::FetchError` with rate limit message

**Solutions**:
1. Increase `LIMITER_BUFFER_PCT` (e.g., from 0.05 to 0.10)
2. Check response headers for actual usage
3. Consider using `DynamicBucket` if exchange provides usage headers
4. Reduce request frequency in tests
5. Add delays between requests in loops

**Debugging**:
```rust
// Add logging to rate limiter
impl RateLimiter for AsterLimiter {
    fn prepare_request(&mut self, weight: usize) -> Option<Duration> {
        let delay = self.bucket.prepare_request(weight);
        if let Some(d) = delay {
            eprintln!("Rate limit: waiting {:?} before request", d);
        }
        delay
    }
}
```

### WebSocket Disconnects

**Symptoms**:
- Constant reconnections
- `Event::Disconnected` events frequently
- Unstable streams
- Timeout errors

**Solutions**:
1. Verify ping/pong handling (respond to `OpCode::Ping` with `OpCode::Pong`)
2. Check subscription message format matches exchange API exactly
3. Add debug logging to see disconnect reasons:
   ```rust
   Err(e) => {
       eprintln!("WebSocket error: {:?}", e);
       let _ = output.send(Event::Disconnected(exchange, e.to_string())).await;
   }
   ```
4. Increase timeout duration if network is slow
5. Check if exchange requires periodic ping messages
6. Verify WebSocket URL is correct (wss://, correct domain, correct path)

**Debugging**:
```rust
// Log all WebSocket frames
match ws.read_frame().await {
    Ok(frame) => {
        eprintln!("Received frame: opcode={:?}, len={}", frame.opcode, frame.payload.len());
        // ... process frame
    }
}
```

### Incorrect Tick Sizes

**Symptoms**:
- Orders would be rejected (if trading enabled)
- Precision errors in displayed prices
- Orderbook levels not aligning correctly
- Price rounding issues

**Solutions**:
1. Verify tick size calculation logic in `fetch_ticksize`
2. Check decimal precision handling
3. Compare with exchange's instrument info endpoint
4. Ensure `Price::from_f32().round_to_min_tick()` is used consistently
5. Check for different tick sizes at different price levels

**Debugging**:
```rust
// Log tick sizes
for (ticker, info) in tickers.iter().take(5) {
    eprintln!("Ticker: {:?}, tick_size: {}, min_qty: {}",
        ticker, info.as_ref().map(|i| i.min_ticksize), info.as_ref().map(|i| i.min_qty));
}
```

### Depth Updates Not Syncing

**Symptoms**:
- Orderbook shows stale data
- Bids/asks not updating
- Orderbook levels incorrect
- Ghost orders (orders that should be removed but persist)

**Solutions**:
1. Verify sequence ID handling (`first_id`, `final_id`) if exchange uses them
2. Check `LocalDepthCache` update logic
3. Ensure snapshot is fetched before processing incremental updates
4. Verify `DepthUpdate::Snapshot` vs `DepthUpdate::Diff` usage
5. Check that price rounding is consistent

**Debugging**:
```rust
// Log depth updates
match parse_websocket_message(frame.payload) {
    Ok(StreamData::Depth(depth)) => {
        eprintln!("Depth update: {} bids, {} asks", depth.bids.len(), depth.asks.len());
        // Log first few levels
        for (i, (price, qty)) in depth.bids.iter().take(3).enumerate() {
            eprintln!("  Bid {}: {} @ {}", i, qty, price);
        }
    }
}
```

### Volume Display Incorrect

**Symptoms**:
- Volume values too high or too low
- Volume units incorrect (base vs quote)
- Inconsistent volume across different views

**Solutions**:
1. Check `SIZE_IN_QUOTE_CURRENCY` handling:
   ```rust
   let size_in_quote = SIZE_IN_QUOTE_CURRENCY.get() == Some(&true);
   let volume = if size_in_quote {
       raw_volume * close_price  // Convert to quote
   } else {
       raw_volume  // Already in base
   };
   ```
2. Verify base vs quote currency conversion
3. Confirm exchange returns volume in expected currency
4. Check API documentation for volume units
5. Compare with exchange's web interface

### Parse Errors

**Symptoms**:
- `AdapterError::ParseError`
- Deserialization failures
- Missing field errors
- Type conversion errors

**Solutions**:
1. Log raw JSON before parsing:
   ```rust
   eprintln!("Raw JSON: {}", std::str::from_utf8(frame.payload).unwrap());
   ```
2. Verify serde struct field names match API response
3. Check `#[serde(rename = "...")]` attributes
4. Ensure custom deserializers (`de_string_to_f32`) are used correctly
5. Handle optional fields with `Option<T>`
6. Use `#[serde(default)]` for fields that may be missing

**Debugging**:
```rust
// Try parsing with better error messages
match serde_json::from_slice::<AsterDepth>(payload) {
    Ok(depth) => { /* ... */ }
    Err(e) => {
        eprintln!("Parse error: {}", e);
        eprintln!("Raw payload: {}", std::str::from_utf8(payload).unwrap_or("<invalid utf8>"));
    }
}
```

### Memory Leaks

**Symptoms**:
- Memory usage grows over time
- Application slows down after running for hours
- Out of memory errors

**Solutions**:
1. Ensure `trades_buffer` is properly cleared after sending:
   ```rust
   let trades: Box<[Trade]> = trades_buffer.drain(..).collect();
   ```
2. Check that `LocalDepthCache` doesn't accumulate stale levels
3. Verify no unbounded `Vec` growth
4. Use `cargo flamegraph` to profile memory usage
5. Check for circular references in `Rc` or `Arc`

### Compilation Errors

**Common issues**:

1. **Lifetime errors**:
   - Ensure references in structs have appropriate lifetimes
   - Use `'static` for string constants
   - Clone data when needed for async blocks

2. **Type mismatches**:
   - Verify function signatures match trait requirements
   - Check `impl Stream<Item = Event>` return types
   - Ensure `AdapterError` is used consistently

3. **Import errors**:
   - Verify all required crates are in `Cargo.toml`
   - Check module visibility (`pub` keywords)
   - Ensure `use` statements are correct

4. **Async/await issues**:
   - All async functions must be `.await`ed
   - Use `tokio::spawn` for background tasks
   - Ensure `#[tokio::test]` for async tests

---

## Quick Reference: File Locations

| Component | File Path | Purpose |
|-----------|-----------|---------|
| Exchange enum | `exchange/src/adapter.rs` | Add exchange variants |
| Provider module | `exchange/src/adapter/aster.rs` | Main implementation |
| Rate limiter | `exchange/src/limiter.rs` | Rate limiting utilities |
| WebSocket utils | `exchange/src/connect.rs` | WebSocket connection |
| Depth management | `exchange/src/depth.rs` | Orderbook cache |
| Core types | `exchange/src/lib.rs` | Ticker, Kline, Trade, etc. |
| Dispatch functions | `exchange/src/adapter.rs` | Central routing |
| UI integration | `src/screen/dashboard.rs` | Exchange selection |
| Stream config | `src/modal/pane/stream.rs` | Stream settings |

---

## Implementation Checklist for Aster DEX

### Phase 1: Research ✅
- [ ] Locate Aster DEX API documentation
- [ ] Document REST endpoints
- [ ] Document WebSocket streams
- [ ] Analyze rate limiting
- [ ] Collect example JSON responses

### Phase 2: Setup ✅
- [ ] Add `AsterLinear` and `AsterSpot` to `Exchange` enum
- [ ] Update `Exchange::ALL` array
- [ ] Add Display/FromStr implementations
- [ ] Configure market types, depth aggregation, timeframes
- [ ] Update `ExchangeInclusive` enum
- [ ] Create `exchange/src/adapter/aster.rs`
- [ ] Define constants (API_DOMAIN, WS_DOMAIN, rate limits)
- [ ] Implement rate limiter
- [ ] Define all serde structs
- [ ] Add module declaration in `adapter.rs`

### Phase 3: REST API ✅
- [ ] Implement `fetch_ticksize`
- [ ] Implement `fetch_ticker_prices`
- [ ] Implement `fetch_klines`
- [ ] Implement `fetch_historical_oi` (if supported)
- [ ] Test each function with `cargo test`

### Phase 4: WebSocket ✅
- [ ] Implement `connect_websocket` helper
- [ ] Implement `parse_websocket_message` helper
- [ ] Implement `connect_market_stream`
- [ ] Implement `connect_kline_stream`
- [ ] Implement depth aggregation (if supported)
- [ ] Test WebSocket connections manually

### Phase 5: Integration ✅
- [ ] Wire up `fetch_ticker_info` dispatch
- [ ] Wire up `fetch_ticker_prices` dispatch
- [ ] Wire up `fetch_klines` dispatch
- [ ] Wire up `fetch_open_interest` dispatch (if applicable)
- [ ] Wire up `connect_market_stream` dispatch
- [ ] Wire up `connect_kline_stream` dispatch
- [ ] Create integration tests
- [ ] Run `cargo build` and fix errors
- [ ] Run all tests

### Phase 6: Frontend ✅
- [ ] Verify exchange appears in UI dropdown
- [ ] Test ticker selection and loading
- [ ] Test depth+trades stream visualization
- [ ] Test kline stream visualization
- [ ] Test different timeframes
- [ ] Monitor performance (CPU, memory)
- [ ] Test error handling and reconnection

### Phase 7: Documentation ✅
- [ ] Add module-level documentation
- [ ] Document complex functions
- [ ] Update README.md
- [ ] Update CHANGELOG (if exists)
- [ ] Run code review checklist
- [ ] Fix any remaining issues

---

## Summary

This guide provides a complete roadmap for implementing a new exchange provider in Flowsurface. The key steps are:

1. **Research** the exchange API thoroughly
2. **Set up** type definitions and module structure
3. **Implement** REST API functions for data fetching
4. **Implement** WebSocket streams for real-time data
5. **Integrate** into the central dispatch system
6. **Test** thoroughly (unit, integration, E2E)
7. **Document** the implementation

By following this structured approach and using existing implementations (especially Hyperliquid) as references, you can successfully add support for any exchange to Flowsurface.

**Key architectural principles**:
- Unified event streaming model
- Consistent error handling
- Rate limiting for all API calls
- Local orderbook management
- Automatic reconnection logic
- Client-side or server-side depth aggregation

Good luck implementing Aster DEX support! 🚀
