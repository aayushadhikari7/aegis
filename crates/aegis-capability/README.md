# aegis-capability

Capability-based security system for the [Aegis](https://crates.io/crates/aegis-wasm) WebAssembly sandbox.

This crate provides:
- `Capability` trait for defining permissions
- `CapabilitySet` container with permission checking
- Built-in capabilities:
  - `FilesystemCapability` - Path-based read/write access
  - `NetworkCapability` - Host/protocol allowlists
  - `LoggingCapability` - Level-filtered logging
  - `ClockCapability` - Time access control

## Usage

This is an internal crate. Use [`aegis-wasm`](https://crates.io/crates/aegis-wasm) for the public API.

```toml
[dependencies]
aegis-wasm = "0.1"
```

## License

MIT OR Apache-2.0
