//! Validate command - Validate a WebAssembly module.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;

use aegis_wasm::prelude::*;

use crate::OutputFormat;

/// Arguments for the validate command.
#[derive(Args)]
pub struct ValidateArgs {
    /// Path to the WebAssembly module
    #[arg(required = true)]
    pub module: PathBuf,

    /// Strict validation mode
    #[arg(long)]
    pub strict: bool,
}

/// Validation result.
#[derive(Debug, Serialize)]
struct ValidationResult {
    valid: bool,
    path: String,
    module_name: Option<String>,
    exports: usize,
    imports: usize,
    warnings: Vec<String>,
    errors: Vec<String>,
}

/// Execute the validate command.
pub fn execute(args: ValidateArgs, format: OutputFormat) -> Result<()> {
    let runtime = Aegis::builder().build().context("Failed to create runtime")?;

    let mut result = ValidationResult {
        valid: true,
        path: args.module.display().to_string(),
        module_name: None,
        exports: 0,
        imports: 0,
        warnings: Vec::new(),
        errors: Vec::new(),
    };

    // Attempt to load and validate the module
    match runtime.load_file(&args.module) {
        Ok(module) => {
            result.module_name = module.name().map(String::from);
            result.exports = module.exports().len();
            result.imports = module.imports().len();

            // Check for common issues
            if module.exports().is_empty() {
                result.warnings.push("Module has no exports".to_string());
            }

            // Check for required functions
            let has_start = module.has_export("_start");
            let has_main = module.has_export("main");
            if !has_start && !has_main {
                result.warnings.push(
                    "Module has no _start or main function - may not be directly executable"
                        .to_string(),
                );
            }

            // Check imports
            for import in module.imports() {
                match import.module.as_str() {
                    "wasi_snapshot_preview1" | "wasi" => {
                        result.warnings.push(format!(
                            "Module imports from '{}' - WASI support required",
                            import.module
                        ));
                    }
                    "env" => {
                        // Common import module, usually OK
                    }
                    other => {
                        if args.strict {
                            result.warnings.push(format!(
                                "Module imports from unknown module: {}",
                                other
                            ));
                        }
                    }
                }
            }

            // Strict mode checks
            if args.strict {
                if module.metadata().memories.is_empty() {
                    result.warnings.push("Module has no memory".to_string());
                }
            }
        }
        Err(e) => {
            result.valid = false;
            result.errors.push(e.to_string());
        }
    }

    // Output results
    match format {
        OutputFormat::Human => {
            if result.valid {
                println!("Module is valid: {}", args.module.display());
                if let Some(name) = &result.module_name {
                    println!("  Name: {}", name);
                }
                println!("  Exports: {}", result.exports);
                println!("  Imports: {}", result.imports);

                if !result.warnings.is_empty() {
                    println!("\nWarnings:");
                    for warning in &result.warnings {
                        println!("  - {}", warning);
                    }
                }
            } else {
                println!("Module is INVALID: {}", args.module.display());
                for error in &result.errors {
                    println!("  Error: {}", error);
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

    if result.valid {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Validation failed"))
    }
}
