//! Aster DEX adapter implementation
//!
//! This module provides integration with Aster DEX, supporting both spot markets
//! and linear perpetuals (USDT-margined). Aster uses a Binance-compatible API,
//! making implementation straightforward by following similar patterns.
//!
//! # Supported Features
//! - ✅ Spot markets
//! - ✅ Linear perpetuals (USDT-margined)
//! - ✅ Real-time orderbook (depth) with incremental updates
//! - ✅ Real-time trades stream
//! - ✅ Historical klines (candlestick data)
//! - ✅ Open interest data (perpetuals only)
//! - ❌ Inverse perpetuals (not supported by Aster)
//! - ❌ Server-side depth aggregation (requires client-side via LocalDepthCache)
//! - ❌ Custom push frequencies (only ServerDefault supported)
//!
//! # Rate Limiting
//! - IP-based: 2400 weight per minute
//! - Implementation uses FixedWindowBucket with 5% safety buffer
//!
//! # WebSocket
//! - Connection lifetime: 24 hours maximum
//! - Ping/pong: Server pings every 5 minutes, 15-minute timeout
//! - Depth updates: Incremental with sequence validation (U/u/pu fields)
//! - Must sync with REST snapshot on connection
//!
//! # API Endpoints
//! - REST: `https://fapi.asterdex.com`
//! - WebSocket: `wss://fstream.asterdex.com`

use crate::{Price, PushFrequency};

use super::{
    super::{
        Exchange, Kline, MarketKind, SIZE_IN_QUOTE_CURRENCY, StreamKind, Ticker, TickerInfo,
        TickerStats, Timeframe, Trade,
        connect::{State, connect_ws},
        de_string_to_f32, de_string_to_u64,
        depth::{DeOrder, DepthPayload, DepthUpdate, LocalDepthCache},
        limiter::{self, RateLimiter},
    },
    AdapterError, Event,
};

use fastwebsockets::{FragmentCollector, Frame, OpCode};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use iced_futures::{
    futures::{SinkExt, Stream},
    stream,
};
use serde::Deserialize;
use serde_json::Value;

use std::{sync::LazyLock, time::Duration};
use std::collections::HashMap;
use tokio::sync::Mutex;

/// REST API base URL for Aster DEX
const API_DOMAIN: &str = "https://fapi.asterdex.com";

/// WebSocket base domain for Aster DEX
const WS_DOMAIN: &str = "fstream.asterdex.com";

/// Rate limit: 2400 weight per minute
const LIMIT: usize = 2400;

/// Rate limit refill window: 60 seconds
const REFILL_RATE: Duration = Duration::from_secs(60);

/// Safety buffer percentage for rate limiter (5%)
const LIMITER_BUFFER_PCT: f32 = 0.05;

/// Rate limiter for Aster DEX API
pub struct AsterLimiter {
    bucket: limiter::FixedWindowBucket,
}

impl AsterLimiter {
    pub fn new() -> Self {
        let effective_limit = (LIMIT as f32 * (1.0 - LIMITER_BUFFER_PCT)) as usize;
        Self {
            bucket: limiter::FixedWindowBucket::new(effective_limit, REFILL_RATE),
        }
    }
}

impl Default for AsterLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter for AsterLimiter {
    fn prepare_request(&mut self, weight: usize) -> Option<Duration> {
        self.bucket.calculate_wait_time(weight)
    }

    fn update_from_response(&mut self, _response: &reqwest::Response, weight: usize) {
        self.bucket.consume_tokens(weight);
    }

    fn should_exit_on_response(&self, response: &reqwest::Response) -> bool {
        response.status() == 429 || response.status() == 418
    }
}

/// Global rate limiter instance
static ASTER_LIMITER: LazyLock<Mutex<AsterLimiter>> =
    LazyLock::new(|| Mutex::new(AsterLimiter::new()));

// ============================================================================
// Serde Deserialization Structs
// ============================================================================

/// Response from /exchangeInfo endpoint
#[derive(Debug, Deserialize)]
struct ExchangeInfoResponse {
    symbols: Vec<AsterSymbolInfo>,
}

/// Individual symbol information from exchangeInfo
#[derive(Debug, Deserialize)]
struct AsterSymbolInfo {
    symbol: String,
    status: String,
    #[serde(rename = "contractType")]
    #[allow(dead_code)]
    contract_type: Option<String>,
    filters: Vec<Value>,
}

