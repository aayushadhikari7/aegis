//! Logging capability for log output.

use serde::{Deserialize, Serialize};

use crate::capability::{
    Action, Capability, CapabilityId, DenialReason, PermissionResult, standard_ids,
};

/// Log levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum LogLevel {
    /// Trace level (most verbose).
    Trace = 0,
    /// Debug level.
    Debug = 1,
    /// Info level.
    #[default]
    Info = 2,
    /// Warning level.
    Warn = 3,
    /// Error level.
    Error = 4,
}

impl LogLevel {
    /// Get the level name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

/// Actions related to logging.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum LoggingAction {
    /// Write a log message.
    Log { level: LogLevel, message_len: usize },
}

impl Action for LoggingAction {
    fn action_type(&self) -> &str {
        "log:write"
    }

    fn description(&self) -> String {
        match self {
            LoggingAction::Log { level, message_len } => {
                format!("Log {} message ({} bytes)", level.as_str(), message_len)
            }
        }
    }
}

/// Capability for logging output.
///
/// This capability controls what log messages a guest can emit.
///
/// # Example
///
/// ```
/// use aegis_capability::builtin::{LoggingCapability, LogLevel};
///
/// // Allow info level and above
/// let cap = LoggingCapability::new(LogLevel::Info, 4096);
/// ```
#[derive(Debug, Clone)]
pub struct LoggingCapability {
    /// Minimum log level allowed.
    min_level: LogLevel,
    /// Maximum message size in bytes.
    max_message_size: usize,
    /// Maximum messages per second (rate limiting).
    max_rate: Option<u32>,
}

impl LoggingCapability {
    /// Create a new logging capability.
    pub fn new(min_level: LogLevel, max_message_size: usize) -> Self {
        Self {
            min_level,
            max_message_size,
            max_rate: None,
        }
    }

    /// Create a capability that allows all log levels.
    pub fn allow_all() -> Self {
        Self::new(LogLevel::Trace, 64 * 1024)
    }

    /// Create a capability for production use (info and above).
    pub fn production() -> Self {
        Self::new(LogLevel::Info, 4096)
    }

    /// Set rate limiting.
    pub fn with_rate_limit(mut self, max_per_second: u32) -> Self {
        self.max_rate = Some(max_per_second);
        self
    }

    /// Get the minimum log level.
    pub fn min_level(&self) -> LogLevel {
        self.min_level
    }

    /// Get the maximum message size.
    pub fn max_message_size(&self) -> usize {
        self.max_message_size
    }

    /// Check if a log level is allowed.
    pub fn is_level_allowed(&self, level: LogLevel) -> bool {
        level >= self.min_level
    }
}

impl Capability for LoggingCapability {
    fn id(&self) -> CapabilityId {
        standard_ids::LOGGING.clone()
    }

    fn name(&self) -> &str {
        "Logging"
    }

    fn description(&self) -> &str {
        "Allows logging output"
    }

    fn permits(&self, action: &dyn Action) -> PermissionResult {
        if action.action_type() != "log:write" {
            return PermissionResult::NotApplicable;
        }
        PermissionResult::NotApplicable
    }

    fn handled_action_types(&self) -> Vec<&'static str> {
        vec!["log:write"]
    }
}

/// Helper function to check logging permission with a concrete action.
#[allow(dead_code)]
pub fn check_logging_permission(
    capability: &LoggingCapability,
    action: &LoggingAction,
) -> PermissionResult {
    match action {
        LoggingAction::Log { level, message_len } => {
            if !capability.is_level_allowed(*level) {
                return PermissionResult::Denied(DenialReason::new(
                    capability.id(),
                    action.action_type(),
                    format!(
                        "Log level {} is below minimum {}",
                        level.as_str(),
                        capability.min_level().as_str()
                    ),
                ));
            }

            if *message_len > capability.max_message_size() {
                return PermissionResult::Denied(DenialReason::new(
                    capability.id(),
                    action.action_type(),
                    format!(
                        "Message size {} exceeds maximum {}",
                        message_len,
                        capability.max_message_size()
                    ),
                ));
            }

            PermissionResult::Allowed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn test_logging_capability_level_check() {
        let cap = LoggingCapability::new(LogLevel::Info, 4096);

        assert!(!cap.is_level_allowed(LogLevel::Trace));
        assert!(!cap.is_level_allowed(LogLevel::Debug));
        assert!(cap.is_level_allowed(LogLevel::Info));
        assert!(cap.is_level_allowed(LogLevel::Warn));
        assert!(cap.is_level_allowed(LogLevel::Error));
    }

    #[test]
    fn test_check_logging_permission() {
        let cap = LoggingCapability::new(LogLevel::Info, 1000);

        let allowed = LoggingAction::Log {
            level: LogLevel::Info,
            message_len: 100,
        };
        assert!(check_logging_permission(&cap, &allowed).is_allowed());

        let denied_level = LoggingAction::Log {
            level: LogLevel::Debug,
            message_len: 100,
        };
        assert!(check_logging_permission(&cap, &denied_level).is_denied());

        let denied_size = LoggingAction::Log {
            level: LogLevel::Error,
            message_len: 2000,
        };
        assert!(check_logging_permission(&cap, &denied_size).is_denied());
    }
}
