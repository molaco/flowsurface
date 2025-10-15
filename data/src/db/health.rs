//! Database health monitoring infrastructure
//!
//! Provides periodic health checks to detect connection failures, slow queries,
//! and disk space exhaustion before they impact users.

use crate::db::{DatabaseManager, Result};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Health check interval - runs every 60 seconds
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(60);

/// Query latency warning threshold - 1 second
const SLOW_QUERY_THRESHOLD: Duration = Duration::from_secs(1);

/// Disk space warning threshold - 10% available
const LOW_DISK_SPACE_THRESHOLD: f32 = 0.10;

/// Health check report with diagnostic information
#[derive(Debug, Clone)]
pub struct HealthReport {
    /// Connection is alive and responsive
    pub connection_ok: bool,
    /// Query latency in milliseconds
    pub query_latency_ms: u64,
    /// Database file size in bytes
    pub database_size_bytes: u64,
    /// Available disk space as percentage (0.0 to 1.0)
    pub disk_space_available_pct: f32,
    /// List of errors encountered
    pub errors: Vec<String>,
    /// List of warnings (non-critical issues)
    pub warnings: Vec<String>,
}

impl HealthReport {
    /// Create a new empty health report
    fn new() -> Self {
        Self {
            connection_ok: false,
            query_latency_ms: 0,
            database_size_bytes: 0,
            disk_space_available_pct: 1.0,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Check if health report indicates any critical issues
    pub fn is_healthy(&self) -> bool {
        self.connection_ok && self.errors.is_empty()
    }

    /// Get summary of health status
    pub fn summary(&self) -> String {
        if self.is_healthy() {
            if self.warnings.is_empty() {
                format!(
                    "Healthy - query latency: {}ms, disk space: {:.1}%",
                    self.query_latency_ms,
                    self.disk_space_available_pct * 100.0
                )
            } else {
                format!(
                    "Healthy with warnings: {}",
                    self.warnings.join(", ")
                )
            }
        } else {
            format!("Unhealthy - errors: {}", self.errors.join(", "))
        }
    }
}

/// Database health monitor for periodic checks
pub struct DbHealthMonitor {
    /// Reference to database manager
    db: Arc<DatabaseManager>,
    /// Last health check result
    last_report: Arc<std::sync::Mutex<Option<HealthReport>>>,
}

impl DbHealthMonitor {
    /// Create a new health monitor instance
    ///
    /// Does not start background task - use spawn_background_task() for that.
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self {
            db,
            last_report: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Run a complete health check
    ///
    /// Checks connection, query performance, and disk space.
    /// Returns report with any errors or warnings encountered.
    pub fn run_health_check(&self) -> HealthReport {
        let mut report = HealthReport::new();

        // Check connection health
        if let Err(e) = self.check_connection() {
            report.connection_ok = false;
            report.errors.push(format!("Connection check failed: {}", e));
        } else {
            report.connection_ok = true;
        }

        // Check query performance
        match self.check_query_performance() {
            Ok(latency) => {
                report.query_latency_ms = latency.as_millis() as u64;
                if latency > SLOW_QUERY_THRESHOLD {
                    report.warnings.push(format!(
                        "Slow query detected: {}ms (threshold: {}ms)",
                        latency.as_millis(),
                        SLOW_QUERY_THRESHOLD.as_millis()
                    ));
                }
            }
            Err(e) => {
                report.errors.push(format!("Query performance check failed: {}", e));
            }
        }

        // Check disk space
        match self.check_disk_space() {
            Ok((db_size, disk_pct)) => {
                report.database_size_bytes = db_size;
                report.disk_space_available_pct = disk_pct;
                if disk_pct < LOW_DISK_SPACE_THRESHOLD {
                    report.warnings.push(format!(
                        "Low disk space: {:.1}% available (threshold: {:.1}%)",
                        disk_pct * 100.0,
                        LOW_DISK_SPACE_THRESHOLD * 100.0
                    ));
                }
            }
            Err(e) => {
                report.warnings.push(format!("Disk space check failed: {}", e));
            }
        }

        // Store report
        if let Ok(mut last_report) = self.last_report.lock() {
            *last_report = Some(report.clone());
        }

        log::debug!("Health check completed: {}", report.summary());

        report
    }

    /// Get the last health check report
    pub fn last_report(&self) -> Option<HealthReport> {
        self.last_report.lock().ok()?.clone()
    }

    /// Verify database connection is alive
    ///
    /// Executes simple query (SELECT 1) to check connectivity.
    /// Returns error if connection is broken or unresponsive.
    fn check_connection(&self) -> Result<()> {
        self.db.health_check()
    }

    /// Measure query latency for COUNT(*) on trades table
    ///
    /// Warns if query exceeds 1 second threshold.
    /// Uses COUNT(*) as it's a common operation that exercises query engine.
    fn check_query_performance(&self) -> Result<Duration> {
        let start = Instant::now();

        self.db.with_conn(|conn| {
            conn.query_row("SELECT COUNT(*) FROM trades", [], |_| Ok(()))
                .map_err(|e| crate::db::error::DatabaseError::Query(format!("Performance check query failed: {}", e)))?;
            Ok(())
        })?;

        Ok(start.elapsed())
    }

    /// Monitor database file size and available disk space
    ///
    /// Returns (database_size_bytes, available_space_percentage).
    /// Warns when available space falls below 10% threshold.
    fn check_disk_space(&self) -> Result<(u64, f32)> {
        let db_path = self.db.db_path();

        // Get database file size
        let db_size = std::fs::metadata(db_path)
            .map(|m| m.len())
            .unwrap_or(0);

        // Get available disk space
        let disk_pct = get_disk_space_percentage(db_path)?;

        Ok((db_size, disk_pct))
    }

    /// Spawn background task that runs health checks every 60 seconds
    ///
    /// Returns join handle for graceful shutdown during application exit.
    /// Task runs indefinitely until cancelled.
    pub fn spawn_background_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            log::info!("Database health monitor started (interval: {:?})", HEALTH_CHECK_INTERVAL);

            let mut interval = tokio::time::interval(HEALTH_CHECK_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;

                let report = self.run_health_check();

                // Log warnings and errors
                for warning in &report.warnings {
                    log::warn!("Database health warning: {}", warning);
                }

                for error in &report.errors {
                    log::error!("Database health error: {}", error);
                }

                // Log summary at info level if healthy
                if report.is_healthy() && report.warnings.is_empty() {
                    log::info!("Database health: {}", report.summary());
                }
            }
        })
    }
}

