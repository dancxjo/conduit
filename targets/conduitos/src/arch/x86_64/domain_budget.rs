//! Root-only RTC budget, independent of the Source LAPIC/PIT timer.
use super::{
    interrupt_controller,
    io::{inb, outb},
};
use crate::protected_region::DomainRefusal;
use core::{
    arch::asm,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

pub(super) const VECTOR: u8 = 0x28;
// An ordinary first Memory edit exhausted the former three-tick quantum
// in a normal host-time QEMU product run. Admit one finite frame-sized turn.
// Sixteen RTC ticks give roughly one 60 Hz frame of execution at 1024 Hz,
// while retaining a finite, independent return even for a non-yielding entry.
const ENTRY_BUDGET_TICKS: u32 = 16;
static NEXT_IDLE_EPOCH: AtomicU64 = AtomicU64::new(0);
static ACTIVE_IDLE_EPOCH: AtomicU64 = AtomicU64::new(0);
static OWNED: AtomicBool = AtomicBool::new(false);
static QUALIFICATION_POLLS: AtomicU32 = AtomicU32::new(0);
static READY: AtomicBool = AtomicBool::new(false);
static SEEN: AtomicU32 = AtomicU32::new(0);
static REMAINING: AtomicU32 = AtomicU32::new(0);
static SOURCE_INTERRUPTS: AtomicU32 = AtomicU32::new(0);
static USER_INTERRUPTS: AtomicU32 = AtomicU32::new(0);

/// Closed mechanism profiles. Draft's larger bound is an unmeasured local
/// preparation ceiling, not evidence that any canonical command fits it.
#[derive(Clone, Copy)]
pub(super) enum EntryBudget {
    Ordinary,
    Draft,
}
impl EntryBudget {
    const fn ticks(self) -> u32 {
        match self {
            Self::Ordinary => ENTRY_BUDGET_TICKS,
            Self::Draft => 64,
        }
    }
}

pub(super) struct Budget {
    rtc_a: u8,
    rtc_b: u8,
    master: u8,
    slave: u8,
    lint: Option<u32>,
}

impl Budget {
    pub fn arm(profile: EntryBudget) -> Result<Self, DomainRefusal> {
        let flags: u64;
        unsafe {
            asm!("pushfq", "pop {}", "cli", out(reg) flags);
        }
        let result = Self::arm_masked(profile);
        if flags & 0x200 != 0 {
            unsafe {
                asm!("sti", options(nostack));
            }
        }
        result
    }
    fn arm_masked(profile: EntryBudget) -> Result<Self, DomainRefusal> {
        if OWNED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(DomainRefusal::Unsupported);
        }
        let rtc_a = read(0x0a);
        let rtc_b = read(0x0b);
        // This profile requires an unowned normal-running RTC interrupt channel.
        if rtc_b & 0x70 != 0 || rtc_a & 0x70 != 0x20 {
            OWNED.store(false, Ordering::Release);
            return Err(DomainRefusal::Unsupported);
        }
        let guard = Self {
            rtc_a,
            rtc_b,
            master: unsafe { inb(0x21) },
            slave: unsafe { inb(0xa1) },
            lint: interrupt_controller::route_budget(),
        };
        REMAINING.store(0, Ordering::Release);
        write(0x0a, (rtc_a & 0x70) | 6); // 32768 / 32 = 1024 Hz.
        let _ = read(0x0c);
        unsafe {
            outb(0xa1, guard.slave & !1);
            outb(0x21, guard.master & !4);
        }
        let before = SEEN.load(Ordering::Acquire);
        write(0x0b, rtc_b | 0x40);
        if !READY.load(Ordering::Acquire) {
            // Earn interrupt delivery in Root before entering any hostile code.
            // Finite polling cannot strand Root if the route is unavailable.
            unsafe {
                asm!("sti", options(nostack));
            }
            let mut polls = 0;
            for _ in 0..1_000_000 {
                polls += 1;
                if SEEN.load(Ordering::Acquire).wrapping_sub(before) >= 2 {
                    break;
                }
                core::hint::spin_loop();
            }
            unsafe {
                asm!("cli", options(nostack));
            }
            QUALIFICATION_POLLS.store(polls, Ordering::Relaxed);
            if SEEN.load(Ordering::Acquire).wrapping_sub(before) < 2 {
                drop(guard);
                return Err(DomainRefusal::Unsupported);
            }
            READY.store(true, Ordering::Release);
        }
        USER_INTERRUPTS.store(0, Ordering::Release);
        SOURCE_INTERRUPTS.store(0, Ordering::Release);
        REMAINING.store(profile.ticks(), Ordering::Release);
        Ok(guard)
    }
}
impl Drop for Budget {
    fn drop(&mut self) {
        let flags: u64;
        unsafe {
            asm!("pushfq", "pop {}", "cli", out(reg) flags);
        }
        REMAINING.store(0, Ordering::Release);
        write(0x0b, self.rtc_b);
        let _ = read(0x0c);
        write(0x0a, self.rtc_a);
        unsafe {
            outb(0xa1, self.slave);
            outb(0x21, self.master);
        }
        interrupt_controller::restore_budget_route(self.lint);
        OWNED.store(false, Ordering::Release);
        if flags & 0x200 != 0 {
            unsafe {
                asm!("sti", options(nostack));
            }
        }
    }
}

