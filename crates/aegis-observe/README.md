# aegis-observe

Observability and metrics for the [Aegis](https://crates.io/crates/aegis-wasm) WebAssembly sandbox.

This crate provides:
- `MetricsCollector` - Execution metrics collection
- `ExecutionReport` - Post-execution summaries
- Timing, memory, and fuel consumption tracking

## Usage

This is an internal crate. Use [`aegis-wasm`](https://crates.io/crates/aegis-wasm) for the public API.

```toml
[dependencies]
aegis-wasm = "0.1"
```

## License

MIT OR Apache-2.0
