//! Bounded native Line transfer between two separately prepared fragment Plays.

use conduit_kernel::scheduler::{RemoteIngressOutcome, SchedulerStatus};

use crate::{
    composition::{MachineRunError, MachineRunReceipt},
    machine::SerialBase,
    tour_two_host_plan::PreparedTourTwoHostPlan,
};

const MAXIMUM_STEPS: u32 = 32;

pub fn run(
    prepared: &mut PreparedTourTwoHostPlan,
    serial: &mut impl SerialBase,
) -> Result<MachineRunReceipt, MachineRunError> {
    let mut bytes = [0_u8; conduit_text::MAX_TEXT_BYTES as usize];
    let mut transfer = None;
    for _ in 0..MAXIMUM_STEPS {
        if let Some((sequence, offered)) = prepared
            .source
            .offer()
            .map_err(|_| MachineRunError::KernelFailure)?
        {
            let len = offered.len();
            bytes[..len].copy_from_slice(offered);
            transfer = Some((sequence, len));
            break;
        }
        step_source(&mut prepared.source)?;
    }
    let (sequence, byte_len) = transfer.ok_or(MachineRunError::StepLimitExceeded)?;
    prepared
        .source
        .accept(sequence)
        .map_err(|_| MachineRunError::KernelFailure)?;
    if !matches!(
        prepared
            .sink
            .admit(sequence, &bytes[..byte_len])
            .map_err(|_| MachineRunError::KernelFailure)?,
        RemoteIngressOutcome::Accepted { sequence: accepted } if accepted == sequence
    ) {
        return Err(MachineRunError::KernelFailure);
    }
    let mut presented = false;
    for _ in 0..MAXIMUM_STEPS {
        if let Some(request) = prepared.sink.request() {
            let value = prepared
                .sink
                .value(request)
                .map_err(|_| MachineRunError::KernelFailure)?;
            if value != &bytes[..byte_len] {
                return Err(MachineRunError::TextMalformedUtf8);
            }
            serial
                .present(value)
                .map_err(|_| MachineRunError::SerialBaseFailure)?;
            prepared
                .sink
                .complete(request)
                .map_err(|_| MachineRunError::KernelFailure)?;
            presented = true;
            break;
        }
        step_sink(&mut prepared.sink)?;
    }
    if !presented {
        return Err(MachineRunError::StepLimitExceeded);
    }
    prepared
        .source
        .delivered(sequence)
        .map_err(|_| MachineRunError::KernelFailure)?;
    prepared
        .sink
        .close()
        .map_err(|_| MachineRunError::KernelFailure)?;
    drain_source(&mut prepared.source)?;
    drain_sink(&mut prepared.sink)?;
    Ok(MachineRunReceipt {
        logical_operations: 2,
        decisions: prepared.source.decisions() + prepared.sink.decisions(),
        kernel_signs: prepared.source.signs() + prepared.sink.signs(),
        timer_irq_wakes: 0,
        idle_entries: 0,
        serial_presentations: 1,
        clock_monotonic: true,
        pending_host_calls: 0,
        overlap_witness: false,
        timer_pending_during_text_progress: false,
        physical_parallelism: false,
    })
}

fn step_source(
    source: &mut crate::tour_two_host_kernel::SourceKernel,
) -> Result<(), MachineRunError> {
    match source.step().map_err(|_| MachineRunError::KernelFailure)? {
        SchedulerStatus::Progress { .. } => Ok(()),
        SchedulerStatus::Drained | SchedulerStatus::Idle | SchedulerStatus::Cancelled => {
            Err(MachineRunError::KernelFailure)
        }
    }
}

fn step_sink(sink: &mut crate::tour_two_host_kernel::SinkKernel) -> Result<(), MachineRunError> {
    match sink.step().map_err(|_| MachineRunError::KernelFailure)? {
        SchedulerStatus::Progress { .. } => Ok(()),
        SchedulerStatus::Drained | SchedulerStatus::Idle | SchedulerStatus::Cancelled => {
            Err(MachineRunError::KernelFailure)
        }
    }
}

fn drain_source(
    source: &mut crate::tour_two_host_kernel::SourceKernel,
) -> Result<(), MachineRunError> {
    for _ in 0..MAXIMUM_STEPS {
        match source.step().map_err(|_| MachineRunError::KernelFailure)? {
            SchedulerStatus::Drained => return Ok(()),
            SchedulerStatus::Progress { .. } => {}
            _ => return Err(MachineRunError::KernelFailure),
        }
    }
    Err(MachineRunError::StepLimitExceeded)
}

fn drain_sink(sink: &mut crate::tour_two_host_kernel::SinkKernel) -> Result<(), MachineRunError> {
    for _ in 0..MAXIMUM_STEPS {
        match sink.step().map_err(|_| MachineRunError::KernelFailure)? {
            SchedulerStatus::Drained => return Ok(()),
            SchedulerStatus::Progress { .. } => {}
            _ => return Err(MachineRunError::KernelFailure),
        }
    }
    Err(MachineRunError::StepLimitExceeded)
}
