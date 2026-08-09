//! Architecture-neutral ownership loop for the first machine-backed kernel profile.

use conduit_kernel::scheduler::SchedulerStatus;

use crate::{
    kernel_profile::{KernelProfile, PRESENT_OPERATION, WAIT_OPERATION},
    machine::{IdleBase, InterruptBase, MonotonicClockBase, SerialBase, TimerBase},
};

const MAXIMUM_KERNEL_STEPS: u32 = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineProof {
    pub logical_operations: u8,
    pub decisions: u32,
    pub kernel_signs: u16,
    pub timer_irq_wakes: u32,
    pub idle_entries: u32,
    pub serial_presentations: u32,
    pub clock_monotonic: bool,
    pub pending_host_operations: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineRunError {
    KernelConstruction,
    KernelFailure,
    UnexpectedHostOperation,
    TimerBaseFailure,
    SerialBaseFailure,
    InterruptBaseFailure,
    FalseIdle,
    StepLimitExceeded,
}

impl MachineRunError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KernelConstruction => "kernel-construction-failed",
            Self::KernelFailure => "production-kernel-failed",
            Self::UnexpectedHostOperation => "unadmitted-host-operation",
            Self::TimerBaseFailure => "timer-base-failed",
            Self::SerialBaseFailure => "serial-base-failed",
            Self::InterruptBaseFailure => "interrupt-base-failed",
            Self::FalseIdle => "kernel-false-idle",
            Self::StepLimitExceeded => "kernel-step-limit-exceeded",
        }
    }
}

pub fn run<C, T, S, I, D>(
    clock: &mut C,
    timer: &mut T,
    serial: &mut S,
    interrupts: &mut I,
    idle: &mut D,
) -> Result<MachineProof, MachineRunError>
where
    C: MonotonicClockBase,
    T: TimerBase,
    S: SerialBase,
    I: InterruptBase,
    D: IdleBase,
{
    let mut kernel = KernelProfile::new().map_err(|_| MachineRunError::KernelConstruction)?;
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
        if let Some(interest) = timer
            .take_wake()
            .map_err(|_| MachineRunError::TimerBaseFailure)?
        {
            kernel
                .complete_timer(interest)
                .map_err(|_| MachineRunError::KernelFailure)?;
        }

        while let Some(request) = kernel.next_host_request() {
            if request.operation == WAIT_OPERATION {
                let interest = KernelProfile::timer_interest(request)
                    .map_err(|_| MachineRunError::UnexpectedHostOperation)?;
                timer
                    .arm(interest)
                    .map_err(|_| MachineRunError::TimerBaseFailure)?;
            } else if request.operation == PRESENT_OPERATION {
                {
                    let value = kernel
                        .host_value(request.input.value)
                        .map_err(|_| MachineRunError::KernelFailure)?;
                    serial
                        .present(value)
                        .map_err(|_| MachineRunError::SerialBaseFailure)?;
                }
                kernel
                    .complete_serial(request)
                    .map_err(|_| MachineRunError::KernelFailure)?;
            } else {
                return Err(MachineRunError::UnexpectedHostOperation);
            }
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
                let ended = clock.now();
                return Ok(MachineProof {
                    logical_operations: crate::kernel_profile::NODE_COUNT as u8,
                    decisions: kernel.decisions(),
                    kernel_signs: kernel.sign_count(),
                    timer_irq_wakes: timer.wake_count(),
                    idle_entries: idle.idle_count(),
                    serial_presentations: serial.presentation_count(),
                    clock_monotonic: ended >= started,
                    pending_host_operations: kernel.pending_host_operations() as u8,
                });
            }
            SchedulerStatus::Cancelled => return Err(MachineRunError::KernelFailure),
        }
    }
    Err(MachineRunError::StepLimitExceeded)
}
