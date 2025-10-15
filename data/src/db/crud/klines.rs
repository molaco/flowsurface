//! Kline CRUD operations with TimeSeries reconstruction

use crate::db::error::{DatabaseError, Result};
use crate::db::helpers::{decimal_to_price, generate_kline_id, get_or_create_ticker_id, price_to_decimal};
use crate::db::DatabaseManager;
use crate::db::crud::KlinesCRUD;
use duckdb::OptionalExt;
use crate::aggr::time::TimeSeries;
use crate::chart::kline::{KlineDataPoint, KlineTrades};
use exchange::util::PriceStep;
use exchange::{Kline, TickerInfo, Timeframe};
use std::collections::BTreeMap;

impl KlinesCRUD for DatabaseManager {
    /// Insert or update klines using INSERT OR REPLACE
    ///
    /// The UNIQUE constraint on (ticker_id, timeframe, candle_time) ensures
    /// that duplicate klines are replaced rather than creating duplicates
    fn insert_klines(
        &self,
        ticker_info: &TickerInfo,
        timeframe: Timeframe,
        klines: &[Kline],
    ) -> Result<usize> {
        if klines.is_empty() {
            return Ok(0);
        }

        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;
            let timeframe_str = format!("{}", timeframe);

            let mut stmt = conn
                .prepare(
                    "INSERT INTO klines (kline_id, ticker_id, timeframe, candle_time, open_price, high_price, low_price, close_price, volume, num_trades)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT (ticker_id, timeframe, candle_time) DO UPDATE SET
                         open_price = EXCLUDED.open_price,
                         high_price = EXCLUDED.high_price,
                         low_price = EXCLUDED.low_price,
                         close_price = EXCLUDED.close_price,
                         volume = EXCLUDED.volume,
                         num_trades = EXCLUDED.num_trades",
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to prepare insert: {}", e)))?;

            for kline in klines {
                let kline_id = generate_kline_id(ticker_id, &timeframe_str, kline.time);

                stmt.execute(duckdb::params![
                    kline_id,
                    ticker_id,
                    timeframe_str.clone(),
                    kline.time as i64,
                    price_to_decimal(kline.open),
                    price_to_decimal(kline.high),
                    price_to_decimal(kline.low),
                    price_to_decimal(kline.close),
                    (kline.volume.0 + kline.volume.1) as f64, // Total volume
                    0i32, // num_trades (can be updated later if available)
                ])
                .map_err(|e| DatabaseError::Query(format!("Failed to insert kline: {}", e)))?;
            }

            Ok(klines.len())
        })
    }

    /// Query klines by timeframe and time range
    fn query_klines(
        &self,
        ticker_info: &TickerInfo,
        timeframe: Timeframe,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<Kline>> {
        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;
            let timeframe_str = format!("{}", timeframe);

            let mut stmt = conn
                .prepare(
                    "SELECT candle_time, open_price, high_price, low_price, close_price, volume
                     FROM klines
                     WHERE ticker_id = ? AND timeframe = ? AND candle_time >= ? AND candle_time <= ?
                     ORDER BY candle_time ASC",
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to prepare query: {}", e)))?;

            let klines_iter = stmt
                .query_map(
                    duckdb::params![ticker_id, timeframe_str, start_time as i64, end_time as i64],
                    |row| {
                        let candle_time: i64 = row.get(0)?;
                        let open_price: f64 = row.get(1)?;
                        let high_price: f64 = row.get(2)?;
                        let low_price: f64 = row.get(3)?;
                        let close_price: f64 = row.get(4)?;
                        let volume: f64 = row.get(5)?;

                        Ok(Kline {
                            time: candle_time as u64,
                            open: decimal_to_price(open_price),
                            high: decimal_to_price(high_price),
                            low: decimal_to_price(low_price),
                            close: decimal_to_price(close_price),
                            volume: (volume as f32 / 2.0, volume as f32 / 2.0), // Split evenly
                        })
                    },
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to query klines: {}", e)))?;

            let mut klines = Vec::new();
            for kline_result in klines_iter {
                klines.push(
                    kline_result
                        .map_err(|e| DatabaseError::Query(format!("Failed to map kline: {}", e)))?,
                );
            }

            Ok(klines)
        })
    }

    /// Load klines as TimeSeries for chart rendering
    ///
    /// Reconstructs TimeSeries<KlineDataPoint> with proper interval and tick_size
    fn load_timeseries(
        &self,
        ticker_info: &TickerInfo,
        timeframe: Timeframe,
        start_time: u64,
        end_time: u64,
    ) -> Result<TimeSeries<KlineDataPoint>> {
        let klines = self.query_klines(ticker_info, timeframe, start_time, end_time)?;

        let mut datapoints = BTreeMap::new();

        for kline in klines {
            let datapoint = KlineDataPoint {
                kline,
                footprint: KlineTrades::new(),
            };
            datapoints.insert(kline.time, datapoint);
        }

        // Calculate tick size from min_ticksize
        let tick_size = PriceStep::from_f32(ticker_info.min_ticksize.as_f32());

        Ok(TimeSeries {
            datapoints,
            interval: timeframe,
            tick_size,
        })
    }

    /// Get the most recent kline for a timeframe
    fn query_latest_kline(
        &self,
        ticker_info: &TickerInfo,
        timeframe: Timeframe,
    ) -> Result<Option<Kline>> {
        self.with_conn(|conn| {
            let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;
            let timeframe_str = format!("{}", timeframe);

            let kline_opt = conn
                .query_row(
                    "SELECT candle_time, open_price, high_price, low_price, close_price, volume
                     FROM klines
                     WHERE ticker_id = ? AND timeframe = ?
                     ORDER BY candle_time DESC
                     LIMIT 1",
                    duckdb::params![ticker_id, timeframe_str],
                    |row| {
                        let candle_time: i64 = row.get(0)?;
                        let open_price: f64 = row.get(1)?;
                        let high_price: f64 = row.get(2)?;
                        let low_price: f64 = row.get(3)?;
                        let close_price: f64 = row.get(4)?;
                        let volume: f64 = row.get(5)?;

                        Ok(Kline {
                            time: candle_time as u64,
                            open: decimal_to_price(open_price),
                            high: decimal_to_price(high_price),
                            low: decimal_to_price(low_price),
                            close: decimal_to_price(close_price),
                            volume: (volume as f32 / 2.0, volume as f32 / 2.0),
                        })
                    },
                )
                .optional()
                .map_err(|e| DatabaseError::Query(format!("Failed to query latest kline: {}", e)))?;

            Ok(kline_opt)
        })
    }

    /// Delete klines older than cutoff timestamp
    fn delete_klines_older_than(&self, cutoff_time: u64) -> Result<usize> {
        self.with_conn(|conn| {
            let deleted = conn
                .execute(
                    "DELETE FROM klines WHERE candle_time < ?",
                    [cutoff_time as i64],
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to delete klines: {}", e)))?;

            Ok(deleted)
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

    fn create_test_klines(count: usize, timeframe: Timeframe) -> Vec<Kline> {
        let interval_ms = timeframe.to_milliseconds();
        (0..count)
            .map(|i| Kline {
                time: 1000000 + (i as u64 * interval_ms),
                open: Price::from_f32(50000.0 + i as f32),
                high: Price::from_f32(50100.0 + i as f32),
                low: Price::from_f32(49900.0 + i as f32),
                close: Price::from_f32(50050.0 + i as f32),
                volume: (100.0, 100.0),
            })
            .collect()
    }

    #[test]
    fn test_insert_and_query_klines() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();
        let timeframe = Timeframe::M1;

        let klines = create_test_klines(50, timeframe);

        // Insert klines
        let inserted = db.insert_klines(&ticker_info, timeframe, &klines).unwrap();
        assert_eq!(inserted, 50);

        // Query back
        let queried = db
            .query_klines(&ticker_info, timeframe, 0, u64::MAX)
            .unwrap();

        assert_eq!(queried.len(), 50);
        assert_eq!(queried[0].time, klines[0].time);
    }

    #[test]
    fn test_insert_or_replace() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();
        let timeframe = Timeframe::M1;

        let klines = create_test_klines(10, timeframe);

        // Insert first time
        db.insert_klines(&ticker_info, timeframe, &klines).unwrap();

        // Insert again (should replace)
        db.insert_klines(&ticker_info, timeframe, &klines).unwrap();

        // Should still only have 10 klines
        let queried = db
            .query_klines(&ticker_info, timeframe, 0, u64::MAX)
            .unwrap();
        assert_eq!(queried.len(), 10);
    }

    #[test]
    fn test_load_timeseries() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();
        let timeframe = Timeframe::M5;

        let klines = create_test_klines(20, timeframe);
        db.insert_klines(&ticker_info, timeframe, &klines).unwrap();

        // Load as TimeSeries
        let timeseries = db
            .load_timeseries(&ticker_info, timeframe, 0, u64::MAX)
            .unwrap();

        assert_eq!(timeseries.datapoints.len(), 20);
        assert_eq!(timeseries.interval, timeframe);
    }

    #[test]
    fn test_query_latest_kline() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();
        let timeframe = Timeframe::H1;

        let klines = create_test_klines(30, timeframe);
        db.insert_klines(&ticker_info, timeframe, &klines).unwrap();

        // Get latest
        let latest = db.query_latest_kline(&ticker_info, timeframe).unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().time, klines[29].time);
    }

    #[test]
    fn test_query_latest_kline_empty() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        let latest = db
            .query_latest_kline(&ticker_info, Timeframe::M1)
            .unwrap();
        assert!(latest.is_none());
    }

    #[test]
    fn test_delete_klines_older_than() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();
        let timeframe = Timeframe::M15;

        let klines = create_test_klines(100, timeframe);
        db.insert_klines(&ticker_info, timeframe, &klines).unwrap();

        // Delete old klines
        let cutoff = klines[50].time;
        let deleted = db.delete_klines_older_than(cutoff).unwrap();
        assert!(deleted >= 50);

        // Verify remaining
        let remaining = db
            .query_klines(&ticker_info, timeframe, 0, u64::MAX)
            .unwrap();
        assert!(remaining.len() <= 50);
    }

    #[test]
    fn test_multiple_timeframes() {
        let (db, _dir) = create_test_db();
        let ticker_info = create_test_ticker_info();

        // Insert klines for multiple timeframes
        for timeframe in [Timeframe::M1, Timeframe::M5, Timeframe::H1] {
            let klines = create_test_klines(10, timeframe);
            db.insert_klines(&ticker_info, timeframe, &klines).unwrap();
        }

        // Query each timeframe
        for timeframe in [Timeframe::M1, Timeframe::M5, Timeframe::H1] {
            let klines = db
                .query_klines(&ticker_info, timeframe, 0, u64::MAX)
                .unwrap();
            assert_eq!(klines.len(), 10);
        }
    }
}
