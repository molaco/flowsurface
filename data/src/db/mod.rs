//! Database infrastructure module for FlowSurface
//!
//! This module provides persistent storage for trading data using DuckDB as an embedded
//! analytics database. It includes:
//! - Thread-safe database connection management via Arc<Mutex<Connection>>
//! - Schema initialization and versioning
//! - Migration system for schema evolution
//! - Rich error types for debugging

use duckdb::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

pub mod crud;
pub mod error;
pub mod health;
pub mod helpers;
pub mod metrics;
pub mod migration;
pub mod migrations;
pub mod query_cache;

pub use crud::{TradesCRUD, KlinesCRUD, DepthCRUD, FootprintCRUD};
pub use error::{DatabaseError, Result};
pub use health::{DbHealthMonitor, HealthReport};
pub use metrics::{PerformanceMetrics, MetricsSnapshot, MetricTimer};
pub use migration::{
    ArchiveMigrator, BackupManager, BackupMetadata, DepthMigrator, HealthCheck, HealthCheckStatus,
    MigrationConfig, MigrationGuard, MigrationStats, ProgressTracker, TimeSeriesMigrator,
};
pub use migrations::{Migration, MigrationManager};
pub use query_cache::{QueryCache, CacheStats};

/// Default memory limit for DuckDB in gigabytes
/// Set to 8GB as recommended for trading workloads with multiple tickers
const DEFAULT_MEMORY_LIMIT_GB: usize = 8;

/// Current schema version - incremented with each schema change
const SCHEMA_VERSION: i32 = 1;

/// Embedded schema SQL - loaded at compile time
const SCHEMA_SQL: &str = include_str!("schema.sql");

/// Configuration options for DatabaseManager initialization
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Memory limit in gigabytes (None for DuckDB default)
    pub memory_limit_gb: Option<usize>,
    /// Temporary directory for spill-to-disk operations
    pub temp_directory: Option<PathBuf>,
    /// Number of threads for query execution (None for auto)
    pub threads: Option<usize>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            memory_limit_gb: Some(DEFAULT_MEMORY_LIMIT_GB),
            temp_directory: None,
            threads: None,
        }
    }
}

impl DatabaseConfig {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set memory limit in gigabytes
    pub fn with_memory_limit(mut self, gb: usize) -> Self {
        self.memory_limit_gb = Some(gb);
        self
    }

    /// Set temporary directory for spill-to-disk operations
    pub fn with_temp_directory(mut self, path: PathBuf) -> Self {
        self.temp_directory = Some(path);
        self
    }

    /// Set number of threads for query execution
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = Some(threads);
        self
    }
}

/// Database statistics for monitoring and health checks
#[derive(Debug, Clone)]
pub struct DbStats {
    /// Total number of trades stored
    pub total_trades: i64,
    /// Total number of klines stored
    pub total_klines: i64,
    /// Total number of tickers
    pub total_tickers: i64,
    /// Database file size in bytes
    pub database_size_bytes: i64,
    /// Current schema version
    pub schema_version: i32,
}

/// Central abstraction for all database operations
///
/// Provides thread-safe access to DuckDB connection following the single-writer
/// MVCC model via Arc<Mutex<Connection>>. This design is optimal for FlowSurface's
/// architecture where trading data flows through a single pipeline but is read by
/// multiple UI components.
pub struct DatabaseManager {
    /// Thread-safe connection wrapper
    conn: Arc<Mutex<Connection>>,
    /// Path to database file for operations like backup
    db_path: PathBuf,
    /// Query cache for avoiding repeated database hits
    query_cache: QueryCache,
    /// Performance metrics tracking
    metrics: PerformanceMetrics,
}

