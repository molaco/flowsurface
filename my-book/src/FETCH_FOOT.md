# How We Get Data from the Internet

## Overview

This document explains how FlowSurface fetches historical and real-time market data from cryptocurrency exchanges for the footprint chart.

---

## Two Data Sources

1. **REST API** (HTTP) - Historical data
2. **WebSocket** (WS) - Real-time streaming data

---

## 1. Historical Data (REST API)

### Flow

```
User opens footprint chart
  ↓
KlineChart::missing_data_task()
  → Creates FetchRange::Kline(start_time, end_time)
  ↓
Task::perform(exchange::fetch_klines(ticker_info, timeframe, range))
  ↓
[Async HTTP request to exchange]
```

### Example - Binance

**File**: `exchange/src/adapter/binance.rs`

```rust
pub async fn fetch_klines(ticker_info, timeframe, range) -> Result<Vec<Kline>> {
    // 1. Build URL
    let url = "https://fapi.binance.com/fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit=500";

    // 2. Rate limiting (check before request)
    let limiter = LIMITER.lock().await;
    if let Some(wait) = limiter.prepare_request(weight: 1) {
        tokio::time::sleep(wait).await;
    }

    // 3. Make HTTP GET request
    let response = HTTP_CLIENT.get(url).send().await?;

    // 4. Update rate limiter from response headers
    limiter.update_from_response(&response);

    // 5. Parse JSON response
    let json = response.text().await?;
    let klines: Vec<Kline> = parse_binance_klines(json)?;

    return Ok(klines);
}
```

### Response Example (Binance klines)

```json
[
  [
    1640995200000,  // Open time
    "50000.0",      // Open
    "50200.0",      // High
    "49900.0",      // Low
    "50100.0",      // Close
    "123.45",       // Volume
    1640998799999,  // Close time
    "6172500.0",    // Quote volume
    1234,           // Number of trades
    "61.725",       // Taker buy base volume
    "3086250.0"     // Taker buy quote volume
  ]
  // ... more candles
]
```

### Parsed to

```rust
Kline {
    time: 1640995200000,
    open: Price(50000.0),
    high: Price(50200.0),
    low: Price(49900.0),
    close: Price(50100.0),
    volume: 123.45,
    // ...
}
```

---

## 2. Real-Time Data (WebSocket)

### Connection Flow

```
Dashboard starts streaming
  ↓
subscribe(streams) in main.rs
  ↓
binance::connect_market_stream(ticker_info, push_freq)
  ↓
[WebSocket connection established]
```

### WebSocket Setup (3 steps)

#### A. TCP Connection

**File**: `exchange/src/connect.rs`

```rust
// 1. Connect to exchange domain
let tcp_stream = TcpStream::connect("fstream.binance.com:443").await?;
```

#### B. TLS Upgrade

```rust
// 2. Wrap TCP with TLS encryption
let tls_connector = TlsConnector::new(root_certs);
let tls_stream = tls_connector.connect(domain, tcp_stream).await?;
```

#### C. WebSocket Upgrade

```rust
// 3. HTTP upgrade request to WebSocket
let request = Request::builder()
    .method("GET")
    .uri("/stream?streams=btcusdt@depth@100ms")
    .header("Host", domain)
    .header("Upgrade", "websocket")
    .header("Connection", "upgrade")
    .header("Sec-WebSocket-Key", random_key)
    .body(Empty::new())?;

let response = hyper_client.send(request).await?;
let ws = fastwebsockets::FragmentCollector::new(response.upgraded().await?);
```

**Result**: Bidirectional WebSocket connection

---

## 3. WebSocket Event Loop

### Implementation

**File**: `exchange/src/adapter/binance.rs`

```rust
pub fn connect_market_stream(ticker_info, push_freq) -> impl Stream<Item = Event> {
    stream::channel(100, async move |mut output| {
        let mut state = State::Disconnected;
        let mut depth_cache = LocalDepthCache::new();
        let mut trade_buffer = Vec::new();

        loop {
            match &mut state {
                State::Disconnected => {
                    // Connect WebSocket
                    match connect_ws(domain, url).await {
                        Ok(ws) => {
                            state = State::Connected(ws);
                            output.send(Event::Connected(exchange)).await;
                        }
                        Err(e) => {
                            output.send(Event::Disconnected(exchange, e)).await;
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                    }
                }

                State::Connected(ws) => {
                    // Read WebSocket frame
                    match ws.read_frame().await {
                        Ok(frame) => {
                            let payload = frame.payload;

                            // Parse JSON (using sonic-rs for speed)
                            let iter = unsafe { to_object_iter_unchecked(payload) };

                            for (key, value) in iter {
                                match key.as_str() {
                                    "e" => {
                                        // Event type
                                        if value.as_str() == "depthUpdate" {
                                            // Parse depth update
                                            let bids = parse_bids(payload);
                                            let asks = parse_asks(payload);
                                            depth_cache.update(bids, asks);
                                        }
                                        else if value.as_str() == "trade" {
                                            // Parse trade
                                            let trade = Trade {
                                                price: parse_price(payload),
                                                qty: parse_qty(payload),
                                                time: parse_time(payload),
                                                is_sell: parse_side(payload),
                                            };
                                            trade_buffer.push(trade);
                                        }
                                    }
                                }
                            }

                            // Send to UI every 100ms
                            if timer_elapsed() {
                                output.send(Event::DepthReceived(
                                    stream_kind,
                                    timestamp,
                                    depth_cache.snapshot(),
                                    trade_buffer.clone()
                                )).await;
                                trade_buffer.clear();
                            }
                        }
                        Err(e) => {
                            // Disconnected - reconnect
                            state = State::Disconnected;
                        }
                    }
                }
            }
        }
    })
}
```

