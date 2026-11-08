//! Read-only recovery evidence for physical stack headroom; never a grant.
use core::sync::atomic::{AtomicUsize, Ordering};
const MARK: u32 = 0x9ac6e517;
static START: AtomicUsize = AtomicUsize::new(0);
static END: AtomicUsize = AtomicUsize::new(0);
/// Mark only unused physical stack below the current frame, before tasks spawn.
pub fn initialize() {
    unsafe extern "C" {
        static __sheap: u32;
    }
    cortex_m::interrupt::free(|_| {
        let start = core::ptr::addr_of!(__sheap) as usize;
        let end = (cortex_m::register::msp::read() as usize).saturating_sub(256) & !3;
        if start >= end {
            return;
        }
        // SAFETY: the linker places every static owner below __sheap. The
        // downward-growing active stack and this call's frame are above end;
        // interrupts are masked, and no task has been spawned yet.
        for address in (start..end).step_by(4) {
            unsafe {
                core::ptr::write_volatile(address as *mut u32, MARK);
            }
        }
        START.store(start, Ordering::Relaxed);
        END.store(end, Ordering::Relaxed);
    });
}
/// Terminal panic path only: report untouched low stack bytes, without writes.
pub fn spare_bytes() -> Option<usize> {
    let start = START.load(Ordering::Relaxed);
    let end = END.load(Ordering::Relaxed);
    if start == 0 || start >= end {
        return None;
    }
    let mut address = start;
    while address < end {
        // SAFETY: same linker-owned physical stack range marked at boot. The
        // panic handler has masked interrupts and no task can mutate it now.
        if unsafe { core::ptr::read_volatile(address as *const u32) } != MARK {
            break;
        }
        address += 4;
    }
    Some(address - start)
}
