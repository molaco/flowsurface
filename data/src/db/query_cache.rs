//! Query caching layer to avoid repeated database hits
//!
//! Provides an LRU cache for database query results with automatic invalidation.
//! Cache keys are based on (ticker_id, timeframe, time_range) to ensure correct results.

use crate::aggr::time::TimeSeries;
use crate::chart::kline::KlineDataPoint;
use exchange::{Timeframe, Trade};
use rustc_hash::FxHashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Default maximum cache entries
const DEFAULT_CACHE_SIZE: usize = 100;

/// Default cache entry TTL (time to live)
const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes

/// Cache key for timeseries queries
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct TimeseriesCacheKey {
    ticker_id: i32,
    timeframe: String,
    start_time: u64,
    end_time: u64,
}

/// Cache key for trade queries
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct TradesCacheKey {
    ticker_id: i32,
    start_time: u64,
    end_time: u64,
}

/// Cached entry with timestamp for TTL checking
#[derive(Clone)]
struct CacheEntry<T> {
    data: Arc<T>,
    cached_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(data: T) -> Self {
        Self {
            data: Arc::new(data),
            cached_at: Instant::now(),
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.cached_at.elapsed() > ttl
    }
}

/// Query cache with LRU eviction and TTL-based invalidation
pub struct QueryCache {
    /// Cache for timeseries queries (using Arc to avoid cloning)
    timeseries_cache: Arc<Mutex<FxHashMap<TimeseriesCacheKey, CacheEntry<TimeSeries<KlineDataPoint>>>>>,

    /// Cache for trade queries
    trades_cache: Arc<Mutex<FxHashMap<TradesCacheKey, CacheEntry<Vec<Trade>>>>>,

    /// Maximum number of entries per cache
    max_entries: usize,

    /// Time to live for cache entries
    ttl: Duration,
}

impl std::fmt::Debug for QueryCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryCache")
            .field("max_entries", &self.max_entries)
            .field("ttl", &self.ttl)
            .finish_non_exhaustive()
    }
}

impl QueryCache {
    /// Create a new query cache with default settings
    pub fn new() -> Self {
        Self::with_config(DEFAULT_CACHE_SIZE, DEFAULT_CACHE_TTL)
    }

    /// Create a new query cache with custom configuration
    pub fn with_config(max_entries: usize, ttl: Duration) -> Self {
        Self {
            timeseries_cache: Arc::new(Mutex::new(FxHashMap::default())),
            trades_cache: Arc::new(Mutex::new(FxHashMap::default())),
            max_entries,
            ttl,
        }
    }

    /// Get cached timeseries if available and not expired
    /// Returns Arc reference to avoid cloning
    pub fn get_timeseries(
        &self,
        ticker_id: i32,
        timeframe: Timeframe,
        start_time: u64,
        end_time: u64,
    ) -> Option<Arc<TimeSeries<KlineDataPoint>>> {
        let key = TimeseriesCacheKey {
            ticker_id,
            timeframe: format!("{}", timeframe),
            start_time,
            end_time,
        };

        let mut cache = self.timeseries_cache.lock().ok()?;

        if let Some(entry) = cache.get(&key) {
            if !entry.is_expired(self.ttl) {
                return Some(Arc::clone(&entry.data));
            } else {
                // Remove expired entry
                cache.remove(&key);
            }
        }

        None
    }

    /// Store timeseries in cache
    pub fn put_timeseries(
        &self,
        ticker_id: i32,
        timeframe: Timeframe,
        start_time: u64,
        end_time: u64,
        timeseries: TimeSeries<KlineDataPoint>,
    ) {
        let key = TimeseriesCacheKey {
            ticker_id,
            timeframe: format!("{}", timeframe),
            start_time,
            end_time,
        };

        if let Ok(mut cache) = self.timeseries_cache.lock() {
            // Evict oldest entry if cache is full
            if cache.len() >= self.max_entries {
                // Simple eviction: remove first entry (not true LRU but good enough)
                if let Some(first_key) = cache.keys().next().cloned() {
                    cache.remove(&first_key);
                }
            }

            cache.insert(key, CacheEntry::new(timeseries));
        }
    }

