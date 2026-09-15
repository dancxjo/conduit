//! Native standing lifecycle for one initial count, one timer bump, and explicit stop.

use conduit_kernel::scheduler::{HostOperationRequest, SchedulerStatus};

use crate::{
    composition::{MachineRunError, MachineRunReceipt},
    machine::{IdleBase, InterruptBase, KernelInterest, MonotonicClockBase, SerialBase, TimerBase},
    tour_timer_kernel::TourTimerKernel,
    tour_timer_plan::PreparedTourTimerPlan,
};

const MAXIMUM_KERNEL_STEPS: u32 = 192;

pub(crate) fn run<C, T, S, I, D>(
    prepared: &mut PreparedTourTimerPlan,
    clock: &mut C,
    timer: &mut T,
    serial: &mut S,
    interrupts: &mut I,
    idle: &mut D,
) -> Result<MachineRunReceipt, MachineRunError>
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
    let _ = interrupts.disable();
    let mut timer_token = None;
    let mut counts = [None, None];
    let mut count_len = 0_usize;

    for _ in 0..MAXIMUM_KERNEL_STEPS {
        if count_len == 1
            && timer.wake_count() == 0
            && let Some(interest) = timer
                .take_wake()
                .map_err(|_| MachineRunError::TimerBaseFailure)?
        {
            prepared
                .kernel
                .complete_timer(interest)
                .map_err(|_| MachineRunError::KernelFailure)?;
            timer_token = None;
        }
        while let Some(request) = prepared.kernel.next_host_request() {
            if prepared.kernel.is_timer(&request) {
                if timer_token.is_some() {
                    return Err(MachineRunError::TimerBaseFailure);
                }
                let interest = timer_interest(request)?;
                timer_token = Some(
                    timer
                        .arm(interest)
                        .map_err(|_| MachineRunError::TimerBaseFailure)?,
                );
            } else if prepared.kernel.is_presentation(&request) {
                present_count(
                    &mut prepared.kernel,
                    request,
                    serial,
                    &mut counts,
                    &mut count_len,
                )?;
            } else {
                return Err(MachineRunError::UnexpectedHostOperation);
            }
        }
        if count_len == 2
            && prepared.kernel.pending_host_operations() == 1
            && let Some(token) = timer_token
        {
            let cancelled = timer
                .cancel(token)
                .map_err(|_| MachineRunError::TimerBaseFailure)?;
            if cancelled.input.value.byte_len != 8 {
                return Err(MachineRunError::TimerBaseFailure);
            }
            prepared
                .kernel
                .cancel()
                .map_err(|_| MachineRunError::KernelFailure)?;
            if prepared
                .kernel
                .step()
                .map_err(|_| MachineRunError::KernelFailure)?
                != SchedulerStatus::Cancelled
                || counts != [Some(0), Some(1)]
            {
                return Err(MachineRunError::KernelFailure);
            }
            return Ok(MachineRunReceipt {
                logical_operations: 3,
                decisions: prepared.kernel.decisions(),
                kernel_signs: prepared.kernel.sign_count(),
                timer_irq_wakes: timer.wake_count(),
                idle_entries: idle.idle_count(),
                serial_presentations: serial.presentation_count(),
                clock_monotonic: clock.now() >= started,
                pending_host_operations: 1,
                overlap_witness: false,
                timer_pending_during_text_progress: true,
                physical_parallelism: false,
            });
        }
        match prepared.kernel.step().map_err(step_error)? {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Idle => {
                if prepared.kernel.pending_host_operations() == 0 {
                    return Err(MachineRunError::FalseIdle);
                }
                idle.wait_for_interrupt()
                    .map_err(|_| MachineRunError::InterruptBaseFailure)?;
            }
            SchedulerStatus::Drained | SchedulerStatus::Cancelled => {
                return Err(MachineRunError::KernelFailure);
            }
        }
    }
    Err(MachineRunError::StepLimitExceeded)
}

fn step_error(_error: conduit_kernel::scheduler::SchedulerError) -> MachineRunError {
    MachineRunError::KernelFailure
}

fn timer_interest(request: HostOperationRequest) -> Result<KernelInterest, MachineRunError> {
    if request.operation != conduit_kernel::HostOperationId(0) {
        return Err(MachineRunError::UnexpectedHostOperation);
    }
    Ok(KernelInterest {
        node: request.node,
        request: request.request,
        input: request.input,
    })
}

fn present_count<S: SerialBase>(
    kernel: &mut TourTimerKernel,
    request: HostOperationRequest,
    serial: &mut S,
    counts: &mut [Option<u64>; 2],
    count_len: &mut usize,
) -> Result<(), MachineRunError> {
    if *count_len >= counts.len() {
        return Err(MachineRunError::SerialBaseFailure);
    }
    let bytes = kernel
        .host_value(request.input.value)
        .map_err(|_| MachineRunError::KernelFailure)?;
    let encoded: [u8; 8] = bytes
        .try_into()
        .map_err(|_| MachineRunError::SerialBaseFailure)?;
    let count = u64::from_le_bytes(encoded);
    let text = match count {
        0 => b"0".as_slice(),
        1 => b"1".as_slice(),
        _ => return Err(MachineRunError::SerialBaseFailure),
    };
    serial
        .present(text)
        .map_err(|_| MachineRunError::SerialBaseFailure)?;
    counts[*count_len] = Some(count);
    *count_len += 1;
    kernel
        .complete_presentation(request)
        .map_err(|_| MachineRunError::KernelFailure)
}
