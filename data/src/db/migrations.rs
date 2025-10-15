use super::error::{DatabaseError, Result};
use super::DatabaseManager;

/// Represents a single database schema migration
#[derive(Debug, Clone)]
pub struct Migration {
    /// Migration version number (must be monotonically increasing)
    pub version: i32,
    /// Human-readable description of what this migration does
    pub description: String,
    /// SQL statements to apply the migration
    pub up_sql: String,
    /// SQL statements to rollback the migration (optional)
    pub down_sql: Option<String>,
}

impl Migration {
    /// Create a new migration
    pub fn new(version: i32, description: impl Into<String>, up_sql: impl Into<String>) -> Self {
        Self {
            version,
            description: description.into(),
            up_sql: up_sql.into(),
            down_sql: None,
        }
    }

    /// Create a new migration with rollback support
    pub fn with_rollback(
        version: i32,
        description: impl Into<String>,
        up_sql: impl Into<String>,
        down_sql: impl Into<String>,
    ) -> Self {
        Self {
            version,
            description: description.into(),
            up_sql: up_sql.into(),
            down_sql: Some(down_sql.into()),
        }
    }
}

/// Manages database schema migrations
pub struct MigrationManager {
    migrations: Vec<Migration>,
}

impl MigrationManager {
    /// Create a new migration manager with the given migrations
    pub fn new(mut migrations: Vec<Migration>) -> Self {
        // Sort migrations by version to ensure correct order
        migrations.sort_by_key(|m| m.version);
        Self { migrations }
    }

