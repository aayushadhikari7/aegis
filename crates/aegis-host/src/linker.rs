//! Safe linker wrapper with capability enforcement.
//!
//! This module provides the `AegisLinker` type which wraps Wasmtime's `Linker`
//! with capability-aware host function registration.

use aegis_capability::{CapabilityId, CapabilitySet};
use tracing::{debug, info};
use wasmtime::{Engine, Linker};

use crate::error::{HostError, HostResult};

/// Information about a registered host function.
#[derive(Debug, Clone)]
pub struct RegisteredFunction {
    /// The import module name.
    pub module: String,
    /// The function name.
    pub name: String,
    /// Required capability, if any.
    pub required_capability: Option<CapabilityId>,
    /// Human-readable description.
    pub description: Option<String>,
}

/// A safe wrapper around Wasmtime's `Linker` with capability enforcement.
///
/// `AegisLinker` tracks registered host functions and their capability
/// requirements, enabling runtime validation of capability grants.
pub struct AegisLinker<T> {
    /// The underlying Wasmtime linker.
    inner: Linker<T>,
    /// Registry of registered functions.
    registered: Vec<RegisteredFunction>,
}

impl<T> AegisLinker<T> {
    /// Create a new linker for the given engine.
    pub fn new(engine: &Engine) -> Self {
        Self {
            inner: Linker::new(engine),
            registered: Vec::new(),
        }
    }

    /// Get a reference to the underlying Wasmtime linker.
    pub fn inner(&self) -> &Linker<T> {
        &self.inner
    }

    /// Get a mutable reference to the underlying Wasmtime linker.
    pub fn inner_mut(&mut self) -> &mut Linker<T> {
        &mut self.inner
    }

    /// Consume this linker and return the underlying Wasmtime linker.
    pub fn into_inner(self) -> Linker<T> {
        self.inner
    }

    /// Get the list of registered functions.
    pub fn registered_functions(&self) -> &[RegisteredFunction] {
        &self.registered
    }

    /// Check if a function is already registered.
    pub fn is_registered(&self, module: &str, name: &str) -> bool {
        self.registered
            .iter()
            .any(|f| f.module == module && f.name == name)
    }

    /// Register a host function.
    ///
    /// # Arguments
    ///
    /// * `module` - The import module name
    /// * `name` - The function name
    /// * `func` - The host function implementation
    pub fn func_wrap<Params, Results>(
        &mut self,
        module: &str,
        name: &str,
        func: impl wasmtime::IntoFunc<T, Params, Results>,
    ) -> HostResult<&mut Self> {
        self.func_wrap_with_capability(module, name, None, func)
    }

    /// Register a host function with a required capability.
    pub fn func_wrap_with_capability<Params, Results>(
        &mut self,
        module: &str,
        name: &str,
        required_capability: Option<CapabilityId>,
        func: impl wasmtime::IntoFunc<T, Params, Results>,
    ) -> HostResult<&mut Self> {
        if self.is_registered(module, name) {
            return Err(HostError::AlreadyRegistered {
                module: module.to_string(),
                name: name.to_string(),
            });
        }

        self.inner
            .func_wrap(module, name, func)
            .map_err(|e| HostError::RegistrationFailed {
                module: module.to_string(),
                name: name.to_string(),
                reason: e.to_string(),
            })?;

        self.registered.push(RegisteredFunction {
            module: module.to_string(),
            name: name.to_string(),
            required_capability,
            description: None,
        });

        debug!(module, name, "Registered host function");
        Ok(self)
    }

    /// Define a module in the linker.
    ///
    /// Note: In wasmtime 29+, `define` requires a store context. Use `define_with_store`
    /// when you have a store available, or use `func_wrap` for most host function needs.
    pub fn define_with_store<S: wasmtime::AsContext<Data = T>>(
        &mut self,
        store: S,
        module: &str,
        name: &str,
        item: impl Into<wasmtime::Extern>,
    ) -> HostResult<&mut Self> {
        self.inner
            .define(store, module, name, item)
            .map_err(|e| HostError::RegistrationFailed {
                module: module.to_string(),
                name: name.to_string(),
                reason: e.to_string(),
            })?;

        Ok(self)
    }

