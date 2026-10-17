//! Proof-only observation of the ABI state seen by actual Root IRQ handlers.
use crate::protected_region::DomainRefusal;
use core::sync::atomic::{AtomicBool, Ordering};

static DIRECTION_FLAG_LEAKED: AtomicBool = AtomicBool::new(false);

pub(super) fn observe_irq() {
    let flags: usize;
    unsafe {
        #[cfg(target_arch = "x86")]
        core::arch::asm!("pushfd", "pop {0:e}", out(reg) flags, options(preserves_flags));
        #[cfg(target_arch = "x86_64")]
        core::arch::asm!("pushfq", "pop {0}", out(reg) flags, options(preserves_flags));
    }
    if flags & (1 << 10) != 0 {
        DIRECTION_FLAG_LEAKED.store(true, Ordering::Release);
    }
}

pub(super) fn finish() -> Result<(), DomainRefusal> {
    if DIRECTION_FLAG_LEAKED.swap(false, Ordering::AcqRel) {
        Err(DomainRefusal::InvalidLifecycle)
    } else {
        Ok(())
    }
}
