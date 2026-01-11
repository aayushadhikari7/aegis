//! Observable events during sandbox execution.

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;

use aegis_capability::CapabilityId;
use crate::report::ExecutionOutcome;

/// Events that can be observed during sandbox execution.
#[derive(Debug, Clone)]
pub enum SandboxEvent {
    /// Module was loaded.
    ModuleLoaded {
        /// Module name, if available.
        name: Option<String>,
        /// Number of exports.
        export_count: usize,
    },
    /// Execution started.
    ExecutionStarted {
        /// Function being executed.
        function: String,
    },
    /// Host function was called.
    HostFunctionCalled {
        /// Module name.
        module: String,
        /// Function name.
        name: String,
        /// Call duration.
        duration: Duration,
    },
    /// Capability was checked.
    CapabilityChecked {
        /// Capability ID.
        id: CapabilityId,
        /// Action being checked.
        action: String,
        /// Whether it was permitted.
        permitted: bool,
    },
    /// Memory grew.
    MemoryGrew {
        /// Previous size in bytes.
        from_bytes: usize,
        /// New size in bytes.
        to_bytes: usize,
    },
    /// Fuel was consumed.
    FuelConsumed {
        /// Amount consumed.
        amount: u64,
        /// Remaining fuel.
        remaining: u64,
    },
    /// Execution completed.
    ExecutionCompleted {
        /// Function that completed.
        function: String,
        /// Execution outcome.
        outcome: ExecutionOutcome,
        /// Total duration.
        duration: Duration,
    },
    /// An error occurred.
    Error {
        /// Error message.
        message: String,
    },
    /// Custom event.
    Custom {
        /// Event name.
        name: String,
        /// Event data.
        data: serde_json::Value,
    },
}

impl SandboxEvent {
    /// Get the event type name.
    pub fn event_type(&self) -> &'static str {
        match self {
            SandboxEvent::ModuleLoaded { .. } => "module_loaded",
            SandboxEvent::ExecutionStarted { .. } => "execution_started",
            SandboxEvent::HostFunctionCalled { .. } => "host_function_called",
            SandboxEvent::CapabilityChecked { .. } => "capability_checked",
            SandboxEvent::MemoryGrew { .. } => "memory_grew",
            SandboxEvent::FuelConsumed { .. } => "fuel_consumed",
            SandboxEvent::ExecutionCompleted { .. } => "execution_completed",
            SandboxEvent::Error { .. } => "error",
            SandboxEvent::Custom { .. } => "custom",
        }
    }
}

/// Subscriber for sandbox events.
pub trait EventSubscriber: Send + Sync {
    /// Called when an event occurs.
    fn on_event(&self, event: &SandboxEvent);

    /// Filter for event types this subscriber is interested in.
    /// Returns true to receive all events.
    fn event_filter(&self) -> Option<Vec<&'static str>> {
        None // Receive all events by default
    }
}

/// A simple logging subscriber that logs events.
pub struct LoggingSubscriber {
    /// Minimum log level for events.
    pub log_level: tracing::Level,
}

impl LoggingSubscriber {
    /// Create a new logging subscriber.
    pub fn new() -> Self {
        Self {
            log_level: tracing::Level::DEBUG,
        }
    }

    /// Set the log level.
    pub fn with_level(mut self, level: tracing::Level) -> Self {
        self.log_level = level;
        self
    }
}

impl Default for LoggingSubscriber {
    fn default() -> Self {
        Self::new()
    }
}

impl EventSubscriber for LoggingSubscriber {
    fn on_event(&self, event: &SandboxEvent) {
        match event {
            SandboxEvent::ModuleLoaded { name, export_count } => {
                tracing::debug!(
                    event = "module_loaded",
                    name = ?name,
                    exports = export_count,
                    "Module loaded"
                );
            }
            SandboxEvent::ExecutionStarted { function } => {
                tracing::debug!(
                    event = "execution_started",
                    function = function,
                    "Execution started"
                );
            }
            SandboxEvent::HostFunctionCalled { module, name, duration } => {
                tracing::trace!(
                    event = "host_function_called",
                    module = module,
                    name = name,
                    duration_us = duration.as_micros(),
                    "Host function called"
                );
            }
            SandboxEvent::CapabilityChecked { id, action, permitted } => {
                if *permitted {
                    tracing::trace!(
                        event = "capability_checked",
                        capability = %id,
                        action = action,
                        permitted = permitted,
                        "Capability check passed"
                    );
                } else {
                    tracing::warn!(
                        event = "capability_checked",
                        capability = %id,
                        action = action,
                        permitted = permitted,
                        "Capability check failed"
                    );
                }
            }
            SandboxEvent::MemoryGrew { from_bytes, to_bytes } => {
                tracing::debug!(
                    event = "memory_grew",
                    from = from_bytes,
                    to = to_bytes,
                    "Memory grew"
                );
            }
            SandboxEvent::FuelConsumed { amount, remaining } => {
                tracing::trace!(
                    event = "fuel_consumed",
                    amount = amount,
                    remaining = remaining,
                    "Fuel consumed"
                );
            }
            SandboxEvent::ExecutionCompleted { function, outcome, duration } => {
                tracing::info!(
                    event = "execution_completed",
                    function = function,
                    success = outcome.is_success(),
                    duration_ms = duration.as_millis(),
                    "Execution completed"
                );
            }
            SandboxEvent::Error { message } => {
                tracing::error!(event = "error", message = message, "Error occurred");
            }
            SandboxEvent::Custom { name, data } => {
                tracing::debug!(
                    event = "custom",
                    name = name,
                    data = %data,
                    "Custom event"
                );
            }
        }
    }
}

