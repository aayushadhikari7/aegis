# aegis-resource

Resource management for the [Aegis](https://crates.io/crates/aegis-wasm) WebAssembly sandbox.

This crate provides:
- `ResourceLimiter` - Memory and table growth limits
- `FuelManager` - CPU time limiting via fuel consumption
- `EpochManager` - Wall-clock timeout enforcement
- `ResourceLimits` - Combined configuration

## Usage

This is an internal crate. Use [`aegis-wasm`](https://crates.io/crates/aegis-wasm) for the public API.

```toml
[dependencies]
aegis-wasm = "0.1"
```

## License

MIT OR Apache-2.0
