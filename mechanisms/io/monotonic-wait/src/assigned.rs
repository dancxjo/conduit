//! Transfer actual retained timer possession under a constrained assignment.
use super::*;
use conduit_plan_lowering::{
    selected_assigned_operation::{
        bind_selected_assigned_configured_operation, AssignedNativeIdentity,
        SelectedAssignedOperationPlan,
    },
    selected_operation::{SelectedCallRefusal, SelectedOperationContract},
};
impl<P: MonotonicWaitProvider> ReadyMonotonicWaitBase<P> {
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
        let owner = slot.as_ref().ok_or(WaitCallRefusal::Possession)?;
        let identity = &owner.identity;
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
        let kind = crate::owner::checked_contract(&table, &claim, owner.attachment, configuration)?;
        let node = bind_selected_assigned_configured_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: &kind,
                call: WAIT_HOST_CALL_CONTRACT,
                input_bytes: PARAMETER_BYTES,
                output_bytes: 0,
                resource_bytes: owner.attachment.resource_bytes,
            },
            configuration,
            AssignedNativeIdentity {
                capability: WAIT_CAPABILITY,
                execution_profile: WAIT_EXECUTION_PROFILE,
                artifact: identity.artifact_id.as_str(),
                base: identity.base_id.as_str(),
                base_implementation: identity.base_implementation_id.as_str(),
                mechanism: WAIT_ATTACHMENT_CLASS,
                authority: WAIT_AUTHORITY,
                resource_class: WAIT_ATTACHMENT_CLASS,
            },
            selected,
        )
        .map_err(|e| match e {
            SelectedCallRefusal::WrongBinding => WaitCallRefusal::WrongBinding,
            SelectedCallRefusal::Possession => WaitCallRefusal::Possession,
        })?;
        let owner = slot.take().expect("preflighted retained timer");
        Ok(MonotonicWaitHostCall::from_bound(
            table,
            handle,
            claim,
            owner.attachment,
            owner.provider,
            node,
        ))
    }
}
