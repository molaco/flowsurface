# Historical Data API Research Report

Research for building advanced data acquisition system for Hyperliquid, Bybit, Binance, and Aster exchanges.

**Target use cases:** Backtesting, analysis, historical data downloads
**Scope:** Trades, klines, depth snapshots, open interest, funding rates
**Mode:** Advanced download queue manager

---

## Executive Summary

| Exchange | Trades | Klines | Depth | Open Interest | Funding Rates | Bulk Download | Best Method |
|----------|--------|--------|-------|---------------|---------------|---------------|-------------|
| **Binance** | ✅ | ✅ | ❌ | ✅ | ✅ | ✅ (Vision) | **S3 Bulk** |
| **Bybit** | ✅ | ✅ | ⚠️ | ✅ | ✅ | ✅ (public.bybit.com) | **S3 Bulk** |
| **Hyperliquid** | ✅ | ✅ | ✅ | ✅ | ✅ | ⚠️ (S3, limited) | **API + S3** |
| **Aster** | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | **API Only** |

**Key Findings:**
- **Binance & Bybit**: Best for bulk downloads via S3/public buckets - fastest method
- **Hyperliquid**: Mixed approach - some S3 data, but limited; API needed for most data
- **Aster**: API only, smaller DEX with limited historical depth

---

## 1. Binance

### 1.1 Bulk Download (Binance Vision) ⭐ RECOMMENDED

**Base URL:** `https://data.binance.vision/`
**GitHub:** https://github.com/binance/binance-public-data

#### Available Data Types

**Spot Market:**
- ✅ Trades
- ✅ AggTrades (aggregated trades)
- ✅ Klines (1s, 1m, 3m, 5m, 15m, 30m, 1h, 2h, 4h, 6h, 8h, 12h, 1d, 3d, 1w, 1mo)

**USD-M Futures:**
- ✅ Trades
- ✅ AggTrades
- ✅ Klines
- ✅ indexPriceKlines
- ✅ markPriceKlines
- ✅ premiumIndexKlines
- ✅ Metrics (trading volume, open interest, etc.)

**COIN-M Futures:**
- Same as USD-M

#### File Structure
```
https://data.binance.vision/data/{market}/{frequency}/{dataType}/{symbol}/{symbol}-{dataType}-{date}.zip

Examples:
https://data.binance.vision/data/spot/daily/trades/BTCUSDT/BTCUSDT-trades-2024-10-16.zip
https://data.binance.vision/data/futures/um/monthly/klines/BTCUSDT/1h/BTCUSDT-1h-2024-10.zip
```

#### Characteristics
- **Format:** CSV in ZIP archives
- **Frequency:** Daily and monthly files
- **Checksums:** SHA256 included for verification
- **Historical Depth:** Varies by symbol, many go back to 2017-2018
- **Timestamps:** Microseconds starting Jan 1, 2025
- **Cost:** FREE, no rate limits for bulk downloads
- **License:** MIT

### 1.2 REST API (Fallback for recent data)

**Base URL:** `https://api.binance.com`

#### Historical Trades
- **Endpoint:** `GET /api/v3/historicalTrades`
- **Weight:** 25
- **Max records:** 1000 per request
- **Requires:** API key
- **Pagination:** fromId parameter

#### Klines
- **Endpoint:** `GET /api/v3/klines`
- **Weight:** Variable (based on limit)
- **Max records:** 500 per request (increased from 1000 to 500)
- **Pagination:** startTime/endTime
- **Intervals:** 1s to 1M

#### Funding Rate History
- **Endpoint:** `GET /fapi/v1/fundingRate`
- **Weight:** Shares 500/5min/IP with fundingInfo
- **Max records:** Default limit, use startTime/endTime
- **Returns:** Most recent data if no time range specified

#### Rate Limits
- **System:** Weight-based per IP
- **Monitoring:** `X-MBX-USED-WEIGHT-*` headers
- **Errors:** HTTP 429 (rate limit), HTTP 418 (banned)
- **Ban Duration:** Automatic based on violation severity

