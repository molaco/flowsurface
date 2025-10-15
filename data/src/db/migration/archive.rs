//! ArchiveMigrator for Binance ZIP archives
//!
//! Parses Binance ZIP archives and bulk-loads trades into database

use super::helpers::{generate_trade_id, get_or_create_ticker_id};
use super::{MigrationConfig, MigrationStats, ProgressTracker};
use crate::db::{DatabaseError, Result};
use duckdb::Connection;
use exchange::adapter::Exchange;
use exchange::{Ticker, TickerInfo};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Parses Binance ZIP archives and bulk-loads trades into database
pub struct ArchiveMigrator {
    config: MigrationConfig,
}

impl ArchiveMigrator {
    pub fn new(config: MigrationConfig) -> Self {
        Self { config }
    }

    /// Walk directory tree finding all ZIP files and migrate each one
    ///
    /// Returns aggregated statistics across all processed archives
    pub fn migrate_zip_archives(
        &self,
        conn: &mut Connection,
        market_data_path: &Path,
    ) -> Result<MigrationStats> {
        log::info!(
            "Scanning for ZIP archives in: {}",
            market_data_path.display()
        );

        let mut all_stats = MigrationStats::new();

        if !market_data_path.exists() {
            let err_msg = format!("Market data path does not exist: {}", market_data_path.display());
            log::warn!("{}", err_msg);
            all_stats.add_error(err_msg);
            return Ok(all_stats);
        }

        // Collect all ZIP files
        let zip_files = self.find_zip_files(market_data_path)?;
        log::info!("Found {} ZIP archives to process", zip_files.len());

        if zip_files.is_empty() {
            return Ok(all_stats);
        }

        let mut tracker = ProgressTracker::new(zip_files.len(), "Archive Migration");

        for (i, zip_path) in zip_files.iter().enumerate() {
            log::debug!("Processing archive {}/{}: {}", i + 1, zip_files.len(), zip_path.display());

            match self.migrate_single_archive(conn, zip_path) {
                Ok(stats) => {
                    all_stats.merge(&stats);
                    all_stats.files_processed += 1;
                }
                Err(e) => {
                    let err_msg = format!("{}: {}", zip_path.display(), e);
                    log::error!("Failed to migrate archive: {}", err_msg);
                    all_stats.add_error(err_msg);
                }
            }

            tracker.update(1);
        }

        tracker.finish();
        log::info!(
            "Archive migration complete: {} files processed, {} trades migrated",
            all_stats.files_processed,
            all_stats.trades_migrated
        );

        Ok(all_stats)
    }

    /// Process a single ZIP file containing Binance aggTrades CSV
    ///
    /// Streams CSV parsing to avoid memory issues on large files (500MB+)
    pub fn migrate_single_archive(&self, conn: &mut Connection, zip_path: &Path) -> Result<MigrationStats> {
        let mut stats = MigrationStats::new();

        if self.config.dry_run {
            log::info!("Dry run mode - skipping archive: {}", zip_path.display());
            return Ok(stats);
        }

        // Parse metadata from path
        let (ticker_info, _date) = match self.parse_archive_path(zip_path) {
            Ok(result) => result,
            Err(e) => {
                let err_msg = format!("Failed to parse archive path: {}", e);
                stats.add_error(err_msg);
                return Ok(stats);
            }
        };

        // Get or create ticker_id
        let ticker_id = match get_or_create_ticker_id(conn, &ticker_info) {
            Ok(id) => id,
            Err(e) => {
                let err_msg = format!("Failed to get ticker_id: {}", e);
                stats.add_error(err_msg);
                return Ok(stats);
            }
        };

        // Open ZIP archive
        let file = fs::File::open(zip_path).map_err(|e| {
            DatabaseError::Migration(format!("Failed to open ZIP archive: {}", e))
        })?;

        let mut archive = zip::ZipArchive::new(file).map_err(|e| {
            DatabaseError::Migration(format!("Failed to read ZIP archive: {}", e))
        })?;

        // Process each file in archive (typically just one CSV)
        for i in 0..archive.len() {
            let file = archive.by_index(i).map_err(|e| {
                DatabaseError::Migration(format!("Failed to access archive entry: {}", e))
            })?;

            let count = match self.stream_csv_insert(conn, file, ticker_id) {
                Ok(c) => c,
                Err(e) => {
                    let err_msg = format!("Failed to process CSV: {}", e);
                    stats.add_error(err_msg);
                    return Ok(stats);
                }
            };

            stats.trades_migrated += count;
        }

        Ok(stats)
    }

    /// Extract symbol and date from Binance archive filesystem path
    ///
    /// Path format: market_data/binance/data/futures/um/daily/aggTrades/BTCUSDT/BTCUSDT-aggTrades-2024-01-15.zip
    pub fn parse_archive_path(&self, zip_path: &Path) -> Result<(TickerInfo, String)> {
        let file_name = zip_path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| DatabaseError::Migration("Invalid file name".to_string()))?;

