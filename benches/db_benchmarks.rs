//! Database performance benchmarks using Criterion

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use data::db::{DatabaseManager, DatabaseConfig, TradesCRUD, KlinesCRUD};
use exchange::{Trade, Kline, TickerInfo, Ticker};
use exchange::util::{Price, MinTicksize};
use exchange::adapter::Exchange;
use tempfile::TempDir;

fn create_test_db() -> (DatabaseManager, TempDir) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("bench.duckdb");
    let config = DatabaseConfig::new().with_memory_limit(2);
    let db = DatabaseManager::with_config(&db_path, config).unwrap();
    (db, dir)
}

fn create_test_ticker() -> TickerInfo {
    let ticker = Ticker::new("BTCUSDT", Exchange::BinanceLinear);
    TickerInfo::new(ticker, 0.1, 0.001, None)
}

fn generate_trades(count: usize) -> Vec<Trade> {
    (0..count)
        .map(|i| Trade {
            time: 1000000000 + (i as u64 * 100),
            price: Price::from_f32(50000.0 + (i as f32 % 100.0)),
            qty: 1.0 + (i as f32 * 0.01),
            is_sell: i % 2 == 0,
        })
        .collect()
}

fn generate_klines(count: usize) -> Vec<Kline> {
    (0..count)
        .map(|i| {
            let base_price = 50000.0 + (i as f32 * 10.0);
            Kline::new(
                1000000000 + (i as u64 * 60000),
                base_price,
                base_price + 50.0,
                base_price - 50.0,
                base_price + 25.0,
                (100.0, 100.0 * base_price),
                MinTicksize::from(0.1),
            )
        })
        .collect()
}

fn bench_bulk_insert_trades(c: &mut Criterion) {
    let mut group = c.benchmark_group("trade_insert");

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || {
                    let (db, _dir) = create_test_db();
                    let ticker = create_test_ticker();
                    let trades = generate_trades(size);
                    (db, ticker, trades, _dir)
                },
                |(db, ticker, trades, _dir)| {
                    db.insert_trades(&ticker, &trades).unwrap();
                    black_box(())
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_query_trades(c: &mut Criterion) {
    let mut group = c.benchmark_group("trade_query");

    // Setup: Insert 100k trades once
    let (db, _dir) = create_test_db();
    let ticker = create_test_ticker();
    let trades = generate_trades(100000);
    db.insert_trades(&ticker, &trades).unwrap();

    for range_size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*range_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(range_size),
            range_size,
            |b, &range_size| {
                let start_time = 1000000000u64;
                let end_time = start_time + (range_size as u64 * 100);

                b.iter(|| {
                    let result = db.query_trades(&ticker, start_time, end_time).unwrap();
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

fn bench_aggregated_query(c: &mut Criterion) {
    let (db, _dir) = create_test_db();
    let ticker = create_test_ticker();
    let trades = generate_trades(50000);
    db.insert_trades(&ticker, &trades).unwrap();

    c.bench_function("aggregated_trades_query", |b| {
        b.iter(|| {
            let result = db
                .query_trades_aggregated(&ticker, 1000000000, 1000000000 + 5000000)
                .unwrap();
            black_box(result)
        });
    });
}

fn bench_kline_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("kline_insert");

    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || {
                    let (db, _dir) = create_test_db();
                    let ticker = create_test_ticker();
                    let klines = generate_klines(size);
                    (db, ticker, klines, _dir)
                },
                |(db, ticker, klines, _dir)| {
                    for kline in &klines {
                        db.insert_kline(&ticker, kline, "1m").unwrap();
                    }
                    black_box(())
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_kline_query(c: &mut Criterion) {
    let (db, _dir) = create_test_db();
    let ticker = create_test_ticker();
    let klines = generate_klines(1000);

    for kline in &klines {
        db.insert_kline(&ticker, kline, "1m").unwrap();
    }

    c.bench_function("kline_query_1000", |b| {
        b.iter(|| {
            let result = db
                .query_klines(&ticker, "1m", 1000000000, 1000000000 + 60000000)
                .unwrap();
            black_box(result)
        });
    });
}

criterion_group!(
    benches,
    bench_bulk_insert_trades,
    bench_query_trades,
    bench_aggregated_query,
    bench_kline_insert,
    bench_kline_query
);

criterion_main!(benches);
