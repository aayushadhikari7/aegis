//! Run command - Execute a WebAssembly module.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args;

use aegis::prelude::*;
use aegis_observe::{ExecutionOutcome, ExecutionReport, ModuleInfo};

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

    /// Arguments to pass to the function (as JSON values)
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
    let mut sandbox = runtime.sandbox().build().context("Failed to create sandbox")?;

    sandbox
        .load_module(&module)
        .context("Failed to load module into sandbox")?;

    // Execute the function
    let start = std::time::Instant::now();
    let result = sandbox.call::<(), ()>(function, ());
    let duration = start.elapsed();

    // Build the report
    let module_info = ModuleInfo {
        name: module.name().map(String::from),
        export_count: module.exports().len(),
        import_count: module.imports().len(),
    };

    let outcome = match &result {
        Ok(()) => ExecutionOutcome::Success { return_value: None },
        Err(e) => ExecutionOutcome::Error {
            message: e.to_string(),
        },
    };

    let metrics = sandbox.metrics().clone();
    let report = ExecutionReport::new(
        module_info,
        outcome,
        aegis_observe::MetricsCollector::new().snapshot(), // Would use actual metrics
    );

    // Output results
    match format {
        OutputFormat::Human => {
            if result.is_ok() {
                if !quiet {
                    println!("Execution completed successfully in {:?}", duration);
                }
                if args.metrics {
                    println!("\nMetrics:");
                    println!("  Duration: {:?}", metrics.duration());
                    println!("  Fuel consumed: {}", metrics.fuel_consumed);
                }
            } else {
                println!("{}", report.to_text());
            }
        }
        OutputFormat::Json | OutputFormat::JsonCompact => {
            let json = if matches!(format, OutputFormat::JsonCompact) {
                serde_json::to_string(&report.to_json())?
            } else {
                report.to_json_pretty()
            };
            println!("{}", json);
        }
    }

    result.map_err(|e| anyhow::anyhow!("Execution failed: {}", e))
}
