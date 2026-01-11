//! Built-in capabilities for common operations.
//!
//! This module provides standard capabilities for:
//!
//! - [`FilesystemCapability`]: File system access
//! - [`NetworkCapability`]: Network access
//! - [`LoggingCapability`]: Logging output
//! - [`ClockCapability`]: Time and clock access

mod clock;
mod filesystem;
mod logging;
mod network;

pub use clock::{ClockCapability, ClockType};
pub use filesystem::{FilesystemCapability, PathPermission};
pub use logging::{LogLevel, LoggingCapability};
pub use network::{HostPattern, NetworkCapability, ProtocolSet};
