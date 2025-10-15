//! Depth snapshot CRUD for orderbook storage and heatmap reconstruction

use crate::db::error::{DatabaseError, Result};
use crate::db::helpers::{generate_snapshot_id, get_or_create_ticker_id};
use crate::db::DatabaseManager;
use crate::db::crud::DepthCRUD;
use duckdb::OptionalExt;
use exchange::depth::Depth;
use exchange::util::Price;
use exchange::TickerInfo;
use std::collections::BTreeMap;

impl DepthCRUD for DatabaseManager {
    /// Insert a full depth snapshot
    ///
    /// Stores bids and asks as JSON arrays for efficient storage and retrieval
    /// Uses Appender API for high-frequency orderbook persistence (100ms intervals)
    fn insert_depth_snapshot(
        &self,
        ticker_info: &TickerInfo,
        snapshot_time: u64,
        depth: &Depth,
    ) -> Result<usize> {
        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;
            let snapshot_id = generate_snapshot_id(ticker_id, snapshot_time);

            // Serialize bids and asks to JSON
            let bids_json = serialize_price_levels(&depth.bids);
            let asks_json = serialize_price_levels(&depth.asks);

            // Use INSERT with ON CONFLICT to handle duplicates gracefully
            conn.execute(
                "INSERT INTO depth_snapshots (snapshot_id, ticker_id, timestamp, bids, asks)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT (snapshot_id) DO UPDATE SET
                    bids = EXCLUDED.bids,
                    asks = EXCLUDED.asks",
                duckdb::params![
                    snapshot_id,
                    ticker_id,
                    snapshot_time as i64,
                    bids_json,
                    asks_json,
                ],
            )
            .map_err(|e| DatabaseError::Query(format!("Failed to insert depth snapshot: {}", e)))?;

            Ok(1)
        })
    }

    /// Query a depth snapshot at specific time
    fn query_depth_snapshot(
        &self,
        ticker_info: &TickerInfo,
        snapshot_time: u64,
    ) -> Result<Option<Depth>> {
        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;

            let result = conn
                .query_row(
                    "SELECT bids, asks FROM depth_snapshots
                     WHERE ticker_id = ? AND timestamp = ?
                     LIMIT 1",
                    duckdb::params![ticker_id, snapshot_time as i64],
                    |row| {
                        let bids_json: String = row.get(0)?;
                        let asks_json: String = row.get(1)?;

                        let bids = deserialize_price_levels(&bids_json)
                            .map_err(|e| duckdb::Error::FromSqlConversionFailure(0, duckdb::types::Type::Text, Box::new(DatabaseError::Query(e.to_string()))))?;
                        let asks = deserialize_price_levels(&asks_json)
                            .map_err(|e| duckdb::Error::FromSqlConversionFailure(1, duckdb::types::Type::Text, Box::new(DatabaseError::Query(e.to_string()))))?;

                        Ok(Depth { bids, asks })
                    },
                )
                .optional()
                .map_err(|e| DatabaseError::Query(format!("Failed to query depth snapshot: {}", e)))?;

            Ok(result)
        })
    }

    /// Query multiple depth snapshots in time range
    fn query_depth_snapshots_range(
        &self,
        ticker_info: &TickerInfo,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<(u64, Depth)>> {
        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;

            let mut stmt = conn
                .prepare(
                    "SELECT timestamp, bids, asks FROM depth_snapshots
                     WHERE ticker_id = ? AND timestamp >= ? AND timestamp <= ?
                     ORDER BY timestamp ASC",
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to prepare query: {}", e)))?;

            let snapshots_iter = stmt
                .query_map(
                    duckdb::params![ticker_id, start_time as i64, end_time as i64],
                    |row| {
                        let timestamp: i64 = row.get(0)?;
                        let bids_json: String = row.get(1)?;
                        let asks_json: String = row.get(2)?;

                        let bids = deserialize_price_levels(&bids_json)
                            .map_err(|e| duckdb::Error::FromSqlConversionFailure(1, duckdb::types::Type::Text, Box::new(DatabaseError::Query(e.to_string()))))?;
                        let asks = deserialize_price_levels(&asks_json)
                            .map_err(|e| duckdb::Error::FromSqlConversionFailure(2, duckdb::types::Type::Text, Box::new(DatabaseError::Query(e.to_string()))))?;

                        Ok((timestamp as u64, Depth { bids, asks }))
                    },
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to query snapshots: {}", e)))?;

            let mut snapshots = Vec::new();
            for snapshot_result in snapshots_iter {
                snapshots.push(
                    snapshot_result.map_err(|e| {
                        DatabaseError::Query(format!("Failed to map snapshot: {}", e))
                    })?,
                );
            }

            Ok(snapshots)
        })
    }

    /// Delete depth snapshots older than cutoff
    fn delete_depth_snapshots_older_than(&self, cutoff_time: u64) -> Result<usize> {
        self.with_conn(|conn| {
            let deleted = conn
                .execute(
                    "DELETE FROM depth_snapshots WHERE timestamp < ?",
                    [cutoff_time as i64],
                )
                .map_err(|e| {
                    DatabaseError::Query(format!("Failed to delete depth snapshots: {}", e))
                })?;

            Ok(deleted)
        })
    }
}