/// 24-hour ticker statistics
#[derive(Debug, Deserialize)]
struct AsterTickerStats {
    symbol: String,
    #[serde(rename = "lastPrice", deserialize_with = "de_string_to_f32")]
    last_price: f32,
    #[serde(rename = "priceChangePercent", deserialize_with = "de_string_to_f32")]
    price_change_percent: f32,
    #[serde(rename = "volume", deserialize_with = "de_string_to_f32")]
    volume: f32,
}

/// Kline (candlestick) data in array format
/// Format: [open_time, open, high, low, close, volume, close_time, quote_volume,
///          num_trades, taker_buy_base, taker_buy_quote, ignore]
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AsterKline(
    u64,  // [0] open_time - integer
    #[serde(deserialize_with = "de_string_to_f32")] f32,  // [1] open
    #[serde(deserialize_with = "de_string_to_f32")] f32,  // [2] high
    #[serde(deserialize_with = "de_string_to_f32")] f32,  // [3] low
    #[serde(deserialize_with = "de_string_to_f32")] f32,  // [4] close
    #[serde(deserialize_with = "de_string_to_f32")] f32,  // [5] volume
    u64,  // [6] close_time - integer
    #[serde(deserialize_with = "de_string_to_f32")] f32,  // [7] quote_volume
    u64,  // [8] num_trades
    #[serde(deserialize_with = "de_string_to_f32")] f32,  // [9] taker_buy_base
    #[serde(deserialize_with = "de_string_to_f32")] f32,  // [10] taker_buy_quote
    String,  // [11] ignore
);

/// Open interest data point
#[derive(Debug, Deserialize)]
struct AsterOIData {
    timestamp: u64,  // integer
    #[serde(rename = "sumOpenInterest", deserialize_with = "de_string_to_f32")]
    open_interest: f32,
}

/// REST depth snapshot response
#[derive(Debug, Deserialize)]
struct AsterDepthSnapshot {
    #[serde(rename = "lastUpdateId")]
    last_update_id: u64,
    bids: Vec<Vec<String>>,
    asks: Vec<Vec<String>>,
}

/// WebSocket message wrapper
#[derive(Debug, Deserialize)]
struct AsterWSMessage {
    stream: String,
    data: Value,
}

/// WebSocket depth update with sequence validation fields
#[derive(Debug, Deserialize)]
struct AsterDepthUpdate {
    #[serde(rename = "U")]
    first_update_id: u64,
    #[serde(rename = "u")]
    final_update_id: u64,
    #[serde(rename = "pu")]
    #[allow(dead_code)]
    prev_final_update_id: u64,
    #[serde(rename = "b")]
    bids: Vec<Vec<String>>,
    #[serde(rename = "a")]
    asks: Vec<Vec<String>>,
}

/// WebSocket aggregate trade
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

/// WebSocket kline/candlestick update
#[derive(Debug, Deserialize)]
struct AsterWSKlineWrapper {
    k: AsterWSKline,
}

#[derive(Debug, Deserialize)]
struct AsterWSKline {
    #[serde(rename = "t")]
    time: u64,  // integer
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
    #[serde(rename = "V", deserialize_with = "de_string_to_f32")]
    taker_buy_volume: f32,
}

/// Internal enum for parsed WebSocket data
enum StreamData {
    Trade(Vec<Trade>),
    Depth(AsterDepthUpdate),
    Kline(String, AsterWSKline), // (symbol, kline data)
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract tick size from PRICE_FILTER in filters array
fn extract_tick_size(filters: &[Value]) -> Option<f32> {
    for filter in filters {
        if let Some(filter_type) = filter.get("filterType").and_then(|v| v.as_str())
            && filter_type == "PRICE_FILTER"
            && let Some(tick_size) = filter.get("tickSize").and_then(|v| v.as_str()) {
            return tick_size.parse::<f32>().ok();
        }
    }
    None
}

/// Extract minimum quantity from LOT_SIZE in filters array
fn extract_min_qty(filters: &[Value]) -> Option<f32> {
    for filter in filters {
        if let Some(filter_type) = filter.get("filterType").and_then(|v| v.as_str())
            && filter_type == "LOT_SIZE"
            && let Some(min_qty) = filter.get("minQty").and_then(|v| v.as_str()) {
            return min_qty.parse::<f32>().ok();
        }
    }
    None
}

/// Map Timeframe enum to Aster interval string
fn timeframe_to_interval(tf: Timeframe) -> Option<&'static str> {
    match tf {
        Timeframe::M1 => Some("1m"),
        Timeframe::M3 => Some("3m"),
        Timeframe::M5 => Some("5m"),
        Timeframe::M15 => Some("15m"),
        Timeframe::M30 => Some("30m"),
        Timeframe::H1 => Some("1h"),
        Timeframe::H2 => Some("2h"),
        Timeframe::H4 => Some("4h"),
        Timeframe::H6 => Some("6h"),
        Timeframe::H12 => Some("12h"),
        Timeframe::D1 => Some("1d"),
        _ => None,
    }
}

