//! TimeSeries migration for klines and footprint data
//!
//! Migrates in-memory TimeSeries<KlineDataPoint> to DuckDB klines and footprint_data tables

use super::helpers::{generate_footprint_id, generate_kline_id, get_or_create_ticker_id, timeframe_to_string};
use super::{MigrationConfig, MigrationStats};
use crate::db::{DatabaseError, Result};
use duckdb::Connection;
use exchange::util::Price;
use exchange::{Kline, TickerInfo, Timeframe};
use std::collections::BTreeMap;

/// Simplified representation of grouped trades for migration
#[derive(Debug, Clone)]
pub struct FootprintData {
    pub price: Price,
    pub buy_qty: f32,
    pub sell_qty: f32,
    pub buy_count: usize,
    pub sell_count: usize,
    pub first_time: u64,
    pub last_time: u64,
}

/// Simplified representation of kline data point for migration
pub struct KlineData {
    pub kline: Kline,
    pub footprint: BTreeMap<Price, FootprintData>,
}

/// Migrates TimeSeries<KlineDataPoint> from memory to DuckDB
pub struct TimeSeriesMigrator {
    config: MigrationConfig,
}

impl TimeSeriesMigrator {
    pub fn new(config: MigrationConfig) -> Self {
        Self { config }
    }

    /// Migrate kline OHLCV data from TimeSeries to klines table
    ///
    /// Uses batch inserts for performance
    pub fn migrate_klines(
        &self,
        conn: &mut Connection,
        timeseries: &BTreeMap<u64, KlineData>,
        ticker_info: &TickerInfo,
        timeframe: Timeframe,
    ) -> Result<usize> {
        log::info!(
            "Migrating {} klines for {} ({})",
            timeseries.len(),
            ticker_info.ticker,
            timeframe
        );

        if self.config.dry_run {
            log::info!("Dry run mode - skipping actual inserts");
            return Ok(timeseries.len());
        }

        // Get or create ticker_id
        let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;
        let timeframe_str = timeframe_to_string(timeframe.to_milliseconds());

        let tx = conn.transaction().map_err(|e| {
            DatabaseError::Transaction(format!("Failed to start transaction: {}", e))
        })?;

        let mut count = 0;
        let mut batch = Vec::new();

        for (timestamp, kline_data) in timeseries {
            let kline = &kline_data.kline;
            let kline_id = generate_kline_id(ticker_id, timeframe.to_milliseconds(), *timestamp);

            batch.push((
                kline_id,
                ticker_id,
                timeframe_str.clone(),
                *timestamp as i64,
                kline.open.to_f32() as f64,
                kline.high.to_f32() as f64,
                kline.low.to_f32() as f64,
                kline.close.to_f32() as f64,
                kline.volume.0,
                kline.volume.1,
            ));

            if batch.len() >= self.config.batch_size {
                self.insert_kline_batch(&tx, &batch)?;
                count += batch.len();
                batch.clear();
            }
        }

        // Insert remaining batch
        if !batch.is_empty() {
            self.insert_kline_batch(&tx, &batch)?;
            count += batch.len();
        }

        tx.commit().map_err(|e| {
            DatabaseError::Transaction(format!("Failed to commit klines transaction: {}", e))
        })?;

        log::info!("Successfully migrated {} klines", count);
        Ok(count)
    }

