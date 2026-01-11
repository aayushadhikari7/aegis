//! Error types for the capability system.

use thiserror::Error;

use crate::capability::{CapabilityId, DenialReason};

/// Errors related to capabilities.
#[derive(Debug, Error)]
pub enum CapabilityError {
    /// Permission was denied by a capability.
    #[error("Permission denied: {reason}")]
    PermissionDenied {
        /// The reason for denial.
        reason: DenialReason,
    },

    /// No capability was found that handles the action.
    #[error("No capability found for action: {action}")]
    NoCapabilityFound {
        /// The action that was attempted.
        action: String,
    },

    /// A capability was not granted.
    #[error("Capability not granted: {0}")]
    NotGranted(CapabilityId),

    /// Two capabilities conflict with each other.
    #[error("Capability conflict: {0} conflicts with {1}")]
    Conflict(CapabilityId, CapabilityId),

    /// Invalid capability configuration.
    #[error("Invalid capability configuration: {0}")]
    InvalidConfig(String),

    /// A capability with this ID already exists.
    #[error("Capability already exists: {0}")]
    AlreadyExists(CapabilityId),

    /// Capability validation failed.
    #[error("Capability validation failed: {0}")]
    ValidationFailed(String),

    /// Ambient authority violation detected.
    #[error("Ambient authority violation: {message}")]
    AmbientAuthorityViolation {
        /// Description of the violation.
        message: String,
    },
}

/// Result type for capability operations.
pub type CapabilityResult<T> = std::result::Result<T, CapabilityError>;
