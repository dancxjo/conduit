//! Bind checked State admission into a fresh immutable ordinary Plan.

use alloc::vec::Vec;
use conduit_core::{
    seal_plan_with_realization_backs, state_resource_budget, verify_plan, Plan,
    PlannedStateBoundary, StatePlanError,
};
use conduit_form::CheckedForm;

use super::{admit_state_graph, StateGraphError};

/// Seal structurally checked State contracts and their evidence reserve.
/// This does not establish that a selected semantic Kind implements those
/// contracts or that initialization/migration is authorized by authored meaning.
/// This does not grant an unsupported Host permission to ignore State: current
/// lowering profiles explicitly refuse until they implement the sealed contract.
pub fn seal_state_plan(
    form: &CheckedForm,
    plan: &Plan,
    states: Vec<PlannedStateBoundary>,
) -> Result<Plan, StateGraphError> {
    let graph = admit_state_graph(form, states)?;
    if !verify_plan(plan)
        || plan.source_document_id != form.source_document_id
        || plan.checked_form_id != form.checked_form_id
        || plan.expanded_form_id != form.expanded_form_id
    {
        return Err(StateGraphError::InvalidPlan);
    }
    if plan
        .fragments
        .iter()
        .any(|fragment| !fragment.states.is_empty())
    {
        return Err(StateGraphError::StateAlreadySealed);
    }
    let mut fragments = plan.fragments.clone();
    for state in graph.states {
        let owners = fragments
            .iter()
            .enumerate()
            .filter_map(|(index, fragment)| {
                fragment
                    .placements
                    .iter()
                    .any(|gear| gear.gear_id == state.gear_id)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        if owners.len() != 1 {
            return Err(StateGraphError::UnknownStatePlacement);
        }
        fragments[owners[0]].states.push(state);
    }
    for fragment in &mut fragments {
        let budget =
            state_resource_budget(&fragment.states).map_err(StateGraphError::InvalidStatePlan)?;
        fragment.sign_storage_budget.item_capacity = fragment
            .sign_storage_budget
            .item_capacity
            .checked_add(budget.sign_storage.item_capacity)
            .ok_or(StateGraphError::InvalidStatePlan(
                StatePlanError::ResourceOverflow,
            ))?;
        fragment.sign_storage_budget.byte_capacity = fragment
            .sign_storage_budget
            .byte_capacity
            .checked_add(budget.sign_storage.byte_capacity)
            .ok_or(StateGraphError::InvalidStatePlan(
                StatePlanError::ResourceOverflow,
            ))?;
    }
    let sealed = seal_plan_with_realization_backs(
        form.identity(),
        plan.realization_backs.clone(),
        fragments,
    );
    if !verify_plan(&sealed) {
        return Err(StateGraphError::InvalidPlan);
    }
    Ok(sealed)
}
