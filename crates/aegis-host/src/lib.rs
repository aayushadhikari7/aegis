//! Aegis Host Function System
//!
//! This crate provides the host function system for the Aegis WebAssembly
//! sandbox runtime. It includes:
//!
//! - [`AegisLinker`]: Safe wrapper around Wasmtime's Linker
//! - [`HostContext`]: Context available to host function implementations
//! - Capability-aware function registration
//!
//! # Host Functions
//!
//! Host functions are the bridge between guest WASM code and the host system.
//! Aegis requires all host functions to be registered with their capability
//! requirements, ensuring that guests can only call functions they have
//! permission to use.
//!
//! # Example
//!
//! ```ignore
//! use aegis_host::{AegisLinker, HostContext, IntoHostContext};
//! use aegis_capability::CapabilityId;
//!
//! let mut linker = AegisLinker::new(&engine);
//!
//! // Register a host function that requires the logging capability
//! linker.func_wrap_with_capability(
//!     "env",
//!     "log",
//!     Some(CapabilityId::new("logging")),
//!     |caller: wasmtime::Caller<'_, MyState>, msg_ptr: i32, msg_len: i32| {
//!         let mut ctx = caller.into_context();
//!         let message = ctx.read_string_with_len(msg_ptr as usize, msg_len as usize)?;
//!         println!("Guest log: {}", message);
//!         Ok(())
//!     },
//! )?;
//! ```

pub mod context;
pub mod error;
pub mod linker;

// Re-export main types
pub use context::{HostContext, IntoHostContext};
pub use error::{HostError, HostResult};
pub use linker::{AegisLinker, AegisLinkerBuilder, RegisteredFunction};

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::context::{HostContext, IntoHostContext};
    pub use crate::error::{HostError, HostResult};
    pub use crate::linker::{AegisLinker, RegisteredFunction};
}
