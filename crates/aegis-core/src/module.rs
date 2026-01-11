//! WASM module loading and validation.
//!
//! This module provides types for loading, validating, and inspecting
//! WebAssembly modules before execution.

use std::path::Path;
use std::sync::Arc;

use tracing::{debug, info};
use wasmtime::{ExternType, Module};

use crate::engine::AegisEngine;
use crate::error::{ModuleError, ModuleResult};

/// A validated WebAssembly module ready for instantiation.
///
/// `ValidatedModule` wraps a Wasmtime module with additional metadata
/// extracted during validation. This ensures that modules are validated
/// once and can be instantiated multiple times efficiently.
#[derive(Clone)]
pub struct ValidatedModule {
    /// The underlying Wasmtime module.
    inner: Module,
    /// Metadata extracted from the module.
    metadata: ModuleMetadata,
}

impl ValidatedModule {
    /// Get a reference to the underlying Wasmtime module.
    pub fn inner(&self) -> &Module {
        &self.inner
    }

    /// Get the module metadata.
    pub fn metadata(&self) -> &ModuleMetadata {
        &self.metadata
    }

    /// Get the module name, if set.
    pub fn name(&self) -> Option<&str> {
        self.metadata.name.as_deref()
    }

    /// Get the list of exports.
    pub fn exports(&self) -> &[ExportInfo] {
        &self.metadata.exports
    }

    /// Get the list of imports.
    pub fn imports(&self) -> &[ImportInfo] {
        &self.metadata.imports
    }

    /// Check if the module has a specific export.
    pub fn has_export(&self, name: &str) -> bool {
        self.metadata.exports.iter().any(|e| e.name == name)
    }

    /// Check if the module requires a specific import.
    pub fn requires_import(&self, module: &str, name: &str) -> bool {
        self.metadata
            .imports
            .iter()
            .any(|i| i.module == module && i.name == name)
    }
}

impl std::fmt::Debug for ValidatedModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValidatedModule")
            .field("name", &self.metadata.name)
            .field("exports", &self.metadata.exports.len())
            .field("imports", &self.metadata.imports.len())
            .finish()
    }
}

/// Metadata extracted from a WASM module.
#[derive(Debug, Clone, Default)]
pub struct ModuleMetadata {
    /// Module name, if specified.
    pub name: Option<String>,
    /// List of exported items.
    pub exports: Vec<ExportInfo>,
    /// List of required imports.
    pub imports: Vec<ImportInfo>,
    /// Memory requirements.
    pub memories: Vec<MemoryInfo>,
}

/// Information about an exported item.
#[derive(Debug, Clone)]
pub struct ExportInfo {
    /// Export name.
    pub name: String,
    /// Type of the export.
    pub kind: ExportKind,
}

/// The kind of an export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportKind {
    /// A function export.
    Function {
        /// Number of parameters.
        params: usize,
        /// Number of results.
        results: usize,
    },
    /// A memory export.
    Memory,
    /// A global export.
    Global,
    /// A table export.
    Table,
}

/// Information about a required import.
#[derive(Debug, Clone)]
pub struct ImportInfo {
    /// Import module name.
    pub module: String,
    /// Import name.
    pub name: String,
    /// Type of the import.
    pub kind: ImportKind,
}

/// The kind of an import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportKind {
    /// A function import.
    Function {
        /// Number of parameters.
        params: usize,
        /// Number of results.
        results: usize,
    },
    /// A memory import.
    Memory,
    /// A global import.
    Global,
    /// A table import.
    Table,
}

/// Information about a memory definition.
#[derive(Debug, Clone)]
pub struct MemoryInfo {
    /// Minimum memory size in pages (64KB each).
    pub min_pages: u64,
    /// Maximum memory size in pages, if specified.
    pub max_pages: Option<u64>,
    /// Whether this is a 64-bit memory.
    pub memory64: bool,
}

/// Loader for WASM modules.
///
/// `ModuleLoader` provides methods for loading and validating WASM modules
/// from various sources.
pub struct ModuleLoader {
    /// Reference to the engine used for compilation.
    engine: Arc<AegisEngine>,
}

impl ModuleLoader {
    /// Create a new module loader with the given engine.
    pub fn new(engine: Arc<AegisEngine>) -> Self {
        Self { engine }
    }

    /// Load and validate a module from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the bytes are not a valid WASM module.
    pub fn load_bytes(&self, bytes: &[u8]) -> ModuleResult<ValidatedModule> {
        debug!(size = bytes.len(), "Loading WASM module from bytes");

        let module = Module::new(self.engine.inner(), bytes)?;
        let metadata = self.extract_metadata(&module);

        info!(
            name = ?metadata.name,
            exports = metadata.exports.len(),
            imports = metadata.imports.len(),
            "Loaded WASM module"
        );

        Ok(ValidatedModule {
            inner: module,
            metadata,
        })
    }

