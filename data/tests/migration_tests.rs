//! Integration tests for data migration logic

use data::db::{
    ArchiveMigrator, BackupManager, DatabaseManager, DepthMigrator, HealthCheckStatus,
    MigrationConfig, MigrationGuard, TimeSeriesMigrator,
};
use exchange::adapter::Exchange;
use exchange::util::Price;
use exchange::{Kline, Ticker, TickerInfo, Timeframe};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;
use zip::write::FileOptions;
use zip::ZipWriter;

// Helper to create test TickerInfo
fn create_test_ticker_info(symbol: &str) -> TickerInfo {
    let ticker = Ticker::new(symbol, Exchange::BinanceLinear);
    TickerInfo::new(ticker, 0.01, 0.001, None)
}

// Helper to create test kline data
fn create_test_kline(timestamp: u64, price: f32) -> Kline {
    Kline {
        time: timestamp,
        open: Price::from_f32(price),
        high: Price::from_f32(price + 10.0),
        low: Price::from_f32(price - 10.0),
        close: Price::from_f32(price + 5.0),
        volume: (100.0, 10000.0),
    }
}

#[test]
fn test_backup_manager_create_and_restore() {
    let temp_dir = TempDir::new().unwrap();
    let backup_root = temp_dir.path().join("backups");
    let db_path = temp_dir.path().join("test.db");

    // Create a test database file
    fs::write(&db_path, b"original database content").unwrap();

    // Create backup
    let manager = BackupManager::new(backup_root.clone());
    let metadata = manager
        .create_pre_migration_backup(&db_path, false)
        .unwrap();

    assert_eq!(metadata.files.len(), 1);
    assert!(metadata.backup_path.exists());

    // Modify original
    fs::write(&db_path, b"modified content").unwrap();

    // Restore from backup
    manager.restore_from_backup(&metadata).unwrap();

    // Verify restoration
    let content = fs::read(&db_path).unwrap();
    assert_eq!(content, b"original database content");
}

#[test]
fn test_backup_manager_list_backups() {
    let temp_dir = TempDir::new().unwrap();
    let backup_root = temp_dir.path().join("backups");
    let db_path = temp_dir.path().join("test.db");

    fs::write(&db_path, b"test content").unwrap();

    let manager = BackupManager::new(backup_root);

    // Create multiple backups
    manager
        .create_pre_migration_backup(&db_path, false)
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    manager
        .create_pre_migration_backup(&db_path, false)
        .unwrap();

    let backups = manager.list_backups().unwrap();
    assert_eq!(backups.len(), 2);

    // Verify sorted by timestamp descending
    assert!(backups[0].timestamp >= backups[1].timestamp);
}

#[test]
fn test_migration_guard_verification() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let backup_root = temp_dir.path().join("backups");

    // Create database
    let db = DatabaseManager::new(&db_path).unwrap();
    drop(db);

    let manager = BackupManager::new(backup_root);
    let backup = manager
        .create_pre_migration_backup(&db_path, false)
        .unwrap();

    let guard = MigrationGuard::new(Some(backup), db_path.clone(), manager);

    // Run verification
    let health_check = guard.verify_migration().unwrap();
    assert_eq!(health_check.status, HealthCheckStatus::Passed);
}

#[test]
fn test_archive_migrator_parse_path() {
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

#[test]
fn test_archive_migrator_with_zip_file() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let archive_dir = temp_dir.path().join("archives");
    fs::create_dir_all(&archive_dir).unwrap();

    // Create a test ZIP file with CSV data
    let zip_path = archive_dir.join("BTCUSDT-aggTrades-2024-01-15.zip");
    let file = fs::File::create(&zip_path).unwrap();
    let mut zip = ZipWriter::new(file);

    zip.start_file("BTCUSDT-aggTrades-2024-01-15.csv", zip::write::SimpleFileOptions::default())
        .unwrap();

    // Write sample CSV data (Binance aggTrades format)
    let csv_data = "1,50000.0,0.001,1,1,1705334400000,true\n\
                    2,50001.0,0.002,2,2,1705334401000,false\n\
                    3,50002.0,0.003,3,3,1705334402000,true\n";
    zip.write_all(csv_data.as_bytes()).unwrap();
    zip.finish().unwrap();

    // Create database and migrate
    let db = DatabaseManager::new(&db_path).unwrap();
    let config = MigrationConfig::default();
    let migrator = ArchiveMigrator::new(config);

    db.with_conn(|conn| {
        let stats = migrator.migrate_single_archive(conn, &zip_path).unwrap();
        assert_eq!(stats.trades_migrated, 3);
        Ok(())
    })
    .unwrap();

    // Verify trades were inserted
    db.with_conn(|conn| {
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trades", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 3);
        Ok(())
    })
    .unwrap();
}

