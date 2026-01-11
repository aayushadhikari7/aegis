//! Execution reports.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use aegis_capability::CapabilityId;
use crate::metrics::MetricsSnapshot;

/// Unique identifier for an execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionId(Uuid);

impl ExecutionId {
    /// Create a new random execution ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ExecutionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ExecutionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Information about a module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    /// Module name, if set.
    pub name: Option<String>,
    /// Number of exports.
    pub export_count: usize,
    /// Number of imports.
    pub import_count: usize,
}

/// Result of an execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionOutcome {
    /// Execution completed successfully.
    Success {
        /// Return value, if any (serialized as JSON).
        return_value: Option<serde_json::Value>,
    },
    /// Execution trapped.
    Trapped {
        /// Trap information.
        trap: TrapInfo,
    },
    /// Execution timed out.
    Timeout {
        /// Time elapsed before timeout.
        elapsed: Duration,
        /// The timeout limit.
        limit: Duration,
    },
    /// Resource was exhausted.
    ResourceExhausted {
        /// Which resource was exhausted.
        resource: ResourceType,
        /// Amount used.
        used: u64,
        /// The limit.
        limit: u64,
    },
    /// Capability was denied.
    CapabilityDenied {
        /// The capability that denied.
        capability: CapabilityId,
        /// The action that was attempted.
        action: String,
    },
    /// Generic error.
    Error {
        /// Error message.
        message: String,
    },
}

impl ExecutionOutcome {
    /// Check if the outcome is successful.
    pub fn is_success(&self) -> bool {
        matches!(self, ExecutionOutcome::Success { .. })
    }

    /// Check if the outcome is a failure.
    pub fn is_failure(&self) -> bool {
        !self.is_success()
    }
}

/// Information about a trap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapInfo {
    /// Trap code name.
    pub code: Option<String>,
    /// Trap message.
    pub message: String,
    /// Stack backtrace, if available.
    pub backtrace: Option<String>,
}

/// Type of resource that was exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceType {
    /// Memory.
    Memory,
    /// Fuel (CPU time).
    Fuel,
    /// Wall-clock time.
    Time,
    /// Stack space.
    Stack,
    /// Table elements.
    Table,
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceType::Memory => write!(f, "memory"),
            ResourceType::Fuel => write!(f, "fuel"),
            ResourceType::Time => write!(f, "time"),
            ResourceType::Stack => write!(f, "stack"),
            ResourceType::Table => write!(f, "table"),
        }
    }
}

/// A diagnostic message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Severity level.
    pub level: DiagnosticLevel,
    /// Message.
    pub message: String,
    /// Additional context.
    pub context: Option<String>,
}

/// Diagnostic severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticLevel {
    /// Informational.
    Info,
    /// Warning.
    Warning,
    /// Error.
    Error,
}

/// Complete execution report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReport {
    /// Unique execution ID.
    pub execution_id: ExecutionId,
    /// Module information.
    pub module: ModuleInfo,
    /// Execution outcome.
    pub outcome: ExecutionOutcome,
    /// Collected metrics.
    pub metrics: MetricsSnapshot,
    /// Diagnostic messages.
    pub diagnostics: Vec<Diagnostic>,
}

impl ExecutionReport {
    /// Create a new execution report.
    pub fn new(
        module: ModuleInfo,
        outcome: ExecutionOutcome,
        metrics: MetricsSnapshot,
    ) -> Self {
        Self {
            execution_id: ExecutionId::new(),
            module,
            outcome,
            metrics,
            diagnostics: Vec::new(),
        }
    }

