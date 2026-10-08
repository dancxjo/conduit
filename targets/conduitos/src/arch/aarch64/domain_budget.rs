//! Root physical timer budget; Source retains its separate virtual timer.
use crate::protected_region::DomainRefusal;
use core::{
    arch::asm,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

pub(super) const IRQ: u32 = 30;
static READY: AtomicBool = AtomicBool::new(false);
static SEEN: AtomicU32 = AtomicU32::new(0);
static REMAINING: AtomicU32 = AtomicU32::new(0);
static PERIOD: AtomicU32 = AtomicU32::new(0);
static USER_INTERRUPTS: AtomicU32 = AtomicU32::new(0);
static SOURCE_INTERRUPTS: AtomicU32 = AtomicU32::new(0);

pub(super) struct Budget {
    control: u64,
    deadline: u64,
    enabled: bool,
    priority: u8,
}
impl Budget {
    /// Caller holds IRQs masked and has installed the domain vector table.
    pub fn arm() -> Result<Self, DomainRefusal> {
        let (control, deadline): (u64, u64);
        unsafe {
            asm!("mrs {0}, cntp_ctl_el0", "mrs {1}, cntp_cval_el0",
                out(reg) control, out(reg) deadline, options(nostack));
        }
        if control & 1 != 0 {
            return Err(DomainRefusal::Unsupported);
        }
        let ticks = u32::try_from((super::counter_frequency() / 1000).max(1))
            .ok()
            .filter(|ticks| *ticks <= i32::MAX as u32)
            .ok_or(DomainRefusal::Unsupported)?;
        PERIOD.store(ticks, Ordering::Release);
        let priority_address = super::GICD_BASE + 0x400 + IRQ as usize;
        let guard = Self {
            control,
            deadline,
            enabled: unsafe { super::read32(super::GICD_BASE + 0x100) & (1 << IRQ) != 0 },
            priority: unsafe { core::ptr::read_volatile(priority_address as *const u8) },
        };
        REMAINING.store(0, Ordering::Release);
        let before = SEEN.load(Ordering::Acquire);
        unsafe {
            super::write8(priority_address, 0x80);
            super::write32(super::GICD_BASE + 0x280, 1 << IRQ);
            super::write32(super::GICD_BASE + 0x100, 1 << IRQ);
            asm!("dsb sy", "isb", options(nostack));
        }
        rearm();
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
        SOURCE_INTERRUPTS.store(0, Ordering::Release);
        REMAINING.store(3, Ordering::Release);
        rearm();
        Ok(guard)
    }
}
impl Drop for Budget {
    fn drop(&mut self) {
        disable();
        REMAINING.store(0, Ordering::Release);
        unsafe {
            super::write32(super::GICD_BASE + 0x280, 1 << IRQ);
            if !self.enabled {
                super::write32(super::GICD_BASE + 0x180, 1 << IRQ);
            }
            super::write8(super::GICD_BASE + 0x400 + IRQ as usize, self.priority);
            asm!("msr cntp_cval_el0, {0}", "msr cntp_ctl_el0, {1}", "isb",
                in(reg) self.deadline, in(reg) self.control, options(nostack));
        }
    }
}

pub(super) fn interrupt(user: bool) -> bool {
    disable();
    SEEN.fetch_add(1, Ordering::Release);
    if user {
        USER_INTERRUPTS.fetch_add(1, Ordering::Relaxed);
        if REMAINING.fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            value.checked_sub(1)
        }) == Ok(1)
        {
            return true;
        }
    }
    rearm();
    false
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
fn rearm() {
    let ticks = u64::from(PERIOD.load(Ordering::Acquire));
    unsafe {
        asm!("msr cntp_tval_el0, {0}", "msr cntp_ctl_el0, {1}", "isb",
            in(reg) ticks, in(reg) 1_u64, options(nostack));
    }
}
fn disable() {
    unsafe {
        asm!("msr cntp_ctl_el0, xzr", "isb", options(nostack));
    }
}
