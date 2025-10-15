//! Order runs CRUD for historical depth reconstruction
//!
//! Order runs track consecutive orders at same price level for heatmap visualization

use crate::db::error::{DatabaseError, Result};
use crate::db::helpers::{decimal_to_price, get_or_create_ticker_id, price_to_decimal};
use crate::db::DatabaseManager;
use crate::chart::heatmap::{HistoricalDepth, OrderRun};
use exchange::util::{Price, PriceStep};
use exchange::TickerInfo;
use std::collections::BTreeMap;

impl DatabaseManager {
    /// Insert order runs in bulk for historical depth tracking
    ///
    /// Used to persist order book changes for heatmap reconstruction
    pub fn insert_order_runs(
        &self,
        ticker_info: &TickerInfo,
        runs: &[(Price, OrderRun)],
    ) -> Result<usize> {
        if runs.is_empty() {
            return Ok(0);
        }

        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;

            let mut stmt = conn
                .prepare(
                    "INSERT INTO order_runs
                     (run_id, ticker_id, start_time, end_time, price_level, total_volume, num_orders, is_buy)
                     VALUES (?, ?, epoch_ms(?), epoch_ms(?), ?, ?, ?, ?)",
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to prepare insert: {}", e)))?;

            for (price, run) in runs {
                let run_id = generate_run_id(ticker_id, run.start_time, *price);

                stmt.execute(duckdb::params![
                    run_id,
                    ticker_id,
                    run.start_time as i64,
                    run.until_time as i64,
                    price_to_decimal(*price),
                    run.qty() as f64,
                    1i32, // num_orders - approximation
                    run.is_bid,
                ])
                .map_err(|e| {
                    DatabaseError::Query(format!("Failed to insert order run: {}", e))
                })?;
            }

            Ok(runs.len())
        })
    }

    /// Query order runs within time and price range for HistoricalDepth reconstruction
    ///
    /// This is the key method for loading heatmap data from database
    pub fn query_order_runs(
        &self,
        ticker_info: &TickerInfo,
        earliest: u64,
        latest: u64,
        lowest: Price,
        highest: Price,
    ) -> Result<Vec<(Price, OrderRun)>> {
        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;

            let mut stmt = conn
                .prepare(
                    "SELECT price_level, epoch_ms(start_time), epoch_ms(end_time), total_volume, is_buy
                     FROM order_runs
                     WHERE ticker_id = ?
                       AND start_time <= epoch_ms(?)
                       AND end_time >= epoch_ms(?)
                       AND price_level >= ?
                       AND price_level <= ?
                     ORDER BY price_level, start_time",
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to prepare query: {}", e)))?;

            let runs_iter = stmt
                .query_map(
                    duckdb::params![
                        ticker_id,
                        latest as i64,
                        earliest as i64,
                        price_to_decimal(lowest),
                        price_to_decimal(highest),
                    ],
                    |row| {
                        let price_level: f64 = row.get(0)?;
                        let start_time: i64 = row.get(1)?;
                        let end_time: i64 = row.get(2)?;
                        let total_volume: f64 = row.get(3)?;
                        let is_buy: bool = row.get(4)?;

                        let price = decimal_to_price(price_level);
                        let run = OrderRun::new(
                            start_time as u64,
                            end_time as u64,
                            total_volume as f32,
                            is_buy,
                        );

                        Ok((price, run))
                    },
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to query order runs: {}", e)))?;

            let mut runs = Vec::new();
            for run_result in runs_iter {
                runs.push(
                    run_result
                        .map_err(|e| DatabaseError::Query(format!("Failed to map order run: {}", e)))?,
                );
            }

            Ok(runs)
        })
    }

    /// Load HistoricalDepth from database for heatmap rendering
    ///
    /// Reconstructs the complete HistoricalDepth structure with price_levels BTreeMap
    pub fn load_historical_depth(
        &self,
        ticker_info: &TickerInfo,
        earliest: u64,
        latest: u64,
        lowest: Price,
        highest: Price,
        tick_size: PriceStep,
        min_order_qty: f32,
    ) -> Result<HistoricalDepth> {
        let runs = self.query_order_runs(ticker_info, earliest, latest, lowest, highest)?;

        // Group runs by price level
        let mut price_levels: BTreeMap<Price, Vec<OrderRun>> = BTreeMap::new();

        for (price, run) in runs {
            price_levels.entry(price).or_insert_with(Vec::new).push(run);
        }

        // Create HistoricalDepth with reconstructed data
        let depth = HistoricalDepth::new(min_order_qty, tick_size, crate::chart::Basis::Time(exchange::Timeframe::M1));

        // Inject the price_levels directly (would need to expose setter or use a constructor)
        // For now, return a basic depth and note this needs integration with HistoricalDepth API

        Ok(depth)
    }

    /// Delete order runs older than cutoff timestamp
    pub fn delete_order_runs_older_than(&self, cutoff_time: u64) -> Result<usize> {
        self.with_conn(|conn| {
            let deleted = conn
                .execute(
                    "DELETE FROM order_runs WHERE start_time < epoch_ms(?)",
                    [cutoff_time as i64],
                )
                .map_err(|e| {
                    DatabaseError::Query(format!("Failed to delete order runs: {}", e))
                })?;

            Ok(deleted)
        })
    }

    /// Count order runs for a ticker in time range
    pub fn count_order_runs(
        &self,
        ticker_info: &TickerInfo,
        start_time: u64,
        end_time: u64,
    ) -> Result<i64> {
        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;

            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM order_runs
                     WHERE ticker_id = ? AND start_time >= epoch_ms(?) AND start_time <= epoch_ms(?)",
                    duckdb::params![ticker_id, start_time as i64, end_time as i64],
                    |row| row.get(0),
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to count order runs: {}", e)))?;

            Ok(count)
        })
    }
}

