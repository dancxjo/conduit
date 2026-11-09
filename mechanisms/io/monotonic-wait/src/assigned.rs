//! Transfer actual retained timer possession under a constrained assignment.
use super::*;
use conduit_core::BoundBaseOperation;
use conduit_plan_lowering::{
    selected_assigned_operation::{
        bind_selected_assigned_configured_operation,
        bind_selected_assigned_fore_configured_operation, AssignedNativeIdentity,
        SelectedAssignedForeOperationPlan, SelectedAssignedOperationPlan,
    },
    selected_operation::{SelectedCallRefusal, SelectedOperationContract},
};
impl<P: MonotonicWaitProvider> ReadyMonotonicWaitBase<P> {
    /// Check retained identity and attachment without transferring the timer,
    /// dispatching a wait or consuming a lease.
    pub fn check_assigned(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        configuration: &[ConfigurationEntry],
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<conduit_kernel::NodeId, WaitCallRefusal> {
        self.check_assigned_profile(table, claim, configuration, selected, None)
    }
    pub fn check_assigned_fore(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        configuration: &[ConfigurationEntry],
        selected: SelectedAssignedForeOperationPlan<'_>,
    ) -> Result<conduit_kernel::NodeId, WaitCallRefusal> {
        self.check_assigned_profile(
            table,
            claim,
            configuration,
            selected.operation,
            Some(selected.fore),
        )
    }
    fn check_assigned_profile(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        configuration: &[ConfigurationEntry],
        selected: SelectedAssignedOperationPlan<'_>,
        fore: Option<&[AssignedForeRequirement]>,
    ) -> Result<conduit_kernel::NodeId, WaitCallRefusal> {
        let identity = &self.identity;
        if claim.host_id != identity.host_id
            || claim.boot_id != identity.boot_id
            || claim.base_instance_id != identity.provider_instance_id
            || claim.base_provider_generation != identity.provider_generation
            || claim.resource_pool_id != identity.resource_pool_id
            || claim.resource_generation_id != identity.resource_generation_id
            || claim.envelope_id != identity.envelope_id
            || claim.implementation_id != identity.implementation_id
        {
            return Err(WaitCallRefusal::WrongBinding);
        }
        let kind = crate::owner::checked_contract(table, claim, self.attachment, configuration)?;
        let definition = SelectedOperationContract {
            kind: &kind,
            call: WAIT_HOST_CALL_CONTRACT,
            input_bytes: PARAMETER_BYTES,
            output_bytes: 0,
            resource_bytes: self.attachment.resource_bytes,
        };
        let native = AssignedNativeIdentity {
            capability: WAIT_CAPABILITY,
            execution_profile: WAIT_EXECUTION_PROFILE,
            artifact: identity.artifact_id.as_str(),
            base: identity.base_id.as_str(),
            base_implementation: identity.base_implementation_id.as_str(),
            mechanism: WAIT_ATTACHMENT_CLASS,
            authority: WAIT_AUTHORITY,
            resource_class: WAIT_ATTACHMENT_CLASS,
        };
        match fore {
            None => bind_selected_assigned_configured_operation(
                table,
                claim,
                definition,
                configuration,
                native,
                selected,
            ),
            Some(fore) => bind_selected_assigned_fore_configured_operation(
                table,
                claim,
                definition,
                configuration,
                native,
                SelectedAssignedForeOperationPlan {
                    operation: selected,
                    fore,
                },
            ),
        }
        .map_err(|e| match e {
            SelectedCallRefusal::WrongBinding => WaitCallRefusal::WrongBinding,
            SelectedCallRefusal::Possession => WaitCallRefusal::Possession,
        })
    }
    /// Refusal preserves Root's initialized slot. Expected configuration comes
    /// from independently checked Source; opaque authority must be locally issued.
    pub fn bind_assigned_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        configuration: &[ConfigurationEntry],
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<MonotonicWaitHostCall<P>, WaitCallRefusal> {
        let node = slot
            .as_ref()
            .ok_or(WaitCallRefusal::Possession)?
            .check_assigned(&table, &claim, configuration, selected)?;
        let operation = BoundBaseOperation::bind(table, handle, claim)
            .map_err(|(_, _, _, reason)| WaitCallRefusal::Capability(reason))?;
        let owner = slot.take().expect("preflighted retained timer");
        Ok(MonotonicWaitHostCall::from_bound(
            operation,
            owner.attachment,
            owner.provider,
            node,
        ))
    }
}

/// Exclusive prepared timer custody. Dropping preserves the original ready slot.
pub struct PreparedAssignedWait<'a, P: MonotonicWaitProvider> {
    slot: &'a mut Option<ReadyMonotonicWaitBase<P>>,
    operation: BoundBaseOperation,
    node: conduit_kernel::NodeId,
}
impl<P: MonotonicWaitProvider> PreparedAssignedWait<'_, P> {
    pub fn node(&self) -> conduit_kernel::NodeId {
        self.node
    }
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.operation.retained_backing_bytes()
    }
    #[inline(always)] // Fuse the affine owner transfer with its final destination.
    pub fn bind(self) -> MonotonicWaitHostCall<P> {
        let owner = self.slot.take().expect("exclusively prepared wait owner");
        MonotonicWaitHostCall::from_bound(
            self.operation,
            owner.attachment,
            owner.provider,
            self.node,
        )
    }
}
impl<P: MonotonicWaitProvider> ReadyMonotonicWaitBase<P> {
    pub fn prepare_assigned_retained<'a>(
        slot: &'a mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        configuration: &[ConfigurationEntry],
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<PreparedAssignedWait<'a, P>, WaitCallRefusal> {
        let node = slot
            .as_ref()
            .ok_or(WaitCallRefusal::Possession)?
            .check_assigned(&table, &claim, configuration, selected)?;
        let operation = BoundBaseOperation::bind(table, handle, claim)
            .map_err(|(_, _, _, reason)| WaitCallRefusal::Capability(reason))?;
        Ok(PreparedAssignedWait {
            slot,
            operation,
            node,
        })
    }
    /// Checks exact local Fore and configuration before lending the owner.
    pub fn prepare_assigned_fore_retained<'a>(
        slot: &'a mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        configuration: &[ConfigurationEntry],
        selected: SelectedAssignedForeOperationPlan<'_>,
    ) -> Result<PreparedAssignedWait<'a, P>, WaitCallRefusal> {
        let node = slot
            .as_ref()
            .ok_or(WaitCallRefusal::Possession)?
            .check_assigned_fore(&table, &claim, configuration, selected)?;
        let operation = BoundBaseOperation::bind(table, handle, claim)
            .map_err(|(_, _, _, reason)| WaitCallRefusal::Capability(reason))?;
        Ok(PreparedAssignedWait {
            slot,
            operation,
            node,
        })
    }
}
