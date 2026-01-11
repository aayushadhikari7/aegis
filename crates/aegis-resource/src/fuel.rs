//! Fuel management for CPU limiting.
//!
//! Fuel provides a deterministic way to limit CPU usage in WebAssembly execution.
//! Each WASM instruction consumes a certain amount of fuel, and execution traps
//! when fuel is exhausted.

use std::sync::atomic::{AtomicU64, Ordering};

use tracing::{debug, info, warn};

use crate::error::{ResourceError, ResourceResult};

/// Configuration for fuel management.
#[derive(Debug, Clone)]
pub struct FuelConfig {
    /// Initial fuel allocation.
    pub initial_fuel: u64,
    /// Whether refueling is allowed during execution.
    pub allow_refuel: bool,
    /// Maximum amount of fuel that can be added via refuel.
    pub max_refuel: u64,
    /// Optional fuel reserve that triggers a warning callback.
    pub low_fuel_threshold: Option<u64>,
}

impl Default for FuelConfig {
    fn default() -> Self {
        Self {
            initial_fuel: 1_000_000_000, // 1 billion units
            allow_refuel: false,
            max_refuel: 0,
            low_fuel_threshold: None,
        }
    }
}

impl FuelConfig {
    /// Create a new fuel configuration.
    pub fn new(initial_fuel: u64) -> Self {
        Self {
            initial_fuel,
            ..Default::default()
        }
    }

    /// Allow refueling with the specified maximum amount.
    pub fn with_refuel(mut self, max_refuel: u64) -> Self {
        self.allow_refuel = true;
        self.max_refuel = max_refuel;
        self
    }

    /// Set a low fuel warning threshold.
    pub fn with_low_fuel_threshold(mut self, threshold: u64) -> Self {
        self.low_fuel_threshold = Some(threshold);
        self
    }

    /// Create a minimal fuel configuration for testing.
    pub fn minimal() -> Self {
        Self::new(10_000)
    }

    /// Create a standard fuel configuration.
    pub fn standard() -> Self {
        Self::default()
    }

    /// Create a generous fuel configuration for compute-intensive tasks.
    pub fn generous() -> Self {
        Self::new(10_000_000_000) // 10 billion units
    }
}

/// Callback type for low fuel warnings.
pub type LowFuelCallback = Box<dyn Fn(u64) + Send + Sync>;

/// Manages fuel consumption for CPU limiting.
///
/// `FuelManager` tracks fuel usage and provides methods for monitoring
/// and managing fuel during WASM execution.
pub struct FuelManager {
    /// Configuration.
    config: FuelConfig,
    /// Total fuel consumed across all executions.
    total_consumed: AtomicU64,
    /// Number of times fuel was exhausted.
    exhaustion_count: AtomicU64,
    /// Number of refuels performed.
    refuel_count: AtomicU64,
    /// Total fuel added via refuel.
    total_refueled: AtomicU64,
}

impl FuelManager {
    /// Create a new fuel manager with the given configuration.
    pub fn new(config: FuelConfig) -> Self {
        info!(
            initial_fuel = config.initial_fuel,
            allow_refuel = config.allow_refuel,
            "Created fuel manager"
        );

        Self {
            config,
            total_consumed: AtomicU64::new(0),
            exhaustion_count: AtomicU64::new(0),
            refuel_count: AtomicU64::new(0),
            total_refueled: AtomicU64::new(0),
        }
    }

