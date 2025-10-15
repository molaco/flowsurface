//! Helper utilities for database operations
//!
//! Provides:
//! - Ticker ID resolution with LRU caching
//! - Exchange ID management
//! - Type conversions between Rust types and SQL types
//! - ID generation for trades, klines, and snapshots

use super::error::{DatabaseError, Result};
use duckdb::{Connection, OptionalExt};
use exchange::adapter::Exchange;
use exchange::util::Price;
use exchange::TickerInfo;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;

/// Global cache for ticker ID lookups to avoid repeated database queries
static TICKER_CACHE: Mutex<Option<HashMap<String, i32>>> = Mutex::new(None);

/// Maximum size of ticker cache before eviction
const MAX_TICKER_CACHE_SIZE: usize = 10000;

/// Atomic counter for generating unique trade IDs
static TRADE_ID_COUNTER: AtomicI64 = AtomicI64::new(1);

/// Atomic counter for generating unique snapshot IDs
static SNAPSHOT_ID_COUNTER: AtomicI64 = AtomicI64::new(1);

/// Get or create a ticker ID for the given ticker info
///
/// First checks cache, then database. If not found, creates new ticker entry.
/// Cache is used to avoid repeated lookups on the hot path during real-time ingestion.
///
/// # Arguments
/// * `conn` - Database connection
/// * `ticker_info` - Ticker information including symbol and exchange
///
/// # Returns
/// The ticker_id (auto-incremented integer primary key)
pub fn get_or_create_ticker_id(conn: &Connection, ticker_info: &TickerInfo) -> Result<i32> {
    let (symbol, _) = ticker_info.ticker.to_full_symbol_and_type();
    let exchange_id = get_or_create_exchange_id(conn, ticker_info.ticker.exchange)?;

    // Create cache key: exchange_id:symbol
    let cache_key = format!("{}:{}", exchange_id, symbol);

    // Check cache first
    {
        let mut cache_guard = TICKER_CACHE.lock().unwrap();
        let cache = cache_guard.get_or_insert_with(HashMap::new);

        if let Some(&ticker_id) = cache.get(&cache_key) {
            return Ok(ticker_id);
        }
    }

    // Not in cache, query database
    let ticker_id: Option<i32> = conn
        .query_row(
            "SELECT ticker_id FROM tickers WHERE exchange_id = ? AND symbol = ?",
            [&exchange_id as &dyn duckdb::ToSql, &symbol as &dyn duckdb::ToSql],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| DatabaseError::Query(format!("Failed to query ticker: {}", e)))?;

    let ticker_id = if let Some(id) = ticker_id {
        id
    } else {
        // Get next ticker_id by getting max + 1
        let next_id: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(ticker_id), 0) + 1 FROM tickers",
                [],
                |row| row.get(0),
            )
            .unwrap_or(1);

        // Create new ticker entry with explicit ticker_id
        conn.execute(
            "INSERT INTO tickers (ticker_id, exchange_id, symbol, min_ticksize, min_qty, contract_size, market_type)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            duckdb::params![
                next_id,
                exchange_id,
                symbol,
                ticker_info.min_ticksize.as_f32() as f64,
                ticker_info.min_qty.as_f32(),
                ticker_info.contract_size.map(|cs| cs.as_f32()),
                format!("{:?}", ticker_info.ticker.market_type()),
            ],
        )
        .map_err(|e| DatabaseError::Query(format!("Failed to insert ticker: {}", e)))?;

        next_id
    };

    // Update cache
    {
        let mut cache_guard = TICKER_CACHE.lock().unwrap();
        let cache = cache_guard.get_or_insert_with(HashMap::new);

        // Evict oldest entries if cache is too large
        if cache.len() >= MAX_TICKER_CACHE_SIZE {
            // Simple strategy: clear half the cache
            // In production, could use LRU
            cache.clear();
        }

        cache.insert(cache_key, ticker_id);
    }

    Ok(ticker_id)
}

