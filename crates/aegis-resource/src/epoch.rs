//! Epoch-based timeout management.
//!
//! Epochs provide a wall-clock timeout mechanism for WASM execution.
//! The engine periodically increments an epoch counter, and stores can
//! be configured with a deadline that causes execution to trap when exceeded.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tracing::{info, warn};

use aegis_core::engine::SharedEngine;
use crate::error::{ResourceError, ResourceResult};

/// Configuration for epoch-based timeout management.
#[derive(Debug, Clone)]
pub struct EpochConfig {
    /// Interval between epoch increments.
    ///
    /// Smaller intervals provide finer-grained timeout control but
    /// incur more overhead.
    pub tick_interval: Duration,
    /// Default timeout for executions.
    pub default_timeout: Duration,
    /// Whether to start the epoch incrementer automatically.
    pub auto_start: bool,
}

impl Default for EpochConfig {
    fn default() -> Self {
        Self {
            tick_interval: Duration::from_millis(10),
            default_timeout: Duration::from_secs(30),
            auto_start: true,
        }
    }
}

impl EpochConfig {
    /// Create a new epoch configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the tick interval.
    pub fn with_tick_interval(mut self, interval: Duration) -> Self {
        self.tick_interval = interval;
        self
    }

    /// Set the default timeout.
    pub fn with_default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Configure auto-start behavior.
    pub fn with_auto_start(mut self, auto_start: bool) -> Self {
        self.auto_start = auto_start;
        self
    }

    /// Calculate the number of epochs for a given duration.
    pub fn epochs_for_duration(&self, duration: Duration) -> u64 {
        let ticks = duration.as_nanos() / self.tick_interval.as_nanos();
        ticks.max(1) as u64
    }
}

/// Manages epoch-based execution timeouts.
///
/// `EpochManager` runs a background thread that periodically increments
/// the engine's epoch counter. This allows stores to be configured with
/// deadline-based interruption.
///
/// # Example
///
/// ```ignore
/// use aegis_resource::epoch::{EpochManager, EpochConfig};
///
/// let engine = create_engine();
/// let manager = EpochManager::new(engine, EpochConfig::default())?;
///
/// // The manager will automatically increment epochs in the background
/// // Stores configured with epoch deadlines will trap when exceeded
/// ```
pub struct EpochManager {
    /// Reference to the engine.
    engine: SharedEngine,
    /// Configuration.
    config: EpochConfig,
    /// Shutdown signal.
    shutdown: Arc<AtomicBool>,
    /// Handle to the incrementer thread.
    thread_handle: Mutex<Option<JoinHandle<()>>>,
    /// Whether the manager is running.
    running: AtomicBool,
    /// Total epochs incremented.
    total_epochs: AtomicU64,
    /// Number of timeout events detected.
    timeout_count: AtomicU64,
}

impl EpochManager {
    /// Create a new epoch manager.
    ///
    /// If `auto_start` is enabled in the config, the background incrementer
    /// thread will be started automatically.
    pub fn new(engine: SharedEngine, config: EpochConfig) -> ResourceResult<Self> {
        if !engine.epoch_enabled() {
            return Err(ResourceError::EpochsDisabled);
        }

        let manager = Self {
            engine,
            config: config.clone(),
            shutdown: Arc::new(AtomicBool::new(false)),
            thread_handle: Mutex::new(None),
            running: AtomicBool::new(false),
            total_epochs: AtomicU64::new(0),
            timeout_count: AtomicU64::new(0),
        };

        if config.auto_start {
            manager.start()?;
        }

        Ok(manager)
    }

