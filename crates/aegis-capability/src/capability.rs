//! Core capability trait and types.
//!
//! This module defines the fundamental abstraction for capabilities in Aegis.
//! Capabilities are explicit, opt-in permissions that control what a sandboxed
//! module can do.

use std::borrow::Cow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::CapabilityError;

/// Unique identifier for a capability type.
///
/// Capability IDs are used to identify and look up capabilities in a set.
/// They should be unique and descriptive.
///
/// # Example
///
/// ```
/// use aegis_capability::CapabilityId;
///
/// let fs_cap = CapabilityId::new("filesystem");
/// let net_cap = CapabilityId::new("network");
///
/// assert_ne!(fs_cap, net_cap);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapabilityId(Cow<'static, str>);

impl CapabilityId {
    /// Create a new capability ID.
    pub fn new(id: impl Into<Cow<'static, str>>) -> Self {
        Self(id.into())
    }

    /// Get the ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PartialEq for CapabilityId {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for CapabilityId {}

impl Hash for CapabilityId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&'static str> for CapabilityId {
    fn from(s: &'static str) -> Self {
        Self::new(s)
    }
}

impl From<String> for CapabilityId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Represents an action that requires capability authorization.
///
/// Actions are checked against capabilities to determine if they are permitted.
/// Each capability type defines what actions it can authorize.
pub trait Action: fmt::Debug + Send + Sync {
    /// Get the type of this action (e.g., "fs:read", "net:connect").
    fn action_type(&self) -> &str;

    /// Get a human-readable description of the action.
    fn description(&self) -> String {
        format!("{:?}", self)
    }
}

/// Result of a permission check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionResult {
    /// The action is allowed.
    Allowed,
    /// The action is denied with a reason.
    Denied(DenialReason),
    /// The capability doesn't handle this action type; delegate to another.
    NotApplicable,
}

impl PermissionResult {
    /// Check if the result is allowed.
    pub fn is_allowed(&self) -> bool {
        matches!(self, PermissionResult::Allowed)
    }

    /// Check if the result is denied.
    pub fn is_denied(&self) -> bool {
        matches!(self, PermissionResult::Denied(_))
    }

    /// Convert to a Result type.
    pub fn to_result(&self) -> Result<(), CapabilityError> {
        match self {
            PermissionResult::Allowed => Ok(()),
            PermissionResult::Denied(reason) => Err(CapabilityError::PermissionDenied {
                reason: reason.clone(),
            }),
            PermissionResult::NotApplicable => Err(CapabilityError::NoCapabilityFound {
                action: "unknown".to_string(),
            }),
        }
    }
}

/// Reason for denying an action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenialReason {
    /// The capability that denied the action.
    pub capability: CapabilityId,
    /// Human-readable explanation.
    pub message: String,
    /// The action that was denied.
    pub action: String,
}

impl DenialReason {
    /// Create a new denial reason.
    pub fn new(
        capability: CapabilityId,
        action: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            capability,
            action: action.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for DenialReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} - {}",
            self.capability, self.action, self.message
        )
    }
}

/// Core trait for all capabilities.
///
/// Capabilities define what actions a sandboxed module is permitted to perform.
/// They follow the principle of least privilege: all permissions must be
/// explicitly granted.
///
/// # Implementing a Capability
///
/// ```ignore
/// use aegis_capability::{Capability, CapabilityId, Action, PermissionResult};
///
/// struct MyCapability {
///     allowed_operations: Vec<String>,
/// }
///
/// impl Capability for MyCapability {
///     fn id(&self) -> CapabilityId {
///         CapabilityId::new("my_capability")
///     }
///
///     fn name(&self) -> &str {
///         "My Capability"
///     }
///
///     fn description(&self) -> &str {
///         "Allows performing my operations"
///     }
///
///     fn permits(&self, action: &dyn Action) -> PermissionResult {
///         // Check if the action is allowed
///         if self.allowed_operations.contains(&action.action_type().to_string()) {
///             PermissionResult::Allowed
///         } else {
///             PermissionResult::Denied(DenialReason::new(
///                 self.id(),
///                 action.action_type(),
///                 "Operation not in allowed list",
///             ))
///         }
///     }
/// }
/// ```
pub trait Capability: Send + Sync + fmt::Debug {
    /// Get the unique identifier for this capability type.
    fn id(&self) -> CapabilityId;