impl DatabaseManager {
    /// Create a new DatabaseManager with default configuration
    ///
    /// Opens or creates DuckDB database at specified path, initializes schema,
    /// and configures default memory limits (8GB).
    ///
    /// # Arguments
    /// * `db_path` - Path to database file (will be created if doesn't exist)
    ///
    /// # Errors
    /// Returns error if:
    /// - Parent directory doesn't exist or isn't writable
    /// - Database file cannot be opened
    /// - Schema initialization fails
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        Self::with_config(db_path, DatabaseConfig::default())
    }

    /// Create a new DatabaseManager with custom configuration
    ///
    /// # Arguments
    /// * `db_path` - Path to database file
    /// * `config` - Custom configuration options
    pub fn with_config<P: AsRef<Path>>(db_path: P, config: DatabaseConfig) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                DatabaseError::Connection(format!(
                    "Failed to create database directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        // Open or create database
        let conn = Connection::open(&db_path).map_err(|e| {
            DatabaseError::Connection(format!(
                "Failed to open database at {}: {}",
                db_path.display(),
                e
            ))
        })?;

        let manager = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path,
            query_cache: QueryCache::new(),
            metrics: PerformanceMetrics::new(),
        };

        // Configure connection
        manager.configure_connection(&config)?;

        // Initialize schema
        manager.initialize_schema()?;

        log::info!(
            "Database initialized at {} (schema version {})",
            manager.db_path.display(),
            SCHEMA_VERSION
        );

        Ok(manager)
    }

    /// Provides safe access to the underlying DuckDB connection
    ///
    /// Acquires mutex lock, executes closure with connection reference,
    /// and automatically releases lock when done.
    ///
    /// # Arguments
    /// * `f` - Closure that receives mutable connection reference
    ///
    /// # Returns
    /// Result from closure execution
    ///
    /// # Errors
    /// Returns LockError if mutex is poisoned
    pub fn with_conn<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut MutexGuard<Connection>) -> Result<T>,
    {
        let mut guard = self
            .conn
            .lock()
            .map_err(|_| DatabaseError::LockError)?;
        f(&mut guard)
    }

    /// Configure DuckDB connection settings
    fn configure_connection(&self, config: &DatabaseConfig) -> Result<()> {
        self.with_conn(|conn| {
            // Set memory limit
            if let Some(memory_gb) = config.memory_limit_gb {
                let memory_limit = format!("{}GB", memory_gb);
                conn.execute(&format!("SET memory_limit='{}'", memory_limit), [])
                    .map_err(|e| {
                        DatabaseError::Configuration(format!("Failed to set memory limit: {}", e))
                    })?;
                log::debug!("Set DuckDB memory limit to {}", memory_limit);
            }

            // Set temp directory
            if let Some(temp_dir) = &config.temp_directory {
                conn.execute(
                    &format!("SET temp_directory='{}'", temp_dir.display()),
                    [],
                )
                .map_err(|e| {
                    DatabaseError::Configuration(format!("Failed to set temp directory: {}", e))
                })?;
                log::debug!("Set DuckDB temp directory to {}", temp_dir.display());
            }

            // Set thread count
            if let Some(threads) = config.threads {
                conn.execute(&format!("SET threads={}", threads), [])
                    .map_err(|e| {
                        DatabaseError::Configuration(format!("Failed to set thread count: {}", e))
                    })?;
                log::debug!("Set DuckDB thread count to {}", threads);
            }

            Ok(())
        })
    }

    /// Initialize database schema on first run
    ///
    /// Executes embedded schema.sql DDL statements atomically within a transaction.
    /// Checks for existing schema to prevent re-initialization.
    fn initialize_schema(&self) -> Result<()> {
        self.with_conn(|conn| {
            // Check if schema is already initialized
            let has_schema: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM information_schema.tables WHERE table_name = 'schema_version'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(false);

            if has_schema {
                log::debug!("Schema already initialized, skipping initialization");
                return Ok(());
            }

            log::info!("Initializing database schema...");

            // Execute schema in a transaction
            let tx = conn.transaction().map_err(|e| {
                DatabaseError::Schema(format!("Failed to start schema transaction: {}", e))
            })?;

            tx.execute_batch(SCHEMA_SQL).map_err(|e| {
                DatabaseError::Schema(format!("Failed to initialize schema: {}", e))
            })?;

            tx.commit().map_err(|e| {
                DatabaseError::Schema(format!("Failed to commit schema: {}", e))
            })?;

            log::info!("Schema initialized successfully");
            Ok(())
        })
    }

    /// Get current schema version from database
    pub fn get_schema_version(&self) -> Result<i32> {
        self.with_conn(|conn| {
            let version: i32 = conn
                .query_row("SELECT MAX(version) FROM schema_version", [], |row| {
                    row.get(0)
                })
                .unwrap_or(0);
            Ok(version)
        })
    }

    /// Reclaim disk space and optimize database
    ///
    /// Runs VACUUM to reclaim unused space and ANALYZE to update statistics
    /// for query optimizer. Should be called periodically after bulk deletions.
    pub fn vacuum(&self) -> Result<()> {
        self.with_conn(|conn| {
            log::info!("Running VACUUM...");
            conn.execute_batch("VACUUM; ANALYZE;").map_err(|e| {
                DatabaseError::Query(format!("Failed to vacuum database: {}", e))
            })?;
            log::info!("VACUUM completed");
            Ok(())
        })
    }

    /// Get database statistics
    pub fn get_stats(&self) -> Result<DbStats> {
        self.with_conn(|conn| {
            let total_trades: i64 = conn
                .query_row("SELECT COUNT(*) FROM trades", [], |row| row.get(0))
                .unwrap_or(0);

            let total_klines: i64 = conn
                .query_row("SELECT COUNT(*) FROM klines", [], |row| row.get(0))
                .unwrap_or(0);

            let total_tickers: i64 = conn
                .query_row("SELECT COUNT(*) FROM tickers", [], |row| row.get(0))
                .unwrap_or(0);

            let database_size_bytes: i64 = std::fs::metadata(&self.db_path)
                .map(|m| m.len() as i64)
                .unwrap_or(0);

            let schema_version: i32 = conn
                .query_row("SELECT MAX(version) FROM schema_version", [], |row| {
                    row.get(0)
                })
                .unwrap_or(0);

            Ok(DbStats {
                total_trades,
                total_klines,
                total_tickers,
                database_size_bytes,
                schema_version,
            })
        })
    }

    /// Get path to database file
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Execute a health check to verify database is accessible
    pub fn health_check(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.query_row("SELECT 1", [], |_| Ok(()))
                .map_err(|e| DatabaseError::Query(format!("Health check failed: {}", e)))?;
            Ok(())
        })
    }

    /// Get reference to query cache
    pub fn cache(&self) -> &QueryCache {
        &self.query_cache
    }

    /// Invalidate cache for a specific ticker
    pub fn invalidate_cache(&self, ticker_id: i32) {
        self.query_cache.invalidate_ticker(ticker_id);
    }

    /// Clear all cached query results
    pub fn clear_cache(&self) {
        self.query_cache.clear();
    }

    /// Get reference to performance metrics
    pub fn metrics(&self) -> &PerformanceMetrics {
        &self.metrics
    }

    /// Get performance metrics snapshot
    pub fn get_metrics_snapshot(&self) -> MetricsSnapshot {
        self.metrics.get_statistics()
    }
}

