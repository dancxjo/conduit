//! Exclusive one-CPU source IRQ lease, not a scheduler or an operation grant.
//! No default offer activates this. Drop permanently quarantines the private
//! route until reboot: untagged late IRQs must never become legacy Host wakes.
use super::{cpu, interrupt_controller, irq};
use conduit_core::OperationSlice;
use core::sync::atomic::{AtomicU32, Ordering};
const FREE: u32 = 0;
const PROBE: u32 = 1;
const READY: u32 = 2;
const SLICE: u32 = 3;
const EXPIRED: u32 = 4;
const POISONED: u32 = 5;
static STATE: AtomicU32 = AtomicU32::new(FREE);
static EPOCH: AtomicU32 = AtomicU32::new(0);
static PULSES: AtomicU32 = AtomicU32::new(0);
static QUANTUM: AtomicU32 = AtomicU32::new(0);
static COUNT: AtomicU32 = AtomicU32::new(0);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Refusal {
    Busy,
    Unavailable,
    Unqualified,
    Stale,
    InvalidSlice,
}
pub(super) fn host_arm_allowed() -> Result<(), Refusal> {
    if STATE.load(Ordering::Acquire) == FREE {
        Ok(())
    } else {
        Err(Refusal::Busy)
    }
}
pub(super) struct ReadyClock {
    epoch: u32,
}
impl ReadyClock {
    /// Earn two actual Root source IRQs under one exclusive lifetime. Interrupts
    /// are temporarily enabled; original IF is restored by InterruptMask on exit.
    /// Timeout is bounded work, not calibrated elapsed time or delivery promise.
    pub(super) fn qualify() -> Result<Self, Refusal> {
        let _mask = cpu::InterruptMask::new();
        host_arm_allowed()?;
        if !can_acquire(
            irq::has_pending_facts(),
            interrupt_controller::private_clock_available(),
        ) {
            return Err(Refusal::Unavailable);
        }
        let epoch = EPOCH
            .load(Ordering::Relaxed)
            .checked_add(1)
            .ok_or(Refusal::Unavailable)?;
        EPOCH.store(epoch, Ordering::Relaxed);
        PULSES.store(0, Ordering::Relaxed);
        STATE.store(PROBE, Ordering::Release);
        interrupt_controller::raw_arm_timer();
        cpu::enable_interrupts();
        for _ in 0..2_000_000 {
            if STATE.load(Ordering::Acquire) == READY {
                break;
            }
            core::hint::spin_loop();
        }
        cpu::disable_interrupts();
        if STATE.load(Ordering::Acquire) != READY {
            interrupt_controller::raw_cancel_timer();
            STATE.store(POISONED, Ordering::Release);
            return Err(Refusal::Unqualified);
        }
        Ok(Self { epoch })
    }
    pub(super) fn begin_slice<E: Copy + Eq>(
        &mut self,
        grant: OperationSlice<E>,
    ) -> Result<SliceClock<E>, Refusal> {
        let _mask = cpu::InterruptMask::new();
        if self.epoch != EPOCH.load(Ordering::Acquire) {
            return Err(Refusal::Stale);
        }
        if STATE.load(Ordering::Acquire) != READY {
            return Err(Refusal::Busy);
        }
        if !interrupt_controller::private_clock_available() {
            return Err(Refusal::Unavailable);
        }
        if grant.interrupts() == 0 {
            return Err(Refusal::InvalidSlice);
        }
        QUANTUM.store(grant.interrupts(), Ordering::Relaxed);
        COUNT.store(0, Ordering::Relaxed);
        STATE.store(SLICE, Ordering::Release);
        interrupt_controller::raw_arm_timer();
        Ok(SliceClock {
            epoch: self.epoch,
            grant,
        })
    }
    /// Charge this count to the SAME ongoing ProtectedOperation. It is per-slice
    /// only and never resets logical remaining fuel or authorizes a new request.
    pub(super) fn finish_slice<E: Copy + Eq>(
        &mut self,
        slice: SliceClock<E>,
    ) -> Result<(OperationSlice<E>, u32, bool), Refusal> {
        let _mask = cpu::InterruptMask::new();
        if slice.epoch != self.epoch || self.epoch != EPOCH.load(Ordering::Acquire) {
            return Err(Refusal::Stale);
        }
        let state = STATE.load(Ordering::Acquire);
        if state != SLICE && state != EXPIRED {
            return Err(Refusal::Stale);
        }
        interrupt_controller::raw_cancel_timer();
        let count = COUNT.load(Ordering::Acquire);
        STATE.store(READY, Ordering::Release);
        Ok((slice.grant, count, state == EXPIRED))
    }
}
impl Drop for ReadyClock {
    fn drop(&mut self) {
        let _mask = cpu::InterruptMask::new();
        if self.epoch == EPOCH.load(Ordering::Acquire) {
            interrupt_controller::raw_cancel_timer();
            STATE.store(POISONED, Ordering::Release);
        }
    }
}
/// Not Copy: one clock observation can be finished only once by its owner.
pub(super) struct SliceClock<E: Copy + Eq> {
    epoch: u32,
    grant: OperationSlice<E>,
}
/// Some means private IRQ consumed; bool requests genuine context capture only
/// when actual CPL3 accounting reaches the selected grant. Parent owns capture.
pub(super) fn source_irq(user: bool) -> Option<bool> {
    match STATE.load(Ordering::Acquire) {
        FREE => None,
        PROBE => {
            if user {
                STATE.store(POISONED, Ordering::Release);
                interrupt_controller::raw_cancel_timer();
                return Some(false);
            }
            let pulses = PULSES.fetch_add(1, Ordering::Relaxed) + 1;
            if pulses == 2 {
                interrupt_controller::raw_cancel_timer();
                STATE.store(READY, Ordering::Release);
            } else {
                interrupt_controller::raw_arm_timer();
            }
            Some(false)
        }
        SLICE => {
            if !user {
                interrupt_controller::raw_cancel_timer();
                STATE.store(POISONED, Ordering::Release);
                return Some(false);
            }
            let count = COUNT.load(Ordering::Relaxed);
            let Some((next, expired)) = count_interrupt(count, QUANTUM.load(Ordering::Relaxed))
            else {
                STATE.store(POISONED, Ordering::Release);
                interrupt_controller::raw_cancel_timer();
                return Some(true);
            };
            COUNT.store(next, Ordering::Release);
            if expired {
                interrupt_controller::raw_cancel_timer();
                STATE.store(EXPIRED, Ordering::Release);
                Some(true)
            } else {
                interrupt_controller::raw_arm_timer();
                Some(false)
            }
        }
        _ => Some(false), // late/private/unarmed IRQ never publishes a Host fact.
    }
}

fn can_acquire(pending_facts: bool, hardware_available: bool) -> bool {
    !pending_facts && hardware_available
}
fn count_interrupt(count: u32, quantum: u32) -> Option<(u32, bool)> {
    if quantum == 0 || count >= quantum {
        return None;
    }
    let next = count.checked_add(1)?;
    Some((next, next == quantum))
}
#[cfg(test)]
#[path = "preemption_clock/tests.rs"]
mod tests;
