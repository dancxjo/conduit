//! Transfer retained capture ownership under a Core-verified complete Plan.
use super::*;
use conduit_plan_lowering::selected_operation::SelectedVerifiedOperationPlan;
impl<P: PulseReadProvider> ReadyPulseBase<P> {
    /// Preflight the retained native identity, exact lowering and capability
    /// possession without taking GPIO ownership or performing electrical work.
    pub fn check_verified(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &PulseReadContract,
        selected: SelectedVerifiedOperationPlan<'_>,
    ) -> Result<conduit_kernel::NodeId, PulseCallRefusal> {
        self.prepare_verified(table, claim, contract, selected)
            .map(|prepared| prepared.node)
    }
    fn prepare_verified(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &PulseReadContract,
        selected: SelectedVerifiedOperationPlan<'_>,
    ) -> Result<crate::owner::PreparedAssignedPulse, PulseCallRefusal> {
        let view = selected
            .plan
            .fragment(selected.fragment_id)
            .ok_or(PulseCallRefusal::WrongBinding)?;
        let gear = view
            .lowering_metadata()
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selected.placement_id)
            .ok_or(PulseCallRefusal::WrongBinding)?;
        self.check_native(claim, contract, gear)?;
        crate::owner::prepare_verified(table, claim, self.attachment, contract, selected)
    }
    /// All binding and codec preparation finish before taking the retained pin.
    /// Refusal preserves Root's slot; this issues no authority and invokes no call.
    pub fn bind_verified_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &PulseReadContract,
        selected: SelectedVerifiedOperationPlan<'_>,
    ) -> Result<PulseHostCall<P>, PulseCallRefusal> {
        let prepared = slot
            .as_ref()
            .ok_or(PulseCallRefusal::Possession)?
            .prepare_verified(&table, &claim, contract, selected)?;
        let owner = slot.take().expect("preflighted retained pulse owner");
        Ok(PulseHostCall::from_assigned(
            table,
            handle,
            claim,
            owner.attachment,
            owner.provider,
            prepared,
        ))
    }
}
