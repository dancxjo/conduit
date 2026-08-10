//! Architecture-neutral ownership loop for an admitted ordinary Plan.

use conduit_kernel::scheduler::SchedulerStatus;

use crate::{
    machine::{IdleBase, InterruptBase, MonotonicClockBase, SerialBase, TimerBase},
    planned_kernel::{PRESENT_NODE, PlannedKernel, TIMER_NODE},
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
    pub overlap_witness: bool,
    pub timer_pending_during_text_progress: bool,
    pub physical_parallelism: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineRunError {
    KernelConstruction,
    KernelFailure,
    TextMalformedUtf8,
    TextOutputOverflow,
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
            Self::TextMalformedUtf8 => "text-upper-malformed-utf8",
            Self::TextOutputOverflow => "text-upper-output-overflow",
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
    kernel: &mut PlannedKernel,
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
            if request.node == TIMER_NODE {
                let interest = PlannedKernel::timer_interest(request)
                    .map_err(|_| MachineRunError::UnexpectedHostOperation)?;
                timer
                    .arm(interest)
                    .map_err(|_| MachineRunError::TimerBaseFailure)?;
            } else if request.node == PRESENT_NODE {
                {
                    let value = kernel
                        .host_value(request.input.value)
                        .map_err(|_| MachineRunError::KernelFailure)?;
                    serial
                        .present(value)
                        .map_err(|_| MachineRunError::SerialBaseFailure)?;
                }
                kernel
                    .complete_presentation(request)
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
                    logical_operations: 2,
                    decisions: kernel.decisions(),
                    kernel_signs: kernel.sign_count(),
                    timer_irq_wakes: timer.wake_count(),
                    idle_entries: idle.idle_count(),
                    serial_presentations: serial.presentation_count(),
                    clock_monotonic: ended >= started,
                    pending_host_operations: kernel.pending_host_operations() as u8,
                    overlap_witness: false,
                    timer_pending_during_text_progress: false,
                    physical_parallelism: false,
                });
            }
            SchedulerStatus::Cancelled => return Err(MachineRunError::KernelFailure),
        }
    }
    Err(MachineRunError::StepLimitExceeded)
}
