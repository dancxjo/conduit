//! Root-only RTC budget, independent of the Source LAPIC/PIT timer.
use super::{inb, outb};
use crate::protected_region::DomainRefusal;
use core::{
    arch::asm,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

pub(super) const VECTOR: u8 = 0x28;
static READY: AtomicBool = AtomicBool::new(false);
static SEEN: AtomicU32 = AtomicU32::new(0);
static REMAINING: AtomicU32 = AtomicU32::new(0);
static SOURCE_INTERRUPTS: AtomicU32 = AtomicU32::new(0);
static USER_INTERRUPTS: AtomicU32 = AtomicU32::new(0);

pub(super) struct Budget {
    rtc_a: u8,
    rtc_b: u8,
    master: u8,
    slave: u8,
}

impl Budget {
    pub fn arm() -> Result<Self, DomainRefusal> {
        let flags: u32;
        unsafe {
            asm!("pushfd", "pop {0:e}", "cli", out(reg) flags);
        }
        let result = Self::arm_masked();
        if flags & 0x200 != 0 {
            unsafe {
                asm!("sti", options(nostack));
            }
        }
        result
    }
    fn arm_masked() -> Result<Self, DomainRefusal> {
        let rtc_a = read(0x0a);
        let rtc_b = read(0x0b);
        // This profile requires an unowned normal-running RTC interrupt channel.
        if rtc_b & 0x70 != 0 || rtc_a & 0x70 != 0x20 {
            return Err(DomainRefusal::Unsupported);
        }
        let guard = Self {
            rtc_a,
            rtc_b,
            master: unsafe { inb(0x21) },
            slave: unsafe { inb(0xa1) },
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
            for _ in 0..1_000_000 {
                if SEEN.load(Ordering::Acquire).wrapping_sub(before) >= 2 {
                    break;
                }
                core::hint::spin_loop();
            }
            unsafe {
                asm!("cli", options(nostack));
            }
            if SEEN.load(Ordering::Acquire).wrapping_sub(before) < 2 {
                drop(guard);
                return Err(DomainRefusal::Unsupported);
            }
            READY.store(true, Ordering::Release);
        }
        USER_INTERRUPTS.store(0, Ordering::Release);
        SOURCE_INTERRUPTS.store(0, Ordering::Release);
        REMAINING.store(3, Ordering::Release);
        Ok(guard)
    }
}
impl Drop for Budget {
    fn drop(&mut self) {
        let flags: u32;
        unsafe {
            asm!("pushfd", "pop {0:e}", "cli", out(reg) flags);
        }
        REMAINING.store(0, Ordering::Release);
        write(0x0b, self.rtc_b);
        let _ = read(0x0c);
        write(0x0a, self.rtc_a);
        unsafe {
            outb(0xa1, self.slave);
            // Source IRQ0 can retire while this budget owns IRQ8/cascade.
            // Restore our route without undoing Source's current IRQ0 mask.
            let source_mask = inb(0x21) & 1;
            outb(0x21, (self.master & !1) | source_mask);
        }
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