### WebSocket Message Example (Binance trade)

```json
{
  "e": "trade",
  "s": "BTCUSDT",
  "t": 12345,
  "p": "50123.45",
  "q": "0.5",
  "T": 1640995200123,
  "m": true
}
```

Parsed to:

```rust
Trade {
    price: Price(50123.45),
    qty: 0.5,
    time: 1640995200123,
    is_sell: true,
}
```

---

## 4. Rate Limiting

### Purpose

Prevents IP bans by respecting exchange limits.

### Implementation

```rust
// Binance limits: 6000 requests/min (Spot), 2400/min (Futures)

struct BinanceLimiter {
    bucket: DynamicBucket {
        limit: 2400,
        current: 1523,  // Used weight
        refill_rate: 60 seconds,
    }
}

impl RateLimiter {
    fn prepare_request(&mut self, weight: usize) -> Option<Duration> {
        if self.current + weight > self.limit {
            // Calculate wait time
            let wait = self.time_until_refill();
            return Some(wait);
        }
        self.current += weight;
        None
    }

    fn update_from_response(&mut self, response: &Response) {
        // Read actual usage from headers
        if let Some(used) = response.headers().get("x-mbx-used-weight-1m") {
            self.current = used.parse().unwrap();
        }
    }
}
```

### Usage

**Before each request**:

```rust
if let Some(wait) = limiter.prepare_request(weight: 5) {
    tokio::time::sleep(wait).await;  // Wait before request
}
```

### Exchange-Specific Limits

- **Binance Spot**: 6000 requests/min
- **Binance Futures**: 2400 requests/min
- **Bybit**: 600 requests/5s
- **Hyperliquid**: 1200 requests/min (conservative)
- **OKX**: 20 requests/2s

---

## 5. Complete Data Flow Diagram

```
┌──────────────────────────────────────────────────┐
│ HISTORICAL DATA (HTTP REST)                      │
├──────────────────────────────────────────────────┤
│                                                  │
│ Chart needs data                                 │
│   ↓                                              │
│ Rate limiter check → wait if needed              │
│   ↓                                              │
│ HTTP GET https://fapi.binance.com/klines         │
│   params: symbol=BTCUSDT, interval=1m, limit=500 │
│   ↓                                              │
│ JSON response [[time, o, h, l, c, v], ...]       │
│   ↓                                              │
│ Parse to Vec<Kline>                              │
│   ↓                                              │
│ TimeSeries::insert_klines()                      │
│   ↓                                              │
│ Chart renders                                    │
│                                                  │
└──────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────┐
│ REAL-TIME DATA (WebSocket)                       │
├──────────────────────────────────────────────────┤
│                                                  │
│ 1. TCP connect to fstream.binance.com:443        │
│     ↓                                            │
│ 2. TLS handshake (encrypt connection)            │
│     ↓                                            │
│ 3. HTTP Upgrade to WebSocket                     │
│     GET /stream?streams=btcusdt@depth@100ms      │
│     ↓                                            │
│ WebSocket established ✓                          │
│     ↓                                            │
│ ┌─────────────────────────────────────────┐      │
│ │ Loop forever:                           │      │
│ │   ws.read_frame() → JSON message        │      │
│ │   ↓                                     │      │
│ │   Parse with sonic-rs (SIMD JSON)       │      │
│ │   ↓                                     │      │
│ │   If trade: add to buffer               │      │
│ │   If depth: update cache                │      │
│ │   ↓                                     │      │
│ │   Every 100ms:                          │      │
│ │     Send Event::DepthReceived(          │      │
│ │       depth: {...},                     │      │
│ │       trades: [...]                     │      │
│ │     )                                   │      │
│ │     ↓                                   │      │
│ │     Dashboard routes to charts          │      │
│ │     ↓                                   │      │
│ │     insert_trades_buffer()              │      │
│ │     ↓                                   │      │
│ │     invalidate() → re-render            │      │
│ └─────────────────────────────────────────┘      │
│                                                  │
└──────────────────────────────────────────────────┘
```

