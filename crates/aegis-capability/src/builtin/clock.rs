//! Clock capability for time access.

use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::capability::{
    Action, Capability, CapabilityId, DenialReason, PermissionResult, standard_ids,
};

/// Type of clock to provide.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClockType {
    /// Real system time.
    RealTime,
    /// Monotonic clock (for duration measurement).
    Monotonic,
    /// Fixed/mocked time (for deterministic execution).
    Fixed(u64), // Unix timestamp in nanoseconds
    /// No clock access (time functions return errors).
    None,
}

impl Default for ClockType {
    fn default() -> Self {
        ClockType::Monotonic
    }
}

/// Actions related to clock/time operations.
#[derive(Debug, Clone)]
pub enum ClockAction {
    /// Get the current time.
    GetTime { clock_type: String },
    /// Get clock resolution.
    GetResolution { clock_type: String },
}

impl Action for ClockAction {
    fn action_type(&self) -> &str {
        match self {
            ClockAction::GetTime { .. } => "clock:time",
            ClockAction::GetResolution { .. } => "clock:resolution",
        }
    }

    fn description(&self) -> String {
        match self {
            ClockAction::GetTime { clock_type } => format!("Get {} time", clock_type),
            ClockAction::GetResolution { clock_type } => {
                format!("Get {} clock resolution", clock_type)
            }
        }
    }
}

/// Capability for clock/time access.
///
/// This capability controls access to time-related functions.
///
/// # Example
///
/// ```
/// use aegis_capability::builtin::{ClockCapability, ClockType};
///
/// // Allow monotonic clock only (for timing measurements)
/// let cap = ClockCapability::monotonic_only();
///
/// // Allow real-time clock
/// let cap = ClockCapability::new(ClockType::RealTime);
///
/// // Fixed time for deterministic tests
/// let cap = ClockCapability::fixed(1704067200_000_000_000); // 2024-01-01 00:00:00 UTC
/// ```
#[derive(Debug, Clone)]
pub struct ClockCapability {
    /// Type of clock to provide.
    clock_type: ClockType,
    /// Allow real-time clock access.
    allow_realtime: bool,
    /// Allow monotonic clock access.
    allow_monotonic: bool,
}

impl ClockCapability {
    /// Create a new clock capability with the given clock type.
    pub fn new(clock_type: ClockType) -> Self {
        let (allow_realtime, allow_monotonic) = match &clock_type {
            ClockType::RealTime => (true, true),
            ClockType::Monotonic => (false, true),
            ClockType::Fixed(_) => (true, true), // Fixed provides both
            ClockType::None => (false, false),
        };

        Self {
            clock_type,
            allow_realtime,
            allow_monotonic,
        }
    }

    /// Create a capability that only allows monotonic clock.
    pub fn monotonic_only() -> Self {
        Self::new(ClockType::Monotonic)
    }

    /// Create a capability that allows real-time clock.
    pub fn realtime() -> Self {
        Self::new(ClockType::RealTime)
    }

    /// Create a capability with a fixed time value.
    pub fn fixed(timestamp_nanos: u64) -> Self {
        Self::new(ClockType::Fixed(timestamp_nanos))
    }

    /// Create a capability that denies all clock access.
    pub fn none() -> Self {
        Self::new(ClockType::None)
    }

    /// Get the clock type.
    pub fn clock_type(&self) -> &ClockType {
        &self.clock_type
    }

    /// Check if real-time clock access is allowed.
    pub fn allows_realtime(&self) -> bool {
        self.allow_realtime
    }

    /// Check if monotonic clock access is allowed.
    pub fn allows_monotonic(&self) -> bool {
        self.allow_monotonic
    }

    /// Get the current time value.
    ///
    /// Returns the timestamp in nanoseconds, or None if clock access is denied.
    pub fn get_time(&self) -> Option<u64> {
        match &self.clock_type {
            ClockType::RealTime => {
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_nanos() as u64)
            }
            ClockType::Monotonic => {
                // For monotonic, we'd use std::time::Instant in real code
                // Here we use system time as a placeholder
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_nanos() as u64)
            }
            ClockType::Fixed(timestamp) => Some(*timestamp),
            ClockType::None => None,
        }
    }
}

impl Capability for ClockCapability {
    fn id(&self) -> CapabilityId {
        standard_ids::CLOCK.clone()
    }

    fn name(&self) -> &str {
        "Clock"
    }

    fn description(&self) -> &str {
        "Allows access to time/clock functions"
    }

    fn permits(&self, action: &dyn Action) -> PermissionResult {
        let action_type = action.action_type();
        if !action_type.starts_with("clock:") {
            return PermissionResult::NotApplicable;
        }

        // Check if any clock access is allowed
        if matches!(self.clock_type, ClockType::None) {
            return PermissionResult::Denied(DenialReason::new(
                self.id(),
                action_type,
                "Clock access is disabled",
            ));
        }

        PermissionResult::Allowed
    }

    fn handled_action_types(&self) -> Vec<&'static str> {
        vec!["clock:time", "clock:resolution"]
    }
}

/// Helper function to check clock permission with a concrete action.
pub fn check_clock_permission(
    capability: &ClockCapability,
    action: &ClockAction,
) -> PermissionResult {
    match action {
        ClockAction::GetTime { clock_type } | ClockAction::GetResolution { clock_type } => {
            let allowed = match clock_type.as_str() {
                "realtime" => capability.allows_realtime(),
                "monotonic" => capability.allows_monotonic(),
                _ => false,
            };

            if allowed {
                PermissionResult::Allowed
            } else {
                PermissionResult::Denied(DenialReason::new(
                    capability.id(),
                    action.action_type(),
                    format!("Clock type '{}' is not allowed", clock_type),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_capability_monotonic() {
        let cap = ClockCapability::monotonic_only();
        assert!(cap.allows_monotonic());
        assert!(!cap.allows_realtime());
    }

    #[test]
    fn test_clock_capability_realtime() {
        let cap = ClockCapability::realtime();
        assert!(cap.allows_realtime());
        assert!(cap.allows_monotonic());
    }

    #[test]
    fn test_clock_capability_fixed() {
        let timestamp = 1704067200_000_000_000u64;
        let cap = ClockCapability::fixed(timestamp);

        assert_eq!(cap.get_time(), Some(timestamp));
    }

    #[test]
    fn test_clock_capability_none() {
        let cap = ClockCapability::none();
        assert!(!cap.allows_realtime());
        assert!(!cap.allows_monotonic());
        assert_eq!(cap.get_time(), None);
    }

    #[test]
    fn test_check_clock_permission() {
        let cap = ClockCapability::monotonic_only();

        let allowed = ClockAction::GetTime {
            clock_type: "monotonic".to_string(),
        };
        assert!(check_clock_permission(&cap, &allowed).is_allowed());

        let denied = ClockAction::GetTime {
            clock_type: "realtime".to_string(),
        };
        assert!(check_clock_permission(&cap, &denied).is_denied());
    }
}
