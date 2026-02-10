#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[allow(unused)]
#[inline(always)]
pub fn raw_fenced(clock: &quanta::Clock) -> u64 {
    use std::sync::atomic::{Ordering, compiler_fence};

    unsafe {
        compiler_fence(Ordering::SeqCst);
        core::arch::x86_64::_mm_lfence();
        let t = clock.raw();
        core::arch::x86_64::_mm_lfence();
        compiler_fence(Ordering::SeqCst);
        t
    }
}

/// Returns the duration of `Median` overhead of 2x[raw_fenced] and 1 read tsc clock
#[allow(unused)]
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
        _mm_lfence();
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