/// A subscriber that collects events for later analysis.
pub struct CollectingSubscriber {
    events: RwLock<Vec<(Instant, SandboxEvent)>>,
    max_events: usize,
}

impl CollectingSubscriber {
    /// Create a new collecting subscriber.
    pub fn new(max_events: usize) -> Self {
        Self {
            events: RwLock::new(Vec::new()),
            max_events,
        }
    }

    /// Get collected events.
    pub fn events(&self) -> Vec<(Instant, SandboxEvent)> {
        self.events.read().clone()
    }

    /// Clear collected events.
    pub fn clear(&self) {
        self.events.write().clear();
    }

    /// Get event count.
    pub fn len(&self) -> usize {
        self.events.read().len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.events.read().is_empty()
    }
}

impl EventSubscriber for CollectingSubscriber {
    fn on_event(&self, event: &SandboxEvent) {
        let mut events = self.events.write();
        if events.len() < self.max_events {
            events.push((Instant::now(), event.clone()));
        }
    }
}

/// Event dispatcher that manages subscribers.
#[derive(Default)]
pub struct EventDispatcher {
    subscribers: RwLock<Vec<Arc<dyn EventSubscriber>>>,
}

impl EventDispatcher {
    /// Create a new event dispatcher.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a subscriber.
    pub fn subscribe(&self, subscriber: Arc<dyn EventSubscriber>) {
        self.subscribers.write().push(subscriber);
    }

    /// Remove all subscribers.
    pub fn clear_subscribers(&self) {
        self.subscribers.write().clear();
    }

    /// Get subscriber count.
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.read().len()
    }

    /// Emit an event to all subscribers.
    pub fn emit(&self, event: SandboxEvent) {
        let subscribers = self.subscribers.read();
        for subscriber in subscribers.iter() {
            // Check filter
            if let Some(filter) = subscriber.event_filter() {
                if !filter.contains(&event.event_type()) {
                    continue;
                }
            }
            subscriber.on_event(&event);
        }
    }
}

impl std::fmt::Debug for EventDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventDispatcher")
            .field("subscriber_count", &self.subscriber_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_event_type() {
        let event = SandboxEvent::ModuleLoaded {
            name: Some("test".to_string()),
            export_count: 5,
        };
        assert_eq!(event.event_type(), "module_loaded");
    }

    #[test]
    fn test_collecting_subscriber() {
        let subscriber = CollectingSubscriber::new(100);

        subscriber.on_event(&SandboxEvent::ExecutionStarted {
            function: "main".to_string(),
        });

        assert_eq!(subscriber.len(), 1);

        let events = subscriber.events();
        match &events[0].1 {
            SandboxEvent::ExecutionStarted { function } => {
                assert_eq!(function, "main");
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_collecting_subscriber_max_events() {
        let subscriber = CollectingSubscriber::new(2);

        for i in 0..5 {
            subscriber.on_event(&SandboxEvent::Custom {
                name: format!("event_{}", i),
                data: serde_json::Value::Null,
            });
        }

        assert_eq!(subscriber.len(), 2); // Should be capped at max
    }

    #[test]
    fn test_event_dispatcher() {
        let dispatcher = EventDispatcher::new();
        let collector = Arc::new(CollectingSubscriber::new(100));

        dispatcher.subscribe(Arc::clone(&collector) as Arc<dyn EventSubscriber>);

        dispatcher.emit(SandboxEvent::ExecutionStarted {
            function: "test".to_string(),
        });

        assert_eq!(collector.len(), 1);
    }

    #[test]
    fn test_event_dispatcher_multiple_subscribers() {
        let dispatcher = EventDispatcher::new();
        let collector1 = Arc::new(CollectingSubscriber::new(100));
        let collector2 = Arc::new(CollectingSubscriber::new(100));

        dispatcher.subscribe(Arc::clone(&collector1) as Arc<dyn EventSubscriber>);
        dispatcher.subscribe(Arc::clone(&collector2) as Arc<dyn EventSubscriber>);

        dispatcher.emit(SandboxEvent::Error {
            message: "test error".to_string(),
        });

        assert_eq!(collector1.len(), 1);
        assert_eq!(collector2.len(), 1);
    }
}
