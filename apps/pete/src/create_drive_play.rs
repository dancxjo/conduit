//! Production-kernel execution of the planned Create differential drive.

use crate::create_drive_kernel_operations::{
    drive_is_admitted, prepare_drive_scheduler, DriveScheduler, DRIVE_NODE, DRIVE_REQUEST,
    REQUEST_BYTES,
};
use crate::{
    lower_create_drive_intent, CreateDriveLoweringRefusal, CreateDriveObservation,
    CreateUartProvider, DriveRefusal, DriveSafetySign, LocalCreateDriveSafety, MotionAuthority,
    SafeDispositionCause, SafetyObservation,
};
use conduit_core::{BootId, HostId, OfferGeneration, Plan, Scalar, SCALAR_ENCODED_LEN};
use conduit_kernel::{
    scheduler::HostOperationRequest, HostOperationDisposition, HostOperationOutcome, SignSink,
};

pub struct PreparedCreateDriveExecution {
    scheduler: DriveScheduler,
    safety: LocalCreateDriveSafety,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub serial_base_id: String,
    pub robot_identity: String,
    pub drive_resource_id: String,
    pub planned_authority_grant_id: String,
    pub ttl_ms: u32,
    advertised_safety_generation: u32,
    dispatched: bool,
    active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateDriveExecutionRefusal {
    KernelRefused,
    MalformedScalarInput,
    Lowering(CreateDriveLoweringRefusal),
    AuthorityGrantMismatch,
    SafetyGenerationRegressed,
    Drive(DriveRefusal),
    InvalidLifecycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateDriveExecutionTerminal {
    MotionAdmitted {
        authority_grant_id: String,
        safety_generation: u32,
        deadline_tick: u64,
        left_mm_s: i16,
        right_mm_s: i16,
    },
    Active,
    SafeDisposition {
        cause: SafeDispositionCause,
        safety_generation: u32,
    },
    KernelRefusedAfterMotion {
        stop_cause: SafeDispositionCause,
        safety_generation: u32,
    },
    CancelledBeforeDispatch,
    Refused(CreateDriveExecutionRefusal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateDriveExecutionReport {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub serial_base_id: String,
    pub robot_identity: String,
    pub drive_resource_id: String,
    pub kernel_decisions: u32,
    pub kernel_signs: u16,
    pub terminal: CreateDriveExecutionTerminal,
}

pub fn prepare_create_drive_execution(
    plan: &Plan,
    evidence: &CreateDriveObservation,
    linear: Scalar,
    angular: Scalar,
) -> Result<PreparedCreateDriveExecution, &'static str> {
    let validated =
        crate::create_drive_plan_validation::validate_create_drive_plan(plan, evidence)?;
    let scheduler = prepare_drive_scheduler(&linear.encode(), &angular.encode())?;
    Ok(PreparedCreateDriveExecution {
        scheduler,
        safety: LocalCreateDriveSafety::new(),
        host_id: evidence.host_id.clone(),
        boot_id: evidence.boot_id.clone(),
        offer_generation: evidence.offer_generation,
        serial_base_id: evidence.serial_base_id.clone(),
        robot_identity: evidence.robot_identity.clone(),
        drive_resource_id: evidence.drive_resource_id.clone(),
        planned_authority_grant_id: validated.authority_grant_id,
        ttl_ms: validated.ttl_ms,
        advertised_safety_generation: evidence.safety.generation,
        dispatched: false,
        active: false,
    })
}

pub fn dispatch_create_drive_execution<'a, P: CreateUartProvider>(
    execution: &mut PreparedCreateDriveExecution,
    provider: &mut P,
    now_tick: u64,
    authority: Option<MotionAuthority<'a>>,
    safety: SafetyObservation,
) -> CreateDriveExecutionReport {
    if execution.dispatched || execution.active {
        return report(
            execution,
            CreateDriveExecutionTerminal::Refused(CreateDriveExecutionRefusal::InvalidLifecycle),
        );
    }
    let request = match next_request(execution) {
        Ok(request) => request,
        Err(refusal) => return report(execution, CreateDriveExecutionTerminal::Refused(refusal)),
    };
    execution.dispatched = true;
    let (linear, angular) = match decode_request(execution, request) {
        Ok(values) => values,
        Err(refusal) => {
            fail_request(execution, request, refusal);
            return report(execution, CreateDriveExecutionTerminal::Refused(refusal));
        }
    };
    let Some(authority) = authority else {
        let refusal = CreateDriveExecutionRefusal::Drive(DriveRefusal::MissingAuthority);
        fail_request(execution, request, refusal);
        return report(execution, CreateDriveExecutionTerminal::Refused(refusal));
    };
    if authority.grant_id != execution.planned_authority_grant_id {
        let refusal = CreateDriveExecutionRefusal::AuthorityGrantMismatch;
        fail_request(execution, request, refusal);
        return report(execution, CreateDriveExecutionTerminal::Refused(refusal));
    }
    if safety.generation < execution.advertised_safety_generation {
        let refusal = CreateDriveExecutionRefusal::SafetyGenerationRegressed;
        fail_request(execution, request, refusal);
        return report(execution, CreateDriveExecutionTerminal::Refused(refusal));
    }
    let motion = match lower_create_drive_intent(linear, angular, execution.ttl_ms) {
        Ok(motion) => motion,
        Err(lowering) => {
            let refusal = CreateDriveExecutionRefusal::Lowering(lowering);
            fail_request(execution, request, refusal);
            return report(execution, CreateDriveExecutionTerminal::Refused(refusal));
        }
    };
    let admitted =
        execution
            .safety
            .admit_motion(provider, now_tick, Some(authority), safety, motion);
    let DriveSafetySign::MotionAdmitted {
        authority_grant_id,
        safety_generation,
        deadline_tick,
    } = admitted
    else {
        let refusal = match admitted {
            DriveSafetySign::Refused(refusal) => CreateDriveExecutionRefusal::Drive(refusal),
            DriveSafetySign::SafeDisposition { .. } => {
                CreateDriveExecutionRefusal::InvalidLifecycle
            }
            DriveSafetySign::MotionAdmitted { .. } => unreachable!(),
        };
        fail_request(execution, request, refusal);
        return report(execution, CreateDriveExecutionTerminal::Refused(refusal));
    };
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
        let stop = execution.safety.stop(provider, safety_generation);
        let terminal = match stop {
            DriveSafetySign::SafeDisposition {
                cause,
                safety_generation,
            } => CreateDriveExecutionTerminal::KernelRefusedAfterMotion {
                stop_cause: cause,
                safety_generation,
            },
            _ => CreateDriveExecutionTerminal::Refused(CreateDriveExecutionRefusal::KernelRefused),
        };
        return report(execution, terminal);
    }
    execution.active = true;
    report(
        execution,
        CreateDriveExecutionTerminal::MotionAdmitted {
            authority_grant_id: authority_grant_id.to_string(),
            safety_generation,
            deadline_tick,
            left_mm_s: motion.left_mm_s,
            right_mm_s: motion.right_mm_s,
        },
    )
}

pub fn supervise_create_drive_execution<P: CreateUartProvider>(
    execution: &mut PreparedCreateDriveExecution,
    provider: &mut P,
    now_tick: u64,
    safety: SafetyObservation,
) -> CreateDriveExecutionReport {
    if !execution.active {
        return report(
            execution,
            CreateDriveExecutionTerminal::Refused(CreateDriveExecutionRefusal::InvalidLifecycle),
        );
    }
    let Some(sign) = execution.safety.supervise(provider, now_tick, safety) else {
        return report(execution, CreateDriveExecutionTerminal::Active);
    };
    execution.active = false;
    let _ = execution.scheduler.cancel();
    report(execution, terminal_from_safety(sign))
}

pub fn cancel_create_drive_execution<P: CreateUartProvider>(
    execution: &mut PreparedCreateDriveExecution,
    provider: &mut P,
    safety_generation: u32,
) -> CreateDriveExecutionReport {
    if !execution.dispatched {
        let _ = execution.scheduler.cancel();
        return report(
            execution,
            CreateDriveExecutionTerminal::CancelledBeforeDispatch,
        );
    }
    if !execution.active {
        return report(
            execution,
            CreateDriveExecutionTerminal::Refused(CreateDriveExecutionRefusal::InvalidLifecycle),
        );
    }
    execution.active = false;
    let sign = execution.safety.stop(provider, safety_generation);
    let _ = execution.scheduler.cancel();
    report(execution, terminal_from_safety(sign))
}

fn next_request(
    execution: &mut PreparedCreateDriveExecution,
) -> Result<HostOperationRequest, CreateDriveExecutionRefusal> {
    for _ in 0..32 {
        execution
            .scheduler
            .step()
            .map_err(|_| CreateDriveExecutionRefusal::KernelRefused)?;
        if let Some(request) = execution.scheduler.next_host_request() {
            return Ok(request);
        }
    }
    Err(CreateDriveExecutionRefusal::KernelRefused)
}

fn decode_request(
    execution: &PreparedCreateDriveExecution,
    request: HostOperationRequest,
) -> Result<(Scalar, Scalar), CreateDriveExecutionRefusal> {
    if request.node != DRIVE_NODE
        || request.request != DRIVE_REQUEST
        || request.input.admitted_bytes != REQUEST_BYTES
    {
        return Err(CreateDriveExecutionRefusal::KernelRefused);
    }
    let bytes = execution
        .scheduler
        .host_value(request.input.value)
        .map_err(|_| CreateDriveExecutionRefusal::KernelRefused)?;
    if bytes.len() != REQUEST_BYTES as usize {
        return Err(CreateDriveExecutionRefusal::MalformedScalarInput);
    }
    let linear = Scalar::decode(&bytes[..SCALAR_ENCODED_LEN])
        .map_err(|_| CreateDriveExecutionRefusal::MalformedScalarInput)?;
    let angular = Scalar::decode(&bytes[SCALAR_ENCODED_LEN..])
        .map_err(|_| CreateDriveExecutionRefusal::MalformedScalarInput)?;
    Ok((linear, angular))
}

fn settle_admission(execution: &mut PreparedCreateDriveExecution) -> bool {
    for _ in 0..16 {
        if execution.scheduler.step().is_err() {
            return false;
        }
        if drive_is_admitted(&execution.scheduler) {
            return true;
        }
    }
    false
}

fn fail_request(
    execution: &mut PreparedCreateDriveExecution,
    request: HostOperationRequest,
    refusal: CreateDriveExecutionRefusal,
) {
    let detail = match refusal {
        CreateDriveExecutionRefusal::MalformedScalarInput => 1,
        CreateDriveExecutionRefusal::Lowering(_) => 2,
        CreateDriveExecutionRefusal::AuthorityGrantMismatch => 3,
        CreateDriveExecutionRefusal::SafetyGenerationRegressed => 4,
        CreateDriveExecutionRefusal::Drive(_) => 5,
        CreateDriveExecutionRefusal::KernelRefused
        | CreateDriveExecutionRefusal::InvalidLifecycle => 6,
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

fn terminal_from_safety(sign: DriveSafetySign<'_>) -> CreateDriveExecutionTerminal {
    match sign {
        DriveSafetySign::SafeDisposition {
            cause,
            safety_generation,
        } => CreateDriveExecutionTerminal::SafeDisposition {
            cause,
            safety_generation,
        },
        DriveSafetySign::Refused(refusal) => {
            CreateDriveExecutionTerminal::Refused(CreateDriveExecutionRefusal::Drive(refusal))
        }
        DriveSafetySign::MotionAdmitted { .. } => {
            CreateDriveExecutionTerminal::Refused(CreateDriveExecutionRefusal::InvalidLifecycle)
        }
    }
}

fn report(
    execution: &PreparedCreateDriveExecution,
    terminal: CreateDriveExecutionTerminal,
) -> CreateDriveExecutionReport {
    CreateDriveExecutionReport {
        host_id: execution.host_id.clone(),
        boot_id: execution.boot_id.clone(),
        offer_generation: execution.offer_generation,
        serial_base_id: execution.serial_base_id.clone(),
        robot_identity: execution.robot_identity.clone(),
        drive_resource_id: execution.drive_resource_id.clone(),
        kernel_decisions: execution.scheduler.decisions(),
        kernel_signs: execution.scheduler.signs().len(),
        terminal,
    }
}

#[cfg(test)]
#[path = "create_drive_play_tests.rs"]
mod tests;
