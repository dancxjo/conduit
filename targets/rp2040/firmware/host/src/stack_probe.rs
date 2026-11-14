//! Read-only recovery evidence for physical stack headroom; never a grant.
use core::sync::atomic::{AtomicUsize, Ordering};
const MARK: u32 = 0x9ac6e517;
static START: AtomicUsize = AtomicUsize::new(0);
static END: AtomicUsize = AtomicUsize::new(0);
static SEEN: AtomicUsize = AtomicUsize::new(0);
static LAST_PHASE: AtomicUsize = AtomicUsize::new(0);
static LAST_SPARE: AtomicUsize = AtomicUsize::new(usize::MAX);
static LAST_MSP: AtomicUsize = AtomicUsize::new(0);
static FIRST_ZERO: AtomicUsize = AtomicUsize::new(0);
// Diagnostic IDs: shared enter/initialized 1/2; prepared 3/4; renderer 5/6;
// admission/Started 7/8; actor construction 9/10; first sample 11/12,
// output 13/14, Mask 15/16, inspection 17/18 (before/after each poll).
// BSP: watermark initialized19; resident init20/21; Root init23/24.

/// One read-only watermark scan per fixed startup phase, never each millisecond.
/// This is single-core diagnostic evidence, not a memory grant or stack bound.
#[inline(never)]
pub fn checkpoint(phase: usize) {
    if phase == 0 || phase >= usize::BITS as usize {
        return;
    }
    let bit = 1usize << phase;
    if SEEN.load(Ordering::Relaxed) & bit != 0 {
        return;
    }
    let first = cortex_m::interrupt::free(|_| {
        let seen = SEEN.load(Ordering::Relaxed);
        if seen & bit != 0 {
            false
        } else {
            SEEN.store(seen | bit, Ordering::Relaxed);
            LAST_PHASE.store(phase, Ordering::Relaxed);
            LAST_SPARE.store(usize::MAX, Ordering::Relaxed);
            LAST_MSP.store(cortex_m::register::msp::read() as usize, Ordering::Relaxed);
            true
        }
    });
    if !first {
        return;
    }
    // Keep interrupts enabled during the scan: the first sample can already
    // own an in-flight timed GPIO capture. An IRQ can lower the watermark while
    // this aligned volatile read proceeds, so attribution is conservative.
    let spare = scan_spare();
    if let Some(spare) = spare {
        LAST_SPARE.store(spare, Ordering::Relaxed);
        if spare == 0 && FIRST_ZERO.load(Ordering::Relaxed) == 0 {
            FIRST_ZERO.store(phase, Ordering::Relaxed);
        }
    }
}

pub fn startup_checkpoint() -> (usize, usize, usize, usize) {
    (
        LAST_PHASE.load(Ordering::Relaxed),
        LAST_SPARE.load(Ordering::Relaxed),
        LAST_MSP.load(Ordering::Relaxed),
        FIRST_ZERO.load(Ordering::Relaxed),
    )
}
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
    scan_spare()
}
fn scan_spare() -> Option<usize> {
    let start = START.load(Ordering::Relaxed);
    let end = END.load(Ordering::Relaxed);
    unsafe extern "C" {
        static __sheap: u32;
        static _stack_start: u32;
    }
    if start != core::ptr::addr_of!(__sheap) as usize
        || start >= end
        || end > core::ptr::addr_of!(_stack_start) as usize
    {
        return None;
    }
    let mut address = start;
    while address < end {
        // SAFETY: same linker-owned physical stack range marked at boot. The
        // terminal path masks interrupts; startup scans are conservative if an
        // IRQ consumes further stack while this single-core scan is paused.
        if unsafe { core::ptr::read_volatile(address as *const u32) } != MARK {
            break;
        }
        address += 4;
    }
    Some(address - start)
}
