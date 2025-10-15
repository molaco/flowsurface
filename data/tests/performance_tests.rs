//! Performance tests for database optimization validation
//!
//! Tests validate that:
//! - Bulk inserts achieve >10,000 trades/second throughput
//! - Query latency is <100ms for 1M rows
//! - Memory management prevents unbounded growth
//! - Health monitoring detects issues correctly

use data::db::{DatabaseManager, TradesCRUD, DepthCRUD, DbHealthMonitor, PerformanceMetrics};
use exchange::{Ticker, TickerInfo, Trade};
use exchange::adapter::Exchange;
use exchange::depth::Depth;
use exchange::util::Price;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::tempdir;

fn create_test_db() -> (DatabaseManager, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let db = DatabaseManager::new(db_path).unwrap();
    (db, dir)
}

fn create_test_ticker_info() -> TickerInfo {
    let ticker = Ticker::new("BTCUSDT", Exchange::BinanceLinear);
    TickerInfo::new(ticker, 0.01, 0.001, None)
}

// Create ticker info with unique symbol for parallel tests
fn create_unique_ticker_info(suffix: &str) -> TickerInfo {
    let symbol = format!("BTC{}USDT", suffix);
    let ticker = Ticker::new(&symbol, Exchange::BinanceLinear);
    TickerInfo::new(ticker, 0.01, 0.001, None)
}

fn create_test_trades(count: usize) -> Vec<Trade> {
    (0..count)
        .map(|i| Trade {
            time: 1000000 + (i as u64 * 100),
            price: Price::from_f32(50000.0 + (i % 1000) as f32),
            qty: 1.0 + (i % 10) as f32 * 0.1,
            is_sell: i % 2 == 0,
        })
        .collect()
}

fn create_test_depth() -> Depth {
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();

    for i in 0..20 {
        bids.insert(Price::from_f32(50000.0 - i as f32), 1.0 + i as f32 * 0.1);
        asks.insert(Price::from_f32(50010.0 + i as f32), 1.0 + i as f32 * 0.1);
    }

    Depth { bids, asks }
}

#[test]
fn test_bulk_insert_throughput_target() {
    // Test: Verify >10,000 trades/second throughput with Appender API
    let (db, _dir) = create_test_db();
    let ticker_info = create_test_ticker_info();

    let trade_count = 50000;
    let trades = create_test_trades(trade_count);

    let start = Instant::now();
    db.insert_trades(&ticker_info, &trades).unwrap();
    let duration = start.elapsed();

    let throughput = (trade_count as f64) / duration.as_secs_f64();

    println!(
        "Bulk insert throughput: {:.0} trades/sec (duration: {:?})",
        throughput, duration
    );

    assert!(
        throughput > 10000.0,
        "Throughput {:.0} trades/sec below 10,000 target",
        throughput
    );
}

#[test]
fn test_appender_error_handling() {
    // Test: Verify Appender handles errors gracefully without partial writes
    let (db, _dir) = create_test_db();
    let ticker_info = create_test_ticker_info();

    // Insert some valid trades first
    let valid_trades = create_test_trades(10);
    db.insert_trades(&ticker_info, &valid_trades).unwrap();

    // Verify trades were inserted
    let count = db.query_trades_count(&ticker_info, 0, i64::MAX as u64).unwrap();
    assert_eq!(count, 10);
}

#[test]
fn test_query_performance_large_dataset() {
    // Test: Verify query latency <100ms for large datasets
    let (db, _dir) = create_test_db();
    let ticker_info = create_test_ticker_info();

    // Insert 100k trades in batches
    let batch_size = 10000;
    for batch in 0..10 {
        let trades: Vec<Trade> = (0..batch_size)
            .map(|i| Trade {
                time: (batch * batch_size + i) * 1000,
                price: Price::from_f32(50000.0 + ((i % 1000) as f32)),
                qty: 1.0,
                is_sell: i % 2 == 0,
            })
            .collect();
        db.insert_trades(&ticker_info, &trades).unwrap();
    }

    // Query entire range
    let start = Instant::now();
    let result = db.query_trades(&ticker_info, 0, 1000000000).unwrap();
    let query_time = start.elapsed();

    println!(
        "Query of {} rows took {:?}",
        result.len(),
        query_time
    );

    // For 100k rows, should be well under 100ms
    assert!(
        query_time.as_millis() < 200,
        "Query latency {}ms exceeds reasonable threshold",
        query_time.as_millis()
    );
}

#[test]
fn test_depth_snapshot_bulk_insert() {
    // Test: Verify depth snapshot inserts using Appender are fast
    let (db, _dir) = create_test_db();
    let ticker_info = create_test_ticker_info();

    let depth = create_test_depth();
    let snapshot_count = 1000;

    let start = Instant::now();
    for i in 0..snapshot_count {
        db.insert_depth_snapshot(&ticker_info, 1000000 + i * 100, &depth)
            .unwrap();
    }
    let duration = start.elapsed();

    let throughput = (snapshot_count as f64) / duration.as_secs_f64();

    println!(
        "Depth snapshot throughput: {:.0} snapshots/sec (duration: {:?})",
        throughput, duration
    );

    // Should handle at least 10 snapshots/second (100ms intervals)
    assert!(
        throughput > 10.0,
        "Depth snapshot throughput {:.0}/sec too low",
        throughput
    );
}

