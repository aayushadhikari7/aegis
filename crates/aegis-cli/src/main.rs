//! Aegis CLI - Command-line interface for the Aegis WebAssembly sandbox.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::EnvFilter;

mod commands;

/// Aegis WebAssembly Sandbox Runtime
#[derive(Parser)]
#[command(name = "aegis")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Subcommand to run
    #[command(subcommand)]
    pub command: Commands,

    /// Configuration file path
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Output format
    #[arg(short = 'f', long, global = true, default_value = "human")]
    pub format: OutputFormat,

    /// Verbosity level (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Quiet mode (suppress non-essential output)
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

/// Output format options.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable output
    #[default]
    Human,
    /// JSON output
    Json,
    /// Compact JSON (single line)
    JsonCompact,
}

/// Available commands.
#[derive(Subcommand)]
pub enum Commands {
    /// Execute a WebAssembly module
    Run(commands::run::RunArgs),
    /// Validate a WebAssembly module
    Validate(commands::validate::ValidateArgs),
    /// Inspect a WebAssembly module
    Inspect(commands::inspect::InspectArgs),
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Initialize logging based on verbosity
    let log_level = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("aegis={}", log_level)));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // Run the command
    let result = match cli.command {
        Commands::Run(args) => commands::run::execute(args, cli.format, cli.quiet),
        Commands::Validate(args) => commands::validate::execute(args, cli.format),
        Commands::Inspect(args) => commands::inspect::execute(args, cli.format),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if !cli.quiet {
                eprintln!("Error: {:#}", e);
            }
            ExitCode::FAILURE
        }
    }
}
