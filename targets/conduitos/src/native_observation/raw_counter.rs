//! Shared raw x86 clock, without frequency or wall-time inference.
pub(super) fn read() -> u64 {
    let low: u32;
    let high: u32;
    // x86_64 guarantees SSE2. The memory clobber fences compiler motion too.
    unsafe {
        core::arch::asm!("lfence", "rdtsc", "lfence", out("eax") low,
            out("edx") high, options(nostack));
    }
    (u64::from(high) << 32) | u64::from(low)
}