// Implement Clone for Arc-based sharing across threads
impl Clone for DatabaseManager {
    fn clone(&self) -> Self {
        Self {
            conn: Arc::clone(&self.conn),
            db_path: self.db_path.clone(),
            query_cache: self.query_cache.clone(),
            metrics: self.metrics.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use tempfile::tempdir;

    #[test]
    fn test_database_initialization() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let db = DatabaseManager::new(&db_path).unwrap();

        // Verify database file was created
        assert!(db_path.exists());

        // Verify we can execute a simple query
        db.with_conn(|conn| {
            let result: i32 = conn.query_row("SELECT 1", [], |row| row.get(0)).unwrap();
            assert_eq!(result, 1);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_schema_tables_exist() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = DatabaseManager::new(&db_path).unwrap();

        // Verify all required tables exist
        let required_tables = vec![
            "schema_version",
            "exchanges",
            "tickers",
            "trades",
            "klines",
            "depth_snapshots",
            "open_interest",
            "footprint_data",
            "order_runs",
            "volume_profiles",
        ];

        db.with_conn(|conn| {
            for table in required_tables {
                let count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = ?",
                        [table],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(count, 1, "Table {} should exist", table);
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_schema_version() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = DatabaseManager::new(&db_path).unwrap();

        let version = db.get_schema_version().unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn test_connection_reuse() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = DatabaseManager::new(&db_path).unwrap();

        // Execute multiple queries - connection should be reused
        for i in 1..=5 {
            db.with_conn(|conn| {
                let result: i32 = conn
                    .query_row(&format!("SELECT {}", i), [], |row| row.get(0))
                    .unwrap();
                assert_eq!(result, i);
                Ok(())
            })
            .unwrap();
        }
    }

    #[test]
    fn test_concurrent_reads() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = DatabaseManager::new(&db_path).unwrap();

        // Insert test data
        db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO exchanges (exchange_id, name) VALUES (1, 'TestExchange')",
                [],
            )
            .unwrap();
            Ok(())
        })
        .unwrap();

        // Spawn multiple reader threads
        let mut handles = vec![];
        for i in 0..5 {
            let db_clone = db.clone();
            let handle = thread::spawn(move || {
                db_clone
                    .with_conn(|conn| {
                        let name: String = conn
                            .query_row(
                                "SELECT name FROM exchanges WHERE exchange_id = 1",
                                [],
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert_eq!(name, "TestExchange");
                        Ok(())
                    })
                    .unwrap();
                log::debug!("Thread {} completed read", i);
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_get_stats() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = DatabaseManager::new(&db_path).unwrap();

        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_trades, 0);
        assert_eq!(stats.total_klines, 0);
        assert_eq!(stats.total_tickers, 0);
        assert_eq!(stats.schema_version, SCHEMA_VERSION);
        assert!(stats.database_size_bytes > 0);
    }

    #[test]
    fn test_vacuum() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = DatabaseManager::new(&db_path).unwrap();

        // Vacuum should complete without error
        db.vacuum().unwrap();
    }

    #[test]
    fn test_health_check() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = DatabaseManager::new(&db_path).unwrap();

        // Health check should pass
        db.health_check().unwrap();
    }

    #[test]
    fn test_custom_config() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let config = DatabaseConfig::new()
            .with_memory_limit(4)
            .with_threads(2);

        let db = DatabaseManager::with_config(&db_path, config).unwrap();

        // Verify database was created successfully
        db.health_check().unwrap();
    }

    #[test]
    fn test_schema_idempotency() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Create database first time
        let db1 = DatabaseManager::new(&db_path).unwrap();
        let version1 = db1.get_schema_version().unwrap();

        // Drop and recreate
        drop(db1);

        // Open again - should reuse existing schema
        let db2 = DatabaseManager::new(&db_path).unwrap();
        let version2 = db2.get_schema_version().unwrap();

        assert_eq!(version1, version2);
    }
}
