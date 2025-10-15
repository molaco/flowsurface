//! Trade CRUD operations using DuckDB Appender API for high-throughput inserts

use crate::db::error::{DatabaseError, Result};
use crate::db::helpers::{
    decimal_to_price, generate_trade_id, get_or_create_ticker_id, price_to_decimal,
};
use crate::db::DatabaseManager;
use crate::db::crud::TradesCRUD;
use exchange::{TickerInfo, Trade};
use exchange::util::Price;

impl TradesCRUD for DatabaseManager {
    /// Insert trades using prepared statement with ON CONFLICT DO NOTHING
    ///
    /// Handles duplicate trades gracefully by ignoring conflicts on trade_id.
    /// This is necessary because exchanges may send the same trade multiple times.
    fn insert_trades(&self, ticker_info: &TickerInfo, trades: &[Trade]) -> Result<usize> {
        if trades.is_empty() {
            return Ok(0);
        }

        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;

            // Use prepared statement with ON CONFLICT DO NOTHING to handle duplicates
            let mut stmt = conn
                .prepare(
                    "INSERT INTO trades (trade_id, ticker_id, timestamp, price, quantity, is_buyer_maker)
                     VALUES (?, ?, ?, ?, ?, ?)
                     ON CONFLICT (trade_id) DO NOTHING"
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to prepare insert: {}", e)))?;

            let mut inserted = 0;
            for trade in trades {
                let trade_id = generate_trade_id(ticker_id, trade.time, trade.price, trade.qty);

                let rows = stmt
                    .execute(duckdb::params![
                        trade_id,
                        ticker_id,
                        trade.time as i64,
                        price_to_decimal(trade.price),
                        trade.qty as f64,
                        !trade.is_sell,
                    ])
                    .map_err(|e| DatabaseError::Query(format!("Failed to insert trade: {}", e)))?;

                inserted += rows;
            }

            Ok(inserted)
        })
    }

    /// Query trades by time range
    ///
    /// Returns strongly-typed Trade structs with Price types reconstructed
    fn query_trades(
        &self,
        ticker_info: &TickerInfo,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<Trade>> {
        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;

            let mut stmt = conn
                .prepare(
                    "SELECT timestamp, price, quantity, is_buyer_maker
                     FROM trades
                     WHERE ticker_id = ? AND timestamp >= ? AND timestamp <= ?
                     ORDER BY timestamp ASC",
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to prepare query: {}", e)))?;

            let trades_iter = stmt
                .query_map(
                    duckdb::params![ticker_id, start_time as i64, end_time as i64],
                    |row| {
                        let timestamp: i64 = row.get(0)?;
                        let price: f64 = row.get(1)?;
                        let quantity: f64 = row.get(2)?;
                        let is_buyer_maker: bool = row.get(3)?;

                        Ok(Trade {
                            time: timestamp as u64,
                            price: decimal_to_price(price),
                            qty: quantity as f32,
                            is_sell: !is_buyer_maker,
                        })
                    },
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to query trades: {}", e)))?;

            let mut trades = Vec::new();
            for trade_result in trades_iter {
                trades.push(
                    trade_result
                        .map_err(|e| DatabaseError::Query(format!("Failed to map trade: {}", e)))?,
                );
            }

            Ok(trades)
        })
    }

    /// Fast count query without materializing trade objects
    fn query_trades_count(
        &self,
        ticker_info: &TickerInfo,
        start_time: u64,
        end_time: u64,
    ) -> Result<i64> {
        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;

            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM trades
                     WHERE ticker_id = ? AND timestamp >= ? AND timestamp <= ?",
                    duckdb::params![ticker_id, start_time as i64, end_time as i64],
                    |row| row.get(0),
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to count trades: {}", e)))?;

            Ok(count)
        })
    }

    /// Delete trades older than cutoff timestamp across all tickers
    fn delete_trades_older_than(&self, cutoff_time: u64) -> Result<usize> {
        self.with_conn(|conn| {
            let deleted = conn
                .execute(
                    "DELETE FROM trades WHERE timestamp < ?",
                    [cutoff_time as i64],
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to delete trades: {}", e)))?;

            Ok(deleted)
        })
    }

    /// Query trades aggregated by price level for efficient footprint reconstruction
    ///
    /// Pre-aggregates buy_qty, sell_qty, buy_count, sell_count at each price level
    /// This is much faster than loading individual trades and aggregating in memory
    fn query_trades_aggregated(
        &self,
        ticker_info: &TickerInfo,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<(Price, f32, f32, usize, usize)>> {
        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;

            let mut stmt = conn
                .prepare(
                    "SELECT
                         price,
                         SUM(CASE WHEN is_buyer_maker = false THEN quantity ELSE 0 END) as buy_volume,
                         SUM(CASE WHEN is_buyer_maker = true THEN quantity ELSE 0 END) as sell_volume,
                         COUNT(CASE WHEN is_buyer_maker = false THEN 1 END) as buy_count,
                         COUNT(CASE WHEN is_buyer_maker = true THEN 1 END) as sell_count
                     FROM trades
                     WHERE ticker_id = ? AND timestamp >= ? AND timestamp <= ?
                     GROUP BY price
                     ORDER BY price",
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to prepare aggregation query: {}", e)))?;

            let aggregated_iter = stmt
                .query_map(
                    duckdb::params![ticker_id, start_time as i64, end_time as i64],
                    |row| {
                        let price: f64 = row.get(0)?;
                        let buy_volume: f64 = row.get(1)?;
                        let sell_volume: f64 = row.get(2)?;
                        let buy_count: i64 = row.get(3)?;
                        let sell_count: i64 = row.get(4)?;

                        Ok((
                            decimal_to_price(price),
                            buy_volume as f32,
                            sell_volume as f32,
                            buy_count as usize,
                            sell_count as usize,
                        ))
                    },
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to query aggregated trades: {}", e)))?;

            let mut aggregated = Vec::new();
            for result in aggregated_iter {
                aggregated.push(
                    result.map_err(|e| {
                        DatabaseError::Query(format!("Failed to map aggregated trade: {}", e))
                    })?,
                );
            }

            Ok(aggregated)
        })
    }

    /// Check database coverage for trade data
    ///
    /// Returns earliest and latest trade timestamps, or None if no data exists
    fn query_trades_coverage(
        &self,
        ticker_info: &TickerInfo,
    ) -> Result<Option<(u64, u64)>> {
        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;

            let result: Option<(i64, i64)> = conn
                .query_row(
                    "SELECT MIN(timestamp), MAX(timestamp)
                     FROM trades
                     WHERE ticker_id = ?",
                    [ticker_id],
                    |row| {
                        let min_time: Option<i64> = row.get(0)?;
                        let max_time: Option<i64> = row.get(1)?;
                        match (min_time, max_time) {
                            (Some(min), Some(max)) => Ok(Some((min, max))),
                            _ => Ok(None),
                        }
                    },
                )
                .unwrap_or(None);

            Ok(result.map(|(min, max)| (min as u64, max as u64)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exchange::adapter::Exchange;
    use exchange::util::Price;
    use exchange::Ticker;
    use tempfile::tempdir;

    fn create_test_db() -> (DatabaseManager, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = DatabaseManager::new(db_path).unwrap();
        (db, dir)
    }

    fn create_test_ticker_info() -> TickerInfo {
        let ticker = Ticker::new("BTCUSDT", Exchange::BinanceLinear);
        TickerInfo::new(ticker, 0.01, 0.001, None)
    }

    fn create_test_trades(count: usize) -> Vec<Trade> {
        (0..count)
            .map(|i| Trade {
                time: 1000000 + (i as u64 * 1000),
                price: Price::from_f32(50000.0 + i as f32),
                qty: 1.0 + (i as f32 * 0.1),
                is_sell: i % 2 == 0,
            })
            .collect()
    }

    #[test]
    fn test_insert_and_query_trades() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        let trades = create_test_trades(100);

        // Insert trades
        let inserted = db.insert_trades(&ticker_info, &trades).unwrap();
        assert_eq!(inserted, 100);

        // Query back
        let queried_trades = db
            .query_trades(&ticker_info, 1000000, 2000000)
            .unwrap();

        assert_eq!(queried_trades.len(), 100);
        assert_eq!(queried_trades[0].time, trades[0].time);
        assert!((queried_trades[0].qty - trades[0].qty).abs() < 0.001);
    }

    #[test]
    fn test_query_trades_count() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        let trades = create_test_trades(50);
        db.insert_trades(&ticker_info, &trades).unwrap();

        let count = db.query_trades_count(&ticker_info, 1000000, 2000000).unwrap();
        assert_eq!(count, 50);
    }

    #[test]
    fn test_delete_trades_older_than() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        let trades = create_test_trades(100);
        db.insert_trades(&ticker_info, &trades).unwrap();

        // Delete trades older than midpoint
        let cutoff = 1000000 + 50000;
        let deleted = db.delete_trades_older_than(cutoff).unwrap();
        assert!(deleted > 0);

        // Verify remaining trades
        let remaining = db.query_trades_count(&ticker_info, 0, u64::MAX).unwrap();
        assert!(remaining < 100);
    }

    #[test]
    fn test_empty_insert() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        let inserted = db.insert_trades(&ticker_info, &[]).unwrap();
        assert_eq!(inserted, 0);
    }

    #[test]
    fn test_price_precision() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        // Test various price precisions
        let test_prices = [
            12345.67890123,
            0.00000001,
            0.12345678,
            99999999.99999999,
        ];

        for price_val in test_prices {
            let trades = vec![Trade {
                time: 1000000,
                price: Price::from_f32(price_val),
                qty: 1.0,
                is_sell: false,
            }];

            db.insert_trades(&ticker_info, &trades).unwrap();
            let queried = db.query_trades(&ticker_info, 999999, 1000001).unwrap();

            assert_eq!(queried.len(), 1);
            // Check precision is maintained (within float tolerance)
            let diff = (queried[0].price.to_f32() - price_val).abs();
            assert!(diff < 1e-6, "Price precision lost: expected {}, got {}, diff {}", price_val, queried[0].price.to_f32(), diff);

            // Clean up for next iteration
            db.delete_trades_older_than(2000000).unwrap();
        }
    }
}
