use super::*;
use crate::{IndependentWatchdogObservation, SafetyInputObservation, UartParity, UartProfile};
use std::vec;
use std::vec::Vec;

struct Provider {
    available: bool,
    bytes: Vec<u8>,
}
impl CreateUartProvider for Provider {
    type Error = ();

    fn is_available(&self) -> bool {
        self.available
    }
    fn profile(&self) -> UartProfile {
        UartProfile {
            baud: 57_600,
            data_bits: 8,
            stop_bits: 1,
            parity: UartParity::None,
        }
    }
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn read_byte(&mut self, _: u64) -> Result<Option<u8>, ()> {
        Ok(None)
    }
}
fn safe() -> SafetyObservation {
    SafetyObservation {
        generation: 7,
        latch_generation: 1,
        latched_hazards: crate::SafetyHazardSet::EMPTY,
        observed_at_tick: 100,
        maximum_age_ticks: 20,
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
        grant_id: "grant/drive",
        valid_until_tick: 1_000,
        safety_class: MotionSafetyAuthority::IndependentWatchdog,
    }
}

#[test]
fn authority_and_fresh_safety_are_mandatory_before_any_motion_byte() {
    let mut provider = Provider {
        available: true,
        bytes: vec![],
    };
    let mut drive = LocalCreateDriveSafety::new();
    let request = DifferentialMotionRequest {
        left_mm_s: 20,
        right_mm_s: 20,
        ttl_ms: 100,
    };
    assert_eq!(
        drive.admit_motion(&mut provider, 100, None, safe(), request),
        DriveSafetySign::Refused(DriveRefusal::MissingAuthority)
    );
    assert!(provider.bytes.is_empty());
    let mut stale = safe();
    stale.observed_at_tick = 0;
    assert!(matches!(
        drive.admit_motion(&mut provider, 100, Some(authority()), stale, request),
        DriveSafetySign::Refused(DriveRefusal::SafetyStaleOrInhibited(
            LocalHazard::BodyLinkLost
        ))
    ));
    assert!(provider.bytes.is_empty());
}

#[test]
fn watchdog_absence_requires_an_exact_reduced_safety_authority() {
    let mut provider = Provider {
        available: true,
        bytes: vec![],
    };
    let mut drive = LocalCreateDriveSafety::new();
    let request = DifferentialMotionRequest {
        left_mm_s: 20,
        right_mm_s: 20,
        ttl_ms: 100,
    };
    let absent = SafetyObservation {
        independent_watchdog: IndependentWatchdogObservation::Absent,
        ..safe()
    };
    assert_eq!(
        drive.admit_motion(&mut provider, 100, Some(authority()), absent, request),
        DriveSafetySign::Refused(DriveRefusal::SafetyAuthorityMismatch)
    );
    assert!(provider.bytes.is_empty());

    let reduced = MotionAuthority {
        grant_id: "grant/reduced-drive",
        valid_until_tick: 1_000,
        safety_class: MotionSafetyAuthority::ReducedWheelsOffFloor,
    };
    assert!(matches!(
        drive.admit_motion(&mut provider, 100, Some(reduced), absent, request),
        DriveSafetySign::MotionAdmitted { .. }
    ));
    assert_eq!(provider.bytes, [145, 0, 20, 0, 20]);
}

#[test]
fn failed_watchdog_is_a_hazard_even_with_reduced_authority() {
    let mut provider = Provider {
        available: true,
        bytes: vec![],
    };
    let failed = SafetyObservation {
        independent_watchdog: IndependentWatchdogObservation::Failed,
        ..safe()
    };
    let reduced = MotionAuthority {
        grant_id: "grant/reduced-drive",
        valid_until_tick: 1_000,
        safety_class: MotionSafetyAuthority::ReducedFloorAcknowledged,
    };
    assert_eq!(
        LocalCreateDriveSafety::new().admit_motion(
            &mut provider,
            100,
            Some(reduced),
            failed,
            DifferentialMotionRequest {
                left_mm_s: 20,
                right_mm_s: 20,
                ttl_ms: 100,
            },
        ),
        DriveSafetySign::Refused(DriveRefusal::SafetyStaleOrInhibited(
            LocalHazard::WatchdogUnhealthy
        ))
    );
    assert!(provider.bytes.is_empty());
}

