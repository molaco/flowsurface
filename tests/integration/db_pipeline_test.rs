//! End-to-end integration tests for database pipeline

use data::db::{DatabaseManager, TradesCRUD, KlinesCRUD, DatabaseConfig};
use exchange::Exchange;

#[path = "../common/mod.rs"]
mod common;

use common::fixtures::{create_test_ticker_info, generate_test_trades, generate_test_klines};
use common::assertions::{assert_trade_eq, compare_trade_vectors};
use common::setup_test_environment;

#[test]
fn test_trades_persist_and_query_cycle() {
    let env = setup_test_environment().expect("Failed to setup test environment");
    let db = env.db();

    let ticker_info = create_test_ticker_info(
        Exchange::BinanceLinear,
        "BTCUSDT",
        0.1,
    );

    // Generate test trades
    let trades = generate_test_trades(100, 1000000000, 1000, 50000.0, 100.0);

    // Insert trades
    db.insert_trades(&ticker_info, &trades)
        .expect("Failed to insert trades");

    // Query trades back
    let start_time = trades.first().unwrap().time;
    let end_time = trades.last().unwrap().time;

    let queried_trades = db
        .query_trades(&ticker_info, start_time, end_time)
        .expect("Failed to query trades");

    // Verify all trades were persisted and queried correctly
    assert_eq!(
        queried_trades.len(),
        trades.len(),
        "Queried trade count mismatch"
    );

    // Compare trades
    compare_trade_vectors(&queried_trades, &trades).expect("Trade vectors don't match");
}

#[test]
fn test_klines_persist_and_query_cycle() {
    let env = setup_test_environment().expect("Failed to setup test environment");
    let db = env.db();

    let ticker_info = create_test_ticker_info(
        Exchange::BinanceLinear,
        "BTCUSDT",
        0.1,
    );

    // Generate test klines (1 minute interval)
    let klines = generate_test_klines(50, 1000000000, 60000, 50000.0);

    // Insert klines
    for kline in &klines {
        db.insert_kline(&ticker_info, kline, "1m")
            .expect("Failed to insert kline");
    }

    // Query klines back
    let start_time = klines.first().unwrap().time;
    let end_time = klines.last().unwrap().time;

    let queried_klines = db
        .query_klines(&ticker_info, "1m", start_time, end_time)
        .expect("Failed to query klines");

    // Verify all klines were persisted
    assert_eq!(
        queried_klines.len(),
        klines.len(),
        "Queried kline count mismatch"
    );
}

#[test]
fn test_multiple_tickers_isolation() {
    let env = setup_test_environment().expect("Failed to setup test environment");
    let db = env.db();

    let ticker1 = create_test_ticker_info(
        Exchange::BinanceFutures,
        "BTCUSDT",
        MinTicksize::ZeroPointOne,
    );

    let ticker2 = create_test_ticker_info(
        Exchange::BinanceLinear,
        "ETHUSDT",
        0.01,
    );

    // Insert trades for both tickers
    let trades1 = generate_test_trades(50, 1000000000, 1000, 50000.0, 100.0);
    let trades2 = generate_test_trades(30, 1000000000, 1000, 3000.0, 10.0);

    db.insert_trades(&ticker1, &trades1)
        .expect("Failed to insert trades for ticker1");
    db.insert_trades(&ticker2, &trades2)
        .expect("Failed to insert trades for ticker2");

    // Query trades for ticker1
    let queried_trades1 = db
        .query_trades(&ticker1, 1000000000, 1000000000 + 100000)
        .expect("Failed to query trades for ticker1");

    // Query trades for ticker2
    let queried_trades2 = db
        .query_trades(&ticker2, 1000000000, 1000000000 + 100000)
        .expect("Failed to query trades for ticker2");

    // Verify isolation - each ticker should only return its own trades
    assert_eq!(queried_trades1.len(), trades1.len());
    assert_eq!(queried_trades2.len(), trades2.len());
}

#[test]
fn test_bulk_insert_performance() {
    let env = setup_test_environment().expect("Failed to setup test environment");
    let db = env.db();

    let ticker_info = create_test_ticker_info(
        Exchange::BinanceLinear,
        "BTCUSDT",
        0.1,
    );

    // Generate large batch of trades
    let trades = generate_test_trades(10000, 1000000000, 100, 50000.0, 100.0);

    // Measure insert time
    let start = std::time::Instant::now();
    db.insert_trades(&ticker_info, &trades)
        .expect("Failed to insert trades");
    let duration = start.elapsed();

    println!(
        "Inserted {} trades in {:?} ({:.2} trades/sec)",
        trades.len(),
        duration,
        trades.len() as f64 / duration.as_secs_f64()
    );

    // Verify the insert rate is reasonable (> 1000 trades/sec)
    let trades_per_sec = trades.len() as f64 / duration.as_secs_f64();
    assert!(
        trades_per_sec > 1000.0,
        "Insert performance too slow: {:.2} trades/sec",
        trades_per_sec
    );
}

#[test]
fn test_query_time_range_boundaries() {
    let env = setup_test_environment().expect("Failed to setup test environment");
    let db = env.db();

    let ticker_info = create_test_ticker_info(
        Exchange::BinanceLinear,
        "BTCUSDT",
        0.1,
    );

    let trades = generate_test_trades(100, 1000000000, 1000, 50000.0, 100.0);
    db.insert_trades(&ticker_info, &trades)
        .expect("Failed to insert trades");

    // Query with exact boundaries
    let start_time = trades[10].time;
    let end_time = trades[20].time;

    let queried = db
        .query_trades(&ticker_info, start_time, end_time)
        .expect("Failed to query trades");

    // Verify boundary inclusivity
    assert!(queried.len() >= 10, "Expected at least 10 trades in range");

    // Verify all returned trades are within range
    for trade in &queried {
        assert!(
            trade.time >= start_time && trade.time <= end_time,
            "Trade outside query range"
        );
    }
}

#[test]
fn test_empty_query_results() {
    let env = setup_test_environment().expect("Failed to setup test environment");
    let db = env.db();

    let ticker_info = create_test_ticker_info(
        Exchange::BinanceLinear,
        "BTCUSDT",
        0.1,
    );

    // Query before any data exists
    let queried = db
        .query_trades(&ticker_info, 1000000000, 1000100000)
        .expect("Failed to query trades");

    assert_eq!(queried.len(), 0, "Expected empty result set");
}

#[test]
fn test_database_stats() {
    let env = setup_test_environment().expect("Failed to setup test environment");
    let db = env.db();

    let ticker_info = create_test_ticker_info(
        Exchange::BinanceLinear,
        "BTCUSDT",
        0.1,
    );

    // Insert some data
    let trades = generate_test_trades(100, 1000000000, 1000, 50000.0, 100.0);
    db.insert_trades(&ticker_info, &trades)
        .expect("Failed to insert trades");

    // Get statistics
    let stats = db.get_stats().expect("Failed to get database stats");

    assert!(stats.total_trades > 0, "Expected non-zero trade count");
    assert!(stats.total_tickers > 0, "Expected non-zero ticker count");
}
