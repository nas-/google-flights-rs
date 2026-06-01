# gflights — Claude instructions

## Python bindings parity rule

Whenever the Rust crate (`src/`) changes, update `gflights-py/` accordingly:

- New public API (method, field, type) → expose it in `gflights-py/src/lib.rs` and update `gflights/_gflights.pyi`
- Renamed/removed API → mirror the rename/removal in the bindings
- New config option or filter → add the corresponding parameter to the affected Python method(s)
- New response field → expose it on the relevant Python data class
- Behaviour change → update any affected Python tests (`gflights-py/tests/`)

The two crates must stay in sync at all times on the `feat/python-bindings` branch (and any branch that includes the bindings).
