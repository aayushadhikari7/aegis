//! Memory resource limiter implementation.
//!
//! This module provides the `AegisResourceLimiter` which implements Wasmtime's
//! `ResourceLimiter` trait to enforce memory and table size limits.

use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::Mutex;
use tracing::{debug, warn};

/// Callback type for memory growth events.
pub type MemoryGrowthCallback = Box<dyn Fn(MemoryGrowthEvent) + Send + Sync>;

/// Event emitted when memory grows.
#[derive(Debug, Clone)]
pub struct MemoryGrowthEvent {
    /// Previous memory size in bytes.
    pub from_bytes: usize,
    /// New memory size in bytes.
    pub to_bytes: usize,
    /// Maximum allowed memory in bytes.
    pub max_bytes: usize,
}

/// Configuration for the resource limiter.
#[derive(Debug, Clone)]
pub struct LimiterConfig {
    /// Maximum memory in bytes.
    pub max_memory_bytes: usize,
    /// Maximum table elements.
    pub max_table_elements: u32,
    /// Maximum number of memory instances.
    pub max_memories: u32,
    /// Maximum number of tables.
    pub max_tables: u32,
}

impl Default for LimiterConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 64 * 1024 * 1024, // 64MB
            max_table_elements: 10_000,
            max_memories: 1,
            max_tables: 10,
        }
    }
}

impl LimiterConfig {
    /// Create a new limiter configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum memory.
    pub fn with_max_memory(mut self, bytes: usize) -> Self {
        self.max_memory_bytes = bytes;
        self
    }

    /// Set the maximum table elements.
    pub fn with_max_table_elements(mut self, elements: u32) -> Self {
        self.max_table_elements = elements;
        self
    }
}

/// Resource limiter that enforces memory and table limits.
///
/// This struct implements tracking of memory usage and can be used
/// to monitor resource consumption during WASM execution.
pub struct AegisResourceLimiter {
    /// Configuration.
    config: LimiterConfig,
    /// Current total memory usage in bytes.
    current_memory: AtomicUsize,
    /// Peak memory usage in bytes.
    peak_memory: AtomicUsize,
    /// Number of memory allocations.
    allocation_count: AtomicUsize,
    /// Optional callback for memory growth events.
    on_memory_grow: Mutex<Option<MemoryGrowthCallback>>,
}

impl AegisResourceLimiter {
    /// Create a new resource limiter with the given configuration.
    pub fn new(config: LimiterConfig) -> Self {
        Self {
            config,
            current_memory: AtomicUsize::new(0),
            peak_memory: AtomicUsize::new(0),
            allocation_count: AtomicUsize::new(0),
            on_memory_grow: Mutex::new(None),
        }
    }

    /// Create a resource limiter with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(LimiterConfig::default())
    }

    /// Set the memory growth callback.
    pub fn set_memory_growth_callback(&self, callback: MemoryGrowthCallback) {
        *self.on_memory_grow.lock() = Some(callback);
    }

    /// Get the current memory usage in bytes.
    pub fn current_memory(&self) -> usize {
        self.current_memory.load(Ordering::Relaxed)
    }

    /// Get the peak memory usage in bytes.
    pub fn peak_memory(&self) -> usize {
        self.peak_memory.load(Ordering::Relaxed)
    }

    /// Get the number of memory allocations.
    pub fn allocation_count(&self) -> usize {
        self.allocation_count.load(Ordering::Relaxed)
    }

    /// Get the remaining memory capacity in bytes.
    pub fn remaining_memory(&self) -> usize {
        self.config
            .max_memory_bytes
            .saturating_sub(self.current_memory())
    }

    /// Get the maximum memory limit in bytes.
    pub fn max_memory(&self) -> usize {
        self.config.max_memory_bytes
    }

    /// Check if memory growth is allowed.
    ///
    /// Returns `true` if the growth is permitted, `false` otherwise.
    pub fn check_memory_growth(&self, current: usize, desired: usize) -> bool {
        if desired > self.config.max_memory_bytes {
            warn!(
                current_bytes = current,
                desired_bytes = desired,
                max_bytes = self.config.max_memory_bytes,
                "Memory growth denied: exceeds limit"
            );
            return false;
        }

        // Update tracking
        self.current_memory.store(desired, Ordering::Relaxed);
        self.allocation_count.fetch_add(1, Ordering::Relaxed);

        // Update peak if necessary
        let mut peak = self.peak_memory.load(Ordering::Relaxed);
        while desired > peak {
            match self.peak_memory.compare_exchange_weak(
                peak,
                desired,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current_peak) => peak = current_peak,
            }
        }

        // Emit callback if set
        if let Some(callback) = self.on_memory_grow.lock().as_ref() {
            callback(MemoryGrowthEvent {
                from_bytes: current,
                to_bytes: desired,
                max_bytes: self.config.max_memory_bytes,
            });
        }

        debug!(
            from_bytes = current,
            to_bytes = desired,
            peak_bytes = self.peak_memory(),
            "Memory growth permitted"
        );

        true
    }

    /// Check if table growth is allowed.
    pub fn check_table_growth(&self, current: u32, desired: u32) -> bool {
        if desired > self.config.max_table_elements {
            warn!(
                current_elements = current,
                desired_elements = desired,
                max_elements = self.config.max_table_elements,
                "Table growth denied: exceeds limit"
            );
            return false;
        }

        debug!(
            from_elements = current,
            to_elements = desired,
            "Table growth permitted"
        );

        true
    }

    /// Reset the limiter statistics.
    pub fn reset(&self) {
        self.current_memory.store(0, Ordering::Relaxed);
        self.peak_memory.store(0, Ordering::Relaxed);
        self.allocation_count.store(0, Ordering::Relaxed);
    }

    /// Get a snapshot of the current statistics.
    pub fn stats(&self) -> LimiterStats {
        LimiterStats {
            current_memory: self.current_memory(),
            peak_memory: self.peak_memory(),
            allocation_count: self.allocation_count(),
            max_memory: self.config.max_memory_bytes,
        }
    }
}

