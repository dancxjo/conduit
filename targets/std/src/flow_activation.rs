//! Standard-host installation of Plan-sealed Flow activation coordinators.

use conduit_composite::{
    KernelOperationRegistry, PlannedActivationCompositeError, PreparedPlannedActivationComposite,
};
use conduit_core::{verify_prepared_plan, FragmentId, Plan, PlannedActivationEntry, PreparedPlan};
use conduit_plan_lowering::activation_fragment::verify_lowered_fragment_activations;
use conduit_plan_lowering::lowering::lower_plan_fragment_from_plan;

#[derive(Debug)]
pub enum FlowActivationPreparationError {
    StalePreparation,
    MissingActivation,
    WrongOwnerFragment,
    Lowering,
    Composite(PlannedActivationCompositeError),
}

/// Prepare one exact each/select/fold/scan coordinator from verified Plan
/// truth. The registry is an explicit preparation input and is never retained
/// as ambient runtime authority.
pub fn prepare_flow_activation(
    plan: &Plan,
    prepared: &PreparedPlan,
    owner_fragment_id: &FragmentId,
    activation_id: &str,
    registry: &KernelOperationRegistry,
) -> Result<PreparedPlannedActivationComposite, FlowActivationPreparationError> {
    if !verify_prepared_plan(prepared, plan) {
        return Err(FlowActivationPreparationError::StalePreparation);
    }
    let (_, lowered) = lower_plan_fragment_from_plan(plan, owner_fragment_id)
        .map_err(|_| FlowActivationPreparationError::Lowering)?;
    if !verify_lowered_fragment_activations(&lowered, plan) {
        return Err(FlowActivationPreparationError::Lowering);
    }
    let mut matches = lowered.entries.iter().filter(|entry| match entry {
        PlannedActivationEntry::Unary(value) => value.activation_id == activation_id,
        PlannedActivationEntry::Fold(value) => value.activation_id == activation_id,
        PlannedActivationEntry::Scan(value) => value.activation_id == activation_id,
    });
    if matches.next().is_none() {
        return Err(FlowActivationPreparationError::WrongOwnerFragment);
    }
    if matches.next().is_some() {
        return Err(FlowActivationPreparationError::MissingActivation);
    }
    PreparedPlannedActivationComposite::prepare(plan, prepared, activation_id, registry)
        .map_err(FlowActivationPreparationError::Composite)
}
