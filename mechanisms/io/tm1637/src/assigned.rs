//! Validate a constrained assignment before taking Root's actual pin owner.
use super::*;
use crate::{codec::PreparedPackets, owner::PreparedWriteBinding};
use conduit_plan_lowering::{
    selected_assigned_operation::{
        bind_selected_assigned_fore_operation, bind_selected_assigned_operation,
        AssignedNativeIdentity, SelectedAssignedForeOperationPlan, SelectedAssignedOperationPlan,
    },
    selected_operation::{SelectedCallRefusal, SelectedOperationContract},
};
impl<P: OpenDrain> ReadyWriteBase<P> {
    pub fn check_assigned(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &WriteContract,
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<conduit_kernel::NodeId, WriteCallRefusal<P::Error>> {
        self.prepare_assigned(table, claim, contract, selected, None)
            .map(|prepared| prepared.node)
    }
    /// The same exact possession checks, with independently linked local Fore pins.
    pub fn check_assigned_fore(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &WriteContract,
        selected: SelectedAssignedForeOperationPlan<'_>,
    ) -> Result<conduit_kernel::NodeId, WriteCallRefusal<P::Error>> {
        self.prepare_assigned(
            table,
            claim,
            contract,
            selected.operation,
            Some(selected.fore),
        )
        .map(|p| p.node)
    }
    fn prepare_assigned(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &WriteContract,
        selected: SelectedAssignedOperationPlan<'_>,
        fore: Option<&[conduit_core::AssignedForeRequirement]>,
    ) -> Result<PreparedWriteBinding, WriteCallRefusal<P::Error>> {
        self.check_claim(claim)?;
        crate::owner::check_possession(table, claim, self.attachment)?;
        let identity = &self.identity;
        let definition = SelectedOperationContract {
            kind: contract.kind(),
            call: WRITE_CALL,
            input_bytes: MAXIMUM_BYTES,
            output_bytes: 0,
            resource_bytes: self.attachment.resource_bytes,
        };
        let native = AssignedNativeIdentity {
            capability: WRITE_CAPABILITY,
            execution_profile: WRITE_EXECUTION_PROFILE,
            artifact: identity.artifact_id.as_str(),
            base: identity.base_id.as_str(),
            base_implementation: identity.base_implementation_id.as_str(),
            mechanism: WRITE_ATTACHMENT_CLASS,
            authority: WRITE_AUTHORITY,
            resource_class: WRITE_ATTACHMENT_CLASS,
        };
        let node = match fore {
            Some(fore) => bind_selected_assigned_fore_operation(
                table,
                claim,
                definition,
                native,
                SelectedAssignedForeOperationPlan {
                    operation: selected,
                    fore,
                },
            ),
            None => bind_selected_assigned_operation(table, claim, definition, native, selected),
        }
        .map_err(|reason| match reason {
            SelectedCallRefusal::WrongBinding => WriteCallRefusal::WrongBinding,
            SelectedCallRefusal::Possession => WriteCallRefusal::Possession,
        })?;
        let packets = PreparedPackets::prepare(contract)
            .map_err(|reason| WriteCallRefusal::Packets(PacketRefusal::Canonical(reason)))?;
        Ok(PreparedWriteBinding { node, packets })
    }
    /// Every fallible identity, assignment and codec check precedes transfer.
    /// Refusal preserves Root's exact initialized provider; success transfers it
    /// once into the current Play and starts no electrical operation by itself.
    pub fn bind_assigned_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &WriteContract,
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<WriteHostCall<P>, WriteCallRefusal<P::Error>> {
        let prepared = slot
            .as_ref()
            .ok_or(WriteCallRefusal::Possession)?
            .prepare_assigned(&table, &claim, contract, selected, None)?;
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
    /// Refusal preserves the initialized pins; success transfers them exactly
    /// once after the explicit Fore decoder and all existing possession checks.
    pub fn bind_assigned_fore_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &WriteContract,
        selected: SelectedAssignedForeOperationPlan<'_>,
    ) -> Result<WriteHostCall<P>, WriteCallRefusal<P::Error>> {
        let prepared = slot
            .as_ref()
            .ok_or(WriteCallRefusal::Possession)?
            .prepare_assigned(
                &table,
                &claim,
                contract,
                selected.operation,
                Some(selected.fore),
            )?;
        let owner = slot.take().expect("preflighted retained Fore output owner");
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