    /// Get the human-readable name of this capability.
    fn name(&self) -> &str;

    /// Get a description of what this capability grants.
    fn description(&self) -> &str;

    /// Check if this capability permits a specific action.
    ///
    /// Returns:
    /// - `Allowed` if the action is permitted
    /// - `Denied` if the action is explicitly denied
    /// - `NotApplicable` if this capability doesn't handle this action type
    fn permits(&self, action: &dyn Action) -> PermissionResult;

    /// Get a list of action types this capability handles.
    ///
    /// This is used for documentation and validation purposes.
    fn handled_action_types(&self) -> Vec<&'static str> {
        Vec::new()
    }

    /// Called when the capability is added to a capability set.
    ///
    /// This can be used to perform validation or initialization.
    fn on_attach(&self) -> Result<(), CapabilityError> {
        Ok(())
    }

    /// Called when the capability is removed from a capability set.
    fn on_detach(&self) {}

    /// Validate that this capability's configuration is valid.
    fn validate(&self) -> Result<(), CapabilityError> {
        Ok(())
    }
}

/// A boxed capability trait object.
pub type BoxedCapability = Box<dyn Capability>;

/// A shared capability reference.
pub type SharedCapability = Arc<dyn Capability>;

/// Standard capability IDs for built-in capabilities.
pub mod standard_ids {
    use super::CapabilityId;

    /// Filesystem capability ID.
    pub const FILESYSTEM: CapabilityId = CapabilityId(std::borrow::Cow::Borrowed("filesystem"));

    /// Network capability ID.
    pub const NETWORK: CapabilityId = CapabilityId(std::borrow::Cow::Borrowed("network"));

    /// Logging capability ID.
    pub const LOGGING: CapabilityId = CapabilityId(std::borrow::Cow::Borrowed("logging"));

    /// Clock capability ID.
    pub const CLOCK: CapabilityId = CapabilityId(std::borrow::Cow::Borrowed("clock"));

    /// Environment variables capability ID.
    pub const ENV: CapabilityId = CapabilityId(std::borrow::Cow::Borrowed("env"));

    /// Random number generation capability ID.
    pub const RANDOM: CapabilityId = CapabilityId(std::borrow::Cow::Borrowed("random"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestAction {
        action_type: String,
    }

    impl Action for TestAction {
        fn action_type(&self) -> &str {
            &self.action_type
        }
    }

    #[derive(Debug)]
    struct TestCapability {
        allowed: Vec<String>,
    }

    impl Capability for TestCapability {
        fn id(&self) -> CapabilityId {
            CapabilityId::new("test")
        }

        fn name(&self) -> &str {
            "Test Capability"
        }

        fn description(&self) -> &str {
            "A test capability"
        }

        fn permits(&self, action: &dyn Action) -> PermissionResult {
            if self.allowed.contains(&action.action_type().to_string()) {
                PermissionResult::Allowed
            } else {
                PermissionResult::Denied(DenialReason::new(
                    self.id(),
                    action.action_type(),
                    "Not in allowed list",
                ))
            }
        }
    }

    #[test]
    fn test_capability_id() {
        let id1 = CapabilityId::new("test");
        let id2 = CapabilityId::new("test");
        let id3 = CapabilityId::new("other");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_capability_permits() {
        let cap = TestCapability {
            allowed: vec!["read".to_string(), "write".to_string()],
        };

        let read_action = TestAction {
            action_type: "read".to_string(),
        };
        let delete_action = TestAction {
            action_type: "delete".to_string(),
        };

        assert!(cap.permits(&read_action).is_allowed());
        assert!(cap.permits(&delete_action).is_denied());
    }

    #[test]
    fn test_permission_result_to_result() {
        let allowed = PermissionResult::Allowed;
        assert!(allowed.to_result().is_ok());

        let denied = PermissionResult::Denied(DenialReason::new(
            CapabilityId::new("test"),
            "action",
            "reason",
        ));
        assert!(denied.to_result().is_err());
    }

    #[test]
    fn test_standard_ids() {
        assert_eq!(standard_ids::FILESYSTEM.as_str(), "filesystem");
        assert_eq!(standard_ids::NETWORK.as_str(), "network");
        assert_eq!(standard_ids::LOGGING.as_str(), "logging");
    }
}
