//! DepthMigrator for order book run data
//!
//! Migrates HistoricalDepth order runs to order_runs table

use super::helpers::{generate_run_id, get_or_create_ticker_id};
use super::{MigrationConfig, MigrationStats};
use crate::db::{DatabaseError, Result};
use duckdb::Connection;
use exchange::util::Price;
use exchange::TickerInfo;
use std::collections::BTreeMap;

/// Simplified representation of an order run for migration
#[derive(Debug, Clone, Copy)]
pub struct OrderRunData {
    pub start_time: u64,
    pub until_time: u64,
    pub qty: f32,
    pub is_bid: bool,
}

/// Migrates HistoricalDepth order runs to order_runs table
pub struct DepthMigrator {
    config: MigrationConfig,
}

impl DepthMigrator {
    pub fn new(config: MigrationConfig) -> Self {
        Self { config }
    }

    /// Migrate all order runs from HistoricalDepth BTreeMap to database
    ///
    /// Iterates through price_levels and inserts each OrderRun
    pub fn migrate_historical_depth(
        &self,
        conn: &mut Connection,
        price_levels: &BTreeMap<Price, Vec<OrderRunData>>,
        ticker_info: &TickerInfo,
    ) -> Result<MigrationStats> {
        log::info!(
            "Migrating order runs for {}",
            ticker_info.ticker
        );

        let mut stats = MigrationStats::new();

        if self.config.dry_run {
            log::info!("Dry run mode - skipping actual inserts");
            let total: usize = price_levels.values().map(|runs| runs.len()).sum();
            stats.runs_migrated = total;
            return Ok(stats);
        }

        // Get or create ticker_id
        let ticker_id = match get_or_create_ticker_id(conn, ticker_info) {
            Ok(id) => id,
            Err(e) => {
                let err_msg = format!("Failed to get ticker_id: {}", e);
                log::error!("{}", err_msg);
                stats.add_error(err_msg);
                return Ok(stats);
            }
        };

        let tx = conn.transaction().map_err(|e| {
            DatabaseError::Transaction(format!("Failed to start transaction: {}", e))
        })?;

        let mut batch = Vec::new();
        let mut count = 0;

        for (price, runs) in price_levels {
            for run in runs {
                let run_id = generate_run_id(ticker_id, price.units, run.start_time);

                batch.push((
                    run_id,
                    ticker_id,
                    price.to_f32() as f64,
                    run.start_time as i64,
                    run.until_time as i64,
                    run.qty,
                    run.is_bid,
                ));

                if batch.len() >= self.config.batch_size {
                    match self.batch_insert_runs(&tx, &batch) {
                        Ok(_) => {
                            count += batch.len();
                            batch.clear();
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to insert batch: {}", e);
                            log::error!("{}", err_msg);
                            stats.add_error(err_msg);
                            return Ok(stats);
                        }
                    }
                }
            }
        }

        // Insert remaining batch
        if !batch.is_empty() {
            match self.batch_insert_runs(&tx, &batch) {
                Ok(_) => count += batch.len(),
                Err(e) => {
                    let err_msg = format!("Failed to insert final batch: {}", e);
                    log::error!("{}", err_msg);
                    stats.add_error(err_msg);
                    return Ok(stats);
                }
            }
        }

        tx.commit().map_err(|e| {
            DatabaseError::Transaction(format!("Failed to commit order runs transaction: {}", e))
        })?;

        log::info!("Successfully migrated {} order runs", count);
        stats.runs_migrated = count;
        Ok(stats)
    }

    /// Uses batch inserts for efficient bulk insertion of order runs
    fn batch_insert_runs(
        &self,
        conn: &duckdb::Transaction,
        batch: &[(i64, i64, f64, i64, i64, f32, bool)],
    ) -> Result<()> {
        let mut stmt = conn
            .prepare(
                "INSERT OR REPLACE INTO order_runs
                 (run_id, ticker_id, price, start_time, until_time, quantity, is_bid)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .map_err(|e| DatabaseError::Query(format!("Failed to prepare statement: {}", e)))?;

        for row in batch {
            stmt.execute(duckdb::params![
                row.0, row.1, row.2, row.3, row.4, row.5, row.6
            ])
            .map_err(|e| DatabaseError::Insert(format!("Failed to insert order run: {}", e)))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_depth_migrator_creation() {
        let config = MigrationConfig::default();
        let migrator = DepthMigrator::new(config);
        assert_eq!(migrator.config.batch_size, 1000);
    }
}
