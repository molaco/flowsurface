# FlowSurface Data Downloads - Complete Reference

## Binance Data Fetching

### 📊 **WebSocket Streams** (Real-Time)

#### 1. **Depth + Trades Stream**
- **URL Pattern**: `wss://{domain}/stream?streams={symbol}@aggTrade/{symbol}@depth@100ms`
- **Data**:
  - **Orderbook Depth**: Bids/Asks (BTreeMap<Price, Quantity>)
  - **Aggregate Trades**: Price, Qty, Time, Side (buffered between depth updates)
- **Update Frequency**: 100ms, 300ms, or 1000ms (configurable)
- **Used By**: Heatmap, Ladder, Kline/Footprint, Time & Sales

#### 2. **Kline Stream**
- **URL Pattern**: `wss://{domain}/stream?streams={symbol}@kline_{interval}`
- **Data**: OHLCV + Buy/Sell Volume
- **Used By**: Kline/Footprint charts (real-time candle updates)

---

### 🔌 **REST API Endpoints**

#### 3. **Klines (Candlesticks)**
- **Function**: `fetch_klines()`
- **Endpoint**:
  - Spot: `/api/v3/klines`
  - Linear: `/fapi/v1/klines`
  - Inverse: `/dapi/v1/klines`
- **Parameters**: symbol, interval, startTime, endTime, limit (max 1000)
- **Returns**: `Vec<Kline>` with OHLCV + buy/sell volume split
- **Weight**: 1-10 (perps), 2 (spot)

#### 4. **Orderbook Snapshot**
- **Function**: `fetch_depth()`
- **Endpoint**:
  - Spot: `/api/v3/depth`
  - Linear: `/fapi/v1/depth`
  - Inverse: `/dapi/v1/depth`
- **Parameters**: symbol, limit (1000-5000)
- **Returns**: Full orderbook snapshot with bids/asks
- **Weight**: 5-250 (spot), 2-20 (perps)
- **Used For**: WebSocket synchronization/resync

#### 5. **Ticker Information (Exchange Metadata)**
- **Function**: `fetch_ticksize()`
- **Endpoint**:
  - Spot: `/api/v3/exchangeInfo`
  - Linear: `/fapi/v1/exchangeInfo`
  - Inverse: `/dapi/v1/exchangeInfo`
- **Returns**: HashMap<Ticker, TickerInfo>
- **Data Extracted**:
  - `tickSize` (min price step)
  - `minQty` (min order quantity)
  - `contractSize` (futures multiplier)
  - Contract type filter (PERPETUAL only)
  - Quote asset filter (USDT/USD only)
  - Status filter (TRADING/HALT only)
- **Weight**: 1-20
- **Called**: Once at startup

#### 6. **Ticker Prices (24hr Stats)**
- **Function**: `fetch_ticker_prices()`
- **Endpoint**:
  - Spot: `/api/v3/ticker/24hr`
  - Linear: `/fapi/v1/ticker/24hr`
  - Inverse: `/dapi/v1/ticker/24hr`
- **Returns**: HashMap<Ticker, TickerStats>
- **Data**:
  - `lastPrice` (current mark price)
  - `priceChangePercent` (24h change %)
  - `quoteVolume`/`volume` (24h trading volume)
- **Weight**: 40-80
- **Update Interval**: 13s (visible) / 300s (hidden)

#### 7. **Open Interest (Perpetuals Only)**
- **Function**: `fetch_historical_oi()`
- **Endpoint**:
  - Linear: `/futures/data/openInterestHist`
  - Inverse: `/futures/data/openInterestHist`
- **Parameters**: symbol/pair, period (5m, 15m, 30m, 1h, etc), startTime, endTime, limit (max 500)
- **Returns**: `Vec<OpenInterest>` with timestamp + value
- **Limitation**: 30-day maximum history
- **Weight**: 1-12
- **NOT Available**: Spot markets, Hyperliquid

