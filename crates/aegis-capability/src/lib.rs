//! Aegis Capability System
//!
//! This crate provides the capability-based security system for the Aegis
//! WebAssembly sandbox runtime. Capabilities are explicit, opt-in permissions
//! that control what a sandboxed module can do.
//!
//! # Capability-Based Security
//!
//! Aegis uses a capability-based security model where:
//!
//! - All permissions must be explicitly granted (no ambient authority)
//! - Capabilities are immutable once execution begins
//! - Capabilities are composable
//! - Absence of a capability guarantees denial
//!
//! # Built-in Capabilities
//!
//! The crate provides several built-in capabilities:
//!
//! - [`FilesystemCapability`]: File system access
//! - [`NetworkCapability`]: Network access
//! - [`LoggingCapability`]: Logging output
//! - [`ClockCapability`]: Time and clock access
//!
//! # Custom Capabilities
//!
//! You can define custom capabilities by implementing the [`Capability`] trait:
//!
//! ```ignore
//! use aegis_capability::{Capability, CapabilityId, Action, PermissionResult};
//!
//! #[derive(Debug)]
//! struct MyCapability;
//!
//! impl Capability for MyCapability {
//!     fn id(&self) -> CapabilityId {
//!         CapabilityId::new("my_capability")
//!     }
//!
//!     fn name(&self) -> &str {
//!         "My Capability"
//!     }
//!
//!     fn description(&self) -> &str {
//!         "Custom capability for my use case"
//!     }
//!
//!     fn permits(&self, action: &dyn Action) -> PermissionResult {
//!         // Implement permission checking logic
//!         PermissionResult::Allowed
//!     }
//! }
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use aegis_capability::{CapabilitySet, CapabilitySetBuilder};
//! use aegis_capability::builtin::{LoggingCapability, ClockCapability};
//!
//! let capabilities = CapabilitySetBuilder::new()
//!     .with(LoggingCapability::production())
//!     .with(ClockCapability::monotonic_only())
//!     .build()?;
//!
//! // Check permissions
//! let result = capabilities.check_permission(&some_action);
//! ```

pub mod builtin;
pub mod capability;
pub mod error;
pub mod set;

// Re-export main types
pub use capability::{
    Action, BoxedCapability, Capability, CapabilityId, DenialReason, PermissionResult,
    SharedCapability, standard_ids,
};
pub use error::{CapabilityError, CapabilityResult};
pub use set::{CapabilitySet, CapabilitySetBuilder};

// Re-export built-in capabilities
pub use builtin::{
    ClockCapability, ClockType, FilesystemCapability, HostPattern, LogLevel, LoggingCapability,
    NetworkCapability, PathPermission, ProtocolSet,
};

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::capability::{Action, Capability, CapabilityId, PermissionResult};
    pub use crate::error::{CapabilityError, CapabilityResult};
    pub use crate::set::{CapabilitySet, CapabilitySetBuilder};

    // Built-in capabilities
    pub use crate::builtin::{
        ClockCapability, FilesystemCapability, LoggingCapability, NetworkCapability,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prelude_imports() {
        use crate::prelude::*;

        let _ = CapabilityId::new("test");
        let _ = CapabilitySet::new();
    }

    #[test]
    fn test_capability_set_with_builtins() {
        let set = CapabilitySetBuilder::new()
            .with(LoggingCapability::production())
            .with(ClockCapability::monotonic_only())
            .build()
            .unwrap();

        assert!(set.has(&standard_ids::LOGGING));
        assert!(set.has(&standard_ids::CLOCK));
        assert!(!set.has(&standard_ids::FILESYSTEM));
    }
}
