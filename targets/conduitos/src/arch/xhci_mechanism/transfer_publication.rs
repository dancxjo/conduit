//! Class-neutral xHCI transfer TRB publication order.
use core::sync::atomic::{Ordering, fence};

/// Publish payload before granting controller ownership through the cycle bit.
pub(crate) fn publish_transfer(trb: [u32; 4], mut write: impl FnMut(usize, u32)) {
    for (word, value) in trb[..3].iter().copied().enumerate() {
        write(word, value);
    }
    fence(Ordering::Release);
    super::ordering::publish_dma();
    write(3, trb[3]);
}

/// Preserve the existing x86 command store. AArch64 publishes ownership after
/// payload through the native full-system barrier, even if a running controller
/// observes its ring before the subsequent doorbell write.
pub(super) unsafe fn write_command_slot(index: usize, trb: [u32; 4]) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(super::DMA.command_ring[index]), trb);
    }
    #[cfg(target_arch = "aarch64")]
    publish_transfer(trb, |word, value| unsafe {
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(super::DMA.command_ring[index][word]),
            value,
        );
    });
}
