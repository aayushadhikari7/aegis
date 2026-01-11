//! # Aegis - WebAssembly Sandbox Runtime
//!
//! Aegis is a local-first runtime that allows users and applications to execute
//! untrusted WebAssembly code safely within a tightly controlled sandbox.
//!
//! ## Features
//!
//! - **Security**: Capability-based security with no ambient authority
//! - **Resource Control**: Memory limits, CPU limits (fuel), and timeouts
//! - **Observability**: Metrics collection and event subscription
//! - **Embeddable**: Library-first design for easy integration
//!
//! ## Quick Start
//!
//! ```ignore
//! use aegis::prelude::*;
//!
//! // Create a runtime
//! let runtime = Aegis::builder()
//!     .with_memory_limit(64 * 1024 * 1024)  // 64MB
//!     .with_fuel_limit(1_000_000_000)        // 1B fuel units
//!     .with_timeout(Duration::from_secs(30))
//!     .build()?;
//!
//! // Load a module
//! let module = runtime.load_file("plugin.wasm")?;
//!
//! // Execute in a sandbox
//! let mut sandbox = runtime.sandbox().build()?;
//! sandbox.load_module(&module)?;
//!
//! let result: i32 = sandbox.call("add", (2i32, 3i32))?;
//! assert_eq!(result, 5);
//! ```
//!
//! ## Security Model
//!
//! Aegis follows the principle of least privilege:
//!
//! 1. **No Ambient Authority**: All permissions must be explicitly granted
//! 2. **Capability-Based**: Each capability explicitly defines allowed actions
//! 3. **Resource Limits**: Memory, CPU, and time are bounded
//! 4. **Isolation**: Each sandbox runs in its own isolated environment
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    Your Application                     │
//! ├─────────────────────────────────────────────────────────┤
//! │                      aegis (facade)                     │
//! │                    ┌─────────────────┐                  │
//! │                    │  Aegis Builder  │                  │
//! │                    └────────┬────────┘                  │
//! │                             │                           │
//! │  ┌──────────────┬──────────┴───────┬───────────────┐   │
//! │  │ aegis-core   │ aegis-capability │ aegis-observe │   │
//! │  │ (engine,     │ (permissions)    │ (metrics,     │   │
//! │  │  sandbox)    │                  │  events)      │   │
//! │  └──────────────┴──────────────────┴───────────────┘   │
//! ├─────────────────────────────────────────────────────────┤
//! │                       Wasmtime                          │
//! └─────────────────────────────────────────────────────────┘
//! ```

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use aegis_capability::{
    CapabilitySet, CapabilitySetBuilder, ClockCapability, FilesystemCapability, LoggingCapability,
    NetworkCapability,
};
use aegis_core::{
    AegisEngine, EngineConfig, ExecutionError, ModuleLoader, ResourceLimits, Sandbox,
    SandboxConfig, SharedEngine, ValidatedModule,
};
use aegis_observe::{EventDispatcher, EventSubscriber};

// Re-export from sub-crates
pub use aegis_capability;
pub use aegis_core;
pub use aegis_host;
pub use aegis_observe;
pub use aegis_resource;

/// Main entry point for Aegis.
pub struct Aegis;

impl Aegis {
    /// Create a new Aegis runtime builder.
    pub fn builder() -> AegisBuilder {
        AegisBuilder::new()
    }

    /// Create a runtime with default configuration.
    pub fn with_defaults() -> Result<AegisRuntime, AegisError> {
        AegisBuilder::new().build()
    }
}

/// Builder for configuring the Aegis runtime.
pub struct AegisBuilder {
    engine_config: EngineConfig,
    resource_limits: ResourceLimits,
    capabilities: CapabilitySetBuilder,
    event_subscribers: Vec<Arc<dyn EventSubscriber>>,
}

impl AegisBuilder {
    /// Create a new builder with default configuration.
    pub fn new() -> Self {
        Self {
            engine_config: EngineConfig::default(),
            resource_limits: ResourceLimits::default(),
            capabilities: CapabilitySetBuilder::new(),
            event_subscribers: Vec::new(),
        }
    }

    // Engine configuration

    /// Enable or disable async execution support.
    pub fn with_async_support(mut self, enabled: bool) -> Self {
        self.engine_config.async_support = enabled;
        self
    }

    /// Enable or disable the Component Model.
    pub fn with_component_model(mut self, enabled: bool) -> Self {
        self.engine_config.component_model = enabled;
        self
    }

    /// Enable or disable debug info.
    pub fn with_debug_info(mut self, enabled: bool) -> Self {
        self.engine_config.debug_info = enabled;
        self
    }

    // Resource limits

    /// Set the maximum memory limit in bytes.
    pub fn with_memory_limit(mut self, bytes: usize) -> Self {
        self.resource_limits.max_memory_bytes = bytes;
        self
    }

    /// Set the initial fuel limit.
    pub fn with_fuel_limit(mut self, fuel: u64) -> Self {
        self.resource_limits.initial_fuel = fuel;
        self
    }