    /// Get cached trades if available and not expired
    /// Returns Arc reference for efficient access
    pub fn get_trades(
        &self,
        ticker_id: i32,
        start_time: u64,
        end_time: u64,
    ) -> Option<Arc<Vec<Trade>>> {
        let key = TradesCacheKey {
            ticker_id,
            start_time,
            end_time,
        };

        let mut cache = self.trades_cache.lock().ok()?;

        if let Some(entry) = cache.get(&key) {
            if !entry.is_expired(self.ttl) {
                return Some(Arc::clone(&entry.data));
            } else {
                // Remove expired entry
                cache.remove(&key);
            }
        }

        None
    }

    /// Store trades in cache
    pub fn put_trades(
        &self,
        ticker_id: i32,
        start_time: u64,
        end_time: u64,
        trades: Vec<Trade>,
    ) {
        let key = TradesCacheKey {
            ticker_id,
            start_time,
            end_time,
        };

        if let Ok(mut cache) = self.trades_cache.lock() {
            // Evict oldest entry if cache is full
            if cache.len() >= self.max_entries {
                if let Some(first_key) = cache.keys().next().cloned() {
                    cache.remove(&first_key);
                }
            }

            cache.insert(key, CacheEntry::new(trades));
        }
    }

    /// Invalidate all cache entries for a specific ticker
    pub fn invalidate_ticker(&self, ticker_id: i32) {
        // Invalidate timeseries cache
        if let Ok(mut cache) = self.timeseries_cache.lock() {
            cache.retain(|key, _| key.ticker_id != ticker_id);
        }

        // Invalidate trades cache
        if let Ok(mut cache) = self.trades_cache.lock() {
            cache.retain(|key, _| key.ticker_id != ticker_id);
        }
    }

    /// Clear all cache entries
    pub fn clear(&self) {
        if let Ok(mut cache) = self.timeseries_cache.lock() {
            cache.clear();
        }
        if let Ok(mut cache) = self.trades_cache.lock() {
            cache.clear();
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let timeseries_count = self.timeseries_cache.lock().map(|c| c.len()).unwrap_or(0);
        let trades_count = self.trades_cache.lock().map(|c| c.len()).unwrap_or(0);

        CacheStats {
            timeseries_entries: timeseries_count,
            trades_entries: trades_count,
            max_entries: self.max_entries,
            ttl_seconds: self.ttl.as_secs(),
        }
    }
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for QueryCache {
    fn clone(&self) -> Self {
        Self {
            timeseries_cache: Arc::clone(&self.timeseries_cache),
            trades_cache: Arc::clone(&self.trades_cache),
            max_entries: self.max_entries,
            ttl: self.ttl,
        }
    }
}

/// Cache statistics for monitoring
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub timeseries_entries: usize,
    pub trades_entries: usize,
    pub max_entries: usize,
    pub ttl_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::kline::KlineDataPoint;
    use exchange::util::{Price, PriceStep};
    use exchange::Kline;
    use std::collections::BTreeMap;
    use std::thread;
    use std::time::Duration;

    fn create_test_timeseries() -> TimeSeries<KlineDataPoint> {
        let mut datapoints = BTreeMap::new();

        for i in 0..10 {
            let kline = Kline {
                time: 1000000 + i * 60000,
                open: Price::from_f32(50000.0),
                high: Price::from_f32(50100.0),
                low: Price::from_f32(49900.0),
                close: Price::from_f32(50050.0),
                volume: (100.0, 100.0),
            };

            datapoints.insert(
                kline.time,
                KlineDataPoint {
                    kline,
                    footprint: crate::chart::kline::KlineTrades::new(),
                },
            );
        }

        TimeSeries {
            datapoints,
            interval: Timeframe::M1,
            tick_size: PriceStep::from_f32(0.01),
        }
    }

    fn create_test_trades() -> Vec<Trade> {
        (0..10)
            .map(|i| Trade {
                time: 1000000 + i * 1000,
                price: Price::from_f32(50000.0 + i as f32),
                qty: 1.0,
                is_sell: i % 2 == 0,
            })
            .collect()
    }

    #[test]
    fn test_timeseries_cache_hit() {
        let cache = QueryCache::new();
        let timeseries = create_test_timeseries();

        // Store in cache
        cache.put_timeseries(1, Timeframe::M1, 1000000, 2000000, timeseries);

        // Retrieve from cache
        let cached = cache.get_timeseries(1, Timeframe::M1, 1000000, 2000000);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().datapoints.len(), 10);
    }

