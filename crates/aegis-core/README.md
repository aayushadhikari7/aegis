# aegis-core

Core runtime engine for the [Aegis](https://crates.io/crates/aegis-wasm) WebAssembly sandbox.

This crate provides:
- `AegisEngine` - Wasmtime engine wrapper with security defaults
- `Sandbox` - Isolated execution environment
- `ModuleLoader` - WASM module loading and validation
- Core error types and configuration

## Usage

This is an internal crate. Use [`aegis-wasm`](https://crates.io/crates/aegis-wasm) for the public API.

```toml
[dependencies]
aegis-wasm = "0.1"
```

## License

MIT OR Apache-2.0