/// Serialize price levels to JSON array format: [[price, qty], [price, qty], ...]
fn serialize_price_levels(levels: &BTreeMap<Price, f32>) -> String {
    let items: Vec<String> = levels
        .iter()
        .map(|(price, qty)| format!("[{},{}]", price.to_f32(), qty))
        .collect();

    format!("[{}]", items.join(","))
}

/// Deserialize price levels from JSON array
fn deserialize_price_levels(json: &str) -> std::result::Result<BTreeMap<Price, f32>, Box<dyn std::error::Error>> {
    let parsed: Vec<Vec<f32>> = serde_json::from_str(json)?;

    let mut levels = BTreeMap::new();
    for item in parsed {
        if item.len() >= 2 {
            let price = Price::from_f32(item[0]);
            let qty = item[1];
            levels.insert(price, qty);
        }
    }

    Ok(levels)
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

    fn create_test_depth() -> Depth {
        let mut bids = BTreeMap::new();
        let mut asks = BTreeMap::new();

        for i in 0..10 {
            bids.insert(Price::from_f32(50000.0 - i as f32), 1.0 + i as f32 * 0.1);
            asks.insert(Price::from_f32(50010.0 + i as f32), 1.0 + i as f32 * 0.1);
        }

        Depth { bids, asks }
    }

    #[test]
    fn test_serialize_deserialize_price_levels() {
        let mut levels = BTreeMap::new();
        levels.insert(Price::from_f32(50000.0), 1.5);
        levels.insert(Price::from_f32(50001.0), 2.5);
        levels.insert(Price::from_f32(50002.0), 3.5);

        let json = serialize_price_levels(&levels);
        let deserialized = deserialize_price_levels(&json).unwrap();

        assert_eq!(deserialized.len(), 3);
        assert!((deserialized.get(&Price::from_f32(50000.0)).unwrap() - 1.5).abs() < 0.01);
    }

    #[test]
    fn test_insert_and_query_depth_snapshot() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        let depth = create_test_depth();
        let snapshot_time = 1000000;

        // Insert snapshot
        let inserted = db
            .insert_depth_snapshot(&ticker_info, snapshot_time, &depth)
            .unwrap();
        assert_eq!(inserted, 1);

        // Query back
        let queried = db
            .query_depth_snapshot(&ticker_info, snapshot_time)
            .unwrap();

        assert!(queried.is_some());
        let queried_depth = queried.unwrap();
        assert_eq!(queried_depth.bids.len(), 10);
        assert_eq!(queried_depth.asks.len(), 10);
    }

    #[test]
    fn test_query_depth_snapshots_range() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        // Insert multiple snapshots
        for i in 0..5 {
            let depth = create_test_depth();
            let snapshot_time = 1000000 + i * 10000;
            db.insert_depth_snapshot(&ticker_info, snapshot_time, &depth)
                .unwrap();
        }

        // Query range
        let snapshots = db
            .query_depth_snapshots_range(&ticker_info, 1000000, 1050000)
            .unwrap();

        assert_eq!(snapshots.len(), 5);
        assert_eq!(snapshots[0].0, 1000000);
    }

    #[test]
    fn test_query_nonexistent_snapshot() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        let result = db
            .query_depth_snapshot(&ticker_info, 9999999)
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_delete_depth_snapshots_older_than() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        // Insert snapshots
        for i in 0..10 {
            let depth = create_test_depth();
            db.insert_depth_snapshot(&ticker_info, 1000000 + i * 10000, &depth)
                .unwrap();
        }

        // Delete old snapshots
        let cutoff = 1000000 + 50000;
        let deleted = db.delete_depth_snapshots_older_than(cutoff).unwrap();
        assert!(deleted >= 5);

        // Verify remaining
        let remaining = db
            .query_depth_snapshots_range(&ticker_info, 0, u64::MAX)
            .unwrap();
        assert!(remaining.len() <= 5);
    }

    #[test]
    fn test_depth_price_precision() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        let mut bids = BTreeMap::new();
        let mut asks = BTreeMap::new();

        // Test various precisions
        bids.insert(Price::from_f32(12345.67890123), 1.23456789);
        asks.insert(Price::from_f32(12346.12345678), 2.34567890);

        let depth = Depth { bids, asks };
        db.insert_depth_snapshot(&ticker_info, 1000000, &depth)
            .unwrap();

        let queried = db
            .query_depth_snapshot(&ticker_info, 1000000)
            .unwrap()
            .unwrap();

        // Check precision is maintained
        assert_eq!(queried.bids.len(), 1);
        assert_eq!(queried.asks.len(), 1);

        let bid_price = *queried.bids.keys().next().unwrap();
        let diff = (bid_price.to_f32() - 12345.67890123).abs();
        assert!(diff < 1e-4);
    }
}
