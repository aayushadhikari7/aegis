//! Capability set management.
//!
//! This module provides the `CapabilitySet` type, which holds a collection
//! of capabilities and provides methods for permission checking.

use std::sync::Arc;

use dashmap::DashMap;
use tracing::{debug, info, warn};

use crate::capability::{
    Action, BoxedCapability, Capability, CapabilityId, DenialReason, PermissionResult,
    SharedCapability,
};
use crate::error::{CapabilityError, CapabilityResult};

/// A set of capabilities granted to a sandbox.
///
/// `CapabilitySet` manages a collection of capabilities and provides
/// methods for checking permissions against actions.
///
/// # Example
///
/// ```ignore
/// use aegis_capability::{CapabilitySet, CapabilitySetBuilder};
/// use aegis_capability::builtin::LoggingCapability;
///
/// let mut set = CapabilitySet::new();
/// set.grant(LoggingCapability::new())?;
///
/// // Check permissions
/// let result = set.check_permission(&some_action);
/// ```
#[derive(Default)]
pub struct CapabilitySet {
    /// Map of capability ID to capability.
    capabilities: DashMap<CapabilityId, SharedCapability>,
}

impl CapabilitySet {
    /// Create an empty capability set.
    pub fn new() -> Self {
        Self {
            capabilities: DashMap::new(),
        }
    }

    /// Create a capability set with the given capabilities.
    pub fn with_capabilities(capabilities: Vec<BoxedCapability>) -> CapabilityResult<Self> {
        let set = Self::new();
        for cap in capabilities {
            set.grant_boxed(cap)?;
        }
        Ok(set)
    }

    /// Grant a capability to this set.
    ///
    /// # Errors
    ///
    /// Returns an error if a capability with the same ID already exists.
    pub fn grant<C: Capability + 'static>(&self, capability: C) -> CapabilityResult<()> {
        self.grant_shared(Arc::new(capability))
    }

    /// Grant a boxed capability.
    pub fn grant_boxed(&self, capability: BoxedCapability) -> CapabilityResult<()> {
        let id = capability.id();

        if self.capabilities.contains_key(&id) {
            return Err(CapabilityError::AlreadyExists(id));
        }

        capability.validate()?;
        capability.on_attach()?;

        let shared: SharedCapability = capability.into();
        self.capabilities.insert(id.clone(), shared);

        info!(capability = %id, "Capability granted");
        Ok(())
    }

    /// Grant a shared capability.
    pub fn grant_shared(&self, capability: SharedCapability) -> CapabilityResult<()> {
        let id = capability.id();

        if self.capabilities.contains_key(&id) {
            return Err(CapabilityError::AlreadyExists(id));
        }

        capability.validate()?;
        capability.on_attach()?;

        self.capabilities.insert(id.clone(), capability);

        info!(capability = %id, "Capability granted");
        Ok(())
    }

    /// Revoke a capability from this set.
    pub fn revoke(&self, id: &CapabilityId) -> Option<SharedCapability> {
        self.capabilities.remove(id).map(|(_, cap)| {
            cap.on_detach();
            info!(capability = %id, "Capability revoked");
            cap
        })
    }

    /// Check if a capability is granted.
    pub fn has(&self, id: &CapabilityId) -> bool {
        self.capabilities.contains_key(id)
    }

    /// Get a capability by ID.
    pub fn get(&self, id: &CapabilityId) -> Option<SharedCapability> {
        self.capabilities.get(id).map(|r| Arc::clone(r.value()))
    }

    /// Get the number of capabilities in the set.
    pub fn len(&self) -> usize {
        self.capabilities.len()
    }

    /// Check if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }

    /// Get all capability IDs.
    pub fn ids(&self) -> Vec<CapabilityId> {
        self.capabilities.iter().map(|r| r.key().clone()).collect()
    }

    /// Check if an action is permitted by any capability in the set.
    ///
    /// This iterates through all capabilities until one either allows or
    /// denies the action. If all capabilities return `NotApplicable`,
    /// the action is denied.
    pub fn check_permission(&self, action: &dyn Action) -> PermissionResult {
        debug!(action_type = action.action_type(), "Checking permission");

        let mut denial: Option<DenialReason> = None;

        for entry in self.capabilities.iter() {
            let result = entry.value().permits(action);

            match result {
                PermissionResult::Allowed => {
                    debug!(
                        capability = %entry.key(),
                        action_type = action.action_type(),
                        "Permission allowed"
                    );
                    return PermissionResult::Allowed;
                }
                PermissionResult::Denied(reason) => {
                    debug!(
                        capability = %entry.key(),
                        action_type = action.action_type(),
                        reason = %reason,
                        "Permission denied"
                    );
                    // Keep the first denial reason
                    if denial.is_none() {
                        denial = Some(reason);
                    }
                }
                PermissionResult::NotApplicable => {
                    // This capability doesn't handle this action type
                    continue;
                }
            }
        }

        // If we have an explicit denial, return it
        if let Some(reason) = denial {
            return PermissionResult::Denied(reason);
        }

        // No capability handled this action - deny by default
        warn!(
            action_type = action.action_type(),
            "No capability found for action"
        );

        PermissionResult::Denied(DenialReason {
            capability: CapabilityId::new("none"),
            action: action.action_type().to_string(),
            message: "No capability grants this permission".to_string(),
        })
    }

    /// Require that an action is permitted.
    ///
    /// Returns `Ok(())` if the action is allowed, or an error if denied.
    pub fn require(&self, action: &dyn Action) -> CapabilityResult<()> {
        self.check_permission(action).to_result()
    }

    /// Validate that all capabilities in the set are compatible.
    pub fn validate(&self) -> CapabilityResult<()> {
        for entry in self.capabilities.iter() {
            entry.value().validate()?;
        }
        Ok(())
    }

    /// Clear all capabilities from the set.
    pub fn clear(&self) {
        for entry in self.capabilities.iter() {
            entry.value().on_detach();
        }
        self.capabilities.clear();
        info!("Capability set cleared");
    }

    /// Iterate over all capabilities.
    pub fn iter(&self) -> impl Iterator<Item = SharedCapability> + '_ {
        self.capabilities.iter().map(|r| Arc::clone(r.value()))
    }
}

