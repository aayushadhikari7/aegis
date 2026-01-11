//! Filesystem capability for file system access.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::capability::{
    Action, Capability, CapabilityId, DenialReason, PermissionResult, standard_ids,
};
use crate::error::CapabilityError;

/// Actions related to filesystem operations.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum FilesystemAction {
    /// Read from a file.
    Read { path: PathBuf },
    /// Write to a file.
    Write { path: PathBuf },
    /// Create a new file.
    Create { path: PathBuf },
    /// Delete a file.
    Delete { path: PathBuf },
    /// List directory contents.
    List { path: PathBuf },
    /// Get file metadata.
    Stat { path: PathBuf },
}

impl Action for FilesystemAction {
    fn action_type(&self) -> &str {
        match self {
            FilesystemAction::Read { .. } => "fs:read",
            FilesystemAction::Write { .. } => "fs:write",
            FilesystemAction::Create { .. } => "fs:create",
            FilesystemAction::Delete { .. } => "fs:delete",
            FilesystemAction::List { .. } => "fs:list",
            FilesystemAction::Stat { .. } => "fs:stat",
        }
    }

    fn description(&self) -> String {
        match self {
            FilesystemAction::Read { path } => format!("Read file: {}", path.display()),
            FilesystemAction::Write { path } => format!("Write file: {}", path.display()),
            FilesystemAction::Create { path } => format!("Create file: {}", path.display()),
            FilesystemAction::Delete { path } => format!("Delete file: {}", path.display()),
            FilesystemAction::List { path } => format!("List directory: {}", path.display()),
            FilesystemAction::Stat { path } => format!("Get metadata: {}", path.display()),
        }
    }
}

#[allow(dead_code)]
impl FilesystemAction {
    /// Get the path associated with this action.
    pub fn path(&self) -> &Path {
        match self {
            FilesystemAction::Read { path }
            | FilesystemAction::Write { path }
            | FilesystemAction::Create { path }
            | FilesystemAction::Delete { path }
            | FilesystemAction::List { path }
            | FilesystemAction::Stat { path } => path,
        }
    }
}

/// Permission for a specific path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathPermission {
    /// The path (directory or file).
    pub path: PathBuf,
    /// Allow read access.
    pub read: bool,
    /// Allow write access.
    pub write: bool,
    /// Allow creating new files.
    pub create: bool,
    /// Allow deleting files.
    pub delete: bool,
}

#[allow(dead_code)]
impl PathPermission {
    /// Create a read-only permission for a path.
    pub fn read_only(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            read: true,
            write: false,
            create: false,
            delete: false,
        }
    }

    /// Create a read-write permission for a path.
    pub fn read_write(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            read: true,
            write: true,
            create: true,
            delete: false,
        }
    }

    /// Create a full permission for a path.
    pub fn full(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            read: true,
            write: true,
            create: true,
            delete: true,
        }
    }

    /// Check if the action's path is under this permission's path.
    fn path_matches(&self, action_path: &Path) -> bool {
        // Canonicalize paths for comparison (handle .. and symlinks)
        // In production, use proper canonicalization
        action_path.starts_with(&self.path)
    }

    /// Check if this permission allows the given action.
    fn allows(&self, action: &FilesystemAction) -> bool {
        if !self.path_matches(action.path()) {
            return false;
        }

        match action {
            FilesystemAction::Read { .. }
            | FilesystemAction::List { .. }
            | FilesystemAction::Stat { .. } => self.read,
            FilesystemAction::Write { .. } => self.write,
            FilesystemAction::Create { .. } => self.create,
            FilesystemAction::Delete { .. } => self.delete,
        }
    }
}

/// Capability for filesystem access.
///
/// This capability controls access to the filesystem, including reading,
/// writing, creating, and deleting files.
///
/// # Example
///
/// ```
/// use aegis_capability::builtin::{FilesystemCapability, PathPermission};
/// use std::path::PathBuf;
///
/// // Read-only access to /data
/// let read_only = FilesystemCapability::new(vec![
///     PathPermission::read_only("/data"),
/// ]);
///
/// // Read-write access to /tmp
/// let read_write = FilesystemCapability::new(vec![
///     PathPermission::read_write("/tmp"),
/// ]);
/// ```
#[derive(Debug, Clone)]
pub struct FilesystemCapability {
    /// Allowed paths with their permissions.
    permissions: Vec<PathPermission>,
}

