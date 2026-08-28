//! QEMU IA-32 mechanisms below the generic machine/Base seam.

use crate::machine::{
    BaseError, FixedTimerSlots, IdleBase, InterruptBase, InterruptState, KernelInterest,
    MonotonicClockBase, SerialBase, TimerBase, TimerToken,
};
use core::sync::atomic::{AtomicBool, Ordering};

pub const PIT_IRQ: u8 = 32;
static FACT_PRESENT: AtomicBool = AtomicBool::new(false);
static FACT_OVERFLOW: AtomicBool = AtomicBool::new(false);
static TIMER_ARM_PENDING: AtomicBool = AtomicBool::new(false);
static mut IDT: [u64; 256] = [0; 256];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptFact {
    Timer,
    WrongSource(u8),
    Overflow,
}

core::arch::global_asm!(
    r#"
.section .text.conduitos_ia32_irq,"ax",@progbits
.global conduitos_ia32_irq_entry
conduitos_ia32_irq_entry:
    pushad
    call conduitos_ia32_irq_handler
    popad
    iretd
"#
);

pub fn initialize_machine() {
    disable_interrupts();
    let handler = conduitos_ia32_irq_entry as *const () as usize as u32;
    let selector: u16;
    unsafe {
        core::arch::asm!("mov {0:x}, cs", out(reg) selector, options(nomem, nostack, preserves_flags))
    };
    let gate = u64::from(handler & 0xffff)
        | (u64::from(selector) << 16)
        | (0x8e_u64 << 40)
        | (u64::from(handler >> 16) << 48);
    unsafe {
        IDT[PIT_IRQ as usize] = gate;
        let descriptor = DescriptorTable {
            limit: (core::mem::size_of::<[u64; 256]>() - 1) as u16,
            base: core::ptr::addr_of!(IDT) as u32,
        };
        core::arch::asm!("lidt [{0}]", in(reg) &descriptor, options(readonly, nostack, preserves_flags));
        remap_pic();
    }
}

pub fn timer_arm() {
    const TICKS: u16 = 1193;
    unsafe {
        outb(0x43, 0x30);
        outb(0x40, TICKS as u8);
        outb(0x40, (TICKS >> 8) as u8);
    }
}

pub fn enable_interrupts() {
    unsafe { core::arch::asm!("sti", options(nomem, nostack, preserves_flags)) }
}

pub fn disable_interrupts() {
    unsafe { core::arch::asm!("cli", options(nomem, nostack, preserves_flags)) }
}

fn interrupts_enabled() -> bool {
    let flags: u32;
    unsafe {
        core::arch::asm!("pushfd", "pop {0:e}", out(reg) flags, options(nomem, preserves_flags))
    };
    flags & (1 << 9) != 0
}

pub fn interruptible_idle() {
    unsafe { core::arch::asm!("hlt", options(nomem, nostack)) }
}

pub fn pop_interrupt() -> Option<InterruptFact> {
    if FACT_OVERFLOW.swap(false, Ordering::AcqRel) {
        return Some(InterruptFact::Overflow);
    }
    FACT_PRESENT
        .swap(false, Ordering::AcqRel)
        .then_some(InterruptFact::Timer)
}

pub fn present(bytes: &[u8]) {
    for byte in bytes {
        unsafe { outb(0xe9, *byte) };
    }
}

pub fn read_counter() -> u64 {
    let low: u32;
    let high: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") low, out("edx") high, options(nomem, nostack)) };
    u64::from(low) | (u64::from(high) << 32)
}

unsafe extern "C" {
    fn conduitos_ia32_irq_entry();
}

#[unsafe(no_mangle)]
extern "C" fn conduitos_ia32_irq_handler() {
    if FACT_PRESENT.swap(true, Ordering::AcqRel) {
        FACT_OVERFLOW.store(true, Ordering::Release);
    }
    unsafe { outb(0x20, 0x20) };
}

#[repr(C, packed)]
struct DescriptorTable {
    limit: u16,
    base: u32,
}

