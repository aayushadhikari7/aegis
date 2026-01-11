//! Inspect command - Inspect a WebAssembly module.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;

use aegis_core::{ExportInfo, ExportKind, ImportInfo, ImportKind};
use aegis_wasm::prelude::*;

use crate::OutputFormat;

/// Arguments for the inspect command.
#[derive(Args)]
pub struct InspectArgs {
    /// Path to the WebAssembly module
    #[arg(required = true)]
    pub module: PathBuf,

    /// Show exports
    #[arg(long)]
    pub exports: bool,

    /// Show imports
    #[arg(long)]
    pub imports: bool,

    /// Show memory information
    #[arg(long)]
    pub memory: bool,

    /// Show all information
    #[arg(long, short)]
    pub all: bool,
}

/// Inspection result.
#[derive(Debug, Serialize)]
struct InspectionResult {
    path: String,
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exports: Option<Vec<ExportDisplay>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    imports: Option<Vec<ImportDisplay>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    memories: Option<Vec<MemoryDisplay>>,
}

#[derive(Debug, Serialize)]
struct ExportDisplay {
    name: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
}

#[derive(Debug, Serialize)]
struct ImportDisplay {
    module: String,
    name: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
}

#[derive(Debug, Serialize)]
struct MemoryDisplay {
    min_pages: u64,
    max_pages: Option<u64>,
    memory64: bool,
}

impl From<&ExportInfo> for ExportDisplay {
    fn from(info: &ExportInfo) -> Self {
        let (kind, signature) = match &info.kind {
            ExportKind::Function { params, results } => (
                "function".to_string(),
                Some(format!("({}) -> ({})", params, results)),
            ),
            ExportKind::Memory => ("memory".to_string(), None),
            ExportKind::Global => ("global".to_string(), None),
            ExportKind::Table => ("table".to_string(), None),
        };

        Self {
            name: info.name.clone(),
            kind,
            signature,
        }
    }
}

impl From<&ImportInfo> for ImportDisplay {
    fn from(info: &ImportInfo) -> Self {
        let (kind, signature) = match &info.kind {
            ImportKind::Function { params, results } => (
                "function".to_string(),
                Some(format!("({}) -> ({})", params, results)),
            ),
            ImportKind::Memory => ("memory".to_string(), None),
            ImportKind::Global => ("global".to_string(), None),
            ImportKind::Table => ("table".to_string(), None),
        };

        Self {
            module: info.module.clone(),
            name: info.name.clone(),
            kind,
            signature,
        }
    }
}

/// Execute the inspect command.
pub fn execute(args: InspectArgs, format: OutputFormat) -> Result<()> {
    let runtime = Aegis::builder()
        .build()
        .context("Failed to create runtime")?;

    let module = runtime
        .load_file(&args.module)
        .context("Failed to load module")?;

    let show_all = args.all || (!args.exports && !args.imports && !args.memory);

    let mut result = InspectionResult {
        path: args.module.display().to_string(),
        name: module.name().map(String::from),
        exports: None,
        imports: None,
        memories: None,
    };

    if show_all || args.exports {
        result.exports = Some(module.exports().iter().map(ExportDisplay::from).collect());
    }

    if show_all || args.imports {
        result.imports = Some(module.imports().iter().map(ImportDisplay::from).collect());
    }

    if show_all || args.memory {
        result.memories = Some(
            module
                .metadata()
                .memories
                .iter()
                .map(|m| MemoryDisplay {
                    min_pages: m.min_pages,
                    max_pages: m.max_pages,
                    memory64: m.memory64,
                })
                .collect(),
        );
    }

    // Output results
    match format {
        OutputFormat::Human => {
            println!("Module: {}", args.module.display());
            if let Some(name) = &result.name {
                println!("Name: {}", name);
            }
            println!();

            if let Some(exports) = &result.exports {
                println!("Exports ({}):", exports.len());
                for export in exports {
                    if let Some(sig) = &export.signature {
                        println!("  {} [{}]: {}", export.name, export.kind, sig);
                    } else {
                        println!("  {} [{}]", export.name, export.kind);
                    }
                }
                println!();
            }

            if let Some(imports) = &result.imports {
                println!("Imports ({}):", imports.len());
                for import in imports {
                    if let Some(sig) = &import.signature {
                        println!(
                            "  {}::{} [{}]: {}",
                            import.module, import.name, import.kind, sig
                        );
                    } else {
                        println!("  {}::{} [{}]", import.module, import.name, import.kind);
                    }
                }
                println!();
            }

            if let Some(memories) = &result.memories {
                println!("Memories ({}):", memories.len());
                for (i, memory) in memories.iter().enumerate() {
                    let max = memory
                        .max_pages
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| "unbounded".to_string());
                    let bits = if memory.memory64 { "64-bit" } else { "32-bit" };
                    println!("  [{}] {} - {} pages ({})", i, memory.min_pages, max, bits);
                }
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        OutputFormat::JsonCompact => {
            println!("{}", serde_json::to_string(&result)?);
        }
    }

    Ok(())
}
