//! Production-kernel dispatch for one planned Create observation.

use crate::create_observation_kernel_operations::{prepare_scheduler, sink_received, Scheduler};
use crate::{
    encode_create_observation, CreateObservationChannel, CreateObservationDispatchFailure,
    CreateObservationEvidence, CreateObservationExecutionFailure, CreateObservationExecutionReport,
    CreateObservationSession, CreateObservationSnapshot, CreateObservationTerminal,
    CreateOdometryAccumulator, CreateOdometryResetAuthority, CreateOdometryResetBinding,
    CreateOdometryResetRefusal, CreateOdometryResetRequest, CreateOdometryResetSign,
    CreateUartProvider,
};
use conduit_core::{BootId, HostId, OfferGeneration, Plan};
use conduit_kernel::{
    scheduler::{HostOperationRequest, SchedulerStatus},
    BoundedValueRef, HostOperationDisposition, HostOperationOutcome, SignSink,
};

pub(super) const MAXIMUM_VALUE_BYTES: usize = conduit_core::ROBOTICS_CHARGING_ENCODED_LEN;

#[derive(Clone, Copy)]
struct PendingObservationCompletion {
    request: HostOperationRequest,
    output: BoundedValueRef,
    canonical: [u8; MAXIMUM_VALUE_BYTES],
    canonical_len: u8,
    generation: u32,
    observed_at_tick: u64,
}

pub struct PreparedCreateObservationExecution {
    scheduler: Scheduler,
    pub(super) channel: CreateObservationChannel,
    pub(super) host_id: HostId,
    pub(super) boot_id: BootId,
    pub(super) offer_generation: OfferGeneration,
    pub(super) serial_base_id: String,
    pub(super) robot_identity: String,
    pub(super) maximum_age_ticks: u32,
    pub(super) next_observation_generation: u32,
    pub(super) dispatched: bool,
    odometry: CreateOdometryAccumulator,
    pub(super) odometry_frame_generation: Option<u32>,
    pub(super) odometry_sample_generation: Option<u32>,
    pending: Option<PendingObservationCompletion>,
}

impl PreparedCreateObservationExecution {
    pub(super) fn kernel_decisions(&self) -> u32 {
        self.scheduler.decisions()
    }

    pub(super) fn kernel_signs(&self) -> u16 {
        self.scheduler.signs().len()
    }
}

pub fn prepare_create_observation_execution(
    plan: &Plan,
    channel: CreateObservationChannel,
    evidence: &CreateObservationEvidence,
) -> Result<PreparedCreateObservationExecution, &'static str> {
    prepare_create_observation_execution_with_odometry(
        plan,
        channel,
        evidence,
        CreateOdometryAccumulator::new(),
    )
}

pub fn prepare_create_observation_execution_with_odometry(
    plan: &Plan,
    channel: CreateObservationChannel,
    evidence: &CreateObservationEvidence,
    odometry: CreateOdometryAccumulator,
) -> Result<PreparedCreateObservationExecution, &'static str> {
    crate::create_observation_plan_validation::validate_plan(plan, channel, evidence)?;
    let maximum_output_bytes = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.implementation_id.as_str() == channel.implementation_id())
        .and_then(|placement| placement.host_operations.first())
        .map(|operation| operation.maximum_output_bytes)
        .ok_or("planned observation operation missing")?;
    let scheduler = prepare_scheduler(maximum_output_bytes)?;
    Ok(PreparedCreateObservationExecution {
        scheduler,
        channel,
        host_id: evidence.host_id.clone(),
        boot_id: evidence.boot_id.clone(),
        offer_generation: evidence.offer_generation,
        serial_base_id: evidence.serial_base_id.clone(),
        robot_identity: evidence.robot_identity.clone(),
        maximum_age_ticks: evidence.maximum_age_ticks,
        next_observation_generation: 1,
        dispatched: false,
        odometry,
        odometry_frame_generation: (channel == CreateObservationChannel::Odometry)
            .then_some(odometry.frame_generation()),
        odometry_sample_generation: (channel == CreateObservationChannel::Odometry)
            .then_some(odometry.sample_generation()),
        pending: None,
    })
}

pub fn create_observation_odometry_state(
    execution: &PreparedCreateObservationExecution,
) -> Option<CreateOdometryAccumulator> {
    (execution.channel == CreateObservationChannel::Odometry).then_some(execution.odometry)
}

pub fn run_create_observation_execution<P: CreateUartProvider>(
    execution: &mut PreparedCreateObservationExecution,
    provider: &mut P,
    read_deadline_tick: u64,
    observed_at_tick: u64,
    now_tick: u64,
) -> CreateObservationExecutionReport {
    if let Err(failure) = dispatch_create_observation_execution(
        execution,
        provider,
        read_deadline_tick,
        observed_at_tick,
        now_tick,
    ) {
        return report(
            execution,
            failure.terminal,
            failure.observation_generation,
            failure.observed_at_tick,
            &[],
        );
    }
    finish_create_observation_execution(execution)
}