    /// Load and validate a module from a file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or is not a valid WASM module.
    pub fn load_file(&self, path: &Path) -> ModuleResult<ValidatedModule> {
        debug!(path = %path.display(), "Loading WASM module from file");

        let module = Module::from_file(self.engine.inner(), path)?;
        let metadata = self.extract_metadata(&module);

        info!(
            path = %path.display(),
            name = ?metadata.name,
            exports = metadata.exports.len(),
            imports = metadata.imports.len(),
            "Loaded WASM module from file"
        );

        Ok(ValidatedModule {
            inner: module,
            metadata,
        })
    }

    /// Load and validate a module from WAT (WebAssembly Text) format.
    ///
    /// This is primarily useful for testing and development.
    ///
    /// # Errors
    ///
    /// Returns an error if the WAT is invalid.
    pub fn load_wat(&self, wat: &str) -> ModuleResult<ValidatedModule> {
        debug!(size = wat.len(), "Loading WASM module from WAT");

        let wasm = wat::parse_str(wat).map_err(|e| ModuleError::Invalid(e.to_string()))?;
        self.load_bytes(&wasm)
    }

    /// Extract metadata from a compiled module.
    fn extract_metadata(&self, module: &Module) -> ModuleMetadata {
        let name = module.name().map(String::from);

        let exports = module
            .exports()
            .map(|export| ExportInfo {
                name: export.name().to_string(),
                kind: extern_type_to_export_kind(export.ty()),
            })
            .collect();

        let imports = module
            .imports()
            .map(|import| ImportInfo {
                module: import.module().to_string(),
                name: import.name().to_string(),
                kind: extern_type_to_import_kind(import.ty()),
            })
            .collect();

        let memories = module
            .exports()
            .filter_map(|export| match export.ty() {
                ExternType::Memory(mem) => Some(MemoryInfo {
                    min_pages: mem.minimum(),
                    max_pages: mem.maximum(),
                    memory64: mem.is_64(),
                }),
                _ => None,
            })
            .collect();

        ModuleMetadata {
            name,
            exports,
            imports,
            memories,
        }
    }
}

fn extern_type_to_export_kind(ty: ExternType) -> ExportKind {
    match ty {
        ExternType::Func(func) => ExportKind::Function {
            params: func.params().len(),
            results: func.results().len(),
        },
        ExternType::Memory(_) => ExportKind::Memory,
        ExternType::Global(_) => ExportKind::Global,
        ExternType::Table(_) => ExportKind::Table,
    }
}

fn extern_type_to_import_kind(ty: ExternType) -> ImportKind {
    match ty {
        ExternType::Func(func) => ImportKind::Function {
            params: func.params().len(),
            results: func.results().len(),
        },
        ExternType::Memory(_) => ImportKind::Memory,
        ExternType::Global(_) => ImportKind::Global,
        ExternType::Table(_) => ImportKind::Table,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EngineConfig;

    fn create_loader() -> ModuleLoader {
        let engine = Arc::new(AegisEngine::new(EngineConfig::default()).unwrap());
        ModuleLoader::new(engine)
    }

    #[test]
    fn test_load_simple_module() {
        let loader = create_loader();

        let module = loader
            .load_wat(
                r#"
            (module
                (func (export "add") (param i32 i32) (result i32)
                    local.get 0
                    local.get 1
                    i32.add
                )
            )
        "#,
            )
            .unwrap();

        assert!(module.has_export("add"));
        assert_eq!(module.exports().len(), 1);
        assert_eq!(module.imports().len(), 0);

        if let ExportKind::Function { params, results } = &module.exports()[0].kind {
            assert_eq!(*params, 2);
            assert_eq!(*results, 1);
        } else {
            panic!("Expected function export");
        }
    }

    #[test]
    fn test_load_module_with_imports() {
        let loader = create_loader();

        let module = loader
            .load_wat(
                r#"
            (module
                (import "env" "log" (func $log (param i32)))
                (func (export "main")
                    i32.const 42
                    call $log
                )
            )
        "#,
            )
            .unwrap();

        assert!(module.has_export("main"));
        assert!(module.requires_import("env", "log"));
        assert_eq!(module.imports().len(), 1);
    }

    #[test]
    fn test_load_module_with_memory() {
        let loader = create_loader();

        let module = loader
            .load_wat(
                r#"
            (module
                (memory (export "memory") 1 10)
            )
        "#,
            )
            .unwrap();

        assert!(module.has_export("memory"));
        assert_eq!(module.metadata().memories.len(), 1);
        assert_eq!(module.metadata().memories[0].min_pages, 1);
        assert_eq!(module.metadata().memories[0].max_pages, Some(10));
    }

    #[test]
    fn test_load_invalid_module() {
        let loader = create_loader();

        let result = loader.load_bytes(&[0, 1, 2, 3]);
        assert!(result.is_err());
    }
}
