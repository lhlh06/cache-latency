# cache-latency

Pointer-chasing cache latency benchmark for `x86_64`.

The benchmark builds a randomized linked ring of 64-byte nodes, pins execution
to a CPU core, warms up the working set, then measures dependent pointer loads
with TSC timestamps.

## Requirements

- Rust toolchain with edition 2024 support.
- x86_64 CPU with `rdtsc`, `rdtscp`, and `SSE2`.
- `hwloc` development library if building with NUMA support through
  `hwlocality`.

On Debian/Ubuntu, the native dependency is typically:

```sh
sudo apt install libhwloc-dev pkg-config
```

## Run

Example with fewer samples:

```sh
cargo run --release -- 30000 100 --sizes 8kiB,32kiB,1MiB,32MiB --core 0
```

CSV output:

```sh
cargo run --release -- --csv > result.csv
```

Default sizes:

```text
8kiB,16kiB,32kiB,64kiB,128kiB,256kiB,512kiB,1miB,2miB,4miB,8miB,12miB,16miB,20miB,32miB,64miB,128miB,256miB,512miB,1giB,2giB
```

## Output

Normal output is printed to stderr:

```text
Size    8.0 KiB | Min   1.20 ns | Med   1.20ns | Avg   1.20 ns | Max   1.20 ns | ~Cyc   6.0 | 5.06 GHz | TSC OH 30 ticks (0.7170%)
```

CSV columns:

```text
size,min,med,avg,max,~cyc,~freq,tsc_overhead,overhead_pct
```

Fields:

- `min`, `med`, `avg`, `max`: latency per pointer dereference in nanoseconds.
- `~cyc`: rough cycle estimate from `min * reported_frequency`.
- `~freq`: frequency reported by the OS at measurement time.
- `tsc_overhead`: measured TSC start/end overhead in ticks.
- `overhead_pct`: median percentage of raw sample ticks spent on TSC overhead.

## Recommended Usage

For stable results:

- Use `--core` to pin the benchmark.
- Run the release build.
- Avoid other load on the selected core.
- Prefer larger `NUM_ITERATIONS` when `overhead_pct` is above about `1%`.
