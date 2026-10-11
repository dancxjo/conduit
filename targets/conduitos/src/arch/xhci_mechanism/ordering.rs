//! Native DMA/device ordering; no architecture is inferred from USB policy.
//! x86 retains its existing coherent strongly ordered register path. QEMU virt
//! uses coherent RAM and Device-nGnRnE registers with full-system barriers.

#[inline]
pub(super) fn publish_dma() {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[inline]
pub(super) fn acquire_dma() {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[inline]
pub(super) fn complete_device_write() {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}
