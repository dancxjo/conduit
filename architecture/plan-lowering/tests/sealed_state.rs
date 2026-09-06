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
