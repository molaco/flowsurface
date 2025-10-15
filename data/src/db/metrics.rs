//! Performance metrics tracking for database operations
//!
//! Tracks insert and query latencies, database size growth, and operation counts.
//! Provides instrumentation for detecting performance regressions.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Performance metrics for database operations
///
/// Uses atomic operations for thread-safe, low-overhead metric recording.
/// Tracks operation counts and latencies for inserts and queries.
#[derive(Clone)]
pub struct PerformanceMetrics {
    /// Total number of insert operations
    insert_count: Arc<AtomicU64>,
    /// Total number of query operations
    query_count: Arc<AtomicU64>,
    /// Total insert latency in microseconds
    insert_latency_us: Arc<AtomicU64>,
    /// Total query latency in microseconds
    query_latency_us: Arc<AtomicU64>,
    /// Maximum insert latency observed (microseconds)
    max_insert_latency_us: Arc<AtomicU64>,
    /// Maximum query latency observed (microseconds)
    max_query_latency_us: Arc<AtomicU64>,
}

impl PerformanceMetrics {
    /// Create new performance metrics instance
    pub fn new() -> Self {
        Self {
            insert_count: Arc::new(AtomicU64::new(0)),
            query_count: Arc::new(AtomicU64::new(0)),
            insert_latency_us: Arc::new(AtomicU64::new(0)),
            query_latency_us: Arc::new(AtomicU64::new(0)),
            max_insert_latency_us: Arc::new(AtomicU64::new(0)),
            max_query_latency_us: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Record insert operation latency
    ///
    /// Updates insert count, total latency, and max latency atomically.
    /// Thread-safe and non-blocking.
    pub fn record_insert_latency(&self, latency: Duration) {
        let latency_us = latency.as_micros() as u64;

        self.insert_count.fetch_add(1, Ordering::Relaxed);
        self.insert_latency_us.fetch_add(latency_us, Ordering::Relaxed);

        // Update max latency if this is higher
        let mut current_max = self.max_insert_latency_us.load(Ordering::Relaxed);
        while latency_us > current_max {
            match self.max_insert_latency_us.compare_exchange_weak(
                current_max,
                latency_us,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_max = actual,
            }
        }
    }

    /// Record query operation latency
    ///
    /// Updates query count, total latency, and max latency atomically.
    pub fn record_query_latency(&self, latency: Duration) {
        let latency_us = latency.as_micros() as u64;

        self.query_count.fetch_add(1, Ordering::Relaxed);
        self.query_latency_us.fetch_add(latency_us, Ordering::Relaxed);

        // Update max latency if this is higher
        let mut current_max = self.max_query_latency_us.load(Ordering::Relaxed);
        while latency_us > current_max {
            match self.max_query_latency_us.compare_exchange_weak(
                current_max,
                latency_us,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_max = actual,
            }
        }
    }

    /// Get current statistics snapshot
    ///
    /// Returns point-in-time view of metrics without blocking.
    pub fn get_statistics(&self) -> MetricsSnapshot {
        let insert_count = self.insert_count.load(Ordering::Relaxed);
        let query_count = self.query_count.load(Ordering::Relaxed);
        let insert_latency_us = self.insert_latency_us.load(Ordering::Relaxed);
        let query_latency_us = self.query_latency_us.load(Ordering::Relaxed);
        let max_insert_latency_us = self.max_insert_latency_us.load(Ordering::Relaxed);
        let max_query_latency_us = self.max_query_latency_us.load(Ordering::Relaxed);

        let avg_insert_latency_us = if insert_count > 0 {
            insert_latency_us / insert_count
        } else {
            0
        };

        let avg_query_latency_us = if query_count > 0 {
            query_latency_us / query_count
        } else {
            0
        };

        MetricsSnapshot {
            insert_count,
            query_count,
            avg_insert_latency_us,
            avg_query_latency_us,
            max_insert_latency_us,
            max_query_latency_us,
        }
    }

    /// Reset all metrics to zero
    ///
    /// Useful for starting fresh measurement periods.
    pub fn reset(&self) {
        self.insert_count.store(0, Ordering::Relaxed);
        self.query_count.store(0, Ordering::Relaxed);
        self.insert_latency_us.store(0, Ordering::Relaxed);
        self.query_latency_us.store(0, Ordering::Relaxed);
        self.max_insert_latency_us.store(0, Ordering::Relaxed);
        self.max_query_latency_us.store(0, Ordering::Relaxed);
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Point-in-time snapshot of performance metrics
#[derive(Debug, Clone, Copy)]
pub struct MetricsSnapshot {
    /// Total insert operations
    pub insert_count: u64,
    /// Total query operations
    pub query_count: u64,
    /// Average insert latency in microseconds
    pub avg_insert_latency_us: u64,
    /// Average query latency in microseconds
    pub avg_query_latency_us: u64,
    /// Maximum insert latency observed (microseconds)
    pub max_insert_latency_us: u64,
    /// Maximum query latency observed (microseconds)
    pub max_query_latency_us: u64,
}

impl MetricsSnapshot {
    /// Format metrics as human-readable string
    pub fn summary(&self) -> String {
        format!(
            "Inserts: {} (avg: {:.2}ms, max: {:.2}ms) | Queries: {} (avg: {:.2}ms, max: {:.2}ms)",
            self.insert_count,
            self.avg_insert_latency_us as f64 / 1000.0,
            self.max_insert_latency_us as f64 / 1000.0,
            self.query_count,
            self.avg_query_latency_us as f64 / 1000.0,
            self.max_query_latency_us as f64 / 1000.0,
        )
    }

    /// Check if metrics indicate performance issues
    ///
    /// Returns true if average or max latencies exceed reasonable thresholds.
    pub fn has_performance_issues(&self) -> bool {
        // Warn if average insert latency > 10ms or max > 100ms
        let slow_inserts = self.avg_insert_latency_us > 10_000 || self.max_insert_latency_us > 100_000;

        // Warn if average query latency > 100ms or max > 1000ms
        let slow_queries = self.avg_query_latency_us > 100_000 || self.max_query_latency_us > 1_000_000;

        slow_inserts || slow_queries
    }

    /// Get list of performance warnings
    pub fn warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        if self.avg_insert_latency_us > 10_000 {
            warnings.push(format!(
                "High average insert latency: {:.2}ms",
                self.avg_insert_latency_us as f64 / 1000.0
            ));
        }

        if self.max_insert_latency_us > 100_000 {
            warnings.push(format!(
                "High max insert latency: {:.2}ms",
                self.max_insert_latency_us as f64 / 1000.0
            ));
        }

        if self.avg_query_latency_us > 100_000 {
            warnings.push(format!(
                "High average query latency: {:.2}ms",
                self.avg_query_latency_us as f64 / 1000.0
            ));
        }

        if self.max_query_latency_us > 1_000_000 {
            warnings.push(format!(
                "High max query latency: {:.2}ms",
                self.max_query_latency_us as f64 / 1000.0
            ));
        }

        warnings
    }
}

/// Timer for measuring operation duration
///
/// Automatically records latency when dropped.
pub struct MetricTimer<'a> {
    metrics: &'a PerformanceMetrics,
    start: Instant,
    operation: MetricOperation,
}

impl<'a> MetricTimer<'a> {
    /// Start timing an insert operation
    pub fn insert(metrics: &'a PerformanceMetrics) -> Self {
        Self {
            metrics,
            start: Instant::now(),
            operation: MetricOperation::Insert,
        }
    }

    /// Start timing a query operation
    pub fn query(metrics: &'a PerformanceMetrics) -> Self {
        Self {
            metrics,
            start: Instant::now(),
            operation: MetricOperation::Query,
        }
    }

    /// Stop timer and record latency manually
    pub fn stop(self) {
        drop(self);
    }
}

impl<'a> Drop for MetricTimer<'a> {
    fn drop(&mut self) {
        let latency = self.start.elapsed();
        match self.operation {
            MetricOperation::Insert => self.metrics.record_insert_latency(latency),
            MetricOperation::Query => self.metrics.record_query_latency(latency),
        }
    }
}

/// Type of database operation being measured
enum MetricOperation {
    Insert,
    Query,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_metrics_creation() {
        let metrics = PerformanceMetrics::new();
        let stats = metrics.get_statistics();

        assert_eq!(stats.insert_count, 0);
        assert_eq!(stats.query_count, 0);
        assert_eq!(stats.avg_insert_latency_us, 0);
        assert_eq!(stats.avg_query_latency_us, 0);
    }

    #[test]
    fn test_record_insert_latency() {
        let metrics = PerformanceMetrics::new();

        metrics.record_insert_latency(Duration::from_millis(5));
        metrics.record_insert_latency(Duration::from_millis(10));

        let stats = metrics.get_statistics();
        assert_eq!(stats.insert_count, 2);
        assert!(stats.avg_insert_latency_us > 0);
        assert_eq!(stats.max_insert_latency_us, 10_000);
    }

    #[test]
    fn test_record_query_latency() {
        let metrics = PerformanceMetrics::new();

        metrics.record_query_latency(Duration::from_millis(20));
        metrics.record_query_latency(Duration::from_millis(30));

        let stats = metrics.get_statistics();
        assert_eq!(stats.query_count, 2);
        assert!(stats.avg_query_latency_us > 0);
        assert_eq!(stats.max_query_latency_us, 30_000);
    }

    #[test]
    fn test_average_calculation() {
        let metrics = PerformanceMetrics::new();

        metrics.record_insert_latency(Duration::from_millis(10));
        metrics.record_insert_latency(Duration::from_millis(20));
        metrics.record_insert_latency(Duration::from_millis(30));

        let stats = metrics.get_statistics();
        assert_eq!(stats.insert_count, 3);
        // Average should be 20ms = 20000us
        assert_eq!(stats.avg_insert_latency_us, 20_000);
    }

    #[test]
    fn test_max_latency_tracking() {
        let metrics = PerformanceMetrics::new();

        metrics.record_insert_latency(Duration::from_millis(5));
        metrics.record_insert_latency(Duration::from_millis(50));
        metrics.record_insert_latency(Duration::from_millis(10));

        let stats = metrics.get_statistics();
        assert_eq!(stats.max_insert_latency_us, 50_000);
    }

    #[test]
    fn test_reset_metrics() {
        let metrics = PerformanceMetrics::new();

        metrics.record_insert_latency(Duration::from_millis(10));
        metrics.record_query_latency(Duration::from_millis(20));

        metrics.reset();

        let stats = metrics.get_statistics();
        assert_eq!(stats.insert_count, 0);
        assert_eq!(stats.query_count, 0);
        assert_eq!(stats.avg_insert_latency_us, 0);
        assert_eq!(stats.avg_query_latency_us, 0);
    }

    #[test]
    fn test_concurrent_recording() {
        let metrics = Arc::new(PerformanceMetrics::new());

        let mut handles = vec![];
        for _ in 0..10 {
            let metrics_clone = Arc::clone(&metrics);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    metrics_clone.record_insert_latency(Duration::from_micros(100));
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = metrics.get_statistics();
        assert_eq!(stats.insert_count, 1000);
    }

    #[test]
    fn test_metric_timer_insert() {
        let metrics = PerformanceMetrics::new();

        {
            let _timer = MetricTimer::insert(&metrics);
            thread::sleep(Duration::from_millis(10));
        } // Timer drops here and records latency

        let stats = metrics.get_statistics();
        assert_eq!(stats.insert_count, 1);
        assert!(stats.avg_insert_latency_us >= 10_000);
    }

    #[test]
    fn test_metric_timer_query() {
        let metrics = PerformanceMetrics::new();

        {
            let _timer = MetricTimer::query(&metrics);
            thread::sleep(Duration::from_millis(5));
        }

        let stats = metrics.get_statistics();
        assert_eq!(stats.query_count, 1);
        assert!(stats.avg_query_latency_us >= 5_000);
    }

    #[test]
    fn test_metrics_snapshot_summary() {
        let metrics = PerformanceMetrics::new();

        metrics.record_insert_latency(Duration::from_millis(5));
        metrics.record_query_latency(Duration::from_millis(10));

        let stats = metrics.get_statistics();
        let summary = stats.summary();

        assert!(summary.contains("Inserts: 1"));
        assert!(summary.contains("Queries: 1"));
    }

    #[test]
    fn test_performance_issues_detection() {
        let metrics = PerformanceMetrics::new();

        // Record slow operation
        metrics.record_insert_latency(Duration::from_millis(150));

        let stats = metrics.get_statistics();
        assert!(stats.has_performance_issues());

        let warnings = stats.warnings();
        assert!(!warnings.is_empty());
    }

    #[test]
    fn test_no_performance_issues() {
        let metrics = PerformanceMetrics::new();

        // Record fast operations
        metrics.record_insert_latency(Duration::from_millis(1));
        metrics.record_query_latency(Duration::from_millis(10));

        let stats = metrics.get_statistics();
        assert!(!stats.has_performance_issues());
        assert!(stats.warnings().is_empty());
    }
}
