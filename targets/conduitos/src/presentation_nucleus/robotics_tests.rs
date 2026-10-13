use alloc::vec::Vec;
use conduit_core::MicroScalar;

use super::{RoboticsDriveEffect, prepare_robotics, run_robotics};

#[test]
fn all_seven_prewake_kinds_plan_and_clear_path_projects() {
    let prepared = prepare_robotics("robot-host", "robot-boot", false, 500, 0).unwrap();
    let fragment = &prepared.plan.fragments[0];
    let robotics = fragment
        .placements
        .iter()
        .filter(|placement| placement.kind_id.as_str().starts_with("robotics/"))
        .collect::<Vec<_>>();
    assert_eq!(robotics.len(), 7);
    assert!(robotics.into_iter().all(|placement| {
        placement.kind_id.as_str().starts_with("robotics/")
            && placement
                .implementation_id
                .as_str()
                .starts_with("conduitos/kernel-robotics-prewake-")
            && placement.host_calls.is_empty()
            && placement.resources.is_empty()
            && placement.authority.is_empty()
    }));
    let proof = run_robotics(&prepared).unwrap();
    assert_eq!((proof.node_count, proof.cord_count), (12, 7));
    assert_eq!(
        proof.effect,
        RoboticsDriveEffect::Projected {
            linear: MicroScalar::from_raw_microunits(750_000),
            angular: MicroScalar::from_raw_microunits(-250_000),
        }
    );
}

#[test]
fn authored_sensor_values_cannot_be_wired_into_the_drive_safety_boundary() {
    for prepared in [
        prepare_robotics("robot-host", "pressed", true, 500, 0).unwrap(),
        prepare_robotics("robot-host", "near", false, 249, 0).unwrap(),
        prepare_robotics("robot-host", "stale", false, 500, 1_001).unwrap(),
    ] {
        assert!(matches!(
            run_robotics(&prepared).unwrap().effect,
            RoboticsDriveEffect::Projected { .. }
        ));
    }
}

#[test]
fn exact_configuration_reseals_plan_identity() {
    let clear = prepare_robotics("robot-host", "robot-boot", false, 500, 0).unwrap();
    let pressed = prepare_robotics("robot-host", "robot-boot", true, 500, 0).unwrap();
    assert_ne!(clear.plan.plan_id, pressed.plan.plan_id);
}

#[test]
fn motor_realization_requires_exact_explicit_micro_grid_projection() {
    let admitted = conduit_core::Scalar::parse_decimal("0.75")
        .unwrap()
        .canonical_bytes()
        .unwrap();
    assert_eq!(
        conduit_core::MathValueRef::validate(&admitted)
            .unwrap()
            .checked_microscalar(),
        Ok(MicroScalar::from_raw_microunits(750_000))
    );
    let third = conduit_core::Scalar::integer(1)
        .checked_div(&conduit_core::Scalar::integer(3))
        .unwrap()
        .canonical_bytes()
        .unwrap();
    assert_eq!(
        conduit_core::MathValueRef::validate(&third)
            .unwrap()
            .checked_microscalar(),
        Err(conduit_core::NumericProjectionRefusal::Inexact)
    );
    assert!(
        conduit_core::validate_primitive_info(conduit_core::MICRO_SCALAR_INFO_ID, &third).is_err()
    );
}

#[test]
fn motor_back_refuses_mathematical_fraction_wire_and_accepts_reviewed_projection() {
    use conduit_kernel::{PortId, ValueRef};
    let mut back = super::robotics_back::RoboticsDriveBack::new();
    let third = conduit_core::Scalar::integer(1)
        .checked_div(&conduit_core::Scalar::integer(3))
        .unwrap()
        .canonical_bytes()
        .unwrap();
    assert!(
        back.accept_for_test(
            PortId(0),
            ValueRef {
                slot: 0,
                generation: 1,
                byte_len: third.len() as u32
            },
            &third
        )
        .is_err()
    );
    let linear = MicroScalar::from_raw_microunits(750_000).encode();
    let angular = MicroScalar::ZERO.encode();
    assert_eq!(
        back.accept_for_test(
            PortId(0),
            ValueRef {
                slot: 0,
                generation: 1,
                byte_len: 8
            },
            &linear
        ),
        Ok(())
    );
    assert_eq!(
        back.accept_for_test(
            PortId(1),
            ValueRef {
                slot: 1,
                generation: 1,
                byte_len: 8
            },
            &angular
        ),
        Ok(())
    );
    assert_eq!(
        back.effect(),
        Some(RoboticsDriveEffect::Projected {
            linear: MicroScalar::from_raw_microunits(750_000),
            angular: MicroScalar::ZERO
        })
    );
}