    /// Start the epoch incrementer thread.
    pub fn start(&self) -> ResourceResult<()> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Ok(()); // Already running
        }

        let engine = Arc::clone(&self.engine);
        let shutdown = Arc::clone(&self.shutdown);
        let tick_interval = self.config.tick_interval;
        let total_epochs = &self.total_epochs as *const AtomicU64 as usize;

        // Safety: We ensure the EpochManager outlives the thread by joining in drop
        let handle = thread::Builder::new()
            .name("aegis-epoch-incrementer".to_string())
            .spawn(move || {
                info!(
                    tick_interval_ms = tick_interval.as_millis(),
                    "Epoch incrementer thread started"
                );

                while !shutdown.load(Ordering::Relaxed) {
                    thread::sleep(tick_interval);
                    engine.increment_epoch();

                    // Update counter (safe because we ensure thread doesn't outlive manager)
                    unsafe {
                        let counter = &*(total_epochs as *const AtomicU64);
                        counter.fetch_add(1, Ordering::Relaxed);
                    }
                }

                info!("Epoch incrementer thread stopped");
            })
            .map_err(|e| ResourceError::ThreadSpawnFailed(e.to_string()))?;

        *self.thread_handle.lock() = Some(handle);

        info!(
            tick_interval_ms = self.config.tick_interval.as_millis(),
            "Started epoch incrementer"
        );

        Ok(())
    }

    /// Stop the epoch incrementer thread.
    pub fn stop(&self) {
        if !self.running.swap(false, Ordering::SeqCst) {
            return; // Not running
        }

        self.shutdown.store(true, Ordering::SeqCst);

        if let Some(handle) = self.thread_handle.lock().take() {
            if let Err(e) = handle.join() {
                warn!("Failed to join epoch incrementer thread: {:?}", e);
            }
        }

        // Reset shutdown flag for potential restart
        self.shutdown.store(false, Ordering::SeqCst);

        info!("Stopped epoch incrementer");
    }

    /// Check if the manager is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get the tick interval.
    pub fn tick_interval(&self) -> Duration {
        self.config.tick_interval
    }

    /// Get the default timeout.
    pub fn default_timeout(&self) -> Duration {
        self.config.default_timeout
    }

    /// Calculate the epoch deadline for a given timeout duration.
    ///
    /// Returns the number of epochs from now that corresponds to the timeout.
    pub fn deadline_for_timeout(&self, timeout: Duration) -> u64 {
        let current = self.engine.current_epoch();
        let epochs = self.config.epochs_for_duration(timeout);
        current + epochs
    }

    /// Record a timeout event.
    pub fn record_timeout(&self) {
        self.timeout_count.fetch_add(1, Ordering::Relaxed);
        warn!(total_timeouts = self.timeout_count(), "Execution timeout occurred");
    }

    /// Get the total number of epochs incremented.
    pub fn total_epochs(&self) -> u64 {
        self.total_epochs.load(Ordering::Relaxed)
    }

    /// Get the number of timeout events.
    pub fn timeout_count(&self) -> u64 {
        self.timeout_count.load(Ordering::Relaxed)
    }

    /// Get the current engine epoch.
    pub fn current_epoch(&self) -> u64 {
        self.engine.current_epoch()
    }

    /// Manually increment the epoch.
    ///
    /// This is useful for testing or when not using the background thread.
    pub fn increment(&self) {
        self.engine.increment_epoch();
        self.total_epochs.fetch_add(1, Ordering::Relaxed);
    }

    /// Get a snapshot of epoch statistics.
    pub fn stats(&self) -> EpochStats {
        EpochStats {
            current_epoch: self.current_epoch(),
            total_epochs: self.total_epochs(),
            timeout_count: self.timeout_count(),
            is_running: self.is_running(),
            tick_interval: self.config.tick_interval,
        }
    }
}

impl Drop for EpochManager {
    fn drop(&mut self) {
        self.stop();
    }
}

impl std::fmt::Debug for EpochManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EpochManager")
            .field("config", &self.config)
            .field("running", &self.is_running())
            .field("current_epoch", &self.current_epoch())
            .field("total_epochs", &self.total_epochs())
            .finish()
    }
}

/// Statistics snapshot from an epoch manager.
#[derive(Debug, Clone)]
pub struct EpochStats {
    /// Current engine epoch.
    pub current_epoch: u64,
    /// Total epochs incremented by this manager.
    pub total_epochs: u64,
    /// Number of timeout events.
    pub timeout_count: u64,
    /// Whether the incrementer is running.
    pub is_running: bool,
    /// Tick interval.
    pub tick_interval: Duration,
}

impl EpochStats {
    /// Estimate the elapsed time based on epochs and tick interval.
    pub fn estimated_elapsed(&self) -> Duration {
        let nanos = self.total_epochs as u128 * self.tick_interval.as_nanos();
        Duration::from_nanos(nanos as u64)
    }
}

/// A guard that ensures execution completes within a timeout.
///
/// When created, it calculates the epoch deadline. The caller is responsible
/// for configuring the store with this deadline.
#[derive(Debug)]
pub struct TimeoutGuard {
    /// The epoch deadline.
    pub deadline: u64,
    /// When the guard was created.
    pub created_at: Instant,
    /// The timeout duration.
    pub timeout: Duration,
}

