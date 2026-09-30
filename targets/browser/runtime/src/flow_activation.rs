//! Browser-host installation of Plan-sealed Flow activation coordinators.

use conduit_composite::{
    KernelOperationRegistry, PlannedActivationCompositeError, PreparedPlannedActivationComposite,
};
use conduit_core::{verify_prepared_plan, FragmentId, Plan, PlannedActivationEntry, PreparedPlan};
use conduit_plan_lowering::activation_fragment::verify_lowered_fragment_activations;
use conduit_plan_lowering::lowering::lower_plan_fragment_from_plan;

#[derive(Debug)]
pub enum FlowActivationPreparationError {
    StalePreparation,
    WrongOwnerFragment,
    AmbiguousActivation,
    Lowering,
    Composite(PlannedActivationCompositeError),
}

/// Browser preparation uses the same portable coordinator and bounds as the
/// standard host. No browser callback or secondary evaluator is introduced.
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
    let count = lowered
        .entries
        .iter()
        .filter(|entry| match entry {
            PlannedActivationEntry::Unary(value) => value.activation_id == activation_id,
            PlannedActivationEntry::Fold(value) => value.activation_id == activation_id,
            PlannedActivationEntry::Scan(value) => value.activation_id == activation_id,
        })
        .count();
    match count {
        0 => return Err(FlowActivationPreparationError::WrongOwnerFragment),
        1 => {}
        _ => return Err(FlowActivationPreparationError::AmbiguousActivation),
    }
    PreparedPlannedActivationComposite::prepare(plan, prepared, activation_id, registry)
        .map_err(FlowActivationPreparationError::Composite)
}