/// Statistics snapshot from a resource limiter.
#[derive(Debug, Clone)]
pub struct LimiterStats {
    /// Current memory usage in bytes.
    pub current_memory: usize,
    /// Peak memory usage in bytes.
    pub peak_memory: usize,
    /// Number of memory allocations.
    pub allocation_count: usize,
    /// Maximum memory limit in bytes.
    pub max_memory: usize,
}

impl LimiterStats {
    /// Calculate memory utilization as a percentage.
    pub fn utilization_percent(&self) -> f64 {
        if self.max_memory == 0 {
            0.0
        } else {
            (self.peak_memory as f64 / self.max_memory as f64) * 100.0
        }
    }
}

impl std::fmt::Debug for AegisResourceLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AegisResourceLimiter")
            .field("config", &self.config)
            .field("current_memory", &self.current_memory())
            .field("peak_memory", &self.peak_memory())
            .field("allocation_count", &self.allocation_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_limiter_creation() {
        let limiter = AegisResourceLimiter::new(LimiterConfig::default());
        assert_eq!(limiter.current_memory(), 0);
        assert_eq!(limiter.peak_memory(), 0);
    }

    #[test]
    fn test_memory_growth_allowed() {
        let config = LimiterConfig::default().with_max_memory(1024 * 1024);
        let limiter = AegisResourceLimiter::new(config);

        assert!(limiter.check_memory_growth(0, 512 * 1024));
        assert_eq!(limiter.current_memory(), 512 * 1024);
    }

    #[test]
    fn test_memory_growth_denied() {
        let config = LimiterConfig::default().with_max_memory(1024 * 1024);
        let limiter = AegisResourceLimiter::new(config);

        assert!(!limiter.check_memory_growth(0, 2 * 1024 * 1024));
    }

    #[test]
    fn test_peak_memory_tracking() {
        let config = LimiterConfig::default().with_max_memory(10 * 1024 * 1024);
        let limiter = AegisResourceLimiter::new(config);

        limiter.check_memory_growth(0, 1024);
        limiter.check_memory_growth(1024, 2048);
        limiter.check_memory_growth(2048, 1024); // Shrink

        assert_eq!(limiter.peak_memory(), 2048);
        assert_eq!(limiter.current_memory(), 1024);
    }

    #[test]
    fn test_memory_growth_callback() {
        use std::sync::atomic::AtomicBool;

        let callback_called = Arc::new(AtomicBool::new(false));
        let callback_called_clone = Arc::clone(&callback_called);

        let limiter = AegisResourceLimiter::with_defaults();
        limiter.set_memory_growth_callback(Box::new(move |_event| {
            callback_called_clone.store(true, Ordering::SeqCst);
        }));

        limiter.check_memory_growth(0, 1024);
        assert!(callback_called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_table_growth() {
        let config = LimiterConfig::default().with_max_table_elements(1000);
        let limiter = AegisResourceLimiter::new(config);

        assert!(limiter.check_table_growth(0, 500));
        assert!(!limiter.check_table_growth(500, 1500));
    }

    #[test]
    fn test_stats() {
        let config = LimiterConfig::default().with_max_memory(1024);
        let limiter = AegisResourceLimiter::new(config);

        limiter.check_memory_growth(0, 512);

        let stats = limiter.stats();
        assert_eq!(stats.current_memory, 512);
        assert_eq!(stats.peak_memory, 512);
        assert_eq!(stats.max_memory, 1024);
        assert!((stats.utilization_percent() - 50.0).abs() < 0.01);
    }
}
