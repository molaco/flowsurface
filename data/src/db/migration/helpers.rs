//! Helper functions for migration operations
//!
//! Provides utility functions for ID generation and ticker management during migration.

use crate::db::{DatabaseError, Result};
use duckdb::Connection;
use exchange::{adapter::Exchange, TickerInfo};

/// Generate a unique trade ID for insertion
///
/// Uses a simple counter-based approach for deterministic IDs
pub fn generate_trade_id(ticker_id: i64, trade_time: u64, counter: u64) -> i64 {
    // Combine ticker_id, trade_time, and counter to create unique ID
    // This ensures idempotent inserts - same data produces same ID
    ((ticker_id as i64) << 48) | ((trade_time >> 20) as i64) << 16 | ((counter & 0xFFFF) as i64)
}

/// Generate deterministic kline ID from components
///
/// Ensures uniqueness and allows idempotent inserts
pub fn generate_kline_id(ticker_id: i64, timeframe_ms: u64, candle_time: u64) -> i64 {
    // Hash-based approach for deterministic ID generation
    // Combine ticker_id, timeframe, and timestamp
    let hash_input = format!("{}:{}:{}", ticker_id, timeframe_ms, candle_time);
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    hash_input.hash(&mut hasher);
    hasher.finish() as i64
}

/// Generate unique ID for footprint_data records
pub fn generate_footprint_id(kline_id: i64, price_level: i64) -> i64 {
    // Combine kline_id with price level for uniqueness
    kline_id ^ price_level
}

/// Generate unique ID for order_runs records
pub fn generate_run_id(ticker_id: i64, price_level: i64, start_time: u64) -> i64 {
    // Combine ticker_id, price, and start_time
    let hash_input = format!("{}:{}:{}", ticker_id, price_level, start_time);
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    hash_input.hash(&mut hasher);
    hasher.finish() as i64
}

/// Get exchange_id from exchange name, creating if needed
pub fn get_or_create_exchange_id(conn: &mut Connection, exchange: Exchange) -> Result<i64> {
    let exchange_name = format!("{:?}", exchange);

    // Try to get existing exchange_id
    let maybe_id: Option<i64> = conn
        .query_row(
            "SELECT exchange_id FROM exchanges WHERE name = ?",
            [&exchange_name],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = maybe_id {
        return Ok(id);
    }

    // Generate exchange_id from exchange enum value (simple hash constrained to TINYINT range)
    let exchange_id: i64 = (format!("{:?}", exchange)
        .bytes()
        .fold(0i64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as i64))
        .abs() % 127) + 1; // Keep in range 1-127 for TINYINT

    // Create new exchange record with explicit ID
    conn.execute(
        "INSERT OR IGNORE INTO exchanges (exchange_id, name) VALUES (?, ?)",
        duckdb::params![exchange_id, exchange_name],
    )
    .map_err(|e| DatabaseError::Insert(format!("Failed to insert exchange: {}", e)))?;

    Ok(exchange_id)
}

/// Resolve TickerInfo to ticker_id, creating ticker record if needed
///
/// Handles exchange_id lookup/creation as well
pub fn get_or_create_ticker_id(conn: &mut Connection, ticker_info: &TickerInfo) -> Result<i64> {
    let symbol = format!("{}", ticker_info.ticker);
    let exchange = ticker_info.exchange();

    // Get or create exchange_id
    let exchange_id = get_or_create_exchange_id(conn, exchange)?;

    // Try to get existing ticker_id
    let maybe_id: Option<i64> = conn
        .query_row(
            "SELECT ticker_id FROM tickers WHERE exchange_id = ? AND symbol = ?",
            duckdb::params![exchange_id, symbol],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = maybe_id {
        return Ok(id);
    }

    // Create new ticker record
    // Power10 has a power field, convert to actual float value
    let min_tick = 10.0_f32.powi(ticker_info.min_ticksize.power as i32);
    let min_qty = 10.0_f32.powi(ticker_info.min_qty.power as i32);
    let contract_size = ticker_info.contract_size.map(|c| 10.0_f32.powi(c.power as i32));

    conn.execute(
        "INSERT INTO tickers (exchange_id, symbol, tick_size, min_quantity, contract_size)
         VALUES (?, ?, ?, ?, ?)",
        duckdb::params![
            exchange_id,
            symbol,
            min_tick,
            min_qty,
            contract_size
        ],
    )
    .map_err(|e| DatabaseError::Insert(format!("Failed to insert ticker: {}", e)))?;

    // Get the auto-generated ID
    let id: i64 = conn
        .query_row(
            "SELECT ticker_id FROM tickers WHERE exchange_id = ? AND symbol = ?",
            duckdb::params![exchange_id, symbol],
            |row| row.get(0),
        )
        .map_err(|e| DatabaseError::Query(format!("Failed to get ticker_id: {}", e)))?;

    Ok(id)
}

/// Look up existing ticker_id without creating new record
///
/// Returns error if ticker doesn't exist
pub fn get_ticker_id(conn: &mut Connection, ticker_info: &TickerInfo) -> Result<i64> {
    let symbol = format!("{}", ticker_info.ticker);
    let exchange = ticker_info.exchange();
    let exchange_name = format!("{:?}", exchange);

    let id: i64 = conn
        .query_row(
            "SELECT t.ticker_id
             FROM tickers t
             JOIN exchanges e ON t.exchange_id = e.exchange_id
             WHERE e.name = ? AND t.symbol = ?",
            [&exchange_name, &symbol],
            |row| row.get(0),
        )
        .map_err(|e| {
            DatabaseError::NotFound(format!(
                "Ticker {}:{} not found in database: {}",
                exchange_name, symbol, e
            ))
        })?;

    Ok(id)
}

/// Convert timeframe enum to milliseconds string for database
pub fn timeframe_to_string(timeframe_ms: u64) -> String {
    match timeframe_ms {
        100 => "100ms".to_string(),
        200 => "200ms".to_string(),
        300 => "300ms".to_string(),
        500 => "500ms".to_string(),
        1000 => "1s".to_string(),
        60000 => "1m".to_string(),
        180000 => "3m".to_string(),
        300000 => "5m".to_string(),
        900000 => "15m".to_string(),
        1800000 => "30m".to_string(),
        3600000 => "1h".to_string(),
        7200000 => "2h".to_string(),
        14400000 => "4h".to_string(),
        21600000 => "6h".to_string(),
        43200000 => "12h".to_string(),
        86400000 => "1d".to_string(),
        _ => format!("{}ms", timeframe_ms),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_trade_id_deterministic() {
        let id1 = generate_trade_id(1, 1000, 0);
        let id2 = generate_trade_id(1, 1000, 0);
        assert_eq!(id1, id2, "Same inputs should produce same ID");
    }

    #[test]
    fn test_generate_kline_id_deterministic() {
        let id1 = generate_kline_id(1, 60000, 1234567890000);
        let id2 = generate_kline_id(1, 60000, 1234567890000);
        assert_eq!(id1, id2, "Same inputs should produce same ID");
    }

    #[test]
    fn test_timeframe_conversion() {
        assert_eq!(timeframe_to_string(60000), "1m");
        assert_eq!(timeframe_to_string(3600000), "1h");
        assert_eq!(timeframe_to_string(86400000), "1d");
    }
}