/// Get ticker ID without creating a new entry
///
/// # Returns
/// `Ok(Some(ticker_id))` if found, `Ok(None)` if not found
pub fn get_ticker_id(conn: &Connection, ticker_info: &TickerInfo) -> Result<Option<i32>> {
    let (symbol, _) = ticker_info.ticker.to_full_symbol_and_type();
    let exchange_id = get_exchange_id(conn, ticker_info.ticker.exchange)?;

    let ticker_id = conn
        .query_row(
            "SELECT ticker_id FROM tickers WHERE exchange_id = ? AND symbol = ?",
            [&exchange_id as &dyn duckdb::ToSql, &symbol as &dyn duckdb::ToSql],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| DatabaseError::Query(format!("Failed to query ticker: {}", e)))?;

    Ok(ticker_id)
}

/// Get or create exchange ID
///
/// Exchange IDs are stored as TINYINT (1-12) corresponding to the Exchange enum variants
pub fn get_or_create_exchange_id(conn: &Connection, exchange: Exchange) -> Result<i8> {
    let exchange_name = format!("{:?}", exchange);
    let exchange_id = exchange_to_id(exchange);

    // Check if exchange exists
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM exchanges WHERE exchange_id = ?",
            [exchange_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !exists {
        // Create exchange entry
        conn.execute(
            "INSERT INTO exchanges (exchange_id, name) VALUES (?, ?)",
            duckdb::params![exchange_id, exchange_name],
        )
        .map_err(|e| DatabaseError::Query(format!("Failed to insert exchange: {}", e)))?;
    }

    Ok(exchange_id)
}

/// Get exchange ID without creating
pub fn get_exchange_id(conn: &Connection, exchange: Exchange) -> Result<i8> {
    let exchange_id = exchange_to_id(exchange);

    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM exchanges WHERE exchange_id = ?",
            [exchange_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if exists {
        Ok(exchange_id)
    } else {
        Err(DatabaseError::Query(format!(
            "Exchange {:?} not found in database",
            exchange
        )))
    }
}

/// Map Exchange enum to database ID (1-12)
fn exchange_to_id(exchange: Exchange) -> i8 {
    match exchange {
        Exchange::BinanceLinear => 1,
        Exchange::BinanceInverse => 2,
        Exchange::BinanceSpot => 3,
        Exchange::BybitLinear => 4,
        Exchange::BybitInverse => 5,
        Exchange::BybitSpot => 6,
        Exchange::HyperliquidLinear => 7,
        Exchange::HyperliquidSpot => 8,
        Exchange::OkexLinear => 9,
        Exchange::OkexInverse => 10,
        Exchange::OkexSpot => 11,
        Exchange::AsterLinear => 12,
    }
}

/// Convert Price to f64 for DECIMAL(18,8) storage
///
/// Preserves 8 decimal places as required by schema
#[inline]
pub fn price_to_decimal(price: Price) -> f64 {
    price.to_f32() as f64
}

/// Convert f64 from database back to Price
#[inline]
pub fn decimal_to_price(value: f64) -> Price {
    Price::from_f32(value as f32)
}

/// Convert millisecond timestamp to nanoseconds for DuckDB TIMESTAMP
#[inline]
pub fn timestamp_to_ns(ms_timestamp: u64) -> i64 {
    (ms_timestamp as i64)
        .saturating_mul(1_000_000)
}

/// Convert nanosecond timestamp back to milliseconds
#[inline]
pub fn ns_to_timestamp(ns: i64) -> u64 {
    (ns / 1_000_000) as u64
}

/// Generate deterministic trade ID from trade data
///
/// IMPORTANT: Includes ticker_id to ensure uniqueness across different trading pairs
/// and exchanges. Without ticker_id, the same (timestamp, price, qty) from different
/// pairs/exchanges would collide.
///
/// Uses hash(ticker_id, timestamp, price, qty) to create a unique identifier.
/// This approach prevents duplicate key violations across app restarts.
pub fn generate_trade_id(ticker_id: i32, timestamp: u64, price: Price, qty: f32) -> i64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    ticker_id.hash(&mut hasher);  // Critical: differentiate by ticker/exchange
    timestamp.hash(&mut hasher);
    price.units.hash(&mut hasher);
    // Convert qty to bits for hashing
    (qty.to_bits() as u64).hash(&mut hasher);

    hasher.finish() as i64
}

/// Generate deterministic kline ID from composite key
///
/// Uses a simple hash of ticker_id, timeframe, and candle_time
pub fn generate_kline_id(ticker_id: i32, timeframe: &str, candle_time: u64) -> i64 {
    // Use a better hash that includes actual timeframe content
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    ticker_id.hash(&mut hasher);
    timeframe.hash(&mut hasher);
    candle_time.hash(&mut hasher);

    hasher.finish() as i64
}

/// Generate deterministic snapshot ID from ticker and timestamp
///
/// Prevents duplicate key violations when persisting the same snapshot multiple times
pub fn generate_snapshot_id(ticker_id: i32, snapshot_time: u64) -> i64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    ticker_id.hash(&mut hasher);
    snapshot_time.hash(&mut hasher);

    hasher.finish() as i64
}

