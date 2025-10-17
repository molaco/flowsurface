# Aster DEX Futures API v3 - Complete Implementation Guide

**Documentation Source**: https://github.com/asterdex/api-docs/blob/master/aster-finance-futures-api-v3.md

**Base API Endpoint**: `https://fapi.asterdex.com`
**WebSocket Endpoint**: `wss://fstream.asterdex.com`

---

## Table of Contents

1. [API Architecture](#1-api-architecture)
2. [Rate Limiting and Security](#2-rate-limiting-and-security)
3. [Data Types and Filters](#3-data-types-and-filters)
4. [Market Data Endpoints](#4-market-data-endpoints)
5. [WebSocket Streams](#5-websocket-streams)
6. [Position and Mode Management](#6-position-and-mode-management)
7. [Order Management](#7-order-management)
8. [Account and Balance Management](#8-account-and-balance-management)
9. [User Data Streams](#9-user-data-streams)
10. [Error Handling](#10-error-handling)
11. [Python Implementation Examples](#11-python-implementation-examples)
12. [Comparison with Binance Futures API](#12-comparison-with-binance-futures-api)

---

## 1. API Architecture

### General API Information

**Base Configuration:**
- **Base Endpoint:** `https://fapi.asterdex.com`
- **Response Format:** JSON objects and arrays
- **Data Ordering:** Ascending order (oldest first)
- **Timestamp Format:** All timestamps are in milliseconds
- **Data Types:** Follows Java data type definitions

### HTTP Return Codes

The API uses standard HTTP status codes with specific meanings:

- **4XX Codes:** Client-side errors (malformed requests, invalid parameters)
- **403 Forbidden:** Web Application Firewall (WAF) limit violation
- **429 Too Many Requests:** Rate limit exceeded
- **418 I'm a teapot:** IP address has been auto-banned after receiving repeated 429 errors
- **5XX Codes:** Internal server errors (issues on Aster's side)
- **503 Service Unavailable:** Request was sent but server didn't respond within timeout period

### Error Response Format

```json
{
  "code": -1121,
  "msg": "Invalid symbol."
}
```

- Any endpoint can return an error
- Errors include both a numeric code and descriptive message
- Standardized format across all endpoints

### General Information on Endpoints

**Request Methods:**
- **GET endpoints:** Parameters sent in query string
- **POST/PUT/DELETE endpoints:** Parameters sent in request body

**Rate Limiting:**
- Tracked per IP address
- Weight-based system for different endpoints
- Order rate limits enforced per account
- **Recommendation:** Use WebSocket streams to reduce REST API calls and avoid access restrictions

**WebSocket Constraints:**
- 24-hour maximum connection validity
- 10 incoming messages per second limit
- Maximum 200 stream subscriptions per connection

### API Authentication Types

The API defines five security types for endpoints:

1. **NONE:** Public endpoints, no authentication required
2. **TRADE:** Trading operations requiring valid signer and signature
3. **USER_DATA:** User-specific data access requiring authentication
4. **USER_STREAM:** User WebSocket streams requiring authentication
5. **MARKET_DATA:** Market data endpoints requiring authentication

### Authentication Flow (Step-by-Step)

**1. Setup Phase:**
- Create an API wallet at Aster DEX
- Obtain API wallet address (this becomes your "signer")
- Securely store the API wallet's private key

**2. Signature Generation Process:**

The authentication uses a Web3-based cryptographic signature:

a. **Parameter Preparation:**
   - Convert all parameters to strings
   - Sort parameters by ASCII order

b. **Encoding:**
   - Use Web3 ABI (Application Binary Interface) parameter encoding
   - Generate Keccak hash from the encoded parameters

c. **Signing:**
   - Sign the Keccak hash using ECDSA (Elliptic Curve Digital Signature Algorithm)
   - Use the API wallet's private key for signing

**3. Required Authentication Parameters:**

Every authenticated request must include:
- **user:** Main account wallet address
- **signer:** API wallet address (the one you created)
- **nonce:** Current timestamp in microseconds (for replay attack prevention)
- **signature:** The cryptographic signature generated above

### Timing Security

**Strict Timestamp Validation:**
- Server timestamp must be within **5000ms** of the request timestamp
- **recvWindow parameter:** Defines the validity window for the request
  - Recommended: ≤5000ms for tighter security
  - Requests outside this window are rejected
- Prevents replay attacks and ensures request freshness

### Key Architectural Decisions

1. **Web3-Native Authentication:**
   - Uses blockchain wallet-based authentication instead of traditional API keys
   - Leverages ECDSA cryptographic signatures
   - Aligns with decentralized exchange (DEX) philosophy

2. **Dual Wallet System:**
   - Separates main account wallet from API wallet (signer)
   - Provides security isolation for API operations

3. **Nonce-Based Replay Protection:**
   - Uses microsecond timestamps as nonces
   - Ensures each request is unique and time-bound

4. **Multi-Tier Rate Limiting:**
   - IP-based restrictions
   - Weight-based endpoint limits
   - Account-level order limits
   - Progressive enforcement (429 → 418 auto-ban)

5. **WebSocket-First Approach:**
   - Encourages WebSocket usage to reduce REST API load
   - Longer connection validity (24 hours)
   - Higher throughput for real-time data

6. **Security-First Design:**
   - Short recvWindow recommendations
   - Strict timing validation
   - WAF protection (403 errors)
   - Comprehensive error reporting

### Security Best Practices

- Use small `recvWindow` values (≤5000ms recommended)
- Protect API wallet private keys securely
- Implement proper error handling for all error codes
- Back off exponentially when receiving 429 responses
- Monitor for 418 status to prevent IP bans
- Prefer WebSocket connections for high-frequency data access

---

## 2. Rate Limiting and Security

### IP Rate Limits

**Implementation:**
- **Weight-based system**: Each API endpoint has a "weight" value that counts toward the limit
- **Limit threshold**: 2400 request weight per minute per IP
- **Tracking headers**: Every request returns `X-MBX-USED-WEIGHT-(intervalNum)(intervalLetter)` header showing current usage
- **Scope**: Limits apply to IP addresses, NOT API keys

**Enforcement:**
- **HTTP 429**: Returned when rate limit is exceeded
- **HTTP 418**: Returned for repeated violations (IP ban warning)
- **Ban duration**: Scales from 2 minutes up to 3 days for repeated violations
- **Required action**: Must back off from API requests when receiving 429

### Order Rate Limits

**Implementation:**
- **Order count tracking**: 1200 orders per minute per account
- **Tracking headers**: `X-MBX-ORDER-COUNT-(intervalNum)(intervalLetter)` included in order responses
- **Scope**: Counted against each account (not IP)
- **Note**: Rejected or unsuccessful orders may not include order count headers

### Authentication Signature Payload

**Required Parameters:**
- `user`: Main account wallet address (Ethereum address format)
- `signer`: API wallet address (the wallet signing the request)
- `nonce`: Current timestamp in microseconds (µs precision)
- `signature`: Generated ECDSA signature

### Timing Security

**Mechanism:**
- **timestamp parameter**: Required for signed endpoints
- **recvWindow parameter**: Optional, defaults to 5000 milliseconds
- **Validation logic**:
```
if (timestamp < (serverTime + 1000) && (serverTime - timestamp) <= recvWindow) {
    // Process request
} else {
    // Reject request
}
```

**Best Practices:**
- Use small recvWindow value (5000ms or less recommended)
- Ensures requests are processed within valid time window
- Protects against replay attacks

### Endpoints Requiring Signature

Four endpoint security types require authentication:

1. **TRADE**: Order placement, cancellation, modification
2. **USER_DATA**: Account information, positions, balances
3. **USER_STREAM**: User data stream authentication
4. **MARKET_DATA**: Market data requiring authentication

**NONE type**: Public endpoints (no signature required)

### Signature Authentication Process

**Step-by-step Algorithm:**

1. **Convert parameters to strings**: All parameter values must be string format
2. **Sort by ASCII order**: Alphabetically sort parameter names
3. **Generate JSON string**: Create JSON representation of sorted parameters
4. **Web3 ABI encoding**: Encode parameters using Ethereum ABI encoding
5. **Keccak hash generation**: Apply Keccak-256 hash function
6. **ECDSA signing**: Sign the hash with API wallet's private key

**Example workflow:**
```
Parameters: {symbol, side, type, quantity, price, timestamp, nonce}
→ Sort ASCII: {nonce, price, quantity, side, symbol, timestamp, type}
→ JSON stringify
→ ABI encode
→ Keccak-256 hash
→ Sign with private key
→ Append signature to request
```

### POST /fapi/v3/order Examples

**Required Parameters:**
- `symbol`: Trading pair (e.g., "BTCUSDT")
- `positionSide`: "LONG" or "SHORT"
- `type`: Order type (LIMIT, MARKET, etc.)
- `side`: "BUY" or "SELL"
- `timeInForce`: Order time in force (GTC, IOC, FOK)
- `quantity`: Order size
- `price`: Limit price (for LIMIT orders)
- `user`: Main account address
- `signer`: API wallet address
- `nonce`: Timestamp in microseconds
- `signature`: Generated signature

**Security headers included in response**:
- `X-MBX-USED-WEIGHT-1M`: Current request weight usage
- `X-MBX-ORDER-COUNT-1M`: Current order count

### GET /fapi/v3/order Examples

**Query Parameters:**
- `symbol`: Trading pair
- `orderId` or `origClientOrderId`: Order identifier
- `user`: Main account address
- `signer`: API wallet address
- `nonce`: Current timestamp (microseconds)
- `signature`: Authentication signature

**Use case**: Query existing order status

### Key Security Recommendations

1. **Use WebSocket streams**: Recommended for data fetching to reduce REST API load
2. **Implement exponential backoff**: When receiving 429 errors
3. **Monitor rate limit headers**: Track usage to avoid violations
4. **Small recvWindow**: Use 5000ms or less for timing security
5. **Secure key storage**: Protect API wallet private key
6. **IP allowlisting**: Consider implementing IP restrictions where possible

---

## 3. Data Types and Filters

### Terminology

1. **Base Asset**: The asset that represents the quantity in a trading pair
2. **Quote Asset**: The asset that represents the price in a trading pair

### ENUM Definitions

#### Symbol Type
- `FUTURE`

#### Contract Type
- `PERPETUAL`

#### Contract Status
- `PENDING_TRADING`
- `TRADING`
- `PRE_SETTLE`
- `SETTLING`
- `CLOSE`

#### Order Status
- `NEW`
- `PARTIALLY_FILLED`
- `FILLED`
- `CANCELED`
- `REJECTED`
- `EXPIRED`

#### Order Types
- `LIMIT`
- `MARKET`
- `STOP`
- `STOP_MARKET`
- `TAKE_PROFIT`
- `TAKE_PROFIT_MARKET`
- `TRAILING_STOP_MARKET`

#### Order Side
- `BUY`
- `SELL`

#### Position Side
- `BOTH` (used in One-way mode)
- `LONG` (used in Hedge mode)
- `SHORT` (used in Hedge mode)

#### Time in Force
- `GTC` (Good Till Cancel)
- `IOC` (Immediate or Cancel)
- `FOK` (Fill or Kill)
- `GTX` (Good Till Crossing/Post Only)

#### Working Type
- `MARK_PRICE`
- `CONTRACT_PRICE`

#### Response Type
- `ACK`
- `RESULT`

#### Kline/Candlestick Intervals
- `1m`, `3m`, `5m`, `15m`, `30m`
- `1h`, `2h`, `4h`, `6h`, `8h`, `12h`
- `1d`, `3d`
- `1w`
- `1M`

#### Rate Limit Types
- `REQUEST_WEIGHT`
- `ORDERS`

#### Rate Limit Intervals
- `MINUTE`

#### Position Modes
- **One-way Mode**: Default mode where positions are on a single side
- **Hedge Mode**: Allows separate LONG and SHORT positions for the same symbol

#### Margin Types
- **Single-Asset Mode**
- **Multi-Assets Mode**

### Symbol Filters System

The filter system validates trading parameters to ensure orders meet exchange requirements.

#### 1. PRICE_FILTER

```json
{
    "filterType": "PRICE_FILTER",
    "minPrice": "0.01",
    "maxPrice": "100000",
    "tickSize": "0.01"
}
```

**Parameters:**
- `minPrice`: Minimum allowed price
- `maxPrice`: Maximum allowed price
- `tickSize`: Price increment intervals

**Validation Rules:**
- Price must be >= `minPrice`
- Price must be <= `maxPrice`
- Price must be divisible by `tickSize`

#### 2. LOT_SIZE

```json
{
    "filterType": "LOT_SIZE",
    "minQty": "0.001",
    "maxQty": "10000",
    "stepSize": "0.001"
}
```

**Parameters:**
- `minQty`: Minimum allowed quantity
- `maxQty`: Maximum allowed quantity
- `stepSize`: Quantity increment intervals

**Validation Rules:**
- Quantity must be >= `minQty`
- Quantity must be <= `maxQty`
- Quantity must be divisible by `stepSize`

#### 3. MARKET_LOT_SIZE

```json
{
    "filterType": "MARKET_LOT_SIZE",
    "minQty": "0.001",
    "maxQty": "10000",
    "stepSize": "0.001"
}
```

Same as LOT_SIZE, but specifically applies to `MARKET` orders only

#### 4. MAX_NUM_ORDERS

```json
{
    "filterType": "MAX_NUM_ORDERS",
    "limit": 200
}
```

**Parameters:**
- `limit`: Maximum number of open orders allowed

**Purpose:** Limits the total number of orders a user can have open simultaneously on a symbol.

#### 5. MAX_NUM_ALGO_ORDERS

```json
{
    "filterType": "MAX_NUM_ALGO_ORDERS",
    "limit": 100
}
```

**Parameters:**
- `limit`: Maximum number of algo orders allowed

**Purpose:** Restricts the number of algorithmic orders. Algo orders include:
- `STOP`
- `STOP_MARKET`
- `TAKE_PROFIT`
- `TAKE_PROFIT_MARKET`
- `TRAILING_STOP_MARKET`

#### 6. PERCENT_PRICE

```json
{
    "filterType": "PERCENT_PRICE",
    "multiplierUp": "1.1500",
    "multiplierDown": "0.8500",
    "multiplierDecimal": 4
}
```

**Parameters:**
- `multiplierUp`: Upper percentage multiplier
- `multiplierDown`: Lower percentage multiplier
- `multiplierDecimal`: Decimal precision for multipliers

**Validation Rules:**
- For `BUY` orders: `price` <= mark price × `multiplierUp`
- For `SELL` orders: `price` >= mark price × `multiplierDown`

**Purpose:** Validates price relative to a percentage of market price. Prevents extreme price deviations.

#### 7. MIN_NOTIONAL

```json
{
    "filterType": "MIN_NOTIONAL",
    "notional": "1"
}
```

**Parameters:**
- `notional`: Minimum notional value required

**Validation Rules:**
- Notional value = `price` × `quantity`
- For `MARKET` orders without a price, the mark price is used
- Order must meet minimum notional value threshold

**Purpose:** Ensures trades meet a certain economic threshold.

### How the Filter System Works

#### Validation Process:

1. **Sequential Validation**: Filters are checked in order of specificity when an order is placed
2. **Rejection on Failure**: If any filter validation fails, the order is rejected with an appropriate error
3. **Combined Effect**: All applicable filters must pass for an order to be accepted

#### Key Benefits:

- **Price Stability**: PRICE_FILTER and PERCENT_PRICE ensure prices are within reasonable ranges
- **Quantity Control**: LOT_SIZE and MARKET_LOT_SIZE prevent invalid quantities
- **Risk Management**: MIN_NOTIONAL ensures meaningful trade sizes
- **System Protection**: MAX_NUM_ORDERS and MAX_NUM_ALGO_ORDERS prevent system overload
- **Market Integrity**: PERCENT_PRICE prevents extreme price manipulation

---

## 4. Market Data Endpoints

### 1. Test Connectivity

**Purpose:** Test API connectivity and verify the endpoint is reachable.

- **Endpoint:** `GET /fapi/v1/ping`
- **Weight:** 1
- **Parameters:** None
- **Response:** `{}`

### 2. Check Server Time

**Purpose:** Get the current server time to synchronize client requests.

- **Endpoint:** `GET /fapi/v1/time`
- **Weight:** 1
- **Parameters:** None
- **Response:**
```json
{
  "serverTime": 1499827319559
}
```

### 3. Exchange Information

**Purpose:** Get comprehensive information about exchange trading rules, symbols, rate limits, assets, and filters.

- **Endpoint:** `GET /fapi/v1/exchangeInfo`
- **Weight:** 1
- **Parameters:** None
- **Response Format:**
```json
{
  "exchangeFilters": [],
  "rateLimits": [
    {
      "rateLimitType": "REQUEST_WEIGHT",
      "interval": "MINUTE",
      "intervalNum": 1,
      "limit": 2400
    },
    {
      "rateLimitType": "ORDERS",
      "interval": "MINUTE",
      "intervalNum": 1,
      "limit": 1200
    }
  ],
  "serverTime": 1565613908500,
  "assets": [
    {
      "asset": "BUSD",
      "marginAvailable": true,
      "autoAssetExchange": 0
    }
  ],
  "symbols": [
    {
      "symbol": "DOGEUSDT",
      "contractType": "PERPETUAL",
      "status": "TRADING",
      "baseAsset": "BLZ",
      "quoteAsset": "USDT",
      "filters": [...],
      "orderTypes": ["LIMIT", "MARKET", "STOP"],
      "timeInForce": ["GTC", "IOC", "FOK"]
    }
  ],
  "timezone": "UTC"
}
```

### 4. Order Book

**Purpose:** Retrieve the current order book (bids and asks) for a specific symbol.

- **Endpoint:** `GET /fapi/v1/depth`
- **Weight:** Varies based on limit (2-20)
  - Limit 5-100: Weight 2
  - Limit 500: Weight 5
  - Limit 1000: Weight 10
  - Higher limits: Weight 20
- **Parameters:**
  - `symbol` (STRING, MANDATORY) - Trading pair symbol
  - `limit` (INT, OPTIONAL) - Number of order book entries (default: 500, valid: 5, 10, 20, 50, 100, 500, 1000)
- **Response Format:**
```json
{
  "lastUpdateId": 1027024,
  "E": 1589436922972,
  "T": 1589436922959,
  "bids": [
    ["4.00000000", "431.00000000"]
  ],
  "asks": [
    ["4.00000200", "12.00000000"]
  ]
}
```

### 5. Recent Trades List

**Purpose:** Get recent market trades for a specific symbol.

- **Endpoint:** `GET /fapi/v1/trades`
- **Weight:** 1
- **Parameters:**
  - `symbol` (STRING, MANDATORY)
  - `limit` (INT, OPTIONAL) - Default: 500, max: 1000
- **Response Format:**
```json
[
  {
    "id": 28457,
    "price": "4.00000100",
    "qty": "12.00000000",
    "quoteQty": "48.00",
    "time": 1499865549590,
    "isBuyerMaker": true
  }
]
```

### 6. Old Trades Lookup

**Purpose:** Get historical/older market trades.

- **Endpoint:** `GET /fapi/v1/historicalTrades`
- **Weight:** 20
- **Parameters:**
  - `symbol` (STRING, MANDATORY)
  - `limit` (INT, OPTIONAL) - Default: 500, max: 1000
  - `fromId` (LONG, OPTIONAL) - Trade ID to fetch from

### 7. Compressed/Aggregate Trades List

**Purpose:** Get compressed, aggregated market trades data.

- **Endpoint:** `GET /fapi/v1/aggTrades`
- **Weight:** 20
- **Parameters:**
  - `symbol` (STRING, MANDATORY)
  - `fromId` (LONG, OPTIONAL)
  - `startTime` (LONG, OPTIONAL)
  - `endTime` (LONG, OPTIONAL)
  - `limit` (INT, OPTIONAL) - Default: 500, max: 1000

### 8. Kline/Candlestick Data

**Purpose:** Get kline/candlestick bars for a specific symbol and interval.

- **Endpoint:** `GET /fapi/v1/klines`
- **Weight:** Varies based on limit (1-10)
- **Parameters:**
  - `symbol` (STRING, MANDATORY)
  - `interval` (ENUM, MANDATORY) - 1m, 3m, 5m, 15m, 30m, 1h, 2h, 4h, 6h, 8h, 12h, 1d, 3d, 1w, 1M
  - `startTime` (LONG, OPTIONAL)
  - `endTime` (LONG, OPTIONAL)
  - `limit` (INT, OPTIONAL) - Default: 500, max: 1500
- **Response Format:**
```json
[
  [
    1499040000000,      // Open time
    "0.01634790",       // Open
    "0.80000000",       // High
    "0.01575800",       // Low
    "0.01577100",       // Close
    "148976.11427815",  // Volume
    1499644799999,      // Close time
    "2434.19055334",    // Quote asset volume
    308,                // Number of trades
    "1756.87402397",    // Taker buy base asset volume
    "28.46694368",      // Taker buy quote asset volume
    "17928899.62484339" // Ignore
  ]
]
```

### 9. Index Price Kline/Candlestick Data

- **Endpoint:** `GET /fapi/v1/indexPriceKlines`
- **Weight:** Varies based on limit (1-10)
- **Parameters:**
  - `pair` (STRING, MANDATORY)
  - `interval` (ENUM, MANDATORY)
  - `startTime`, `endTime`, `limit` (OPTIONAL)

### 10. Mark Price Kline/Candlestick Data

- **Endpoint:** `GET /fapi/v1/markPriceKlines`
- **Weight:** Varies based on limit (1-10)
- **Parameters:**
  - `symbol` (STRING, MANDATORY)
  - `interval` (ENUM, MANDATORY)
  - `startTime`, `endTime`, `limit` (OPTIONAL)

### 11. Mark Price

**Purpose:** Get mark price and funding rate information.

- **Endpoint:** `GET /fapi/v1/premiumIndex`
- **Weight:** 1 (single symbol) or 10 (all symbols)
- **Parameters:**
  - `symbol` (STRING, OPTIONAL) - If omitted, returns all symbols
- **Response:**
```json
{
  "symbol": "BTCUSDT",
  "markPrice": "11793.63104562",
  "indexPrice": "11781.80495970",
  "estimatedSettlePrice": "11781.16138815",
  "lastFundingRate": "0.00038246",
  "nextFundingTime": 1597392000000,
  "interestRate": "0.00010000",
  "time": 1597370495002
}
```

### 12. Get Funding Rate History

- **Endpoint:** `GET /fapi/v1/fundingRate`
- **Weight:** 1
- **Parameters:**
  - `symbol` (STRING, OPTIONAL)
  - `startTime`, `endTime` (LONG, OPTIONAL)
  - `limit` (INT, OPTIONAL) - Default: 100, max: 1000

### 13. 24hr Ticker Price Change Statistics

- **Endpoint:** `GET /fapi/v1/ticker/24hr`
- **Weight:** 1 (single symbol) or 40 (all symbols)
- **Parameters:**
  - `symbol` (STRING, OPTIONAL)

### 14. Symbol Price Ticker

- **Endpoint:** `GET /fapi/v1/ticker/price`
- **Weight:** 1 (single symbol) or 2 (all symbols)
- **Parameters:**
  - `symbol` (STRING, OPTIONAL)

### 15. Symbol Order Book Ticker

**Purpose:** Get the best bid and ask price and quantity.

- **Endpoint:** `GET /fapi/v1/ticker/bookTicker`
- **Weight:** 1 (single symbol) or 2 (all symbols)
- **Parameters:**
  - `symbol` (STRING, OPTIONAL)
- **Response:**
```json
{
  "symbol": "BTCUSDT",
  "bidPrice": "4.00000000",
  "bidQty": "431.00000000",
  "askPrice": "4.00000200",
  "askQty": "9.00000000",
  "time": 1589437530011
}
```

---

## 5. WebSocket Streams

### WebSocket Connection Architecture

**Base Configuration:**
- **Base URL**: `wss://fstream.asterdex.com`
- **Connection Methods**:
  - **Raw Streams**: `/ws/<streamName>` - Single stream access
  - **Combined Streams**: `/stream?streams=<streamName1>/<streamName2>/<streamName3>` - Multiple streams
- **Combined Stream Format**: Events are wrapped as `{"stream":"<streamName>","data":<rawPayload>}`
- **Stream Naming**: All symbols must be **lowercase**

### Connection Limits & Rules

- **Connection Validity**: 24 hours (automatic disconnect at 24-hour mark)
- **Maximum Streams per Connection**: 200 streams
- **Message Rate Limit**: 10 incoming messages per second (exceeding causes disconnect)
- **Keep-Alive Mechanism**:
  - Server sends `ping frame` every 5 minutes
  - Connection disconnected if no `pong frame` received within 15 minutes
  - Unsolicited `pong frames` are allowed
- **IP Bans**: Repeated disconnections may result in IP ban

### Live Subscribing/Unsubscribing

#### Subscribe to Stream(s)

**Request:**
```json
{
  "method": "SUBSCRIBE",
  "params": [
    "btcusdt@aggTrade",
    "btcusdt@depth"
  ],
  "id": 1
}
```

**Response:**
```json
{
  "result": null,
  "id": 1
}
```

#### Unsubscribe from Stream(s)

**Request:**
```json
{
  "method": "UNSUBSCRIBE",
  "params": [
    "btcusdt@depth"
  ],
  "id": 312
}
```

#### Listing Subscriptions

**Request:**
```json
{
  "method": "LIST_SUBSCRIPTIONS",
  "id": 3
}
```

**Response:**
```json
{
  "result": [
    "btcusdt@aggTrade"
  ],
  "id": 3
}
```

### Stream Types

#### 1. Aggregate Trade Streams

**Stream Name**: `<symbol>@aggTrade`
**Update Speed**: 100ms

**Payload:**
```json
{
  "e": "aggTrade",
  "E": 123456789,
  "s": "BTCUSDT",
  "a": 5933014,
  "p": "0.001",
  "q": "100",
  "f": 100,
  "l": 105,
  "T": 123456785,
  "m": true
}
```

#### 2. Mark Price Streams

**Stream Name**: `<symbol>@markPrice` or `<symbol>@markPrice@1s`
**Update Speed**: 3000ms or 1000ms

**Payload:**
```json
{
  "e": "markPriceUpdate",
  "E": 1562305380000,
  "s": "BTCUSDT",
  "p": "11794.15000000",
  "i": "11784.62659091",
  "P": "11784.25641265",
  "r": "0.00038167",
  "T": 1562306400000
}
```

#### 3. Kline/Candlestick Streams

**Stream Name**: `<symbol>@kline_<interval>`
**Update Speed**: 250ms

**Available Intervals**: 1m, 3m, 5m, 15m, 30m, 1h, 2h, 4h, 6h, 8h, 12h, 1d, 3d, 1w, 1M

**Payload:**
```json
{
  "e": "kline",
  "E": 123456789,
  "s": "BTCUSDT",
  "k": {
    "t": 123400000,
    "T": 123460000,
    "s": "BTCUSDT",
    "i": "1m",
    "f": 100,
    "L": 200,
    "o": "0.0010",
    "c": "0.0020",
    "h": "0.0025",
    "l": "0.0015",
    "v": "1000",
    "n": 100,
    "x": false,
    "q": "1.0000",
    "V": "500",
    "Q": "0.500",
    "B": "123456"
  }
}
```

#### 4. Individual Symbol Ticker Streams

**Stream Name**: `<symbol>@ticker`
**Update Speed**: 500ms

24hr rolling window statistics

#### 5. All Market Tickers Stream

**Stream Name**: `!ticker@arr`
**Update Speed**: 1000ms

#### 6. Book Ticker Streams

**Stream Name**: `<symbol>@bookTicker`
**Update Speed**: Real-time

**Payload:**
```json
{
  "e": "bookTicker",
  "u": 400900217,
  "E": 1568014460893,
  "T": 1568014460891,
  "s": "BNBUSDT",
  "b": "25.35190000",
  "B": "31.21000000",
  "a": "25.36520000",
  "A": "40.66000000"
}
```

#### 7. Liquidation Order Streams

**Stream Name**: `<symbol>@forceOrder`
**Update Speed**: 1000ms

#### 8. Partial Book Depth Streams

**Stream Name**: `<symbol>@depth<levels>` or `<symbol>@depth<levels>@500ms` or `<symbol>@depth<levels>@100ms`
**Valid Levels**: 5, 10, or 20
**Update Speed**: 250ms, 500ms, or 100ms

#### 9. Diff. Book Depth Streams

**Stream Name**: `<symbol>@depth` or `<symbol>@depth@500ms` or `<symbol>@depth@100ms`
**Update Speed**: 250ms, 500ms, or 100ms

### How to Manage a Local Order Book Correctly

**Step-by-Step Process:**

1. **Open WebSocket Stream**
   - Connect to: `wss://fstream.asterdex.com/stream?streams=btcusdt@depth`

2. **Buffer Events**
   - Buffer all events received from the stream
   - For same price level, latest received update covers the previous one

3. **Get Depth Snapshot**
   - Fetch REST API: `https://fapi.asterdex.com/fapi/v1/depth?symbol=BTCUSDT&limit=1000`
   - Extract `lastUpdateId` from snapshot

4. **Drop Old Events**
   - Drop any buffered event where `u` < `lastUpdateId`

5. **Validate First Event**
   - The first processed event must satisfy: `U` <= `lastUpdateId` **AND** `u` >= `lastUpdateId`
   - This ensures no gaps between snapshot and stream

6. **Continuous Event Processing**
   - While listening to stream, each new event's `pu` should equal the previous event's `u`
   - If `pu` ≠ previous `u`, reinitialize from step 3 (get new snapshot)

7. **Apply Updates**
   - The data in each event represents **absolute** quantity for a price level
   - Update local order book with the new price/quantity pairs

8. **Remove Zero Quantities**
   - If quantity is `0`, **remove** that price level from local order book

9. **Handle Missing Price Levels**
   - Receiving an event that removes a non-existent price level is normal and can be ignored

**Key Field Explanations:**

- **`U`**: First update ID in the current event
- **`u`**: Final update ID in the current event
- **`pu`**: Final update ID from the previous event (previous `u` value)
- **`b`**: Bids array (buy orders)
- **`a`**: Asks array (sell orders)
- **`lastUpdateId`**: The update ID from the REST snapshot

---

## 6. Position and Mode Management

### 1. Change Position Mode (TRADE)

- **Endpoint:** `POST /fapi/v1/positionSide/dual`
- **Purpose:** Switch between One-way Mode and Hedge Mode
- **Parameters:**
  - `dualSidePosition` (STRING, REQUIRED):
    - `"true"` = Hedge Mode (allows separate LONG and SHORT positions)
    - `"false"` = One-way Mode (only one position per symbol)
  - `recvWindow` (LONG, OPTIONAL)
  - `timestamp` (LONG, REQUIRED)
- **Response:** Success message with code 200

**Position Mode Explanation:**
- **One-way Mode (Default):** Only allows one position per symbol. The `positionSide` defaults to "BOTH"
- **Hedge Mode:** Allows separate LONG and SHORT positions for the same symbol simultaneously

### 2. Get Current Position Mode (USER_DATA)

- **Endpoint:** `GET /fapi/v1/positionSide/dual`
- **Response:**
```json
{
  "dualSidePosition": true
}
```

### 3. Change Multi-Assets Mode (TRADE)

- **Endpoint:** `POST /fapi/v1/multiAssetsMargin`
- **Purpose:** Enable/disable Multi-Assets Mode for flexible margin management
- **Parameters:**
  - `multiAssetsMargin` (STRING, REQUIRED):
    - `"true"` = Multi-Assets Mode (margin across different assets)
    - `"false"` = Single-Asset Mode

### 4. Get Current Multi-Assets Mode (USER_DATA)

- **Endpoint:** `GET /fapi/v1/multiAssetsMargin`
- **Response:**
```json
{
  "multiAssetsMargin": true
}
```

### 5. Change Initial Leverage (TRADE)

- **Endpoint:** `POST /fapi/v1/leverage`
- **Parameters:**
  - `symbol` (STRING, REQUIRED)
  - `leverage` (INT, REQUIRED)
- **Response:**
```json
{
  "symbol": "BTCUSDT",
  "leverage": 21,
  "maxNotionalValue": "1000000"
}
```

### 6. Change Margin Type (TRADE)

- **Endpoint:** `POST /fapi/v1/marginType`
- **Purpose:** Switch between ISOLATED and CROSS margin modes
- **Parameters:**
  - `symbol` (STRING, REQUIRED)
  - `marginType` (ENUM, REQUIRED): `"ISOLATED"` or `"CROSSED"`

**Margin Type Explanation:**

1. **ISOLATED Margin:**
   - Margin is limited to a specific position
   - Risk is contained to the margin allocated to that position
   - Position will be liquidated if isolated margin is depleted

2. **CROSS/CROSSED Margin:**
   - Uses entire account balance to prevent liquidation
   - Margin is shared across all positions
   - More flexible but higher risk

### 7. Modify Isolated Position Margin (TRADE)

- **Endpoint:** `POST /fapi/v1/positionMargin`
- **Purpose:** Add or reduce margin for isolated positions
- **Parameters:**
  - `symbol` (STRING, REQUIRED)
  - `positionSide` (ENUM, OPTIONAL): `"LONG"`, `"SHORT"`, or `"BOTH"` (default: "BOTH")
  - `amount` (DECIMAL, REQUIRED)
  - `type` (INT, REQUIRED):
    - `1` = Add margin
    - `2` = Reduce margin
- **Response:**
```json
{
  "amount": 100,
  "code": 200,
  "msg": "Successfully add margin",
  "type": 1
}
```

### 8. Get Position Margin Change History (TRADE)

- **Endpoint:** `GET /fapi/v1/positionMargin`
- **Weight:** 5
- **Purpose:** Retrieve history of margin modifications

### 9. Position Information V3 (USER_DATA)

- **Endpoint:** `GET /fapi/v3/positionRisk`
- **Weight:** 5
- **Purpose:** Get comprehensive information about current positions
- **Parameters:**
  - `symbol` (STRING, OPTIONAL) - If omitted, returns all positions
- **Response:**
```json
[
  {
    "symbol": "BTCUSDT",
    "positionAmt": "0.087",
    "entryPrice": "9707.29",
    "breakEvenPrice": "9700.50",
    "markPrice": "9706.11",
    "unRealizedProfit": "-0.07729521",
    "liquidationPrice": "8570.3",
    "leverage": 20,
    "maxNotionalValue": "250000",
    "marginType": "isolated",
    "isolatedMargin": "9.12345678",
    "isAutoAddMargin": false,
    "positionSide": "BOTH",
    "notional": "845.83715468",
    "isolatedWallet": "9.12345678",
    "updateTime": 1625474304765
  }
]
```

---

## 7. Order Management

### 1. New Order (TRADE)

**Endpoint**: `POST /fapi/v1/order`
**Weight**: 1

**Required Parameters:**
- `symbol` (STRING)
- `side` (ENUM) - BUY or SELL
- `type` (ENUM) - Order type
- `timestamp` (LONG)

**Optional Parameters:**
- `positionSide` (ENUM) - BOTH (default), LONG, or SHORT
- `timeInForce` (ENUM) - GTC, IOC, FOK, GTX
- `quantity` (DECIMAL)
- `reduceOnly` (STRING) - "true" or "false"
- `price` (DECIMAL)
- `newClientOrderId` (STRING)
- `stopPrice` (DECIMAL)
- `closePosition` (STRING)
- `activationPrice` (DECIMAL)
- `callbackRate` (DECIMAL)
- `workingType` (ENUM) - MARK_PRICE or CONTRACT_PRICE
- `priceProtect` (STRING)
- `newOrderRespType` (ENUM) - ACK or RESULT

**Order Type Requirements:**

| Order Type | Required Parameters |
|------------|-------------------|
| `LIMIT` | `timeInForce`, `quantity`, `price` |
| `MARKET` | `quantity` |
| `STOP` / `TAKE_PROFIT` | `quantity`, `price`, `stopPrice` |
| `STOP_MARKET` / `TAKE_PROFIT_MARKET` | `stopPrice` |
| `TRAILING_STOP_MARKET` | `callbackRate` |

### 2. Place Multiple Orders (TRADE)

**Endpoint**: `POST /fapi/v1/batchOrders`
**Weight**: 5
**Maximum**: 5 orders per batch

**Parameters:**
- `batchOrders` (LIST<JSON>) - Array of order objects (max 5)

**Notes:**
- Batch orders are processed concurrently
- Individual orders can succeed or fail independently
- Response maintains the same order as the request

### 3. Query Order (USER_DATA)

**Endpoint**: `GET /fapi/v1/order`
**Weight**: 1

**Required:**
- `symbol` (STRING)
- `timestamp` (LONG)

**Either one required:**
- `orderId` (LONG)
- `origClientOrderId` (STRING)

**Important:** Orders will NOT be found if:
- Order status is `CANCELED` or `EXPIRED`, AND
- Order has NO filled trade, AND
- Created time + 7 days < current time

### 4. Cancel Order (TRADE)

**Endpoint**: `DELETE /fapi/v1/order`
**Weight**: 1

**Required:**
- `symbol` (STRING)
- `timestamp` (LONG)

**Either one required:**
- `orderId` (LONG)
- `origClientOrderId` (STRING)

### 5. Cancel All Open Orders (TRADE)

**Endpoint**: `DELETE /fapi/v1/allOpenOrders`
**Weight**: 1

**Required:**
- `symbol` (STRING)
- `timestamp` (LONG)

**Response:**
```json
{
  "code": "200",
  "msg": "The operation of cancel all open order is done."
}
```

### 6. Cancel Multiple Orders (TRADE)

**Endpoint**: `DELETE /fapi/v1/batchOrders`
**Weight**: 1
**Maximum**: 10 orders per batch

**Either one required:**
- `orderIdList` (LIST<LONG>) - Max 10
- `origClientOrderIdList` (LIST<STRING>) - Max 10

### 7. Auto-Cancel All Open Orders (TRADE)

**Endpoint**: `POST /fapi/v1/countdownCancelAll`
**Weight**: 10

**Parameters:**
- `symbol` (STRING, REQUIRED)
- `countdownTime` (LONG, REQUIRED) - Milliseconds (0 = cancel timer)

**Behavior:**
- Acts as a **deadman switch** for order management
- Must be called repeatedly as heartbeats to maintain the timer
- If not called within the countdown period, all orders for the symbol are automatically cancelled
- System checks countdowns approximately **every 10 milliseconds**

**Recommended Usage:**
Call endpoint every 30 seconds with `countdownTime` of 120000 (120 seconds) to maintain a 2-minute safety window.

### 8. Query Current Open Order (USER_DATA)

**Endpoint**: `GET /fapi/v1/openOrder`
**Weight**: 1

Returns error if the queried order has been filled or cancelled.

### 9. Current All Open Orders (USER_DATA)

**Endpoint**: `GET /fapi/v1/openOrders`
**Weight**:
- 1 for single symbol
- 40 when symbol is omitted (all symbols)

**Parameters:**
- `symbol` (STRING, OPTIONAL) - If omitted, returns orders for all symbols

### 10. All Orders (USER_DATA)

**Endpoint**: `GET /fapi/v1/allOrders`
**Weight**: 5

**Parameters:**
- `symbol` (STRING, REQUIRED)
- `orderId` (LONG, OPTIONAL) - If set, returns orders >= orderId
- `startTime` (LONG, OPTIONAL)
- `endTime` (LONG, OPTIONAL)
- `limit` (INT, OPTIONAL) - Default 500, max 1000

**Time Constraints:**
- Query time period must be less than 7 days
- Without `orderId`, returns most recent orders

### Order Lifecycle

**Order States:**
1. **NEW** - Order accepted by the system
2. **PARTIALLY_FILLED** - Order partially executed
3. **FILLED** - Order fully executed
4. **CANCELED** - Order cancelled
5. **REJECTED** - Order rejected
6. **EXPIRED** - Order expired

**Conditional Order Triggering:**

**STOP / STOP_MARKET Orders:**
- **BUY**: Triggered when latest price ≥ `stopPrice`
- **SELL**: Triggered when latest price ≤ `stopPrice`

**TAKE_PROFIT / TAKE_PROFIT_MARKET Orders:**
- **BUY**: Triggered when latest price ≤ `stopPrice`
- **SELL**: Triggered when latest price ≥ `stopPrice`

**TRAILING_STOP_MARKET Orders:**
- **BUY**:
  - Lowest price after placement ≤ `activationPrice`
  - Latest price ≥ lowest price × (1 + `callbackRate`)
- **SELL**:
  - Highest price after placement ≥ `activationPrice`
  - Latest price ≤ highest price × (1 - `callbackRate`)

---

## 8. Account and Balance Management

### 1. Transfer Between Futures And Spot (USER_DATA)

**Endpoint**: `POST /fapi/v3/asset/wallet/transfer`
**Weight**: 5

**Parameters:**
- `amount` (DECIMAL, REQUIRED)
- `asset` (STRING, REQUIRED) - e.g., "USDT"
- `clientTranId` (STRING, REQUIRED)
- `kindType` (STRING, REQUIRED):
  - `FUTURE_SPOT`: Convert futures to spot
  - `SPOT_FUTURE`: Convert spot to futures

**Response:**
```json
{
    "tranId": 21841,
    "status": "SUCCESS"
}
```

### 2. Futures Account Balance v3 (USER_DATA)

**Endpoint**: `GET /fapi/v3/balance`
**Weight**: 5

**Purpose:** Retrieve detailed account balance information for all assets

**Response:**
```json
[
    {
        "accountAlias": "SgsR",
        "asset": "USDT",
        "balance": "122607.35137903",
        "crossWalletBalance": "23.72469206",
        "crossUnPnl": "0.00000000",
        "availableBalance": "23.72469206",
        "maxWithdrawAmount": "23.72469206",
        "marginAvailable": true,
        "updateTime": 1617939110373
    }
]
```

### 3. Account Information v3 (USER_DATA)

**Endpoint**: `GET /fapi/v3/account`
**Weight**: 5

**Purpose:** Comprehensive account status including positions, balances, and trading configuration

**Response:**
```json
{
    "feeTier": 0,
    "canTrade": true,
    "canDeposit": true,
    "canWithdraw": true,
    "updateTime": 0,
    "totalInitialMargin": "0.00000000",
    "totalMaintMargin": "0.00000000",
    "totalWalletBalance": "23.72469206",
    "totalUnrealizedProfit": "0.00000000",
    "totalMarginBalance": "23.72469206",
    "totalPositionInitialMargin": "0.00000000",
    "totalOpenOrderInitialMargin": "0.00000000",
    "totalCrossWalletBalance": "23.72469206",
    "totalCrossUnPnl": "0.00000000",
    "availableBalance": "23.72469206",
    "maxWithdrawAmount": "23.72469206",
    "assets": [...]
}
```

### 4. Account Trade List (USER_DATA)

**Endpoint**: `GET /fapi/v1/userTrades`
**Weight**: 5

**Purpose:** Retrieve historical trades for a specific symbol

**Parameters:**
- `symbol` (STRING, REQUIRED)
- `startTime` (LONG, OPTIONAL)
- `endTime` (LONG, OPTIONAL)
- `fromId` (LONG, OPTIONAL)
- `limit` (INT, OPTIONAL) - Default: 500, max: 1000

**Response:**
```json
[
    {
        "buyer": false,
        "commission": "-0.07819010",
        "commissionAsset": "USDT",
        "id": 698759,
        "maker": false,
        "orderId": 25851813,
        "price": "7819.01",
        "qty": "0.002",
        "quoteQty": "15.63802",
        "realizedPnl": "-0.91539999",
        "side": "SELL",
        "positionSide": "SHORT",
        "symbol": "BTCUSDT",
        "time": 1569514978020
    }
]
```

### 5. Get Income History (USER_DATA)

**Endpoint**: `GET /fapi/v1/income`
**Weight**: 30

**Purpose:** Track all income and expense events including transfers, fees, P&L, and funding

**Parameters:**
- `symbol` (STRING, OPTIONAL)
- `incomeType` (STRING, OPTIONAL):
  - `TRANSFER`
  - `WELCOME_BONUS`
  - `REALIZED_PNL`
  - `FUNDING_FEE`
  - `COMMISSION`
  - `INSURANCE_CLEAR`
  - `MARKET_MERCHANT_RETURN_REWARD`
- `startTime`, `endTime` (LONG, OPTIONAL)
- `limit` (INT, OPTIONAL) - Default: 100, max: 1000

**Response:**
```json
[
    {
        "symbol": "",
        "incomeType": "TRANSFER",
        "income": "-0.37500000",
        "asset": "USDT",
        "info": "TRANSFER",
        "time": 1570608000000,
        "tranId": "9689322392",
        "tradeId": ""
    }
]
```

### 6. Notional and Leverage Brackets (USER_DATA)

**Endpoint**: `GET /fapi/v1/leverageBracket`
**Weight**: 1

**Purpose:** Retrieve leverage and notional value limits for symbols

**Response:**
```json
[
    {
        "symbol": "ETHUSDT",
        "brackets": [
            {
                "bracket": 1,
                "initialLeverage": 75,
                "notionalCap": 10000,
                "notionalFloor": 0,
                "maintMarginRatio": 0.0065,
                "cum": 0
            }
        ]
    }
]
```

### 7. Position ADL Quantile Estimation (USER_DATA)

**Endpoint**: `GET /fapi/v1/adlQuantile`
**Weight**: 5

**Purpose:** Estimate position's Auto-Deleveraging (ADL) queue position

**Response:**
```json
[
    {
        "symbol": "ETHUSDT",
        "adlQuantile": {
            "LONG": 3,
            "SHORT": 3,
            "HEDGE": 0
        }
    }
]
```

**Notes:**
- Values update every 30 seconds
- Values 0-4 indicate ADL likelihood from low to high

### 8. User's Force Orders (USER_DATA)

**Endpoint**: `GET /fapi/v1/forceOrders`
**Weight**: 20 (with symbol), 50 (without symbol)

**Purpose:** Retrieve liquidation and ADL orders executed on user's account

**Parameters:**
- `symbol` (STRING, OPTIONAL)
- `autoCloseType` (ENUM, OPTIONAL):
  - `LIQUIDATION`
  - `ADL`
- `startTime`, `endTime` (LONG, OPTIONAL)
- `limit` (INT, OPTIONAL) - Default: 50, max: 100

### 9. User Commission Rate (USER_DATA)

**Endpoint**: `GET /fapi/v1/commissionRate`
**Weight**: 20

**Purpose:** Retrieve current commission rates for a specific symbol

**Parameters:**
- `symbol` (STRING, REQUIRED)

**Response:**
```json
{
    "symbol": "BTCUSDT",
    "makerCommissionRate": "0.0002",
    "takerCommissionRate": "0.0004"
}
```

---

## 9. User Data Streams

### User Data Stream Lifecycle

#### 1. Start User Data Stream (USER_STREAM)

**Endpoint**: `POST /fapi/v1/listenKey`
**Weight**: 1

**Response:**
```json
{
  "listenKey": "pqia91ma19a5s61cv6a81va65sdf19v8a65a1a5s61cv6a81va65sdf19v8a65a1"
}
```

**Behavior:**
- Creates a new listenKey for WebSocket connection
- Valid for 60 minutes after creation
- If an account already has an active listenKey, that same listenKey is returned and its validity is extended for 60 minutes

#### 2. Keepalive User Data Stream (USER_STREAM)

**Endpoint**: `PUT /fapi/v1/listenKey`
**Weight**: 1

**Behavior:**
- Extends listenKey validity for 60 minutes
- Recommended to send a ping about every 60 minutes

#### 3. Close User Data Stream (USER_STREAM)

**Endpoint**: `DELETE /fapi/v1/listenKey`
**Weight**: 1

**Behavior:**
- Closes the WebSocket stream
- Invalidates the current listenKey

### Connection Rules

- A listenKey is valid for **60 minutes** after creation/extension
- A single WebSocket connection is only valid for **24 hours**
- User data stream payloads are **not guaranteed to be in order** during heavy periods
- **Always order updates using the `E` field (event time)**

### User Data Stream Events

#### Event 1: User Data Stream Expired

**Payload:**
```json
{
  "e": "listenKeyExpired",
  "E": 1576653824250
}
```

**When Triggered:**
- When the listenKey used for the user data stream turns expired

#### Event 2: Margin Call

**Payload:**
```json
{
  "e": "MARGIN_CALL",
  "E": 1587727187525,
  "cw": "3.16812045",
  "p": [
    {
      "s": "ETHUSDT",
      "ps": "LONG",
      "pa": "1.327",
      "mt": "CROSSED",
      "iw": "0",
      "mp": "187.17127",
      "up": "-1.166074",
      "mm": "1.614445"
    }
  ]
}
```

**When Triggered:**
- When the user's position risk ratio is too high

**Important:** This message is only used as **risk guidance information** and is not recommended for investment strategies. In highly volatile markets, the position may already be liquidated when this stream is pushed.

#### Event 3: Balance and Position Update

**Payload:**
```json
{
  "e": "ACCOUNT_UPDATE",
  "E": 1564745798939,
  "T": 1564745798938,
  "a": {
    "m": "ORDER",
    "B": [
      {
        "a": "USDT",
        "wb": "122624.12345678",
        "cw": "100.12345678",
        "bc": "50.12345678"
      }
    ],
    "P": [
      {
        "s": "BTCUSDT",
        "pa": "0",
        "ep": "0.00000",
        "cr": "200",
        "up": "0",
        "mt": "isolated",
        "iw": "0.00000000",
        "ps": "BOTH"
      }
    ]
  }
}
```

**When Triggered:**
- When balance or position gets updated

**Event Reason Types (`m` field):**
- DEPOSIT
- WITHDRAW
- ORDER
- FUNDING_FEE
- WITHDRAW_REJECT
- ADJUSTMENT
- INSURANCE_CLEAR
- ADMIN_DEPOSIT
- ADMIN_WITHDRAW
- MARGIN_TRANSFER
- MARGIN_TYPE_CHANGE
- ASSET_TRANSFER
- OPTIONS_PREMIUM_FEE
- OPTIONS_SETTLE_PROFIT
- AUTO_EXCHANGE

#### Event 4: Order Update

**Payload:**
```json
{
  "e": "ORDER_TRADE_UPDATE",
  "E": 1568879465651,
  "T": 1568879465650,
  "o": {
    "s": "BTCUSDT",
    "c": "TEST",
    "S": "SELL",
    "o": "TRAILING_STOP_MARKET",
    "f": "GTC",
    "q": "0.001",
    "p": "0",
    "ap": "0",
    "sp": "7103.04",
    "x": "NEW",
    "X": "NEW",
    "i": 8886774,
    "l": "0",
    "z": "0",
    "L": "0",
    "T": 1568879465651,
    "t": 0,
    "b": "0",
    "a": "9.91",
    "m": false,
    "R": false,
    "wt": "CONTRACT_PRICE",
    "ot": "TRAILING_STOP_MARKET",
    "ps": "LONG",
    "cp": false,
    "AP": "7476.89",
    "cr": "5.0",
    "rp": "0"
  }
}
```

**When Triggered:**
- When a new order is created
- When order status changes

**Execution Types:**
- NEW
- CANCELED
- CALCULATED (Liquidation Execution)
- EXPIRED
- TRADE

**Order Status:**
- NEW
- PARTIALLY_FILLED
- FILLED
- CANCELED
- EXPIRED
- NEW_INSURANCE (Liquidation with Insurance Fund)

#### Event 5: Account Configuration Update

**Payload (Leverage Change):**
```json
{
  "e": "ACCOUNT_CONFIG_UPDATE",
  "E": 1611646737479,
  "T": 1611646737476,
  "ac": {
    "s": "BTCUSDT",
    "l": 25
  }
}
```

**Payload (Multi-Assets Mode Change):**
```json
{
  "e": "ACCOUNT_CONFIG_UPDATE",
  "E": 1611646737479,
  "T": 1611646737476,
  "ai": {
    "j": true
  }
}
```

**When Triggered:**
- When account configuration is changed

### Key Recommendations

1. **Use WebSocket streams** for getting data to ensure timeliness
2. **Order updates using event time (`E`)** since payloads are not guaranteed to be in order
3. **Get order status from WebSocket** user data stream (recommended during volatile markets)
4. **Send keepalive** about every 60 minutes to prevent stream closure
5. **Handle 24-hour disconnection** and reconnect with a new listenKey

---

## 10. Error Handling

### Error Code Categories

#### 10xx - General Server or Network Issues

These errors relate to server connectivity, authentication, and network-level problems.

#### 11xx - Request Issues

**Documented Error Code:**
- **-1121**: "Invalid symbol" - The trading pair symbol provided is not recognized

#### 20xx - Processing Issues

**Documented Error Codes:**
- **-2011**: "Unknown order sent" - The order reference provided does not exist
- **-2021**: "Order would immediately trigger" - For `TRAILING_STOP_MARKET` orders:
  - For **BUY orders**: activation price must be smaller than the latest price
  - For **SELL orders**: activation price must be larger than the latest price
- **-2022**: "ReduceOnly Order is rejected" - Order would increase position size instead of reducing it

#### 40xx - Filters and Other Issues

**Documented Error Code:**
- **-4001**: "Invalid symbol" - Another variant for invalid trading pair symbols

### HTTP Status Codes

- **403**: Web Application Firewall (WAF) limit has been violated
- **418**: IP has been auto-banned for continuing to send requests after receiving 429 errors
- **429**: Request rate limit has been broken/exceeded
- **503**: The API successfully sent a message but did not receive a response within the timeout period

### Error Response Format

```json
{
  "code": -1121,
  "msg": "Invalid symbol."
}
```

### Recommended Error Handling Strategies

#### 1. Rate Limit Management
- When receiving a **429 error**, implement exponential backoff
- Respect weight-based request limits and order count limits
- Repeated violations can lead to automatic IP bans (418 error)

#### 2. Request Validation
- Validate all order parameters before submission
- Verify symbol validity
- Check price and quantity constraints
- Ensure position mode compatibility

#### 3. Timing and Security
- Use a small `recvWindow` value (recommended: 5000ms or less)
- Ensure proper timestamp synchronization
- Implement signature validation correctly

#### 4. Order-Specific Handling
- **Trailing Stop Market Orders**: Verify activation price meets directional requirements
- **Reduce-Only Orders**: Confirm the order will actually reduce position size
- Check position mode settings when placing orders

#### 5. General Best Practices
- Implement robust error catching and logging
- Monitor error responses and patterns
- Handle both expected and unexpected error scenarios
- Retry logic should respect rate limits and include backoff periods

---

## 11. Python Implementation Examples

### Required Dependencies

```python
# Python 3.9.6
# Required packages:
# eth-account~=0.13.7
# eth-abi~=5.2.0
# web3~=7.11.0
# requests~=2.32.3
```

### Complete Authentication Implementation

**Imports:**
```python
import json
import math
import time
import requests
from eth_abi import encode
from eth_account import Account
from eth_account.messages import encode_defunct
from web3 import Web3
```

**Configuration:**
```python
user = '0x63DD5aCC6b1aa0f563956C0e534DD30B6dcF7C4e'  # User address
signer = '0x21cF8Ae13Bb72632562c6Fff438652Ba1a151bb0'  # Signer address
priKey = "0x4fd0a42218f3eae43a6ce26d22544e986139a01e5b34a62db53757ffca81bae1"  # Private key
host = 'https://fapi.asterdex.com'
```

### Authentication Signature Function

```python
def sign(my_dict, nonce):
    # Remove None values and add required fields
    my_dict = {key: value for key, value in my_dict.items() if value is not None}
    my_dict['recvWindow'] = 50000
    my_dict['timestamp'] = int(round(time.time() * 1000))

    # Convert all values to strings
    def _trim_dict(my_dict):
        for key in my_dict:
            value = my_dict[key]
            my_dict[key] = str(value)
        return my_dict

    _trim_dict(my_dict)

    # Create sorted JSON string
    json_str = json.dumps(my_dict, sort_keys=True).replace(' ', '').replace('\'', '\"')

    # Encode using Web3 ABI encoding
    encoded = encode(['string', 'address', 'address', 'uint256'],
                     [json_str, user, signer, nonce])
    keccak_hex = Web3.keccak(encoded).hex()

    # Sign the hash
    signable_msg = encode_defunct(hexstr=keccak_hex)
    signed_message = Account.sign_message(signable_message=signable_msg, private_key=priKey)

    # Add authentication parameters
    my_dict['nonce'] = nonce
    my_dict['user'] = user
    my_dict['signer'] = signer
    my_dict['signature'] = '0x' + signed_message.signature.hex()

    return my_dict
```

### API Call Wrapper

```python
def call(api):
    # Generate unique nonce using microsecond timestamp
    nonce = math.trunc(time.time() * 1000000)
    my_dict = api['params']
    send(api['url'], api['method'], sign(my_dict, nonce))
```

### Common API Operations

**1. Place Order:**
```python
placeOrder = {
    'url': '/fapi/v3/order',
    'method': 'POST',
    'params': {
        'symbol': 'SANDUSDT',
        'positionSide': 'BOTH',
        'type': 'LIMIT',
        'side': 'BUY',
        'timeInForce': 'GTC',
        'quantity': "30",
        'price': 0.325,
        'reduceOnly': True
    }
}

call(placeOrder)
```

**2. Get Order:**
```python
getOrder = {
    'url': '/fapi/v3/order',
    'method': 'GET',
    'params': {
        'symbol': 'SANDUSDT',
        'side': "BUY",
        'type': 'LIMIT',
        'orderId': 2194215
    }
}

call(getOrder)
```

### Best Practices

1. **Authentication Security:**
   - Store private keys securely, never commit to version control
   - Use environment variables for sensitive credentials
   - Generate unique nonce for each request

2. **Request Handling:**
   - Set appropriate `recvWindow` (50000 ms recommended)
   - Always include timestamp with requests
   - Sort parameters in ASCII order before signing

3. **Parameter Management:**
   - Remove None/null values before signing
   - Use exact parameter names as specified in API docs
   - Convert all values to strings for encoding

4. **Error Handling:**
   - Implement retry logic for network errors
   - Respect rate limits
   - Validate responses before processing

### Key Authentication Flow Summary

1. **Prepare Parameters:** Remove null values, add recvWindow and timestamp
2. **String Conversion:** Convert all parameter values to strings
3. **JSON Encoding:** Create sorted JSON string (no spaces, double quotes)
4. **ABI Encoding:** Encode [json_str, user_address, signer_address, nonce]
5. **Hash Generation:** Create Keccak-256 hash of encoded data
6. **Signature:** Sign the hash using eth_account with private key
7. **Request Assembly:** Add nonce, user, signer, and signature to parameters
8. **API Call:** Send authenticated request to endpoint

---

## 12. Comparison with Binance Futures API

### Overview

Aster DEX has deliberately designed its API to be **highly compatible with Binance Futures API**, making migration easier for developers already familiar with Binance's ecosystem.

### Similarities

#### Endpoint Structure
- **Binance**: `https://fapi.binance.com`
- **Aster DEX**: `https://fapi.asterdex.com`
- Both use `/fapi/v1` and `/fapi/v3` path structure

#### WebSocket Endpoints
- **Binance**: `wss://fstream.binance.com`
- **Aster DEX**: `wss://fstream.asterdex.com`
- Both connections valid for 24 hours

#### Common Endpoints
- `/fapi/v1/ping` - Test connectivity
- `/fapi/v1/time` - Get server time
- `/fapi/v1/exchangeInfo` - Exchange information
- `/fapi/v1/depth` - Order book
- `/fapi/v1/trades` - Recent trades
- `/fapi/v1/order` - Place/query orders
- `/fapi/v1/openOrders` - Get all open orders

#### Rate Limiting
- Same header naming: `X-MBX-USED-WEIGHT-(intervalNum)(intervalLetter)`
- Same error codes: HTTP 429, 418
- Similar IP ban scaling (2 minutes to 3 days)

### Key Differences

#### Authentication (Most Significant Difference)

**Binance Futures:**
- **Method**: HMAC SHA256 signature
- **Parameters**: `apiKey`, `signature`, `timestamp`, `recvWindow`
- **Signature**: Generated using secretKey

**Aster DEX:**
- **Method**: Web3 EVM-style Keccak signature
- **Parameters**: `user`, `signer`, `nonce`, `signature`
- **Signature**: Generated using blockchain wallet private key

| Aspect | Binance | Aster DEX |
|--------|---------|-----------|
| **Algorithm** | HMAC SHA256 | Keccak (Web3) |
| **Key Type** | API Keys | Wallet Keys |
| **Timestamp** | Milliseconds | Microseconds |
| **Identity** | API Key/Secret | Wallet addresses |
| **Custody** | Custodial | Non-custodial |

### Unique Aster DEX Features

#### 1. Hidden Orders
- **Complete invisibility**: Orders completely hidden from public order book
- **No information leakage**: Price, quantity, and direction not visible
- **Direct matching**: Orders go directly to matching engine
- **Benefits**: Reduces slippage, protects strategies, minimizes front-running

**Praised by CZ (Changpeng Zhao)** - Former Binance CEO highlighted this feature

#### 2. Grid Trading API
- **Automated strategy**: Set price levels in advance
- **Auto-execution**: System automatically buys on drops and sells on rises
- **Native support**: Available through API

#### 3. Ultra-High Leverage
- **Up to 1001x leverage** available in Pro Mode

#### 4. Cross-Chain Support
- **Multiple blockchains**: BNB Chain, Ethereum, Solana, Arbitrum
- **Aster Chain**: Proprietary chain for high-performance trading
- **Zero-knowledge settlement**: Enhanced privacy

#### 5. MEV Protection
- **MEV-free execution**: Protection from Maximal Extractable Value attacks

#### 6. Non-Custodial Architecture
- **Self-custody**: Users link private wallets
- **Smart contract execution**: Trades handled by smart contracts
- **Withdraw anytime**: No approval needed

### Migration Considerations

#### Easy Migration Path

**Pros:**
- Endpoint structure nearly identical
- Similar error handling
- Same parameter patterns
- WebSocket compatibility
- 80-90% of code can remain unchanged

**Critical Changes Required:**

1. **Authentication Layer** (High Priority):
   - Replace HMAC SHA256 with Keccak/Web3 signature
   - Integrate Web3 libraries (ethers.js, web3.js)
   - Update signature logic
   - Change timestamp precision (ms to µs)
   - Add wallet management

**Code Structure Changes:**
```javascript
// Binance Style
const signature = crypto.createHmac('sha256', secretKey)
  .update(queryString)
  .digest('hex');

// Aster DEX Style
const params = sortParamsInASCII(parameters);
const encoded = web3.eth.abi.encodeParameters(types, params);
const hash = web3.utils.keccak256(encoded);
const signature = web3.eth.accounts.sign(hash, privateKey);
```

#### Migration Effort Estimate

- **Public endpoints**: 1-2 days (low effort)
- **Authentication**: 3-5 days (medium effort)
- **Order/balance endpoints**: 1-2 days (low effort)
- **WebSocket**: 2-3 days (medium effort)
- **Testing**: 1-2 days (low effort)

**Total**: ~7-14 days for full migration

### When to Choose Aster DEX

✅ **Good fit if:**
- Need non-custodial trading
- Want hidden orders
- Operating in DeFi ecosystem
- Want cross-chain flexibility
- Need automated grid trading
- Prefer blockchain transparency

❌ **Consider Binance if:**
- Need maximum liquidity
- Prefer fiat on/off ramps
- Want simpler authentication
- Need lowest latency
- Prefer centralized custody

### Final Recommendation

Aster DEX provides an excellent migration path for teams already using Binance Futures API. The deliberate API compatibility minimizes migration costs while providing unique DeFi advantages like non-custodial trading, hidden orders, and MEV protection. The main investment is implementing Web3 authentication, but this unlocks the benefits of decentralized trading with relatively low migration barriers.

---

## Summary

Aster DEX Futures API v3 is a comprehensive, Binance-compatible API for decentralized perpetual futures trading. Key highlights:

- **Web3-native authentication** using blockchain wallets
- **High compatibility** with Binance Futures API (80-90% code reuse)
- **Unique features**: Hidden orders, grid trading, MEV protection
- **Non-custodial**: Users maintain control of funds
- **Cross-chain support**: Multiple blockchains supported
- **Comprehensive**: Full REST and WebSocket APIs for trading, positions, and account management

For implementation, the main challenge is adapting authentication from HMAC to Web3 signatures. Once this is addressed, the rest of the integration follows familiar patterns for developers experienced with centralized exchange APIs.
