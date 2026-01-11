<div align="center">

# Aegis

**Run untrusted WebAssembly code safely**

[![Crates.io](https://img.shields.io/crates/v/aegis-wasm.svg)](https://crates.io/crates/aegis-wasm)
[![docs.rs](https://img.shields.io/docsrs/aegis-wasm)](https://docs.rs/aegis-wasm)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](#license)

[Installation](#installation) | [CLI Usage](#cli-usage) | [Library Usage](#library-usage) | [Features](#features)

</div>

---

## What is Aegis?

Aegis is a **WebAssembly sandbox** that lets you run untrusted code without risk. The code:

- Cannot access your filesystem (unless you allow it)
- Cannot access the network (unless you allow it)
- Cannot use unlimited memory (you set the limit)
- Cannot run forever (you set the timeout)
- Cannot crash your application

**Use cases:** Plugin systems, serverless functions, game mods, user-submitted code, CI/CD isolation, safe scripting.

---

## Installation

### CLI Tool

```bash
cargo install aegis-wasm-cli
```

### As a Library

```bash
cargo add aegis-wasm
```

Or add to your `Cargo.toml`:

```toml
[dependencies]
aegis-wasm = "0.1"
```

---

## CLI Usage

### Running WebAssembly

```bash
# Run a function with arguments
aegis run module.wasm --function add -- 5 10
# Output: Result: 15

# Run the default function (_start or main)
aegis run module.wasm

# Run with resource limits
aegis run module.wasm --function process \
    --memory-limit 33554432 \
    --fuel-limit 1000000 \
    --timeout 10

# Show execution metrics
aegis run module.wasm --function main --metrics
```

### Granting Permissions

By default, WASM code has **zero permissions**. You grant what it needs:

```bash
# Allow reading from /data directory
aegis run module.wasm --allow-read /data

# Allow reading and writing to /tmp
aegis run module.wasm --allow-write /tmp

# Allow logging output
aegis run module.wasm --allow-logging

# Allow access to system clock
aegis run module.wasm --allow-clock

# Combine permissions
aegis run module.wasm \
    --allow-read /data \
    --allow-write /tmp \
    --allow-logging
```

### Validating Modules

Check if a WASM file is valid before running:

```bash
aegis validate module.wasm
# Output: Module is valid

aegis validate corrupt.wasm
# Output: Module is invalid: ...
```

### Inspecting Modules

See what a WASM module contains:

```bash
# Show everything
aegis inspect module.wasm --all

# Show only exports (functions you can call)
aegis inspect module.wasm --exports

# Show only imports (what the module needs)
aegis inspect module.wasm --imports
```

Example output:
```
Module: plugin.wasm

Exports (3):
  add [function]: (i32, i32) -> (i32)
  multiply [function]: (i32, i32) -> (i32)
  memory [memory]: 1 pages

Imports (1):
  env.log [function]: (i32) -> ()
```

### Output Formats

```bash
# Human-readable (default)
aegis run module.wasm --function add -- 2 3

# JSON output
aegis run module.wasm --function add --format json -- 2 3

# Compact JSON (single line)
aegis run module.wasm --function add --format json-compact -- 2 3
```

### All CLI Options

```
aegis run <MODULE> [OPTIONS] [-- ARGS...]

Options:
  -e, --function <NAME>     Function to call (default: _start or main)
  --memory-limit <BYTES>    Max memory in bytes (default: 64MB)
  --fuel-limit <UNITS>      Max CPU fuel units (default: 1B)
  --timeout <SECONDS>       Max execution time (default: 30s)
  --allow-read <PATH>       Grant read access to path
  --allow-write <PATH>      Grant read/write access to path
  --allow-logging           Enable logging output
  --allow-clock             Enable clock/time access
  --metrics                 Show execution metrics
  -f, --format <FORMAT>     Output format: human, json, json-compact
  -v, --verbose             Increase verbosity (-v, -vv, -vvv)
  -q, --quiet               Suppress non-essential output
```

---

## Library Usage

### Basic Example

```rust
use aegis_wasm::prelude::*;
use std::time::Duration;

fn main() -> Result<(), AegisError> {
    // Create a sandboxed runtime
    let runtime = Aegis::builder()
        .with_memory_limit(64 * 1024 * 1024)  // 64 MB max
        .with_fuel_limit(1_000_000_000)        // 1 billion instructions max
        .with_timeout(Duration::from_secs(30)) // 30 second timeout
        .build()?;

    // Load a WASM module
    let module = runtime.load_file("plugin.wasm")?;

    // Create a sandbox and run code
    let mut sandbox = runtime.sandbox().build()?;
    sandbox.load_module(&module)?;

    // Call a function
    let result: i32 = sandbox.call("add", (2i32, 3i32))?;
    println!("2 + 3 = {}", result);

    Ok(())
}
```

### Loading WASM from Different Sources

```rust
// From a file
let module = runtime.load_file("plugin.wasm")?;

// From bytes (e.g., uploaded by user)
let wasm_bytes: Vec<u8> = receive_upload();
let module = runtime.load_bytes(&wasm_bytes)?;

// From WAT text format (useful for testing)
let module = runtime.load_wat(r#"
    (module
        (func (export "double") (param i32) (result i32)
            local.get 0
            i32.const 2
            i32.mul
        )
    )
"#)?;
```

### Granting Capabilities

```rust
use aegis_wasm::prelude::*;

let runtime = Aegis::builder()
    .with_memory_limit(64 * 1024 * 1024)
    .with_fuel_limit(1_000_000_000)

    // Allow reading from specific paths
    .with_filesystem(FilesystemCapability::read_only(&["/data", "/config"]))

    // Allow reading AND writing to specific paths
    .with_filesystem(FilesystemCapability::read_write(&["/tmp/sandbox"]))

    // Allow logging
    .with_logging(LoggingCapability::all())

    // Allow clock access
    .with_clock(ClockCapability::monotonic_only())

    .build()?;
```

### Registering Host Functions

Let WASM code call your Rust functions:

```rust
let mut sandbox = runtime.sandbox().build()?;

// Register a function that WASM can call
sandbox.register_func("env", "print_number", |value: i32| {
    println!("WASM says: {}", value);
})?;

sandbox.register_func("env", "add_numbers", |a: i32, b: i32| -> i32 {
    a + b
})?;

sandbox.load_module(&module)?;
sandbox.call_void("main")?;
```

### Getting Execution Metrics

```rust
let mut sandbox = runtime.sandbox().build()?;
sandbox.load_module(&module)?;

let result: i32 = sandbox.call("compute", (input,))?;

// Check how much resources were used
let metrics = sandbox.metrics();
println!("Execution time: {:?}", metrics.duration());
println!("Fuel consumed: {}", metrics.fuel_consumed);
println!("Peak memory: {} bytes", metrics.peak_memory);
```

### Handling Errors

```rust
use aegis_wasm::prelude::*;

match sandbox.call::<(i32,), i32>("process", (input,)) {
    Ok(result) => println!("Success: {}", result),

    Err(ExecutionError::OutOfFuel { consumed, limit }) => {
        println!("Code used too much CPU: {} / {}", consumed, limit);
    }

    Err(ExecutionError::Timeout(duration)) => {
        println!("Code took too long: {:?}", duration);
    }

    Err(ExecutionError::MemoryExceeded { used, limit }) => {
        println!("Code used too much memory: {} / {}", used, limit);
    }

    Err(ExecutionError::Trap(info)) => {
        println!("Code crashed: {}", info.message);
    }

    Err(e) => println!("Other error: {}", e),
}
```

### Reusing Sandboxes

```rust
let mut sandbox = runtime.sandbox().build()?;
sandbox.load_module(&module)?;

// Process multiple inputs with the same sandbox
for input in inputs {
    let result: i32 = sandbox.call("process", (input,))?;
    println!("Result: {}", result);
}

// Or reset and reuse
sandbox.reset();
sandbox.load_module(&another_module)?;
```

---

## Features

### Resource Limits

| Resource | What it Limits | Default |
|----------|---------------|---------|
| **Memory** | Max RAM the code can use | 64 MB |
| **Fuel** | Max CPU instructions (deterministic) | 1 billion |
| **Timeout** | Max wall-clock time | 30 seconds |
| **Stack** | Max call stack depth | 512 KB |

### Capabilities (Permissions)

| Capability | What it Allows |
|------------|---------------|
| **Filesystem** | Read/write specific directories |
| **Network** | Connect to specific hosts |
| **Logging** | Print output |
| **Clock** | Access system time |

**Principle:** Code has **zero** permissions by default. You explicitly grant what it needs.

### Supported Value Types

The CLI and library support these WASM types:

| Type | Example |
|------|---------|
| `i32` | `42`, `-17` |
| `i64` | `9999999999` |
| `f32` | `3.14` |
| `f64` | `3.141592653589793` |

---

## Security Model

```
┌─────────────────────────────────────┐
│         Untrusted WASM Code         │
├─────────────────────────────────────┤
│  Capability Layer (Permissions)     │  ← Can it access this resource?
├─────────────────────────────────────┤
│  Resource Limiter (Memory/CPU)      │  ← Has it exceeded limits?
├─────────────────────────────────────┤
│  Wasmtime Sandbox (Memory Safety)   │  ← Is the code valid?
└─────────────────────────────────────┘
```

**Guarantees:**
1. Code cannot access anything you don't explicitly allow
2. Code cannot use more resources than you allocate
3. Code cannot crash your application
4. Code cannot escape the sandbox

---

## Project Structure

| Crate | Description |
|-------|-------------|
| [`aegis-wasm`](https://crates.io/crates/aegis-wasm) | Main library - start here |
| [`aegis-wasm-cli`](https://crates.io/crates/aegis-wasm-cli) | Command-line tool |
| [`aegis-core`](https://crates.io/crates/aegis-core) | Low-level engine and sandbox |
| [`aegis-capability`](https://crates.io/crates/aegis-capability) | Permission system |
| [`aegis-resource`](https://crates.io/crates/aegis-resource) | Memory/CPU/time limits |
| [`aegis-host`](https://crates.io/crates/aegis-host) | Host function registration |
| [`aegis-observe`](https://crates.io/crates/aegis-observe) | Metrics and monitoring |

---

## Requirements

- **Rust 1.85+**
- Works on Linux, macOS, and Windows

---

## License

Dual-licensed under:

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

Choose whichever you prefer.

---

## Contributing

Contributions welcome! Feel free to:

- Open issues for bugs or feature requests
- Submit pull requests
- Improve documentation

---

<div align="center">

**[GitHub](https://github.com/aayushadhikari7/aegis)** | **[Crates.io](https://crates.io/crates/aegis-wasm)** | **[Docs](https://docs.rs/aegis-wasm)** | **[Issues](https://github.com/aayushadhikari7/aegis/issues)**

</div>
