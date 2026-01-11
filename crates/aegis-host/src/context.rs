//! Host function execution context.
//!
//! This module provides the `HostContext` type which is available to host
//! function implementations for accessing sandbox state and capabilities.

use std::sync::Arc;

use aegis_capability::{Action, CapabilityId, CapabilitySet, PermissionResult};
use wasmtime::Caller;

use crate::error::{HostError, HostResult};

/// Context available to host function implementations.
///
/// `HostContext` provides safe access to sandbox state, capability checking,
/// and memory operations from within host function implementations.
pub struct HostContext<'a, T> {
    /// The Wasmtime caller.
    caller: Caller<'a, T>,
    /// Reference to the capability set.
    capabilities: Option<Arc<CapabilitySet>>,
}

impl<'a, T> HostContext<'a, T> {
    /// Create a new host context.
    pub fn new(caller: Caller<'a, T>) -> Self {
        Self {
            caller,
            capabilities: None,
        }
    }

    /// Create a host context with capabilities.
    pub fn with_capabilities(caller: Caller<'a, T>, capabilities: Arc<CapabilitySet>) -> Self {
        Self {
            caller,
            capabilities: Some(capabilities),
        }
    }

    /// Get a reference to the underlying Wasmtime caller.
    pub fn caller(&self) -> &Caller<'a, T> {
        &self.caller
    }

    /// Get a mutable reference to the underlying Wasmtime caller.
    pub fn caller_mut(&mut self) -> &mut Caller<'a, T> {
        &mut self.caller
    }

    /// Access the store data.
    pub fn data(&self) -> &T {
        self.caller.data()
    }

    /// Access the store data mutably.
    pub fn data_mut(&mut self) -> &mut T {
        self.caller.data_mut()
    }

    /// Check if a capability is available.
    pub fn has_capability(&self, id: &CapabilityId) -> bool {
        self.capabilities
            .as_ref()
            .map(|caps| caps.has(id))
            .unwrap_or(false)
    }

    /// Require that a capability is available.
    pub fn require_capability(&self, id: &CapabilityId) -> HostResult<()> {
        if self.has_capability(id) {
            Ok(())
        } else {
            Err(HostError::CapabilityNotGranted(id.clone()))
        }
    }

    /// Check permission for an action.
    pub fn check_permission(&self, action: &dyn Action) -> PermissionResult {
        self.capabilities
            .as_ref()
            .map(|caps| caps.check_permission(action))
            .unwrap_or(PermissionResult::NotApplicable)
    }

    /// Require permission for an action.
    pub fn require_permission(&self, action: &dyn Action) -> HostResult<()> {
        match self.check_permission(action) {
            PermissionResult::Allowed => Ok(()),
            PermissionResult::Denied(reason) => Err(HostError::PermissionDenied {
                action: action.action_type().to_string(),
                reason: reason.message,
            }),
            PermissionResult::NotApplicable => Err(HostError::NoCapabilityForAction {
                action: action.action_type().to_string(),
            }),
        }
    }

    /// Get the default memory export.
    pub fn get_memory(&mut self) -> HostResult<wasmtime::Memory> {
        self.caller
            .get_export("memory")
            .and_then(|e| e.into_memory())
            .ok_or(HostError::MemoryNotFound)
    }

    /// Read bytes from guest memory.
    pub fn read_memory(&mut self, offset: usize, len: usize) -> HostResult<Vec<u8>> {
        let memory = self.get_memory()?;
        let data = memory.data(&self.caller);

        if offset + len > data.len() {
            return Err(HostError::MemoryAccessOutOfBounds {
                offset,
                len,
                memory_size: data.len(),
            });
        }

        Ok(data[offset..offset + len].to_vec())
    }

    /// Write bytes to guest memory.
    pub fn write_memory(&mut self, offset: usize, data: &[u8]) -> HostResult<()> {
        let memory = self.get_memory()?;
        let mem_data = memory.data_mut(&mut self.caller);

        if offset + data.len() > mem_data.len() {
            return Err(HostError::MemoryAccessOutOfBounds {
                offset,
                len: data.len(),
                memory_size: mem_data.len(),
            });
        }

        mem_data[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Read a null-terminated string from guest memory.
    pub fn read_string(&mut self, offset: usize, max_len: usize) -> HostResult<String> {
        let memory = self.get_memory()?;
        let data = memory.data(&self.caller);

        if offset >= data.len() {
            return Err(HostError::MemoryAccessOutOfBounds {
                offset,
                len: 1,
                memory_size: data.len(),
            });
        }

        let end = (offset + max_len).min(data.len());
        let slice = &data[offset..end];

        // Find null terminator or use max_len
        let len = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());

        String::from_utf8(slice[..len].to_vec())
            .map_err(|e| HostError::InvalidUtf8(e.to_string()))
    }

    /// Read a string with explicit length from guest memory.
    pub fn read_string_with_len(&mut self, offset: usize, len: usize) -> HostResult<String> {
        let bytes = self.read_memory(offset, len)?;
        String::from_utf8(bytes).map_err(|e| HostError::InvalidUtf8(e.to_string()))
    }
}

impl<'a, T> std::fmt::Debug for HostContext<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostContext")
            .field("has_capabilities", &self.capabilities.is_some())
            .finish()
    }
}

/// Extension trait for creating host contexts from callers.
pub trait IntoHostContext<'a, T> {
    /// Convert into a host context.
    fn into_context(self) -> HostContext<'a, T>;

    /// Convert into a host context with capabilities.
    fn into_context_with_caps(self, capabilities: Arc<CapabilitySet>) -> HostContext<'a, T>;
}

impl<'a, T> IntoHostContext<'a, T> for Caller<'a, T> {
    fn into_context(self) -> HostContext<'a, T> {
        HostContext::new(self)
    }

    fn into_context_with_caps(self, capabilities: Arc<CapabilitySet>) -> HostContext<'a, T> {
        HostContext::with_capabilities(self, capabilities)
    }
}
