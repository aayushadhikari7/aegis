<div align="center">

# Aegis

**A secure WebAssembly sandbox runtime for executing untrusted code at near-native speed**

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](#license)

[Features](#features) | [Quick Start](#quick-start) | [Documentation](#documentation) | [Examples](#examples)

</div>

---

## Overview

Aegis enables safe execution of untrusted WebAssembly code through a capability-based security model, strict resource limits, and deterministic execution boundaries. Built on [Wasmtime](https://wasmtime.dev/), it provides JIT-compiled performance while ensuring complete isolation.

## Features

| Feature | Description |
|---------|-------------|
| **Secure Sandboxing** | Strong isolation guarantees with no escape vectors |
| **Capability-Based Security** | Explicit, opt-in permissions with zero ambient authority |
| **Resource Limits** | Configurable memory, CPU (fuel), and wall-clock time limits |
| **Near-Native Performance** | JIT compilation via Wasmtime |
| **Dual Interface** | Embeddable library + standalone CLI tool |
| **Observability** | Built-in metrics collection and event streaming |

## Installation

### From Source

```bash
git clone https://github.com/aayushadhikari7/aegis.git
cd aegis
cargo build --release
```

### As a Dependency

```toml
[dependencies]
aegis = "0.1"
```

## Quick Start

### Library Usage

```rust
use aegis::prelude::*;
use std::time::Duration;

fn main() -> Result<(), AegisError> {
    let runtime = Aegis::builder()
        .with_memory_limit(64 * 1024 * 1024)   // 64 MB
        .with_fuel_limit(1_000_000_000)         // 1B instructions
        .with_timeout(Duration::from_secs(30))
        .build()?;

    let module = runtime.load_file("plugin.wasm")?;
    let mut sandbox = runtime.sandbox().build()?;
    sandbox.load_module(&module)?;

    let result: i32 = sandbox.call("add", (2i32, 3i32))?;
    println!("Result: {}", result);

    Ok(())
}
```

### CLI Usage

```bash
# Execute with limits
aegis run module.wasm -f main --memory-limit 64MB --timeout 10s

# Grant capabilities
aegis run module.wasm --allow-read /data --allow-logging

# Validate module
aegis validate module.wasm

# Inspect exports/imports
aegis inspect module.wasm --all
```

## Security Model

Aegis implements **defense in depth** through multiple isolation layers:

```
    Untrusted WASM Code
           │
    ┌──────▼──────┐
    │  Capability │ ← Explicit permission checks
    │    Layer    │
    ├─────────────┤
    │  Resource   │ ← Memory, CPU, time limits
    │   Limiter   │
    ├─────────────┤
    │  Wasmtime   │ ← Memory-safe execution
    │   Sandbox   │
    └─────────────┘
```

### Principles

1. **No Ambient Authority** - All permissions must be explicitly granted
2. **Fail Secure** - Absence of capability guarantees denial
3. **Least Privilege** - Grant only what's needed
4. **Immutable Grants** - Permissions locked at execution start

### Built-in Capabilities

| Capability | Description |
|------------|-------------|
| `FilesystemCapability` | Path-scoped file read/write |
| `NetworkCapability` | Host and protocol allowlists |
| `LoggingCapability` | Level-filtered logging output |
| `ClockCapability` | Monotonic and real-time access |

### Resource Limits

| Resource | Enforcement |
|----------|-------------|
| **Memory** | Hard limit on linear memory growth |
| **CPU** | Fuel-based instruction counting |
| **Time** | Epoch-based wall-clock timeout |
| **Stack** | Maximum WASM call depth |

## Architecture

```
┌───────────────────────────────────────────────────────────┐
│                     Your Application                       │
├───────────────────────────────────────────────────────────┤
│                        aegis                               │
│  ┌─────────────┬─────────────────┬──────────────────────┐ │
│  │ aegis-core  │ aegis-capability│   aegis-observe      │ │
│  │ engine,     │  permissions,   │   metrics, events,   │ │
│  │ sandbox     │  built-ins      │   reports            │ │
│  ├─────────────┼─────────────────┼──────────────────────┤ │
│  │ aegis-host  │ aegis-resource  │   aegis-cli          │ │
│  │ host funcs  │ limits, fuel    │   CLI interface      │ │
│  └─────────────┴─────────────────┴──────────────────────┘ │
├───────────────────────────────────────────────────────────┤
│                        Wasmtime                            │
└───────────────────────────────────────────────────────────┘
```

### Crate Overview

| Crate | Purpose |
|-------|---------|
| `aegis` | Public API facade with builder pattern |
| `aegis-core` | Wasmtime engine wrapper and sandbox management |
| `aegis-capability` | Capability traits and built-in implementations |
| `aegis-resource` | Memory limiter, fuel manager, epoch handler |
| `aegis-host` | Host function registration and context |
| `aegis-observe` | Metrics collection and event dispatch |
| `aegis-cli` | Command-line interface |

## Configuration

### Programmatic

```rust
let runtime = Aegis::builder()
    // Engine
    .with_async_support(true)

    // Limits
    .with_memory_limit(64 * 1024 * 1024)
    .with_fuel_limit(1_000_000_000)
    .with_timeout(Duration::from_secs(30))

    // Capabilities
    .with_filesystem(FilesystemCapability::read_only(&["/data"]))
    .with_logging(LoggingCapability::production())

    .build()?;
```

### Configuration File (`aegis.toml`)

```toml
[limits]
memory_bytes = 67108864
fuel = 1000000000
timeout_seconds = 30

[capabilities.filesystem]
paths = [
    { path = "/tmp/sandbox", read = true, write = true },
    { path = "/data", read = true, write = false },
]

[capabilities.logging]
enabled = true
min_level = "info"
```

## Examples

### Custom Host Functions

```rust
let mut sandbox = runtime.sandbox().build()?;

sandbox.register_func("env", "log", |val: i32| {
    println!("Guest: {}", val);
})?;

sandbox.load_module(&module)?;
sandbox.call_void("main")?;
```

### Event Streaming

```rust
use std::sync::Arc;

let collector = Arc::new(CollectingSubscriber::new(1000));
let runtime = Aegis::builder()
    .with_event_subscriber(collector.clone())
    .build()?;

// Execute...

for (ts, event) in collector.events() {
    println!("{:?}: {:?}", ts, event);
}
```

## Performance

| Metric | Value |
|--------|-------|
| Cold start | ~1ms (small modules) |
| Host call overhead | <5% with capability checks |
| Memory efficiency | Wasmtime pooling allocator |

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
