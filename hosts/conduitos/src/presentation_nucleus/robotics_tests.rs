use alloc::vec::Vec;
use conduit_core::Scalar;

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
            && placement.host_operations.is_empty()
            && placement.resources.is_empty()
            && placement.authority.is_empty()
    }));
    let proof = run_robotics(&prepared).unwrap();
    assert_eq!((proof.node_count, proof.cord_count), (10, 7));
    assert_eq!(
        proof.effect,
        RoboticsDriveEffect::Projected {
            linear: Scalar::from_raw_microunits(750_000),
            angular: Scalar::from_raw_microunits(-250_000),
        }
    );
}

#[test]
fn bumper_near_range_and_stale_range_are_distinct_suppression_inputs() {
    for prepared in [
        prepare_robotics("robot-host", "pressed", true, 500, 0).unwrap(),
        prepare_robotics("robot-host", "near", false, 249, 0).unwrap(),
        prepare_robotics("robot-host", "stale", false, 500, 1_001).unwrap(),
    ] {
        assert_eq!(
            run_robotics(&prepared).unwrap().effect,
            RoboticsDriveEffect::Suppressed
        );
    }
}

#[test]
fn exact_configuration_reseals_plan_identity() {
    let clear = prepare_robotics("robot-host", "robot-boot", false, 500, 0).unwrap();
    let pressed = prepare_robotics("robot-host", "robot-boot", true, 500, 0).unwrap();
    assert_ne!(clear.plan.plan_id, pressed.plan.plan_id);
}