        // Extract symbol from filename (e.g., BTCUSDT-aggTrades-2024-01-15.zip)
        let parts: Vec<&str> = file_name.split('-').collect();
        if parts.len() < 4 {
            return Err(DatabaseError::Migration(format!(
                "Invalid Binance archive filename format: {}",
                file_name
            )));
        }

        let symbol = parts[0];
        let date = format!("{}-{}-{}", parts[2], parts[3], parts[4].trim_end_matches(".zip"));

        // Determine exchange from path (binance futures)
        // For now, default to BinanceLinear
        let exchange = Exchange::BinanceLinear;
        let ticker = Ticker::new(symbol, exchange);

        // Create TickerInfo with default values (will be created/updated in database)
        let ticker_info = TickerInfo::new(ticker, 0.01, 0.001, None);

        Ok((ticker_info, date))
    }

    /// Stream CSV records and insert in batches
    ///
    /// Avoids loading entire file into memory
    fn stream_csv_insert<R: Read>(
        &self,
        conn: &mut Connection,
        reader: R,
        ticker_id: i64,
    ) -> Result<usize> {
        let tx = conn.transaction().map_err(|e| {
            DatabaseError::Transaction(format!("Failed to start transaction: {}", e))
        })?;

        let mut csv_reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(reader);

        let mut batch = Vec::new();
        let mut count = 0;
        let mut trade_counter: u64 = 0;

        for result in csv_reader.records() {
            let record = result.map_err(|e| {
                DatabaseError::Migration(format!("Failed to parse CSV record: {}", e))
            })?;

            if record.len() < 7 {
                continue; // Skip invalid records
            }

            // Binance aggTrades CSV format:
            // agg_trade_id, price, quantity, first_trade_id, last_trade_id, timestamp, is_buyer_maker
            let _trade_id: i64 = record[0].parse().unwrap_or(0);
            let price: f64 = record[1].parse().unwrap_or(0.0);
            let quantity: f32 = record[2].parse().unwrap_or(0.0);
            let timestamp: i64 = record[5].parse().unwrap_or(0);
            let is_buyer_maker: bool = record[6].parse().unwrap_or(false);
            let is_sell = is_buyer_maker; // Buyer maker means it's a sell

            // Generate deterministic ID for idempotent inserts
            let db_trade_id = generate_trade_id(ticker_id, timestamp as u64, trade_counter);
            trade_counter += 1;

            batch.push((db_trade_id, ticker_id, timestamp, price, quantity, is_sell));

            if batch.len() >= self.config.batch_size {
                self.insert_trade_batch(&tx, &batch)?;
                count += batch.len();
                batch.clear();
            }
        }

        // Insert remaining batch
        if !batch.is_empty() {
            self.insert_trade_batch(&tx, &batch)?;
            count += batch.len();
        }

        tx.commit().map_err(|e| {
            DatabaseError::Transaction(format!("Failed to commit trades transaction: {}", e))
        })?;

        Ok(count)
    }

    fn insert_trade_batch(
        &self,
        conn: &duckdb::Transaction,
        batch: &[(i64, i64, i64, f64, f32, bool)],
    ) -> Result<()> {
        let mut stmt = conn
            .prepare(
                "INSERT OR REPLACE INTO trades
                 (trade_id, ticker_id, trade_time, price, quantity, is_sell)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .map_err(|e| DatabaseError::Query(format!("Failed to prepare statement: {}", e)))?;

        for row in batch {
            stmt.execute(duckdb::params![row.0, row.1, row.2, row.3, row.4, row.5])
                .map_err(|e| DatabaseError::Insert(format!("Failed to insert trade: {}", e)))?;
        }

        Ok(())
    }

    /// Find all ZIP files recursively in directory
    fn find_zip_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut zip_files = Vec::new();

        if !root.is_dir() {
            return Ok(zip_files);
        }

        let entries = fs::read_dir(root).map_err(|e| {
            DatabaseError::Migration(format!("Failed to read directory: {}", e))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                DatabaseError::Migration(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();

            if path.is_dir() {
                // Recursively search subdirectories
                let sub_files = self.find_zip_files(&path)?;
                zip_files.extend(sub_files);
            } else if path.extension().and_then(|s| s.to_str()) == Some("zip") {
                zip_files.push(path);
            }
        }

        Ok(zip_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_migrator_creation() {
        let config = MigrationConfig::default();
        let migrator = ArchiveMigrator::new(config);
        assert_eq!(migrator.config.batch_size, 1000);
    }

    #[test]
    fn test_parse_archive_path() {
        let config = MigrationConfig::default();
        let migrator = ArchiveMigrator::new(config);

        let path = PathBuf::from(
            "market_data/binance/data/futures/um/daily/aggTrades/BTCUSDT/BTCUSDT-aggTrades-2024-01-15.zip",
        );

        let result = migrator.parse_archive_path(&path);
        assert!(result.is_ok());

        let (ticker_info, date) = result.unwrap();
        assert_eq!(format!("{}", ticker_info.ticker), "BTCUSDT");
        assert_eq!(date, "2024-01-15");
    }
}
