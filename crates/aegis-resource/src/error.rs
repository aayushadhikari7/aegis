//! Error types for resource management.

use thiserror::Error;

/// Errors related to resource management.
#[derive(Debug, Error)]
pub enum ResourceError {
    /// Memory allocation failed.
    #[error("Memory allocation failed: requested {requested} bytes, available {available} bytes")]
    MemoryAllocationFailed {
        /// Requested memory in bytes.
        requested: usize,
        /// Available memory in bytes.
        available: usize,
    },

    /// Memory limit exceeded.
    #[error("Memory limit exceeded: {used} bytes used, limit is {limit} bytes")]
    MemoryLimitExceeded {
        /// Memory used in bytes.
        used: usize,
        /// Memory limit in bytes.
        limit: usize,
    },

    /// Fuel exhausted.
    #[error("Fuel exhausted: consumed {consumed} units, limit was {limit} units")]
    FuelExhausted {
        /// Fuel consumed.
        consumed: u64,
        /// Fuel limit.
        limit: u64,
    },

    /// Refuel request denied.
    #[error("Refuel denied: {reason}")]
    RefuelDenied {
        /// Reason for denial.
        reason: String,
    },

    /// Execution timeout.
    #[error("Execution timeout after {elapsed:?}, limit was {limit:?}")]
    Timeout {
        /// Time elapsed.
        elapsed: std::time::Duration,
        /// Time limit.
        limit: std::time::Duration,
    },

    /// Stack overflow.
    #[error("Stack overflow")]
    StackOverflow,

    /// Table size exceeded.
    #[error("Table size exceeded: {current} elements, limit is {limit} elements")]
    TableSizeExceeded {
        /// Current table size.
        current: u32,
        /// Table size limit.
        limit: u32,
    },

    /// Epochs are disabled.
    #[error("Epoch-based interruption is disabled in the engine configuration")]
    EpochsDisabled,

    /// Fuel is disabled.
    #[error("Fuel consumption is disabled in the engine configuration")]
    FuelDisabled,

    /// Failed to spawn thread.
    #[error("Failed to spawn thread: {0}")]
    ThreadSpawnFailed(String),

    /// Configuration error.
    #[error("Invalid resource configuration: {0}")]
    InvalidConfig(String),
}

/// Result type for resource operations.
pub type ResourceResult<T> = std::result::Result<T, ResourceError>;
