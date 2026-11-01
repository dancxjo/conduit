//! Transfer the actual initialized output provider under a Core-verified Plan.
//! This issues no authority and neither starts a Play nor authorizes its Fore.
use super::*;
use crate::{codec::PreparedPackets, owner::PreparedWriteBinding};
use conduit_plan_lowering::selected_operation::{
    bind_selected_verified_operation, SelectedCallRefusal, SelectedOperationContract,
    SelectedVerifiedOperationPlan,
};
impl<P: OpenDrain> ReadyWriteBase<P> {
    pub fn check_verified(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &WriteContract,
        selected: SelectedVerifiedOperationPlan<'_>,
    ) -> Result<conduit_kernel::NodeId, WriteCallRefusal<P::Error>> {
        self.prepare_verified(table, claim, contract, selected)
            .map(|p| p.node)
    }
    fn prepare_verified(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &WriteContract,
        selected: SelectedVerifiedOperationPlan<'_>,
    ) -> Result<PreparedWriteBinding, WriteCallRefusal<P::Error>> {
        self.check_claim(claim)?;
        crate::owner::check_possession(table, claim, self.attachment)?;
        let fragment = selected
            .plan
            .fragment(selected.fragment_id)
            .ok_or(WriteCallRefusal::WrongBinding)?;
        let gear = fragment
            .lowering_metadata()
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selected.placement_id)
            .ok_or(WriteCallRefusal::WrongBinding)?;
        self.check_selected_native(gear)?;
        let node = bind_selected_verified_operation(
            table,
            claim,
            SelectedOperationContract {
                kind: contract.kind(),
                call: WRITE_CALL,
                input_bytes: MAXIMUM_BYTES,
                output_bytes: 0,
                resource_bytes: self.attachment.resource_bytes,
            },
            selected,
        )
        .map_err(|e| match e {
            SelectedCallRefusal::WrongBinding => WriteCallRefusal::WrongBinding,
            SelectedCallRefusal::Possession => WriteCallRefusal::Possession,
        })?;
        let packets = PreparedPackets::prepare(contract)
            .map_err(|e| WriteCallRefusal::Packets(PacketRefusal::Canonical(e)))?;
        Ok(PreparedWriteBinding { node, packets })
    }
    /// All identity, exact-selection and codec checks finish before the retained
    /// slot is taken. Refusal leaves the actual pins available to Root.
    pub fn bind_verified_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &WriteContract,
        selected: SelectedVerifiedOperationPlan<'_>,
    ) -> Result<WriteHostCall<P>, WriteCallRefusal<P::Error>> {
        let prepared = slot
            .as_ref()
            .ok_or(WriteCallRefusal::Possession)?
            .prepare_verified(&table, &claim, contract, selected)?;
        let owner = slot.take().expect("preflighted retained output owner");
        Ok(WriteHostCall::from_prepared(
            table,
            handle,
            claim,
            owner.attachment,
            owner.provider,
            prepared,
        ))
    }
}
