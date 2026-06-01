//! Parsing benchmarks — network latency dominates end-to-end, so only the
//! pure parsing path is benchmarked here.
//!
//! Fixtures are the same files used by the unit-test suite in
//! `src/parsers/response/flight_response.rs`.
//!
//! Run from the project root:
//! ```text
//! cargo bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gflights::parsers::flight_response::{
    create_raw_response_vec, FlightResponseContainer, RawResponse,
};
use std::fs;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn load_fixture(path: &str) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("benchmark fixture not found: {path} — run from project root"))
}

fn parse_inner(fixture: &str) -> RawResponse {
    RawResponse::try_from(fixture).expect("fixture must be valid")
}

fn parse_container(fixture: String) -> FlightResponseContainer {
    create_raw_response_vec(fixture).expect("fixture must be valid")
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

/// Parse the inner `RawResponse` JSON (no outer wrb.fr envelope).
/// Uses the same `lux_tokyo_oneway.txt` fixture as the existing unit tests.
fn bench_parse_inner_response(c: &mut Criterion) {
    let fixture = load_fixture("test_files/lux_tokyo_oneway.txt");
    c.bench_function("parse_inner_response/lux_tokyo", |b| {
        b.iter(|| parse_inner(black_box(&fixture)))
    });

    // Also bench a larger fixture to see how throughput scales with size.
    let large_fixture = load_fixture("test_files/lux_dubai_oneway.txt");
    c.bench_function("parse_inner_response/lux_dubai", |b| {
        b.iter(|| parse_inner(black_box(&large_fixture)))
    });
}

/// Parse the full outer wrb.fr response through `create_raw_response_vec`.
/// This exercises the entire pipeline: outer decode → inner decode → struct
/// construction.
fn bench_parse_flight_response_vec(c: &mut Criterion) {
    let fixture = load_fixture("test_files/response_with_first_fixed_full.txt");
    c.bench_function("parse_flight_response_vec", |b| {
        b.iter_batched(
            || fixture.clone(),
            |input| parse_container(black_box(input)),
            criterion::BatchSize::SmallInput,
        )
    });
}

/// Benchmark `get_all_flights()` (deduplication + collection) in isolation,
/// separate from the initial parse cost.
fn bench_get_all_flights(c: &mut Criterion) {
    let fixture = load_fixture("test_files/response_with_first_fixed_full.txt");
    let container = parse_container(fixture);

    c.bench_function("get_all_flights", |b| {
        b.iter(|| container.get_all_flights())
    });
}

/// Compare parsing throughput across differently-sized response files.
fn bench_parse_size_comparison(c: &mut Criterion) {
    let fixtures = [
        ("lux_milan_oneway", "test_files/lux_milan_oneway.txt"),
        ("lux_tokyo_oneway", "test_files/lux_tokyo_oneway.txt"),
        ("lux_dubai_oneway", "test_files/lux_dubai_oneway.txt"),
    ];

    let mut group = c.benchmark_group("parse_inner_by_size");
    for (name, path) in &fixtures {
        let fixture = load_fixture(path);
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &fixture,
            |b, f| b.iter(|| parse_inner(black_box(f))),
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_parse_inner_response,
    bench_parse_flight_response_vec,
    bench_get_all_flights,
    bench_parse_size_comparison,
);
criterion_main!(benches);