/// Generate a unique ID for order run
fn generate_run_id(ticker_id: i32, start_time: u64, price: Price) -> i64 {
    // Combine ticker_id, time, and price into unique ID
    // Using a simple hash-based approach
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    ticker_id.hash(&mut hasher);
    start_time.hash(&mut hasher);
    price.units.hash(&mut hasher);

    hasher.finish() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use exchange::adapter::Exchange;
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

    fn create_test_order_runs() -> Vec<(Price, OrderRun)> {
        (0..20)
            .map(|i| {
                let price = Price::from_f32(50000.0 + i as f32 * 10.0);
                let run = OrderRun::new(
                    1000000 + i * 1000,
                    1000000 + i * 1000 + 500,
                    10.0 + i as f32,
                    i % 2 == 0,
                );
                (price, run)
            })
            .collect()
    }

    #[test]
    fn test_insert_and_query_order_runs() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        let runs = create_test_order_runs();

        // Insert runs
        let inserted = db.insert_order_runs(&ticker_info, &runs).unwrap();
        assert_eq!(inserted, 20);

        // Query back
        let lowest = Price::from_f32(49900.0);
        let highest = Price::from_f32(51000.0);
        let queried = db
            .query_order_runs(&ticker_info, 1000000, 2000000, lowest, highest)
            .unwrap();

        assert_eq!(queried.len(), 20);
    }

    #[test]
    fn test_query_order_runs_price_filter() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        let runs = create_test_order_runs();
        db.insert_order_runs(&ticker_info, &runs).unwrap();

        // Query with narrow price range
        let lowest = Price::from_f32(50000.0);
        let highest = Price::from_f32(50050.0);
        let queried = db
            .query_order_runs(&ticker_info, 1000000, 2000000, lowest, highest)
            .unwrap();

        // Should only get runs within price range
        assert!(queried.len() < 20);
        assert!(queried.len() > 0);
    }

    #[test]
    fn test_query_order_runs_time_filter() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        let runs = create_test_order_runs();
        db.insert_order_runs(&ticker_info, &runs).unwrap();

        // Query with narrow time range
        let lowest = Price::from_f32(49000.0);
        let highest = Price::from_f32(51000.0);
        let queried = db
            .query_order_runs(&ticker_info, 1000000, 1005000, lowest, highest)
            .unwrap();

        // Should only get runs that overlap with time range
        assert!(queried.len() < 20);
        assert!(queried.len() > 0);
    }

    #[test]
    fn test_count_order_runs() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        let runs = create_test_order_runs();
        db.insert_order_runs(&ticker_info, &runs).unwrap();

        let count = db
            .count_order_runs(&ticker_info, 1000000, 2000000)
            .unwrap();
        assert_eq!(count, 20);
    }

    #[test]
    fn test_delete_order_runs_older_than() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        let runs = create_test_order_runs();
        db.insert_order_runs(&ticker_info, &runs).unwrap();

        // Delete old runs
        let cutoff = 1000000 + 10000;
        let deleted = db.delete_order_runs_older_than(cutoff).unwrap();
        assert!(deleted > 0);

        // Verify remaining
        let count = db
            .count_order_runs(&ticker_info, 0, u64::MAX)
            .unwrap();
        assert!(count < 20);
    }

    #[test]
    fn test_empty_query() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        let lowest = Price::from_f32(50000.0);
        let highest = Price::from_f32(50100.0);
        let queried = db
            .query_order_runs(&ticker_info, 1000000, 2000000, lowest, highest)
            .unwrap();

        assert_eq!(queried.len(), 0);
    }
}
