//! Configuration types for the Aegis runtime.
//!
//! This module provides configuration structures for customizing the behavior
//! of the Aegis engine and sandbox execution.

use std::time::Duration;

/// Configuration for the Aegis engine.
///
/// This controls how the underlying Wasmtime engine is configured.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Enable fuel-based CPU limiting.
    ///
    /// When enabled, WASM execution consumes "fuel" and will trap when
    /// fuel is exhausted. This provides deterministic CPU limiting.
    pub fuel_enabled: bool,

    /// Enable epoch-based interruption.
    ///
    /// When enabled, execution can be interrupted based on epoch deadlines.
    /// This provides wall-clock timeout support.
    pub epoch_enabled: bool,

    /// Maximum WASM stack size in bytes.
    ///
    /// Defaults to 1MB.
    pub max_wasm_stack: usize,

    /// Enable async execution support.
    ///
    /// When enabled, the engine supports async host functions and
    /// cooperative yielding during execution.
    pub async_support: bool,

    /// Enable the WebAssembly Component Model.
    ///
    /// This allows loading and executing WASM components in addition
    /// to core modules.
    pub component_model: bool,

    /// Enable debug information in compiled code.
    ///
    /// This increases compilation time and memory usage but provides
    /// better error messages and backtraces.
    pub debug_info: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            fuel_enabled: true,
            epoch_enabled: true,
            max_wasm_stack: 1024 * 1024, // 1MB
            async_support: false,
            component_model: false,
            debug_info: false,
        }
    }
}

impl EngineConfig {
    /// Create a new engine configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable fuel-based CPU limiting.
    pub fn with_fuel(mut self, enabled: bool) -> Self {
        self.fuel_enabled = enabled;
        self
    }

    /// Enable epoch-based interruption.
    pub fn with_epochs(mut self, enabled: bool) -> Self {
        self.epoch_enabled = enabled;
        self
    }

    /// Set the maximum WASM stack size.
    pub fn with_max_wasm_stack(mut self, bytes: usize) -> Self {
        self.max_wasm_stack = bytes;
        self
    }

    /// Enable async execution support.
    pub fn with_async(mut self, enabled: bool) -> Self {
        self.async_support = enabled;
        self
    }

    /// Enable the Component Model.
    pub fn with_component_model(mut self, enabled: bool) -> Self {
        self.component_model = enabled;
        self
    }

    /// Enable debug information.
    pub fn with_debug_info(mut self, enabled: bool) -> Self {
        self.debug_info = enabled;
        self
    }

    /// Create a configuration optimized for security.
    ///
    /// This enables all safety features and uses conservative limits.
    pub fn secure() -> Self {
        Self {
            fuel_enabled: true,
            epoch_enabled: true,
            max_wasm_stack: 512 * 1024, // 512KB
            async_support: false,
            component_model: false,
            debug_info: false,
        }
    }

    /// Create a configuration optimized for performance.
    ///
    /// This relaxes some limits for better throughput.
    pub fn performance() -> Self {
        Self {
            fuel_enabled: false,
            epoch_enabled: true,
            max_wasm_stack: 2 * 1024 * 1024, // 2MB
            async_support: false,
            component_model: false,
            debug_info: false,
        }
    }
}

/// Configuration for sandbox execution.
///
/// This controls resource limits and behavior for individual sandbox instances.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Resource limits for this sandbox.
    pub limits: ResourceLimits,

    /// Whether to collect detailed metrics during execution.
    pub collect_metrics: bool,

    /// Whether to allow the sandbox to be reused after execution.
    pub reusable: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            limits: ResourceLimits::default(),
            collect_metrics: true,
            reusable: false,
        }
    }
}

impl SandboxConfig {
    /// Create a new sandbox configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set resource limits.
    pub fn with_limits(mut self, limits: ResourceLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Enable or disable metrics collection.
    pub fn with_metrics(mut self, enabled: bool) -> Self {
        self.collect_metrics = enabled;
        self
    }

    /// Enable or disable sandbox reuse.
    pub fn with_reusable(mut self, enabled: bool) -> Self {
        self.reusable = enabled;
        self
    }
}

/// Resource limits for sandbox execution.
///
/// These limits control memory, CPU, and time constraints for WASM execution.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum memory in bytes.
    ///
    /// Defaults to 64MB.
    pub max_memory_bytes: usize,

    /// Maximum number of memory instances.
    ///
    /// Defaults to 1.
    pub max_memories: u32,

    /// Maximum table elements.
    ///
    /// Defaults to 10,000.
    pub max_table_elements: u32,

    /// Initial fuel allocation.
    ///
    /// Defaults to 1 billion units.
    pub initial_fuel: u64,

    /// Maximum execution timeout.
    ///
    /// Defaults to 30 seconds.
    pub timeout: Duration,

    /// Maximum WASM stack size in bytes.
    ///
    /// This is typically inherited from EngineConfig but can be
    /// overridden per-sandbox.
    pub max_stack: Option<usize>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 64 * 1024 * 1024, // 64MB
            max_memories: 1,
            max_table_elements: 10_000,
            initial_fuel: 1_000_000_000,
            timeout: Duration::from_secs(30),
            max_stack: None,
        }
    }
}

impl ResourceLimits {
    /// Create resource limits with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum memory limit.
    pub fn with_max_memory(mut self, bytes: usize) -> Self {
        self.max_memory_bytes = bytes;
        self
    }

    /// Set the initial fuel allocation.
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.initial_fuel = fuel;
        self
    }

    /// Set the execution timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the maximum stack size.
    pub fn with_max_stack(mut self, bytes: usize) -> Self {
        self.max_stack = Some(bytes);
        self
    }

    /// Create minimal resource limits for testing.
    pub fn minimal() -> Self {
        Self {
            max_memory_bytes: 1024 * 1024, // 1MB
            max_memories: 1,
            max_table_elements: 1_000,
            initial_fuel: 10_000,
            timeout: Duration::from_secs(1),
            max_stack: Some(256 * 1024),
        }
    }

    /// Create standard resource limits for typical workloads.
    pub fn standard() -> Self {
        Self::default()
    }

    /// Create generous resource limits for compute-intensive workloads.
    pub fn generous() -> Self {
        Self {
            max_memory_bytes: 256 * 1024 * 1024, // 256MB
            max_memories: 4,
            max_table_elements: 100_000,
            initial_fuel: 10_000_000_000,
            timeout: Duration::from_secs(300),
            max_stack: Some(4 * 1024 * 1024),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_config_defaults() {
        let config = EngineConfig::default();
        assert!(config.fuel_enabled);
        assert!(config.epoch_enabled);
        assert_eq!(config.max_wasm_stack, 1024 * 1024);
        assert!(!config.async_support);
    }

    #[test]
    fn test_engine_config_builder() {
        let config = EngineConfig::new()
            .with_fuel(false)
            .with_async(true)
            .with_max_wasm_stack(2 * 1024 * 1024);

        assert!(!config.fuel_enabled);
        assert!(config.async_support);
        assert_eq!(config.max_wasm_stack, 2 * 1024 * 1024);
    }

    #[test]
    fn test_resource_limits_presets() {
        let minimal = ResourceLimits::minimal();
        let standard = ResourceLimits::standard();
        let generous = ResourceLimits::generous();

        assert!(minimal.max_memory_bytes < standard.max_memory_bytes);
        assert!(standard.max_memory_bytes < generous.max_memory_bytes);
        assert!(minimal.initial_fuel < standard.initial_fuel);
    }
}
