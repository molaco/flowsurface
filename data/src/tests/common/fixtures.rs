//! Test fixture generators for trades, klines, depth, and timeseries

use exchange::{Trade, Kline, TickerInfo, Ticker};
use exchange::util::{Price, MinTicksize};
use exchange::adapter::Exchange;
use std::path::Path;
use std::io::Write;

/// Creates a realistic TickerInfo instance for testing
pub fn create_test_ticker_info(
    exchange: Exchange,
    symbol: &str,
    min_ticksize: f32,
) -> TickerInfo {
    let ticker = Ticker::new(symbol, exchange);
    TickerInfo::new(ticker, min_ticksize, 0.001, None)
}

/// Generates realistic test trades with configurable parameters
pub fn generate_test_trades(
    count: usize,
    start_time: u64,
    time_interval_ms: u64,
    base_price: f32,
    price_variance: f32,
) -> Vec<Trade> {
    let mut trades = Vec::with_capacity(count);
    let mut current_price = base_price;

    for i in 0..count {
        // Simulate realistic price movement with some randomness
        let price_change = (((i * 17) % 100) as f32 - 50.0) / 100.0 * price_variance;
        current_price = (current_price + price_change).max(base_price * 0.9).min(base_price * 1.1);

        let trade = Trade {
            time: start_time + (i as u64 * time_interval_ms),
            is_sell: (i * 13) % 2 == 0,
            price: Price::from_f32(current_price).round_to_min_tick(MinTicksize::from(0.01)),
            qty: ((i % 10) + 1) as f32 * 0.1,
        };

        trades.push(trade);
    }

    trades
}

/// Generates realistic test klines with proper OHLCV relationships
pub fn generate_test_klines(
    count: usize,
    start_time: u64,
    interval_ms: u64,
    base_price: f32,
) -> Vec<Kline> {
    let mut klines = Vec::with_capacity(count);
    let mut current_close = base_price;

    for i in 0..count {
        let open = current_close;

        // Generate realistic high/low/close
        let high_offset = ((i * 7) % 20) as f32;
        let low_offset = ((i * 11) % 20) as f32;

        let high = open + high_offset;
        let low = (open - low_offset).max(open * 0.95);

        // Close moves randomly between low and high
        let close_ratio = ((i * 19) % 100) as f32 / 100.0;
        let close = low + (high - low) * close_ratio;
        current_close = close;

        let volume = (
            ((i % 100) + 50) as f32 * 10.0,  // base volume
            ((i % 100) + 50) as f32 * 10.0 * close,  // quote volume
        );

        let kline = Kline::new(
            start_time + (i as u64 * interval_ms),
            open,
            high,
            low,
            close,
            volume,
            MinTicksize::from(0.01),
        );

        klines.push(kline);
    }

    klines
}

/// Creates a sample Binance ZIP archive with aggTrades CSV format
pub fn create_sample_zip_archive(
    output_path: &Path,
    symbol: &str,
    trade_count: usize,
    start_time: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    use zip::write::{FileOptions, ZipWriter};
    use zip::CompressionMethod;

    let file = std::fs::File::create(output_path)?;
    let mut zip = ZipWriter::new(file);

    let options: FileOptions<()> = FileOptions::default()
        .compression_method(CompressionMethod::Deflated);

    // Binance aggTrades CSV filename format
    let csv_filename = format!("{}-aggTrades.csv", symbol);
    zip.start_file(&csv_filename, options)?;

    // Write CSV header (Binance format)
    // agg_trade_id,price,quantity,first_trade_id,last_trade_id,transact_time,is_buyer_maker
    writeln!(zip, "agg_trade_id,price,quantity,first_trade_id,last_trade_id,transact_time,is_buyer_maker")?;

    // Generate and write trades
    let trades = generate_test_trades(trade_count, start_time, 1000, 50000.0, 100.0);

    for (i, trade) in trades.iter().enumerate() {
        writeln!(
            zip,
            "{},{},{},{},{},{},{}",
            i + 1,
            trade.price.to_f32(),
            trade.qty,
            i + 1,
            i + 1,
            trade.time,
            if trade.is_sell { "true" } else { "false" }
        )?;
    }

    zip.finish()?;
    Ok(())
}

/// Compares two f32 values with a tolerance
pub fn approx_equal(a: f32, b: f32, epsilon: f32) -> bool {
    (a - b).abs() < epsilon
}

/// Compares two Price values with tolerance
pub fn price_approx_equal(a: Price, b: Price) -> bool {
    approx_equal(a.to_f32(), b.to_f32(), 0.00001)
}
