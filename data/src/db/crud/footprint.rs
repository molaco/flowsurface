//! Footprint data CRUD for price-level aggregations within klines

use crate::db::error::{DatabaseError, Result};
use crate::db::helpers::{
    decimal_to_price, generate_footprint_id, get_or_create_ticker_id, price_to_decimal,
};
use crate::db::DatabaseManager;
use crate::db::crud::{FootprintCRUD, KlinesCRUD};
use crate::aggr::time::TimeSeries;
use crate::chart::kline::{GroupedTrades, KlineDataPoint, KlineTrades};
use exchange::util::Price;
use exchange::{TickerInfo, Timeframe};
use rustc_hash::FxHashMap;
use std::collections::BTreeMap;

impl FootprintCRUD for DatabaseManager {
    /// Insert footprint data for a kline
    ///
    /// Stores each price level as a separate row with buy/sell volume breakdown
    fn insert_footprint(
        &self,
        ticker_info: &TickerInfo,
        timeframe: Timeframe,
        kline_time: u64,
        footprint: &KlineTrades,
    ) -> Result<usize> {
        if footprint.trades.is_empty() {
            return Ok(0);
        }

        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;
            let timeframe_str = format!("{}", timeframe);

            // Delete existing footprint data for this kline first
            conn.execute(
                "DELETE FROM footprint_data WHERE ticker_id = ? AND candle_time = ? AND timeframe = ?",
                duckdb::params![ticker_id, kline_time as i64, timeframe_str.clone()],
            )
            .map_err(|e| DatabaseError::Query(format!("Failed to delete old footprint: {}", e)))?;

            let mut stmt = conn
                .prepare(
                    "INSERT INTO footprint_data
                     (footprint_id, ticker_id, candle_time, timeframe, price_level, buy_volume, sell_volume, delta, num_trades)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to prepare insert: {}", e)))?;

            let mut count = 0;
            for (price, grouped_trades) in &footprint.trades {
                let footprint_id = generate_footprint_id(ticker_id, &timeframe_str, kline_time, *price);

                stmt.execute(duckdb::params![
                    footprint_id,
                    ticker_id,
                    kline_time as i64,
                    timeframe_str.clone(),
                    price_to_decimal(*price),
                    grouped_trades.buy_qty as f64,
                    grouped_trades.sell_qty as f64,
                    grouped_trades.delta_qty() as f64,
                    (grouped_trades.buy_count + grouped_trades.sell_count) as i32,
                ])
                .map_err(|e| {
                    DatabaseError::Query(format!("Failed to insert footprint level: {}", e))
                })?;

                count += 1;
            }

            Ok(count)
        })
    }

    /// Query footprint data for a specific kline
    fn query_footprint(
        &self,
        ticker_info: &TickerInfo,
        timeframe: Timeframe,
        kline_time: u64,
    ) -> Result<Option<KlineTrades>> {
        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;
            let timeframe_str = format!("{}", timeframe);

            let mut stmt = conn
                .prepare(
                    "SELECT price_level, buy_volume, sell_volume, num_trades
                     FROM footprint_data
                     WHERE ticker_id = ? AND candle_time = ? AND timeframe = ?",
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to prepare query: {}", e)))?;

            let levels_iter = stmt
                .query_map(
                    duckdb::params![ticker_id, kline_time as i64, timeframe_str],
                    |row| {
                        let price_level: f64 = row.get(0)?;
                        let buy_volume: f64 = row.get(1)?;
                        let sell_volume: f64 = row.get(2)?;
                        let num_trades: i32 = row.get(3)?;

                        Ok((
                            decimal_to_price(price_level),
                            buy_volume as f32,
                            sell_volume as f32,
                            num_trades as usize,
                        ))
                    },
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to query footprint: {}", e)))?;

            let mut trades = FxHashMap::default();
            let mut has_data = false;

            for level_result in levels_iter {
                has_data = true;
                let (price, buy_vol, sell_vol, num_trades) = level_result.map_err(|e| {
                    DatabaseError::Query(format!("Failed to map footprint level: {}", e))
                })?;

                let grouped = GroupedTrades {
                    buy_qty: buy_vol,
                    sell_qty: sell_vol,
                    first_time: kline_time, // Approximation
                    last_time: kline_time,  // Approximation
                    buy_count: num_trades / 2,
                    sell_count: num_trades / 2,
                };

                trades.insert(price, grouped);
            }

            if !has_data {
                return Ok(None);
            }

            let mut kline_trades = KlineTrades { trades, poc: None };

            // Calculate POC from the reconstructed data
            kline_trades.calculate_poc();

            Ok(Some(kline_trades))
        })
    }

    /// Query footprints for multiple klines in time range
    fn query_footprints_range(
        &self,
        ticker_info: &TickerInfo,
        timeframe: Timeframe,
        start_time: u64,
        end_time: u64,
    ) -> Result<BTreeMap<u64, KlineTrades>> {
        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;
            let timeframe_str = format!("{}", timeframe);

            let mut stmt = conn
                .prepare(
                    "SELECT candle_time, price_level, buy_volume, sell_volume, num_trades
                     FROM footprint_data
                     WHERE ticker_id = ? AND timeframe = ? AND candle_time >= ? AND candle_time <= ?
                     ORDER BY candle_time, price_level",
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to prepare query: {}", e)))?;

            let levels_iter = stmt
                .query_map(
                    duckdb::params![ticker_id, timeframe_str, start_time as i64, end_time as i64],
                    |row| {
                        let candle_time: i64 = row.get(0)?;
                        let price_level: f64 = row.get(1)?;
                        let buy_volume: f64 = row.get(2)?;
                        let sell_volume: f64 = row.get(3)?;
                        let num_trades: i32 = row.get(4)?;

                        Ok((
                            candle_time as u64,
                            decimal_to_price(price_level),
                            buy_volume as f32,
                            sell_volume as f32,
                            num_trades as usize,
                        ))
                    },
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to query footprints: {}", e)))?;

            let mut footprints: BTreeMap<u64, KlineTrades> = BTreeMap::new();

            for level_result in levels_iter {
                let (candle_time, price, buy_vol, sell_vol, num_trades) = level_result
                    .map_err(|e| {
                        DatabaseError::Query(format!("Failed to map footprint level: {}", e))
                    })?;

                let kline_trades = footprints
                    .entry(candle_time)
                    .or_insert_with(|| KlineTrades {
                        trades: FxHashMap::default(),
                        poc: None,
                    });

                let grouped = GroupedTrades {
                    buy_qty: buy_vol,
                    sell_qty: sell_vol,
                    first_time: candle_time,
                    last_time: candle_time,
                    buy_count: num_trades / 2,
                    sell_count: num_trades / 2,
                };

                kline_trades.trades.insert(price, grouped);
            }

            // Calculate POC for each kline
            for kline_trades in footprints.values_mut() {
                kline_trades.calculate_poc();
            }

            Ok(footprints)
        })
    }

    /// Load TimeSeries with both klines and footprints
    ///
    /// This is the most efficient way to load complete chart data as it
    /// combines kline and footprint queries
    fn load_timeseries_with_footprints(
        &self,
        ticker_info: &TickerInfo,
        timeframe: Timeframe,
        start_time: u64,
        end_time: u64,
    ) -> Result<TimeSeries<KlineDataPoint>> {
        // First load the klines
        let mut timeseries = self.load_timeseries(ticker_info, timeframe, start_time, end_time)?;

        // Then load footprints for the same range
        let footprints = self.query_footprints_range(ticker_info, timeframe, start_time, end_time)?;

        // Merge footprints into the timeseries
        for (candle_time, footprint) in footprints {
            if let Some(datapoint) = timeseries.datapoints.get_mut(&candle_time) {
                datapoint.footprint = footprint;
            }
        }

        Ok(timeseries)
    }

    /// Delete footprint data older than cutoff
    fn delete_footprints_older_than(&self, cutoff_time: u64) -> Result<usize> {
        self.with_conn(|conn| {
            let deleted = conn
                .execute(
                    "DELETE FROM footprint_data WHERE candle_time < ?",
                    [cutoff_time as i64],
                )
                .map_err(|e| {
                    DatabaseError::Query(format!("Failed to delete footprints: {}", e))
                })?;

            Ok(deleted)
        })
    }
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

    fn create_test_footprint() -> KlineTrades {
        let mut trades = FxHashMap::default();

        for i in 0..20 {
            let price = Price::from_f32(50000.0 + i as f32);
            let grouped = GroupedTrades {
                buy_qty: 10.0 + i as f32,
                sell_qty: 8.0 + i as f32 * 0.5,
                first_time: 1000000,
                last_time: 1000100,
                buy_count: 10,
                sell_count: 8,
            };
            trades.insert(price, grouped);
        }

        let mut footprint = KlineTrades { trades, poc: None };
        footprint.calculate_poc();
        footprint
    }

    #[test]
    fn test_insert_and_query_footprint() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();
        let timeframe = Timeframe::M1;

        let footprint = create_test_footprint();
        let kline_time = 1000000;

        // Insert footprint
        let inserted = db
            .insert_footprint(&ticker_info, timeframe, kline_time, &footprint)
            .unwrap();
        assert_eq!(inserted, 20);

        // Query back
        let queried = db
            .query_footprint(&ticker_info, timeframe, kline_time)
            .unwrap();

        assert!(queried.is_some());
        let queried_footprint = queried.unwrap();
        assert_eq!(queried_footprint.trades.len(), 20);
        assert!(queried_footprint.poc.is_some());
    }

    #[test]
    fn test_query_nonexistent_footprint() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        let result = db
            .query_footprint(&ticker_info, Timeframe::M1, 9999999)
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_query_footprints_range() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();
        let timeframe = Timeframe::M5;

        // Insert multiple footprints
        for i in 0..5 {
            let footprint = create_test_footprint();
            let kline_time = 1000000 + i * 300000; // 5 minute intervals
            db.insert_footprint(&ticker_info, timeframe, kline_time, &footprint)
                .unwrap();
        }

        // Query range
        let footprints = db
            .query_footprints_range(&ticker_info, timeframe, 1000000, 2000000)
            .unwrap();

        assert_eq!(footprints.len(), 5);
        assert!(footprints.values().all(|f| f.trades.len() == 20));
    }

    #[test]
    fn test_load_timeseries_with_footprints() {
        use exchange::Kline;

        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();
        let timeframe = Timeframe::M1;

        // Insert klines
        let klines: Vec<Kline> = (0..10)
            .map(|i| Kline {
                time: 1000000 + i * 60000,
                open: Price::from_f32(50000.0),
                high: Price::from_f32(50100.0),
                low: Price::from_f32(49900.0),
                close: Price::from_f32(50050.0),
                volume: (100.0, 100.0),
            })
            .collect();

        db.insert_klines(&ticker_info, timeframe, &klines).unwrap();

        // Insert footprints for some klines
        for i in 0..5 {
            let footprint = create_test_footprint();
            let kline_time = 1000000 + i * 60000;
            db.insert_footprint(&ticker_info, timeframe, kline_time, &footprint)
                .unwrap();
        }

        // Load timeseries with footprints
        let timeseries = db
            .load_timeseries_with_footprints(&ticker_info, timeframe, 1000000, 2000000)
            .unwrap();

        assert_eq!(timeseries.datapoints.len(), 10);

        // Check that first 5 have footprints, rest have empty footprints
        let mut count_with_footprints = 0;
        for datapoint in timeseries.datapoints.values() {
            if !datapoint.footprint.trades.is_empty() {
                count_with_footprints += 1;
            }
        }

        assert_eq!(count_with_footprints, 5);
    }

    #[test]
    fn test_delete_footprints_older_than() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();
        let timeframe = Timeframe::M15;

        // Insert footprints
        for i in 0..10 {
            let footprint = create_test_footprint();
            let kline_time = 1000000 + i * 900000; // 15 minute intervals
            db.insert_footprint(&ticker_info, timeframe, kline_time, &footprint)
                .unwrap();
        }

        // Delete old footprints
        let cutoff = 1000000 + 4500000;
        let deleted = db.delete_footprints_older_than(cutoff).unwrap();
        assert!(deleted >= 100); // 5 klines * 20 price levels each

        // Verify remaining
        let remaining = db
            .query_footprints_range(&ticker_info, timeframe, 0, u64::MAX)
            .unwrap();
        assert!(remaining.len() <= 5);
    }

    #[test]
    fn test_footprint_replace() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();
        let timeframe = Timeframe::M1;
        let kline_time = 1000000;

        // Insert first footprint
        let footprint1 = create_test_footprint();
        db.insert_footprint(&ticker_info, timeframe, kline_time, &footprint1)
            .unwrap();

        // Insert again (should replace)
        let footprint2 = create_test_footprint();
        db.insert_footprint(&ticker_info, timeframe, kline_time, &footprint2)
            .unwrap();

        // Query should still return single footprint
        let queried = db
            .query_footprint(&ticker_info, timeframe, kline_time)
            .unwrap()
            .unwrap();

        assert_eq!(queried.trades.len(), 20);
    }
}
