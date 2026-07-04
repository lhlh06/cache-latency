use std::hint::black_box;
use std::sync::atomic::{Ordering, compiler_fence};
use std::time::{Duration, Instant};

use bytesize::ByteSize;
use core_affinity::CoreId;
use rand::rng;
use rand::seq::SliceRandom;
use seq_macro::seq;

use crate::CliArgs;
use crate::util::{measure_tsc_overhead, tsc_end, tsc_start};

#[derive(Clone, Copy)]
#[cfg(target_arch = "x86_64")]
#[repr(align(64))]
pub struct PaddedNode {
    next: *mut PaddedNode,
}

impl Default for PaddedNode {
    fn default() -> Self {
        Self {
            next: std::ptr::null_mut(),
        }
    }
}

#[cfg(target_arch = "x86_64")]
const _: () = assert!(size_of::<PaddedNode>() == 64);

#[derive(Debug, Default)]
struct Result {
    size: String,
    min: f64,
    med: f64,
    avg: f64,
    max: f64,
    est_cycle: f64,
    est_freq: f64,
    tsc_overhead: u64,
    overhead_ratio: f64,
}

pub fn run_benchmark(core: CoreId, args: &CliArgs) {
    let mut results = Vec::with_capacity(args.sizes.len());

    // pin to selected core
    assert!(
        core_affinity::set_for_current(core),
        "failed to pin benchmark thread to core {}",
        core.id
    );

    for size in &args.sizes {
        assert!(
            size.as_u64() <= usize::MAX as u64,
            "Buffer size exceeds max usize limit!"
        );
        let result = benchmark(
            size.as_u64() as usize,
            core,
            args.num_iterations,
            args.num_samples,
            args,
        );
        results.push(result);
    }

    if args.csv {
        println!("size,min,med,avg,max,~cyc,~freq,tsc_overhead,overhead_pct");
        results.iter().for_each(|result| {
            println!(
                "{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{},{:.4}",
                result.size,
                result.min,
                result.med,
                result.avg,
                result.max,
                result.est_cycle,
                result.est_freq,
                result.tsc_overhead,
                result.overhead_ratio,
            );
        });
    }
}

fn benchmark(
    buffer_size_bytes: usize,
    core: CoreId,
    num_iterations: usize,
    num_samples: usize,
    args: &CliArgs,
) -> Result {
    let mut system = sysinfo::System::new();

    let node_size = size_of::<PaddedNode>();
    let num_elements = buffer_size_bytes / node_size;

    if num_elements < 2 {
        panic!("The number of elemets is too small to run benchmark.");
    }

    let (_arena, mut current_ptr) = build_pointer_ring(num_elements);

    // Warm up the current working set with the same dependent-load pattern
    // used by the measured pointer-chasing loop.
    current_ptr = warmup_pointer_chasing(current_ptr);

    // read frequency after warmup
    system.refresh_cpu_frequency();
    let sys_bench_freq = system.cpus()[core.id].frequency() as f64 / 1000.0;

    // sample
    let mut sample_latencies = Vec::with_capacity(num_samples);
    let mut overhead_ratios = Vec::with_capacity(num_samples);

    // loop unrolling
    const BATCH_SIZE: usize = 16;
    let loop_count = num_iterations / BATCH_SIZE;
    let remainder = num_iterations % BATCH_SIZE;

    let clock = quanta::Clock::new();

    // Warmup again due to reading frequency
    current_ptr = warmup_pointer_chasing(current_ptr);

    let tsc_overhead = measure_tsc_overhead(500);

    for _ in 0..num_samples {
        compiler_fence(Ordering::SeqCst);

        let start = tsc_start();
        for _ in 0..loop_count {
            seq!(_ in 0..16 {
                unsafe {
                    current_ptr = std::ptr::read_volatile(&(*current_ptr).next);
                }
            });
        }

        for _ in 0..remainder {
            unsafe {
                current_ptr = std::ptr::read_volatile(&(*current_ptr).next);
            }
        }
        let end = tsc_end();
        compiler_fence(Ordering::SeqCst);

        let raw_elapsed_ticks = end.saturating_sub(start);
        let elapsed_ticks = if args.subtract_overhead {
            raw_elapsed_ticks.saturating_sub(tsc_overhead)
        } else {
            raw_elapsed_ticks
        };

        let overhead_ratio = if raw_elapsed_ticks == 0 {
            0.0
        } else {
            tsc_overhead as f64 / raw_elapsed_ticks as f64 * 100.0
        };

        let duration_ns = clock.delta_as_nanos(0, elapsed_ticks);
        let latency = duration_ns as f64 / num_iterations as f64;
        sample_latencies.push(latency);
        overhead_ratios.push(overhead_ratio);
    }
    black_box(current_ptr);

    // collect latencies
    let min_latency = sample_latencies
        .iter()
        .fold(f64::INFINITY, |a, &b| a.min(b));
    let max_latency = sample_latencies
        .iter()
        .fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let sum_latency: f64 = sample_latencies.iter().sum();
    let mean_latency = sum_latency / num_samples as f64;
    let median_latency = median(&mut sample_latencies);
    let overhead_ratio = median(&mut overhead_ratios);
    let est_cycles = min_latency * sys_bench_freq;

    eprintln!(
        "Size {:>10} | Min {:>6.2} ns | Med {:>6.2}ns | Avg {:>6.2} ns | Max {:>6.2} ns | ~Cyc {:>5.1} | {:.2} GHz | TSC OH {} ticks ({:.4}%)",
        ByteSize(buffer_size_bytes.try_into().unwrap()),
        min_latency,
        median_latency,
        mean_latency,
        max_latency,
        est_cycles,
        sys_bench_freq,
        tsc_overhead,
        overhead_ratio,
    );

    Result {
        size: ByteSize(buffer_size_bytes.try_into().unwrap()).to_string(),
        min: min_latency,
        med: median_latency,
        max: max_latency,
        est_cycle: est_cycles,
        avg: mean_latency,
        est_freq: sys_bench_freq,
        tsc_overhead,
        overhead_ratio,
    }
}

fn warmup_pointer_chasing(mut current_ptr: *mut PaddedNode) -> *mut PaddedNode {
    let warmup_start = Instant::now();
    let min_warmup_duration = Duration::from_millis(500);

    while warmup_start.elapsed() < min_warmup_duration {
        for _ in 0..10_000 {
            unsafe {
                current_ptr = std::ptr::read_volatile(&(*current_ptr).next);
            }
        }
    }

    current_ptr
}

fn build_pointer_ring(num_elements: usize) -> (Vec<PaddedNode>, *mut PaddedNode) {
    assert!(num_elements >= 2, "pointer ring needs at least two nodes");

    let mut arena = vec![PaddedNode::default(); num_elements];
    let mut indices: Vec<usize> = (0..num_elements).collect();
    indices.shuffle(&mut rng());

    let base_ptr = arena.as_mut_ptr();
    unsafe {
        for pair in indices.windows(2) {
            (*base_ptr.add(pair[0])).next = base_ptr.add(pair[1]);
        }

        let first_idx = indices[0];
        let last_idx = indices[indices.len() - 1];
        (*base_ptr.add(last_idx)).next = base_ptr.add(first_idx);

        (arena, base_ptr.add(first_idx))
    }
}

fn median(values: &mut [f64]) -> f64 {
    assert!(!values.is_empty(), "cannot calculate median of empty data");
    values.sort_by(|a, b| a.total_cmp(b));

    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}
