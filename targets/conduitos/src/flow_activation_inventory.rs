//! Resident embedded inventory fence for Flow activation coordinators.
//!
//! ConduitOS does not yet carry an admitted resident `KernelCompositeHost`
//! pool. Whole-Plan lowering therefore refuses activation-owning fragments
//! instead of presenting a non-executable target façade.

use conduit_core::{FragmentId, Plan};
use conduit_plan_lowering::lowering::{
    LoweredPlanFragment, LoweringError, lower_plan_fragment_from_plan,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResidentFlowActivationRefusal {
    Lowering(LoweringError),
    InventoryNotResident,
}

pub fn lower_resident_fragment(
    plan: &Plan,
    fragment_id: &FragmentId,
) -> Result<LoweredPlanFragment, ResidentFlowActivationRefusal> {
    let (fragment, activations) = lower_plan_fragment_from_plan(plan, fragment_id)
        .map_err(ResidentFlowActivationRefusal::Lowering)?;
    if activations.entries.is_empty() {
        Ok(fragment)
    } else {
        Err(ResidentFlowActivationRefusal::InventoryNotResident)
    }
}
