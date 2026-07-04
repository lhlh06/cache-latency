# cache-latency

Pointer-chasing cache latency benchmark for `x86_64`.

The benchmark builds a randomized linked ring of 64-byte nodes, pins execution
to a CPU core, warms up the working set, then measures dependent pointer loads
with TSC timestamps.

## Requirements

- Rust toolchain with edition 2024 support.
- x86_64 CPU with `rdtsc`, `rdtscp`, and `SSE2`.

## Run

Example with fewer samples:

```sh
cargo run --release -- 30000 100 --sizes 8kiB,32kiB,1MiB,32MiB --core 0
```

CSV output:

```sh
cargo run --release -- --csv > result.csv
```

## Recommended Usage

For stable results:

- Use `--core` to pin the benchmark.
- Run the release build.
- Avoid other load on the selected core.
- Prefer larger `NUM_ITERATIONS` when `overhead_pct` is above about `1%`.