#[test]
fn test_timeseries_migrator() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let db = DatabaseManager::new(&db_path).unwrap();
    let config = MigrationConfig::default();
    let migrator = TimeSeriesMigrator::new(config);

    let ticker_info = create_test_ticker_info("BTCUSDT");
    let timeframe = Timeframe::M1;

    // Create test timeseries data
    use data::db::migration::timeseries::{FootprintData, KlineData};
    let mut timeseries = BTreeMap::new();

    let timestamp = 1705334400000u64;
    let kline = create_test_kline(timestamp, 50000.0);

    let mut footprint = BTreeMap::new();
    footprint.insert(
        Price::from_f32(50000.0),
        FootprintData {
            price: Price::from_f32(50000.0),
            buy_qty: 1.0,
            sell_qty: 0.5,
            buy_count: 10,
            sell_count: 5,
            first_time: timestamp,
            last_time: timestamp + 1000,
        },
    );

    timeseries.insert(
        timestamp,
        KlineData {
            kline,
            footprint: footprint.clone(),
        },
    );

    // Migrate klines
    db.with_conn(|conn| {
        let count = migrator
            .migrate_klines(conn, &timeseries, &ticker_info, timeframe)
            .unwrap();
        assert_eq!(count, 1);
        Ok(())
    })
    .unwrap();

    // Verify klines were inserted
    db.with_conn(|conn| {
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM klines", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        Ok(())
    })
    .unwrap();

    // Migrate footprints
    db.with_conn(|conn| {
        let count = migrator
            .migrate_footprints(conn, &timeseries, &ticker_info, timeframe)
            .unwrap();
        assert_eq!(count, 1);
        Ok(())
    })
    .unwrap();

    // Verify footprints were inserted
    db.with_conn(|conn| {
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM footprint_data", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        Ok(())
    })
    .unwrap();
}

#[test]
fn test_depth_migrator() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let db = DatabaseManager::new(&db_path).unwrap();
    let config = MigrationConfig::default();
    let migrator = DepthMigrator::new(config);

    let ticker_info = create_test_ticker_info("BTCUSDT");

    // Create test order runs
    use data::db::migration::depth::OrderRunData;
    let mut price_levels = BTreeMap::new();

    let runs = vec![
        OrderRunData {
            start_time: 1705334400000,
            until_time: 1705334460000,
            qty: 10.0,
            is_bid: true,
        },
        OrderRunData {
            start_time: 1705334460000,
            until_time: 1705334520000,
            qty: 15.0,
            is_bid: true,
        },
    ];

    price_levels.insert(Price::from_f32(50000.0), runs);

    // Migrate order runs
    db.with_conn(|conn| {
        let stats = migrator
            .migrate_historical_depth(conn, &price_levels, &ticker_info)
            .unwrap();
        assert_eq!(stats.runs_migrated, 2);
        Ok(())
    })
    .unwrap();

    // Verify order runs were inserted
    db.with_conn(|conn| {
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM order_runs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);
        Ok(())
    })
    .unwrap();
}

#[test]
fn test_migration_idempotency() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let db = DatabaseManager::new(&db_path).unwrap();
    let config = MigrationConfig::default();
    let migrator = TimeSeriesMigrator::new(config.clone());

    let ticker_info = create_test_ticker_info("BTCUSDT");
    let timeframe = Timeframe::M1;

    use data::db::migration::timeseries::KlineData;
    let mut timeseries = BTreeMap::new();

    let timestamp = 1705334400000u64;
    let kline = create_test_kline(timestamp, 50000.0);

    timeseries.insert(timestamp, KlineData {
        kline,
        footprint: BTreeMap::new(),
    });

    // Migrate once
    db.with_conn(|conn| {
        migrator
            .migrate_klines(conn, &timeseries, &ticker_info, timeframe)
            .unwrap();
        Ok(())
    })
    .unwrap();

    // Migrate again (should be idempotent)
    db.with_conn(|conn| {
        migrator
            .migrate_klines(conn, &timeseries, &ticker_info, timeframe)
            .unwrap();
        Ok(())
    })
    .unwrap();

    // Verify only one kline exists
    db.with_conn(|conn| {
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM klines", [], |row| row.get(0))
            .unwrap();
        // Due to INSERT OR REPLACE, we should still have 1 record
        assert_eq!(count, 1);
        Ok(())
    })
    .unwrap();
}

#[test]
fn test_migration_config_options() {
    let config = MigrationConfig::new()
        .with_batch_size(500)
        .with_dry_run(true)
        .with_backup(false)
        .with_verification(false);

    assert_eq!(config.batch_size, 500);
    assert!(config.dry_run);
    assert!(!config.create_backup);
    assert!(!config.verify_migration);
}
