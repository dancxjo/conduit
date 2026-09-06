//! Common admission checks before allocating numeric lowering tables.
use super::{KernelStorageProfile, LoweringError};
use conduit_core::{mandatory_sign_storage_requirement, verify_plan_fragment, PlanFragment};

pub(super) fn validate_fragment(
    fragment: &PlanFragment,
    profile: KernelStorageProfile,
) -> Result<(), LoweringError> {
    if !verify_plan_fragment(fragment) {
        return Err(LoweringError::InvalidFragment);
    }
    // Fresh initialization profiles must never silently reset retained State.
    if let Some(state) = fragment
        .states
        .iter()
        .find(|state| state.retained.is_some())
    {
        if !profile.supports_owned_state_continuity() {
            return Err(LoweringError::UnsupportedState(state.state_id.clone()));
        }
    }
    if let Some(state) = fragment.states.first() {
        let Some((instances, bytes)) = profile.state_storage() else {
            return Err(LoweringError::UnsupportedState(state.state_id.clone()));
        };
        if fragment.states.len() > usize::from(instances)
            || fragment
                .states
                .iter()
                .any(|state| state.maximum_value_bytes > bytes)
        {
            return Err(LoweringError::StateStorageExceeded);
        }
    }
    if fragment.placements.is_empty() {
        return Err(LoweringError::EmptyFragment);
    }
    let mut expected = mandatory_sign_storage_requirement(&fragment.expected_sign)
        .ok_or(LoweringError::SignBudgetInvalid)?;
    let state = conduit_core::state_resource_budget(&fragment.states)
        .map_err(|_| LoweringError::SignBudgetInvalid)?
        .sign_storage;
    expected.item_capacity = expected
        .item_capacity
        .checked_add(state.item_capacity)
        .ok_or(LoweringError::SignBudgetInvalid)?;
    expected.byte_capacity = expected
        .byte_capacity
        .checked_add(state.byte_capacity)
        .ok_or(LoweringError::SignBudgetInvalid)?;
    if expected != fragment.sign_storage_budget {
        return Err(LoweringError::SignBudgetInvalid);
    }
    Ok(())
}