    /// Create a fuel manager with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(FuelConfig::default())
    }

    /// Get the initial fuel allocation.
    pub fn initial_fuel(&self) -> u64 {
        self.config.initial_fuel
    }

    /// Check if refueling is allowed.
    pub fn refuel_allowed(&self) -> bool {
        self.config.allow_refuel
    }

    /// Get the maximum refuel amount.
    pub fn max_refuel(&self) -> u64 {
        self.config.max_refuel
    }

    /// Record fuel consumption.
    pub fn record_consumption(&self, consumed: u64) {
        self.total_consumed.fetch_add(consumed, Ordering::Relaxed);
        debug!(consumed, total = self.total_consumed(), "Recorded fuel consumption");
    }

    /// Record a fuel exhaustion event.
    pub fn record_exhaustion(&self) {
        self.exhaustion_count.fetch_add(1, Ordering::Relaxed);
        warn!(
            total_exhaustions = self.exhaustion_count(),
            "Fuel exhausted"
        );
    }

    /// Attempt to refuel.
    ///
    /// Returns the amount of fuel that can be added, or an error if refueling
    /// is not allowed.
    pub fn request_refuel(&self, requested: u64) -> ResourceResult<u64> {
        if !self.config.allow_refuel {
            return Err(ResourceError::RefuelDenied {
                reason: "Refueling is not allowed".to_string(),
            });
        }

        let amount = requested.min(self.config.max_refuel);
        self.refuel_count.fetch_add(1, Ordering::Relaxed);
        self.total_refueled.fetch_add(amount, Ordering::Relaxed);

        debug!(requested, granted = amount, "Refuel granted");

        Ok(amount)
    }

    /// Get total fuel consumed across all executions.
    pub fn total_consumed(&self) -> u64 {
        self.total_consumed.load(Ordering::Relaxed)
    }

    /// Get the number of fuel exhaustion events.
    pub fn exhaustion_count(&self) -> u64 {
        self.exhaustion_count.load(Ordering::Relaxed)
    }

    /// Get the number of refuel operations.
    pub fn refuel_count(&self) -> u64 {
        self.refuel_count.load(Ordering::Relaxed)
    }

    /// Get total fuel added via refueling.
    pub fn total_refueled(&self) -> u64 {
        self.total_refueled.load(Ordering::Relaxed)
    }

    /// Reset statistics.
    pub fn reset_stats(&self) {
        self.total_consumed.store(0, Ordering::Relaxed);
        self.exhaustion_count.store(0, Ordering::Relaxed);
        self.refuel_count.store(0, Ordering::Relaxed);
        self.total_refueled.store(0, Ordering::Relaxed);
    }

    /// Get a snapshot of fuel statistics.
    pub fn stats(&self) -> FuelStats {
        FuelStats {
            initial_fuel: self.config.initial_fuel,
            total_consumed: self.total_consumed(),
            exhaustion_count: self.exhaustion_count(),
            refuel_count: self.refuel_count(),
            total_refueled: self.total_refueled(),
        }
    }
}

impl std::fmt::Debug for FuelManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FuelManager")
            .field("config", &self.config)
            .field("total_consumed", &self.total_consumed())
            .field("exhaustion_count", &self.exhaustion_count())
            .finish()
    }
}

/// Statistics snapshot from a fuel manager.
#[derive(Debug, Clone)]
pub struct FuelStats {
    /// Initial fuel allocation.
    pub initial_fuel: u64,
    /// Total fuel consumed.
    pub total_consumed: u64,
    /// Number of exhaustion events.
    pub exhaustion_count: u64,
    /// Number of refuel operations.
    pub refuel_count: u64,
    /// Total fuel added via refueling.
    pub total_refueled: u64,
}

impl FuelStats {
    /// Calculate the effective fuel used (consumed - refueled).
    pub fn effective_consumed(&self) -> u64 {
        self.total_consumed.saturating_sub(self.total_refueled)
    }

    /// Calculate fuel efficiency as instructions per unit.
    /// (This is a placeholder - actual calculation would need instruction counts)
    pub fn had_exhaustions(&self) -> bool {
        self.exhaustion_count > 0
    }
}

