//! Backup management for safe migration operations
//!
//! Creates timestamped backups before migration and supports rollback functionality

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Manifest describing backup contents and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    /// Timestamp when backup was created
    pub timestamp: String,
    /// Path to backup directory
    pub backup_path: PathBuf,
    /// List of backed up files with their original paths
    pub files: Vec<BackupFile>,
    /// Schema version at time of backup
    pub schema_version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFile {
    /// Original file path
    pub original_path: PathBuf,
    /// Backup file path
    pub backup_path: PathBuf,
    /// File size in bytes
    pub size_bytes: u64,
}

/// Creates and restores backups with manifest tracking
pub struct BackupManager {
    backup_root: PathBuf,
}

impl BackupManager {
    /// Create a new BackupManager with specified backup root directory
    pub fn new(backup_root: PathBuf) -> Self {
        Self { backup_root }
    }

    /// Create timestamped backup before migration begins
    ///
    /// Copies database file and optionally market_data files
    pub fn create_pre_migration_backup(
        &self,
        db_path: &Path,
        include_market_data: bool,
    ) -> std::io::Result<BackupMetadata> {
        // Create backup root if it doesn't exist
        fs::create_dir_all(&self.backup_root)?;

        // Generate timestamp-based backup directory
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_dir = self.backup_root.join(format!("backup_{}", timestamp));
        fs::create_dir_all(&backup_dir)?;

        log::info!("Creating backup in: {}", backup_dir.display());

        let mut backed_up_files = Vec::new();

        // Backup database file if it exists
        if db_path.exists() {
            let db_backup = backup_dir.join("flowsurface.duckdb");
            fs::copy(db_path, &db_backup)?;

            let size_bytes = fs::metadata(db_path)?.len();
            backed_up_files.push(BackupFile {
                original_path: db_path.to_path_buf(),
                backup_path: db_backup,
                size_bytes,
            });

            log::info!("Backed up database: {} bytes", size_bytes);
        }

        // Backup market_data if requested
        if include_market_data {
            // This would be implemented based on actual market data location
            // For now, we'll skip it as it's optional
            log::debug!("Market data backup not yet implemented");
        }

        // Create manifest
        let metadata = BackupMetadata {
            timestamp: timestamp.clone(),
            backup_path: backup_dir.clone(),
            files: backed_up_files,
            schema_version: 1, // This should come from database
        };

        // Write manifest
        let manifest_path = backup_dir.join("manifest.json");
        let manifest_json = serde_json::to_string_pretty(&metadata)?;
        fs::write(manifest_path, manifest_json)?;

        log::info!("Backup created successfully: {}", timestamp);

        Ok(metadata)
    }

    /// Restore application state from backup directory
    ///
    /// Used for rollback after failed migration
    pub fn restore_from_backup(&self, backup_metadata: &BackupMetadata) -> std::io::Result<()> {
        log::info!(
            "Restoring from backup: {}",
            backup_metadata.backup_path.display()
        );

        for file in &backup_metadata.files {
            if !file.backup_path.exists() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Backup file not found: {}", file.backup_path.display()),
                ));
            }

            // Create parent directory if needed
            if let Some(parent) = file.original_path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Restore file
            fs::copy(&file.backup_path, &file.original_path)?;
            log::info!(
                "Restored: {} -> {}",
                file.backup_path.display(),
                file.original_path.display()
            );
        }

        log::info!("Backup restored successfully");
        Ok(())
    }

    /// Load backup metadata from manifest file
    pub fn load_backup_metadata(&self, backup_dir: &Path) -> std::io::Result<BackupMetadata> {
        let manifest_path = backup_dir.join("manifest.json");
        let manifest_json = fs::read_to_string(manifest_path)?;
        let metadata: BackupMetadata = serde_json::from_str(&manifest_json)?;
        Ok(metadata)
    }

    /// List all available backups with metadata
    ///
    /// Sorted by timestamp descending (newest first)
    pub fn list_backups(&self) -> std::io::Result<Vec<BackupMetadata>> {
        if !self.backup_root.exists() {
            return Ok(Vec::new());
        }

        let mut backups = Vec::new();

        for entry in fs::read_dir(&self.backup_root)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let manifest_path = path.join("manifest.json");
                if manifest_path.exists() {
                    if let Ok(metadata) = self.load_backup_metadata(&path) {
                        backups.push(metadata);
                    }
                }
            }
        }

        // Sort by timestamp descending
        backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(backups)
    }

    /// Remove backups older than retention period
    ///
    /// Default retention: 7 days
    pub fn cleanup_old_backups(&self, retention_days: u64) -> std::io::Result<usize> {
        let now = SystemTime::now();
        let retention_duration = std::time::Duration::from_secs(retention_days * 24 * 3600);

        let mut removed_count = 0;

        for backup in self.list_backups()? {
            if let Ok(metadata) = fs::metadata(&backup.backup_path) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age > retention_duration {
                            log::info!("Removing old backup: {}", backup.timestamp);
                            fs::remove_dir_all(&backup.backup_path)?;
                            removed_count += 1;
                        }
                    }
                }
            }
        }

        if removed_count > 0 {
            log::info!("Cleaned up {} old backup(s)", removed_count);
        }

        Ok(removed_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_backup_creation() {
        let temp_dir = TempDir::new().unwrap();
        let backup_root = temp_dir.path().join("backups");
        let db_path = temp_dir.path().join("test.db");

        // Create a dummy database file
        fs::write(&db_path, b"test database content").unwrap();

        let manager = BackupManager::new(backup_root.clone());
        let metadata = manager
            .create_pre_migration_backup(&db_path, false)
            .unwrap();

        assert_eq!(metadata.files.len(), 1);
        assert!(metadata.backup_path.exists());
        assert!(metadata.backup_path.join("manifest.json").exists());
    }

    #[test]
    fn test_backup_restore() {
        let temp_dir = TempDir::new().unwrap();
        let backup_root = temp_dir.path().join("backups");
        let db_path = temp_dir.path().join("test.db");

        // Create and backup
        fs::write(&db_path, b"original content").unwrap();
        let manager = BackupManager::new(backup_root);
        let metadata = manager
            .create_pre_migration_backup(&db_path, false)
            .unwrap();

        // Modify original
        fs::write(&db_path, b"modified content").unwrap();

        // Restore
        manager.restore_from_backup(&metadata).unwrap();

        // Verify restore
        let content = fs::read_to_string(&db_path).unwrap();
        assert_eq!(content, "original content");
    }

    #[test]
    fn test_list_backups() {
        let temp_dir = TempDir::new().unwrap();
        let backup_root = temp_dir.path().join("backups");
        let db_path = temp_dir.path().join("test.db");

        fs::write(&db_path, b"test").unwrap();

        let manager = BackupManager::new(backup_root);

        // Create multiple backups
        manager
            .create_pre_migration_backup(&db_path, false)
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        manager
            .create_pre_migration_backup(&db_path, false)
            .unwrap();

        let backups = manager.list_backups().unwrap();
        assert_eq!(backups.len(), 2);
    }
}
