use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::machine::{
    BaseError, FixedTimerSlots, IdleBase, InterruptBase, InterruptState, KernelInterest,
    MonotonicClockBase, SerialBase, TimerBase, TimerToken,
};

use super::{
    InterruptFact, disable_interrupts, enable_interrupts, interruptible_idle, interrupts_enabled,
    pop_interrupt, present, read_counter, timer_arm,
};

static TIMER_ARM_PENDING: AtomicBool = AtomicBool::new(false);
static TIMER_PENDING_TICKS: AtomicU64 = AtomicU64::new(0);

fn arm_pending_source() -> bool {
    if !TIMER_ARM_PENDING.swap(false, Ordering::AcqRel) {
        return true;
    }
    let ticks = TIMER_PENDING_TICKS.swap(0, Ordering::AcqRel);
    if ticks == 0 {
        timer_arm()
    } else {
        super::domain_budget::source_arm_ticks(ticks)
    }
}

#[cfg(feature = "ordinary-domain-proof")]
pub(super) fn start_pending_source_timer() {
    if !arm_pending_source() {
        super::emergency_halt();
    }
}

pub struct Clock(u64);
impl Clock {
    pub const fn new() -> Self {
        Self(0)
    }
}
impl MonotonicClockBase for Clock {
    fn provider_generation(&self) -> Option<u64> {
        Some(1)
    }
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
    fn arm_after_milliseconds(
        &mut self,
        interest: KernelInterest,
        milliseconds: u64,
    ) -> Result<TimerToken, BaseError> {
        let (numerator, denominator) = counter_frequency_ratio()?;
        let ticks = crate::timer_duration::duration_ticks(milliseconds, numerator, denominator)?;
        let token = self.slots.arm(interest)?;
        self.active = Some(token);
        TIMER_PENDING_TICKS.store(ticks, Ordering::Release);
        TIMER_ARM_PENDING.store(true, Ordering::Release);
        Ok(token)
    }

    fn provider_generation(&self) -> Option<u64> {
        Some(1)
    }
    fn arm(&mut self, interest: KernelInterest) -> Result<TimerToken, BaseError> {
        let token = self.slots.arm(interest)?;
        if self.active.replace(token).is_some() {
            return Err(BaseError::SlotFull);
        }
        TIMER_PENDING_TICKS.store(0, Ordering::Release);
        TIMER_ARM_PENDING.store(true, Ordering::Release);
        Ok(token)
    }
    fn cancel(&mut self, token: TimerToken) -> Result<KernelInterest, BaseError> {
        let interest = self.slots.cancel(token)?;
        self.active = None;
        TIMER_ARM_PENDING.store(false, Ordering::Release);
        TIMER_PENDING_TICKS.store(0, Ordering::Release);
        super::domain_budget::source_cancel();
        Ok(interest)
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
    fn provider_generation(&self) -> Option<u64> {
        // One fixed Boot-owned UART provider; replacement requires a new Boot.
        Some(1)
    }
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
        if !arm_pending_source() {
            return Err(BaseError::Unavailable);
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

// CPUCFG's constant-counter ratio, as specified by the LoongArch CPU
// configuration interface. Preserve the ratio until upward tick rounding.
fn counter_frequency_ratio() -> Result<(u64, u64), BaseError> {
    fn configuration(index: usize) -> u64 {
        let value: u64;
        unsafe {
            core::arch::asm!("cpucfg {value}, {index}", value = out(reg) value, index = in(reg) index, options(nostack));
        }
        value
    }
    if configuration(2) & (1 << 14) == 0 {
        return Err(BaseError::Unavailable);
    }
    let base = configuration(4) & 0xffff_ffff;
    let ratio = configuration(5);
    let multiplier = ratio & 0xffff;
    let divisor = (ratio >> 16) & 0xffff;
    if base == 0 || multiplier == 0 || divisor == 0 {
        return Err(BaseError::Unavailable);
    }
    Ok((base * multiplier, divisor))
}
