use std::hint::black_box;

use std::time::{Duration, Instant};

use bytesize::ByteSize;
use core_affinity::CoreId;
use rand::rng;
use rand::seq::SliceRandom;
use seq_macro::seq;

use crate::CliArgs;
use crate::topo::SystemTopology;

#[derive(Clone, Copy, Default)]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[repr(align(64))]
pub struct PaddedNode {
    next_index: usize,
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
const _: () = assert!(size_of::<PaddedNode>() == 64);

pub fn run_benchmark(
    buffer_size_bytes: usize,
    core: CoreId,
    iterations: usize,
    samples: usize,
    args: &CliArgs,
) {
    if args.numa {
        let topo = SystemTopology::new();
        topo.bind(core.id, 0);
    } else {
        core_affinity::set_for_current(core);
    }

    let mut system = sysinfo::System::new();

    let node_size = size_of::<PaddedNode>();
    let num_elements = buffer_size_bytes / node_size;

    if num_elements < 2 {
        println!("The number of elemets is too small to run benchmark.");
        return;
    }

    let mut arena: Vec<PaddedNode> = vec![PaddedNode::default(); num_elements];

    // Random Shuffle
    let mut indices: Vec<usize> = (0..num_elements).collect();
    let mut rng = rng();
    indices.shuffle(&mut rng);

    // indices[i] -> indices[i+1]
    for i in 0..num_elements - 1 {
        let curr_idx = indices[i];
        let next_idx = indices[i + 1];
        arena[curr_idx].next_index = next_idx;
    }

    let last_idx = indices[num_elements - 1];
    let first_idx = indices[0];
    arena[last_idx].next_index = first_idx;

    let mut current_idx = first_idx;

    // Warmup
    {
        let warmup_start = Instant::now();
        let min_warmup_duration = Duration::from_millis(500);

        while warmup_start.elapsed() < min_warmup_duration {
            for _ in 0..1_000 {
                unsafe {
                    current_idx = black_box(arena.get_unchecked(current_idx).next_index);
                }
            }
        }
    }

    // read frequency after warmup
    system.refresh_cpu_frequency();
    let sys_bench_freq = system.cpus()[core.id].frequency() as f64 / 1000.0;

    current_idx = black_box(current_idx);

    // NOTE: Pointer Chasing by index

    // sample
    let iterations_per_sample = iterations / samples;
    let mut sample_latencies = Vec::with_capacity(samples);
    // loop unrolling
    let batch_size = 16;
    let loop_count = iterations_per_sample / batch_size;
    let remainder = iterations_per_sample % batch_size;

    let clock = quanta::Clock::new();

    // warmup again due to `read frequency`
    {
        let warmup_start = Instant::now();
        let min_warmup_duration = Duration::from_millis(500);
        while warmup_start.elapsed() < min_warmup_duration {
            for _ in 0..1_000 {
                unsafe {
                    current_idx = black_box(arena.get_unchecked(current_idx).next_index);
                }
            }
        }
    }

    for _ in 0..samples {
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);

        let start = clock.raw();
        for _ in 0..loop_count {
            unsafe {
                seq!(_ in 0..16 {
                        current_idx = arena.get_unchecked(current_idx).next_index;
                });
            }
        }

        for _ in 0..remainder {
            unsafe {
                current_idx = arena.get_unchecked(current_idx).next_index;
            }
        }
        let end = clock.raw();
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);

        // let cycles = end - start;
        let duration_ns = clock.delta_as_nanos(start, end);
        let latency = duration_ns as f64 / iterations_per_sample as f64;
        sample_latencies.push(latency);
    }
    black_box(current_idx);

    let min_latency = sample_latencies
        .iter()
        .fold(f64::INFINITY, |a, &b| a.min(b));

    let max_latency = sample_latencies
        .iter()
        .fold(f64::NEG_INFINITY, |a, &b| a.max(b));

    let sum_latency: f64 = sample_latencies.iter().sum();
    let mean_latency = sum_latency / samples as f64;

    let est_cycles = min_latency * sys_bench_freq;

    // TODO: show data in table format
    println!(
        "Size {:>10} | Min {:>6.2} ns | Avg {:>6.2} ns | Max {:>6.2} ns | ~Cyc {:>5.1} | {:.2} GHz",
        ByteSize(buffer_size_bytes.try_into().unwrap()),
        min_latency,
        mean_latency,
        max_latency,
        est_cycles,
        sys_bench_freq
    );
}
