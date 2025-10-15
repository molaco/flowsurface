//! Common test utilities, fixtures, and helpers for database testing

pub mod fixtures;
pub mod assertions;

use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use crate::db::{DatabaseManager, DatabaseConfig};

/// Test environment providing initialized database and temporary directories
pub struct TestEnvironment {
    pub temp_dir: TempDir,
    pub db_manager: Arc<DatabaseManager>,
    pub db_path: PathBuf,
}

impl TestEnvironment {
    /// Creates a new test environment with a temporary database
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.duckdb");

        let config = DatabaseConfig::new()
            .with_memory_limit(1) // 1GB for tests
            .with_temp_directory(temp_dir.path().to_path_buf());

        let db_manager = DatabaseManager::with_config(&db_path, config)?;

        Ok(Self {
            temp_dir,
            db_manager: Arc::new(db_manager),
            db_path,
        })
    }

    /// Returns the path to the temporary database
    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }

    /// Returns a reference to the database manager
    pub fn db(&self) -> &DatabaseManager {
        &self.db_manager
    }
}

/// Creates a test environment for use in tests
pub fn setup_test_environment() -> Result<TestEnvironment, Box<dyn std::error::Error>> {
    TestEnvironment::new()
}

// Async wait function removed - not needed for synchronous tests
