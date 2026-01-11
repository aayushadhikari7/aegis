//! Aegis Observability
//!
//! This crate provides observability features for the Aegis WebAssembly
//! sandbox runtime, including:
//!
//! - [`MetricsCollector`]: Collects execution metrics
//! - [`ExecutionReport`]: Complete execution reports
//! - [`EventDispatcher`]: Observable event system
//!
//! # Metrics Collection
//!
//! ```ignore
//! use aegis_observe::MetricsCollector;
//!
//! let collector = MetricsCollector::new();
//! collector.record_start();
//! // ... execute WASM ...
//! collector.record_end();
//!
//! let snapshot = collector.snapshot();
//! println!("Execution time: {:?}", snapshot.timing.execution_time);
//! ```
//!
//! # Execution Reports
//!
//! ```ignore
//! use aegis_observe::{ExecutionReport, ExecutionOutcome, ModuleInfo};
//!
//! let report = ExecutionReport::new(
//!     module_info,
//!     ExecutionOutcome::Success { return_value: None },
//!     metrics.snapshot(),
//! );
//!
//! println!("{}", report.to_text());
//! ```
//!
//! # Event Subscription
//!
//! ```ignore
//! use aegis_observe::{EventDispatcher, LoggingSubscriber};
//! use std::sync::Arc;
//!
//! let dispatcher = EventDispatcher::new();
//! dispatcher.subscribe(Arc::new(LoggingSubscriber::new()));
//!
//! dispatcher.emit(SandboxEvent::ExecutionStarted {
//!     function: "main".to_string(),
//! });
//! ```

pub mod events;
pub mod metrics;
pub mod report;

// Re-export main types
pub use events::{
    CollectingSubscriber, EventDispatcher, EventSubscriber, LoggingSubscriber, SandboxEvent,
};
pub use metrics::{
    CapabilityUsageMetrics, FuelMetrics, HostCallMetrics, MemoryMetrics, MetricsCollector,
    MetricsSnapshot, TimingMetrics,
};
pub use report::{
    Diagnostic, DiagnosticLevel, ExecutionId, ExecutionOutcome, ExecutionReport, ModuleInfo,
    ResourceType, TrapInfo,
};

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::events::{EventDispatcher, EventSubscriber, SandboxEvent};
    pub use crate::metrics::{MetricsCollector, MetricsSnapshot};
    pub use crate::report::{ExecutionOutcome, ExecutionReport, ModuleInfo};
}