/// Estimates for fuel costs of common operations.
///
/// These are approximate values and the actual fuel consumption depends on
/// the Wasmtime configuration.
#[derive(Debug, Clone, Copy)]
pub struct FuelCostEstimates {
    /// Cost per basic instruction (add, sub, etc.).
    pub per_instruction: u64,
    /// Cost per memory page allocation (64KB).
    pub per_memory_page: u64,
    /// Cost per host function call.
    pub per_host_call: u64,
    /// Cost per indirect call.
    pub per_indirect_call: u64,
}

impl Default for FuelCostEstimates {
    fn default() -> Self {
        Self {
            per_instruction: 1,
            per_memory_page: 1000,
            per_host_call: 100,
            per_indirect_call: 10,
        }
    }
}

impl FuelCostEstimates {
    /// Estimate the fuel needed for a number of instructions.
    pub fn estimate_instructions(&self, count: u64) -> u64 {
        count * self.per_instruction
    }

    /// Estimate the fuel needed for memory allocation.
    pub fn estimate_memory_pages(&self, pages: u64) -> u64 {
        pages * self.per_memory_page
    }

    /// Estimate the fuel needed for host calls.
    pub fn estimate_host_calls(&self, count: u64) -> u64 {
        count * self.per_host_call
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuel_config_creation() {
        let config = FuelConfig::new(1_000_000);
        assert_eq!(config.initial_fuel, 1_000_000);
        assert!(!config.allow_refuel);
    }

    #[test]
    fn test_fuel_config_with_refuel() {
        let config = FuelConfig::new(1_000_000).with_refuel(500_000);
        assert!(config.allow_refuel);
        assert_eq!(config.max_refuel, 500_000);
    }

    #[test]
    fn test_fuel_manager_creation() {
        let manager = FuelManager::new(FuelConfig::default());
        assert_eq!(manager.total_consumed(), 0);
        assert_eq!(manager.exhaustion_count(), 0);
    }

    #[test]
    fn test_fuel_consumption_tracking() {
        let manager = FuelManager::new(FuelConfig::default());

        manager.record_consumption(1000);
        manager.record_consumption(500);

        assert_eq!(manager.total_consumed(), 1500);
    }

    #[test]
    fn test_fuel_exhaustion_tracking() {
        let manager = FuelManager::new(FuelConfig::default());

        manager.record_exhaustion();
        manager.record_exhaustion();

        assert_eq!(manager.exhaustion_count(), 2);
    }

    #[test]
    fn test_refuel_denied_by_default() {
        let manager = FuelManager::new(FuelConfig::default());
        let result = manager.request_refuel(1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_refuel_allowed() {
        let config = FuelConfig::new(1_000_000).with_refuel(500_000);
        let manager = FuelManager::new(config);

        let amount = manager.request_refuel(1000).unwrap();
        assert_eq!(amount, 1000);
        assert_eq!(manager.refuel_count(), 1);
    }

    #[test]
    fn test_refuel_capped_at_max() {
        let config = FuelConfig::new(1_000_000).with_refuel(500);
        let manager = FuelManager::new(config);

        let amount = manager.request_refuel(1000).unwrap();
        assert_eq!(amount, 500); // Capped at max_refuel
    }

    #[test]
    fn test_stats() {
        let config = FuelConfig::new(1_000_000).with_refuel(500_000);
        let manager = FuelManager::new(config);

        manager.record_consumption(5000);
        manager.request_refuel(1000).unwrap();

        let stats = manager.stats();
        assert_eq!(stats.initial_fuel, 1_000_000);
        assert_eq!(stats.total_consumed, 5000);
        assert_eq!(stats.total_refueled, 1000);
        assert_eq!(stats.effective_consumed(), 4000);
    }

    #[test]
    fn test_fuel_cost_estimates() {
        let estimates = FuelCostEstimates::default();

        assert_eq!(estimates.estimate_instructions(1000), 1000);
        assert_eq!(estimates.estimate_memory_pages(10), 10_000);
        assert_eq!(estimates.estimate_host_calls(100), 10_000);
    }
}