pub(super) fn interrupt(user: bool) -> bool {
    let status = read(0x0c);
    unsafe {
        outb(0xa0, 0x20);
        outb(0x20, 0x20);
    }
    if status & 0x40 == 0 {
        return false;
    }
    SEEN.fetch_add(1, Ordering::Release);
    if !user {
        return false;
    }
    USER_INTERRUPTS.fetch_add(1, Ordering::Relaxed);
    let previous = REMAINING.fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
        value.checked_sub(1)
    });
    previous == Ok(1)
}
pub(super) fn source_interrupt() {
    SOURCE_INTERRUPTS.fetch_add(1, Ordering::Relaxed);
}
pub(super) fn user_interrupts() -> (u32, u32) {
    (
        USER_INTERRUPTS.load(Ordering::Relaxed),
        SOURCE_INTERRUPTS.load(Ordering::Relaxed),
    )
}
// Root owns the CMOS index port and keeps NMI enabled. The index port is
// write-only on this platform; reading 0x70 cannot recover an earlier mask.
pub(super) fn initialize() {
    READY.store(false, Ordering::Release);
    unsafe {
        outb(0x70, 0x0a);
    }
}
fn read(register: u8) -> u8 {
    unsafe {
        outb(0x70, register);
        inb(0x71)
    }
}
fn write(register: u8, value: u8) {
    unsafe {
        outb(0x70, register);
        outb(0x71, value);
    }
}

/// A synchronous Root-only lease of the existing independent RTC wake route.
/// No caller may enter eligible execution or invoke a callback while held.
/// Periodic RTC facts are readiness, never HID/Source completion authority.
pub(super) struct RootIdleLease {
    _budget: Budget,
    deadline: super::deadline::CandidateDeadline,
    seen: u32,
    epoch: u64,
}
impl RootIdleLease {
    pub fn try_arm() -> Option<Self> {
        if super::cpu::interrupts_enabled() {
            return None;
        }
        let epoch = NEXT_IDLE_EPOCH.load(Ordering::Relaxed).checked_add(1)?;
        let deadline = super::deadline::CandidateDeadline::admit_duration(8)?;
        let budget = Budget::arm(EntryBudget::Ordinary).ok()?;
        NEXT_IDLE_EPOCH.store(epoch, Ordering::Relaxed);
        ACTIVE_IDLE_EPOCH.store(epoch, Ordering::Release);
        let lease = Self {
            _budget: budget,
            epoch,
            deadline,
            seen: SEEN.load(Ordering::Acquire),
        };
        lease.may_idle().then_some(lease)
    }
    pub fn may_idle(&self) -> bool {
        super::hid_idle_fence::may_idle(
            self.epoch,
            ACTIVE_IDLE_EPOCH.load(Ordering::Acquire),
            self.seen,
            SEEN.load(Ordering::Acquire),
            self.deadline.elapsed_millis(),
        )
    }
}

#[unsafe(no_mangle)]
pub(super) extern "C" fn conduitos_root_rtc_handler() {
    interrupt(false);
}

impl Drop for RootIdleLease {
    fn drop(&mut self) {
        // Invalidate readiness before the Budget field restores the physical
        // RTC route. Late RTC facts remain readiness and never Source wakes.
        ACTIVE_IDLE_EPOCH.store(0, Ordering::Release);
    }
}

pub(super) fn qualification_polls() -> u32 {
    QUALIFICATION_POLLS.load(Ordering::Relaxed)
}