#[test]
fn unavailable_auxiliary_input_and_contact_are_not_reported_as_clear() {
    let request = DifferentialMotionRequest {
        left_mm_s: 20,
        right_mm_s: 20,
        ttl_ms: 100,
    };
    let mut provider = Provider {
        available: true,
        bytes: vec![],
    };
    let unavailable = SafetyObservation {
        emergency_stop: SafetyInputObservation::Unavailable,
        ..safe()
    };
    assert_eq!(
        LocalCreateDriveSafety::new().admit_motion(
            &mut provider,
            100,
            Some(authority()),
            unavailable,
            request,
        ),
        DriveSafetySign::Refused(DriveRefusal::SafetyAuthorityMismatch)
    );
    let contact = SafetyObservation {
        contact: true,
        ..safe()
    };
    assert_eq!(
        LocalCreateDriveSafety::new().admit_motion(
            &mut provider,
            100,
            Some(authority()),
            contact,
            request,
        ),
        DriveSafetySign::Refused(DriveRefusal::SafetyStaleOrInhibited(LocalHazard::Contact))
    );
    assert!(provider.bytes.is_empty());
}

#[test]
fn ttl_expiry_and_hazard_force_exact_zero_wheel_command() {
    let mut provider = Provider {
        available: true,
        bytes: vec![],
    };
    let mut drive = LocalCreateDriveSafety::new();
    let request = DifferentialMotionRequest {
        left_mm_s: 20,
        right_mm_s: -20,
        ttl_ms: 100,
    };
    assert!(matches!(
        drive.admit_motion(&mut provider, 100, Some(authority()), safe(), request),
        DriveSafetySign::MotionAdmitted {
            deadline_tick: 200,
            ..
        }
    ));
    assert!(matches!(
        drive.supervise(
            &mut provider,
            200,
            SafetyObservation {
                observed_at_tick: 200,
                ..safe()
            }
        ),
        Some(DriveSafetySign::SafeDisposition {
            cause: SafeDispositionCause::DeadlineExpired,
            ..
        })
    ));
    assert_eq!(&provider.bytes[5..], &[145, 0, 0, 0, 0]);

    let mut hazard = safe();
    hazard.observed_at_tick = 210;
    hazard.wheel_drop = true;
    hazard.generation = 8;
    assert!(matches!(
        drive.supervise(&mut provider, 210, hazard),
        Some(DriveSafetySign::SafeDisposition {
            cause: SafeDispositionCause::Hazard(LocalHazard::WheelDrop),
            safety_generation: 8
        })
    ));
}

#[test]
fn future_clock_and_regressed_safety_generation_fail_closed() {
    let mut provider = Provider {
        available: true,
        bytes: vec![],
    };
    let mut drive = LocalCreateDriveSafety::new();
    let request = DifferentialMotionRequest {
        left_mm_s: 20,
        right_mm_s: 20,
        ttl_ms: 100,
    };
    let mut future = safe();
    future.observed_at_tick = 101;
    assert_eq!(
        drive.admit_motion(&mut provider, 100, Some(authority()), future, request),
        DriveSafetySign::Refused(DriveRefusal::SafetyStaleOrInhibited(
            LocalHazard::SafetyClockInvalid
        ))
    );
    assert!(provider.bytes.is_empty());

    assert!(matches!(
        drive.admit_motion(&mut provider, 100, Some(authority()), safe(), request),
        DriveSafetySign::MotionAdmitted { .. }
    ));
    let regressed = SafetyObservation {
        generation: 6,
        observed_at_tick: 101,
        ..safe()
    };
    assert!(matches!(
        drive.supervise(&mut provider, 101, regressed),
        Some(DriveSafetySign::SafeDisposition {
            cause: SafeDispositionCause::Hazard(LocalHazard::SafetyGenerationRegressed),
            ..
        })
    ));
    assert_eq!(&provider.bytes[5..], &[145, 0, 0, 0, 0]);
}

#[test]
fn cleared_raw_contact_still_refuses_motion_while_the_local_latch_remains() {
    let base = safe();
    let inputs = |generation, contact| crate::SafetyInputs {
        generation,
        observed_at_tick: 100,
        maximum_age_ticks: base.maximum_age_ticks,
        emergency_stop: base.emergency_stop,
        wheel_drop: base.wheel_drop,
        cliff: base.cliff,
        contact,
        tilt: base.tilt,
        impact: base.impact,
        charging: base.charging,
        control_alive: base.control_alive,
        body_link_alive: base.body_link_alive,
        independent_watchdog: base.independent_watchdog,
    };
    let mut envelope = crate::LocalSafetyEnvelope::new();
    envelope.observe(inputs(1, true), 100).unwrap();
    envelope.observe(inputs(2, false), 100).unwrap();
    let mut provider = Provider {
        available: true,
        bytes: vec![],
    };
    let mut drive = LocalCreateDriveSafety::new();
    assert_eq!(
        drive.admit_motion(
            &mut provider,
            100,
            Some(authority()),
            envelope.snapshot().unwrap(),
            DifferentialMotionRequest {
                left_mm_s: 20,
                right_mm_s: 20,
                ttl_ms: 100,
            },
        ),
        DriveSafetySign::Refused(DriveRefusal::SafetyStaleOrInhibited(LocalHazard::Contact))
    );
    assert!(provider.bytes.is_empty());
}
