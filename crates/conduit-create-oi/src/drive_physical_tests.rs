use super::*;
use crate::{
    ContactWithdrawalSign, IndependentWatchdogObservation, SafetyHazardSet, SafetyInputObservation,
    UartProfile, WithdrawalPreemption,
};
use std::vec::Vec;

struct Provider(Vec<u8>);

impl CreateUartProvider for Provider {
    type Error = ();

    fn is_available(&self) -> bool {
        true
    }

    fn profile(&self) -> UartProfile {
        UartProfile::CREATE_OI
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }

    fn read_byte(&mut self, _: u64) -> Result<Option<u8>, Self::Error> {
        Ok(None)
    }
}

fn safety() -> SafetyObservation {
    SafetyObservation {
        generation: 7,
        latch_generation: 1,
        latched_hazards: SafetyHazardSet::EMPTY,
        observed_at_tick: 100,
        maximum_age_ticks: 10,
        emergency_stop: SafetyInputObservation::Clear,
        wheel_drop: false,
        cliff: false,
        contact: false,
        tilt: SafetyInputObservation::Clear,
        impact: SafetyInputObservation::Clear,
        charging: false,
        control_alive: true,
        body_link_alive: true,
        independent_watchdog: IndependentWatchdogObservation::Healthy,
    }
}

fn authority() -> MotionAuthority<'static> {
    MotionAuthority {
        grant_id: "grant/physical-test",
        valid_until_tick: 1_000,
        safety_class: MotionSafetyAuthority::IndependentWatchdog,
    }
}

fn observation(
    safety: SafetyObservation,
    generation: u32,
    tick: u64,
    left: bool,
    distance_mm: Option<i32>,
) -> PhysicalActuatorObservation {
    PhysicalActuatorObservation {
        safety,
        contact: ContactFrame {
            generation,
            observed_at_tick: tick,
            maximum_age_ticks: 10,
            left,
            right: false,
        },
        distance_mm,
        explicitly_disarmed: false,
        motor_feedback_invalid: false,
    }
}

fn admit_forward(drive: &mut LocalCreateDriveSafety, provider: &mut Provider) {
    assert!(matches!(
        drive.admit_motion(
            provider,
            100,
            Some(authority()),
            safety(),
            DifferentialMotionRequest {
                left_mm_s: 100,
                right_mm_s: 100,
                ttl_ms: 500,
            },
        ),
        DriveSafetySign::MotionAdmitted { .. }
    ));
}

#[test]
fn physical_supervisor_routes_contact_through_the_non_bypassable_reflex() {
    let mut provider = Provider(Vec::new());
    let mut drive = LocalCreateDriveSafety::new();
    admit_forward(&mut drive, &mut provider);
    assert_eq!(
        drive.supervise_physical(
            &mut provider,
            101,
            observation(safety(), 1, 101, false, Some(0))
        ),
        None
    );
    let mut contact_safety = safety();
    contact_safety.generation = 8;
    contact_safety.observed_at_tick = 102;
    contact_safety.contact = true;
    assert!(matches!(
        drive.supervise_physical(
            &mut provider,
            102,
            observation(contact_safety, 2, 102, true, Some(0)),
        ),
        Some(CreateActuatorSupervisionSign::ContactWithdrawal(
            ContactWithdrawalSign::Started {
                preempted_command_generation: 1,
                ..
            }
        ))
    ));
    contact_safety.generation = 9;
    contact_safety.observed_at_tick = 352;
    contact_safety.control_alive = false;
    assert!(matches!(
        drive.supervise_physical(
            &mut provider,
            352,
            observation(contact_safety, 3, 352, true, Some(-10)),
        ),
        Some(CreateActuatorSupervisionSign::ContactWithdrawal(
            ContactWithdrawalSign::Completed { .. }
        ))
    ));
    assert!(provider.0.ends_with(&[145, 0, 0, 0, 0]));
}

#[test]
fn physical_supervisor_preempts_active_withdrawal_on_stronger_truth() {
    let mut provider = Provider(Vec::new());
    let mut drive = LocalCreateDriveSafety::new();
    admit_forward(&mut drive, &mut provider);
    drive.supervise_physical(
        &mut provider,
        101,
        observation(safety(), 1, 101, false, None),
    );
    let mut observed = safety();
    observed.generation = 8;
    observed.observed_at_tick = 102;
    observed.contact = true;
    drive.supervise_physical(
        &mut provider,
        102,
        observation(observed, 2, 102, true, None),
    );
    observed.generation = 9;
    observed.observed_at_tick = 103;
    observed.cliff = true;
    assert!(matches!(
        drive.supervise_physical(
            &mut provider,
            103,
            observation(observed, 3, 103, true, None)
        ),
        Some(CreateActuatorSupervisionSign::ContactWithdrawal(
            ContactWithdrawalSign::Preempted {
                cause: WithdrawalPreemption::Cliff,
                stop_confirmed: true,
                ..
            }
        ))
    ));
}