    /// Set the execution timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.resource_limits.timeout = timeout;
        self
    }

    /// Set custom resource limits.
    pub fn with_resource_limits(mut self, limits: ResourceLimits) -> Self {
        self.resource_limits = limits;
        self
    }

    // Capabilities

    /// Add the filesystem capability.
    pub fn with_filesystem(mut self, config: FilesystemCapability) -> Self {
        self.capabilities = self.capabilities.with(config);
        self
    }

    /// Add the network capability.
    pub fn with_network(mut self, config: NetworkCapability) -> Self {
        self.capabilities = self.capabilities.with(config);
        self
    }

    /// Add the logging capability.
    pub fn with_logging(mut self, config: LoggingCapability) -> Self {
        self.capabilities = self.capabilities.with(config);
        self
    }

    /// Add the clock capability.
    pub fn with_clock(mut self, config: ClockCapability) -> Self {
        self.capabilities = self.capabilities.with(config);
        self
    }

    /// Add a custom capability.
    pub fn with_capability<C: aegis_capability::Capability + 'static>(mut self, cap: C) -> Self {
        self.capabilities = self.capabilities.with(cap);
        self
    }

    // Observability

    /// Add an event subscriber.
    pub fn with_event_subscriber(mut self, subscriber: Arc<dyn EventSubscriber>) -> Self {
        self.event_subscribers.push(subscriber);
        self
    }

    /// Build the runtime.
    pub fn build(self) -> Result<AegisRuntime, AegisError> {
        let engine = AegisEngine::new(self.engine_config).map_err(AegisError::Engine)?;
        let shared_engine = Arc::new(engine);

        let capabilities = self.capabilities.build().map_err(AegisError::Capability)?;

        let event_dispatcher = EventDispatcher::new();
        for subscriber in self.event_subscribers {
            event_dispatcher.subscribe(subscriber);
        }

        Ok(AegisRuntime {
            engine: shared_engine,
            default_limits: self.resource_limits,
            default_capabilities: Arc::new(capabilities),
            event_dispatcher: Arc::new(event_dispatcher),
        })
    }
}

impl Default for AegisBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A configured Aegis runtime.
pub struct AegisRuntime {
    engine: SharedEngine,
    default_limits: ResourceLimits,
    default_capabilities: Arc<CapabilitySet>,
    event_dispatcher: Arc<EventDispatcher>,
}

impl AegisRuntime {
    /// Get a reference to the engine.
    pub fn engine(&self) -> &SharedEngine {
        &self.engine
    }

    /// Get the default resource limits.
    pub fn default_limits(&self) -> &ResourceLimits {
        &self.default_limits
    }

    /// Get the default capabilities.
    pub fn default_capabilities(&self) -> &Arc<CapabilitySet> {
        &self.default_capabilities
    }

    /// Get the event dispatcher.
    pub fn event_dispatcher(&self) -> &Arc<EventDispatcher> {
        &self.event_dispatcher
    }

    /// Create a module loader.
    pub fn loader(&self) -> ModuleLoader {
        ModuleLoader::new(Arc::clone(&self.engine))
    }

    /// Load a module from bytes.
    pub fn load_bytes(&self, bytes: &[u8]) -> Result<ValidatedModule, AegisError> {
        self.loader().load_bytes(bytes).map_err(AegisError::Module)
    }

    /// Load a module from a file.
    pub fn load_file(&self, path: impl AsRef<Path>) -> Result<ValidatedModule, AegisError> {
        self.loader()
            .load_file(path.as_ref())
            .map_err(AegisError::Module)
    }

    /// Load a module from WAT text format.
    pub fn load_wat(&self, wat: &str) -> Result<ValidatedModule, AegisError> {
        self.loader().load_wat(wat).map_err(AegisError::Module)
    }

    /// Create a sandbox builder with default configuration.
    pub fn sandbox(&self) -> RuntimeSandboxBuilder<'_> {
        RuntimeSandboxBuilder::new(self)
    }

    /// Execute a module quickly with default settings.
    ///
    /// This is a convenience method for simple use cases.
    pub fn execute<R: wasmtime::WasmResults>(
        &self,
        module: &ValidatedModule,
        function: &str,
    ) -> Result<R, AegisError> {
        let mut sandbox = self.sandbox().build()?;
        sandbox.load_module(module).map_err(AegisError::Execution)?;
        sandbox.call(function, ()).map_err(AegisError::Execution)
    }
}

impl std::fmt::Debug for AegisRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AegisRuntime")
            .field("default_limits", &self.default_limits)
            .finish()
    }
}

/// Builder for creating sandboxes from a runtime.
pub struct RuntimeSandboxBuilder<'a> {
    runtime: &'a AegisRuntime,
    limits: Option<ResourceLimits>,
    capabilities: Option<Arc<CapabilitySet>>,
}

impl<'a> RuntimeSandboxBuilder<'a> {
    fn new(runtime: &'a AegisRuntime) -> Self {
        Self {
            runtime,
            limits: None,
            capabilities: None,
        }
    }