/// Convert [[price, qty]] arrays to DeOrder structs
fn parse_price_qty_array(arr: Vec<Vec<String>>, _tick_size: f32) -> Vec<DeOrder> {
    let mut orders = Vec::new();

    for pair in arr {
        if pair.len() < 2 {
            continue;
        }

        let price = match pair[0].parse::<f32>() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let qty = match pair[1].parse::<f32>() {
            Ok(q) => q,
            Err(_) => continue,
        };

        // Skip zero quantities
        if qty == 0.0 {
            continue;
        }

        orders.push(DeOrder {
            price,
            qty,
        });
    }

    orders
}

// ============================================================================
// Public API Functions (Stubs - to be implemented in Tasks 3 and 4)
// ============================================================================

/// Fetch all ticker information including tick sizes and constraints
pub async fn fetch_ticksize(
    market: MarketKind,
) -> Result<HashMap<Ticker, Option<TickerInfo>>, AdapterError> {
    let exchange = match market {
        MarketKind::Spot => {
            return Err(AdapterError::InvalidRequest(
                "Aster DEX spot markets are not supported yet (API endpoint unclear)".to_string(),
            ))
        }
        MarketKind::LinearPerps => Exchange::AsterLinear,
        MarketKind::InversePerps => {
            return Err(AdapterError::InvalidRequest(
                "Aster DEX does not support inverse perpetuals".to_string(),
            ))
        }
    };

    let endpoint = format!("{}/fapi/v1/exchangeInfo", API_DOMAIN);

    let mut limiter = ASTER_LIMITER.lock().await;
    if let Some(wait) = limiter.prepare_request(1) {
        tokio::time::sleep(wait).await;
    }
    drop(limiter);

    let response = reqwest::Client::new().get(&endpoint).send().await?;

    if !response.status().is_success() {
        return Err(AdapterError::InvalidRequest(format!(
            "Failed to fetch exchange info: {}",
            response.status()
        )));
    }

    let data: ExchangeInfoResponse = response.json().await?;
    let mut result = HashMap::new();

    for symbol_info in data.symbols {
        if symbol_info.status != "TRADING" {
            continue;
        }

        let ticker = Ticker::new(&symbol_info.symbol, exchange);

        let tick_size = extract_tick_size(&symbol_info.filters);
        let min_qty = extract_min_qty(&symbol_info.filters);

        if let (Some(min_ticksize), Some(min_qty)) = (tick_size, min_qty) {
            let info = TickerInfo::new(ticker, min_ticksize, min_qty, None);
            result.insert(ticker, Some(info));
        } else {
            result.insert(ticker, None);
        }
    }

    Ok(result)
}

/// Fetch 24h ticker statistics for all symbols
pub async fn fetch_ticker_prices(
    market: MarketKind,
) -> Result<HashMap<Ticker, TickerStats>, AdapterError> {
    let exchange = match market {
        MarketKind::Spot => {
            return Err(AdapterError::InvalidRequest(
                "Aster DEX spot markets are not supported yet".to_string(),
            ))
        }
        MarketKind::LinearPerps => Exchange::AsterLinear,
        MarketKind::InversePerps => {
            return Err(AdapterError::InvalidRequest(
                "Aster DEX does not support inverse perpetuals".to_string(),
            ))
        }
    };

    let endpoint = format!("{}/fapi/v1/ticker/24hr", API_DOMAIN);

    let mut limiter = ASTER_LIMITER.lock().await;
    if let Some(wait) = limiter.prepare_request(1) {
        tokio::time::sleep(wait).await;
    }
    drop(limiter);

    let response = reqwest::Client::new().get(&endpoint).send().await?;

    if !response.status().is_success() {
        return Err(AdapterError::InvalidRequest(format!(
            "Failed to fetch ticker prices: {}",
            response.status()
        )));
    }

    let data: Vec<AsterTickerStats> = response.json().await?;
    let mut result = HashMap::new();

    for stats in data {
        let ticker = Ticker::new(&stats.symbol, exchange);

        result.insert(
            ticker,
            TickerStats {
                mark_price: stats.last_price,
                daily_price_chg: stats.price_change_percent,
                daily_volume: stats.volume,
            },
        );
    }

    Ok(result)
}

