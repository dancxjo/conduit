//! Exact selected possession and one supervised join operation.
use crate::{AttachmentRecord, NetworkJoinProvider, PreparedJoinContract};
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_net::*;
use conduit_plan_lowering::{
    selected_assigned_operation::{
        bind_selected_assigned_operation_for_subject, AssignedNativeIdentity,
        SelectedAssignedOperationPlan,
    },
    selected_operation::{
        bind_selected_operation_for_subject, SelectedCallRefusal, SelectedOperationPlan,
    },
};

#[derive(Debug, PartialEq, Eq)]
pub enum JoinCallRefusal {
    OwnerUnavailable,
    Binding(SelectedCallRefusal),
    Possession,
    WrongBinding,
    StaleRequest,
    Request(NetworkJoinError),
    Provider(StationAttemptFailure),
    InvalidAttachment,
    Capability(BaseCapabilityRefusal),
}
pub struct AssignedJoinSelection<'a> {
    pub contract: &'a PreparedJoinContract,
    pub native: AssignedNativeIdentity<'a>,
    pub plan: SelectedAssignedOperationPlan<'a>,
}
pub struct NetworkJoinHostCall<P> {
    operation: BoundBaseOperation,
    provider: P,
    node: NodeId,
    generation: u64,
    invoked: bool,
    cleanup_required: bool,
    output: Option<AttachmentRecord>,
}
impl<P: NetworkJoinProvider> NetworkJoinHostCall<P> {
    /// # Safety
    /// Root must own this actual provider and exact Base/resource generation,
    /// independently admit the Plan/Play, reserve the nonzero attachment
    /// generation, and admit all SDK work/storage and cleanup deadlines. The
    /// provider must publish attachment only after actual link/address success.
    /// This cooperative boundary does not confine hostile native code.
    pub unsafe fn bind_selected(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        provider: P,
        generation: u64,
        contract: &PreparedJoinContract,
        selected: SelectedOperationPlan<'_>,
    ) -> Result<Self, JoinCallRefusal> {
        let node = Self::validate_selected(&table, &claim, generation, contract, selected)?;
        BoundBaseOperation::check(&table, &handle, &claim).map_err(JoinCallRefusal::Capability)?;
        Ok(Self::bound(
            table, handle, claim, provider, generation, node,
        ))
    }
    pub(crate) fn validate_selected(
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        generation: u64,
        contract: &PreparedJoinContract,
        selected: SelectedOperationPlan<'_>,
    ) -> Result<NodeId, JoinCallRefusal> {
        Self::check_possession(table, claim, generation)?;
        let gear = selected
            .fragment
            .placements
            .iter()
            .find(|g| &g.placement_id == selected.placement_id)
            .ok_or(JoinCallRefusal::WrongBinding)?;
        let [resource] = gear.resources.as_slice() else {
            return Err(JoinCallRefusal::WrongBinding);
        };
        if resource.class_id.as_str() != WIFI_STATION_RESOURCE_CLASS || resource.units != 1 {
            return Err(JoinCallRefusal::WrongBinding);
        }
        let node = bind_selected_operation_for_subject(
            table,
            claim,
            contract.definition(),
            &contract.subject,
            selected,
        )
        .map_err(JoinCallRefusal::Binding)?;
        Ok(node)
    }
    pub(crate) fn validate_direct(
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        generation: u64,
        contract: &PreparedJoinContract,
        selected: conduit_plan_lowering::selected_operation::SelectedDirectOperationPlan<
            '_,
            3,
            2,
            1,
            3,
            2,
            3,
        >,
    ) -> Result<NodeId, JoinCallRefusal> {
        Self::check_possession(table, claim, generation)?;
        let (_, fragment) = selected.tables.preparation_scope();
        let gear = fragment
            .placements
            .iter()
            .find(|g| &g.placement_id == selected.placement_id)
            .ok_or(JoinCallRefusal::WrongBinding)?;
        let [resource] = gear.resources.as_slice() else {
            return Err(JoinCallRefusal::WrongBinding);
        };
        if resource.class_id.as_str() != WIFI_STATION_RESOURCE_CLASS || resource.units != 1 {
            return Err(JoinCallRefusal::WrongBinding);
        }
        let node =
            conduit_plan_lowering::selected_operation::bind_selected_direct_operation_for_subject(
                table,
                claim,
                contract.definition(),
                &contract.subject,
                selected,
            )
            .map_err(JoinCallRefusal::Binding)?;
        Ok(node)
    }
    pub(crate) fn validate_flat_direct(
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        generation: u64,
        contract: &CheckedNetworkContract,
        selected: conduit_plan_lowering::selected_operation::SelectedFlatDirectOperationPlan<
            '_,
            3,
            2,
            1,
            3,
            2,
            3,
        >,
    ) -> Result<NodeId, JoinCallRefusal> {
        Self::check_possession(table, claim, generation)?;
        let view = selected.tables.preparation_scope().view();
        let gear = view
            .placements
            .iter()
            .find(|g| g.placement_id == selected.placement_id)
            .ok_or(JoinCallRefusal::WrongBinding)?;
        let mut resources = gear.resources.iter();
        let resource = resources.next().ok_or(JoinCallRefusal::WrongBinding)?;
        if resources.next().is_some() {
            return Err(JoinCallRefusal::WrongBinding);
        };
        if resource.class_id != WIFI_STATION_RESOURCE_CLASS || resource.units != 1 {
            return Err(JoinCallRefusal::WrongBinding);
        }
        let node =
            conduit_plan_lowering::selected_operation::bind_selected_flat_direct_operation_for_subject(
                table,
                claim,
                conduit_plan_lowering::selected_operation::SelectedOperationContractRef{kind:contract.kind(),semantic_contract:contract.semantic_contract(),call:NETWORK_JOIN_HOST_CALL,input_bytes:MAXIMUM_JOIN_INPUT_BYTES,output_bytes:MAXIMUM_JOIN_OUTPUT_BYTES,resource_bytes:1},
                NETWORK_CONFIG_SUBJECT,
                &[],
                selected,
            )
            .map_err(JoinCallRefusal::Binding)?;
        Ok(node)
    }

