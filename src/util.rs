#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub fn raw_fenced(clock: &quanta::Clock) -> u64 {
    unsafe {
        core::arch::x86_64::_mm_lfence();
    }
    let t = clock.raw();
    unsafe {
        core::arch::x86_64::_mm_lfence();
    }
    t
}