    /// Validate that all required capabilities are present in the given set.
    pub fn validate_capabilities(&self, capabilities: &CapabilitySet) -> HostResult<()> {
        for func in &self.registered {
            if let Some(ref required) = func.required_capability {
                if !capabilities.has(required) {
                    return Err(HostError::CapabilityNotGranted(required.clone()));
                }
            }
        }
        Ok(())
    }

    /// Get functions that require a specific capability.
    pub fn functions_requiring(&self, capability: &CapabilityId) -> Vec<&RegisteredFunction> {
        self.registered
            .iter()
            .filter(|f| f.required_capability.as_ref() == Some(capability))
            .collect()
    }

    /// Get functions that require capabilities not in the given set.
    pub fn missing_capabilities(&self, capabilities: &CapabilitySet) -> Vec<CapabilityId> {
        let mut missing = Vec::new();

        for func in &self.registered {
            if let Some(ref required) = func.required_capability {
                if !capabilities.has(required) && !missing.contains(required) {
                    missing.push(required.clone());
                }
            }
        }

        missing
    }
}

impl<T> std::fmt::Debug for AegisLinker<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AegisLinker")
            .field("registered_functions", &self.registered.len())
            .finish()
    }
}

/// Builder for constructing an `AegisLinker` with common host functions.
pub struct AegisLinkerBuilder<T> {
    linker: AegisLinker<T>,
}

impl<T: Send + 'static> AegisLinkerBuilder<T> {
    /// Create a new builder.
    pub fn new(engine: &Engine) -> Self {
        Self {
            linker: AegisLinker::new(engine),
        }
    }

    /// Build the linker.
    pub fn build(self) -> AegisLinker<T> {
        info!(
            functions = self.linker.registered.len(),
            "Built AegisLinker"
        );
        self.linker
    }

    /// Access the linker being built.
    pub fn linker_mut(&mut self) -> &mut AegisLinker<T> {
        &mut self.linker
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aegis_capability::CapabilitySet;
    use wasmtime::Engine;

    fn create_engine() -> Engine {
        Engine::default()
    }

    #[test]
    fn test_linker_creation() {
        let engine = create_engine();
        let linker = AegisLinker::<()>::new(&engine);
        assert!(linker.registered_functions().is_empty());
    }

    #[test]
    fn test_func_wrap() {
        let engine = create_engine();
        let mut linker = AegisLinker::<()>::new(&engine);

        linker
            .func_wrap("env", "test", |_: i32| -> i32 { 42 })
            .unwrap();

        assert!(linker.is_registered("env", "test"));
        assert_eq!(linker.registered_functions().len(), 1);
    }

    #[test]
    fn test_duplicate_registration() {
        let engine = create_engine();
        let mut linker = AegisLinker::<()>::new(&engine);

        linker
            .func_wrap("env", "test", |_: i32| -> i32 { 42 })
            .unwrap();

        let result = linker.func_wrap("env", "test", |_: i32| -> i32 { 0 });
        assert!(result.is_err());
    }

    #[test]
    fn test_capability_validation() {
        let engine = create_engine();
        let mut linker = AegisLinker::<()>::new(&engine);

        let cap_id = CapabilityId::new("test_cap");
        linker
            .func_wrap_with_capability("env", "test", Some(cap_id.clone()), |_: i32| -> i32 { 42 })
            .unwrap();

        // Empty capability set should fail validation
        let empty_caps = CapabilitySet::new();
        assert!(linker.validate_capabilities(&empty_caps).is_err());
    }

    #[test]
    fn test_missing_capabilities() {
        let engine = create_engine();
        let mut linker = AegisLinker::<()>::new(&engine);

        let cap1 = CapabilityId::new("cap1");
        let cap2 = CapabilityId::new("cap2");

        linker
            .func_wrap_with_capability("env", "func1", Some(cap1.clone()), || {})
            .unwrap();
        linker
            .func_wrap_with_capability("env", "func2", Some(cap2.clone()), || {})
            .unwrap();

        let empty_caps = CapabilitySet::new();
        let missing = linker.missing_capabilities(&empty_caps);

        assert_eq!(missing.len(), 2);
        assert!(missing.contains(&cap1));
        assert!(missing.contains(&cap2));
    }
}
