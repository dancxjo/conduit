//! Complete integer and SIMD state preservation across a Nano IRQ.
core::arch::global_asm!(include_str!("vectors.S"));
unsafe extern "C" {
    static conduitos_nano_vectors: u8;
}
pub fn address() -> usize {
    core::ptr::addr_of!(conduitos_nano_vectors) as usize
}