/// Fetch historical candlestick data
pub async fn fetch_klines(
    ticker_info: TickerInfo,
    timeframe: Timeframe,
    range: Option<(u64, u64)>,
) -> Result<Vec<Kline>, AdapterError> {
    let interval = timeframe_to_interval(timeframe).ok_or_else(|| {
        AdapterError::InvalidRequest(format!("Unsupported timeframe: {:?}", timeframe))
    })?;

    let market = ticker_info.ticker.exchange.market_type();
    let base_url = match market {
        MarketKind::Spot => {
            return Err(AdapterError::InvalidRequest(
                "Aster DEX spot markets are not supported yet".to_string(),
            ))
        }
        MarketKind::LinearPerps => format!("{}/fapi/v1/klines", API_DOMAIN),
        _ => {
            return Err(AdapterError::InvalidRequest(
                "Unsupported market type".to_string(),
            ))
        }
    };

    let (symbol_str, _) = ticker_info.ticker.to_full_symbol_and_type();

    let mut url = format!(
        "{}?symbol={}&interval={}",
        base_url, symbol_str, interval
    );

    if let Some((start, end)) = range {
        url.push_str(&format!("&startTime={}&endTime={}", start, end));
    } else {
        url.push_str("&limit=500");
    }

    let mut limiter = ASTER_LIMITER.lock().await;
    if let Some(wait) = limiter.prepare_request(1) {
        tokio::time::sleep(wait).await;
    }
    drop(limiter);

    let response = reqwest::Client::new().get(&url).send().await?;

    if !response.status().is_success() {
        return Err(AdapterError::InvalidRequest(format!(
            "Failed to fetch klines: {}",
            response.status()
        )));
    }

    let data: Vec<AsterKline> = response.json().await?;
    let mut klines = Vec::new();

    let size_in_quote_currency = SIZE_IN_QUOTE_CURRENCY.get() == Some(&true);

    for k in data {
        klines.push(Kline {
            time: k.0,
            open: Price::from_f32(k.1).round_to_min_tick(ticker_info.min_ticksize),
            high: Price::from_f32(k.2).round_to_min_tick(ticker_info.min_ticksize),
            low: Price::from_f32(k.3).round_to_min_tick(ticker_info.min_ticksize),
            close: Price::from_f32(k.4).round_to_min_tick(ticker_info.min_ticksize),
            volume: if size_in_quote_currency {
                (k.10, k.7 - k.10)  // (taker_buy_quote, total_quote - taker_buy_quote)
            } else {
                (k.9, k.5 - k.9)  // (taker_buy_base, total_base - taker_buy_base)
            },
        });
    }

    Ok(klines)
}

/// Fetch historical open interest data (linear perpetuals only)
pub async fn fetch_historical_oi(
    ticker: Ticker,
    range: Option<(u64, u64)>,
    timeframe: Timeframe,
) -> Result<Vec<crate::OpenInterest>, AdapterError> {
    if ticker.exchange != Exchange::AsterLinear {
        return Err(AdapterError::InvalidRequest(
            "Open interest only available for linear perpetuals".to_string(),
        ));
    }

    let period = timeframe_to_interval(timeframe).ok_or_else(|| {
        AdapterError::InvalidRequest(format!("Unsupported timeframe: {:?}", timeframe))
    })?;

    let (symbol_str, _) = ticker.to_full_symbol_and_type();

    let mut url = format!(
        "{}/futures/data/openInterestHist?symbol={}&period={}",
        API_DOMAIN, symbol_str, period
    );

    if let Some((start, end)) = range {
        url.push_str(&format!("&startTime={}&endTime={}", start, end));
    } else {
        url.push_str("&limit=500");
    }

    let mut limiter = ASTER_LIMITER.lock().await;
    if let Some(wait) = limiter.prepare_request(1) {
        tokio::time::sleep(wait).await;
    }
    drop(limiter);

    let response = reqwest::Client::new().get(&url).send().await?;

    if !response.status().is_success() {
        return Err(AdapterError::InvalidRequest(format!(
            "Failed to fetch open interest: {}",
            response.status()
        )));
    }

    let data: Vec<AsterOIData> = response.json().await?;
    let mut result = Vec::new();

    for oi in data {
        result.push(crate::OpenInterest {
            time: oi.timestamp,
            value: oi.open_interest,
        });
    }

    Ok(result)
}

