//! Metrics collection during sandbox execution.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use aegis_capability::CapabilityId;

/// Collects metrics during sandbox execution.
#[derive(Default)]
pub struct MetricsCollector {
    /// Timing metrics.
    timing: RwLock<TimingMetrics>,
    /// Memory metrics.
    memory: RwLock<MemoryMetrics>,
    /// Fuel metrics.
    fuel: RwLock<FuelMetrics>,
    /// Capability usage metrics.
    capability_usage: RwLock<CapabilityUsageMetrics>,
    /// Host call metrics.
    host_calls: RwLock<HostCallMetrics>,
}

impl MetricsCollector {
    /// Create a new metrics collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the start of execution.
    pub fn record_start(&self) {
        let mut timing = self.timing.write();
        timing.start_time = Some(Instant::now());
    }

    /// Record the end of execution.
    pub fn record_end(&self) {
        let mut timing = self.timing.write();
        timing.end_time = Some(Instant::now());
        if let (Some(start), Some(end)) = (timing.start_time, timing.end_time) {
            timing.execution_time = end.duration_since(start);
        }
    }

    /// Record compilation time.
    pub fn record_compilation_time(&self, duration: Duration) {
        self.timing.write().compilation_time = duration;
    }

    /// Record instantiation time.
    pub fn record_instantiation_time(&self, duration: Duration) {
        self.timing.write().instantiation_time = duration;
    }

    /// Record memory allocation.
    pub fn record_memory_allocation(&self, bytes: usize) {
        let mut memory = self.memory.write();
        memory.allocation_count += 1;
        memory.final_memory = bytes;
        if bytes > memory.peak_memory {
            memory.peak_memory = bytes;
        }
    }

    /// Record initial memory size.
    pub fn record_initial_memory(&self, bytes: usize) {
        self.memory.write().initial_memory = bytes;
    }

    /// Record fuel consumption.
    pub fn record_fuel_consumed(&self, initial: u64, remaining: u64) {
        let mut fuel = self.fuel.write();
        fuel.initial_fuel = initial;
        fuel.remaining_fuel = remaining;
        fuel.consumed_fuel = initial.saturating_sub(remaining);
    }

    /// Record a refuel event.
    pub fn record_refuel(&self, amount: u64) {
        let mut fuel = self.fuel.write();
        fuel.refuel_events.push(RefuelEvent {
            amount,
            timestamp: Instant::now(),
        });
    }

    /// Record capability usage.
    pub fn record_capability_usage(&self, capability: &CapabilityId) {
        let mut usage = self.capability_usage.write();
        *usage.usage_counts.entry(capability.clone()).or_insert(0) += 1;
    }

    /// Record a denied capability attempt.
    pub fn record_capability_denied(&self, capability: &CapabilityId, action: String, reason: String) {
        self.capability_usage.write().denied_attempts.push(DeniedAttempt {
            capability: capability.clone(),
            action,
            reason,
            timestamp: Instant::now(),
        });
    }

    /// Record a host function call.
    pub fn record_host_call(&self, function: &str, duration: Duration) {
        let mut calls = self.host_calls.write();
        *calls.call_counts.entry(function.to_string()).or_insert(0) += 1;
        *calls
            .call_durations
            .entry(function.to_string())
            .or_insert(Duration::ZERO) += duration;
    }

    /// Get a snapshot of all metrics.
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            timing: self.timing.read().clone(),
            memory: self.memory.read().clone(),
            fuel: self.fuel.read().clone(),
            capability_usage: self.capability_usage.read().clone(),
            host_calls: self.host_calls.read().clone(),
        }
    }

    /// Reset all metrics.
    pub fn reset(&self) {
        *self.timing.write() = TimingMetrics::default();
        *self.memory.write() = MemoryMetrics::default();
        *self.fuel.write() = FuelMetrics::default();
        *self.capability_usage.write() = CapabilityUsageMetrics::default();
        *self.host_calls.write() = HostCallMetrics::default();
    }
}

impl std::fmt::Debug for MetricsCollector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetricsCollector")
            .field("timing", &*self.timing.read())
            .field("memory", &*self.memory.read())
            .field("fuel", &*self.fuel.read())
            .finish()
    }
}

/// Snapshot of collected metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Timing metrics.
    pub timing: TimingMetrics,
    /// Memory metrics.
    pub memory: MemoryMetrics,
    /// Fuel metrics.
    pub fuel: FuelMetrics,
    /// Capability usage metrics.
    pub capability_usage: CapabilityUsageMetrics,
    /// Host call metrics.
    pub host_calls: HostCallMetrics,
}