pub fn dispatch_create_observation_execution<P: CreateUartProvider>(
    execution: &mut PreparedCreateObservationExecution,
    provider: &mut P,
    read_deadline_tick: u64,
    observed_at_tick: u64,
    now_tick: u64,
) -> Result<(), CreateObservationDispatchFailure> {
    if execution.pending.is_some() || execution.dispatched {
        return Err(CreateObservationDispatchFailure {
            terminal: CreateObservationTerminal::Failed(
                CreateObservationExecutionFailure::KernelRefused,
            ),
            observation_generation: None,
            observed_at_tick: None,
        });
    }
    let request = match next_request(execution) {
        Ok(request) => request,
        Err(terminal) => {
            return Err(CreateObservationDispatchFailure {
                terminal,
                observation_generation: None,
                observed_at_tick: None,
            });
        }
    };
    execution.dispatched = true;
    let mut session = CreateObservationSession::new();
    let observation = match session
        .start(provider)
        .and_then(|()| session.read(provider, read_deadline_tick))
    {
        Ok(observation) => observation,
        Err(failure) => {
            let _ = session.pause(provider);
            let terminal = CreateObservationTerminal::Failed(
                CreateObservationExecutionFailure::Session(failure),
            );
            fail_request(execution, request, terminal);
            return Err(CreateObservationDispatchFailure {
                terminal,
                observation_generation: None,
                observed_at_tick: None,
            });
        }
    };
    if let Err(failure) = session.pause(provider) {
        let terminal =
            CreateObservationTerminal::Failed(CreateObservationExecutionFailure::Session(failure));
        fail_request(execution, request, terminal);
        return Err(CreateObservationDispatchFailure {
            terminal,
            observation_generation: None,
            observed_at_tick: None,
        });
    }
    let odometry = if execution.channel == CreateObservationChannel::Odometry {
        match execution.odometry.integrate(
            observation.group_zero.distance_delta_mm,
            observation.group_zero.angle_delta_degrees,
        ) {
            Ok(sample) => {
                execution.odometry_frame_generation = Some(sample.frame_generation);
                execution.odometry_sample_generation = Some(sample.sample_generation);
                Some(sample)
            }
            Err(failure) => {
                let terminal =
                    CreateObservationTerminal::Failed(CreateObservationExecutionFailure::Encoding(
                        crate::CreateObservationEncodeRefusal::Odometry(failure),
                    ));
                fail_request(execution, request, terminal);
                return Err(CreateObservationDispatchFailure {
                    terminal,
                    observation_generation: None,
                    observed_at_tick: Some(observed_at_tick),
                });
            }
        }
    } else {
        None
    };
    let generation = execution.next_observation_generation;
    execution.next_observation_generation = execution.next_observation_generation.saturating_add(1);
    let snapshot = CreateObservationSnapshot {
        host_id: execution.host_id.clone(),
        boot_id: execution.boot_id.clone(),
        offer_generation: execution.offer_generation,
        serial_base_id: execution.serial_base_id.clone(),
        robot_identity: execution.robot_identity.clone(),
        observation_generation: generation,
        observed_at_tick,
        maximum_age_ticks: execution.maximum_age_ticks,
        observation,
        odometry,
    };
    let encoded = match encode_create_observation(&snapshot, execution.channel, now_tick) {
        Ok(Some(encoded)) => encoded,
        Ok(None) => {
            let terminal = CreateObservationTerminal::Failed(
                CreateObservationExecutionFailure::MissingCurrentValue,
            );
            fail_request(execution, request, terminal);
            return Err(CreateObservationDispatchFailure {
                terminal,
                observation_generation: Some(generation),
                observed_at_tick: Some(observed_at_tick),
            });
        }
        Err(failure) => {
            let terminal = CreateObservationTerminal::Failed(
                CreateObservationExecutionFailure::Encoding(failure),
            );
            fail_request(execution, request, terminal);
            return Err(CreateObservationDispatchFailure {
                terminal,
                observation_generation: Some(generation),
                observed_at_tick: Some(observed_at_tick),
            });
        }
    };
    let value = match execution.scheduler.store_host_value(encoded.as_bytes()) {
        Ok(value) => value,
        Err(_) => {
            let terminal =
                CreateObservationTerminal::Failed(CreateObservationExecutionFailure::KernelRefused);
            fail_request(execution, request, terminal);
            return Err(CreateObservationDispatchFailure {
                terminal,
                observation_generation: Some(generation),
                observed_at_tick: Some(observed_at_tick),
            });
        }
    };
    let bounded = BoundedValueRef::new(value, encoded.as_bytes().len() as u32)
        .expect("stored fixed observation is bounded");
    let mut canonical = [0_u8; MAXIMUM_VALUE_BYTES];
    canonical[..encoded.as_bytes().len()].copy_from_slice(encoded.as_bytes());
    execution.pending = Some(PendingObservationCompletion {
        request,
        output: bounded,
        canonical,
        canonical_len: encoded.as_bytes().len() as u8,
        generation,
        observed_at_tick,
    });
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateObservationOdometryResetRefusal {
    WrongChannel,
    ObservationInFlightOrFinished,
    Reset(CreateOdometryResetRefusal),
}

pub fn reset_create_observation_odometry(
    execution: &mut PreparedCreateObservationExecution,
    request: CreateOdometryResetRequest<'_>,
    authority: Option<CreateOdometryResetAuthority<'_>>,
    now_tick: u64,
) -> Result<CreateOdometryResetSign, CreateObservationOdometryResetRefusal> {
    if execution.channel != CreateObservationChannel::Odometry {
        return Err(CreateObservationOdometryResetRefusal::WrongChannel);
    }
    if execution.dispatched || execution.pending.is_some() {
        return Err(CreateObservationOdometryResetRefusal::ObservationInFlightOrFinished);
    }
    let sign = execution
        .odometry
        .reset(
            request,
            authority,
            CreateOdometryResetBinding {
                host_id: &execution.host_id,
                boot_id: &execution.boot_id,
                offer_generation: execution.offer_generation,
                implementation_id: execution.channel.implementation_id(),
            },
            now_tick,
        )
        .map_err(CreateObservationOdometryResetRefusal::Reset)?;
    execution.odometry_frame_generation = Some(sign.current_frame_generation);
    execution.odometry_sample_generation = Some(0);
    Ok(sign)
}

pub fn finish_create_observation_execution(
    execution: &mut PreparedCreateObservationExecution,
) -> CreateObservationExecutionReport {
    let Some(pending) = execution.pending.take() else {
        return report(
            execution,
            CreateObservationTerminal::Failed(CreateObservationExecutionFailure::KernelRefused),
            None,
            None,
            &[],
        );
    };
    if execution
        .scheduler
        .complete_host_operation(
            pending.request.node,
            pending.request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(pending.output),
                failure: None,
            },
        )
        .is_err()
    {
        return report(
            execution,
            CreateObservationTerminal::Failed(CreateObservationExecutionFailure::KernelRefused),
            Some(pending.generation),
            Some(pending.observed_at_tick),
            &[],
        );
    }
    for _ in 0..32 {
        match execution.scheduler.step() {
            Ok(SchedulerStatus::Complete) if sink_received(&execution.scheduler) => {
                return report(
                    execution,
                    CreateObservationTerminal::Completed,
                    Some(pending.generation),
                    Some(pending.observed_at_tick),
                    &pending.canonical[..usize::from(pending.canonical_len)],
                );
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    report(
        execution,
        CreateObservationTerminal::Failed(CreateObservationExecutionFailure::KernelRefused),
        Some(pending.generation),
        Some(pending.observed_at_tick),
        &[],
    )
}

pub fn cancel_create_observation_execution(
    execution: &mut PreparedCreateObservationExecution,
) -> CreateObservationExecutionReport {
    let pending = execution.pending.take();
    let terminal = if execution.dispatched {
        CreateObservationTerminal::CancelledAfterDispatch
    } else {
        CreateObservationTerminal::CancelledBeforeDispatch
    };
    let _ = execution.scheduler.cancel();
    report(
        execution,
        terminal,
        pending.map(|value| value.generation),
        pending.map(|value| value.observed_at_tick),
        &[],
    )
}

fn next_request(
    execution: &mut PreparedCreateObservationExecution,
) -> Result<HostOperationRequest, CreateObservationTerminal> {
    for _ in 0..16 {
        execution.scheduler.step().map_err(|_| {
            CreateObservationTerminal::Failed(CreateObservationExecutionFailure::KernelRefused)
        })?;
        if let Some(request) = execution.scheduler.next_host_request() {
            if execution
                .scheduler
                .host_value(request.input.value)
                .is_ok_and(|value| value.is_empty())
            {
                return Ok(request);
            }
            return Err(CreateObservationTerminal::Failed(
                CreateObservationExecutionFailure::KernelRefused,
            ));
        }
    }
    Err(CreateObservationTerminal::Failed(
        CreateObservationExecutionFailure::KernelRefused,
    ))
}

fn fail_request(
    execution: &mut PreparedCreateObservationExecution,
    request: HostOperationRequest,
    terminal: CreateObservationTerminal,
) {
    let detail = match terminal {
        CreateObservationTerminal::Failed(CreateObservationExecutionFailure::Session(_)) => 1,
        CreateObservationTerminal::Failed(CreateObservationExecutionFailure::Encoding(_)) => 2,
        CreateObservationTerminal::Failed(
            CreateObservationExecutionFailure::MissingCurrentValue,
        ) => 3,
        _ => 4,
    };
    let _ = execution.scheduler.complete_host_operation(
        request.node,
        request.request,
        HostOperationOutcome {
            disposition: HostOperationDisposition::Failed,
            output: None,
            failure: Some(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::HostOperationFailed,
                detail,
            }),
        },
    );
}

use crate::create_observation_execution_report::report;

#[cfg(test)]
#[path = "create_observation_play_tests.rs"]
mod tests;