---

## 2. Bybit

### 2.1 Bulk Download (public.bybit.com) ⭐ RECOMMENDED

**Base URL:** `https://public.bybit.com/`

#### Available Data Types
- ✅ trading (historical trades)
- ✅ spot (spot trades)
- ✅ kline_for_metatrader4
- ✅ premium_index
- ✅ spot_index

#### File Structure
```
https://public.bybit.com/{dataType}/{SYMBOL}/{SYMBOL}{YYYY-MM-DD}.csv.gz

Examples:
https://public.bybit.com/trading/BTCUSD/BTCUSD2021-01-01.csv.gz
https://public.bybit.com/spot/BTCUSDT/BTCUSDT2023-05-15.csv.gz
```

#### CSV Column Structure (Trading)
```
timestamp, symbol, side, size, price, tickDirection, trdMatchID, grossValue, homeNotional, foreignNotional
```

#### Characteristics
- **Format:** CSV.GZ (gzipped CSV)
- **Frequency:** Daily files
- **Organization:** Per-symbol directories
- **Historical Depth:** Varies, generally from 2020+
- **Cost:** FREE, no rate limits
- **Tools:** Community Python libraries available (bybit-bulk-downloader, bybit-history)

### 2.2 REST API (V5)

**Base URL:** `https://api.bybit.com`

#### Recent Public Trades
- **Endpoint:** `GET /v5/market/recent-trade`
- **Max records:** 1000
- **Covers:** Spot, USDT perpetual, USDC perpetual, Inverse, Options
- **Limitation:** Only ~few minutes of data during busy periods

#### User Execution History
- **Endpoint:** `GET /v5/execution/list`
- **Pagination:** Cursor-based
- **Sort:** execTime descending
- **Max export:** 10,000 records via web UI

#### Klines
- **Endpoint:** `GET /v5/market/kline`
- **Max records:** 1000 per request
- **Default:** 200 records
- **Intervals:** 1, 3, 5, 15, 30, 60, 120, 240, 360, 720 (minutes), D, W, M
- **Categories:** Spot, USDT Perpetual, USDC Perpetual, Inverse Perpetual
- **Sort:** Reverse chronological

#### Depth/Order Book
- **Endpoint:** `GET /v5/market/orderbook`
- **Supports:** Configurable depth levels
- **Real-time:** WebSocket available

#### Open Interest
- **Endpoint:** `GET /v5/market/open-interest`
- **Available for:** Perpetuals and futures

#### Funding Rate
- **Endpoint:** `GET /v5/market/funding/history`
- **Historical access:** Yes

#### Rate Limits (V5)
- **HTTP:** 600 requests per 5-second window per IP (120 req/sec)
- **Enhanced (SDK):** 400 req/sec (24,000 req/min)
- **Headers:** `X-Bapi-Limit-Status`, `X-Bapi-Limit`, `X-Bapi-Limit-Reset-Timestamp`
- **Per UID:** Varies by endpoint category
  - Trade: 10-20 req/sec
  - Position: 10-50 req/sec
  - Account: 5-50 req/sec
  - Asset: 5-300 req/min
- **Error:** retCode 10006 "Too many visits!"
- **Ban:** 10 minutes automatic for HTTP IP limit violation
- **WebSocket:** Max 500 connections per 5min, 1000 per IP

---

## 3. Hyperliquid

### 3.1 S3 Bucket (Limited) ⚠️

**Bucket:** `hyperliquid-archive`
**Node Data:** `s3://hl-mainnet-node-data/`

#### Available via S3
- ✅ L2 book snapshots (market_data/)
- ✅ Asset contexts (asset_ctxs/)
- ✅ Trade fills (`node_fills_by_block/`, `node_fills/`, `node_trades/`)
- ❌ Candles/klines (NOT available)
- ❌ Spot asset data (NOT available)

#### Access Method
```bash
# Requester pays transfer costs
aws s3 cp s3://hyperliquid-archive/market_data/[date]/[hour]/[datatype]/[coin].lz4 ./

# Node data
aws s3 cp s3://hl-mainnet-node-data/node_fills_by_block/[file] ./
```