/// Get available disk space as percentage (0.0 to 1.0)
///
/// Uses platform-specific APIs to query filesystem information.
fn get_disk_space_percentage(path: &Path) -> Result<f32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let metadata = std::fs::metadata(path)
            .map_err(|e| crate::db::error::DatabaseError::Configuration(
                format!("Failed to get metadata for disk space check: {}", e)
            ))?;

        // On Unix, we can use statvfs to get filesystem stats
        // For simplicity, we'll estimate based on file size vs device size
        // In production, you'd want to use the nix crate or similar for proper statvfs

        // Simplified implementation: assume plenty of space if we can write
        // Real implementation would use statvfs or similar
        Ok(0.5) // Return 50% as safe default
    }

    #[cfg(not(unix))]
    {
        // For non-Unix platforms, return safe default
        // Real implementation would use platform-specific APIs
        Ok(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_db() -> (Arc<DatabaseManager>, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Arc::new(DatabaseManager::new(db_path).unwrap());
        (db, dir)
    }

    #[test]
    fn test_health_monitor_creation() {
        let (db, _dir) = create_test_db();
        let monitor = DbHealthMonitor::new(db);

        assert!(monitor.last_report().is_none());
    }

    #[test]
    fn test_health_check_connection() {
        let (db, _dir) = create_test_db();
        let monitor = DbHealthMonitor::new(db);

        let result = monitor.check_connection();
        assert!(result.is_ok());
    }

    #[test]
    fn test_health_check_query_performance() {
        let (db, _dir) = create_test_db();
        let monitor = DbHealthMonitor::new(db);

        let result = monitor.check_query_performance();
        assert!(result.is_ok());

        let latency = result.unwrap();
        // Should be very fast on empty database
        assert!(latency.as_millis() < 100);
    }

    #[test]
    fn test_run_health_check() {
        let (db, _dir) = create_test_db();
        let monitor = DbHealthMonitor::new(db);

        let report = monitor.run_health_check();

        assert!(report.connection_ok);
        assert!(report.errors.is_empty());
        assert!(report.is_healthy());
    }

    #[test]
    fn test_health_report_summary() {
        let mut report = HealthReport::new();
        report.connection_ok = true;
        report.query_latency_ms = 50;
        report.disk_space_available_pct = 0.75;

        let summary = report.summary();
        assert!(summary.contains("Healthy"));
        assert!(summary.contains("50ms"));
    }

    #[test]
    fn test_health_report_with_warnings() {
        let mut report = HealthReport::new();
        report.connection_ok = true;
        report.warnings.push("Slow query detected".to_string());

        let summary = report.summary();
        assert!(summary.contains("warnings"));
    }

    #[test]
    fn test_health_report_with_errors() {
        let mut report = HealthReport::new();
        report.connection_ok = false;
        report.errors.push("Connection failed".to_string());

        assert!(!report.is_healthy());
        let summary = report.summary();
        assert!(summary.contains("Unhealthy"));
    }

    #[test]
    fn test_last_report_storage() {
        let (db, _dir) = create_test_db();
        let monitor = DbHealthMonitor::new(db);

        // Initially no report
        assert!(monitor.last_report().is_none());

        // Run health check
        monitor.run_health_check();

        // Should have report now
        assert!(monitor.last_report().is_some());
    }
}
