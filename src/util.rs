#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[inline(always)]
pub fn raw_fenced(clock: &quanta::Clock) -> u64 {
    use std::sync::atomic::{Ordering, compiler_fence};

    compiler_fence(Ordering::SeqCst);
    unsafe {
        core::arch::x86_64::_mm_lfence();
    }
    let t = clock.raw();
    unsafe {
        core::arch::x86_64::_mm_lfence();
    }
    compiler_fence(Ordering::SeqCst);
    t
}

/// Returns the duration of `Median` overhead of 2x[raw_fenced] and 1 read tsc clock
pub fn measure_overhead_ns(clock: &quanta::Clock, num_iterations: usize) -> u64 {
    let mut overhead = Vec::with_capacity(num_iterations);

    for _ in 0..num_iterations {
        let start = raw_fenced(clock);
        let end = raw_fenced(clock);
        let ns = clock.delta_as_nanos(start, end);
        overhead.push(ns);
    }

    overhead.sort_by(|a, b| a.partial_cmp(b).unwrap());
    overhead[overhead.len() / 2]
}