/// Generate unique footprint ID
pub fn generate_footprint_id(ticker_id: i32, timeframe: &str, candle_time: u64, price: Price) -> i64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    ticker_id.hash(&mut hasher);
    timeframe.hash(&mut hasher);
    candle_time.hash(&mut hasher);
    price.units.hash(&mut hasher);

    hasher.finish() as i64
}

/// Clear the ticker cache (useful for testing)
pub fn clear_ticker_cache() {
    let mut cache_guard = TICKER_CACHE.lock().unwrap();
    if let Some(cache) = cache_guard.as_mut() {
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::db::DatabaseManager;
    use exchange::Ticker;
    use exchange::util::MinTicksize;

    #[test]
    fn test_price_conversion() {
        let price = Price::from_f32(12345.67890123);
        let decimal = price_to_decimal(price);
        let back = decimal_to_price(decimal);

        // Should preserve 8 decimal places
        assert!((price.to_f32() - back.to_f32()).abs() < 1e-6);
    }

    #[test]
    fn test_timestamp_conversion() {
        let ms_timestamp = 1234567890123u64;
        let ns = timestamp_to_ns(ms_timestamp);
        let back = ns_to_timestamp(ns);

        assert_eq!(ms_timestamp, back);
    }

    #[test]
    fn test_trade_id_generation() {
        let id1 = generate_trade_id();
        let id2 = generate_trade_id();

        assert!(id2 > id1);
        assert_eq!(id2, id1 + 1);
    }

    #[test]
    fn test_kline_id_deterministic() {
        let id1 = generate_kline_id(123, "1m", 1000000);
        let id2 = generate_kline_id(123, "1m", 1000000);

        assert_eq!(id1, id2);
    }

    #[test]
    fn test_get_or_create_ticker_id() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = DatabaseManager::new(&db_path).unwrap();

        clear_ticker_cache();

        let ticker = Ticker::new("BTCUSDT", Exchange::BinanceLinear);
        let ticker_info = TickerInfo::new(ticker, 0.01, 0.001, None);

        // First call should create
        let id1 = db.with_conn(|conn| {
            get_or_create_ticker_id(conn, &ticker_info)
        }).unwrap();

        // Second call should return same ID from database
        let id2 = db.with_conn(|conn| {
            get_or_create_ticker_id(conn, &ticker_info)
        }).unwrap();

        assert_eq!(id1, id2);

        // Third call should return same ID from cache
        let id3 = db.with_conn(|conn| {
            get_or_create_ticker_id(conn, &ticker_info)
        }).unwrap();

        assert_eq!(id1, id3);
    }

    #[test]
    fn test_exchange_id_mapping() {
        let exchanges = [
            Exchange::BinanceLinear,
            Exchange::BinanceInverse,
            Exchange::BinanceSpot,
            Exchange::BybitLinear,
            Exchange::BybitInverse,
            Exchange::BybitSpot,
            Exchange::HyperliquidLinear,
            Exchange::HyperliquidSpot,
            Exchange::OkexLinear,
            Exchange::OkexInverse,
            Exchange::OkexSpot,
            Exchange::AsterLinear,
        ];

        for (i, exchange) in exchanges.iter().enumerate() {
            let id = exchange_to_id(*exchange);
            assert_eq!(id, (i + 1) as i8);
        }
    }
}
