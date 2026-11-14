//! Exact constrained assignment preflight before original clock custody moves.
use super::{codec::PreparedClockCodec, contract::*, owner::*, retained::*};
use conduit_core::*;
use conduit_kernel::NodeId;
use conduit_plan_lowering::{
    selected_assigned_operation::{
        bind_selected_assigned_fore_operation, bind_selected_assigned_operation,
        AssignedNativeIdentity, SelectedAssignedForeOperationPlan, SelectedAssignedOperationPlan,
    },
    selected_operation::{SelectedCallRefusal, SelectedOperationContract},
};

impl<P: MonotonicDeadlineProvider> ReadyClockAtBase<P> {
    pub fn check_assigned(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &MonotonicClockContract,
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<NodeId, ClockCallRefusal> {
        self.check_profile(table, claim, contract, selected, None)
    }
    pub fn check_assigned_fore(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &MonotonicClockContract,
        selected: SelectedAssignedForeOperationPlan<'_>,
    ) -> Result<NodeId, ClockCallRefusal> {
        self.check_profile(
            table,
            claim,
            contract,
            selected.operation,
            Some(selected.fore),
        )
    }
    fn check_profile(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &MonotonicClockContract,
        selected: SelectedAssignedOperationPlan<'_>,
        fore: Option<&[AssignedForeRequirement]>,
    ) -> Result<NodeId, ClockCallRefusal> {
        checked_possession(table, claim)?;
        let i = &self.identity;
        let scope = &table
            .inspections()
            .next()
            .ok_or(ClockCallRefusal::Possession)?
            .scope;
        if claim.host_id != i.host_id
            || claim.boot_id != i.boot_id
            || claim.base_instance_id != i.provider_instance_id
            || claim.base_provider_generation != i.provider_generation
            || claim.resource_pool_id != i.resource_pool_id
            || claim.resource_generation_id != i.resource_generation_id
            || claim.envelope_id != i.envelope_id
            || claim.implementation_id != i.implementation_id
            || scope.capability_id != i.capability_id
            || scope.authority_contract_id != i.authority_contract_id
        {
            return Err(ClockCallRefusal::WrongBinding);
        }
        let definition = SelectedOperationContract {
            kind: contract.kind(),
            call: CLOCK_CALL,
            input_bytes: CLOCK_MAXIMUM_BYTES,
            output_bytes: CLOCK_MAXIMUM_BYTES,
            resource_bytes: self.attachment.resource_bytes,
        };
        let native = AssignedNativeIdentity {
            capability: i.capability_id.as_str(),
            execution_profile: i.execution_profile_id.as_str(),
            artifact: i.artifact_id.as_str(),
            base: i.base_id.as_str(),
            base_implementation: i.base_implementation_id.as_str(),
            mechanism: &i.mechanism_family,
            authority: i.authority_contract_id.as_str(),
            resource_class: &i.resource_class,
        };
        match fore {
            None => bind_selected_assigned_operation(table, claim, definition, native, selected),
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
        }
        .map_err(|e| match e {
            SelectedCallRefusal::WrongBinding => ClockCallRefusal::WrongBinding,
            SelectedCallRefusal::Possession => ClockCallRefusal::Possession,
        })
    }
    /// Root must charge codec/provider/identity/table storage before preparation.
    /// A refused or unused ticket leaves the same initialized provider in slot.
    pub fn prepare_assigned_retained<'a>(
        slot: &'a mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &MonotonicClockContract,
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<PreparedAssignedClockAt<'a, P>, ClockCallRefusal> {
        let node = slot
            .as_ref()
            .ok_or(ClockCallRefusal::Possession)?
            .check_assigned(&table, &claim, contract, selected)?;
        Self::prepare_checked(slot, table, handle, claim, contract, node)
    }
    pub fn prepare_assigned_fore_retained<'a>(
        slot: &'a mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &MonotonicClockContract,
        selected: SelectedAssignedForeOperationPlan<'_>,
    ) -> Result<PreparedAssignedClockAt<'a, P>, ClockCallRefusal> {
        let node = slot
            .as_ref()
            .ok_or(ClockCallRefusal::Possession)?
            .check_assigned_fore(&table, &claim, contract, selected)?;
        Self::prepare_checked(slot, table, handle, claim, contract, node)
    }
    fn prepare_checked<'a>(
        slot: &'a mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &MonotonicClockContract,
        node: NodeId,
    ) -> Result<PreparedAssignedClockAt<'a, P>, ClockCallRefusal> {
        let codec = PreparedClockCodec::new(contract).map_err(ClockCallRefusal::Canonical)?;
        let operation = BoundBaseOperation::bind(table, handle, claim)
            .map_err(|(_, _, _, reason)| ClockCallRefusal::Capability(reason))?;
        Ok(PreparedAssignedClockAt {
            slot,
            operation,
            node,
            codec,
        })
    }
    pub fn bind_assigned_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &MonotonicClockContract,
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<MonotonicClockHostCall<P>, ClockCallRefusal> {
        Self::prepare_assigned_retained(slot, table, handle, claim, contract, selected)
            .map(PreparedAssignedClockAt::bind)
    }
}

/// Exclusive preflight loan. Dropping cannot take or replace Root's provider.
pub struct PreparedAssignedClockAt<'a, P: MonotonicDeadlineProvider> {
    slot: &'a mut Option<ReadyClockAtBase<P>>,
    operation: BoundBaseOperation,
    node: NodeId,
    codec: PreparedClockCodec,
}
impl<P: MonotonicDeadlineProvider> PreparedAssignedClockAt<'_, P> {
    /// Reachable owned operation and codec backing. The borrowed ready
    /// provider, inline ticket and allocator overhead are funded separately.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.operation
            .retained_backing_bytes()?
            .checked_add(self.codec.retained_backing_bytes()?)
    }

    pub fn node(&self) -> NodeId {
        self.node
    }
    /// Allocation-free transfer of the original provider and prepared codecs.
    pub fn bind(self) -> MonotonicClockHostCall<P> {
        let ready = self.slot.take().expect("exclusively prepared clock owner");
        MonotonicClockHostCall::from_bound(self.operation, ready.provider, self.node, self.codec)
    }
}