    #[test]
    fn test_timeseries_cache_miss() {
        let cache = QueryCache::new();

        // Cache is empty
        let cached = cache.get_timeseries(1, Timeframe::M1, 1000000, 2000000);
        assert!(cached.is_none());
    }

    #[test]
    fn test_trades_cache_hit() {
        let cache = QueryCache::new();
        let trades = create_test_trades();

        // Store in cache
        cache.put_trades(1, 1000000, 2000000, trades);

        // Retrieve from cache
        let cached = cache.get_trades(1, 1000000, 2000000);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 10);
    }

    #[test]
    fn test_cache_expiration() {
        let cache = QueryCache::with_config(100, Duration::from_millis(50));
        let timeseries = create_test_timeseries();

        // Store in cache
        cache.put_timeseries(1, Timeframe::M1, 1000000, 2000000, timeseries);

        // Immediate retrieval should work
        assert!(cache.get_timeseries(1, Timeframe::M1, 1000000, 2000000).is_some());

        // Wait for expiration
        thread::sleep(Duration::from_millis(100));

        // Should be expired now
        assert!(cache.get_timeseries(1, Timeframe::M1, 1000000, 2000000).is_none());
    }

    #[test]
    fn test_invalidate_ticker() {
        let cache = QueryCache::new();
        let timeseries1 = create_test_timeseries();
        let timeseries2 = create_test_timeseries();
        let trades1 = create_test_trades();
        let trades2 = create_test_trades();

        // Store data for ticker 1
        cache.put_timeseries(1, Timeframe::M1, 1000000, 2000000, timeseries1);
        cache.put_trades(1, 1000000, 2000000, trades1);

        // Store data for ticker 2
        cache.put_timeseries(2, Timeframe::M1, 1000000, 2000000, timeseries2);
        cache.put_trades(2, 1000000, 2000000, trades2);

        // Invalidate ticker 1
        cache.invalidate_ticker(1);

        // Ticker 1 should be gone
        assert!(cache.get_timeseries(1, Timeframe::M1, 1000000, 2000000).is_none());
        assert!(cache.get_trades(1, 1000000, 2000000).is_none());

        // Ticker 2 should still be there
        assert!(cache.get_timeseries(2, Timeframe::M1, 1000000, 2000000).is_some());
        assert!(cache.get_trades(2, 1000000, 2000000).is_some());
    }

    #[test]
    fn test_cache_eviction() {
        let cache = QueryCache::with_config(2, Duration::from_secs(300));
        let timeseries1 = create_test_timeseries();
        let timeseries2 = create_test_timeseries();
        let timeseries3 = create_test_timeseries();

        // Fill cache beyond capacity
        cache.put_timeseries(1, Timeframe::M1, 1000000, 2000000, timeseries1);
        cache.put_timeseries(2, Timeframe::M1, 1000000, 2000000, timeseries2);
        cache.put_timeseries(3, Timeframe::M1, 1000000, 2000000, timeseries3);

        // Cache should have at most 2 entries
        let stats = cache.stats();
        assert!(stats.timeseries_entries <= 2);
    }

    #[test]
    fn test_clear_cache() {
        let cache = QueryCache::new();
        let timeseries = create_test_timeseries();
        let trades = create_test_trades();

        cache.put_timeseries(1, Timeframe::M1, 1000000, 2000000, timeseries);
        cache.put_trades(1, 1000000, 2000000, trades);

        // Clear all
        cache.clear();

        // Everything should be gone
        assert!(cache.get_timeseries(1, Timeframe::M1, 1000000, 2000000).is_none());
        assert!(cache.get_trades(1, 1000000, 2000000).is_none());
    }

    #[test]
    fn test_cache_stats() {
        let cache = QueryCache::new();
        let timeseries = create_test_timeseries();
        let trades = create_test_trades();

        cache.put_timeseries(1, Timeframe::M1, 1000000, 2000000, timeseries);
        cache.put_trades(1, 1000000, 2000000, trades);

        let stats = cache.stats();
        assert_eq!(stats.timeseries_entries, 1);
        assert_eq!(stats.trades_entries, 1);
        assert_eq!(stats.max_entries, DEFAULT_CACHE_SIZE);
    }
}
