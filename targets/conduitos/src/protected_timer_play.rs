//! The native product enters the private ordinary runtime through its Root owner.
use crate::{
    composition::{MachineRunError, MachineRunReceipt},
    machine::{IdleBase, InterruptBase, MonotonicClockBase, SerialBase, TimerBase},
    tour_timer_plan::PreparedTourTimerPlan,
};

pub(crate) fn run<
    C: MonotonicClockBase,
    T: TimerBase,
    S: SerialBase,
    I: InterruptBase,
    D: IdleBase,
>(
    prepared: &mut PreparedTourTimerPlan,
    clock: &mut C,
    timer: &mut T,
    serial: &mut S,
    interrupts: &mut I,
    idle: &mut D,
) -> Result<MachineRunReceipt, MachineRunError> {
    prepared.kernel.run(clock, timer, serial, interrupts, idle)
}
