//! Orange Pi 5 RK3588S mechanisms below the generic Host seam.
//!
//! U-Boot owns DRAM, clocks, and pinmux initialization. ConduitOS consumes the
//! exact UART2 and architectural counter facts sealed by the board image.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::machine::{
    BaseError, FixedTimerSlots, IdleBase, InterruptBase, InterruptState, KernelInterest,
    MonotonicClockBase, SerialBase, TimerBase, TimerToken,
};

const UART2_BASE: usize = 0xfeb5_0000;
const UART_REGISTER_SHIFT: usize = 2;
const UART_LINE_STATUS: usize = 5 << UART_REGISTER_SHIFT;
const UART_TX_EMPTY: u32 = 1 << 5;

static INTERRUPTS_ENABLED: AtomicBool = AtomicBool::new(false);
static TIMER_DEADLINE: AtomicU64 = AtomicU64::new(0);

pub fn enable_fp_simd() {
    let mut cpacr: u64;
    unsafe {
        core::arch::asm!("mrs {cpacr}, cpacr_el1", cpacr = out(reg) cpacr, options(nostack));
        cpacr |= 0b11 << 20;
        core::arch::asm!("msr cpacr_el1, {cpacr}", "isb", cpacr = in(reg) cpacr, options(nostack));
    }
}

pub fn initialize_machine() {
    disable_interrupts();
    present(b"CONDUIT_ORANGE_PI_5_MACHINE_DETAIL rk3588s-uart2\n");
}

pub fn present(bytes: &[u8]) {
    for byte in bytes {
        while unsafe { read32(UART2_BASE + UART_LINE_STATUS) } & UART_TX_EMPTY == 0 {}
        unsafe { write32(UART2_BASE, u32::from(*byte)) };
    }
}

pub fn read_counter() -> u64 {
    let value;
    unsafe {
        core::arch::asm!("mrs {value}, cntpct_el0", value = out(reg) value, options(nostack))
    };
    value
}

fn counter_frequency() -> u64 {
    let value;
    unsafe {
        core::arch::asm!("mrs {value}, cntfrq_el0", value = out(reg) value, options(nostack))
    };
    value
}

pub fn enable_interrupts() {
    INTERRUPTS_ENABLED.store(true, Ordering::Release);
}

pub fn disable_interrupts() {
    INTERRUPTS_ENABLED.store(false, Ordering::Release);
    unsafe { core::arch::asm!("msr daifset, #2", "isb", options(nostack, preserves_flags)) };
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
        let ticks = (counter_frequency() / 1_000).max(1);
        TIMER_DEADLINE.store(read_counter().saturating_add(ticks), Ordering::Release);
        Ok(token)
    }

    fn cancel(&mut self, token: TimerToken) -> Result<KernelInterest, BaseError> {
        self.active = None;
        TIMER_DEADLINE.store(0, Ordering::Release);
        self.slots.cancel(token)
    }

    fn take_wake(&mut self) -> Result<Option<KernelInterest>, BaseError> {
        let deadline = TIMER_DEADLINE.load(Ordering::Acquire);
        if deadline == 0 || read_counter() < deadline {
            return Ok(None);
        }
        TIMER_DEADLINE.store(0, Ordering::Release);
        let token = self.active.take().ok_or(BaseError::StaleWake)?;
        let interest = self.slots.wake(token)?;
        self.wakes = self.wakes.checked_add(1).ok_or(BaseError::Unavailable)?;
        Ok(Some(interest))
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
            enabled: INTERRUPTS_ENABLED.swap(false, Ordering::AcqRel),
        };
        disable_interrupts();
        state
    }

    fn restore(&mut self, state: InterruptState) {
        INTERRUPTS_ENABLED.store(state.enabled, Ordering::Release);
    }

    fn is_enabled(&self) -> bool {
        INTERRUPTS_ENABLED.load(Ordering::Acquire)
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
        let deadline = TIMER_DEADLINE.load(Ordering::Acquire);
        while deadline != 0 && read_counter() < deadline {
            core::hint::spin_loop();
        }
        Ok(())
    }

    fn idle_count(&self) -> u32 {
        self.0
    }
}

unsafe fn read32(address: usize) -> u32 {
    unsafe { core::ptr::read_volatile(address as *const u32) }
}

unsafe fn write32(address: usize, value: u32) {
    unsafe { core::ptr::write_volatile(address as *mut u32, value) }
}
