#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[inline(always)]
pub fn tsc_start() -> u64 {
    use std::{
        arch::x86_64::{_mm_lfence, _rdtsc},
        sync::atomic::{Ordering, compiler_fence},
    };
    unsafe {
        compiler_fence(Ordering::SeqCst);
        _mm_lfence();
        let tsc = _rdtsc();
        compiler_fence(Ordering::SeqCst);

        tsc
    }
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[inline(always)]
pub fn tsc_end() -> u64 {
    use std::{
        arch::x86_64::{__rdtscp, _mm_lfence},
        sync::atomic::{Ordering, compiler_fence},
    };
    let mut aux = 0;
    unsafe {
        compiler_fence(Ordering::SeqCst);
        let tsc: u64 = __rdtscp(&mut aux);
        _mm_lfence();
        compiler_fence(Ordering::SeqCst);

        tsc
    }
}

pub fn measure_tsc_overhead(num_iterations: usize) -> u64 {
    // better be 500 or larger
    let num_iterations = num_iterations.max(500);

    let mut overhead = Vec::with_capacity(num_iterations);

    // warmup
    for _ in 0..100 {
        let _ = tsc_start();
        let _ = tsc_end();
    }

    for _ in 0..num_iterations {
        let start = tsc_start();
        let end = tsc_end();
        overhead.push(end.saturating_sub(start));
    }

    overhead.sort_unstable();
    overhead[overhead.len() / 2]
}
