//! Plan-level admission for an explicitly approved same-specialization handoff.
use conduit_core::{
    seal_plan_with_realization_backs, state_resource_budget, verify_plan, FormIdentity, Plan,
    PlanId, RetainedStateProvenance, StateId,
};

/// A decision supplied by the lifecycle owner, not authority minted by planning.
/// It approves only State continuity between these exact candidates. Destination
/// host operations/resources still require their ordinary fresh admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateContinuityApproval {
    pub source_plan: PlanId,
    pub destination_plan: PlanId,
    pub state: StateId,
    pub maximum_value_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateContinuityRefusal {
    InvalidPlan,
    ApprovalMismatch,
    FormMismatch,
    SourceMismatch,
    StateMissing,
    ContractMismatch,
    CapacityExceeded,
    AlreadyRetained,
    GenerationMismatch,
}

/// Seal a fresh Plan with an exact retained-value obligation. The caller must
/// obtain the provenance from its retired runtime owner; a serialized record
/// alone does not attest that the value was committed or that execution ended.
/// This supports unchanged checked meaning, not arbitrary schema migration.
pub fn seal_state_continuity(
    source: &Plan,
    destination: &Plan,
    retained: RetainedStateProvenance,
    approval: &StateContinuityApproval,
) -> Result<Plan, StateContinuityRefusal> {
    use StateContinuityRefusal as R;
    if !verify_plan(source) || !verify_plan(destination) {
        return Err(R::InvalidPlan);
    }
    if approval.source_plan != source.plan_id
        || approval.destination_plan != destination.plan_id
        || approval.state != retained.source_state
    {
        return Err(R::ApprovalMismatch);
    }
    let identity = |plan: &Plan| FormIdentity {
        source_document_id: plan.source_document_id.clone(),
        checked_form_id: plan.checked_form_id.clone(),
        expanded_form_id: plan.expanded_form_id.clone(),
    };
    if identity(source) != retained.source_form || identity(source) != identity(destination) {
        return Err(R::FormMismatch);
    }
    let source_fragment = source
        .fragments
        .iter()
        .find(|fragment| {
            fragment.host_id == retained.source_play.host_id
                && fragment.boot_id == retained.source_play.boot_id
        })
        .ok_or(R::SourceMismatch)?;
    if retained.source_play.plan_id != source.plan_id {
        return Err(R::SourceMismatch);
    }
    let prior = source_fragment
        .states
        .iter()
        .find(|state| state.state_id == retained.source_state)
        .ok_or(R::StateMissing)?;
    let mut fragments = destination.fragments.clone();
    let next = fragments
        .iter_mut()
        .flat_map(|fragment| &mut fragment.states)
        .find(|state| state.state_id == retained.source_state)
        .ok_or(R::StateMissing)?;
    if next.retained.is_some() {
        return Err(R::AlreadyRetained);
    }
    if next.value_kind != prior.value_kind
        || retained.value_kind != prior.value_kind
        || next.gear_id != prior.gear_id
        || next.initial_value != prior.initial_value
        || next.continuation != prior.continuation
    {
        return Err(R::ContractMismatch);
    }
    if approval.maximum_value_bytes != next.maximum_value_bytes
        || retained.current_value.len() > prior.maximum_value_bytes as usize
        || retained.current_value.len() > next.maximum_value_bytes as usize
    {
        return Err(R::CapacityExceeded);
    }
    if prior.retained.as_ref().is_some_and(|old| {
        retained.generation < old.generation
            || (retained.generation == old.generation
                && retained.current_value != old.current_value)
    }) || (retained.generation == 0 && retained.current_value != prior.initial_value)
    {
        return Err(R::GenerationMismatch);
    }
    // Check the source's continuation bound and exact ActivePlay digest too.
    let mut source_contract = prior.clone();
    source_contract.retained = Some(retained.clone());
    state_resource_budget(core::slice::from_ref(&source_contract))
        .map_err(|_| R::SourceMismatch)?;
    next.retained = Some(retained);
    let replacement = seal_plan_with_realization_backs(
        identity(destination),
        destination.realization_backs.clone(),
        fragments,
    );
    if !verify_plan(&replacement) {
        return Err(R::InvalidPlan);
    }
    Ok(replacement)
}