    /// # Safety
    /// Same ownership, admission and cleanup requirements as `bind_selected`.
    /// Native identity and resident requirements must be retained independently
    /// by Root, never inferred from the incoming assignment.
    pub unsafe fn bind_assigned(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        provider: P,
        generation: u64,
        selection: AssignedJoinSelection<'_>,
    ) -> Result<Self, JoinCallRefusal> {
        Self::check_possession(&table, &claim, generation)?;
        if selection.native.resource_class != WIFI_STATION_RESOURCE_CLASS {
            return Err(JoinCallRefusal::WrongBinding);
        }
        let node = bind_selected_assigned_operation_for_subject(
            &table,
            &claim,
            selection.contract.definition(),
            &selection.contract.subject,
            selection.native,
            selection.plan,
        )
        .map_err(JoinCallRefusal::Binding)?;
        BoundBaseOperation::check(&table, &handle, &claim).map_err(JoinCallRefusal::Capability)?;
        Ok(Self::bound(
            table, handle, claim, provider, generation, node,
        ))
    }
    pub(crate) fn bound(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        provider: P,
        generation: u64,
        node: NodeId,
    ) -> Self {
        Self::bound_operation(
            BoundBaseOperation::bind(table, handle, claim)
                .unwrap_or_else(|_| panic!("preflighted actual bearer")),
            provider,
            generation,
            node,
        )
    }
    pub(crate) fn bound_operation(
        operation: BoundBaseOperation,
        provider: P,
        generation: u64,
        node: NodeId,
    ) -> Self {
        Self {
            operation,
            provider,
            generation,
            node,
            invoked: false,
            cleanup_required: false,
            output: None,
        }
    }
    fn check_possession(
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        generation: u64,
    ) -> Result<(), JoinCallRefusal> {
        let mut entries = table.inspections();
        let entry = entries.next().ok_or(JoinCallRefusal::Possession)?;
        if entries.next().is_some()
            || generation == 0
            || entry.scope.maximum_operations != 1
            || entry.scope.maximum_in_flight != 1
            || entry.scope.maximum_parameter_bytes < MAXIMUM_JOIN_INPUT_BYTES
            || entry.scope.maximum_result_bytes < MAXIMUM_JOIN_OUTPUT_BYTES
            || entry.scope.authority_contract_id.as_str() != NETWORK_CONFIG_AUTHORITY
            || claim.parameter_bytes != MAXIMUM_JOIN_INPUT_BYTES
            || claim.work_units != 1
        {
            return Err(JoinCallRefusal::Possession);
        }
        Ok(())
    }
    pub async fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<&[u8], JoinCallRefusal> {
        self.check_binding(node, call)?;
        if request != RequestId(0) || self.invoked {
            return Err(JoinCallRefusal::StaleRequest);
        }
        let request = decode_network_join_request(input).map_err(JoinCallRefusal::Request)?;
        let lease = self
            .operation
            .authorize()
            .map_err(JoinCallRefusal::Capability)?;
        self.invoked = true;
        self.cleanup_required = true;
        self.output = None;
        // Provider future is created after this guard: it drops/quiesces first.
        let lease = crate::lease::Pending::new(&mut self.operation, lease);
        let result = match self.provider.join(request).await {
            Ok(record) => {
                let info =
                    decode_network_attachment(record.bytes()).map_err(JoinCallRefusal::Request)?;
                if info.host_id != lease.scope().host_id.as_str()
                    || info.boot_id != lease.scope().boot_id.as_str()
                    || info.resource_pool_id != lease.scope().resource_pool_id.as_str()
                    || info.generation != self.generation
                {
                    Err(JoinCallRefusal::InvalidAttachment)
                } else {
                    self.output = Some(record);
                    Ok(())
                }
            }
            Err(error) => Err(JoinCallRefusal::Provider(error)),
        };
        let bytes = if result.is_ok() {
            self.output.as_ref().unwrap().bytes().len() as u32
        } else {
            0
        };
        lease.complete(bytes).map_err(JoinCallRefusal::Capability)?;
        result?;
        Ok(self.output.as_ref().unwrap().bytes())
    }
    /// Safety cleanup, never a new join or a claim of restored attachment.
    /// Failure/cancellation leaves the obligation set; Root must supervise it.
    pub async fn cleanup(&mut self) -> Result<(), StationFailure> {
        if self.cleanup_required {
            self.provider.cleanup().await?;
            self.cleanup_required = false;
        }
        Ok(())
    }
    pub fn cleanup_required(&self) -> bool {
        self.cleanup_required
    }
    /// Return the actual provider only after its cleanup obligation completed
    /// and exact selected possession was revoked without in-flight work.
    /// Refusal preserves the complete owner. The returned provider remains
    /// spent; this does not reset its request, attachment or native generations.
    pub fn retire_retained(slot: &mut Option<Self>) -> Result<P, JoinCallRefusal> {
        let owner = slot.as_ref().ok_or(JoinCallRefusal::OwnerUnavailable)?;
        if !owner.retirement_ready() {
            return Err(JoinCallRefusal::Possession);
        }
        Ok(slot.take().expect("preflighted retained owner").provider)
    }
    pub fn retirement_ready(&self) -> bool {
        let inspection = self.inspection();
        !self.cleanup_required
            && inspection.lifecycle == CapabilityLifecycle::Revoked
            && inspection.in_flight_operations == 0
    }
    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), JoinCallRefusal> {
        self.check_binding(node, call)?;
        self.operation.revoke().map_err(JoinCallRefusal::Capability)
    }
    pub fn inspection(&self) -> &CapabilityInspection {
        self.operation.inspection()
    }
    /// Read-only observation of the actual retained native owner. This cannot
    /// invoke, replace or rearm the selected provider or manufacture attachment.
    pub fn observe_provider<R>(&self, observe: impl FnOnce(&P) -> R) -> R {
        observe(&self.provider)
    }
    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), JoinCallRefusal> {
        if node != self.node || call != HostCallId(0) {
            Err(JoinCallRefusal::WrongBinding)
        } else {
            Ok(())
        }
    }
}
