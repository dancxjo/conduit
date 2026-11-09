//! Exact assigned preparation retains Root's capture owner until complete admission.
use super::*;
use crate::owner::{check_possession, PreparedPulseBinding};
use conduit_core::BoundBaseOperation;
use conduit_plan_lowering::{
    selected_assigned_operation::{
        bind_selected_assigned_fore_operation, bind_selected_assigned_operation,
        AssignedNativeIdentity, SelectedAssignedForeOperationPlan, SelectedAssignedOperationPlan,
    },
    selected_operation::{SelectedCallRefusal, SelectedOperationContract},
};
/// Exclusive preparation custody, not resource admission or permission to Start.
/// Dropping preparation leaves the exact initialized capture provider in its slot.
pub struct PreparedAssignedPulse<'a, P: PulseReadProvider> {
    prepared: PreparedPulseBinding,
    operation: BoundBaseOperation,
    slot: &'a mut Option<ReadyPulseBase<P>>,
}
impl<P: PulseReadProvider> PreparedAssignedPulse<'_, P> {
    /// Codec and bound capability backing only. Inline/allocator overhead,
    /// installation and independently supplied preparation tables are separate.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.prepared
            .codec
            .retained_backing_bytes()?
            .checked_add(self.operation.retained_backing_bytes()?)
    }
    pub fn node(&self) -> conduit_kernel::NodeId {
        self.prepared.node
    }
    /// Root must independently admit the whole workset before this transfer.
    /// No allocation, electrical effect, or execution authority is added here.
    #[inline(always)] // Fuse the affine owner transfer with its final destination.
    pub fn bind(self) -> PulseHostCall<P> {
        let owner = self.slot.take().expect("exclusively prepared pulse owner");
        PulseHostCall::from_prepared(
            self.operation,
            owner.attachment,
            owner.provider,
            self.prepared,
        )
    }
}
impl<P: PulseReadProvider> ReadyPulseBase<P> {
    pub fn check_assigned(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &PulseReadContract,
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<conduit_kernel::NodeId, PulseCallRefusal> {
        self.prepare_assigned(table, claim, contract, selected, None)
            .map(|p| p.node)
    }
    fn prepare_assigned(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &PulseReadContract,
        selected: SelectedAssignedOperationPlan<'_>,
        fore: Option<&[AssignedForeRequirement]>,
    ) -> Result<PreparedPulseBinding, PulseCallRefusal> {
        let id = &self.identity;
        if claim.host_id != id.host_id
            || claim.boot_id != id.boot_id
            || claim.base_instance_id != id.provider_instance_id
            || claim.base_provider_generation != id.provider_generation
            || claim.resource_pool_id != id.resource_pool_id
            || claim.resource_generation_id != id.resource_generation_id
            || claim.envelope_id != id.envelope_id
            || claim.implementation_id != id.implementation_id
        {
            return Err(PulseCallRefusal::WrongBinding);
        }
        check_possession(table, claim, self.attachment)?;
        let definition = SelectedOperationContract {
            kind: contract.kind(),
            call: PULSE_READ_CALL,
            input_bytes: PULSE_READ_MAXIMUM_BYTES,
            output_bytes: PULSE_READ_MAXIMUM_BYTES,
            resource_bytes: self.attachment.resource_bytes,
        };
        let native = AssignedNativeIdentity {
            capability: PULSE_CAPABILITY,
            execution_profile: PULSE_EXECUTION_PROFILE,
            artifact: id.artifact_id.as_str(),
            base: id.base_id.as_str(),
            base_implementation: id.base_implementation_id.as_str(),
            mechanism: PULSE_ATTACHMENT_CLASS,
            authority: PULSE_AUTHORITY,
            resource_class: PULSE_ATTACHMENT_CLASS,
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
        .map_err(|e| match e {
            SelectedCallRefusal::WrongBinding => PulseCallRefusal::WrongBinding,
            SelectedCallRefusal::Possession => PulseCallRefusal::Possession,
        })?;
        Ok(PreparedPulseBinding {
            node,
            codec: PreparedPulseCodec::prepare(contract).map_err(PulseCallRefusal::Canonical)?,
        })
    }
    /// All fallible checks and codec preparation precede withdrawal of possession.
    pub fn prepare_assigned_retained<'a>(
        slot: &'a mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &PulseReadContract,
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<PreparedAssignedPulse<'a, P>, PulseCallRefusal> {
        let prepared = slot
            .as_ref()
            .ok_or(PulseCallRefusal::Possession)?
            .prepare_assigned(&table, &claim, contract, selected, None)?;
        let operation = BoundBaseOperation::bind(table, handle, claim)
            .map_err(|(_, _, _, reason)| PulseCallRefusal::Capability(reason))?;
        Ok(PreparedAssignedPulse {
            prepared,
            operation,
            slot,
        })
    }

    /// Exact local Fore pins are checked before the capture owner is withdrawn.
    pub fn prepare_assigned_fore_retained<'a>(
        slot: &'a mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &PulseReadContract,
        selected: SelectedAssignedForeOperationPlan<'_>,
    ) -> Result<PreparedAssignedPulse<'a, P>, PulseCallRefusal> {
        let prepared = slot
            .as_ref()
            .ok_or(PulseCallRefusal::Possession)?
            .prepare_assigned(
                &table,
                &claim,
                contract,
                selected.operation,
                Some(selected.fore),
            )?;
        let operation = BoundBaseOperation::bind(table, handle, claim)
            .map_err(|(_, _, _, reason)| PulseCallRefusal::Capability(reason))?;
        Ok(PreparedAssignedPulse {
            prepared,
            operation,
            slot,
        })
    }
    /// Refusal preserves the initialized owner. Success transfers it exactly once.
    pub fn bind_assigned_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &PulseReadContract,
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<PulseHostCall<P>, PulseCallRefusal> {
        Ok(Self::prepare_assigned_retained(slot, table, handle, claim, contract, selected)?.bind())
    }
}
