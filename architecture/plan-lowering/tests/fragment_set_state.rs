//! Fresh local State cells compose only within one explicit storage admission.
use conduit_core::{SourceDocumentId, StateLifetime};
use conduit_plan_lowering::{
    fragment_set::{
        lower_local_fragment_set, lower_local_fragment_set_from_plans, FragmentSetBounds,
        FragmentSetError,
    },
    lowering::{LoweringError, FIXED_KERNEL_STORAGE_PROFILE},
};
#[path = "../../core/tests/common/sealed_state.rs"]
mod common;

fn bounds() -> FragmentSetBounds {
    FragmentSetBounds {
        fragments: 2,
        nodes: 2,
        cords: 0,
        queue_slots: 0,
        value_bytes: 0,
        sign_items: 16,
        sign_bytes: 1024,
    }
}
fn plan(source: &str, lifetime: StateLifetime) -> conduit_core::Plan {
    let mut fragment = common::fragment();
    fragment.source_document_id = SourceDocumentId::from(source);
    fragment.states[0].lifetime = lifetime;
    common::seal(fragment)
}

#[test]
fn two_fresh_partitions_preserve_exact_state_contracts_and_rebase_cell_and_node_owners() {
    let first = plan("first", StateLifetime::Step);
    let second = plan("second", StateLifetime::Play);
    let profile = FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(2, 1)
        .unwrap();
    for lowered in [
        lower_local_fragment_set(
            &[&first.fragments[0], &second.fragments[0]],
            profile,
            bounds(),
        )
        .unwrap(),
        lower_local_fragment_set_from_plans(&[&first, &second], profile, bounds()).unwrap(),
    ] {
        assert_eq!(lowered.state_slots, 2);
        assert_eq!(lowered.nodes, 2);
        for (index, (original, part)) in [first.clone(), second.clone()]
            .iter()
            .zip(&lowered.partitions)
            .enumerate()
        {
            let state = &part.states[0];
            assert_eq!(state.slot, index as u16);
            assert_eq!(state.node.0, index as u16);
            assert_eq!(state.node, part.nodes[0].node);
            assert_eq!(state.node, part.identity.placements[0].0);
            assert_eq!(state.contract, original.fragments[0].states[0]);
            assert_eq!(part.identity.plan_id, original.plan_id);
        }
    }
}

#[test]
fn combined_state_cells_cannot_reuse_each_partitions_individually_admitted_slot() {
    let first = plan("first", StateLifetime::Step);
    let second = plan("second", StateLifetime::Play);
    let fragments = [&first.fragments[0], &second.fragments[0]];
    let error = lower_local_fragment_set(
        &fragments,
        FIXED_KERNEL_STORAGE_PROFILE
            .with_state_storage(1, 1)
            .unwrap(),
        bounds(),
    )
    .unwrap_err();
    assert!(matches!(error, FragmentSetError::Capacity(deficit)
        if deficit.resource == "state-slots" && deficit.required == 2 && deficit.available == 1));
    assert!(matches!(
        lower_local_fragment_set(&fragments, FIXED_KERNEL_STORAGE_PROFILE, bounds()),
        Err(FragmentSetError::Fragment(LoweringError::UnsupportedState(
            _
        )))
    ));
}

#[test]
fn fresh_fragment_sets_refuse_retained_and_cross_play_lifetimes_even_with_owned_continuity_profile()
{
    let profile = FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(2, 1)
        .unwrap()
        .with_owned_state_continuity();
    let retained = common::seal(common::retained_fragment());
    assert_eq!(
        lower_local_fragment_set(&[&retained.fragments[0]], profile, bounds()).unwrap_err(),
        FragmentSetError::StateUnsupported
    );
    for lifetime in [
        StateLifetime::Wake,
        StateLifetime::Boot,
        StateLifetime::Body,
    ] {
        let plan = plan("long-lived", lifetime);
        assert!(conduit_core::verify_plan(&plan));
        assert_eq!(
            lower_local_fragment_set_from_plans(&[&plan], profile, bounds()).unwrap_err(),
            FragmentSetError::StateUnsupported
        );
    }
}
