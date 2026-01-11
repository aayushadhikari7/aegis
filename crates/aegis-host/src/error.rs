//! Error types for the host function system.

use aegis_capability::CapabilityId;
use thiserror::Error;

/// Errors related to host functions.
#[derive(Debug, Error)]
pub enum HostError {
    /// A required capability was not granted.
    #[error("Capability not granted: {0}")]
    CapabilityNotGranted(CapabilityId),

    /// Permission was denied for an action.
    #[error("Permission denied for action '{action}': {reason}")]
    PermissionDenied {
        /// The action that was denied.
        action: String,
        /// The reason for denial.
        reason: String,
    },

    /// No capability handles the requested action.
    #[error("No capability found for action: {action}")]
    NoCapabilityForAction {
        /// The action that was attempted.
        action: String,
    },

    /// Memory export not found.
    #[error("Memory export 'memory' not found")]
    MemoryNotFound,

    /// Memory access out of bounds.
    #[error("Memory access out of bounds: offset={offset}, len={len}, memory_size={memory_size}")]
    MemoryAccessOutOfBounds {
        /// The offset attempted.
        offset: usize,
        /// The length attempted.
        len: usize,
        /// The actual memory size.
        memory_size: usize,
    },

    /// Invalid UTF-8 in string.
    #[error("Invalid UTF-8: {0}")]
    InvalidUtf8(String),

    /// Function registration failed.
    #[error("Failed to register function '{module}::{name}': {reason}")]
    RegistrationFailed {
        /// The module name.
        module: String,
        /// The function name.
        name: String,
        /// The reason for failure.
        reason: String,
    },

    /// Function already registered.
    #[error("Function already registered: {module}::{name}")]
    AlreadyRegistered {
        /// The module name.
        module: String,
        /// The function name.
        name: String,
    },

    /// Underlying Wasmtime error.
    #[error("Wasmtime error: {0}")]
    Wasmtime(#[from] wasmtime::Error),

    /// Generic host error.
    #[error("Host error: {0}")]
    Other(String),
}

/// Result type for host operations.
pub type HostResult<T> = std::result::Result<T, HostError>;
