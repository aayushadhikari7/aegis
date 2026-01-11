//! Sandbox execution environment.
//!
//! This module provides the `Sandbox` type, which represents an isolated
//! execution environment for running WebAssembly modules.

use std::time::{Duration, Instant};

use tracing::{debug, info, warn};
use uuid::Uuid;
use wasmtime::{Instance, Linker, Store, StoreLimits, StoreLimitsBuilder};

use crate::config::{ResourceLimits, SandboxConfig};
use crate::engine::SharedEngine;
use crate::error::{ExecutionError, ExecutionResult, TrapInfo};
use crate::module::ValidatedModule;

/// Unique identifier for a sandbox instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SandboxId(Uuid);

impl SandboxId {
    /// Create a new random sandbox ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SandboxId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SandboxId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Internal data stored in the Wasmtime Store.
pub struct SandboxData<S = ()> {
    /// Unique identifier for this sandbox.
    pub id: SandboxId,
    /// User-provided state.
    pub user_state: S,
    /// Resource limits.
    pub limits: StoreLimits,
    /// Execution metrics.
    pub metrics: SandboxMetrics,
    /// Configuration.
    config: SandboxConfig,
}

impl<S> SandboxData<S> {
    /// Access the user state.
    pub fn state(&self) -> &S {
        &self.user_state
    }

    /// Access the user state mutably.
    pub fn state_mut(&mut self) -> &mut S {
        &mut self.user_state
    }
}

/// Metrics collected during sandbox execution.
#[derive(Debug, Clone, Default)]
pub struct SandboxMetrics {
    /// When execution started.
    pub start_time: Option<Instant>,
    /// When execution ended.
    pub end_time: Option<Instant>,
    /// Total fuel consumed.
    pub fuel_consumed: u64,
    /// Peak memory usage in bytes.
    pub peak_memory: usize,
    /// Number of host function calls.
    pub host_calls: u64,
}

impl SandboxMetrics {
    /// Get the execution duration.
    pub fn duration(&self) -> Option<Duration> {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            _ => None,
        }
    }
}

/// A sandboxed execution environment for WebAssembly modules.
///
/// The `Sandbox` provides isolation guarantees by:
/// - Enforcing memory limits
/// - Tracking and limiting CPU usage via fuel
/// - Supporting execution timeouts via epochs
/// - Isolating state between executions
///
/// # Type Parameters
///
/// - `S`: User-provided state type that can be accessed from host functions.
///
/// # Example
///
/// ```ignore
/// use aegis_core::{AegisEngine, Sandbox, SandboxConfig};
///
/// let engine = AegisEngine::default_engine()?.into_shared();
/// let mut sandbox = Sandbox::new(engine, (), SandboxConfig::default())?;
///
/// // Load and execute a module
/// sandbox.load_module(&module)?;
/// let result: i32 = sandbox.call("add", (2i32, 3i32))?;
/// ```
pub struct Sandbox<S = ()> {
    /// Shared engine reference.
    engine: SharedEngine,
    /// Wasmtime store with sandbox data.
    store: Store<SandboxData<S>>,
    /// Wasmtime linker for host function registration.
    linker: Linker<SandboxData<S>>,
    /// Currently loaded instance.
    instance: Option<Instance>,
    /// Currently loaded module.
    module: Option<ValidatedModule>,
}

