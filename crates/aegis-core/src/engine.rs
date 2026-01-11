//! Wasmtime engine wrapper for Aegis.
//!
//! This module provides the `AegisEngine` type, which wraps the Wasmtime engine
//! with Aegis-specific configuration and functionality.

use std::sync::Arc;

use parking_lot::RwLock;
use tracing::{debug, info};
use wasmtime::{Config, Engine};

use crate::config::EngineConfig;
use crate::error::EngineResult;

/// The core Aegis engine that wraps Wasmtime.
///
/// `AegisEngine` is responsible for:
/// - Configuring the underlying Wasmtime engine
/// - Managing epoch-based interruption
/// - Providing a shared engine instance for module compilation
///
/// # Example
///
/// ```
/// use aegis_core::{AegisEngine, EngineConfig};
///
/// let config = EngineConfig::default();
/// let engine = AegisEngine::new(config).unwrap();
/// ```
pub struct AegisEngine {
    /// The underlying Wasmtime engine.
    inner: Engine,
    /// Configuration used to create this engine.
    config: EngineConfig,
    /// Current epoch value for timeout management.
    epoch: RwLock<u64>,
}

impl AegisEngine {
    /// Create a new Aegis engine with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the Wasmtime engine cannot be created with
    /// the given configuration.
    pub fn new(config: EngineConfig) -> EngineResult<Self> {
        let mut wasmtime_config = Config::new();

        // Configure fuel-based CPU limiting
        wasmtime_config.consume_fuel(config.fuel_enabled);

        // Configure epoch-based interruption
        wasmtime_config.epoch_interruption(config.epoch_enabled);

        // Configure stack size
        wasmtime_config.max_wasm_stack(config.max_wasm_stack);

        // Configure async support
        wasmtime_config.async_support(config.async_support);

        // Configure Component Model
        wasmtime_config.wasm_component_model(config.component_model);

        // Configure debug info
        wasmtime_config.debug_info(config.debug_info);

        // Enable WASM features
        wasmtime_config.wasm_bulk_memory(true);
        wasmtime_config.wasm_multi_value(true);
        wasmtime_config.wasm_reference_types(true);
        wasmtime_config.wasm_simd(true);

        let inner = Engine::new(&wasmtime_config)?;

        info!(
            fuel = config.fuel_enabled,
            epochs = config.epoch_enabled,
            async_support = config.async_support,
            "Created Aegis engine"
        );

        Ok(Self {
            inner,
            config,
            epoch: RwLock::new(0),
        })
    }

    /// Create a new engine with default configuration.
    pub fn default_engine() -> EngineResult<Self> {
        Self::new(EngineConfig::default())
    }

    /// Get a reference to the underlying Wasmtime engine.
    pub fn inner(&self) -> &Engine {
        &self.inner
    }

    /// Get the configuration used to create this engine.
    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// Increment the epoch counter.
    ///
    /// This is used for epoch-based timeout management. Each increment
    /// advances the epoch, and stores configured with a deadline will
    /// trap when the epoch exceeds their deadline.
    pub fn increment_epoch(&self) {
        if self.config.epoch_enabled {
            let mut epoch = self.epoch.write();
            *epoch += 1;
            self.inner.increment_epoch();
            debug!(epoch = *epoch, "Incremented engine epoch");
        }
    }

    /// Get the current epoch value.
    pub fn current_epoch(&self) -> u64 {
        *self.epoch.read()
    }

    /// Check if fuel-based limiting is enabled.
    pub fn fuel_enabled(&self) -> bool {
        self.config.fuel_enabled
    }

    /// Check if epoch-based interruption is enabled.
    pub fn epoch_enabled(&self) -> bool {
        self.config.epoch_enabled
    }

    /// Check if async support is enabled.
    pub fn async_enabled(&self) -> bool {
        self.config.async_support
    }
}

impl std::fmt::Debug for AegisEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AegisEngine")
            .field("config", &self.config)
            .field("epoch", &*self.epoch.read())
            .finish()
    }
}

/// A shared reference to an Aegis engine.
///
/// This is the recommended way to share an engine across multiple sandboxes.
pub type SharedEngine = Arc<AegisEngine>;

/// Extension trait for creating shared engines.
pub trait IntoShared {
    /// Convert into a shared engine reference.
    fn into_shared(self) -> SharedEngine;
}

impl IntoShared for AegisEngine {
    fn into_shared(self) -> SharedEngine {
        Arc::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = AegisEngine::new(EngineConfig::default()).unwrap();
        assert!(engine.fuel_enabled());
        assert!(engine.epoch_enabled());
    }

    #[test]
    fn test_engine_epoch_increment() {
        let engine = AegisEngine::new(EngineConfig::default()).unwrap();
        assert_eq!(engine.current_epoch(), 0);

        engine.increment_epoch();
        assert_eq!(engine.current_epoch(), 1);

        engine.increment_epoch();
        assert_eq!(engine.current_epoch(), 2);
    }

    #[test]
    fn test_engine_without_epochs() {
        let config = EngineConfig::default().with_epochs(false);
        let engine = AegisEngine::new(config).unwrap();

        assert!(!engine.epoch_enabled());

        // Increment should be a no-op when epochs are disabled
        engine.increment_epoch();
        assert_eq!(engine.current_epoch(), 0);
    }

    #[test]
    fn test_shared_engine() {
        let engine = AegisEngine::new(EngineConfig::default())
            .unwrap()
            .into_shared();

        let engine2 = Arc::clone(&engine);
        engine.increment_epoch();

        // Both references should see the same epoch
        assert_eq!(engine.current_epoch(), 1);
        assert_eq!(engine2.current_epoch(), 1);
    }
}