    /// Override resource limits.
    pub fn with_limits(mut self, limits: ResourceLimits) -> Self {
        self.limits = Some(limits);
        self
    }

    /// Override memory limit.
    pub fn with_memory_limit(mut self, bytes: usize) -> Self {
        let mut limits = self
            .limits
            .take()
            .unwrap_or_else(|| self.runtime.default_limits.clone());
        limits.max_memory_bytes = bytes;
        self.limits = Some(limits);
        self
    }

    /// Override fuel limit.
    pub fn with_fuel_limit(mut self, fuel: u64) -> Self {
        let mut limits = self
            .limits
            .take()
            .unwrap_or_else(|| self.runtime.default_limits.clone());
        limits.initial_fuel = fuel;
        self.limits = Some(limits);
        self
    }

    /// Override timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        let mut limits = self
            .limits
            .take()
            .unwrap_or_else(|| self.runtime.default_limits.clone());
        limits.timeout = timeout;
        self.limits = Some(limits);
        self
    }

    /// Override capabilities.
    pub fn with_capabilities(mut self, capabilities: Arc<CapabilitySet>) -> Self {
        self.capabilities = Some(capabilities);
        self
    }

    /// Build the sandbox.
    pub fn build(self) -> Result<Sandbox<()>, AegisError> {
        let limits = self
            .limits
            .unwrap_or_else(|| self.runtime.default_limits.clone());
        let config = SandboxConfig::default().with_limits(limits);

        Sandbox::new(Arc::clone(&self.runtime.engine), (), config).map_err(AegisError::Execution)
    }

    /// Build the sandbox with custom state.
    pub fn build_with_state<S: Send + 'static>(self, state: S) -> Result<Sandbox<S>, AegisError> {
        let limits = self
            .limits
            .unwrap_or_else(|| self.runtime.default_limits.clone());
        let config = SandboxConfig::default().with_limits(limits);

        Sandbox::new(Arc::clone(&self.runtime.engine), state, config).map_err(AegisError::Execution)
    }
}

/// Errors from the Aegis runtime.
#[derive(Debug, thiserror::Error)]
pub enum AegisError {
    /// Engine error.
    #[error("Engine error: {0}")]
    Engine(#[from] aegis_core::EngineError),

    /// Module error.
    #[error("Module error: {0}")]
    Module(#[from] aegis_core::ModuleError),

    /// Execution error.
    #[error("Execution error: {0}")]
    Execution(#[from] ExecutionError),

    /// Capability error.
    #[error("Capability error: {0}")]
    Capability(#[from] aegis_capability::CapabilityError),
}

/// Prelude module for convenient imports.
pub mod prelude {
    // Main types
    pub use crate::{Aegis, AegisBuilder, AegisError, AegisRuntime};

    // Core types
    pub use aegis_core::{
        AegisEngine, EngineConfig, ModuleLoader, ResourceLimits, Sandbox, SandboxBuilder,
        SandboxConfig, ValidatedModule,
    };

    // Capability types
    pub use aegis_capability::{
        Capability, CapabilityId, CapabilitySet, ClockCapability, FilesystemCapability,
        LoggingCapability, NetworkCapability, PathPermission, PermissionResult,
    };

    // Resource types
    pub use aegis_resource::{EpochConfig, EpochManager, FuelConfig, FuelManager};

    // Observability types
    pub use aegis_observe::{
        EventDispatcher, EventSubscriber, ExecutionOutcome, ExecutionReport, MetricsCollector,
        SandboxEvent,
    };

    // Common std types
    pub use std::sync::Arc;
    pub use std::time::Duration;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aegis_builder() {
        let runtime = Aegis::builder()
            .with_memory_limit(32 * 1024 * 1024)
            .with_fuel_limit(100_000)
            .with_timeout(Duration::from_secs(5))
            .build()
            .unwrap();

        assert_eq!(runtime.default_limits().max_memory_bytes, 32 * 1024 * 1024);
        assert_eq!(runtime.default_limits().initial_fuel, 100_000);
    }

    #[test]
    fn test_load_and_execute() {
        let runtime = Aegis::builder().build().unwrap();

        let module = runtime
            .load_wat(
                r#"
            (module
                (func (export "answer") (result i32)
                    i32.const 42
                )
            )
        "#,
            )
            .unwrap();

        let mut sandbox = runtime.sandbox().build().unwrap();
        sandbox.load_module(&module).unwrap();

        let result: i32 = sandbox.call("answer", ()).unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_sandbox_builder_overrides() {
        let runtime = Aegis::builder().with_fuel_limit(1_000_000).build().unwrap();

        let sandbox = runtime
            .sandbox()
            .with_fuel_limit(500_000)
            .with_memory_limit(16 * 1024 * 1024)
            .build()
            .unwrap();

        // Verify overrides were applied
        assert_eq!(sandbox.remaining_fuel(), Some(500_000));
    }

    #[test]
    fn test_prelude_imports() {
        use crate::prelude::*;

        let _runtime = Aegis::builder().build().unwrap();
    }
}
