use crate::machine::{
    BaseError, FixedTimerSlots, IdleBase, InterruptBase, InterruptState, KernelInterest,
    MonotonicClockBase, SerialBase, TimerBase, TimerToken,
};
use core::sync::atomic::{AtomicBool, Ordering};

use super::{
    InterruptFact, disable_interrupts, enable_interrupts, interruptible_idle, interrupts_enabled,
    pop_interrupt, present, read_counter, timer_arm,
};

static TIMER_ARM_PENDING: AtomicBool = AtomicBool::new(false);

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