impl Clone for CapabilitySet {
    fn clone(&self) -> Self {
        let new_set = Self::new();
        for entry in self.capabilities.iter() {
            new_set
                .capabilities
                .insert(entry.key().clone(), Arc::clone(entry.value()));
        }
        new_set
    }
}

impl std::fmt::Debug for CapabilitySet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapabilitySet")
            .field("capabilities", &self.ids())
            .finish()
    }
}

/// Builder for constructing capability sets.
#[derive(Default)]
pub struct CapabilitySetBuilder {
    capabilities: Vec<BoxedCapability>,
}

impl CapabilitySetBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a capability.
    pub fn with<C: Capability + 'static>(mut self, capability: C) -> Self {
        self.capabilities.push(Box::new(capability));
        self
    }

    /// Add a boxed capability.
    pub fn with_boxed(mut self, capability: BoxedCapability) -> Self {
        self.capabilities.push(capability);
        self
    }

    /// Build the capability set.
    pub fn build(self) -> CapabilityResult<CapabilitySet> {
        CapabilitySet::with_capabilities(self.capabilities)
    }
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
    struct AllowAllCapability;

    impl Capability for AllowAllCapability {
        fn id(&self) -> CapabilityId {
            CapabilityId::new("allow_all")
        }

        fn name(&self) -> &str {
            "Allow All"
        }

        fn description(&self) -> &str {
            "Allows all actions"
        }

        fn permits(&self, _action: &dyn Action) -> PermissionResult {
            PermissionResult::Allowed
        }
    }

    #[derive(Debug)]
    struct DenyAllCapability;

    impl Capability for DenyAllCapability {
        fn id(&self) -> CapabilityId {
            CapabilityId::new("deny_all")
        }

        fn name(&self) -> &str {
            "Deny All"
        }

        fn description(&self) -> &str {
            "Denies all actions"
        }

        fn permits(&self, action: &dyn Action) -> PermissionResult {
            PermissionResult::Denied(DenialReason::new(
                self.id(),
                action.action_type(),
                "All actions denied",
            ))
        }
    }

    #[test]
    fn test_empty_set() {
        let set = CapabilitySet::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn test_grant_capability() {
        let set = CapabilitySet::new();
        set.grant(AllowAllCapability).unwrap();

        assert!(set.has(&CapabilityId::new("allow_all")));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_grant_duplicate() {
        let set = CapabilitySet::new();
        set.grant(AllowAllCapability).unwrap();

        let result = set.grant(AllowAllCapability);
        assert!(result.is_err());
    }

    #[test]
    fn test_revoke_capability() {
        let set = CapabilitySet::new();
        set.grant(AllowAllCapability).unwrap();

        let removed = set.revoke(&CapabilityId::new("allow_all"));
        assert!(removed.is_some());
        assert!(set.is_empty());
    }

    #[test]
    fn test_check_permission_allowed() {
        let set = CapabilitySet::new();
        set.grant(AllowAllCapability).unwrap();

        let action = TestAction {
            action_type: "test".to_string(),
        };
        let result = set.check_permission(&action);
        assert!(result.is_allowed());
    }

    #[test]
    fn test_check_permission_denied() {
        let set = CapabilitySet::new();
        set.grant(DenyAllCapability).unwrap();

        let action = TestAction {
            action_type: "test".to_string(),
        };
        let result = set.check_permission(&action);
        assert!(result.is_denied());
    }

    #[test]
    fn test_empty_set_denies() {
        let set = CapabilitySet::new();

        let action = TestAction {
            action_type: "test".to_string(),
        };
        let result = set.check_permission(&action);
        assert!(result.is_denied());
    }

    #[test]
    fn test_builder() {
        let set = CapabilitySetBuilder::new()
            .with(AllowAllCapability)
            .build()
            .unwrap();

        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_clone() {
        let set = CapabilitySet::new();
        set.grant(AllowAllCapability).unwrap();

        let cloned = set.clone();
        assert_eq!(cloned.len(), 1);
        assert!(cloned.has(&CapabilityId::new("allow_all")));
    }
}
