//! Progress tracking for migration operations
//!
//! Provides console output with ETA estimation for long-running migrations

use std::time::{Duration, Instant};

/// Tracks and displays migration progress with ETA estimation
pub struct ProgressTracker {
    total_items: usize,
    processed_items: usize,
    start_time: Instant,
    last_update: Instant,
    label: String,
}

impl ProgressTracker {
    /// Create a new progress tracker
    pub fn new(total_items: usize, label: impl Into<String>) -> Self {
        let now = Instant::now();
        Self {
            total_items,
            processed_items: 0,
            start_time: now,
            last_update: now,
            label: label.into(),
        }
    }

    /// Update progress with newly processed count
    pub fn update(&mut self, processed_count: usize) {
        self.processed_items += processed_count;

        let now = Instant::now();
        // Only update display every 500ms to avoid spam
        if now.duration_since(self.last_update) < Duration::from_millis(500) {
            return;
        }

        self.last_update = now;
        self.display_progress();
    }

    /// Set processed count directly (useful for batch operations)
    pub fn set_processed(&mut self, processed_count: usize) {
        self.processed_items = processed_count;

        let now = Instant::now();
        if now.duration_since(self.last_update) < Duration::from_millis(500) {
            return;
        }

        self.last_update = now;
        self.display_progress();
    }

    /// Display current progress with ETA
    fn display_progress(&self) {
        if self.total_items == 0 {
            return;
        }

        let elapsed = self.start_time.elapsed();
        let percent = (self.processed_items as f64 / self.total_items as f64) * 100.0;

        // Calculate ETA
        let eta = if self.processed_items > 0 {
            let rate = self.processed_items as f64 / elapsed.as_secs_f64();
            let remaining = self.total_items - self.processed_items;
            let eta_secs = remaining as f64 / rate;
            format_duration(Duration::from_secs_f64(eta_secs))
        } else {
            "calculating...".to_string()
        };

        log::info!(
            "{}: {}/{} ({:.1}%) - Elapsed: {} - ETA: {}",
            self.label,
            self.processed_items,
            self.total_items,
            percent,
            format_duration(elapsed),
            eta
        );
    }

    /// Mark migration as complete and display final stats
    pub fn finish(&self) {
        let elapsed = self.start_time.elapsed();
        log::info!(
            "{}: Completed {} items in {}",
            self.label,
            self.processed_items,
            format_duration(elapsed)
        );
    }
}

/// Format duration as human-readable string
fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(3660)), "1h 1m");
    }

    #[test]
    fn test_progress_tracker() {
        let mut tracker = ProgressTracker::new(100, "Test");
        tracker.update(50);
        assert_eq!(tracker.processed_items, 50);
        tracker.finish();
    }
}