#### Characteristics
- **Update Frequency:** ~Monthly uploads
- **Cost:** Requester pays AWS transfer fees
- **Format:** LZ4 compressed
- **Limitation:** Incomplete - must use API for most data

### 3.2 REST API ⭐ PRIMARY METHOD

**Base URL (Mainnet):** `https://api.hyperliquid.xyz`
**Testnet:** `https://api.hyperliquid-testnet.xyz`

#### Historical Trades
- **Endpoint:** POST with `{"type": "userFills", "user": "address"}`
- **Max records:** 2000 most recent fills
- **User-specific only**

- **Endpoint:** POST with `{"type": "userFillsByTime", ...}`
- **Max records:** 2000 per response
- **Total available:** 10,000 most recent fills

- **Endpoint:** POST with `{"type": "recentTrades", "coin": "BTC"}`
- **For:** Recent market trades

#### Klines/Candlestick
- **Endpoint:** POST with `{"type": "candleSnapshot", "req": {...}}`
```json
{
  "type": "candleSnapshot",
  "req": {
    "coin": "BTC",
    "interval": "1h",
    "startTime": 1754300000000,
    "endTime": 1754400000000
  }
}
```
- **Max candles:** 5000 most recent
- **Intervals:** 1m, 3m, 5m, 15m, 30m, 1h, 2h, 4h, 8h, 12h, 1d, 3d, 1w, 1M
- **Limitation:** ⚠️ Only recent 5000 candles available

#### Order Book Depth
- **Endpoint:** POST `{"type": "l2Book", "coin": "ETH", "nSigFigs": 5}`
- **Weight:** 2
- **Real-time:** WebSocket subscription `l2Book` available

#### Open Interest
- **Endpoint:** POST `{"type": "metaAndAssetCtxs"}`
- **Returns:**
  - openInterest
  - dayNtlVlm (daily volume)
  - funding rates
  - markPx, midPx, oraclePx
  - premium
  - prevDayPx

#### Funding Rate History
- **Endpoint:** Via Chainstack: `fundingHistory`
- **Available:** Yes, historical funding data

#### Rate Limits
- **REST:** 1200 total weight per minute
- **Weight per request:**
  - Most `exchange` requests: `1 + floor(batch_length / 40)`
  - `info` requests: 2, 20, or 60 depending on type
- **WebSocket:**
  - Max connections: 100
  - Max subscriptions: 1000
  - Max messages: 2000 per minute
  - Max users: 10 for user-specific subscriptions
  - Max inflight: 100 post messages

#### Unique Features
- **Address-based limits:** 1 request per 1 USDC traded
- **Initial buffer:** 10,000 requests
- **Open order limit:** 1000 + 1 per 5M USDC volume (max 5000)

---

## 4. Aster DEX

### 4.1 REST API (Only Option)

**Base URL:** `https://fapi.asterdex.com`
**Format:** JSON (object or array)

#### Historical Trades
- **Recent trades:** Endpoint available
- **Old trades lookup:** `GET /fapi/v3/historicalTrades` (MARKET_DATA)
  - Max records: 500 (default), 1000 (max)
  - Pagination: tradeId
  - Sort: Ascending (oldest first)
- **Aggregate trades:** Compressed aggregate market trades available

**Note:** Only market trades returned (order book fills). Insurance fund trades and ADL trades excluded.

#### Klines
- **Endpoints:**
  - `/fapi/v3/klines` (standard)
  - `/fapi/v3/indexPriceKlines`
  - `/fapi/v3/markPriceKlines`
- **Multiple intervals supported**

#### Depth/Order Book
- **Endpoint:** `/fapi/v3/depth` (full order book)
- **WebSocket:** Partial/diff depth streams available
- **Depth levels:** Configurable

#### Open Interest
- **Available:** Not explicitly detailed in docs
- **Check:** May be available via market info endpoints