unsafe fn remap_pic() {
    unsafe {
        outb(0x20, 0x11);
        outb(0xa0, 0x11);
        outb(0x21, 0x20);
        outb(0xa1, 0x28);
        outb(0x21, 0x04);
        outb(0xa1, 0x02);
        outb(0x21, 0x01);
        outb(0xa1, 0x01);
        outb(0x21, 0xfe);
        outb(0xa1, 0xff);
    }
}

unsafe fn outb(port: u16, value: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags))
    };
}

pub struct Clock(u64);
impl Clock {
    pub const fn new() -> Self {
        Self(0)
    }
}
impl MonotonicClockBase for Clock {
    fn now(&mut self) -> u64 {
        self.0 = read_counter().max(self.0);
        self.0
    }
}

pub struct Timer {
    slots: FixedTimerSlots<1>,
    active: Option<TimerToken>,
    wakes: u32,
}
impl Timer {
    pub const fn new() -> Self {
        Self {
            slots: FixedTimerSlots::new(),
            active: None,
            wakes: 0,
        }
    }
}
impl TimerBase for Timer {
    fn arm(&mut self, interest: KernelInterest) -> Result<TimerToken, BaseError> {
        let token = self.slots.arm(interest)?;
        if self.active.replace(token).is_some() {
            return Err(BaseError::SlotFull);
        }
        TIMER_ARM_PENDING.store(true, Ordering::Release);
        Ok(token)
    }
    fn cancel(&mut self, token: TimerToken) -> Result<KernelInterest, BaseError> {
        self.active = None;
        TIMER_ARM_PENDING.store(false, Ordering::Release);
        self.slots.cancel(token)
    }
    fn take_wake(&mut self) -> Result<Option<KernelInterest>, BaseError> {
        match pop_interrupt() {
            None => Ok(None),
            Some(InterruptFact::Timer) => {
                let token = self.active.take().ok_or(BaseError::StaleWake)?;
                let interest = self.slots.wake(token)?;
                self.wakes = self.wakes.checked_add(1).ok_or(BaseError::Unavailable)?;
                Ok(Some(interest))
            }
            Some(InterruptFact::WrongSource(_) | InterruptFact::Overflow) => {
                Err(BaseError::Unavailable)
            }
        }
    }
    fn wake_count(&self) -> u32 {
        self.wakes
    }
}

pub struct Serial(u32);
impl Serial {
    pub const fn new() -> Self {
        Self(0)
    }
}
impl SerialBase for Serial {
    fn present(&mut self, bytes: &[u8]) -> Result<(), BaseError> {
        present(bytes);
        self.0 = self.0.checked_add(1).ok_or(BaseError::Unavailable)?;
        Ok(())
    }
    fn presentation_count(&self) -> u32 {
        self.0
    }
}

pub struct Interrupts;
impl Interrupts {
    pub const fn new() -> Self {
        Self
    }
}
impl InterruptBase for Interrupts {
    fn enable(&mut self) {
        enable_interrupts();
    }
    fn disable(&mut self) -> InterruptState {
        let state = InterruptState {
            enabled: interrupts_enabled(),
        };
        disable_interrupts();
        state
    }
    fn restore(&mut self, state: InterruptState) {
        if state.enabled {
            enable_interrupts();
        } else {
            disable_interrupts();
        }
    }
    fn is_enabled(&self) -> bool {
        interrupts_enabled()
    }
}

pub struct Idle(u32);
impl Idle {
    pub const fn new() -> Self {
        Self(0)
    }
}
impl IdleBase for Idle {
    fn wait_for_interrupt(&mut self) -> Result<(), BaseError> {
        self.0 = self.0.checked_add(1).ok_or(BaseError::Unavailable)?;
        if TIMER_ARM_PENDING.swap(false, Ordering::AcqRel) {
            timer_arm();
        }
        enable_interrupts();
        interruptible_idle();
        disable_interrupts();
        Ok(())
    }
    fn idle_count(&self) -> u32 {
        self.0
    }
}
