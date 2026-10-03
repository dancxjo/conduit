//! CPU/device ordering for admitted little-endian 32-bit register operations.
use super::RegisterRefusal;
use core::sync::atomic::{Ordering, compiler_fence};

pub(super) fn supported() -> Result<(), RegisterRefusal> {
    if cfg!(any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "loongarch64"
    )) {
        Ok(())
    } else {
        Err(RegisterRefusal::UnsupportedOrdering)
    }
}

pub(super) fn before() {
    barrier();
}
pub(super) fn after() {
    barrier();
}
fn barrier() {
    compiler_fence(Ordering::SeqCst);
    // Mapping/cache attributes remain the architecture resource owner's duty.
    // These barriers provide ordering; they do not perform DMA cache maintenance.
    #[cfg(target_arch = "x86")]
    core::sync::atomic::fence(Ordering::SeqCst);
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!("mfence", options(nostack, preserves_flags));
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    #[cfg(target_arch = "riscv64")]
    unsafe {
        core::arch::asm!("fence iorw, iorw", options(nostack));
    }
    #[cfg(target_arch = "loongarch64")]
    unsafe {
        core::arch::asm!("dbar 0", options(nostack));
    }
    compiler_fence(Ordering::SeqCst);
}
