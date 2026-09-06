use conduit_core::StateId;
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweringError};

#[path = "../../core/tests/common/sealed_state.rs"]
mod common;

#[test]
fn current_profiles_refuse_state_instead_of_ignoring_its_sealed_contract() {
    let plan = common::seal(common::fragment());
    assert!(matches!(lower_plan_fragment(&plan.fragments[0]),
        Err(LoweringError::UnsupportedState(state)) if state == StateId::from("retained")));
}

#[test]
fn altered_state_refuses_as_invalid_before_profile_admission() {
    let mut plan = common::seal(common::fragment());
    plan.fragments[0].states[0].initial_value = vec![8];
    assert!(matches!(
        lower_plan_fragment(&plan.fragments[0]),
        Err(LoweringError::InvalidFragment)
    ));
}

#[test]
fn explicit_state_storage_lowers_every_exact_contract_and_numeric_owner() {
    use conduit_plan_lowering::lowering::{
        lower_plan_fragment_for_profile, FIXED_KERNEL_STORAGE_PROFILE,
    };
    let plan = common::seal(common::fragment());
    let profile = FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(1, 1)
        .unwrap();
    let lowered = lower_plan_fragment_for_profile(&plan.fragments[0], profile).unwrap();
    assert_eq!(lowered.states.len(), 1);
    let state = &lowered.states[0];
    assert_eq!(state.contract, plan.fragments[0].states[0]);
    assert_eq!(state.slot, 0);
    assert_eq!(state.node, lowered.nodes[0].node);
    assert_eq!(state.next, lowered.nodes[0].inputs[0].port);
    assert_eq!(state.current, lowered.nodes[0].outputs[0].port);
    assert_eq!(
        lowered.sign_items,
        plan.fragments[0].sign_storage_budget.item_capacity
    );
}

#[test]
fn smaller_selected_state_storage_refuses_before_installation() {
    use conduit_plan_lowering::lowering::{
        lower_plan_fragment_for_profile, FIXED_KERNEL_STORAGE_PROFILE,
    };
    let mut fragment = common::fragment();
    fragment.states[0].maximum_value_bytes = 2;
    let plan = common::seal(fragment);
    let profile = FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(1, 1)
        .unwrap();
    assert_eq!(
        lower_plan_fragment_for_profile(&plan.fragments[0], profile),
        Err(LoweringError::StateStorageExceeded)
    );
    assert!(FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(0, 1)
        .is_err());
    assert!(FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(1, 0)
        .is_err());
}

#[test]
fn fresh_state_profile_refuses_retained_state_instead_of_resetting_it() {
    use conduit_plan_lowering::lowering::{
        lower_plan_fragment_for_profile, FIXED_KERNEL_STORAGE_PROFILE,
    };
    let plan = common::seal(common::retained_fragment());
    assert!(conduit_core::verify_plan(&plan));
    let profile = FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(1, 1)
        .unwrap();
    assert!(matches!(
        lower_plan_fragment_for_profile(&plan.fragments[0], profile),
        Err(LoweringError::UnsupportedState(_))
    ));
}