impl FilesystemCapability {
    /// Create a new filesystem capability with the given permissions.
    pub fn new(permissions: Vec<PathPermission>) -> Self {
        Self { permissions }
    }

    /// Create a read-only capability for the given paths.
    pub fn read_only(paths: &[impl AsRef<Path>]) -> Self {
        Self {
            permissions: paths
                .iter()
                .map(|p| PathPermission::read_only(p.as_ref()))
                .collect(),
        }
    }

    /// Create a read-write capability for the given paths.
    pub fn read_write(paths: &[impl AsRef<Path>]) -> Self {
        Self {
            permissions: paths
                .iter()
                .map(|p| PathPermission::read_write(p.as_ref()))
                .collect(),
        }
    }

    /// Add a permission to this capability.
    pub fn add_permission(&mut self, permission: PathPermission) {
        self.permissions.push(permission);
    }

    /// Get the permissions.
    pub fn permissions(&self) -> &[PathPermission] {
        &self.permissions
    }
}

impl Capability for FilesystemCapability {
    fn id(&self) -> CapabilityId {
        standard_ids::FILESYSTEM.clone()
    }

    fn name(&self) -> &str {
        "Filesystem"
    }

    fn description(&self) -> &str {
        "Allows access to the filesystem"
    }

    fn permits(&self, action: &dyn Action) -> PermissionResult {
        // Check if this is a filesystem action
        let action_type = action.action_type();
        if !action_type.starts_with("fs:") {
            return PermissionResult::NotApplicable;
        }

        // We can't actually downcast here without the concrete action type,
        // so we return NotApplicable. Use check_filesystem_permission() for
        // concrete FilesystemAction checks.
        PermissionResult::NotApplicable
    }

    fn handled_action_types(&self) -> Vec<&'static str> {
        vec![
            "fs:read",
            "fs:write",
            "fs:create",
            "fs:delete",
            "fs:list",
            "fs:stat",
        ]
    }

    fn validate(&self) -> Result<(), CapabilityError> {
        if self.permissions.is_empty() {
            return Err(CapabilityError::InvalidConfig(
                "Filesystem capability has no permissions configured".to_string(),
            ));
        }
        Ok(())
    }
}

/// Helper function to check filesystem permission with a concrete action.
#[allow(dead_code)]
pub fn check_filesystem_permission(
    capability: &FilesystemCapability,
    action: &FilesystemAction,
) -> PermissionResult {
    for perm in capability.permissions() {
        if perm.allows(action) {
            return PermissionResult::Allowed;
        }
    }

    PermissionResult::Denied(DenialReason::new(
        capability.id(),
        action.action_type(),
        format!("No permission for path: {}", action.path().display()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_permission_read_only() {
        let perm = PathPermission::read_only("/data");
        assert!(perm.read);
        assert!(!perm.write);
        assert!(!perm.create);
        assert!(!perm.delete);
    }

    #[test]
    fn test_path_permission_allows_read() {
        let perm = PathPermission::read_only("/data");
        let action = FilesystemAction::Read {
            path: PathBuf::from("/data/file.txt"),
        };
        assert!(perm.allows(&action));
    }

    #[test]
    fn test_path_permission_denies_write() {
        let perm = PathPermission::read_only("/data");
        let action = FilesystemAction::Write {
            path: PathBuf::from("/data/file.txt"),
        };
        assert!(!perm.allows(&action));
    }

    #[test]
    fn test_path_permission_denies_outside_path() {
        let perm = PathPermission::full("/data");
        let action = FilesystemAction::Read {
            path: PathBuf::from("/etc/passwd"),
        };
        assert!(!perm.allows(&action));
    }

    #[test]
    fn test_filesystem_capability_creation() {
        let cap = FilesystemCapability::read_only(&["/data", "/tmp"]);
        assert_eq!(cap.permissions().len(), 2);
    }

    #[test]
    fn test_check_filesystem_permission() {
        let cap = FilesystemCapability::read_write(&["/tmp"]);

        let read_action = FilesystemAction::Read {
            path: PathBuf::from("/tmp/test.txt"),
        };
        assert!(check_filesystem_permission(&cap, &read_action).is_allowed());

        let write_action = FilesystemAction::Write {
            path: PathBuf::from("/tmp/test.txt"),
        };
        assert!(check_filesystem_permission(&cap, &write_action).is_allowed());

        let outside_action = FilesystemAction::Read {
            path: PathBuf::from("/etc/passwd"),
        };
        assert!(check_filesystem_permission(&cap, &outside_action).is_denied());
    }
}
