# aegis-wasm-cli

Command-line interface for the [Aegis](https://crates.io/crates/aegis-wasm) WebAssembly sandbox.

## Installation

```bash
cargo install aegis-wasm-cli
```

## Usage

```bash
# Run a WebAssembly module
aegis run module.wasm --function add -- 2 3

# Run with resource limits
aegis run module.wasm --memory-limit 67108864 --fuel-limit 1000000 --timeout 30

# Run with capabilities
aegis run module.wasm --allow-read /data --allow-logging

# Validate a module
aegis validate module.wasm

# Inspect module exports/imports
aegis inspect module.wasm --all
```

## Commands

- `run` - Execute a WebAssembly module
- `validate` - Validate a WebAssembly module
- `inspect` - Inspect module exports, imports, and metadata

## Output Formats

- `--format human` - Human-readable output (default)
- `--format json` - Pretty JSON
- `--format json-compact` - Compact JSON

## License

MIT OR Apache-2.0