    /// Add a diagnostic message.
    pub fn add_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Add an info diagnostic.
    pub fn add_info(&mut self, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Info,
            message: message.into(),
            context: None,
        });
    }

    /// Add a warning diagnostic.
    pub fn add_warning(&mut self, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Warning,
            message: message.into(),
            context: None,
        });
    }

    /// Add an error diagnostic.
    pub fn add_error(&mut self, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Error,
            message: message.into(),
            context: None,
        });
    }

    /// Check if execution was successful.
    pub fn is_success(&self) -> bool {
        self.outcome.is_success()
    }

    /// Format as human-readable text.
    pub fn to_text(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("Execution Report: {}\n", self.execution_id));
        output.push_str(&format!("Module: {:?}\n", self.module.name));
        output.push_str("\n");

        output.push_str(&format!("Outcome: "));
        match &self.outcome {
            ExecutionOutcome::Success { return_value } => {
                output.push_str("Success\n");
                if let Some(value) = return_value {
                    output.push_str(&format!("  Return: {}\n", value));
                }
            }
            ExecutionOutcome::Trapped { trap } => {
                output.push_str(&format!("Trapped: {}\n", trap.message));
            }
            ExecutionOutcome::Timeout { elapsed, limit } => {
                output.push_str(&format!("Timeout: {:?} / {:?}\n", elapsed, limit));
            }
            ExecutionOutcome::ResourceExhausted { resource, used, limit } => {
                output.push_str(&format!(
                    "Resource Exhausted: {} ({} / {})\n",
                    resource, used, limit
                ));
            }
            ExecutionOutcome::CapabilityDenied { capability, action } => {
                output.push_str(&format!(
                    "Capability Denied: {} for action '{}'\n",
                    capability, action
                ));
            }
            ExecutionOutcome::Error { message } => {
                output.push_str(&format!("Error: {}\n", message));
            }
        }

        output.push_str("\n");
        output.push_str("Metrics:\n");
        output.push_str(&format!(
            "  Execution Time: {:?}\n",
            self.metrics.timing.execution_time
        ));
        output.push_str(&format!(
            "  Peak Memory: {} bytes\n",
            self.metrics.memory.peak_memory
        ));
        output.push_str(&format!(
            "  Fuel Consumed: {}\n",
            self.metrics.fuel.consumed_fuel
        ));

        if !self.diagnostics.is_empty() {
            output.push_str("\nDiagnostics:\n");
            for diag in &self.diagnostics {
                let level = match diag.level {
                    DiagnosticLevel::Info => "INFO",
                    DiagnosticLevel::Warning => "WARN",
                    DiagnosticLevel::Error => "ERROR",
                };
                output.push_str(&format!("  [{}] {}\n", level, diag.message));
            }
        }

        output
    }

    /// Format as JSON.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// Format as pretty JSON string.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricsCollector;

    #[test]
    fn test_execution_id() {
        let id1 = ExecutionId::new();
        let id2 = ExecutionId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_execution_outcome_is_success() {
        let success = ExecutionOutcome::Success { return_value: None };
        assert!(success.is_success());

        let failure = ExecutionOutcome::Error {
            message: "test".to_string(),
        };
        assert!(failure.is_failure());
    }

    #[test]
    fn test_execution_report_creation() {
        let module = ModuleInfo {
            name: Some("test".to_string()),
            export_count: 5,
            import_count: 2,
        };
        let outcome = ExecutionOutcome::Success { return_value: None };
        let metrics = MetricsCollector::new().snapshot();

        let report = ExecutionReport::new(module, outcome, metrics);
        assert!(report.is_success());
    }

    #[test]
    fn test_execution_report_diagnostics() {
        let module = ModuleInfo {
            name: None,
            export_count: 0,
            import_count: 0,
        };
        let metrics = MetricsCollector::new().snapshot();
        let mut report = ExecutionReport::new(
            module,
            ExecutionOutcome::Success { return_value: None },
            metrics,
        );

        report.add_info("Test info");
        report.add_warning("Test warning");

        assert_eq!(report.diagnostics.len(), 2);
    }

    #[test]
    fn test_execution_report_to_text() {
        let module = ModuleInfo {
            name: Some("test_module".to_string()),
            export_count: 1,
            import_count: 0,
        };
        let metrics = MetricsCollector::new().snapshot();
        let report = ExecutionReport::new(
            module,
            ExecutionOutcome::Success { return_value: None },
            metrics,
        );

        let text = report.to_text();
        assert!(text.contains("test_module"));
        assert!(text.contains("Success"));
    }
}
