//! Migration verification and rollback logic
//!
//! Provides RAII guard that verifies migration and rolls back on failure

use super::backup::{BackupManager, BackupMetadata};
use crate::db::{DatabaseError, Result};
use duckdb::Connection;
use std::path::PathBuf;

/// Status codes for health check results
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthCheckStatus {
    Passed,
    Warning,
    Failed,
}

/// Result of migration verification
#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub status: HealthCheckStatus,
    pub messages: Vec<String>,
}

impl HealthCheck {
    pub fn passed() -> Self {
        Self {
            status: HealthCheckStatus::Passed,
            messages: Vec::new(),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            status: HealthCheckStatus::Warning,
            messages: vec![message.into()],
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            status: HealthCheckStatus::Failed,
            messages: vec![message.into()],
        }
    }

    pub fn add_message(&mut self, message: impl Into<String>) {
        self.messages.push(message.into());
    }

    pub fn is_failed(&self) -> bool {
        self.status == HealthCheckStatus::Failed
    }

    pub fn is_passed(&self) -> bool {
        self.status == HealthCheckStatus::Passed
    }
}

/// RAII guard that verifies migration and rolls back on failure
///
/// Ensures data integrity or restores backup
pub struct MigrationGuard {
    backup_metadata: Option<BackupMetadata>,
    db_path: PathBuf,
    backup_manager: BackupManager,
}

impl MigrationGuard {
    /// Create guard with backup metadata and database path
    pub fn new(
        backup_metadata: Option<BackupMetadata>,
        db_path: PathBuf,
        backup_manager: BackupManager,
    ) -> Self {
        Self {
            backup_metadata,
            db_path,
            backup_manager,
        }
    }

    /// Run comprehensive verification checks on migrated data
    pub fn verify_migration(&self) -> Result<HealthCheck> {
        log::info!("Running migration verification checks...");

        // Open database connection
        let conn = Connection::open(&self.db_path).map_err(|e| {
            DatabaseError::Connection(format!("Failed to open database for verification: {}", e))
        })?;

        let mut health_check = HealthCheck::passed();

        // Check 1: Verify all required tables exist
        if let Err(e) = self.check_table_existence(&conn) {
            health_check.status = HealthCheckStatus::Failed;
            health_check.add_message(format!("Table existence check failed: {}", e));
            return Ok(health_check);
        }

        // Check 2: Verify row counts are reasonable
        match self.check_row_counts(&conn) {
            Ok(warnings) => {
                for warning in warnings {
                    health_check.add_message(warning);
                    if health_check.status == HealthCheckStatus::Passed {
                        health_check.status = HealthCheckStatus::Warning;
                    }
                }
            }
            Err(e) => {
                health_check.status = HealthCheckStatus::Failed;
                health_check.add_message(format!("Row count check failed: {}", e));
                return Ok(health_check);
            }
        }

        // Check 3: Sample data integrity
        if let Err(e) = self.check_data_integrity(&conn) {
            health_check.status = HealthCheckStatus::Failed;
            health_check.add_message(format!("Data integrity check failed: {}", e));
            return Ok(health_check);
        }

        if health_check.messages.is_empty() {
            health_check.add_message("All verification checks passed");
        }

        log::info!("Verification completed with status: {:?}", health_check.status);
        Ok(health_check)
    }

    /// Rollback migration if verification failed
    pub fn rollback_if_failed(&self, health_check: &HealthCheck) -> Result<()> {
        if !health_check.is_failed() {
            return Ok(());
        }

        log::error!("Migration verification failed, initiating rollback...");

        // Print all error messages
        for msg in &health_check.messages {
            log::error!("  - {}", msg);
        }

        if let Some(backup) = &self.backup_metadata {
            // Delete corrupted database
            if self.db_path.exists() {
                std::fs::remove_file(&self.db_path).map_err(|e| {
                    DatabaseError::Migration(format!("Failed to remove corrupted database: {}", e))
                })?;
            }

            // Restore from backup
            self.backup_manager
                .restore_from_backup(backup)
                .map_err(|e| DatabaseError::Migration(format!("Failed to restore backup: {}", e)))?;

            log::info!("Rollback completed successfully");
            Ok(())
        } else {
            Err(DatabaseError::Migration(
                "No backup available for rollback".to_string(),
            ))
        }
    }

    /// Verify all required tables exist in database
    fn check_table_existence(&self, conn: &Connection) -> Result<()> {
        let required_tables = vec![
            "schema_version",
            "exchanges",
            "tickers",
            "trades",
            "klines",
            "depth_snapshots",
            "footprint_data",
            "order_runs",
        ];

        for table in required_tables {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = ?",
                    [table],
                    |row| row.get(0),
                )
                .map_err(|e| {
                    DatabaseError::Query(format!("Failed to check table existence: {}", e))
                })?;

            if count == 0 {
                return Err(DatabaseError::Migration(format!(
                    "Required table '{}' does not exist",
                    table
                )));
            }
        }

        Ok(())
    }

    /// Verify row counts are reasonable
    fn check_row_counts(&self, conn: &Connection) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        // Check if exchanges table has at least one entry
        let exchange_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM exchanges", [], |row| row.get(0))
            .unwrap_or(0);

        if exchange_count == 0 {
            warnings.push("No exchanges found in database".to_string());
        }

        // Check if tickers table has at least one entry
        let ticker_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tickers", [], |row| row.get(0))
            .unwrap_or(0);

        if ticker_count == 0 {
            warnings.push("No tickers found in database".to_string());
        }

        Ok(warnings)
    }

    /// Sample data for invalid values
    fn check_data_integrity(&self, conn: &Connection) -> Result<()> {
        // Check for negative prices in trades (should never happen)
        let negative_prices: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM trades WHERE price < 0",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if negative_prices > 0 {
            return Err(DatabaseError::Migration(format!(
                "Found {} trades with negative prices",
                negative_prices
            )));
        }

        // Check for null required fields in klines
        let null_klines: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM klines WHERE open_price IS NULL OR close_price IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if null_klines > 0 {
            return Err(DatabaseError::Migration(format!(
                "Found {} klines with null price fields",
                null_klines
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_health_check_status() {
        let passed = HealthCheck::passed();
        assert!(passed.is_passed());
        assert!(!passed.is_failed());

        let failed = HealthCheck::failed("error");
        assert!(failed.is_failed());
        assert!(!failed.is_passed());
    }

    #[test]
    fn test_health_check_messages() {
        let mut check = HealthCheck::passed();
        check.add_message("message 1");
        check.add_message("message 2");
        assert_eq!(check.messages.len(), 2);
    }
}
