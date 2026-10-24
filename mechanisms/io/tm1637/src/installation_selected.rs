//! Ordinary selected transfer of the actual retained initialized pin provider.
use super::*;
use conduit_plan_lowering::selected_operation::SelectedOperationPlan;
impl<P: OpenDrain> ReadyWriteBase<P> {
    /// Complete all native identity, selected Plan and codec checks before
    /// taking Root's provider. Refusal preserves the initialized retained slot.
    pub fn bind_selected_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &WriteContract,
        selection: SelectedOperationPlan<'_>,
    ) -> Result<WriteHostCall<P>, WriteCallRefusal<P::Error>> {
        let owner = slot.as_ref().ok_or(WriteCallRefusal::Possession)?;
        owner.check_claim(&claim)?;
        let gear = selection
            .fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selection.placement_id)
            .ok_or(WriteCallRefusal::WrongBinding)?;
        let base = gear.base.as_ref().ok_or(WriteCallRefusal::WrongBinding)?;
        let identity = &owner.identity;
        if gear.artifact_id != identity.artifact_id
            || gear.execution_profile_id.as_str() != WRITE_EXECUTION_PROFILE
            || gear.capability_id.as_str() != WRITE_CAPABILITY
            || base.base_id != identity.base_id
            || base.implementation_id != identity.base_implementation_id
            || base.mechanism_family.as_str() != WRITE_ATTACHMENT_CLASS
            || base.enforcement_class != BaseEnforcementClass::Cooperative
            || !gear.authority.iter().any(|authority| {
                authority.contract_id.as_str() == WRITE_AUTHORITY
                    && authority.host_call_contract_id.as_str() == WRITE_CALL
                    && authority.subject_kind.as_str() == WRITE_KIND
            })
        {
            return Err(WriteCallRefusal::WrongBinding);
        }
        let prepared = WriteHostCall::<P>::prepare_selected(
            &table,
            &claim,
            owner.attachment,
            contract,
            selection,
        )?;
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