impl TimeoutGuard {
    /// Create a new timeout guard.
    pub fn new(manager: &EpochManager, timeout: Duration) -> Self {
        Self {
            deadline: manager.deadline_for_timeout(timeout),
            created_at: Instant::now(),
            timeout,
        }
    }

    /// Check if the timeout has elapsed based on wall clock.
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= self.timeout
    }

    /// Get the remaining time.
    pub fn remaining(&self) -> Duration {
        self.timeout.saturating_sub(self.created_at.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aegis_core::{AegisEngine, EngineConfig, IntoShared};

    fn create_engine() -> SharedEngine {
        AegisEngine::new(EngineConfig::default().with_epochs(true))
            .unwrap()
            .into_shared()
    }

    #[test]
    fn test_epoch_config() {
        let config = EpochConfig::new()
            .with_tick_interval(Duration::from_millis(5))
            .with_default_timeout(Duration::from_secs(10));

        assert_eq!(config.tick_interval, Duration::from_millis(5));
        assert_eq!(config.default_timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_epochs_for_duration() {
        let config = EpochConfig::new().with_tick_interval(Duration::from_millis(10));

        assert_eq!(config.epochs_for_duration(Duration::from_millis(100)), 10);
        assert_eq!(config.epochs_for_duration(Duration::from_secs(1)), 100);
    }

    #[test]
    fn test_epoch_manager_creation() {
        let engine = create_engine();
        let config = EpochConfig::new().with_auto_start(false);
        let manager = EpochManager::new(engine, config).unwrap();

        assert!(!manager.is_running());
        assert_eq!(manager.total_epochs(), 0);
    }

    #[test]
    fn test_epoch_manager_manual_increment() {
        let engine = create_engine();
        let config = EpochConfig::new().with_auto_start(false);
        let manager = EpochManager::new(engine, config).unwrap();

        let initial = manager.current_epoch();
        manager.increment();
        manager.increment();

        assert_eq!(manager.current_epoch(), initial + 2);
        assert_eq!(manager.total_epochs(), 2);
    }

    #[test]
    fn test_deadline_calculation() {
        let engine = create_engine();
        let config = EpochConfig::new()
            .with_tick_interval(Duration::from_millis(10))
            .with_auto_start(false);
        let manager = EpochManager::new(engine, config).unwrap();

        let deadline = manager.deadline_for_timeout(Duration::from_millis(100));
        assert_eq!(deadline, 10); // 100ms / 10ms per tick = 10 epochs
    }

    #[test]
    fn test_timeout_guard() {
        let engine = create_engine();
        let config = EpochConfig::new()
            .with_tick_interval(Duration::from_millis(10))
            .with_auto_start(false);
        let manager = EpochManager::new(engine, config).unwrap();

        let guard = TimeoutGuard::new(&manager, Duration::from_secs(1));
        assert!(!guard.is_expired());
        assert!(guard.remaining() <= Duration::from_secs(1));
    }

    #[test]
    fn test_epoch_manager_start_stop() {
        let engine = create_engine();
        let config = EpochConfig::new()
            .with_tick_interval(Duration::from_millis(1))
            .with_auto_start(false);
        let manager = EpochManager::new(engine, config).unwrap();

        assert!(!manager.is_running());

        manager.start().unwrap();
        assert!(manager.is_running());

        // Let it run for a bit
        thread::sleep(Duration::from_millis(50));
        assert!(manager.total_epochs() > 0);

        manager.stop();
        assert!(!manager.is_running());
    }

    #[test]
    fn test_epoch_manager_auto_start() {
        let engine = create_engine();
        let config = EpochConfig::new()
            .with_tick_interval(Duration::from_millis(10))
            .with_auto_start(true);
        let manager = EpochManager::new(engine, config).unwrap();

        // Auto-start should have started the manager
        assert!(manager.is_running());

        // Stop cleanly
        manager.stop();
        assert!(!manager.is_running());
    }

    #[test]
    fn test_epochs_disabled_error() {
        let engine = AegisEngine::new(EngineConfig::default().with_epochs(false))
            .unwrap()
            .into_shared();

        let result = EpochManager::new(engine, EpochConfig::default());
        assert!(result.is_err());
    }
}