#### Funding Rate
- **Endpoint:** `/fapi/v3/fundingRate`
- **Historical:** Yes

#### Rate Limits
- **System:** IP-based request weight
- **Limit enforcement:** 429 (rate limit violated)
- **Ban duration:** 2 minutes to 3 days for IP bans
- **Recommendation:** Use WebSocket to reduce restrictions

#### Characteristics
- **Smaller DEX:** Less historical depth than major exchanges
- **Documentation:** https://github.com/asterdex/api-docs
- **Format:** Similar to Binance Futures API (v3 style)
- **Timestamps:** Milliseconds

---

## Implementation Recommendations

### Priority 1: Bulk Download (Binance & Bybit)

**Why:** Fastest, most reliable, no rate limits, free

**Implementation:**
1. Direct HTTP downloads from S3/public buckets
2. Parse CSV/CSV.GZ files
3. Insert into DuckDB
4. Progress tracking per file
5. Resume capability

**Estimated Speed:**
- Limited by network bandwidth
- Can download multiple files in parallel
- 100s of MB/sec possible

### Priority 2: API Pagination (Hyperliquid & Aster)

**Why:** No bulk option available

**Implementation:**
1. Request-based chunking
2. Respect rate limits (exponential backoff)
3. Cursor/time-based pagination
4. Insert batches into DuckDB
5. Track last synced timestamp/ID

**Estimated Speed:**
- Hyperliquid: ~1200 requests/min = 2.4M records/min (if 2000 per request)
- Aster: ~Similar to Binance API limits

### Priority 3: Hybrid (Recent data via API)

**Why:** Fill gaps between bulk files and real-time

**Use cases:**
- Download bulk up to yesterday
- Use API for today's data
- Scheduled sync jobs

### Data Validation Strategy

1. **Gap Detection:**
   - Check for missing time ranges
   - Identify missing days in bulk downloads
   - Detect trade ID discontinuities

2. **Duplicate Prevention:**
   - Use deterministic IDs (already implemented)
   - ON CONFLICT DO NOTHING (already implemented)
   - Verify checksums for bulk files

3. **Integrity Checks:**
   - Timestamp ordering
   - Price sanity checks
   - Volume reasonableness

### Queue Manager Design

**Job Structure:**
```rust
struct DownloadJob {
    exchange: Exchange,
    ticker: Ticker,
    data_type: DataType,  // Trades, Klines, etc.
    date_range: (DateTime, DateTime),
    method: DownloadMethod,  // Bulk or API
    priority: u8,
    status: JobStatus,  // Queued, Running, Paused, Complete, Failed
    progress: f32,
    speed: f64,  // records/sec or MB/sec
    eta: Option<Duration>,
}

enum DownloadMethod {
    Bulk { base_url: String, file_pattern: String },
    Api { endpoint: String, pagination: PaginationMethod },
}

enum PaginationMethod {
    Cursor { cursor_field: String },
    TimeRange { chunk_size: Duration },
    Offset { limit: usize },
}
```

**Parallelization:**
- Binance/Bybit: Up to 10 concurrent downloads per exchange (no rate limit)
- Hyperliquid: Rate-limited queue (respect 1200 weight/min)
- Aster: Rate-limited queue (respect IP weight)
- Per-exchange rate limiter with token bucket algorithm

### Storage Estimates

**Trades:**
- ~100 bytes per trade record
- 1M trades = ~100 MB
- High-volume pair (BTCUSDT): ~500K trades/day = 50 MB/day

**Klines:**
- ~200 bytes per kline
- 1440 1m candles/day = 288 KB/day
- Negligible compared to trades

**Total for 1 year, 1 pair:**
- Trades: ~18 GB
- Klines (all intervals): ~1 GB

---

## Rate Limit Handling Implementation