---

## 6. Key Technologies

### HTTP & WebSocket Stack

1. **HTTP Client**: `reqwest` with `rustls-tls` (TLS encryption)
2. **WebSocket**: `fastwebsockets` (fast frame parsing)
3. **JSON Parsing**: `sonic-rs` (SIMD-accelerated, unsafe for speed)
4. **Async Runtime**: `tokio` (handles thousands of connections)
5. **TLS**: `tokio-rustls` + `webpki-roots` (certificate validation)

### Connection Layers

```
Application (FlowSurface)
    ↓
WebSocket Protocol (fastwebsockets)
    ↓
TLS/SSL Encryption (tokio-rustls)
    ↓
TCP Connection (tokio::net::TcpStream)
    ↓
Internet
    ↓
Exchange Server
```

---

## 7. Request Handler & Deduplication

### Purpose

Prevents duplicate requests and tracks request state.

### Implementation

**File**: `exchange/src/fetcher.rs`

```rust
pub struct RequestHandler {
    requests: HashMap<Uuid, FetchRequest>,
}

struct FetchRequest {
    fetch_type: FetchRange,  // Kline(start, end) | Trades(start, end) | OI(start, end)
    status: RequestStatus,   // Pending | Completed | Failed
}

impl RequestHandler {
    pub fn add_request(&mut self, fetch: FetchRange) -> Result<Option<Uuid>> {
        // Check for duplicate
        if let Some(existing) = self.requests.iter().find(|req| req.same_with(fetch)) {
            match existing.status {
                Pending => return Err(ReqError::Overlaps),
                Completed(ts) => {
                    // Allow retry after 30s cooldown
                    if now() - ts > 30_000 {
                        return Ok(Some(existing_id));
                    }
                    return Ok(None);  // Skip duplicate
                }
                Failed(msg) => return Err(ReqError::Failed(msg)),
            }
        }

        // Add new request
        let id = Uuid::new_v4();
        self.requests.insert(id, FetchRequest::new(fetch));
        Ok(Some(id))
    }

    pub fn mark_completed(&mut self, id: Uuid) {
        self.requests[id].status = Completed(now());
    }

    pub fn mark_failed(&mut self, id: Uuid, error: String) {
        self.requests[id].status = Failed(error);
    }
}
```

### Usage

```rust
// In KlineChart
match self.request_handler.add_request(FetchRange::Kline(start, end)) {
    Ok(Some(req_id)) => {
        // Launch fetch task
        Task::perform(fetch_klines(...)).map(move |klines| {
            Message::KlinesFetched(req_id, klines)
        })
    }
    Ok(None) => {
        // Duplicate request, skip
        None
    }
    Err(ReqError::Overlaps) => {
        // Already fetching, wait
        None
    }
    Err(ReqError::Failed(msg)) => {
        // Previous attempt failed
        show_error(msg);
        None
    }
}
```

---

## 8. Exchange-Specific Implementations

### Binance

- **REST**: `https://fapi.binance.com/fapi/v1/klines`
- **WebSocket**: `wss://fstream.binance.com/stream`
- **Rate Limit**: Dynamic bucket, reads headers `x-mbx-used-weight-1m`

### Bybit

- **REST**: `https://api.bybit.com/v5/market/kline`
- **WebSocket**: `wss://stream.bybit.com/v5/public/linear`
- **Rate Limit**: Fixed window, 600/5s

### Hyperliquid

- **REST**: `https://api.hyperliquid.xyz/info`
- **WebSocket**: `wss://api.hyperliquid.xyz/ws`
- **Rate Limit**: Conservative 1200/min

### OKX

- **REST**: `https://www.okx.com/api/v5/market/history-candles`
- **WebSocket**: `wss://ws.okx.com:8443/ws/v5/public`
- **Rate Limit**: Fixed window, 20/2s

All exchanges follow the same pattern:
1. `fetch_klines()` - Historical data
2. `connect_market_stream()` - Real-time WebSocket
3. `fetch_ticksize()` - Instrument metadata

---

## Summary

### Historical Data Flow

```
HTTP REST API → JSON → Vec<Kline/Trade> → TimeSeries → Chart
```

### Real-Time Data Flow

```
WebSocket → Streaming JSON → Event loop → Live updates → Chart
```

### Key Features

- **Rate limiting**: Prevents bans by tracking request weights
- **Deduplication**: Avoids redundant requests
- **Auto-reconnect**: WebSocket reconnects on disconnect
- **4 exchanges**: Binance, Bybit, Hyperliquid, OKX (same pattern)
- **Performance**: SIMD JSON parsing, async I/O, connection pooling

---

## Related Documentation

- [FOOT.md](./FOOT.md) - Complete footprint chart implementation
- [FOOT_SUMMARY.md](./FOOT_SUMMARY.md) - Concise overview
- [WHAT.md](./WHAT.md) - Full project documentation
