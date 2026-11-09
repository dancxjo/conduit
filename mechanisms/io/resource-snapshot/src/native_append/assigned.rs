//! Exact compact/native binding against original local provider possession.
use super::*;
use conduit_plan_lowering::{
    selected_assigned_operation::{
        bind_selected_assigned_configured_operation,
        bind_selected_assigned_fore_configured_operation, AssignedNativeIdentity,
        SelectedAssignedForeOperationPlan, SelectedAssignedOperationPlan,
    },
    selected_operation::SelectedOperationContract,
};
pub struct PreparedAssignedAppender<'a, S> {
    slot: &'a mut Option<ReadyRecordAppender<S>>,
    operation: c::BoundBaseOperation,
    node: NodeId,
}
impl<S: NorStorage> PreparedAssignedAppender<'_, S> {
    pub fn node(&self) -> NodeId {
        self.node
    }
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.operation.retained_backing_bytes()
    }
    pub fn bind(self) -> RecordAppendHostCall<S> {
        RecordAppendHostCall {
            operation: self.operation,
            ready: self.slot.take().expect("exclusive prepared append owner"),
            node: self.node,
            state: State::Available,
            next_request: 0,
            custody: None,
            input: [0; MAXIMUM_RECORD_PAYLOAD_BYTES],
            input_len: 0,
            cached: None,
            write_failed: false,
        }
    }
}
impl<S: NorStorage> ReadyRecordAppender<S> {
    /// Independent checked Kind/configuration come from Root's immutable Source
    /// definition, never the received assignment. No rich Kind is retained.
    pub fn prepare_assigned_retained<'a>(
        slot: &'a mut Option<Self>,
        table: c::BaseCapabilityTable,
        handle: c::BaseCapabilityHandle,
        claim: c::BaseOperationClaim,
        kind: &c::Kind,
        configuration: &[c::ConfigurationEntry],
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<PreparedAssignedAppender<'a, S>, AppendCallRefusal> {
        Self::prepare_profile(
            slot,
            table,
            handle,
            claim,
            kind,
            configuration,
            selected,
            None,
        )
    }
    #[allow(clippy::too_many_arguments)] // Explicit Fore pins remain separate from received assignment.
    pub fn prepare_assigned_fore_retained<'a>(
        slot: &'a mut Option<Self>,
        table: c::BaseCapabilityTable,
        handle: c::BaseCapabilityHandle,
        claim: c::BaseOperationClaim,
        kind: &c::Kind,
        configuration: &[c::ConfigurationEntry],
        selected: SelectedAssignedForeOperationPlan<'_>,
    ) -> Result<PreparedAssignedAppender<'a, S>, AppendCallRefusal> {
        Self::prepare_profile(
            slot,
            table,
            handle,
            claim,
            kind,
            configuration,
            selected.operation,
            Some(selected.fore),
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn prepare_profile<'a>(
        slot: &'a mut Option<Self>,
        table: c::BaseCapabilityTable,
        handle: c::BaseCapabilityHandle,
        claim: c::BaseOperationClaim,
        kind: &c::Kind,
        configuration: &[c::ConfigurationEntry],
        selected: SelectedAssignedOperationPlan<'_>,
        fore: Option<&[c::AssignedForeRequirement]>,
    ) -> Result<PreparedAssignedAppender<'a, S>, AppendCallRefusal> {
        use AppendCallRefusal as E;
        let owner = slot.as_ref().ok_or(E::Possession)?;
        let identity = &owner.identity;
        if claim.host_id != identity.host_id
            || claim.boot_id != identity.boot_id
            || claim.base_instance_id != identity.provider_instance_id
            || claim.base_provider_generation != identity.provider_generation
            || claim.resource_pool_id != identity.resource_pool_id
            || claim.resource_generation_id != identity.resource_generation_id
            || claim.envelope_id != identity.envelope_id
            || claim.implementation_id != identity.implementation_id
            || claim.parameter_bytes != MAXIMUM_RECORD_PAYLOAD_BYTES as u32
            || claim.work_units < u64::from(owner.attachment().maximum_append_media_calls)
            || kind.kind_id.as_str() != APPEND_KIND
            || kind.kind_contract_revision.as_str() != APPEND_REVISION
        {
            return Err(E::WrongBinding);
        }
        {
            let mut inspections = table.inspections();
            let scope = &inspections.next().ok_or(E::Possession)?.scope;
            if inspections.next().is_some()
                || scope.maximum_operations == 0
                || scope.maximum_operations > owner.attachment().maximum_operations
                || scope.maximum_in_flight != 1
                || scope.maximum_result_bytes != APPEND_RESULT_BYTES
                || scope.authority_grant_id != owner.original.grant_id
            {
                return Err(E::Possession);
            }
        }
        if !configuration.is_empty() {
            return Err(E::WrongBinding);
        }
        let scope = &table.inspections().next().ok_or(E::Possession)?.scope;
        if scope.authority_contract_id != owner.original.contract_id
            || scope.operation_contract_id != owner.original.host_call_contract_id
            || scope.subject_kind != owner.original.subject_kind
            || scope.capability_id != owner.original.capability_id
        {
            return Err(E::Possession);
        }
        let definition = SelectedOperationContract {
            kind,
            call: APPEND_OPERATION,
            input_bytes: MAXIMUM_RECORD_PAYLOAD_BYTES as u32,
            output_bytes: APPEND_RESULT_BYTES,
            resource_bytes: u64::from(owner.attachment().resource_bytes),
        };
        let native = AssignedNativeIdentity {
            capability: APPEND_CAPABILITY,
            execution_profile: APPEND_PROFILE,
            artifact: identity.artifact_id.as_str(),
            base: identity.base_id.as_str(),
            base_implementation: identity.base_implementation_id.as_str(),
            mechanism: APPEND_MECHANISM,
            authority: APPEND_AUTHORITY,
            resource_class: APPEND_CLASS,
        };
        let node = match fore {
            None => bind_selected_assigned_configured_operation(
                &table,
                &claim,
                definition,
                configuration,
                native,
                selected,
            ),
            Some(fore) => bind_selected_assigned_fore_configured_operation(
                &table,
                &claim,
                definition,
                configuration,
                native,
                SelectedAssignedForeOperationPlan {
                    operation: selected,
                    fore,
                },
            ),
        }
        .map_err(|_| E::WrongBinding)?;
        let operation = c::BoundBaseOperation::bind(table, handle, claim)
            .map_err(|(_, _, _, reason)| E::Capability(reason))?;
        Ok(PreparedAssignedAppender {
            slot,
            operation,
            node,
        })
    }
}
