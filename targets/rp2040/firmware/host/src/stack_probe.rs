//! Read-only recovery evidence for physical stack headroom; never a grant.
use core::cell::RefCell;
use core::sync::atomic::{AtomicUsize, Ordering};
static TRACE: critical_section::Mutex<RefCell<crate::phase_trace::Trace>> =
    critical_section::Mutex::new(RefCell::new(crate::phase_trace::Trace::new()));
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
    let (used, peak) = crate::heap::usage();
    // Keep interrupts enabled during the scan: the first sample can already
    // own an in-flight timed GPIO capture. An IRQ can lower the watermark while
    // this aligned volatile read proceeds, so attribution is conservative.
    let spare = scan_spare();
    critical_section::with(|cs| {
        TRACE.borrow(cs).borrow_mut().record(
            phase,
            used,
            peak,
            LAST_MSP.load(Ordering::Relaxed),
            spare,
        );
    });
    if let Some(spare) = spare {
        LAST_SPARE.store(spare, Ordering::Relaxed);
        if spare == 0 && FIRST_ZERO.load(Ordering::Relaxed) == 0 {
            FIRST_ZERO.store(phase, Ordering::Relaxed);
        }
    }
}

/// Observe the actual Root lease once, before Root initialization. This is
/// coarse custody of the complete arena, not actual occupancy or retirement.
pub fn retain_arena(bytes: usize) {
    critical_section::with(|cs| {
        let accepted = TRACE.borrow(cs).borrow_mut().retain_arena(bytes);
        debug_assert!(accepted);
    });
}

/// Copy completed first-phase observations into existing inspection storage.
/// No heap allocation, scan, grant, or change to any owner lifetime occurs here.
pub fn encode_trace(output: &mut [u8]) -> Option<usize> {
    critical_section::with(|cs| TRACE.borrow(cs).borrow().encode_into(output))
}

pub const MEMORY_REPORT_BYTES: usize = 40 + crate::phase_trace::ENCODED_BYTES;

/// Existing build-bound maintenance observation, available beside either
/// carriage. Static IMAGE storage already includes the retained phase trace.
/// The caller's fixed reply buffer must also fit the linked static/stack budget.
pub fn encode_memory_report(output: &mut [u8]) -> Option<usize> {
    if output.len() < MEMORY_REPORT_BYTES {
        return None;
    }
    let (used, peak) = crate::heap::usage();
    let words = [
        u32::try_from(crate::config::HEAP_BYTES).ok()?,
        u32::try_from(used).ok()?,
        u32::try_from(peak).ok()?,
        cortex_m::register::msp::read(),
        match spare_bytes() {
            Some(bytes) => u32::try_from(bytes).ok()?,
            None => u32::MAX,
        },
        crate::phase_trace::ENCODED_BYTES as u32,
    ];
    encode_trace(&mut output[40..MEMORY_REPORT_BYTES])?;
    output[..4].copy_from_slice(b"CMEM");
    output[4..8].copy_from_slice(&1u32.to_le_bytes());
    output[8..16].copy_from_slice(&embassy_time::Instant::now().as_millis().to_le_bytes());
    for (slot, word) in output[16..40].chunks_exact_mut(4).zip(words) {
        slot.copy_from_slice(&word.to_le_bytes());
    }
    Some(MEMORY_REPORT_BYTES)
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