    /// Get the current schema version from the database
    fn get_current_version(db: &DatabaseManager) -> Result<i32> {
        db.with_conn(|conn| {
            let version: i32 = conn
                .query_row(
                    "SELECT MAX(version) FROM schema_version",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            Ok(version)
        })
    }

    /// Apply all pending migrations (versions > current schema version)
    pub fn apply_pending(&self, db: &DatabaseManager) -> Result<usize> {
        let current_version = Self::get_current_version(db)?;
        let mut applied_count = 0;

        for migration in &self.migrations {
            if migration.version <= current_version {
                continue;
            }

            log::info!(
                "Applying migration {} (version {}): {}",
                applied_count + 1,
                migration.version,
                migration.description
            );

            // Apply migration in a transaction
            db.with_conn(|conn| {
                let tx = conn.transaction()
                    .map_err(|e| DatabaseError::Schema(format!("Failed to start transaction: {}", e)))?;

                // Execute the migration SQL
                tx.execute_batch(&migration.up_sql)
                    .map_err(|e| DatabaseError::Schema(format!(
                        "Failed to apply migration {}: {}",
                        migration.version, e
                    )))?;

                // Record the migration
                tx.execute(
                    "INSERT INTO schema_version (version, description) VALUES (?, ?)",
                    [&migration.version as &dyn duckdb::ToSql, &migration.description as &dyn duckdb::ToSql],
                )
                .map_err(|e| DatabaseError::Schema(format!(
                    "Failed to record migration {}: {}",
                    migration.version, e
                )))?;

                tx.commit()
                    .map_err(|e| DatabaseError::Schema(format!("Failed to commit migration: {}", e)))?;

                Ok(())
            })?;

            applied_count += 1;
            log::info!(
                "Successfully applied migration {} (version {})",
                applied_count,
                migration.version
            );
        }

        if applied_count > 0 {
            log::info!("Applied {} migration(s)", applied_count);
        } else {
            log::debug!("No pending migrations");
        }

        Ok(applied_count)
    }

    /// Rollback the most recently applied migration
    pub fn rollback_last(&self, db: &DatabaseManager) -> Result<()> {
        let current_version = Self::get_current_version(db)?;

        if current_version == 0 {
            return Err(DatabaseError::Schema(
                "No migrations to rollback".to_string(),
            ));
        }

        // Find the migration to rollback
        let migration = self
            .migrations
            .iter()
            .find(|m| m.version == current_version)
            .ok_or_else(|| {
                DatabaseError::Schema(format!(
                    "Migration version {} not found in migration list",
                    current_version
                ))
            })?;

        let down_sql = migration.down_sql.as_ref().ok_or_else(|| {
            DatabaseError::Schema(format!(
                "Migration version {} has no rollback SQL",
                current_version
            ))
        })?;

        log::info!(
            "Rolling back migration version {}: {}",
            migration.version,
            migration.description
        );

        // Rollback in a transaction
        db.with_conn(|conn| {
            let tx = conn.transaction()
                .map_err(|e| DatabaseError::Schema(format!("Failed to start transaction: {}", e)))?;

            // Execute the rollback SQL
            tx.execute_batch(down_sql)
                .map_err(|e| DatabaseError::Schema(format!(
                    "Failed to rollback migration {}: {}",
                    migration.version, e
                )))?;

            // Remove the migration record
            tx.execute(
                "DELETE FROM schema_version WHERE version = ?",
                [&migration.version],
            )
            .map_err(|e| DatabaseError::Schema(format!(
                "Failed to remove migration record {}: {}",
                migration.version, e
            )))?;

            tx.commit()
                .map_err(|e| DatabaseError::Schema(format!("Failed to commit rollback: {}", e)))?;

            Ok(())
        })?;

        log::info!(
            "Successfully rolled back migration version {}",
            migration.version
        );

        Ok(())
    }

    /// Get list of all available migrations
    pub fn list_migrations(&self) -> &[Migration] {
        &self.migrations
    }

    /// Check if there are pending migrations
    pub fn has_pending_migrations(&self, db: &DatabaseManager) -> Result<bool> {
        let current_version = Self::get_current_version(db)?;
        Ok(self.migrations.iter().any(|m| m.version > current_version))
    }
}

/// Get the list of all available migrations
///
/// Initial implementation returns empty Vec since Task 1 only creates initial schema.
/// Future tasks will add migrations here as schema evolves.
pub fn get_migrations() -> Vec<Migration> {
    vec![
        // Future migrations will be added here
        // Example:
        // Migration::new(
        //     2,
        //     "Add index on trades timestamp",
        //     "CREATE INDEX idx_trades_time ON trades(timestamp);"
        // ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_db() -> (DatabaseManager, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = DatabaseManager::new(db_path).unwrap();
        (db, dir)
    }

    #[test]
    fn test_migration_creation() {
        let migration = Migration::new(2, "Test migration", "CREATE TABLE test (id INTEGER);");
        assert_eq!(migration.version, 2);
        assert_eq!(migration.description, "Test migration");
        assert!(migration.down_sql.is_none());

        let migration_with_rollback = Migration::with_rollback(
            3,
            "Test migration with rollback",
            "CREATE TABLE test2 (id INTEGER);",
            "DROP TABLE test2;",
        );
        assert_eq!(migration_with_rollback.version, 3);
        assert!(migration_with_rollback.down_sql.is_some());
    }

    #[test]
    fn test_no_pending_migrations_initially() {
        let (db, _dir) = create_test_db();
        let manager = MigrationManager::new(get_migrations());

        assert!(!manager.has_pending_migrations(&db).unwrap());
    }

    #[test]
    fn test_apply_pending_migrations() {
        let (db, _dir) = create_test_db();

        let migrations = vec![
            Migration::new(
                2,
                "Add test table",
                "CREATE TABLE test_migration (id INTEGER PRIMARY KEY, value VARCHAR);",
            ),
        ];

        let manager = MigrationManager::new(migrations);
        let applied = manager.apply_pending(&db).unwrap();
        assert_eq!(applied, 1);

        // Verify the table was created
        db.with_conn(|conn| {
            let count: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'test_migration'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1);
            Ok(())
        })
        .unwrap();

        // Applying again should do nothing
        let applied_again = manager.apply_pending(&db).unwrap();
        assert_eq!(applied_again, 0);
    }

    #[test]
    fn test_migration_rollback() {
        let (db, _dir) = create_test_db();

        let migrations = vec![
            Migration::with_rollback(
                2,
                "Add test table with rollback",
                "CREATE TABLE test_rollback (id INTEGER PRIMARY KEY);",
                "DROP TABLE test_rollback;",
            ),
        ];

        let manager = MigrationManager::new(migrations);

        // Apply the migration
        manager.apply_pending(&db).unwrap();

        // Verify table exists
        db.with_conn(|conn| {
            let count: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'test_rollback'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1);
            Ok(())
        })
        .unwrap();

        // Rollback the migration
        manager.rollback_last(&db).unwrap();

        // Verify table no longer exists
        db.with_conn(|conn| {
            let count: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'test_rollback'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_migrations_sorted_by_version() {
        let migrations = vec![
            Migration::new(3, "Third", "SELECT 3;"),
            Migration::new(1, "First", "SELECT 1;"),
            Migration::new(2, "Second", "SELECT 2;"),
        ];

        let manager = MigrationManager::new(migrations);
        let sorted = manager.list_migrations();

        assert_eq!(sorted[0].version, 1);
        assert_eq!(sorted[1].version, 2);
        assert_eq!(sorted[2].version, 3);
    }
}
