//! Finite IA-32 timer identity and IRQ mailbox, independent of port I/O.
//!
//! The caller masks CPU interrupts while changing the mailbox or timer state.
//! IRQ facts retain the generation present at delivery, never a later arm.

use crate::machine::{BaseError, FixedTimerSlots, KernelInterest, TimerToken};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub struct IrqMailbox {
    armed: AtomicU32,
    generation: AtomicU32,
    present: AtomicBool,
    overflow: AtomicBool,
}

impl IrqMailbox {
    pub const fn new() -> Self {
        Self {
            armed: AtomicU32::new(0),
            generation: AtomicU32::new(0),
            present: AtomicBool::new(false),
            overflow: AtomicBool::new(false),
        }
    }

    pub fn start(&self, generation: u32) -> Result<(), BaseError> {
        if generation == 0 {
            return Err(BaseError::StaleWake);
        }
        if self.overflow.load(Ordering::Acquire) {
            return Err(BaseError::RingFull);
        }
        if self.present.load(Ordering::Acquire) {
            return Err(BaseError::StaleWake);
        }
        if self.armed.load(Ordering::Acquire) != 0 {
            return Err(BaseError::SlotFull);
        }
        self.armed.store(generation, Ordering::Release);
        Ok(())
    }

    pub fn retire(&self) {
        self.armed.store(0, Ordering::Release);
    }

    pub fn has_fact(&self) -> bool {
        self.present.load(Ordering::Acquire) || self.overflow.load(Ordering::Acquire)
    }

    pub fn publish(&self) {
        if self.present.load(Ordering::Acquire) {
            self.overflow.store(true, Ordering::Release);
        } else {
            self.generation
                .store(self.armed.load(Ordering::Acquire), Ordering::Release);
            self.present.store(true, Ordering::Release);
        }
    }

    pub fn pop(&self) -> Result<Option<u32>, BaseError> {
        if self.overflow.swap(false, Ordering::AcqRel) {
            return Err(BaseError::RingFull);
        }
        Ok(self
            .present
            .swap(false, Ordering::AcqRel)
            .then(|| self.generation.load(Ordering::Acquire)))
    }
}

pub struct TimerState {
    slots: FixedTimerSlots<1>,
}

impl TimerState {
    pub const fn new() -> Self {
        Self {
            slots: FixedTimerSlots::new(),
        }
    }

    pub fn arm(&mut self, interest: KernelInterest) -> Result<TimerToken, BaseError> {
        self.slots.arm(interest)
    }

    pub fn cancel(&mut self, token: TimerToken) -> Result<KernelInterest, BaseError> {
        self.slots.cancel(token)
    }

    pub fn wake(&mut self, generation: u32) -> Result<KernelInterest, BaseError> {
        let token = TimerToken {
            slot: 0,
            generation,
        };
        self.slots.wake(token)
    }
}

#[cfg(test)]
#[path = "timer_lifecycle_tests.rs"]
mod tests;