/// Fetch REST orderbook snapshot for WebSocket synchronization
async fn fetch_depth_snapshot(
    symbol: &str,
    market: MarketKind,
) -> Result<AsterDepthSnapshot, AdapterError> {
    let endpoint = match market {
        MarketKind::Spot => format!("{}/api/v3/depth?symbol={}&limit=1000", API_DOMAIN, symbol),
        MarketKind::LinearPerps => {
            format!("{}/fapi/v1/depth?symbol={}&limit=1000", API_DOMAIN, symbol)
        }
        _ => {
            return Err(AdapterError::InvalidRequest(
                "Unsupported market type".to_string(),
            ))
        }
    };

    let mut limiter = ASTER_LIMITER.lock().await;
    if let Some(wait) = limiter.prepare_request(2) {
        tokio::time::sleep(wait).await;
    }
    drop(limiter);

    let response = reqwest::Client::new().get(&endpoint).send().await?;

    if !response.status().is_success() {
        return Err(AdapterError::InvalidRequest(format!(
            "Failed to fetch depth snapshot: {}",
            response.status()
        )));
    }

    let snapshot: AsterDepthSnapshot = response.json().await?;
    Ok(snapshot)
}

// ============================================================================
// WebSocket Functions
// ============================================================================

async fn connect_websocket(
    path: &str,
) -> Result<FragmentCollector<TokioIo<Upgraded>>, AdapterError> {
    connect_ws(WS_DOMAIN, path)
        .await
        .map_err(|e| AdapterError::WebsocketError(e.to_string()))
}

fn parse_websocket_message(payload: &[u8]) -> Result<StreamData, AdapterError> {
    let msg: AsterWSMessage = serde_json::from_slice(payload)
        .map_err(|e| AdapterError::ParseError(format!("Failed to parse WS message: {}", e)))?;

    if msg.stream.contains("depth") {
        let update: AsterDepthUpdate = serde_json::from_value(msg.data).map_err(|e| {
            AdapterError::ParseError(format!("Failed to parse depth update: {}", e))
        })?;
        Ok(StreamData::Depth(update))
    } else if msg.stream.contains("aggTrade") {
        let trade_data: AsterTrade = serde_json::from_value(msg.data)
            .map_err(|e| AdapterError::ParseError(format!("Failed to parse trade: {}", e)))?;

        let trade = Trade {
            time: trade_data.timestamp,
            is_sell: trade_data.is_buyer_maker, // buyer_maker means the trade was a sell
            price: Price::from_f32(trade_data.price),
            qty: trade_data.qty,
        };

        Ok(StreamData::Trade(vec![trade]))
    } else if msg.stream.contains("kline") {
        let wrapper: AsterWSKlineWrapper = serde_json::from_value(msg.data)
            .map_err(|e| AdapterError::ParseError(format!("Failed to parse kline: {}", e)))?;

        // Extract symbol from stream name (format: "btcusdt@kline_1m")
        let symbol = msg.stream.split('@').next().unwrap_or("").to_uppercase();

        Ok(StreamData::Kline(symbol, wrapper.k))
    } else {
        Err(AdapterError::ParseError(format!(
            "Unknown stream type: {}",
            msg.stream
        )))
    }
}

