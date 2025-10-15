//! CRUD operations for all data types
//!
//! This module provides Create, Read, Update, Delete operations for:
//! - Trades: Individual trade executions
//! - Klines: OHLCV candlestick data across multiple timeframes
//! - Depth: Orderbook snapshots for heatmap reconstruction
//! - Footprint: Price-level aggregations within klines

use super::error::Result;
use exchange::{Kline, TickerInfo, Timeframe, Trade};
use exchange::depth::Depth;
use crate::chart::kline::KlineTrades;
use crate::aggr::time::TimeSeries;
use crate::chart::kline::KlineDataPoint;

/// Trait for Trade CRUD operations
pub trait TradesCRUD {
    /// Insert multiple trades in bulk using DuckDB Appender API
    fn insert_trades(&self, ticker_info: &TickerInfo, trades: &[Trade]) -> Result<usize>;

    /// Query trades by time range
    fn query_trades(&self, ticker_info: &TickerInfo, start_time: u64, end_time: u64) -> Result<Vec<Trade>>;

    /// Count trades in time range
    fn query_trades_count(&self, ticker_info: &TickerInfo, start_time: u64, end_time: u64) -> Result<i64>;

    /// Delete trades older than cutoff timestamp
    fn delete_trades_older_than(&self, cutoff_time: u64) -> Result<usize>;

    /// Query trades aggregated by price level for efficient footprint reconstruction
    fn query_trades_aggregated(&self, ticker_info: &TickerInfo, start_time: u64, end_time: u64) -> Result<Vec<(exchange::util::Price, f32, f32, usize, usize)>>;

    /// Check database coverage for trade data
    fn query_trades_coverage(&self, ticker_info: &TickerInfo) -> Result<Option<(u64, u64)>>;
}

/// Trait for Kline CRUD operations
pub trait KlinesCRUD {
    /// Insert or update klines for a specific timeframe
    fn insert_klines(&self, ticker_info: &TickerInfo, timeframe: Timeframe, klines: &[Kline]) -> Result<usize>;

    /// Query klines by timeframe and time range
    fn query_klines(&self, ticker_info: &TickerInfo, timeframe: Timeframe, start_time: u64, end_time: u64) -> Result<Vec<Kline>>;

    /// Load klines as TimeSeries for chart rendering
    fn load_timeseries(&self, ticker_info: &TickerInfo, timeframe: Timeframe, start_time: u64, end_time: u64) -> Result<TimeSeries<KlineDataPoint>>;

    /// Get the most recent kline for a timeframe
    fn query_latest_kline(&self, ticker_info: &TickerInfo, timeframe: Timeframe) -> Result<Option<Kline>>;

    /// Delete klines older than cutoff timestamp
    fn delete_klines_older_than(&self, cutoff_time: u64) -> Result<usize>;
}

/// Trait for Depth snapshot CRUD operations
pub trait DepthCRUD {
    /// Insert a full depth snapshot
    fn insert_depth_snapshot(&self, ticker_info: &TickerInfo, snapshot_time: u64, depth: &Depth) -> Result<usize>;

    /// Query a depth snapshot at specific time
    fn query_depth_snapshot(&self, ticker_info: &TickerInfo, snapshot_time: u64) -> Result<Option<Depth>>;

    /// Query multiple depth snapshots in time range
    fn query_depth_snapshots_range(&self, ticker_info: &TickerInfo, start_time: u64, end_time: u64) -> Result<Vec<(u64, Depth)>>;

    /// Delete depth snapshots older than cutoff
    fn delete_depth_snapshots_older_than(&self, cutoff_time: u64) -> Result<usize>;
}

/// Trait for Footprint data CRUD operations
pub trait FootprintCRUD {
    /// Insert footprint data for a kline
    fn insert_footprint(&self, ticker_info: &TickerInfo, timeframe: Timeframe, kline_time: u64, footprint: &KlineTrades) -> Result<usize>;

    /// Query footprint data for a specific kline
    fn query_footprint(&self, ticker_info: &TickerInfo, timeframe: Timeframe, kline_time: u64) -> Result<Option<KlineTrades>>;

    /// Query footprints for multiple klines in time range
    fn query_footprints_range(&self, ticker_info: &TickerInfo, timeframe: Timeframe, start_time: u64, end_time: u64) -> Result<std::collections::BTreeMap<u64, KlineTrades>>;

    /// Load TimeSeries with both klines and footprints
    fn load_timeseries_with_footprints(&self, ticker_info: &TickerInfo, timeframe: Timeframe, start_time: u64, end_time: u64) -> Result<TimeSeries<KlineDataPoint>>;

    /// Delete footprint data older than cutoff
    fn delete_footprints_older_than(&self, cutoff_time: u64) -> Result<usize>;
}

// Import implementations
pub mod trades;
pub mod klines;
pub mod depth;
pub mod footprint;
pub mod order_runs;
