//! Common admission checks before allocating numeric lowering tables.
use super::LoweringError;
use conduit_core::{mandatory_sign_storage_requirement, verify_plan_fragment, PlanFragment};

pub(super) fn validate_fragment(fragment: &PlanFragment) -> Result<(), LoweringError> {
    if !verify_plan_fragment(fragment) {
        return Err(LoweringError::InvalidFragment);
    }
    if let Some(state) = fragment.states.first() {
        return Err(LoweringError::UnsupportedState(state.state_id.clone()));
    }
    if fragment.placements.is_empty() {
        return Err(LoweringError::EmptyFragment);
    }
    if mandatory_sign_storage_requirement(&fragment.expected_sign)
        != Some(fragment.sign_storage_budget)
    {
        return Err(LoweringError::SignBudgetInvalid);
    }
    Ok(())
}