#### 8. **Historical Trades (aggTrades)**
- **Function**: `fetch_trades()` → `get_hist_trades()` + `fetch_intraday_trades()`
- **Data Sources**:

  **A. ZIP Archive Downloads** (Historical):
  - **URL**: `https://data.binance.vision/data/{spot|futures/um|futures/cm}/daily/aggTrades/{SYMBOL}/{SYMBOL}-aggTrades-YYYY-MM-DD.zip`
  - **Format**: CSV files with columns: agg_trade_id, price, quantity, first_trade_id, last_trade_id, timestamp, is_buyer_maker
  - **Cache Location**: `market_data/binance/data/futures/{um|cm}/daily/aggTrades/`
  - **Retention**: 4 days (auto-cleanup)
  - **Used For**: Footprint backfill

  **B. REST API** (Intraday):
  - **Endpoint**:
    - Spot: `/api/v3/aggTrades`
    - Linear: `/fapi/v1/aggTrades`
    - Inverse: `/dapi/v1/aggTrades`
  - **Parameters**: symbol, startTime, limit (max 1000)
  - **Weight**: 4 (spot), 20 (perps)
  - **Used For**: Today's trades (not in ZIP yet)

---

## 📦 **Data By Exchange - Comparison Table**

| Data Type | Binance | Bybit | Hyperliquid | OKX | Source Type | Notes |
|-----------|---------|-------|-------------|-----|-------------|-------|
| **Klines (OHLCV)** | ✅ | ✅ | ✅ | ✅ | REST API | All exchanges support |
| **Klines (Live)** | ✅ | ✅ | ✅ | ✅ | WebSocket | Real-time updates |
| **Historical Trades** | ✅ ZIP + API | ❌ | ❌ | 🚧 WIP | REST/ZIP | **Binance only** for backfill |
| **Live Trades** | ✅ | ✅ | ✅ | ✅ | WebSocket | All exchanges support |
| **Orderbook Depth** | ✅ | ✅ | ✅ | ✅ | WebSocket | Real-time only, no historical |
| **Depth Snapshot** | ✅ | ✅ | ✅ | ✅ | REST API | For WS sync/resync |
| **Ticker Metadata** | ✅ | ✅ | ✅ | ✅ | REST API | tickSize, minQty, contractSize |
| **Ticker Prices (24h)** | ✅ | ✅ | ✅ | ✅ | REST API | Mark price, volume, % change |
| **Open Interest** | ✅ Perps | ✅ Perps | ❌ | ✅ Perps | REST API | 30-day limit (Binance) |
| **Open Interest (Live)** | ❌ | ❌ | ❌ | ❌ | - | Not implemented |

### Exchange-Specific Notes

#### **Binance**
- **Unique**: Historical trade ZIP archives (public data repository)
- **OI Limit**: 30 days maximum
- **Rate Limiting**: Dynamic bucket using `x-mbx-used-weight-1m` header
- **Domains**:
  - Spot: `api.binance.com` / `stream.binance.com`
  - Linear: `fapi.binance.com` / `fstream.binance.com`
  - Inverse: `dapi.binance.com` / `dstream.binance.com`

#### **Bybit**
- **Location**: `exchange/src/adapter/bybit.rs`
- **No historical trades**: Lacks bulk historical API
- **Volume Split**: Single-colored bars when buy/sell split unavailable
- **Depth Levels**: Reduced from 500→200 at 100ms (API limitation)
- **OI Support**: Linear & Inverse perpetuals only

#### **Hyperliquid**
- **Location**: `exchange/src/adapter/hyperliquid.rs`
- **No historical trades**: Lacks bulk historical API
- **No OI data**: Open interest not captured (TODO: line 144)
- **Rate Limit**: 1200/min (conservative)

#### **OKX**
- **Location**: `exchange/src/adapter/okex.rs`
- **Historical Trades**: Work In Progress (WIP)
- **OI Support**: Linear & Inverse perpetuals only
- **Rate Limit**: 20 requests per 2 seconds

---

## 📊 **Data Structures Summary**

