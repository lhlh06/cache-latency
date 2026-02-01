use std::hint::black_box;

use std::time::{Duration, Instant};

use bytesize::ByteSize;
use core_affinity::CoreId;
use rand::rng;
use rand::seq::SliceRandom;
use seq_macro::seq;

#[derive(Clone, Copy)]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
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

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
const _: () = assert!(size_of::<PaddedNode>() == 64);

pub fn run_benchmark(buffer_size_bytes: usize, core: CoreId, iterations: usize, samples: usize) {
    core_affinity::set_for_current(core);
    let mut system = sysinfo::System::new();

    let node_size = size_of::<PaddedNode>();
    let num_elements = buffer_size_bytes / node_size;

    if num_elements < 2 {
        println!("The number of elemets is too small to run benchmark.");
        return;
    }

    // Assign memory layout
    let mut arena: Vec<PaddedNode> = Vec::with_capacity(num_elements);
    for _ in 0..num_elements {
        arena.push(PaddedNode::default());
    }

    // Random Shuffle
    let mut indices: Vec<usize> = (0..num_elements).collect();
    let mut rng = rng();
    indices.shuffle(&mut rng);

    // Pointer Swizzling
    let base_ptr = arena.as_mut_ptr();

    unsafe {
        for i in 0..num_elements - 1 {
            let curr_idx = indices[i];
            let next_idx = indices[i + 1];

            let curr_node_ptr = base_ptr.add(curr_idx);
            let next_node_ptr = base_ptr.add(next_idx);

            (*curr_node_ptr).next = next_node_ptr;
        }

        let last_idx = indices[num_elements - 1];
        let first_idx = indices[0];
        (*base_ptr.add(last_idx)).next = base_ptr.add(first_idx);
    }

    let mut current_ptr = unsafe { base_ptr.add(indices[0]) };

    // Warmup
    {
        let warmup_start = Instant::now();
        let min_warmup_duration = Duration::from_millis(500);

        while warmup_start.elapsed() < min_warmup_duration {
            for _ in 0..1_000 {
                unsafe {
                    current_ptr = (*current_ptr).next;
                }
            }
        }
    }

    // read frequency after warmup
    system.refresh_cpu_frequency();
    let sys_bench_freq = system.cpus()[core.id].frequency() as f64 / 1000.0;

    // Warmup again due to `read frequency`
    {
        let warmup_start = Instant::now();
        let min_warmup_duration = Duration::from_millis(500);
        while warmup_start.elapsed() < min_warmup_duration {
            for _ in 0..1_000 {
                unsafe {
                    current_ptr = (*current_ptr).next;
                }
            }
        }
    }

    // NOTE: Pointer Chasing

    // sample
    let iterations_per_sample = iterations / samples;
    let mut sample_latencies = Vec::with_capacity(samples);
    // loop unrolling
    let batch_size = 16;
    let loop_count = iterations_per_sample / batch_size;
    let remainder = iterations_per_sample % batch_size;

    let clock = quanta::Clock::new();

    for _ in 0..samples {
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);

        let start = clock.raw();
        for _ in 0..loop_count {
            seq!(_ in 0..16 {
                unsafe {
                    current_ptr = (*current_ptr).next;
                }
            });
        }

        for _ in 0..remainder {
            unsafe {
                current_ptr = (*current_ptr).next;
            }
        }
        let end = clock.raw();
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);

        // let cycles = end - start;
        let duration_ns = clock.delta_as_nanos(start, end);
        let latency = duration_ns as f64 / iterations_per_sample as f64;
        sample_latencies.push(latency);
    }
    black_box(current_ptr);

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
