//! Run command - Execute a WebAssembly module.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args;

use aegis_observe::{ExecutionOutcome, ExecutionReport, ModuleInfo};
use aegis_wasm::prelude::*;

use crate::OutputFormat;

/// Arguments for the run command.
#[derive(Args)]
pub struct RunArgs {
    /// Path to the WebAssembly module
    #[arg(required = true)]
    pub module: PathBuf,

    /// Function to execute (default: _start or main)
    #[arg(short = 'e', long)]
    pub function: Option<String>,

    /// Arguments to pass to the function
    #[arg(last = true)]
    pub args: Vec<String>,

    /// Memory limit in bytes (default: 64MB)
    #[arg(long, default_value = "67108864")]
    pub memory_limit: usize,

    /// Fuel limit for execution (default: 1B)
    #[arg(long, default_value = "1000000000")]
    pub fuel_limit: u64,

    /// Execution timeout in seconds (default: 30)
    #[arg(long, default_value = "30")]
    pub timeout: u64,

    /// Grant filesystem read access to paths
    #[arg(long = "allow-read")]
    pub allow_read: Vec<PathBuf>,

    /// Grant filesystem read-write access to paths
    #[arg(long = "allow-write")]
    pub allow_write: Vec<PathBuf>,

    /// Enable logging capability
    #[arg(long)]
    pub allow_logging: bool,

    /// Enable clock capability
    #[arg(long)]
    pub allow_clock: bool,

    /// Show execution metrics
    #[arg(long)]
    pub metrics: bool,
}

/// Parse a CLI argument into a WASM value based on expected type.
fn parse_wasm_arg(arg: &str, expected_type: wasmtime::ValType) -> Result<wasmtime::Val> {
    match expected_type {
        wasmtime::ValType::I32 => {
            let val: i32 = arg.parse().context("Expected i32 value")?;
            Ok(wasmtime::Val::I32(val))
        }
        wasmtime::ValType::I64 => {
            let val: i64 = arg.parse().context("Expected i64 value")?;
            Ok(wasmtime::Val::I64(val))
        }
        wasmtime::ValType::F32 => {
            let val: f32 = arg.parse().context("Expected f32 value")?;
            Ok(wasmtime::Val::F32(val.to_bits()))
        }
        wasmtime::ValType::F64 => {
            let val: f64 = arg.parse().context("Expected f64 value")?;
            Ok(wasmtime::Val::F64(val.to_bits()))
        }
        _ => anyhow::bail!("Unsupported parameter type: {:?}", expected_type),
    }
}

/// Format a WASM value for display.
fn format_wasm_val(val: &wasmtime::Val) -> String {
    match val {
        wasmtime::Val::I32(v) => v.to_string(),
        wasmtime::Val::I64(v) => v.to_string(),
        wasmtime::Val::F32(v) => f32::from_bits(*v).to_string(),
        wasmtime::Val::F64(v) => f64::from_bits(*v).to_string(),
        wasmtime::Val::V128(v) => format!("{:?}", v),
        wasmtime::Val::FuncRef(_) => "<funcref>".to_string(),
        wasmtime::Val::ExternRef(_) => "<externref>".to_string(),
        wasmtime::Val::AnyRef(_) => "<anyref>".to_string(),
    }
}

/// Execute the run command.
pub fn execute(args: RunArgs, format: OutputFormat, quiet: bool) -> Result<()> {
    // Build the runtime
    let mut builder = Aegis::builder()
        .with_memory_limit(args.memory_limit)
        .with_fuel_limit(args.fuel_limit)
        .with_timeout(Duration::from_secs(args.timeout));

    // Add capabilities based on flags
    if !args.allow_read.is_empty() {
        builder = builder.with_filesystem(FilesystemCapability::read_only(&args.allow_read));
    }

    if !args.allow_write.is_empty() {
        builder = builder.with_filesystem(FilesystemCapability::read_write(&args.allow_write));
    }

    if args.allow_logging {
        builder = builder.with_logging(LoggingCapability::production());
    }

    if args.allow_clock {
        builder = builder.with_clock(ClockCapability::monotonic_only());
    }

    let runtime = builder.build().context("Failed to create runtime")?;

    // Load the module
    let module = runtime
        .load_file(&args.module)
        .context("Failed to load module")?;

    // Determine the function to call
    let function = args.function.as_deref().unwrap_or_else(|| {
        // Try to find _start or main
        if module.has_export("_start") {
            "_start"
        } else if module.has_export("main") {
            "main"
        } else {
            // Default to first exported function
            module
                .exports()
                .first()
                .map(|e| e.name.as_str())
                .unwrap_or("_start")
        }
    });

    if !quiet {
        tracing::info!(
            module = %args.module.display(),
            function = function,
            "Executing module"
        );
    }

    // Create sandbox and execute
    let mut sandbox = runtime
        .sandbox()
        .build()
        .context("Failed to create sandbox")?;

    sandbox
        .load_module(&module)
        .context("Failed to load module into sandbox")?;

    // Get function signature for argument parsing
    let func_type = sandbox
        .get_func_type(function)
        .context(format!("Function '{}' not found", function))?;

    let param_types: Vec<_> = func_type.params().collect();

    // Validate argument count
    if args.args.len() != param_types.len() {
        anyhow::bail!(
            "Function '{}' expects {} arguments, got {}",
            function,
            param_types.len(),
            args.args.len()
        );
    }

    // Parse arguments
    let wasm_args: Vec<wasmtime::Val> = args
        .args
        .iter()
        .zip(param_types.iter())
        .map(|(arg, ty)| parse_wasm_arg(arg, ty.clone()))
        .collect::<Result<Vec<_>>>()?;

    // Execute the function
    let start = std::time::Instant::now();
    let result = sandbox.call_dynamic(function, wasm_args);
    let duration = start.elapsed();

    // Build the report
    let module_info = ModuleInfo {
        name: module.name().map(String::from),
        export_count: module.exports().len(),
        import_count: module.imports().len(),
    };

    let outcome = match &result {
        Ok(results) => {
            let return_value = if results.is_empty() {
                None
            } else {
                let formatted = results
                    .iter()
                    .map(format_wasm_val)
                    .collect::<Vec<_>>()
                    .join(", ");
                Some(serde_json::Value::String(formatted))
            };
            ExecutionOutcome::Success { return_value }
        }
        Err(e) => ExecutionOutcome::Error {
            message: e.to_string(),
        },
    };

    let metrics = sandbox.metrics().clone();
    let report = ExecutionReport::new(
        module_info,
        outcome.clone(),
        aegis_observe::MetricsCollector::new().snapshot(),
    );

    // Output results
    match format {
        OutputFormat::Human => match &result {
            Ok(results) => {
                if !quiet {
                    if results.is_empty() {
                        println!("Execution completed successfully in {:?}", duration);
                    } else {
                        let formatted: Vec<_> = results.iter().map(format_wasm_val).collect();
                        println!("Result: {}", formatted.join(", "));
                        if !quiet {
                            println!("Completed in {:?}", duration);
                        }
                    }
                }
                if args.metrics {
                    println!("\nMetrics:");
                    println!("  Duration: {:?}", metrics.duration());
                    println!("  Fuel consumed: {}", metrics.fuel_consumed);
                }
            }
            Err(_) => {
                println!("{}", report.to_text());
            }
        },
        OutputFormat::Json | OutputFormat::JsonCompact => {
            let json = if matches!(format, OutputFormat::JsonCompact) {
                serde_json::to_string(&report.to_json())?
            } else {
                report.to_json_pretty()
            };
            println!("{}", json);
        }
    }

    result
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("Execution failed: {}", e))
}
