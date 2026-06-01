# gflights — Claude instructions

## Workspace layout

```
google-flights-rs/
├── src/                    Rust library + CLI
│   ├── bin/cli/            CLI subcommands
│   ├── parsers/            Request builders & response parsers
│   └── requests/           ApiClient, Config, retry logic
├── gflights-py/            Python bindings (pyo3 + maturin)
│   ├── src/lib.rs          Rust extension (_gflights)
│   ├── gflights/           Python package (re-exports, stubs, types)
│   └── tests/              Python test suite
├── benches/                Criterion benchmarks
└── tests/                  Rust integration / live API tests
```

---

## Git workflow

- **Never commit directly to `main`** — always use a feature branch and open a PR.
- Branch naming: `feat/<topic>`, `fix/<topic>`, `chore/<topic>`.

---

## Before every commit — run locally first

Always verify locally before pushing. CI runs the same checks and failing there wastes time.

### Rust crate

```sh
cargo fmt                                        # format (required — CI blocks on diff)
cargo clippy --all-targets -- -D warnings        # lint (zero warnings policy)
cargo test --lib                                 # 152 unit tests
cargo test --bin gflights                        # 13 CLI tests
cargo test --doc                                 # doc tests
cargo build --benches                            # ensure benchmarks still compile
```

### Python bindings (run from `gflights-py/`)

```sh
cd gflights-py
python -m maturin develop                        # rebuild extension after Rust changes
.venv/Scripts/pytest.exe tests/test_import.py tests/test_types.py tests/test_errors.py -v
```

All offline tests must pass before pushing.

---

## Live / integration tests

These hit the real Google Flights API — run manually, not in CI.

### Rust
```sh
RUN_LIVE_TESTS=1 cargo test --test live_api -- --ignored
```

### Python
```sh
cd gflights-py
RUN_LIVE_TESTS=1 .venv/Scripts/pytest.exe tests/test_live.py -v
```

---

## Test coverage

Keep line coverage **≥ 80%** for the Rust crate.

```sh
cargo install cargo-tarpaulin          # one-time
cargo tarpaulin --out Stdout           # check coverage
```

Current baseline: **84%** overall (parsers 84–99%; `api.rs` ~26% — network code, accepted).
If coverage drops below 80%, add tests before merging.

---

## Python bindings parity rule

Whenever `src/` (the Rust crate) changes, update `gflights-py/` to match:

| Rust change | Bindings update needed |
|---|---|
| New public method / field / type | Expose in `gflights-py/src/lib.rs`; add to `gflights/_gflights.pyi` |
| Renamed / removed API | Mirror in bindings |
| New `Config` option or filter | Add parameter to the affected Python method(s) |
| New response field | Expose on the relevant Python data class |
| Behaviour change | Update affected Python tests |

The Rust crate and Python bindings must stay in sync at all times.

---

## Building the Python extension

```sh
cd gflights-py
uv venv --python 3.11 .venv            # one-time
uv pip install maturin pytest pytest-asyncio
python -m maturin develop              # build + install into .venv
```

After any change to `gflights-py/src/lib.rs`, re-run `maturin develop` before running Python tests.

---

## Security & dependency hygiene

```sh
cargo audit                            # check for CVEs (runs in CI)
```

Zero CVE policy — fix or justify any advisory before merging.

---

## Benchmarks

```sh
cargo bench                            # must be run from the project root (test_files/ paths)
```

Benchmarks are in `benches/parse.rs`. They use fixtures from `test_files/`.
Do not move or rename fixtures without updating the benchmark.

---

## Publishing (Rust crate)

`cargo publish --dry-run` must be clean before tagging a release.
The crate is live at https://crates.io/crates/gflights.
