//! Production-kernel lifecycle for one bounded Create docking request.

use crate::create_dock_kernel_operations::{
    dock_is_admitted, prepare_dock_scheduler, DockScheduler, DOCK_NODE, DOCK_REQUEST, REQUEST_BYTES,
};
use crate::{
    encode_seek_dock, encode_stop, write_command, CreateDockObservation, CreateOiFailure,
    CreateUartProvider, LocalHazard, MotionAuthority, MotionSafetyAuthority, SafetyObservation,
};
use conduit_core::{BootId, HostId, InfoBool, OfferGeneration, Plan};
use conduit_kernel::{
    scheduler::HostOperationRequest, HostOperationDisposition, HostOperationOutcome, SignSink,
};

pub struct PreparedCreateDockExecution {
    scheduler: DockScheduler,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub serial_base_id: String,
    pub robot_identity: String,
    pub dock_resource_id: String,
    pub timer_resource_id: String,
    pub planned_authority_grant_id: String,
    pub timeout_ms: u32,
    advertised_safety_generation: u32,
    active_safety_generation: u32,
    authority_valid_until_tick: u64,
    deadline_tick: u64,
    dispatched: bool,
    active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DockSafeDisposition {
    Verified,
    Failed(CreateOiFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateDockExecutionRefusal {
    KernelRefused,
    MalformedBooleanInput,
    FalseRequest,
    MissingAuthority,
    AuthorityGrantMismatch,
    AuthorityExpired,
    SafetyAuthorityMismatch,
    SafetyGenerationRegressed,
    SafetyStaleOrInhibited(LocalHazard),
    DeadlineOverflow,
    Device(CreateOiFailure),
    InvalidLifecycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateDockExecutionTerminal {
    Docking {
        authority_grant_id: String,
        safety_generation: u32,
        deadline_tick: u64,
    },
    Active,
    Docked {
        safety_generation: u32,
    },
    ChargingObservedButStopFailed {
        safety_generation: u32,
        failure: CreateOiFailure,
    },
    TimedOut {
        safety_generation: u32,
        safe_disposition: DockSafeDisposition,
    },
    Cancelled {
        safety_generation: u32,
        safe_disposition: DockSafeDisposition,
    },
    AuthorityExpired {
        safety_generation: u32,
        safe_disposition: DockSafeDisposition,
    },
    SafetyInhibited {
        hazard: LocalHazard,
        safety_generation: u32,
        safe_disposition: DockSafeDisposition,
    },
    KernelRefusedAfterDockCommand {
        safety_generation: u32,
        safe_disposition: DockSafeDisposition,
    },
    CancelledBeforeDispatch,
    Refused(CreateDockExecutionRefusal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateDockExecutionReport {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub serial_base_id: String,
    pub robot_identity: String,
    pub dock_resource_id: String,
    pub timer_resource_id: String,
    pub kernel_decisions: u32,
    pub kernel_signs: u16,
    pub terminal: CreateDockExecutionTerminal,
}

pub fn dock_authority_admits(command: &[u8]) -> bool {
    command == encode_seek_dock().as_bytes() || command == encode_stop().as_bytes()
}

pub fn prepare_create_dock_execution(
    plan: &Plan,
    evidence: &CreateDockObservation,
) -> Result<PreparedCreateDockExecution, &'static str> {
    let validated = crate::create_dock_plan_validation::validate_create_dock_plan(plan, evidence)?;
    let scheduler = prepare_dock_scheduler(InfoBool::new(true).encode())?;
    Ok(PreparedCreateDockExecution {
        scheduler,
        host_id: evidence.host_id.clone(),
        boot_id: evidence.boot_id.clone(),
        offer_generation: evidence.offer_generation,
        serial_base_id: evidence.serial_base_id.clone(),
        robot_identity: evidence.robot_identity.clone(),
        dock_resource_id: evidence.dock_resource_id.clone(),
        timer_resource_id: evidence.timer_resource_id.clone(),
        planned_authority_grant_id: validated.authority_grant_id,
        timeout_ms: validated.timeout_ms,
        advertised_safety_generation: evidence.safety.generation,
        active_safety_generation: 0,
        authority_valid_until_tick: 0,
        deadline_tick: 0,
        dispatched: false,
        active: false,
    })
}

pub fn dispatch_create_dock_execution<'a, P: CreateUartProvider>(
    execution: &mut PreparedCreateDockExecution,
    provider: &mut P,
    now_tick: u64,
    authority: Option<MotionAuthority<'a>>,
    safety: SafetyObservation,
) -> CreateDockExecutionReport {
    if execution.dispatched || execution.active {
        return refused(execution, CreateDockExecutionRefusal::InvalidLifecycle);
    }
    let request = match next_request(execution) {
        Ok(request) => request,
        Err(refusal) => return refused(execution, refusal),
    };
    execution.dispatched = true;
    let request_value = match decode_request(execution, request) {
        Ok(value) => value,
        Err(refusal) => {
            fail_request(execution, request, refusal);
            return refused(execution, refusal);
        }
    };
    if !request_value.get() {
        fail_request(execution, request, CreateDockExecutionRefusal::FalseRequest);
        return refused(execution, CreateDockExecutionRefusal::FalseRequest);
    }
    let authority = match admit_authority(execution, authority, now_tick, safety) {
        Ok(authority) => authority,
        Err(refusal) => {
            fail_request(execution, request, refusal);
            return refused(execution, refusal);
        }
    };
    let Some(deadline_tick) = now_tick.checked_add(u64::from(execution.timeout_ms)) else {
        fail_request(
            execution,
            request,
            CreateDockExecutionRefusal::DeadlineOverflow,
        );
        return refused(execution, CreateDockExecutionRefusal::DeadlineOverflow);
    };
    if let Err(failure) = write_command(provider, &encode_seek_dock()) {
        let refusal = CreateDockExecutionRefusal::Device(failure);
        fail_request(execution, request, refusal);
        return refused(execution, refusal);
    }
    if execution
        .scheduler
        .complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .is_err()
        || !settle_admission(execution)
    {
        let safe_disposition = stop(provider);
        return report(
            execution,
            CreateDockExecutionTerminal::KernelRefusedAfterDockCommand {
                safety_generation: safety.generation,
                safe_disposition,
            },
        );
    }
    execution.active = true;
    execution.active_safety_generation = safety.generation;
    execution.authority_valid_until_tick = authority.valid_until_tick;
    execution.deadline_tick = deadline_tick;
    report(
        execution,
        CreateDockExecutionTerminal::Docking {
            authority_grant_id: authority.grant_id.to_string(),
            safety_generation: safety.generation,
            deadline_tick,
        },
    )
}

pub fn supervise_create_dock_execution<P: CreateUartProvider>(
    execution: &mut PreparedCreateDockExecution,
    provider: &mut P,
    now_tick: u64,
    safety: SafetyObservation,
) -> CreateDockExecutionReport {
    if !execution.active {
        return refused(execution, CreateDockExecutionRefusal::InvalidLifecycle);
    }
    if safety.generation < execution.active_safety_generation {
        return terminate_safety(
            execution,
            provider,
            LocalHazard::SafetyGenerationRegressed,
            safety.generation,
        );
    }
    if safety.charging {
        execution.active = false;
        let safe_disposition = stop(provider);
        let _ = execution.scheduler.cancel();
        let terminal = match safe_disposition {
            DockSafeDisposition::Verified => CreateDockExecutionTerminal::Docked {
                safety_generation: safety.generation,
            },
            DockSafeDisposition::Failed(failure) => {
                CreateDockExecutionTerminal::ChargingObservedButStopFailed {
                    safety_generation: safety.generation,
                    failure,
                }
            }
        };
        return report(execution, terminal);
    }
    if now_tick >= execution.authority_valid_until_tick {
        execution.active = false;
        let safe_disposition = stop(provider);
        let _ = execution.scheduler.cancel();
        return report(
            execution,
            CreateDockExecutionTerminal::AuthorityExpired {
                safety_generation: safety.generation,
                safe_disposition,
            },
        );
    }
    if now_tick >= execution.deadline_tick {
        execution.active = false;
        let safe_disposition = stop(provider);
        let _ = execution.scheduler.cancel();
        return report(
            execution,
            CreateDockExecutionTerminal::TimedOut {
                safety_generation: safety.generation,
                safe_disposition,
            },
        );
    }
    if let Some(hazard) = safety.first_hazard(now_tick) {
        return terminate_safety(execution, provider, hazard, safety.generation);
    }
    report(execution, CreateDockExecutionTerminal::Active)
}

pub fn cancel_create_dock_execution<P: CreateUartProvider>(
    execution: &mut PreparedCreateDockExecution,
    provider: &mut P,
    safety_generation: u32,
) -> CreateDockExecutionReport {
    if !execution.dispatched {
        let _ = execution.scheduler.cancel();
        return report(
            execution,
            CreateDockExecutionTerminal::CancelledBeforeDispatch,
        );
    }
    if !execution.active {
        return refused(execution, CreateDockExecutionRefusal::InvalidLifecycle);
    }
    execution.active = false;
    let safe_disposition = stop(provider);
    let _ = execution.scheduler.cancel();
    report(
        execution,
        CreateDockExecutionTerminal::Cancelled {
            safety_generation,
            safe_disposition,
        },
    )
}

fn admit_authority<'a>(
    execution: &PreparedCreateDockExecution,
    authority: Option<MotionAuthority<'a>>,
    now_tick: u64,
    safety: SafetyObservation,
) -> Result<MotionAuthority<'a>, CreateDockExecutionRefusal> {
    let authority = authority.ok_or(CreateDockExecutionRefusal::MissingAuthority)?;
    if authority.grant_id != execution.planned_authority_grant_id {
        return Err(CreateDockExecutionRefusal::AuthorityGrantMismatch);
    }
    if authority.valid_until_tick <= now_tick {
        return Err(CreateDockExecutionRefusal::AuthorityExpired);
    }
    if safety.generation < execution.advertised_safety_generation {
        return Err(CreateDockExecutionRefusal::SafetyGenerationRegressed);
    }
    if let Some(hazard) = safety.first_hazard(now_tick) {
        return Err(CreateDockExecutionRefusal::SafetyStaleOrInhibited(hazard));
    }
    match (
        safety.has_complete_independent_envelope(),
        authority.safety_class,
    ) {
        (true, MotionSafetyAuthority::IndependentWatchdog)
        | (
            false,
            MotionSafetyAuthority::ReducedWheelsOffFloor
            | MotionSafetyAuthority::ReducedFloorAcknowledged,
        ) => Ok(authority),
        _ => Err(CreateDockExecutionRefusal::SafetyAuthorityMismatch),
    }
}

fn next_request(
    execution: &mut PreparedCreateDockExecution,
) -> Result<HostOperationRequest, CreateDockExecutionRefusal> {
    for _ in 0..24 {
        execution
            .scheduler
            .step()
            .map_err(|_| CreateDockExecutionRefusal::KernelRefused)?;
        if let Some(request) = execution.scheduler.next_host_request() {
            return Ok(request);
        }
    }
    Err(CreateDockExecutionRefusal::KernelRefused)
}

fn decode_request(
    execution: &PreparedCreateDockExecution,
    request: HostOperationRequest,
) -> Result<InfoBool, CreateDockExecutionRefusal> {
    if request.node != DOCK_NODE
        || request.request != DOCK_REQUEST
        || request.input.admitted_bytes != REQUEST_BYTES
    {
        return Err(CreateDockExecutionRefusal::KernelRefused);
    }
    let bytes = execution
        .scheduler
        .host_value(request.input.value)
        .map_err(|_| CreateDockExecutionRefusal::KernelRefused)?;
    InfoBool::decode(bytes).map_err(|_| CreateDockExecutionRefusal::MalformedBooleanInput)
}

fn settle_admission(execution: &mut PreparedCreateDockExecution) -> bool {
    for _ in 0..12 {
        if execution.scheduler.step().is_err() {
            return false;
        }
        if dock_is_admitted(&execution.scheduler) {
            return true;
        }
    }
    false
}

fn fail_request(
    execution: &mut PreparedCreateDockExecution,
    request: HostOperationRequest,
    refusal: CreateDockExecutionRefusal,
) {
    let detail = match refusal {
        CreateDockExecutionRefusal::MalformedBooleanInput => 1,
        CreateDockExecutionRefusal::FalseRequest => 2,
        CreateDockExecutionRefusal::MissingAuthority => 3,
        CreateDockExecutionRefusal::AuthorityGrantMismatch => 4,
        CreateDockExecutionRefusal::AuthorityExpired => 5,
        CreateDockExecutionRefusal::SafetyAuthorityMismatch => 6,
        CreateDockExecutionRefusal::SafetyGenerationRegressed => 7,
        CreateDockExecutionRefusal::SafetyStaleOrInhibited(_) => 8,
        CreateDockExecutionRefusal::DeadlineOverflow => 9,
        CreateDockExecutionRefusal::Device(_) => 10,
        CreateDockExecutionRefusal::KernelRefused
        | CreateDockExecutionRefusal::InvalidLifecycle => 11,
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

fn terminate_safety<P: CreateUartProvider>(
    execution: &mut PreparedCreateDockExecution,
    provider: &mut P,
    hazard: LocalHazard,
    safety_generation: u32,
) -> CreateDockExecutionReport {
    execution.active = false;
    let safe_disposition = stop(provider);
    let _ = execution.scheduler.cancel();
    report(
        execution,
        CreateDockExecutionTerminal::SafetyInhibited {
            hazard,
            safety_generation,
            safe_disposition,
        },
    )
}

fn stop<P: CreateUartProvider>(provider: &mut P) -> DockSafeDisposition {
    match write_command(provider, &encode_stop()) {
        Ok(()) => DockSafeDisposition::Verified,
        Err(failure) => DockSafeDisposition::Failed(failure),
    }
}

fn refused(
    execution: &PreparedCreateDockExecution,
    refusal: CreateDockExecutionRefusal,
) -> CreateDockExecutionReport {
    report(execution, CreateDockExecutionTerminal::Refused(refusal))
}

fn report(
    execution: &PreparedCreateDockExecution,
    terminal: CreateDockExecutionTerminal,
) -> CreateDockExecutionReport {
    CreateDockExecutionReport {
        host_id: execution.host_id.clone(),
        boot_id: execution.boot_id.clone(),
        offer_generation: execution.offer_generation,
        serial_base_id: execution.serial_base_id.clone(),
        robot_identity: execution.robot_identity.clone(),
        dock_resource_id: execution.dock_resource_id.clone(),
        timer_resource_id: execution.timer_resource_id.clone(),
        kernel_decisions: execution.scheduler.decisions(),
        kernel_signs: execution.scheduler.signs().len(),
        terminal,
    }
}

#[cfg(test)]
#[path = "create_dock_play_tests.rs"]
mod tests;
