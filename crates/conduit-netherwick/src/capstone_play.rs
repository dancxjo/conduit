//! Deterministic execution receipt for the exact sealed capstone Plan.

use crate::capstone_kernel::{
    prepare_scheduler, CapstoneScheduler, DRIVE_NODE, OBSERVATION_NODE, OBSERVATION_REQUEST,
};
use crate::{capstone_plan, CapstoneHostEvidence};
use conduit_core::{ConfigurationValue, Plan, Scalar};
use conduit_kernel::{
    scheduler::{HostOperationRequest, SchedulerStatus},
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationOutcome,
    SignSink, ValueStorage,
};

pub struct PreparedCapstoneExecution {
    scheduler: CapstoneScheduler,
    pub plan_id: String,
    pub host_id: String,
    pub boot_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapstoneExecutionRefusal {
    StaleOrUnavailableEvidence,
    WrongPlanIdentity,
    InvalidConfiguration,
    KernelPreparation,
    KernelLifecycle,
    ObservationProviderLost,
    DriveProviderLost,
    AuthorityLost,
    Pressure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapstoneDeterministicFault {
    None,
    ObservationProviderLost,
    DriveProviderLost,
    AuthorityLost,
    Pressure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapstoneExecutionReceipt {
    pub plan_id: String,
    pub host_id: String,
    pub boot_id: String,
    pub bump: bool,
    pub selected_linear_microunits: i64,
    pub angular_microunits: i64,
    pub ttl_ms: u64,
    pub kernel_decisions: u32,
    pub kernel_signs: u16,
    pub proof_class: &'static str,
    pub physical_motion_claimed: bool,
}

pub fn prepare_capstone_execution(
    plan: &Plan,
    evidence: &CapstoneHostEvidence,
    now_tick: u64,
) -> Result<PreparedCapstoneExecution, CapstoneExecutionRefusal> {
    let expected = capstone_plan(evidence, now_tick)
        .map_err(|_| CapstoneExecutionRefusal::StaleOrUnavailableEvidence)?;
    if !conduit_core::verify_plan(plan) || plan.plan_id != expected.plan_id {
        return Err(CapstoneExecutionRefusal::WrongPlanIdentity);
    }
    let fragment = plan
        .fragments
        .first()
        .ok_or(CapstoneExecutionRefusal::WrongPlanIdentity)?;
    let requested = placement(fragment, "requested")?;
    let stopped = placement(fragment, "stopped")?;
    let requested_linear = scalar_configuration(requested, "linear-microunits")?.encode();
    let requested_angular = scalar_configuration(requested, "angular-microunits")?.encode();
    let stopped_linear = scalar_configuration(stopped, "linear-microunits")?.encode();
    let scheduler = prepare_scheduler(&requested_linear, &requested_angular, &stopped_linear)
        .map_err(|_| CapstoneExecutionRefusal::KernelPreparation)?;
    Ok(PreparedCapstoneExecution {
        scheduler,
        plan_id: plan.plan_id.as_str().to_string(),
        host_id: fragment.host_id.as_str().to_string(),
        boot_id: fragment.boot_id.as_str().to_string(),
    })
}

pub fn run_capstone_deterministic_vector(
    plan: &Plan,
    evidence: &CapstoneHostEvidence,
    now_tick: u64,
    bump: bool,
    fault: CapstoneDeterministicFault,
) -> Result<CapstoneExecutionReceipt, CapstoneExecutionRefusal> {
    let mut execution = prepare_capstone_execution(plan, evidence, now_tick)?;
    let observation = next_request(&mut execution.scheduler)?;
    if observation.node != OBSERVATION_NODE || observation.request != OBSERVATION_REQUEST {
        return Err(CapstoneExecutionRefusal::KernelLifecycle);
    }
    match fault {
        CapstoneDeterministicFault::ObservationProviderLost => {
            fail_request(&mut execution.scheduler, observation, 1)?;
            return Err(CapstoneExecutionRefusal::ObservationProviderLost);
        }
        CapstoneDeterministicFault::Pressure => {
            let oversized = [0_u8; 2];
            let value = execution
                .scheduler
                .store_host_value(&oversized)
                .map_err(|_| CapstoneExecutionRefusal::Pressure)?;
            let outcome = HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(value, 2)
                        .map_err(|_| CapstoneExecutionRefusal::Pressure)?,
                ),
                failure: None,
            };
            execution
                .scheduler
                .complete_host_operation(observation.node, observation.request, outcome)
                .map_err(|_| CapstoneExecutionRefusal::Pressure)?;
            return Err(CapstoneExecutionRefusal::Pressure);
        }
        _ => {}
    }
    let observation_value = execution
        .scheduler
        .store_host_value(&conduit_core::InfoBool::new(bump).encode())
        .map_err(|_| CapstoneExecutionRefusal::Pressure)?;
    execution
        .scheduler
        .complete_host_operation(
            observation.node,
            observation.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(observation_value, conduit_core::BOOL_ENCODED_LEN as u32)
                        .map_err(|_| CapstoneExecutionRefusal::KernelLifecycle)?,
                ),
                failure: None,
            },
        )
        .map_err(|_| CapstoneExecutionRefusal::KernelLifecycle)?;

    let drive = next_request(&mut execution.scheduler)?;
    if drive.node != DRIVE_NODE {
        return Err(CapstoneExecutionRefusal::KernelLifecycle);
    }
    let selected = Scalar::decode(
        execution
            .scheduler
            .values()
            .get(drive.input.value)
            .map_err(|_| CapstoneExecutionRefusal::KernelLifecycle)?,
    )
    .map_err(|_| CapstoneExecutionRefusal::KernelLifecycle)?;
    match fault {
        CapstoneDeterministicFault::DriveProviderLost => {
            fail_request(&mut execution.scheduler, drive, 2)?;
            return Err(CapstoneExecutionRefusal::DriveProviderLost);
        }
        CapstoneDeterministicFault::AuthorityLost => {
            execution
                .scheduler
                .cancel()
                .map_err(|_| CapstoneExecutionRefusal::KernelLifecycle)?;
            return Err(CapstoneExecutionRefusal::AuthorityLost);
        }
        _ => {}
    }
    execution
        .scheduler
        .complete_host_operation(
            drive.node,
            drive.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .map_err(|_| CapstoneExecutionRefusal::KernelLifecycle)?;
    execution
        .scheduler
        .run(128)
        .map_err(|_| CapstoneExecutionRefusal::KernelLifecycle)?;

    Ok(CapstoneExecutionReceipt {
        plan_id: execution.plan_id,
        host_id: execution.host_id,
        boot_id: execution.boot_id,
        bump,
        selected_linear_microunits: selected.raw_microunits(),
        angular_microunits: 0,
        ttl_ms: drive_ttl(plan)?,
        kernel_decisions: execution.scheduler.decisions(),
        kernel_signs: execution.scheduler.signs().len(),
        proof_class: "deterministic-production-kernel-shape",
        physical_motion_claimed: false,
    })
}