impl<S: Send + 'static> Sandbox<S> {
    /// Create a new sandbox with the given engine and user state.
    pub fn new(
        engine: SharedEngine,
        user_state: S,
        config: SandboxConfig,
    ) -> ExecutionResult<Self> {
        let id = SandboxId::new();

        // Build store limits from resource limits
        let limits = StoreLimitsBuilder::new()
            .memory_size(config.limits.max_memory_bytes)
            .table_elements(config.limits.max_table_elements as usize)
            .instances(1)
            .tables(10)
            .memories(config.limits.max_memories as usize)
            .build();

        let data = SandboxData {
            id,
            user_state,
            limits,
            metrics: SandboxMetrics::default(),
            config: config.clone(),
        };

        let mut store = Store::new(engine.inner(), data);

        // Configure store limits
        store.limiter(|data| &mut data.limits);

        // Configure fuel if enabled
        if engine.fuel_enabled() {
            store.set_fuel(config.limits.initial_fuel)?;
        }

        // Configure epoch deadline if enabled
        if engine.epoch_enabled() {
            // Calculate epochs based on timeout
            // Assuming 10ms per epoch tick
            let deadline_epochs = (config.limits.timeout.as_millis() / 10) as u64;
            store.epoch_deadline_trap();
            store.set_epoch_deadline(deadline_epochs.max(1));
        }

        let linker = Linker::new(engine.inner());

        info!(sandbox_id = %id, "Created new sandbox");

        Ok(Self {
            engine,
            store,
            linker,
            instance: None,
            module: None,
        })
    }

    /// Get the sandbox ID.
    pub fn id(&self) -> SandboxId {
        self.store.data().id
    }

    /// Get a reference to the engine.
    pub fn engine(&self) -> &SharedEngine {
        &self.engine
    }

    /// Access the user state.
    pub fn state(&self) -> &S {
        &self.store.data().user_state
    }

    /// Access the user state mutably.
    pub fn state_mut(&mut self) -> &mut S {
        &mut self.store.data_mut().user_state
    }

    /// Get the execution metrics.
    pub fn metrics(&self) -> &SandboxMetrics {
        &self.store.data().metrics
    }

    /// Get a mutable reference to the linker for registering host functions.
    pub fn linker_mut(&mut self) -> &mut Linker<SandboxData<S>> {
        &mut self.linker
    }

    /// Register a host function.
    ///
    /// # Example
    ///
    /// ```ignore
    /// sandbox.register_func("env", "log", |caller: Caller<'_, SandboxData<()>>, val: i32| {
    ///     println!("Guest logged: {}", val);
    /// })?;
    /// ```
    pub fn register_func<Params, Results>(
        &mut self,
        module: &str,
        name: &str,
        func: impl wasmtime::IntoFunc<SandboxData<S>, Params, Results>,
    ) -> ExecutionResult<()> {
        self.linker.func_wrap(module, name, func)?;
        debug!(module, name, "Registered host function");
        Ok(())
    }

    /// Load a validated module into the sandbox.
    ///
    /// This compiles and instantiates the module, linking it with any
    /// registered host functions.
    pub fn load_module(&mut self, module: &ValidatedModule) -> ExecutionResult<()> {
        debug!(
            sandbox_id = %self.id(),
            module_name = ?module.name(),
            "Loading module into sandbox"
        );

        let instance = self.linker.instantiate(&mut self.store, module.inner())?;

        self.instance = Some(instance);
        self.module = Some(module.clone());

        info!(
            sandbox_id = %self.id(),
            module_name = ?module.name(),
            "Module loaded successfully"
        );

        Ok(())
    }

    /// Check if a module is currently loaded.
    pub fn is_loaded(&self) -> bool {
        self.instance.is_some()
    }

    /// Get information about the loaded module.
    pub fn loaded_module(&self) -> Option<&ValidatedModule> {
        self.module.as_ref()
    }

    /// Call an exported function with no arguments and no return value.
    pub fn call_void(&mut self, name: &str) -> ExecutionResult<()> {
        self.call::<(), ()>(name, ())
    }

    /// Call an exported function.
    ///
    /// # Type Parameters
    ///
    /// - `P`: Parameter type (must implement `WasmParams`)
    /// - `R`: Return type (must implement `WasmResults`)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let result: i32 = sandbox.call("add", (2i32, 3i32))?;
    /// ```
    pub fn call<P, R>(&mut self, name: &str, params: P) -> ExecutionResult<R>
    where
        P: wasmtime::WasmParams,
        R: wasmtime::WasmResults,
    {
        let instance = self
            .instance
            .as_ref()
            .ok_or(ExecutionError::ModuleNotLoaded)?;

        let func = instance
            .get_typed_func::<P, R>(&mut self.store, name)
            .map_err(|_| ExecutionError::FunctionNotFound(name.to_string()))?;

        // Record start time
        self.store.data_mut().metrics.start_time = Some(Instant::now());

        // Get initial fuel
        let initial_fuel = if self.engine.fuel_enabled() {
            self.store.get_fuel().unwrap_or(0)
        } else {
            0
        };

        debug!(sandbox_id = %self.id(), function = name, "Calling function");

        // Execute the function
        let result = func.call(&mut self.store, params);

        // Record end time
        self.store.data_mut().metrics.end_time = Some(Instant::now());

        // Calculate fuel consumed
        if self.engine.fuel_enabled() {
            let remaining_fuel = self.store.get_fuel().unwrap_or(0);
            self.store.data_mut().metrics.fuel_consumed = initial_fuel.saturating_sub(remaining_fuel);
        }

        // Handle the result
        match result {
            Ok(value) => {
                info!(
                    sandbox_id = %self.id(),
                    function = name,
                    duration = ?self.store.data().metrics.duration(),
                    "Function call completed successfully"
                );
                Ok(value)
            }
            Err(err) => {
                // Check if it's a trap first, then inspect the trap message
                if let Some(trap) = err.downcast_ref::<wasmtime::Trap>() {
                    let trap_msg = trap.to_string();

                    // Check for out of fuel
                    if trap_msg.contains("fuel") {
                        let limit = self.store.data().config.limits.initial_fuel;
                        warn!(
                            sandbox_id = %self.id(),
                            function = name,
                            "Out of fuel"
                        );
                        return Err(ExecutionError::OutOfFuel {
                            consumed: self.store.data().metrics.fuel_consumed,
                            limit,
                        });
                    }

                    // Check for epoch deadline
                    if trap_msg.contains("epoch") {
                        warn!(
                            sandbox_id = %self.id(),
                            function = name,
                            "Execution timeout"
                        );
                        return Err(ExecutionError::Timeout(
                            self.store.data().config.limits.timeout,
                        ));
                    }

                    // Generic trap
                    warn!(
                        sandbox_id = %self.id(),
                        function = name,
                        trap = ?trap,
                        "Function trapped"
                    );
                    return Err(ExecutionError::Trap(TrapInfo::from(trap.clone())));
                }

                // Generic wasmtime error
                Err(ExecutionError::Wasmtime(err))
            }
        }
    }

    /// Get the remaining fuel.
    pub fn remaining_fuel(&self) -> Option<u64> {
        if self.engine.fuel_enabled() {
            self.store.get_fuel().ok()
        } else {
            None
        }
    }

    /// Add more fuel to the sandbox.
    pub fn add_fuel(&mut self, fuel: u64) -> ExecutionResult<()> {
        if self.engine.fuel_enabled() {
            let current = self.store.get_fuel()?;
            self.store.set_fuel(current + fuel)?;
            debug!(sandbox_id = %self.id(), added = fuel, total = current + fuel, "Added fuel");
        }
        Ok(())
    }

    /// Get the type signature of an exported function.
    ///
    /// Returns the function type if the function exists, or None otherwise.
    pub fn get_func_type(&mut self, name: &str) -> Option<wasmtime::FuncType> {
        let instance = self.instance.as_ref()?;
        let func = instance.get_func(&mut self.store, name)?;
        Some(func.ty(&self.store))
    }

    /// Call an exported function with dynamic typing.
    ///
    /// This is useful for CLI tools or scenarios where function signatures
    /// aren't known at compile time.
    ///
    /// # Arguments
    ///
    /// - `name`: Name of the exported function
    /// - `params`: Vector of parameter values
    ///
    /// # Returns
    ///
    /// Vector of return values from the function.
    pub fn call_dynamic(
        &mut self,
        name: &str,
        params: Vec<wasmtime::Val>,
    ) -> ExecutionResult<Vec<wasmtime::Val>> {
        let instance = self
            .instance
            .as_ref()
            .ok_or(ExecutionError::ModuleNotLoaded)?;

        let func = instance
            .get_func(&mut self.store, name)
            .ok_or_else(|| ExecutionError::FunctionNotFound(name.to_string()))?;

        // Get function type to determine result count
        let func_type = func.ty(&self.store);
        let result_count = func_type.results().len();
        let mut results = vec![wasmtime::Val::I32(0); result_count];

        // Record start time
        self.store.data_mut().metrics.start_time = Some(Instant::now());

        // Get initial fuel
        let initial_fuel = if self.engine.fuel_enabled() {
            self.store.get_fuel().unwrap_or(0)
        } else {
            0
        };

        debug!(sandbox_id = %self.id(), function = name, "Calling function (dynamic)");

        // Execute the function
        let call_result = func.call(&mut self.store, &params, &mut results);

        // Record end time
        self.store.data_mut().metrics.end_time = Some(Instant::now());

        // Record fuel consumption
        if self.engine.fuel_enabled() {
            let remaining = self.store.get_fuel().unwrap_or(0);
            self.store.data_mut().metrics.fuel_consumed = initial_fuel.saturating_sub(remaining);
        }

        match call_result {
            Ok(()) => {
                info!(
                    sandbox_id = %self.id(),
                    function = name,
                    "Function call completed successfully"
                );
                Ok(results)
            }
            Err(err) => {
                // Check if it's a trap first, then inspect the trap message
                if let Some(trap) = err.downcast_ref::<wasmtime::Trap>() {
                    let trap_msg = trap.to_string();

                    if trap_msg.contains("fuel") {
                        let limit = self.store.data().config.limits.initial_fuel;
                        warn!(sandbox_id = %self.id(), function = name, "Out of fuel");
                        return Err(ExecutionError::OutOfFuel {
                            consumed: self.store.data().metrics.fuel_consumed,
                            limit,
                        });
                    }

                    if trap_msg.contains("epoch") {
                        warn!(sandbox_id = %self.id(), function = name, "Execution timeout");
                        return Err(ExecutionError::Timeout(
                            self.store.data().config.limits.timeout,
                        ));
                    }

                    warn!(sandbox_id = %self.id(), function = name, trap = ?trap, "Function trapped");
                    return Err(ExecutionError::Trap(TrapInfo::from(trap.clone())));
                }

                Err(ExecutionError::Wasmtime(err))
            }
        }
    }

    /// Reset the sandbox for reuse.
    ///
    /// This clears the current instance and resets metrics, but preserves
    /// registered host functions.
    pub fn reset(&mut self) {
        self.instance = None;
        self.module = None;
        self.store.data_mut().metrics = SandboxMetrics::default();

        // Reset fuel if enabled
        if self.engine.fuel_enabled() {
            let initial = self.store.data().config.limits.initial_fuel;
            let _ = self.store.set_fuel(initial);
        }

        debug!(sandbox_id = %self.id(), "Sandbox reset");
    }
}

