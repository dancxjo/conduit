//! Architecture-neutral ownership loop for the admitted ordinary text Plan.

use conduit_kernel::scheduler::SchedulerStatus;

use crate::{
    composition::{MachineProof, MachineRunError},
    machine::{IdleBase, InterruptBase, MonotonicClockBase, SerialBase},
    text_planned_kernel::TextPlannedKernel,
};

const MAXIMUM_KERNEL_STEPS: u32 = 128;

pub fn run<C, S, I, D>(
    kernel: &mut TextPlannedKernel,
    clock: &mut C,
    serial: &mut S,
    interrupts: &mut I,
    idle: &mut D,
) -> Result<MachineProof, MachineRunError>
where
    C: MonotonicClockBase,
    S: SerialBase,
    I: InterruptBase,
    D: IdleBase,
{
    let started = clock.now();
    let disabled_state = interrupts.disable();
    if interrupts.is_enabled() {
        return Err(MachineRunError::InterruptBaseFailure);
    }
    interrupts.restore(disabled_state);
    if disabled_state.enabled != interrupts.is_enabled() {
        return Err(MachineRunError::InterruptBaseFailure);
    }
    let _ = interrupts.disable();

    for _ in 0..MAXIMUM_KERNEL_STEPS {
        while let Some(request) = kernel.next_host_request() {
            if !kernel.is_presentation_request(&request) {
                return Err(MachineRunError::UnexpectedHostOperation);
            }
            {
                let value = kernel
                    .host_value(request.input.value)
                    .map_err(|_| MachineRunError::KernelFailure)?;
                core::str::from_utf8(value).map_err(|_| MachineRunError::SerialBaseFailure)?;
                serial
                    .present(value)
                    .map_err(|_| MachineRunError::SerialBaseFailure)?;
            }
            kernel
                .complete_presentation(request)
                .map_err(|_| MachineRunError::KernelFailure)?;
        }

        match kernel.step().map_err(|_| MachineRunError::KernelFailure)? {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Idle => {
                if kernel.pending_host_operations() == 0 {
                    return Err(MachineRunError::FalseIdle);
                }
                idle.wait_for_interrupt()
                    .map_err(|_| MachineRunError::InterruptBaseFailure)?;
            }
            SchedulerStatus::Complete => {
                return Ok(MachineProof {
                    logical_operations: 2,
                    decisions: kernel.decisions(),
                    kernel_signs: kernel.sign_count(),
                    timer_irq_wakes: 0,
                    idle_entries: idle.idle_count(),
                    serial_presentations: serial.presentation_count(),
                    clock_monotonic: clock.now() >= started,
                    pending_host_operations: kernel.pending_host_operations() as u8,
                });
            }
            SchedulerStatus::Cancelled => return Err(MachineRunError::KernelFailure),
        }
    }
    Err(MachineRunError::StepLimitExceeded)
}