fn next_request(
    scheduler: &mut CapstoneScheduler,
) -> Result<HostOperationRequest, CapstoneExecutionRefusal> {
    for _ in 0..128 {
        if let Some(request) = scheduler.next_host_request() {
            return Ok(request);
        }
        match scheduler
            .step()
            .map_err(|_| CapstoneExecutionRefusal::KernelLifecycle)?
        {
            SchedulerStatus::Progress { .. } => {}
            _ => return Err(CapstoneExecutionRefusal::KernelLifecycle),
        }
    }
    Err(CapstoneExecutionRefusal::KernelLifecycle)
}

fn fail_request(
    scheduler: &mut CapstoneScheduler,
    request: HostOperationRequest,
    detail: u16,
) -> Result<(), CapstoneExecutionRefusal> {
    scheduler
        .complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Failed,
                output: None,
                failure: Some(Failure {
                    code: FailureCode::HostOperationFailed,
                    detail,
                }),
            },
        )
        .map_err(|_| CapstoneExecutionRefusal::KernelLifecycle)
}

fn placement<'a>(
    fragment: &'a conduit_core::PlanFragment,
    suffix: &str,
) -> Result<&'a conduit_core::PlannedGear, CapstoneExecutionRefusal> {
    fragment
        .placements
        .iter()
        .find(|placement| placement.gear_id.as_str().ends_with(suffix))
        .ok_or(CapstoneExecutionRefusal::WrongPlanIdentity)
}