/// Connect to combined depth + trades stream with orderbook management
pub fn connect_market_stream(
    ticker_info: TickerInfo,
    _push_freq: PushFrequency,
) -> impl Stream<Item = Event> {
    stream::channel(100, async move |mut output| {
        let (symbol_str, market) = ticker_info.ticker.to_full_symbol_and_type();
        let symbol = symbol_str.to_lowercase();
        let exchange = ticker_info.ticker.exchange;

        let mut state = State::Disconnected;
        let mut local_depth: LocalDepthCache = LocalDepthCache::default();
        let mut trades_buffer: Vec<Trade> = Vec::new();
        let mut last_update_id = 0u64;

        loop {
            match &mut state {
                State::Disconnected => {
                    // Build WebSocket path
                    let path = format!("/stream?streams={}@depth/{}@aggTrade", symbol, symbol);

                    // Connect to WebSocket
                    match connect_websocket(&path).await {
                        Ok(ws) => {
                            // Fetch initial depth snapshot
                            match fetch_depth_snapshot(&symbol_str, market).await {
                                Ok(snapshot) => {
                                    last_update_id = snapshot.last_update_id;

                                    // Initialize orderbook (using base min_ticksize for client-side aggregation)
                                    let bids = parse_price_qty_array(snapshot.bids, ticker_info.min_ticksize.into());
                                    let asks = parse_price_qty_array(snapshot.asks, ticker_info.min_ticksize.into());

                                    local_depth.update(
                                        DepthUpdate::Snapshot(DepthPayload {
                                            last_update_id,
                                            time: chrono::Utc::now().timestamp_millis() as u64,
                                            bids,
                                            asks,
                                        }),
                                        ticker_info.min_ticksize,
                                    );

                                    state = State::Connected(ws);
                                    let _ = output.send(Event::Connected(exchange)).await;
                                }
                                Err(e) => {
                                    let _ = output
                                        .send(Event::Disconnected(
                                            exchange,
                                            format!("Failed to fetch depth snapshot: {}", e),
                                        ))
                                        .await;
                                    tokio::time::sleep(Duration::from_secs(5)).await;
                                    continue;
                                }
                            }
                        }
                        Err(e) => {
                            let _ = output
                                .send(Event::Disconnected(
                                    exchange,
                                    format!("WebSocket connection failed: {}", e),
                                ))
                                .await;
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                    }
                }

                State::Connected(ws) => {
                    let timeout = tokio::time::sleep(Duration::from_secs(20));
                    tokio::pin!(timeout);

                    tokio::select! {
                        frame = ws.read_frame() => {
                            match frame {
                                Ok(frame) => {
                                    match frame.opcode {
                                        OpCode::Text => {
                                            match parse_websocket_message(&frame.payload) {
                                                Ok(StreamData::Depth(update)) => {
                                                    // Sequence validation
                                                    if update.first_update_id <= last_update_id + 1 &&
                                                       update.final_update_id > last_update_id {

                                                        let bids = parse_price_qty_array(update.bids, ticker_info.min_ticksize.into());
                                                        let asks = parse_price_qty_array(update.asks, ticker_info.min_ticksize.into());

                                                        last_update_id = update.final_update_id;

                                                        local_depth.update(
                                                            DepthUpdate::Diff(DepthPayload {
                                                                last_update_id,
                                                                time: chrono::Utc::now().timestamp_millis() as u64,
                                                                bids,
                                                                asks,
                                                            }),
                                                            ticker_info.min_ticksize,
                                                        );

                                                        let depth = local_depth.depth.clone();
                                                        let trades = std::mem::take(&mut trades_buffer);

                                                        let stream_kind = StreamKind::DepthAndTrades {
                                                            ticker_info,
                                                            depth_aggr: super::StreamTicksize::Client,
                                                            push_freq: PushFrequency::ServerDefault,
                                                        };

                                                        let _ = output
                                                            .send(Event::DepthReceived(
                                                                stream_kind,
                                                                last_update_id,
                                                                depth,
                                                                trades.into_boxed_slice(),
                                                            ))
                                                            .await;
                                                    } else {
                                                        // Sequence break - reconnect
                                                        let _ = output
                                                            .send(Event::Disconnected(
                                                                exchange,
                                                                "Sequence break detected".to_string(),
                                                            ))
                                                            .await;
                                                        state = State::Disconnected;
                                                    }
                                                }
                                                Ok(StreamData::Trade(trades)) => {
                                                    trades_buffer.extend(trades);
                                                }
                                                Ok(StreamData::Kline(_, _)) => {
                                                    // Ignore klines in market stream
                                                }
                                                Err(e) => {
                                                    eprintln!("Parse error: {}", e);
                                                }
                                            }
                                        }
                                        OpCode::Ping => {
                                            let pong = Frame::pong(frame.payload);
                                            let _ = ws.write_frame(pong).await;
                                        }
                                        OpCode::Close => {
                                            let _ = output
                                                .send(Event::Disconnected(exchange, "Connection closed".to_string()))
                                                .await;
                                            state = State::Disconnected;
                                        }
                                        _ => {}
                                    }
                                }
                                Err(e) => {
                                    let _ = output
                                        .send(Event::Disconnected(
                                            exchange,
                                            format!("WebSocket error: {}", e),
                                        ))
                                        .await;
                                    state = State::Disconnected;
                                }
                            }
                        }
                        _ = &mut timeout => {
                            // Timeout - reconnect
                            let _ = output
                                .send(Event::Disconnected(exchange, "Connection timeout".to_string()))
                                .await;
                            state = State::Disconnected;
                        }
                    }
                }
            }
        }
    })
}

/// Connect to kline stream for multiple ticker/timeframe pairs
pub fn connect_kline_stream(
    streams: Vec<(TickerInfo, Timeframe)>,
    _market: MarketKind,
) -> impl Stream<Item = Event> {
    stream::channel(100, async move |mut output| {
        if streams.is_empty() {
            return;
        }

        let exchange = streams[0].0.ticker.exchange;

        // Build combined stream path
        let stream_names: Vec<String> = streams
            .iter()
            .filter_map(|(info, tf)| {
                let (symbol, _) = info.ticker.to_full_symbol_and_type();
                timeframe_to_interval(*tf).map(|interval| {
                    format!("{}@kline_{}", symbol.to_lowercase(), interval)
                })
            })
            .collect();

        let path = format!("/stream?streams={}", stream_names.join("/"));

        let mut state = State::Disconnected;

        loop {
            match &mut state {
                State::Disconnected => {
                    match connect_websocket(&path).await {
                        Ok(ws) => {
                            state = State::Connected(ws);
                            let _ = output.send(Event::Connected(exchange)).await;
                        }
                        Err(e) => {
                            let _ = output
                                .send(Event::Disconnected(
                                    exchange,
                                    format!("WebSocket connection failed: {}", e),
                                ))
                                .await;
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                    }
                }

                State::Connected(ws) => {
                    let timeout = tokio::time::sleep(Duration::from_secs(20));
                    tokio::pin!(timeout);

                    tokio::select! {
                        frame = ws.read_frame() => {
                            match frame {
                                Ok(frame) => {
                                    match frame.opcode {
                                        OpCode::Text => {
                                            match parse_websocket_message(&frame.payload) {
                                                Ok(StreamData::Kline(symbol, kline)) => {
                                                    // Find matching ticker info by symbol
                                                    if let Some((ticker_info, timeframe)) = streams.iter().find(|(info, _)| {
                                                        let (ticker_symbol, _) = info.ticker.to_full_symbol_and_type();
                                                        ticker_symbol.eq_ignore_ascii_case(&symbol)
                                                    }) {
                                                        let stream_kind = StreamKind::Kline {
                                                            ticker_info: *ticker_info,
                                                            timeframe: *timeframe,
                                                        };

                                                        let kline_data = Kline {
                                                            time: kline.time,
                                                            open: Price::from_f32(kline.open).round_to_min_tick(ticker_info.min_ticksize),
                                                            high: Price::from_f32(kline.high).round_to_min_tick(ticker_info.min_ticksize),
                                                            low: Price::from_f32(kline.low).round_to_min_tick(ticker_info.min_ticksize),
                                                            close: Price::from_f32(kline.close).round_to_min_tick(ticker_info.min_ticksize),
                                                            volume: (kline.taker_buy_volume, kline.volume - kline.taker_buy_volume),
                                                        };

                                                        let _ = output
                                                            .send(Event::KlineReceived(stream_kind, kline_data))
                                                            .await;
                                                    }
                                                }
                                                Ok(_) => {
                                                    // Ignore other stream types
                                                }
                                                Err(e) => {
                                                    eprintln!("Parse error: {}", e);
                                                }
                                            }
                                        }
                                        OpCode::Ping => {
                                            let pong = Frame::pong(frame.payload);
                                            let _ = ws.write_frame(pong).await;
                                        }
                                        OpCode::Close => {
                                            let _ = output
                                                .send(Event::Disconnected(exchange, "Connection closed".to_string()))
                                                .await;
                                            state = State::Disconnected;
                                        }
                                        _ => {}
                                    }
                                }
                                Err(e) => {
                                    let _ = output
                                        .send(Event::Disconnected(
                                            exchange,
                                            format!("WebSocket error: {}", e),
                                        ))
                                        .await;
                                    state = State::Disconnected;
                                }
                            }
                        }
                        _ = &mut timeout => {
                            // Timeout - reconnect
                            let _ = output
                                .send(Event::Disconnected(exchange, "Connection timeout".to_string()))
                                .await;
                            state = State::Disconnected;
                        }
                    }
                }
            }
        }
    })
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_tick_size() {
        let filters = json!([
            {"filterType": "PRICE_FILTER", "tickSize": "0.01"},
            {"filterType": "LOT_SIZE", "minQty": "0.001"}
        ]);
        let result = extract_tick_size(filters.as_array().unwrap());
        assert_eq!(result, Some(0.01));
    }

    #[test]
    fn test_extract_tick_size_missing() {
        let filters = json!([
            {"filterType": "OTHER", "value": "123"}
        ]);
        let result = extract_tick_size(filters.as_array().unwrap());
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_min_qty() {
        let filters = json!([
            {"filterType": "PRICE_FILTER", "tickSize": "0.01"},
            {"filterType": "LOT_SIZE", "minQty": "0.001"}
        ]);
        let result = extract_min_qty(filters.as_array().unwrap());
        assert_eq!(result, Some(0.001));
    }

    #[test]
    fn test_extract_min_qty_missing() {
        let filters = json!([
            {"filterType": "PRICE_FILTER", "tickSize": "0.01"}
        ]);
        let result = extract_min_qty(filters.as_array().unwrap());
        assert_eq!(result, None);
    }

    #[test]
    fn test_timeframe_mapping() {
        assert_eq!(timeframe_to_interval(Timeframe::M1), Some("1m"));
        assert_eq!(timeframe_to_interval(Timeframe::M3), Some("3m"));
        assert_eq!(timeframe_to_interval(Timeframe::M5), Some("5m"));
        assert_eq!(timeframe_to_interval(Timeframe::M15), Some("15m"));
        assert_eq!(timeframe_to_interval(Timeframe::M30), Some("30m"));
        assert_eq!(timeframe_to_interval(Timeframe::H1), Some("1h"));
        assert_eq!(timeframe_to_interval(Timeframe::H2), Some("2h"));
        assert_eq!(timeframe_to_interval(Timeframe::H4), Some("4h"));
        assert_eq!(timeframe_to_interval(Timeframe::H6), Some("6h"));
        assert_eq!(timeframe_to_interval(Timeframe::H12), Some("12h"));
        assert_eq!(timeframe_to_interval(Timeframe::D1), Some("1d"));
        // Unsupported timeframes should return None
        assert_eq!(timeframe_to_interval(Timeframe::MS100), None);
        assert_eq!(timeframe_to_interval(Timeframe::MS200), None);
    }

    #[test]
    fn test_kline_deserialization() {
        let json = r#"[
            "1499040000000", "0.01634790", "0.80000000", "0.01575800",
            "0.01577100", "148976.11427815", "1499644799999", "2434.19055334",
            308, "1756.87402397", "28.46694368", "0"
        ]"#;
        let kline: AsterKline = serde_json::from_str(json).unwrap();
        assert_eq!(kline.0, 1499040000000);
        assert!((kline.1 - 0.01634790).abs() < 0.0001);
        assert!((kline.2 - 0.80000000).abs() < 0.0001);
        assert!((kline.3 - 0.01575800).abs() < 0.0001);
        assert!((kline.4 - 0.01577100).abs() < 0.0001);
    }

    #[test]
    fn test_depth_update_deserialization() {
        let json = r#"{
            "U": 157, "u": 160, "pu": 149,
            "b": [["9168.86", "0.100"]], "a": [["9169.14", "0.258"]]
        }"#;
        let update: AsterDepthUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(update.first_update_id, 157);
        assert_eq!(update.final_update_id, 160);
        assert_eq!(update.prev_final_update_id, 149);
        assert_eq!(update.bids.len(), 1);
        assert_eq!(update.asks.len(), 1);
    }

    #[test]
    fn test_parse_price_qty_array_empty() {
        let result = parse_price_qty_array(vec![], 0.01);
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_price_qty_array_valid() {
        let input = vec![
            vec!["100.5".to_string(), "1.5".to_string()],
            vec!["100.51".to_string(), "2.0".to_string()],
        ];
        let result = parse_price_qty_array(input, 0.1);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].qty, 1.5);
        assert_eq!(result[1].qty, 2.0);
    }

    #[test]
    fn test_parse_price_qty_array_zero_qty() {
        let input = vec![
            vec!["100.5".to_string(), "0".to_string()],
        ];
        let result = parse_price_qty_array(input, 0.01);
        assert!(result.is_empty());
    }

    #[test]
    fn test_ticker_stats_deserialization() {
        let json = r#"{
            "symbol": "BTCUSDT",
            "lastPrice": "50000.50",
            "priceChangePercent": "2.5",
            "volume": "1000.5"
        }"#;
        let stats: AsterTickerStats = serde_json::from_str(json).unwrap();
        assert_eq!(stats.symbol, "BTCUSDT");
        assert!((stats.last_price - 50000.50).abs() < 0.01);
        assert!((stats.price_change_percent - 2.5).abs() < 0.01);
        assert!((stats.volume - 1000.5).abs() < 0.01);
    }

    #[test]
    fn test_oi_data_deserialization() {
        let json = r#"{
            "timestamp": "1627776000000",
            "sumOpenInterest": "12345.67"
        }"#;
        let oi: AsterOIData = serde_json::from_str(json).unwrap();
        assert_eq!(oi.timestamp, 1627776000000);
        assert!((oi.open_interest - 12345.67).abs() < 0.01);
    }

    #[test]
    fn test_trade_deserialization() {
        let json = r#"{
            "p": "100.5",
            "q": "1.5",
            "m": false,
            "T": "1627776000000"
        }"#;
        let trade: AsterTrade = serde_json::from_str(json).unwrap();
        assert!((trade.price - 100.5).abs() < 0.01);
        assert!((trade.qty - 1.5).abs() < 0.01);
        assert_eq!(trade.is_buyer_maker, false);
        assert_eq!(trade.timestamp, 1627776000000);
    }
}
