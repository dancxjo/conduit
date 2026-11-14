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
pub struct PreparedAssignedSnapshot<'a, S> {
    slot: &'a mut Option<ReadySnapshotPublisher<S>>,
    operation: c::BoundBaseOperation,
    node: NodeId,
}
impl<S: NorStorage> PreparedAssignedSnapshot<'_, S> {
    pub fn node(&self) -> NodeId {
        self.node
    }
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.operation.retained_backing_bytes()
    }
    pub fn bind(self) -> SnapshotPublishHostCall<S> {
        SnapshotPublishHostCall {
            operation: self.operation,
            ready: self.slot.take().expect("exclusive prepared snapshot owner"),
            node: self.node,
            state: State::Ready,
        }
    }
}
impl<S: NorStorage> ReadySnapshotPublisher<S> {
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
    ) -> Result<PreparedAssignedSnapshot<'a, S>, SnapshotCallRefusal> {
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
    ) -> Result<PreparedAssignedSnapshot<'a, S>, SnapshotCallRefusal> {
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
    ) -> Result<PreparedAssignedSnapshot<'a, S>, SnapshotCallRefusal> {
        use SnapshotCallRefusal as E;
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
            || claim.parameter_bytes != crate::MAXIMUM_SNAPSHOT_BYTES as u32
            || claim.work_units < owner.maximum_publish_work()
            || kind.kind_id.as_str() != SNAPSHOT_KIND
            || kind.kind_contract_revision.as_str() != SNAPSHOT_REVISION
        {
            return Err(E::WrongBinding);
        }
        {
            let mut inspections = table.inspections();
            let scope = &inspections.next().ok_or(E::Possession)?.scope;
            if inspections.next().is_some()
                || scope.maximum_operations != 1
                || scope.maximum_in_flight != 1
                || scope.maximum_result_bytes
                    < conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32
                || scope.authority_grant_id != owner.authority().grant_id
            {
                return Err(E::Possession);
            }
        }
        if configuration.len() != 1 || configuration[0].key != "reference" {
            return Err(E::WrongBinding);
        }
        let c::ConfigurationValue::Text(text) = &configuration[0].value else {
            return Err(E::WrongBinding);
        };
        if text.len() != owner.reference.len() * 2 {
            return Err(E::WrongBinding);
        }
        for (pair, expected) in text
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .zip(&owner.reference)
        {
            let hex = b"0123456789abcdef";
            if pair
                != &[
                    hex[usize::from(expected >> 4)],
                    hex[usize::from(expected & 15)],
                ]
            {
                return Err(E::WrongBinding);
            }
        }
        let definition = SelectedOperationContract {
            kind,
            call: crate::PUBLISH_OPERATION,
            input_bytes: crate::MAXIMUM_SNAPSHOT_BYTES as u32,
            output_bytes: conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32,
            resource_bytes: owner.slot_bytes() as u64,
        };
        let native = AssignedNativeIdentity {
            capability: SNAPSHOT_CAPABILITY,
            execution_profile: SNAPSHOT_PROFILE,
            artifact: identity.artifact_id.as_str(),
            base: identity.base_id.as_str(),
            base_implementation: identity.base_implementation_id.as_str(),
            mechanism: SNAPSHOT_MECHANISM,
            authority: crate::AUTHORITY_CONTRACT,
            resource_class: SNAPSHOT_CLASS,
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
        Ok(PreparedAssignedSnapshot {
            slot,
            operation,
            node,
        })
    }
}