/// Timing-related metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TimingMetrics {
    /// When execution started.
    #[serde(skip)]
    pub start_time: Option<Instant>,
    /// When execution ended.
    #[serde(skip)]
    pub end_time: Option<Instant>,
    /// Total execution time.
    #[serde(with = "duration_serde")]
    pub execution_time: Duration,
    /// Time spent compiling the module.
    #[serde(with = "duration_serde")]
    pub compilation_time: Duration,
    /// Time spent instantiating the module.
    #[serde(with = "duration_serde")]
    pub instantiation_time: Duration,
}

/// Memory-related metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryMetrics {
    /// Initial memory size in bytes.
    pub initial_memory: usize,
    /// Peak memory usage in bytes.
    pub peak_memory: usize,
    /// Final memory size in bytes.
    pub final_memory: usize,
    /// Number of memory allocations.
    pub allocation_count: u64,
}

/// Fuel-related metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FuelMetrics {
    /// Initial fuel allocation.
    pub initial_fuel: u64,
    /// Fuel consumed.
    pub consumed_fuel: u64,
    /// Remaining fuel.
    pub remaining_fuel: u64,
    /// Refuel events.
    #[serde(skip)]
    pub refuel_events: Vec<RefuelEvent>,
}

/// A refuel event.
#[derive(Debug, Clone)]
pub struct RefuelEvent {
    /// Amount of fuel added.
    pub amount: u64,
    /// When the refuel occurred.
    pub timestamp: Instant,
}

/// Capability usage metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityUsageMetrics {
    /// Count of uses per capability.
    pub usage_counts: HashMap<CapabilityId, u64>,
    /// Denied permission attempts.
    #[serde(skip)]
    pub denied_attempts: Vec<DeniedAttempt>,
}

/// A denied capability attempt.
#[derive(Debug, Clone)]
pub struct DeniedAttempt {
    /// The capability that denied the action.
    pub capability: CapabilityId,
    /// The action that was attempted.
    pub action: String,
    /// The reason for denial.
    pub reason: String,
    /// When the denial occurred.
    pub timestamp: Instant,
}

/// Host call metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostCallMetrics {
    /// Per-function call counts.
    pub call_counts: HashMap<String, u64>,
    /// Per-function total time.
    #[serde(skip)]
    pub call_durations: HashMap<String, Duration>,
}

/// Custom serde for Duration.
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_nanos().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let nanos = u128::deserialize(deserializer)?;
        Ok(Duration::from_nanos(nanos as u64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_timing() {
        let collector = MetricsCollector::new();

        collector.record_start();
        std::thread::sleep(Duration::from_millis(10));
        collector.record_end();

        let snapshot = collector.snapshot();
        assert!(snapshot.timing.execution_time >= Duration::from_millis(10));
    }

    #[test]
    fn test_metrics_collector_memory() {
        let collector = MetricsCollector::new();

        collector.record_initial_memory(1000);
        collector.record_memory_allocation(2000);
        collector.record_memory_allocation(1500);

        let snapshot = collector.snapshot();
        assert_eq!(snapshot.memory.initial_memory, 1000);
        assert_eq!(snapshot.memory.peak_memory, 2000);
        assert_eq!(snapshot.memory.final_memory, 1500);
        assert_eq!(snapshot.memory.allocation_count, 2);
    }

    #[test]
    fn test_metrics_collector_fuel() {
        let collector = MetricsCollector::new();

        collector.record_fuel_consumed(1000, 750);

        let snapshot = collector.snapshot();
        assert_eq!(snapshot.fuel.initial_fuel, 1000);
        assert_eq!(snapshot.fuel.consumed_fuel, 250);
        assert_eq!(snapshot.fuel.remaining_fuel, 750);
    }

    #[test]
    fn test_metrics_collector_capability_usage() {
        let collector = MetricsCollector::new();
        let cap_id = CapabilityId::new("test");

        collector.record_capability_usage(&cap_id);
        collector.record_capability_usage(&cap_id);

        let snapshot = collector.snapshot();
        assert_eq!(snapshot.capability_usage.usage_counts.get(&cap_id), Some(&2));
    }

    #[test]
    fn test_metrics_collector_reset() {
        let collector = MetricsCollector::new();
        collector.record_fuel_consumed(1000, 500);

        collector.reset();

        let snapshot = collector.snapshot();
        assert_eq!(snapshot.fuel.initial_fuel, 0);
    }
}