fn scalar_configuration(
    placement: &conduit_core::PlannedGear,
    key: &str,
) -> Result<Scalar, CapstoneExecutionRefusal> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (found, ConfigurationValue::I64(value)) if found == key => {
                Some(Scalar::from_raw_microunits(*value))
            }
            _ => None,
        })
        .ok_or(CapstoneExecutionRefusal::InvalidConfiguration)
}

fn drive_ttl(plan: &Plan) -> Result<u64, CapstoneExecutionRefusal> {
    let fragment = plan
        .fragments
        .first()
        .ok_or(CapstoneExecutionRefusal::WrongPlanIdentity)?;
    placement(fragment, "drive")?
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("ttl-ms", ConfigurationValue::U64(value)) => Some(*value),
            _ => None,
        })
        .ok_or(CapstoneExecutionRefusal::InvalidConfiguration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CapstoneHostClass, CreateDriveObservation, CreateObservationEvidence,
        IndependentWatchdogObservation, OiMode, SafetyInputObservation, SafetyObservation,
    };
    use conduit_core::{BootId, HostId, OfferGeneration};

    fn evidence() -> CapstoneHostEvidence {
        let host_id = HostId::from("host/std-capstone");
        let boot_id = BootId::from("boot/std-capstone");
        CapstoneHostEvidence {
            class: CapstoneHostClass::Std,
            observation: CreateObservationEvidence {
                host_id: host_id.clone(),
                boot_id: boot_id.clone(),
                offer_generation: OfferGeneration(7),
                serial_base_id: "base/std-prolific".into(),
                robot_identity: "device/create1".into(),
                session_resource_id: "session/std-observation".into(),
                mode: OiMode::Safe,
                observed_at_tick: 100,
                maximum_age_ticks: 10,
            },
            drive: CreateDriveObservation {
                host_id,
                boot_id,
                offer_generation: OfferGeneration(7),
                serial_base_id: "base/std-prolific".into(),
                robot_identity: "device/create1".into(),
                drive_resource_id: "session/std-drive".into(),
                mode: OiMode::Safe,
                safety: SafetyObservation {
                    generation: 3,
                    latch_generation: 1,
                    latched_hazards: crate::SafetyHazardSet::EMPTY,
                    observed_at_tick: 100,
                    maximum_age_ticks: 10,
                    emergency_stop: SafetyInputObservation::Unavailable,
                    wheel_drop: false,
                    cliff: false,
                    contact: false,
                    tilt: SafetyInputObservation::Unavailable,
                    impact: SafetyInputObservation::Unavailable,
                    charging: false,
                    control_alive: true,
                    body_link_alive: true,
                    independent_watchdog: IndependentWatchdogObservation::Absent,
                },
            },
            serialized_client_pool_id: "session/std-serialized-provider".into(),
            watchdog_pool_id: None,
            translator_pool_id: None,
        }
    }

    #[test]
    fn clear_and_contact_vectors_reach_motion_and_zero_through_one_kernel() {
        let evidence = evidence();
        let plan = capstone_plan(&evidence, 105).unwrap();
        let clear = run_capstone_deterministic_vector(
            &plan,
            &evidence,
            105,
            false,
            CapstoneDeterministicFault::None,
        )
        .unwrap();
        let contact = run_capstone_deterministic_vector(
            &plan,
            &evidence,
            105,
            true,
            CapstoneDeterministicFault::None,
        )
        .unwrap();
        assert_eq!(clear.selected_linear_microunits, 100_000);
        assert_eq!(contact.selected_linear_microunits, 0);
        assert_eq!(clear.angular_microunits, 0);
        assert_eq!(contact.angular_microunits, 0);
        assert_eq!(clear.plan_id, contact.plan_id);
        assert_eq!(clear.ttl_ms, 250);
        assert!(!clear.physical_motion_claimed);
        assert!(!contact.physical_motion_claimed);
        assert!(clear.kernel_decisions > 0 && clear.kernel_signs > 0);

        let mut pico = evidence.clone();
        pico.class = CapstoneHostClass::PicoW;
        pico.observation.host_id = HostId::from("host/pico-capstone");
        pico.observation.boot_id = BootId::from("boot/pico-capstone");
        pico.observation.serial_base_id = "base/pico-uart0".into();
        pico.observation.session_resource_id = "session/pico-observation".into();
        pico.drive.host_id = pico.observation.host_id.clone();
        pico.drive.boot_id = pico.observation.boot_id.clone();
        pico.drive.serial_base_id = pico.observation.serial_base_id.clone();
        pico.drive.drive_resource_id = "session/pico-drive".into();
        pico.drive.safety.emergency_stop = SafetyInputObservation::Clear;
        pico.drive.safety.tilt = SafetyInputObservation::Clear;
        pico.drive.safety.impact = SafetyInputObservation::Clear;
        pico.drive.safety.independent_watchdog = IndependentWatchdogObservation::Healthy;
        pico.serialized_client_pool_id = "session/pico-serialized-provider".into();
        pico.watchdog_pool_id = Some("base/pico-watchdog".into());
        pico.translator_pool_id = Some("attachment/pico-level-translator".into());
        let pico_plan = capstone_plan(&pico, 105).unwrap();
        let pico_clear = run_capstone_deterministic_vector(
            &pico_plan,
            &pico,
            105,
            false,
            CapstoneDeterministicFault::None,
        )
        .unwrap();
        assert_ne!(clear.plan_id, pico_clear.plan_id);
        assert_ne!(clear.host_id, pico_clear.host_id);
        assert_eq!(
            (
                clear.selected_linear_microunits,
                clear.angular_microunits,
                clear.ttl_ms,
                clear.proof_class,
                clear.physical_motion_claimed,
            ),
            (
                pico_clear.selected_linear_microunits,
                pico_clear.angular_microunits,
                pico_clear.ttl_ms,
                pico_clear.proof_class,
                pico_clear.physical_motion_claimed,
            )
        );
    }

    #[test]
    fn provider_authority_pressure_stale_and_plan_failures_remain_distinct() {
        let evidence = evidence();
        let plan = capstone_plan(&evidence, 105).unwrap();
        for (fault, expected) in [
            (
                CapstoneDeterministicFault::ObservationProviderLost,
                CapstoneExecutionRefusal::ObservationProviderLost,
            ),
            (
                CapstoneDeterministicFault::DriveProviderLost,
                CapstoneExecutionRefusal::DriveProviderLost,
            ),
            (
                CapstoneDeterministicFault::AuthorityLost,
                CapstoneExecutionRefusal::AuthorityLost,
            ),
            (
                CapstoneDeterministicFault::Pressure,
                CapstoneExecutionRefusal::Pressure,
            ),
        ] {
            assert_eq!(
                run_capstone_deterministic_vector(&plan, &evidence, 105, false, fault),
                Err(expected)
            );
        }

        let mut stale = evidence.clone();
        stale.observation.observed_at_tick = 90;
        stale.drive.safety.observed_at_tick = 90;
        assert!(matches!(
            prepare_capstone_execution(&plan, &stale, 105),
            Err(CapstoneExecutionRefusal::StaleOrUnavailableEvidence)
        ));
        let mut wrong = plan.clone();
        wrong.fragments[0].boot_id = BootId::from("boot/wrong");
        assert!(matches!(
            prepare_capstone_execution(&wrong, &evidence, 105),
            Err(CapstoneExecutionRefusal::WrongPlanIdentity)
        ));
    }
}
