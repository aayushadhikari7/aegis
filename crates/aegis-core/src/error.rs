//! Core error types for Aegis.
//!
//! This module defines the error hierarchy used throughout the Aegis runtime.
//! Errors are categorized by their origin and type to enable proper handling
//! and reporting.

use std::time::Duration;
use thiserror::Error;

/// Top-level error type for Aegis core operations.
#[derive(Debug, Error)]
pub enum AegisError {
    /// Error during engine creation or configuration.
    #[error("Engine error: {0}")]
    Engine(#[from] EngineError),

    /// Error during module loading or validation.
    #[error("Module error: {0}")]
    Module(#[from] ModuleError),

    /// Error during WASM execution.
    #[error("Execution error: {0}")]
    Execution(#[from] ExecutionError),
}

/// Errors during engine creation and configuration.
#[derive(Debug, Error)]
pub enum EngineError {
    /// Invalid engine configuration provided.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Underlying Wasmtime error.
    #[error("Wasmtime error: {0}")]
    Wasmtime(#[from] wasmtime::Error),
}

/// Errors during module loading and validation.
#[derive(Debug, Error)]
pub enum ModuleError {
    /// The WASM module is invalid or malformed.
    #[error("Invalid WASM module: {0}")]
    Invalid(String),

    /// Module validation failed.
    #[error("Module validation failed: {0}")]
    ValidationFailed(String),

    /// IO error reading the module.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// A required import is missing.
    #[error("Missing import: module='{module}', name='{name}'")]
    MissingImport {
        /// The import module name.
        module: String,
        /// The import name.
        name: String,
    },

    /// Underlying Wasmtime error.
    #[error("Wasmtime error: {0}")]
    Wasmtime(#[from] wasmtime::Error),
}

/// Errors during WASM execution.
#[derive(Debug, Error)]
pub enum ExecutionError {
    /// A WASM trap occurred during execution.
    #[error("WASM trap: {0}")]
    Trap(#[from] TrapInfo),

    /// Execution exceeded the timeout limit.
    #[error("Execution timeout after {0:?}")]
    Timeout(Duration),

    /// Execution ran out of fuel (CPU limit exceeded).
    #[error("Out of fuel: consumed {consumed}, limit was {limit}")]
    OutOfFuel {
        /// Amount of fuel consumed.
        consumed: u64,
        /// The fuel limit that was set.
        limit: u64,
    },

    /// Memory limit was exceeded.
    #[error("Memory limit exceeded: used {used} bytes, limit {limit} bytes")]
    MemoryExceeded {
        /// Memory used in bytes.
        used: usize,
        /// Memory limit in bytes.
        limit: usize,
    },

    /// The requested function was not found in the module.
    #[error("Function not found: '{0}'")]
    FunctionNotFound(String),

    /// Type mismatch when calling a function.
    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch {
        /// Expected type signature.
        expected: String,
        /// Actual type signature.
        actual: String,
    },

    /// The module has not been loaded yet.
    #[error("Module not loaded")]
    ModuleNotLoaded,

    /// Underlying Wasmtime error.
    #[error("Wasmtime error: {0}")]
    Wasmtime(#[from] wasmtime::Error),
}

/// Information about a WASM trap.
#[derive(Debug, Clone)]
pub struct TrapInfo {
    /// The trap code name, if available.
    pub code: Option<String>,
    /// Human-readable trap message.
    pub message: String,
    /// Stack backtrace, if available.
    pub backtrace: Option<String>,
}

impl std::fmt::Display for TrapInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(code) = &self.code {
            write!(f, "[{}] {}", code, self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for TrapInfo {}

impl From<wasmtime::Trap> for TrapInfo {
    fn from(trap: wasmtime::Trap) -> Self {
        Self {
            code: None,
            message: trap.to_string(),
            backtrace: None,
        }
    }
}

/// Result type alias for Aegis operations.
pub type Result<T> = std::result::Result<T, AegisError>;

/// Result type alias for engine operations.
pub type EngineResult<T> = std::result::Result<T, EngineError>;

/// Result type alias for module operations.
pub type ModuleResult<T> = std::result::Result<T, ModuleError>;

/// Result type alias for execution operations.
pub type ExecutionResult<T> = std::result::Result<T, ExecutionError>;
