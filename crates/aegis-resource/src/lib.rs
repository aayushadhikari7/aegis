//! Aegis Resource Management
//!
//! This crate provides resource management functionality for the Aegis
//! WebAssembly sandbox runtime, including:
//!
//! - Memory limiting via [`AegisResourceLimiter`]
//! - CPU limiting via fuel management in [`FuelManager`]
//! - Timeout management via epochs in [`EpochManager`]
//!
//! # Resource Management Strategy
//!
//! Aegis uses a multi-layered approach to resource management:
//!
//! 1. **Memory Limits**: Hard limits on linear memory growth
//! 2. **Fuel Limits**: Deterministic CPU limiting via fuel consumption
//! 3. **Epoch Timeouts**: Wall-clock timeout via epoch-based interruption
//!
//! ## Memory Limiting
//!
//! Memory limits are enforced via [`AegisResourceLimiter`], which implements
//! Wasmtime's `ResourceLimiter` trait. This prevents guests from allocating
//! unbounded memory.
//!
//! ```ignore
//! use aegis_resource::limiter::{AegisResourceLimiter, LimiterConfig};
//!
//! let limiter = AegisResourceLimiter::new(
//!     LimiterConfig::default().with_max_memory(64 * 1024 * 1024)
//! );
//! ```
//!
//! ## Fuel Limiting
//!
//! Fuel provides deterministic CPU limiting. Each WASM instruction consumes
//! fuel, and execution traps when fuel is exhausted.
//!
//! ```ignore
//! use aegis_resource::fuel::{FuelManager, FuelConfig};
//!
//! let manager = FuelManager::new(FuelConfig::new(1_000_000_000));
//! ```
//!
//! ## Epoch Timeouts
//!
//! Epochs provide wall-clock timeout support. A background thread increments
//! the epoch counter, and stores configured with deadlines will trap when
//! the deadline is exceeded.
//!
//! ```ignore
//! use aegis_resource::epoch::{EpochManager, EpochConfig};
//!
//! let manager = EpochManager::new(engine, EpochConfig::default())?;
//! ```

pub mod epoch;
pub mod error;
pub mod fuel;
pub mod limiter;

// Re-export main types
pub use epoch::{EpochConfig, EpochManager, EpochStats, TimeoutGuard};
pub use error::{ResourceError, ResourceResult};
pub use fuel::{FuelConfig, FuelCostEstimates, FuelManager, FuelStats};
pub use limiter::{AegisResourceLimiter, LimiterConfig, LimiterStats, MemoryGrowthEvent};

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::epoch::{EpochConfig, EpochManager, TimeoutGuard};
    pub use crate::error::{ResourceError, ResourceResult};
    pub use crate::fuel::{FuelConfig, FuelManager};
    pub use crate::limiter::{AegisResourceLimiter, LimiterConfig};
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_prelude_imports() {
        // Verify that prelude exports work
        use crate::prelude::*;

        let _config = LimiterConfig::default();
        let _fuel = FuelConfig::default();
        let _epoch = EpochConfig::default();
    }
}