    fn insert_kline_batch(
        &self,
        conn: &duckdb::Transaction,
        batch: &[(i64, i64, String, i64, f64, f64, f64, f64, f32, f32)],
    ) -> Result<()> {
        let mut stmt = conn
            .prepare(
                "INSERT OR REPLACE INTO klines
                 (kline_id, ticker_id, timeframe, candle_time, open_price, high_price,
                  low_price, close_price, base_volume, quote_volume)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .map_err(|e| DatabaseError::Query(format!("Failed to prepare statement: {}", e)))?;

        for row in batch {
            stmt.execute(duckdb::params![
                row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8, row.9
            ])
            .map_err(|e| DatabaseError::Insert(format!("Failed to insert kline: {}", e)))?;
        }

        Ok(())
    }

    /// Migrate footprint data (price-level trade aggregation) to footprint_data table
    ///
    /// Extracts trade groups from each datapoint and inserts them
    pub fn migrate_footprints(
        &self,
        conn: &mut Connection,
        timeseries: &BTreeMap<u64, KlineData>,
        ticker_info: &TickerInfo,
        timeframe: Timeframe,
    ) -> Result<usize> {
        log::info!(
            "Migrating footprints for {} ({})",
            ticker_info.ticker,
            timeframe
        );

        if self.config.dry_run {
            log::info!("Dry run mode - skipping actual inserts");
            let total: usize = timeseries
                .values()
                .map(|kd| kd.footprint.len())
                .sum();
            return Ok(total);
        }

        // Get or create ticker_id
        let ticker_id = get_or_create_ticker_id(conn, ticker_info)?;

        let tx = conn.transaction().map_err(|e| {
            DatabaseError::Transaction(format!("Failed to start transaction: {}", e))
        })?;

        let mut count = 0;
        let mut batch = Vec::new();

        for (timestamp, kline_data) in timeseries {
            let kline_id = generate_kline_id(ticker_id, timeframe.to_milliseconds(), *timestamp);

            for (price, group) in &kline_data.footprint {
                let footprint_id = generate_footprint_id(kline_id, price.units);

                batch.push((
                    footprint_id,
                    kline_id,
                    price.to_f32() as f64,
                    group.buy_qty,
                    group.sell_qty,
                    group.buy_count as i64,
                    group.sell_count as i64,
                    group.first_time as i64,
                    group.last_time as i64,
                ));

                if batch.len() >= self.config.batch_size {
                    self.insert_footprint_batch(&tx, &batch)?;
                    count += batch.len();
                    batch.clear();
                }
            }
        }

        // Insert remaining batch
        if !batch.is_empty() {
            self.insert_footprint_batch(&tx, &batch)?;
            count += batch.len();
        }

        tx.commit().map_err(|e| {
            DatabaseError::Transaction(format!("Failed to commit footprints transaction: {}", e))
        })?;

        log::info!("Successfully migrated {} footprint records", count);
        Ok(count)
    }

    fn insert_footprint_batch(
        &self,
        conn: &duckdb::Transaction,
        batch: &[(i64, i64, f64, f32, f32, i64, i64, i64, i64)],
    ) -> Result<()> {
        let mut stmt = conn
            .prepare(
                "INSERT OR REPLACE INTO footprint_data
                 (footprint_id, kline_id, price, buy_quantity, sell_quantity,
                  buy_count, sell_count, first_trade_time, last_trade_time)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .map_err(|e| DatabaseError::Query(format!("Failed to prepare statement: {}", e)))?;

        for row in batch {
            stmt.execute(duckdb::params![
                row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8
            ])
            .map_err(|e| {
                DatabaseError::Insert(format!("Failed to insert footprint: {}", e))
            })?;
        }

        Ok(())
    }

    /// Convenience function to migrate both klines and footprints in one call
    pub fn migrate_timeseries(
        &self,
        conn: &mut Connection,
        timeseries: &BTreeMap<u64, KlineData>,
        ticker_info: &TickerInfo,
        timeframe: Timeframe,
    ) -> Result<MigrationStats> {
        let mut stats = MigrationStats::new();

        match self.migrate_klines(conn, timeseries, ticker_info, timeframe) {
            Ok(count) => stats.klines_migrated = count,
            Err(e) => {
                let err_msg = format!("Failed to migrate klines: {}", e);
                log::error!("{}", err_msg);
                stats.add_error(err_msg);
                return Ok(stats);
            }
        }

        match self.migrate_footprints(conn, timeseries, ticker_info, timeframe) {
            Ok(count) => stats.footprints_migrated = count,
            Err(e) => {
                let err_msg = format!("Failed to migrate footprints: {}", e);
                log::error!("{}", err_msg);
                stats.add_error(err_msg);
            }
        }

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exchange::adapter::Exchange;
    use exchange::Ticker;
    use tempfile::TempDir;

    #[test]
    fn test_timeseries_migrator_creation() {
        let config = MigrationConfig::default();
        let migrator = TimeSeriesMigrator::new(config);
        assert_eq!(migrator.config.batch_size, 1000);
    }
}