```rust
struct RateLimiter {
    max_requests: usize,
    window: Duration,
    tokens: Arc<Mutex<usize>>,
    last_refill: Arc<Mutex<Instant>>,
}

impl RateLimiter {
    async fn acquire(&self) -> Result<(), Error> {
        // Token bucket algorithm
        // Wait if no tokens available
        // Refill based on time elapsed
    }

    fn get_wait_time(&self) -> Duration {
        // Calculate how long to wait for next token
    }
}

// Per-exchange rate limiters
struct ExchangeRateLimiter {
    binance: RateLimiter,     // Weight-based
    bybit: RateLimiter,       // 120 req/sec
    hyperliquid: RateLimiter, // 1200 weight/min
    aster: RateLimiter,       // IP-based weight
}
```

---

## Error Handling & Retry Strategy

### Retryable Errors
- Network timeouts
- HTTP 429 (rate limit)
- HTTP 5xx (server errors)
- Connection resets

### Non-Retryable Errors
- HTTP 400 (bad request)
- HTTP 401/403 (auth errors)
- HTTP 404 (data not found)
- Invalid parameters

### Retry Logic
```rust
async fn download_with_retry(
    job: &DownloadJob,
    max_retries: u32,
) -> Result<Vec<u8>, Error> {
    let mut attempt = 0;
    let mut delay = Duration::from_secs(1);

    loop {
        match download(job).await {
            Ok(data) => return Ok(data),
            Err(e) if e.is_retryable() && attempt < max_retries => {
                attempt += 1;
                tokio::time::sleep(delay).await;
                delay *= 2; // Exponential backoff
            }
            Err(e) => return Err(e),
        }
    }
}
```

---

## UI/UX Requirements

### Advanced Mode Features

1. **Job Configuration Panel:**
   - Exchange selector
   - Ticker multi-select
   - Data type checkboxes (trades, klines, depth, etc.)
   - Date range picker
   - Priority slider

2. **Queue View:**
   - List of all jobs (queued, running, paused, completed, failed)
   - Drag to reorder
   - Pause/resume/cancel buttons
   - Retry failed jobs

3. **Progress Display (Per Job):**
   - Status badge
   - Progress bar
   - Speed (records/sec or MB/sec)
   - ETA
   - Records downloaded / total

4. **Overall Stats:**
   - Total jobs: X queued, Y running, Z completed
   - Overall download speed
   - Total data downloaded (MB/GB)
   - Overall ETA

5. **Validation Panel:**
   - Gap detection results
   - Data integrity checks
   - Warnings/errors

6. **Database Stats (Already Implemented):**
   - Total records by type
   - Size on disk
   - Per-ticker breakdown

---

## Conclusion & Next Steps

### Best Practices Summary

1. **Use bulk downloads for Binance & Bybit** - fastest and most reliable
2. **Use API pagination for Hyperliquid & Aster** - only option
3. **Implement per-exchange rate limiters** - avoid bans
4. **Batch inserts into DuckDB** - optimal performance
5. **Track progress persistently** - allow resume after crashes
6. **Validate data quality** - detect gaps and duplicates

### Complexity Assessment

- **Binance Bulk:** ⭐⭐☆☆☆ (Easy - HTTP download + CSV parse)
- **Bybit Bulk:** ⭐⭐☆☆☆ (Easy - same as Binance)
- **Hyperliquid API:** ⭐⭐⭐☆☆ (Medium - rate limiting + pagination)
- **Aster API:** ⭐⭐⭐☆☆ (Medium - similar to Hyperliquid)
- **Queue Manager:** ⭐⭐⭐⭐☆ (Complex - concurrent jobs, progress tracking, UI)

### Development Phases

**Phase 1: Bulk Download (Binance only)**
- Single ticker, single date
- CSV parsing
- DuckDB insertion
- Basic progress tracking

**Phase 2: Bulk Multi-Ticker**
- Queue system
- Parallel downloads
- Progress UI
- Gap detection

**Phase 3: API Pagination (Hyperliquid)**
- Rate limiter
- Cursor-based pagination
- Error handling & retry

**Phase 4: Complete System**
- All 4 exchanges
- All data types
- Advanced UI
- Validation suite

---

**Research completed:** 2025-10-17
**Researched by:** Claude (Sonnet 4.5)
