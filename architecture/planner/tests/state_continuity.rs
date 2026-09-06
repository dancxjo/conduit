use conduit_core::*;
use conduit_planner::state_delay::continuity::*;
#[path = "../../core/tests/common/sealed_state.rs"]
mod common;

fn candidates() -> (Plan, Plan, RetainedStateProvenance, StateContinuityApproval) {
    let source = common::seal(common::fragment());
    let mut destination = common::fragment();
    destination.states[0].maximum_value_bytes = 2;
    destination.boot_id = "replacement-boot".into();
    destination.placements[0].boot_id = destination.boot_id.clone();
    let destination = common::seal(destination);
    let retained = common::retained_fragment()
        .states
        .remove(0)
        .retained
        .unwrap();
    let approval = StateContinuityApproval {
        source_plan: source.plan_id.clone(),
        destination_plan: destination.plan_id.clone(),
        state: retained.source_state.clone(),
        maximum_value_bytes: 2,
    };
    (source, destination, retained, approval)
}

#[test]
fn explicit_capacity_upgrade_seals_same_form_and_preserves_fresh_boot_bindings() {
    let (source, destination, retained, approval) = candidates();
    let replacement =
        seal_state_continuity(&source, &destination, retained.clone(), &approval).unwrap();
    assert!(verify_plan(&replacement));
    assert_ne!(replacement.plan_id, destination.plan_id);
    assert_eq!(replacement.checked_form_id, source.checked_form_id);
    assert_eq!(
        replacement.fragments[0].boot_id,
        destination.fragments[0].boot_id
    );
    assert_ne!(
        replacement.fragments[0].boot_id,
        retained.source_play.boot_id
    );
    assert_eq!(
        replacement.fragments[0].placements,
        destination.fragments[0].placements
    );
    assert_eq!(replacement.fragments[0].states[0].retained, Some(retained));
    assert_eq!(replacement.fragments[0].states[0].initial_value, vec![7]);
    assert!(destination.fragments[0].states[0].retained.is_none());
}

#[test]
fn approval_cannot_be_reused_for_other_candidates_or_capacity() {
    let (source, destination, retained, approval) = candidates();
    let mutations: [fn(&mut StateContinuityApproval); 4] = [
        |a| a.source_plan = "other".into(),
        |a| a.destination_plan = "other".into(),
        |a| a.state = "other".into(),
        |a| a.maximum_value_bytes = 1,
    ];
    for mutate in mutations {
        let mut changed = approval.clone();
        mutate(&mut changed);
        assert!(seal_state_continuity(&source, &destination, retained.clone(), &changed).is_err());
    }
}

#[test]
fn invented_source_bytes_generation_and_identity_refuse() {
    let (source, destination, retained, approval) = candidates();
    let mutations: [fn(&mut RetainedStateProvenance); 5] = [
        |r| r.current_value = vec![1, 2], // Fits destination but never fit source.
        |r| r.generation = 0,             // A fresh cell cannot contain a noninitial value.
        |r| r.source_play.boot_id = "other-boot".into(),
        |r| r.source_form.checked_form_id = "other-form".into(),
        |r| r.source_play.plan_id = "other-plan".into(),
    ];
    for mutate in mutations {
        let mut changed = retained.clone();
        mutate(&mut changed);
        assert!(seal_state_continuity(&source, &destination, changed, &approval).is_err());
    }
}

#[test]
fn incompatible_initialization_and_specialization_are_not_capacity_upgrades() {
    let (source, destination, retained, mut approval) = candidates();
    let mut fragment = destination.fragments[0].clone();
    fragment.states[0].initial_value = vec![8];
    let changed = common::seal(fragment);
    approval.destination_plan = changed.plan_id.clone();
    assert_eq!(
        seal_state_continuity(&source, &changed, retained.clone(), &approval),
        Err(StateContinuityRefusal::ContractMismatch)
    );
    let mut fragment = destination.fragments[0].clone();
    fragment.checked_form_id = "other-specialization".into();
    let changed = common::seal(fragment);
    approval.destination_plan = changed.plan_id.clone();
    assert_eq!(
        seal_state_continuity(&source, &changed, retained, &approval),
        Err(StateContinuityRefusal::FormMismatch)
    );
}

#[test]
fn smaller_destination_refuses_a_value_that_fit_the_source() {
    let mut source_fragment = common::fragment();
    source_fragment.states[0].maximum_value_bytes = 2;
    let source = common::seal(source_fragment);
    let destination = common::seal(common::fragment());
    let mut retained = common::retained_fragment()
        .states
        .remove(0)
        .retained
        .unwrap();
    retained.source_play = bind_active_play(
        &source.plan_id,
        &source.fragments[0].host_id,
        &source.fragments[0].boot_id,
        3,
    );
    retained.current_value = vec![9, 9];
    let approval = StateContinuityApproval {
        source_plan: source.plan_id.clone(),
        destination_plan: destination.plan_id.clone(),
        state: retained.source_state.clone(),
        maximum_value_bytes: 1,
    };
    assert_eq!(
        seal_state_continuity(&source, &destination, retained, &approval),
        Err(StateContinuityRefusal::CapacityExceeded)
    );
}

#[test]
fn second_handoff_cannot_roll_back_generation_or_change_bytes_without_a_transition() {
    let source = common::seal(common::retained_fragment());
    let destination = common::seal(common::fragment());
    let mut retained = source.fragments[0].states[0].retained.clone().unwrap();
    retained.source_play = bind_active_play(
        &source.plan_id,
        &source.fragments[0].host_id,
        &source.fragments[0].boot_id,
        4,
    );
    let approval = StateContinuityApproval {
        source_plan: source.plan_id.clone(),
        destination_plan: destination.plan_id.clone(),
        state: retained.source_state.clone(),
        maximum_value_bytes: 1,
    };
    assert!(seal_state_continuity(&source, &destination, retained.clone(), &approval).is_ok());
    let mut rollback = retained.clone();
    rollback.generation -= 1;
    assert_eq!(
        seal_state_continuity(&source, &destination, rollback, &approval),
        Err(StateContinuityRefusal::GenerationMismatch)
    );
    retained.current_value = vec![8];
    assert_eq!(
        seal_state_continuity(&source, &destination, retained, &approval),
        Err(StateContinuityRefusal::GenerationMismatch)
    );
}