impl<S: Send + 'static> std::fmt::Debug for Sandbox<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sandbox")
            .field("id", &self.id())
            .field("loaded", &self.is_loaded())
            .field("metrics", self.metrics())
            .finish()
    }
}

/// Builder for creating sandboxes with custom configuration.
pub struct SandboxBuilder<S = ()> {
    engine: SharedEngine,
    user_state: Option<S>,
    config: SandboxConfig,
}

impl<S: Send + 'static> SandboxBuilder<S> {
    /// Create a new sandbox builder.
    pub fn new(engine: SharedEngine) -> Self {
        Self {
            engine,
            user_state: None,
            config: SandboxConfig::default(),
        }
    }

    /// Set the user state.
    pub fn with_state(mut self, state: S) -> Self {
        self.user_state = Some(state);
        self
    }

    /// Set the sandbox configuration.
    pub fn with_config(mut self, config: SandboxConfig) -> Self {
        self.config = config;
        self
    }

    /// Set resource limits.
    pub fn with_limits(mut self, limits: ResourceLimits) -> Self {
        self.config.limits = limits;
        self
    }

    /// Set the memory limit.
    pub fn with_memory_limit(mut self, bytes: usize) -> Self {
        self.config.limits.max_memory_bytes = bytes;
        self
    }

    /// Set the fuel limit.
    pub fn with_fuel_limit(mut self, fuel: u64) -> Self {
        self.config.limits.initial_fuel = fuel;
        self
    }

    /// Set the execution timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.config.limits.timeout = timeout;
        self
    }

    /// Build the sandbox.
    pub fn build(self) -> ExecutionResult<Sandbox<S>>
    where
        S: Default,
    {
        let state = self.user_state.unwrap_or_default();
        Sandbox::new(self.engine, state, self.config)
    }

    /// Build the sandbox with the provided state.
    pub fn build_with_state(self, state: S) -> ExecutionResult<Sandbox<S>> {
        Sandbox::new(self.engine, state, self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::config::EngineConfig;
    use crate::engine::AegisEngine;
    use crate::module::ModuleLoader;

    fn create_engine() -> SharedEngine {
        Arc::new(AegisEngine::new(EngineConfig::default()).unwrap())
    }

    #[test]
    fn test_sandbox_creation() {
        let engine = create_engine();
        let sandbox = Sandbox::<()>::new(engine, (), SandboxConfig::default()).unwrap();

        assert!(!sandbox.is_loaded());
    }

    #[test]
    fn test_sandbox_builder() {
        let engine = create_engine();
        let sandbox = SandboxBuilder::<()>::new(engine)
            .with_memory_limit(1024 * 1024)
            .with_fuel_limit(10_000)
            .with_timeout(Duration::from_secs(5))
            .build()
            .unwrap();

        assert!(!sandbox.is_loaded());
    }

    #[test]
    fn test_load_and_call() {
        let engine = create_engine();
        let loader = ModuleLoader::new(Arc::clone(&engine));

        let module = loader
            .load_wat(
                r#"
            (module
                (func (export "add") (param i32 i32) (result i32)
                    local.get 0
                    local.get 1
                    i32.add
                )
            )
        "#,
            )
            .unwrap();

        let mut sandbox = Sandbox::<()>::new(engine, (), SandboxConfig::default()).unwrap();
        sandbox.load_module(&module).unwrap();

        assert!(sandbox.is_loaded());

        let result: i32 = sandbox.call("add", (2i32, 3i32)).unwrap();
        assert_eq!(result, 5);
    }

    #[test]
    fn test_fuel_consumption() {
        let engine = create_engine();
        let loader = ModuleLoader::new(Arc::clone(&engine));

        let module = loader
            .load_wat(
                r#"
            (module
                (func (export "count") (param i32) (result i32)
                    (local $i i32)
                    (local.set $i (i32.const 0))
                    (block $done
                        (loop $loop
                            (br_if $done (i32.ge_u (local.get $i) (local.get 0)))
                            (local.set $i (i32.add (local.get $i) (i32.const 1)))
                            (br $loop)
                        )
                    )
                    (local.get $i)
                )
            )
        "#,
            )
            .unwrap();

        let mut sandbox = SandboxBuilder::<()>::new(engine)
            .with_fuel_limit(1_000_000)
            .build()
            .unwrap();

        sandbox.load_module(&module).unwrap();

        let initial_fuel = sandbox.remaining_fuel().unwrap();
        let _result: i32 = sandbox.call("count", (100i32,)).unwrap();
        let remaining_fuel = sandbox.remaining_fuel().unwrap();

        assert!(remaining_fuel < initial_fuel, "Fuel should be consumed");
        assert!(sandbox.metrics().fuel_consumed > 0);
    }

    #[test]
    fn test_out_of_fuel() {
        let engine = create_engine();
        let loader = ModuleLoader::new(Arc::clone(&engine));

        let module = loader
            .load_wat(
                r#"
            (module
                (func (export "infinite")
                    (loop $loop
                        (br $loop)
                    )
                )
            )
        "#,
            )
            .unwrap();

        let mut sandbox = SandboxBuilder::<()>::new(engine)
            .with_fuel_limit(1000)
            .build()
            .unwrap();

        sandbox.load_module(&module).unwrap();

        let result = sandbox.call::<(), ()>("infinite", ());
        assert!(matches!(result, Err(ExecutionError::OutOfFuel { .. })));
    }

    #[test]
    fn test_function_not_found() {
        let engine = create_engine();
        let loader = ModuleLoader::new(Arc::clone(&engine));

        let module = loader
            .load_wat(
                r#"
            (module
                (func (export "exists"))
            )
        "#,
            )
            .unwrap();

        let mut sandbox = Sandbox::<()>::new(engine, (), SandboxConfig::default()).unwrap();
        sandbox.load_module(&module).unwrap();

        let result = sandbox.call::<(), ()>("does_not_exist", ());
        assert!(matches!(result, Err(ExecutionError::FunctionNotFound(_))));
    }

    #[test]
    fn test_sandbox_reset() {
        let engine = create_engine();
        let loader = ModuleLoader::new(Arc::clone(&engine));

        let module = loader
            .load_wat(
                r#"
            (module
                (func (export "noop"))
            )
        "#,
            )
            .unwrap();

        let mut sandbox = SandboxBuilder::<()>::new(engine)
            .with_fuel_limit(1_000_000)
            .build()
            .unwrap();

        sandbox.load_module(&module).unwrap();
        sandbox.call::<(), ()>("noop", ()).unwrap();

        let fuel_after_call = sandbox.remaining_fuel().unwrap();

        sandbox.reset();

        assert!(!sandbox.is_loaded());
        assert!(sandbox.remaining_fuel().unwrap() > fuel_after_call);
    }
}