| Data Type | Source | Format | Update Frequency | Persistence |
|-----------|--------|--------|------------------|-------------|
| **Klines** | REST API | `Vec<Kline>` | On chart init | In-memory only |
| **Klines (Live)** | WebSocket | `Kline` | Per candle interval | In-memory only |
| **Trades** | ZIP + REST API | `Vec<Trade>` | On chart init | In-memory only |
| **Trades (Live)** | WebSocket | `Box<[Trade]>` | 100-1000ms | In-memory only |
| **Orderbook Depth** | WebSocket + REST | `Depth { bids, asks }` | 100-1000ms | In-memory only |
| **Ticker Info** | REST API | `HashMap<Ticker, TickerInfo>` | Once at startup | In-memory only |
| **Ticker Prices** | REST API | `HashMap<Ticker, TickerStats>` | 13s / 300s | In-memory only |
| **Open Interest** | REST API | `Vec<OpenInterest>` | On indicator enable | In-memory only |
| **ZIP Archives** | Binance Data Vision | CSV in ZIP | On demand | 4-day cache on disk |

---

## 🎯 **Key Points**

### Limitations by Exchange
1. **Footprint Backfill**: Only Binance (uses ZIP archives)
2. **Orderbook History**: None - all exchanges real-time only
3. **Open Interest**: Binance (30-day limit), Bybit, OKX - Perpetuals only
4. **Historical Trades**:
   - ✅ Binance: Full backfill via ZIP archives
   - ❌ Bybit: Real-time only
   - ❌ Hyperliquid: Real-time only
   - 🚧 OKX: Work in progress

### Data Storage
- **All market data is in-memory** - charts cleared on close
- **Only persisted**: Application state (`saved-state.json`)
  - Layout configuration
  - Theme settings
  - Window specs
  - Audio config
- **Exception**: Binance aggTrades ZIP files (4-day disk cache)

### Rate Limiting
| Exchange | Strategy | Limit | Window |
|----------|----------|-------|--------|
| **Binance Spot** | Dynamic bucket | 6000 requests | 1 minute |
| **Binance Perps** | Dynamic bucket | 2400 requests | 1 minute |
| **Bybit** | Fixed window | 600 requests | 5 seconds |
| **Hyperliquid** | Fixed window | 1200 requests | 1 minute |
| **OKX** | Fixed window | 20 requests | 2 seconds |

### WebSocket Streams by Exchange

**Binance**:
- Depth + Trades: `{symbol}@aggTrade/{symbol}@depth@100ms`
- Klines: `{symbol}@kline_{interval}`

**Bybit**:
- Trades: `publicTrade.{symbol}`
- Depth: `orderbook.{levels}.{symbol}`
- Klines: `kline.{interval}.{symbol}`

**Hyperliquid**:
- All data: `allMids` subscription
- Trades: Per-asset subscription
- Klines: Constructed from trades/allMids

**OKX**:
- Trades: `trades:{symbol}`
- Depth: `books5:{symbol}` or `books:{symbol}`
- Klines: `candle{interval}:{symbol}`

---

## 🔄 **Data Flow Summary**

### Initialization (Chart Creation)
```
1. fetch_ticker_info() → Exchange metadata (once at startup)
2. fetch_klines() → Historical OHLCV
3. fetch_trades() → Historical trades (Binance only via ZIP)
4. fetch_open_interest() → Historical OI (if enabled)
```

### Real-Time Updates
```
WebSocket Streams:
  - DepthReceived → Orderbook + Trade buffer (100-1000ms)
  - KlineReceived → Latest candle update (per interval)

Periodic REST:
  - fetch_ticker_prices() → Every 13s (visible) / 300s (hidden)
```

### Footprint Chart Flow (Binance)
```
1. Download ZIP: https://data.binance.vision/.../aggTrades/{SYMBOL}-YYYY-MM-DD.zip
2. Cache locally: market_data/binance/data/futures/um/daily/aggTrades/
3. Extract CSV → Parse trades
4. Fetch intraday: /api/v3/aggTrades (today's data)
5. Aggregate: TimeSeries<KlineDataPoint> with footprint
6. Real-time: WebSocket trades → Update footprint
```

### Footprint Chart Flow (Other Exchanges)
```
1. fetch_klines() → OHLCV only
2. NO historical trades → Empty footprint initially
3. Real-time: WebSocket trades → Populate footprint going forward
4. Historical candles remain empty (only OHLC bars)
```
