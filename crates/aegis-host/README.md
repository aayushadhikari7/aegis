# aegis-host

Host function system for the [Aegis](https://crates.io/crates/aegis-wasm) WebAssembly sandbox.

This crate provides:
- `AegisLinker` - Safe wrapper around Wasmtime's linker
- `HostContext` - Context available to host functions
- Automatic capability checks for host function calls

## Usage

This is an internal crate. Use [`aegis-wasm`](https://crates.io/crates/aegis-wasm) for the public API.

```toml
[dependencies]
aegis-wasm = "0.1"
```

## License

MIT OR Apache-2.0
