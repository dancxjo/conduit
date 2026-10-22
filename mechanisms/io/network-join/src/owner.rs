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
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
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
        Self::check_possession(&table, &claim, generation)?;
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
            &table,
            &claim,
            contract.definition(),
            &contract.subject,
            selected,
        )
        .map_err(JoinCallRefusal::Binding)?;
        Ok(Self::bound(
            table, handle, claim, provider, generation, node,
        ))
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
        Ok(Self::bound(
            table, handle, claim, provider, generation, node,
        ))
    }
    fn bound(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        provider: P,
        generation: u64,
        node: NodeId,
    ) -> Self {
        Self {
            table,
            handle,
            claim,
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
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(JoinCallRefusal::Capability)?;
        self.invoked = true;
        self.cleanup_required = true;
        self.output = None;
        // Provider future is created after this guard: it drops/quiesces first.
        let lease = crate::lease::Pending::new(&mut self.table, &mut self.handle, lease);
        let result = match self.provider.join(request).await {
            Ok(record) => {
                let info =
                    decode_network_attachment(record.bytes()).map_err(JoinCallRefusal::Request)?;
                if info.host_id != self.claim.host_id.as_str()
                    || info.boot_id != self.claim.boot_id.as_str()
                    || info.resource_pool_id != self.claim.resource_pool_id.as_str()
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
    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), JoinCallRefusal> {
        self.check_binding(node, call)?;
        self.table
            .revoke(&self.handle)
            .map_err(JoinCallRefusal::Capability)
    }
    pub fn inspection(&self) -> &CapabilityInspection {
        self.table
            .inspections()
            .next()
            .expect("single admitted capability")
    }
    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), JoinCallRefusal> {
        if node != self.node || call != HostCallId(0) {
            Err(JoinCallRefusal::WrongBinding)
        } else {
            Ok(())
        }
    }
}