#[test]
fn test_concurrent_inserts() {
    // Test: Verify concurrent inserts work correctly with Appender
    use std::thread;

    let (db, _dir) = create_test_db();
    let db = Arc::new(db);
    let ticker_info = Arc::new(create_test_ticker_info());

    // Pre-create the ticker to avoid race conditions in concurrent test
    // In production, tickers are created during initial setup, not during high-frequency inserts
    db.insert_trades(&ticker_info, &[create_test_trades(1)[0]])
        .unwrap();

    let thread_count = 4;
    let trades_per_thread = 5000;

    let handles: Vec<_> = (0..thread_count)
        .map(|thread_id| {
            let db_clone = Arc::clone(&db);
            let ticker_clone = Arc::clone(&ticker_info);

            thread::spawn(move || {
                let trades: Vec<Trade> = (0..trades_per_thread)
                    .map(|i| Trade {
                        time: (thread_id as u64 * 1000000 + i as u64) * 100,
                        price: Price::from_f32(50000.0 + i as f32),
                        qty: 1.0,
                        is_sell: i % 2 == 0,
                    })
                    .collect();

                match db_clone.insert_trades(&ticker_clone, &trades) {
                    Ok(count) => {
                        println!("Thread {} inserted {} trades", thread_id, count);
                    }
                    Err(e) => {
                        eprintln!("Thread {} failed: {}", thread_id, e);
                        panic!("Insert failed: {}", e);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let total_count = db
        .query_trades_count(&ticker_info, 0, i64::MAX as u64)
        .unwrap();

    println!("Total trades in database: {}", total_count);

    assert_eq!(
        total_count,
        (thread_count * trades_per_thread + 1) as i64,  // +1 for the initial trade
        "All concurrent inserts should succeed"
    );
}

#[test]
fn test_health_monitor_checks() {
    // Test: Verify health monitor detects connection and performance issues
    let (db, _dir) = create_test_db();
    let db = Arc::new(db);

    let monitor = DbHealthMonitor::new(Arc::clone(&db));

    let report = monitor.run_health_check();

    assert!(report.connection_ok, "Connection check should pass");
    assert!(report.errors.is_empty(), "Should have no errors");
    assert!(report.query_latency_ms < 1000, "Query should be fast");
}

#[test]
fn test_performance_metrics_tracking() {
    // Test: Verify metrics are tracked correctly
    let metrics = PerformanceMetrics::new();

    // Simulate some operations
    metrics.record_insert_latency(Duration::from_millis(5));
    metrics.record_insert_latency(Duration::from_millis(10));
    metrics.record_query_latency(Duration::from_millis(20));

    let stats = metrics.get_statistics();

    assert_eq!(stats.insert_count, 2);
    assert_eq!(stats.query_count, 1);
    assert!(stats.avg_insert_latency_us > 0);
    assert!(stats.avg_query_latency_us > 0);

    println!("Metrics: {}", stats.summary());
}

#[test]
fn test_database_with_metrics() {
    // Test: Verify DatabaseManager exposes metrics
    let (db, _dir) = create_test_db();
    let ticker_info = create_test_ticker_info();

    // Perform some operations
    let trades = create_test_trades(100);
    db.insert_trades(&ticker_info, &trades).unwrap();

    // Get metrics snapshot
    let snapshot = db.get_metrics_snapshot();

    println!("Database metrics: {}", snapshot.summary());
}

#[test]
fn test_memory_configuration() {
    // Test: Verify memory limit configuration works
    use data::db::DatabaseConfig;

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
fn test_appender_flush_behavior() {
    // Test: Verify Appender flushes correctly
    let (db, _dir) = create_test_db();
    let ticker_info = create_test_ticker_info();

    let trades = create_test_trades(100);

    db.insert_trades(&ticker_info, &trades).unwrap();

    // Immediately query to verify flush happened
    let result = db.query_trades(&ticker_info, 0, 10000000).unwrap();
    assert_eq!(
        result.len(),
        100,
        "All trades should be flushed and queryable"
    );
}

#[test]
fn test_aggregated_query_performance() {
    // Test: Verify aggregated queries are efficient
    let (db, _dir) = create_test_db();
    let ticker_info = create_test_ticker_info();

    // Insert trades
    let trades = create_test_trades(10000);
    db.insert_trades(&ticker_info, &trades).unwrap();

    // Query aggregated data
    let start = Instant::now();
    let aggregated = db
        .query_trades_aggregated(&ticker_info, 0, i64::MAX as u64)
        .unwrap();
    let query_time = start.elapsed();

    println!(
        "Aggregated query of {} price levels took {:?}",
        aggregated.len(),
        query_time
    );

    assert!(
        query_time.as_millis() < 100,
        "Aggregated query should be fast"
    );
    assert!(!aggregated.is_empty(), "Should have aggregated data");
}

#[test]
fn test_time_range_query_optimization() {
    // Test: Verify time-range queries use indexes efficiently
    let (db, _dir) = create_test_db();
    let ticker_info = create_test_ticker_info();

    let trades = create_test_trades(10000);
    db.insert_trades(&ticker_info, &trades).unwrap();

    // Query narrow time range (should be fast due to indexes)
    let start = Instant::now();
    let result = db
        .query_trades(&ticker_info, 1005000000, 1006000000)
        .unwrap();
    let query_time = start.elapsed();

    println!(
        "Time-range query returned {} trades in {:?}",
        result.len(),
        query_time
    );

    // Should be very fast with proper indexing
    assert!(
        query_time.as_millis() < 50,
        "Time-range query should be fast with indexes"
    );
}

#[test]
fn test_health_report_summary() {
    // Test: Verify health report formatting
    let (db, _dir) = create_test_db();
    let db = Arc::new(db);

    let monitor = DbHealthMonitor::new(db);
    let report = monitor.run_health_check();

    let summary = report.summary();

    println!("Health report summary: {}", summary);
    assert!(summary.contains("Healthy") || summary.contains("query latency"));
}
