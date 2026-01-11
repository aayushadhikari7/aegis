//! Aegis Core - WebAssembly Sandbox Runtime
//!
//! This crate provides the core functionality for the Aegis WebAssembly sandbox runtime.
//! It includes:
//!
//! - [`AegisEngine`]: The core engine that wraps Wasmtime
//! - [`ModuleLoader`]: Loading and validating WASM modules
//! - [`Sandbox`]: Isolated execution environment
//! - Configuration types for customizing behavior
//!
//! # Quick Start
//!
//! ```ignore
//! use aegis_core::prelude::*;
//!
//! // Create an engine
//! let engine = AegisEngine::default_engine()?.into_shared();
//!
//! // Load a module
//! let loader = ModuleLoader::new(engine.clone());
//! let module = loader.load_file(Path::new("module.wasm"))?;
//!
//! // Create a sandbox and execute
//! let mut sandbox = SandboxBuilder::new(engine)
//!     .with_fuel_limit(1_000_000)
//!     .build()?;
//!
//! sandbox.load_module(&module)?;
//! let result: i32 = sandbox.call("add", (2i32, 3i32))?;
//! ```
//!
//! # Security Model
//!
//! Aegis provides security through multiple layers:
//!
//! 1. **Memory Isolation**: Each sandbox has its own linear memory space
//! 2. **Resource Limits**: Memory, CPU (fuel), and time limits are enforced
//! 3. **Capability-Based Security**: Host functions require explicit capabilities
//! 4. **No Ambient Authority**: All permissions must be explicitly granted
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │              Application                │
//! ├─────────────────────────────────────────┤
//! │            aegis (facade)               │
//! ├─────────────────────────────────────────┤
//! │  aegis-core  │  aegis-capability  │ ... │
//! ├─────────────────────────────────────────┤
//! │              Wasmtime                   │
//! └─────────────────────────────────────────┘
//! ```

pub mod config;
pub mod engine;
pub mod error;
pub mod module;
pub mod sandbox;

// Re-export main types at crate root
pub use config::{EngineConfig, ResourceLimits, SandboxConfig};
pub use engine::{AegisEngine, IntoShared, SharedEngine};
pub use error::{
    AegisError, EngineError, ExecutionError, ModuleError, Result, TrapInfo,
};
pub use module::{
    ExportInfo, ExportKind, ImportInfo, ImportKind, MemoryInfo, ModuleLoader, ModuleMetadata,
    ValidatedModule,
};
pub use sandbox::{Sandbox, SandboxBuilder, SandboxData, SandboxId, SandboxMetrics};

/// Prelude module for convenient imports.
///
/// # Example
///
/// ```ignore
/// use aegis_core::prelude::*;
/// ```
pub mod prelude {
    pub use crate::config::{EngineConfig, ResourceLimits, SandboxConfig};
    pub use crate::engine::{AegisEngine, IntoShared, SharedEngine};
    pub use crate::error::{AegisError, ExecutionError, ModuleError, Result};
    pub use crate::module::{ModuleLoader, ValidatedModule};
    pub use crate::sandbox::{Sandbox, SandboxBuilder, SandboxId};
}

#[cfg(test)]
mod tests {
    use super::prelude::*;
    use std::sync::Arc;

    #[test]
    fn test_end_to_end() {
        // Create engine
        let engine = AegisEngine::default_engine().unwrap().into_shared();

        // Load module
        let loader = ModuleLoader::new(Arc::clone(&engine));
        let module = loader
            .load_wat(
                r#"
            (module
                (func (export "double") (param i32) (result i32)
                    local.get 0
                    i32.const 2
                    i32.mul
                )
            )
        "#,
            )
            .unwrap();

        // Create sandbox
        let mut sandbox = SandboxBuilder::<()>::new(engine).build().unwrap();

        // Load and execute
        sandbox.load_module(&module).unwrap();
        let result: i32 = sandbox.call("double", (21i32,)).unwrap();

        assert_eq!(result, 42);
    }
}
