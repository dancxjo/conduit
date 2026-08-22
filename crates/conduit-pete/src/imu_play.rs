//! Production-kernel execution for one planned MPU-6050 observation.

use crate::create_observation_kernel_operations::{prepare_scheduler, sink_received, Scheduler};
use crate::{
    validate_mpu6050_plan, Mpu6050Evidence, Mpu6050ExecutionFailure, Mpu6050Realization,
    Mpu6050Snapshot,
};
use conduit_core::{BootId, HostId, OfferGeneration, Plan};
use conduit_kernel::{
    scheduler::{HostOperationRequest, SchedulerStatus},
    BoundedValueRef, HostOperationDisposition, HostOperationOutcome, SignSink,
};
use conduit_mpu6050::Mpu6050I2cProvider;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mpu6050Terminal {
    Completed,
    CancelledBeforeDispatch,
    CancelledAfterDispatch,
    Failed(Mpu6050PlayFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mpu6050PlayFailure {
    DeviceOrDerivation(Mpu6050ExecutionFailure),
    KernelRefused,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mpu6050ExecutionReport {
    pub terminal: Mpu6050Terminal,
    pub plan_id: conduit_core::PlanId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub i2c_base_id: String,
    pub attachment_id: String,
    pub body_frame_id: String,
    pub mounting_id: String,
    pub raw: Option<conduit_mpu6050::RawImuSample>,
    pub orientation: Option<conduit_core::OrientationObservation>,
    pub calibration_generation: Option<u64>,
    pub tilt_active: Option<bool>,
    pub impact_active: Option<bool>,
    pub kernel_decisions: u32,
    pub kernel_signs: u16,
    pub proof_class: &'static str,
    pub physical_hil_claimed: bool,
}

#[derive(Clone, Copy)]
struct PendingCompletion {
    request: HostOperationRequest,
    output: BoundedValueRef,
    canonical: [u8; conduit_core::ROBOTICS_ORIENTATION_ENCODED_LEN],
}

pub struct PreparedMpu6050Execution {
    scheduler: Scheduler,
    realization: Mpu6050Realization,
    plan_id: conduit_core::PlanId,
    evidence: Mpu6050Evidence,
    dispatched: bool,
    pending: Option<PendingCompletion>,
    snapshot: Option<Mpu6050Snapshot>,
}

pub fn prepare_mpu6050_execution(
    plan: &Plan,
    evidence: &Mpu6050Evidence,
) -> Result<PreparedMpu6050Execution, &'static str> {
    validate_mpu6050_plan(plan, evidence)?;
    Ok(PreparedMpu6050Execution {
        scheduler: prepare_scheduler(conduit_core::ROBOTICS_ORIENTATION_ENCODED_LEN as u32)?,
        realization: Mpu6050Realization::new(evidence).map_err(|_| "invalid MPU-6050 evidence")?,
        plan_id: plan.plan_id.clone(),
        evidence: evidence.clone(),
        dispatched: false,
        pending: None,
        snapshot: None,
    })
}

pub fn run_mpu6050_execution<P: Mpu6050I2cProvider>(
    execution: &mut PreparedMpu6050Execution,
    provider: &mut P,
    observed_at_tick: u64,
    now_tick: u64,
) -> Mpu6050ExecutionReport {
    if let Err(failure) =
        dispatch_mpu6050_execution(execution, provider, observed_at_tick, now_tick)
    {
        return report(execution, Mpu6050Terminal::Failed(failure), None);
    }
    finish_mpu6050_execution(execution)
}

pub fn dispatch_mpu6050_execution<P: Mpu6050I2cProvider>(
    execution: &mut PreparedMpu6050Execution,
    provider: &mut P,
    observed_at_tick: u64,
    now_tick: u64,
) -> Result<(), Mpu6050PlayFailure> {
    if execution.dispatched || execution.pending.is_some() {
        return Err(Mpu6050PlayFailure::KernelRefused);
    }
    let request = next_request(execution)?;
    execution.dispatched = true;
    let snapshot = match execution.realization.observe(
        &execution.evidence,
        provider,
        observed_at_tick,
        now_tick,
    ) {
        Ok(snapshot) => snapshot,
        Err(failure) => {
            let play_failure = Mpu6050PlayFailure::DeviceOrDerivation(failure);
            fail_request(execution, request, play_failure);
            return Err(play_failure);
        }
    };
    let canonical = snapshot.orientation.encode();
    let value = execution
        .scheduler
        .store_host_value(&canonical)
        .map_err(|_| Mpu6050PlayFailure::KernelRefused)?;
    let output = BoundedValueRef::new(value, canonical.len() as u32)
        .expect("stored orientation has exact bounded length");
    execution.pending = Some(PendingCompletion {
        request,
        output,
        canonical,
    });
    execution.snapshot = Some(snapshot);
    Ok(())
}

pub fn finish_mpu6050_execution(
    execution: &mut PreparedMpu6050Execution,
) -> Mpu6050ExecutionReport {
    let Some(pending) = execution.pending.take() else {
        return report(
            execution,
            Mpu6050Terminal::Failed(Mpu6050PlayFailure::KernelRefused),
            None,
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
            Mpu6050Terminal::Failed(Mpu6050PlayFailure::KernelRefused),
            None,
        );
    }
    for _ in 0..32 {
        match execution.scheduler.step() {
            Ok(SchedulerStatus::Complete) if sink_received(&execution.scheduler) => {
                return report(
                    execution,
                    Mpu6050Terminal::Completed,
                    Some(pending.canonical),
                );
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    report(
        execution,
        Mpu6050Terminal::Failed(Mpu6050PlayFailure::KernelRefused),
        None,
    )
}

pub fn cancel_mpu6050_execution(
    execution: &mut PreparedMpu6050Execution,
) -> Mpu6050ExecutionReport {
    let terminal = if execution.dispatched {
        Mpu6050Terminal::CancelledAfterDispatch
    } else {
        Mpu6050Terminal::CancelledBeforeDispatch
    };
    execution.pending = None;
    let _ = execution.scheduler.cancel();
    report(execution, terminal, None)
}

fn next_request(
    execution: &mut PreparedMpu6050Execution,
) -> Result<HostOperationRequest, Mpu6050PlayFailure> {
    for _ in 0..16 {
        execution
            .scheduler
            .step()
            .map_err(|_| Mpu6050PlayFailure::KernelRefused)?;
        if let Some(request) = execution.scheduler.next_host_request() {
            if execution
                .scheduler
                .host_value(request.input.value)
                .is_ok_and(|value| value.is_empty())
            {
                return Ok(request);
            }
            return Err(Mpu6050PlayFailure::KernelRefused);
        }
    }
    Err(Mpu6050PlayFailure::KernelRefused)
}

fn fail_request(
    execution: &mut PreparedMpu6050Execution,
    request: HostOperationRequest,
    failure: Mpu6050PlayFailure,
) {
    let detail = match failure {
        Mpu6050PlayFailure::DeviceOrDerivation(_) => 1,
        Mpu6050PlayFailure::KernelRefused => 2,
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

fn report(
    execution: &PreparedMpu6050Execution,
    terminal: Mpu6050Terminal,
    canonical: Option<[u8; conduit_core::ROBOTICS_ORIENTATION_ENCODED_LEN]>,
) -> Mpu6050ExecutionReport {
    let snapshot = canonical.and(execution.snapshot.as_ref());
    Mpu6050ExecutionReport {
        terminal,
        plan_id: execution.plan_id.clone(),
        host_id: execution.evidence.host_id.clone(),
        boot_id: execution.evidence.boot_id.clone(),
        offer_generation: execution.evidence.offer_generation,
        i2c_base_id: execution.evidence.i2c_base_id.clone(),
        attachment_id: execution.evidence.attachment_id.clone(),
        body_frame_id: execution.evidence.body_frame_id.clone(),
        mounting_id: execution.evidence.mounting_id.clone(),
        raw: snapshot.map(|value| value.raw),
        orientation: snapshot.map(|value| value.orientation),
        calibration_generation: snapshot.map(|value| value.derived.calibration_generation),
        tilt_active: snapshot.map(|value| value.derived.tilt_active),
        impact_active: snapshot.map(|value| value.derived.impact_active),
        kernel_decisions: execution.scheduler.decisions(),
        kernel_signs: execution.scheduler.signs().len(),
        proof_class: "deterministic-production-kernel-shape",
        physical_hil_claimed: false,
    }
}

#[cfg(test)]
#[path = "imu_play_tests.rs"]
mod tests;
