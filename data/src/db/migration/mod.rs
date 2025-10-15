//! Data migration subsystem for converting existing FlowSurface data to DuckDB format
//!
//! This module provides comprehensive migration functionality to transition from file-based
//! and in-memory storage to database-backed persistence. It handles three primary data sources:
//!
//! 1. In-memory TimeSeries<KlineDataPoint> structures containing klines and footprint data
//! 2. HistoricalDepth structures with order book run data
//! 3. Binance ZIP archives (up to 4 days retention) containing aggTrades CSV files
//!
//! The migration system prioritizes data safety through:
//! - Mandatory backup creation before migration
//! - Comprehensive verification checks
//! - Automatic rollback on verification failure
//! - Transparent progress reporting

pub mod archive;
pub mod backup;
pub mod depth;
pub mod helpers;
pub mod progress;
pub mod timeseries;
pub mod verification;

pub use archive::ArchiveMigrator;
pub use backup::{BackupManager, BackupMetadata};
pub use depth::DepthMigrator;
pub use helpers::*;
pub use progress::ProgressTracker;
pub use timeseries::TimeSeriesMigrator;
pub use verification::{HealthCheck, HealthCheckStatus, MigrationGuard};

use std::fmt;

/// Statistics tracking for migration operations
#[derive(Debug, Clone, Default)]
pub struct MigrationStats {
    /// Number of files successfully processed
    pub files_processed: usize,
    /// Total number of trades migrated
    pub trades_migrated: usize,
    /// Total number of klines migrated
    pub klines_migrated: usize,
    /// Total number of footprint records migrated
    pub footprints_migrated: usize,
    /// Total number of order runs migrated
    pub runs_migrated: usize,
    /// List of errors encountered during migration
    pub errors: Vec<String>,
}

impl MigrationStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }

    pub fn merge(&mut self, other: &MigrationStats) {
        self.files_processed += other.files_processed;
        self.trades_migrated += other.trades_migrated;
        self.klines_migrated += other.klines_migrated;
        self.footprints_migrated += other.footprints_migrated;
        self.runs_migrated += other.runs_migrated;
        self.errors.extend(other.errors.clone());
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

impl fmt::Display for MigrationStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Migration Statistics:")?;
        writeln!(f, "  Files processed: {}", self.files_processed)?;
        writeln!(f, "  Trades migrated: {}", self.trades_migrated)?;
        writeln!(f, "  Klines migrated: {}", self.klines_migrated)?;
        writeln!(f, "  Footprints migrated: {}", self.footprints_migrated)?;
        writeln!(f, "  Order runs migrated: {}", self.runs_migrated)?;
        if self.has_errors() {
            writeln!(f, "  Errors encountered: {}", self.errors.len())?;
            for (i, error) in self.errors.iter().enumerate() {
                writeln!(f, "    {}: {}", i + 1, error)?;
            }
        }
        Ok(())
    }
}

/// Configuration options for migration operations
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    /// Batch size for bulk inserts (default: 1000)
    pub batch_size: usize,
    /// Dry run mode - don't actually write to database
    pub dry_run: bool,
    /// Create backup before migration
    pub create_backup: bool,
    /// Verify migration after completion
    pub verify_migration: bool,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            dry_run: false,
            create_backup: true,
            verify_migration: true,
        }
    }
}

impl MigrationConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub fn with_backup(mut self, create_backup: bool) -> Self {
        self.create_backup = create_backup;
        self
    }

    pub fn with_verification(mut self, verify: bool) -> Self {
        self.verify_migration = verify;
        self
    }
}
