//! Root multiplexes Source wake and domain budget onto the one LoongArch countdown timer.
use crate::protected_region::DomainRefusal;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

const NONE: u64 = u64::MAX;
const PERIOD: u64 = 400_000;
static SOURCE: AtomicU64 = AtomicU64::new(NONE);
static BUDGET: AtomicU64 = AtomicU64::new(NONE);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static READY: AtomicBool = AtomicBool::new(false);
static SEEN: AtomicU32 = AtomicU32::new(0);
static REMAINING: AtomicU32 = AtomicU32::new(0);
static USER_INTERRUPTS: AtomicU32 = AtomicU32::new(0);
static USER_TRAPS: AtomicU32 = AtomicU32::new(0);
static SOURCE_INTERRUPTS: AtomicU32 = AtomicU32::new(0);

pub(super) struct Budget;
impl Budget {
    /// Caller holds Root interrupts masked; Root's vector is installed.
    pub fn arm() -> Result<Self, DomainRefusal> {
        if ACTIVE.swap(true, Ordering::AcqRel) {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        let guard = Self;
        REMAINING.store(0, Ordering::Release);
        let before = SEEN.load(Ordering::Acquire);
        BUDGET.store(
            super::read_counter().saturating_add(PERIOD),
            Ordering::Release,
        );
        if !program() {
            return Err(DomainRefusal::Unsupported);
        }
        if !READY.load(Ordering::Acquire) {
            super::enable_interrupts();
            for _ in 0..1_000_000 {
                if SEEN.load(Ordering::Acquire).wrapping_sub(before) >= 2 {
                    break;
                }
                core::hint::spin_loop();
            }
            super::disable_interrupts();
            if SEEN.load(Ordering::Acquire).wrapping_sub(before) < 2 {
                return Err(DomainRefusal::Unsupported);
            }
            READY.store(true, Ordering::Release);
        }
        USER_INTERRUPTS.store(0, Ordering::Release);
        USER_TRAPS.store(0, Ordering::Release);
        SOURCE_INTERRUPTS.store(0, Ordering::Release);
        REMAINING.store(3, Ordering::Release);
        BUDGET.store(
            super::read_counter().saturating_add(PERIOD),
            Ordering::Release,
        );
        if !program() {
            return Err(DomainRefusal::Unsupported);
        }
        Ok(guard)
    }
}
impl Drop for Budget {
    fn drop(&mut self) {
        BUDGET.store(NONE, Ordering::Release);
        REMAINING.store(0, Ordering::Release);
        ACTIVE.store(false, Ordering::Release);
        if !program() {
            super::emergency_halt();
        }
    }
}

pub(super) fn source_arm() -> bool {
    source_arm_ticks(PERIOD)
}
pub(super) fn source_arm_ticks(ticks: u64) -> bool {
    let Some(deadline) = super::read_counter()
        .checked_add(ticks)
        .filter(|deadline| *deadline != NONE)
    else {
        return false;
    };
    if ticks == 0 {
        return false;
    }
    SOURCE.store(deadline, Ordering::Release);
    program()
}
pub(super) fn source_cancel() {
    SOURCE.store(NONE, Ordering::Release);
    if !program() {
        super::emergency_halt();
    }
}

/// Returns whether the admitted user budget ended. Source completion remains
/// the existing interrupt fact; it never becomes a budget-owned wake.
pub(super) fn interrupt(user: bool) -> bool {
    if user {
        USER_TRAPS.fetch_add(1, Ordering::Relaxed);
    }
    let now = super::read_counter();
    let source = SOURCE.load(Ordering::Acquire);
    if source != NONE && source <= now {
        SOURCE.store(NONE, Ordering::Release);
        super::record_timer_interrupt();
        if user {
            SOURCE_INTERRUPTS.fetch_add(1, Ordering::Relaxed);
        }
    }
    let budget = BUDGET.load(Ordering::Acquire);
    let mut expired = false;
    if budget != NONE && budget <= now {
        SEEN.fetch_add(1, Ordering::Release);
        if user {
            USER_INTERRUPTS.fetch_add(1, Ordering::Relaxed);
            expired = REMAINING.fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_sub(1)
            }) == Ok(1);
        }
        BUDGET.store(
            if expired {
                NONE
            } else {
                now.saturating_add(PERIOD)
            },
            Ordering::Release,
        );
    }
    if !program() {
        super::emergency_halt();
    }
    expired
}
pub(super) fn user_interrupts() -> (u32, u32) {
    (
        USER_INTERRUPTS.load(Ordering::Relaxed),
        SOURCE_INTERRUPTS.load(Ordering::Relaxed),
    )
}
pub(super) fn interrupt_entries() -> u32 {
    USER_TRAPS.load(Ordering::Relaxed)
}
fn program() -> bool {
    let deadline = SOURCE
        .load(Ordering::Acquire)
        .min(BUDGET.load(Ordering::Acquire));
    super::write_csr::<0x41>(0);
    super::write_csr::<0x44>(1);
    if deadline == NONE {
        super::change_csr::<0x04>(0, super::ECFG_TIMER);
        return true;
    }
    // INITVAL occupies the aligned countdown itself: low bits are mode bits.
    // Avoid zero counts; a deadline already due produces a four-tick timer.
    let delta = deadline.saturating_sub(super::read_counter()).max(4);
    let count = delta.saturating_add(3) & !3;
    super::change_csr::<0x04>(super::ECFG_TIMER, super::ECFG_TIMER);
    super::write_csr::<0x41>(count as usize | 1);
    super::read_csr::<0x41>() & 1 != 0
}
