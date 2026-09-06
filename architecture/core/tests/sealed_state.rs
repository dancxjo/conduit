use conduit_core::*;
#[path = "common/sealed_state.rs"]
mod common;
use common::{fragment, seal};

#[test]
fn every_state_contract_field_is_an_immutable_plan_commitment() {
    let plan = seal(fragment());
    assert!(verify_plan(&plan));
    let changes: [fn(&mut PlannedStateBoundary); 6] = [
        |state| state.state_id = StateId::from("other"),
        |state| state.gear_id = GearId::from("other"),
        |state| state.value_kind = KindId::from("other@1"),
        |state| state.initial_value = vec![8],
        |state| state.maximum_value_bytes = 2,
        |state| state.continuation = StateContinuation::MaximumTransitions(3),
    ];
    for change in changes {
        let mut altered = plan.clone();
        change(&mut altered.fragments[0].states[0]);
        assert!(!verify_plan(&altered));
        let resealed = seal(altered.fragments.remove(0));
        assert_ne!(resealed.plan_id, plan.plan_id);
    }
}

#[test]
fn larger_state_capacity_requires_a_new_plan_but_not_new_form_identity() {
    let small = seal(fragment());
    let mut larger = fragment();
    larger.states[0].maximum_value_bytes = 4;
    let large = seal(larger);
    assert!(verify_plan(&large));
    assert_ne!(small.plan_id, large.plan_id);
    assert_eq!(small.checked_form_id, large.checked_form_id);
    assert_eq!(small.source_document_id, large.source_document_id);
}

#[test]
fn resealing_cannot_admit_invalid_state_placement_type_or_evidence_capacity() {
    let changes: [fn(&mut PlanFragment); 6] = [
        |fragment| fragment.states[0].maximum_value_bytes = 0,
        |fragment| fragment.states[0].gear_id = GearId::from("absent"),
        |fragment| fragment.states[0].value_kind = KindId::from("wrong@1"),
        |fragment| fragment.states.push(fragment.states[0].clone()),
        |fragment| fragment.sign_storage_budget.item_capacity = 1,
        |fragment| fragment.sign_storage_budget.byte_capacity = 63,
    ];
    for change in changes {
        let mut invalid = fragment();
        change(&mut invalid);
        assert!(!verify_plan(&seal(invalid)));
    }
}

#[test]
fn state_identity_cannot_be_duplicated_across_host_fragments() {
    let first = fragment();
    let mut second = fragment();
    second.host_id = HostId::from("other-host");
    second.boot_id = BootId::from("other-boot");
    second.placements[0].host_id = second.host_id.clone();
    second.placements[0].boot_id = second.boot_id.clone();
    second.placements[0].gear_id = GearId::from("other-cell");
    second.placements[0].placement_id = PlacementId::from("other-placement");
    second.startup_order = vec![second.placements[0].placement_id.clone()];
    second.states[0].gear_id = second.placements[0].gear_id.clone();
    let identity = FormIdentity {
        source_document_id: first.source_document_id.clone(),
        checked_form_id: first.checked_form_id.clone(),
        expanded_form_id: first.expanded_form_id.clone(),
    };
    let duplicated = seal_plan(identity.clone(), vec![first.clone(), second.clone()]);
    assert!(!verify_plan(&duplicated));
    second.states[0].state_id = StateId::from("other-state");
    assert!(verify_plan(&seal_plan(identity, vec![first, second])));
}

#[test]
fn retained_provenance_changes_plan_without_replacing_authored_initialization() {
    let fresh = seal(fragment());
    let continued = seal(common::retained_fragment());
    assert!(verify_plan(&continued));
    assert_ne!(fresh.plan_id, continued.plan_id);
    assert_eq!(fresh.checked_form_id, continued.checked_form_id);
    assert_eq!(continued.fragments[0].states[0].initial_value, vec![7]);
    let retained = continued.fragments[0].states[0].retained.as_ref().unwrap();
    assert_eq!(retained.current_value, vec![9]);
    assert_eq!(retained.generation, 17);
    assert_eq!(retained.source_play.plan_id, fresh.plan_id);
    let mutations: [fn(&mut RetainedStateProvenance); 8] = [
        |r| r.generation += 1,
        |r| r.current_value[0] = 10,
        |r| r.source_form.source_document_id = "other-source".into(),
        |r| r.source_form.checked_form_id = "other-checked".into(),
        |r| r.source_form.expanded_form_id = "other-expanded".into(),
        |r| r.source_play.play_sequence += 1,
        |r| r.source_state = "other-state".into(),
        |r| r.value_kind = "other-kind".into(),
    ];
    for mutate in mutations {
        let mut changed = continued.clone();
        mutate(changed.fragments[0].states[0].retained.as_mut().unwrap());
        assert!(!verify_plan(&changed));
    }
}

#[test]
fn malformed_retained_identity_type_capacity_and_generation_refuse_even_after_reseal() {
    let mutations: [fn(&mut PlannedStateBoundary); 6] = [
        |s| s.retained.as_mut().unwrap().source_play.boot_id = "stale-boot".into(),
        |s| s.retained.as_mut().unwrap().source_state = "other-state".into(),
        |s| s.retained.as_mut().unwrap().value_kind = "wrong-kind".into(),
        |s| s.retained.as_mut().unwrap().current_value = vec![1, 2],
        |s| s.retained.as_mut().unwrap().source_form.checked_form_id = "".into(),
        |s| s.continuation = StateContinuation::MaximumTransitions(16),
    ];
    for mutate in mutations {
        let mut fragment = common::retained_fragment();
        mutate(&mut fragment.states[0]);
        assert!(!verify_plan(&seal(fragment)));
    }
}
