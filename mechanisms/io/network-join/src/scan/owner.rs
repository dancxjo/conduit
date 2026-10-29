use super::*;
use crate::installation::NetworkNativeIdentity;
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::selected_operation::{
    bind_selected_operation_for_subject, SelectedCallRefusal, SelectedOperationPlan,
};
pub const SCAN_CAPABILITY: &str = "conduit/network-scan@1";
pub const SCAN_BASE_KIND: &str = "machine/wifi-scan";
/// Core requires one nonzero accounting unit even for a zero-input Host Call,
/// as for the existing credential source. This is not an authored input byte.
pub const SCAN_OPERATION_PARAMETER_BYTES: u32 = 1;
#[derive(Debug, PartialEq, Eq)]
pub enum ScanCallRefusal {
    Unavailable,
    InvalidIdentity,
    WrongBinding,
    StaleRequest,
    Deadline,
    Representation(ScanFailure),
    Provider(ScanFailure),
    Binding(SelectedCallRefusal),
    Capability(BaseCapabilityRefusal),
    Retirement,
}
/// Unselected actual provider. Descriptors alone cannot construct this owner.
pub struct ReadyNetworkScanBase<P> {
    identity: NetworkNativeIdentity,
    request: NetworkScanRequest,
    provider: P,
}
impl<P: NetworkScanProvider> ReadyNetworkScanBase<P> {
    /// # Safety
    /// Caller exclusively owns this actual initialized provider/controller and
    /// persistent event subscription. Its SDK storage, absolute deadlines and
    /// service transitions already have finite independent admission. No other
    /// join/scan may share this radio until real retirement returns it.
    #[allow(
        clippy::result_large_err,
        reason = "Refusal returns the actual affine initialized provider and identity without allocation"
    )]
    pub unsafe fn new(
        identity: NetworkNativeIdentity,
        request: NetworkScanRequest,
        provider: P,
    ) -> Result<Self, (ScanCallRefusal, NetworkNativeIdentity, P)> {
        if request.validate().is_err()
            || identity.provider_generation == 0
            || [
                identity.host.as_str(),
                identity.boot.as_str(),
                identity.base.as_str(),
                identity.instance.as_str(),
                identity.resource.as_str(),
                &identity.resource_generation.0,
                identity.envelope.as_str(),
                identity.artifact.as_str(),
                identity.implementation.as_str(),
                identity.base_implementation.as_str(),
            ]
            .iter()
            .any(|s| s.is_empty())
        {
            return Err((ScanCallRefusal::InvalidIdentity, identity, provider));
        }
        Ok(Self {
            identity,
            request,
            provider,
        })
    }
    pub fn into_unbound_parts(self) -> (NetworkNativeIdentity, NetworkScanRequest, P) {
        (self.identity, self.request, self.provider)
    }
    pub fn native_identity(&self) -> &NetworkNativeIdentity {
        &self.identity
    }
    fn check_claim(&self, claim: &BaseOperationClaim) -> Result<(), ScanCallRefusal> {
        let i = &self.identity;
        if claim.host_id != i.host
            || claim.boot_id != i.boot
            || claim.base_instance_id != i.instance
            || claim.base_provider_generation != i.provider_generation
            || claim.resource_pool_id != i.resource
            || claim.resource_generation_id != i.resource_generation
            || claim.envelope_id != i.envelope
            || claim.implementation_id != i.implementation
        {
            return Err(ScanCallRefusal::WrongBinding);
        }
        Ok(())
    }
    fn check_possession(
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
    ) -> Result<(), ScanCallRefusal> {
        let mut entries = table.inspections();
        let e = entries.next().ok_or(ScanCallRefusal::WrongBinding)?;
        if entries.next().is_some()
            || e.scope.maximum_operations != 1
            || e.scope.maximum_in_flight != 1
            || e.scope.maximum_parameter_bytes != SCAN_OPERATION_PARAMETER_BYTES
            || e.scope.maximum_result_bytes != MAXIMUM_SCAN_OUTPUT_BYTES
            || e.scope.authority_contract_id.as_str() != NETWORK_SCAN_AUTHORITY
            || claim.parameter_bytes != SCAN_OPERATION_PARAMETER_BYTES
            || claim.work_units != 1
        {
            return Err(ScanCallRefusal::WrongBinding);
        }
        Ok(())
    }
    fn transfer(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        node: NodeId,
    ) -> Result<NetworkScanHostCall<P>, ScanCallRefusal> {
        let operation = BoundBaseOperation::bind(table, handle, claim)
            .map_err(|(_, _, _, error)| ScanCallRefusal::Capability(error))?;
        let r = slot.take().expect("validated owner");
        Ok(NetworkScanHostCall {
            operation,
            provider: r.provider,
            request: r.request,
            node,
            invoked: false,
            cleanup_required: false,
            records: [ScanObservation::EMPTY; 16],
            output: [0; MAXIMUM_SCAN_OUTPUT_BYTES as usize],
            length: 0,
        })
    }
    /// Compact assigned facts are checked by the same canonical binder. All
    /// implementation/authority pins originate in the actual retained owner.
    pub fn bind_assigned_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &PreparedScanContract,
        selected: conduit_plan_lowering::selected_assigned_operation::SelectedAssignedOperationPlan<
            '_,
        >,
    ) -> Result<NetworkScanHostCall<P>, ScanCallRefusal> {
        use conduit_plan_lowering::selected_assigned_operation::{
            bind_selected_assigned_operation_for_subject, AssignedNativeIdentity,
        };
        let ready = slot.as_ref().ok_or(ScanCallRefusal::Unavailable)?;
        ready.check_claim(&claim)?;
        Self::check_possession(&table, &claim)?;
        let i = &ready.identity;
        let native = AssignedNativeIdentity {
            capability: SCAN_CAPABILITY,
            execution_profile: NETWORK_SCAN_EXECUTION_PROFILE,
            artifact: i.artifact.as_str(),
            base: i.base.as_str(),
            base_implementation: i.base_implementation.as_str(),
            mechanism: SCAN_BASE_KIND,
            authority: NETWORK_SCAN_AUTHORITY,
            resource_class: WIFI_STATION_RESOURCE_CLASS,
        };
        let node = bind_selected_assigned_operation_for_subject(
            &table,
            &claim,
            contract.definition(),
            &contract.subject,
            native,
            selected,
        )
        .map_err(ScanCallRefusal::Binding)?;
        Self::transfer(slot, table, handle, claim, node)
    }
    /// Validation leaves the actual ready owner in its original slot on every
    /// refusal. Issued bearer arguments are consumed, never silently reissued.
    pub fn bind_selected_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &PreparedScanContract,
        selected: SelectedOperationPlan<'_>,
    ) -> Result<NetworkScanHostCall<P>, ScanCallRefusal> {
        let ready = slot.as_ref().ok_or(ScanCallRefusal::Unavailable)?;
        ready.check_claim(&claim)?;
        Self::check_possession(&table, &claim)?;
        let i = &ready.identity;
        let gear = selected
            .fragment
            .placements
            .iter()
            .find(|g| &g.placement_id == selected.placement_id)
            .ok_or(ScanCallRefusal::WrongBinding)?;
        let base = gear.base.as_ref().ok_or(ScanCallRefusal::WrongBinding)?;
        if gear.artifact_id != i.artifact
            || gear.implementation_id != i.implementation
            || gear.execution_profile_id.as_str() != NETWORK_SCAN_EXECUTION_PROFILE
            || gear.capability_id.as_str() != SCAN_CAPABILITY
            || gear.limits != contract.kind.limits
            || base.base_id != i.base
            || base.provider_instance_id != i.instance
            || base.provider_generation != i.provider_generation
            || base.implementation_id != i.base_implementation
            || base.mechanism_family.as_str() != SCAN_BASE_KIND
            || base.enforcement_class != BaseEnforcementClass::Cooperative
        {
            return Err(ScanCallRefusal::WrongBinding);
        }
        let [resource] = gear.resources.as_slice() else {
            return Err(ScanCallRefusal::WrongBinding);
        };
        if resource.class_id.as_str() != WIFI_STATION_RESOURCE_CLASS || resource.units != 1 {
            return Err(ScanCallRefusal::WrongBinding);
        }
        let node = bind_selected_operation_for_subject(
            &table,
            &claim,
            contract.definition(),
            &contract.subject,
            selected,
        )
        .map_err(ScanCallRefusal::Binding)?;
        Self::transfer(slot, table, handle, claim, node)
    }
}
/// One selected scan, with retained fixed output and mandatory cleanup custody.
pub struct NetworkScanHostCall<P> {
    operation: BoundBaseOperation,
    provider: P,
    request: NetworkScanRequest,
    node: NodeId,
    invoked: bool,
    cleanup_required: bool,
    records: [ScanObservation; 16],
    output: [u8; MAXIMUM_SCAN_OUTPUT_BYTES as usize],
    length: usize,
}
impl<P: NetworkScanProvider> NetworkScanHostCall<P> {
    pub async fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        id: RequestId,
        input: &[u8],
    ) -> Result<&[u8], ScanCallRefusal> {
        self.binding(node, call)?;
        if id != RequestId(0) || self.invoked {
            return Err(ScanCallRefusal::StaleRequest);
        }
        if !input.is_empty() {
            return Err(ScanCallRefusal::Representation(ScanFailure::Malformed));
        }
        let request = self.request;
        let started = self.provider.now_ms();
        if started >= request.scan_deadline_ms {
            return Err(ScanCallRefusal::Deadline);
        }
        let lease = self
            .operation
            .authorize()
            .map_err(ScanCallRefusal::Capability)?;
        self.invoked = true;
        self.cleanup_required = true;
        self.length = 0;
        let lease = crate::lease::Pending::new(&mut self.operation, lease);
        let completion = self.provider.scan(request, &mut self.records).await;
        let finished = self.provider.now_ms();
        let result = match completion {
            Ok(_) if finished >= request.scan_deadline_ms => Err(ScanCallRefusal::Deadline),
            Ok(c)
                if c.generation == request.generation
                    && c.completed_ms >= started
                    && c.completed_ms <= finished
                    && finished >= started =>
            {
                self.records
                    .get(..c.count as usize)
                    .ok_or(ScanCallRefusal::Representation(ScanFailure::Malformed))
                    .and_then(|records| {
                        if records.iter().any(|r| r.observed_ms < started) {
                            return Err(ScanCallRefusal::Representation(ScanFailure::Malformed));
                        }
                        c.encode(records, &mut self.output)
                            .map_err(ScanCallRefusal::Representation)
                    })
            }
            Ok(_) => Err(ScanCallRefusal::WrongBinding),
            Err(e) => Err(ScanCallRefusal::Provider(e)),
        };
        let bytes = result.as_ref().copied().unwrap_or(0);
        lease
            .complete(bytes as u32)
            .map_err(ScanCallRefusal::Capability)?;
        self.length = result?;
        Ok(&self.output[..self.length])
    }
    pub fn completed_output(&self) -> Option<&[u8]> {
        let i = self.inspection();
        (self.length > 0 && i.completed_operations == 1 && i.in_flight_operations == 0)
            .then_some(&self.output[..self.length])
    }
    /// Borrow only the original validated observations of this owner's exact
    /// successful dispatch. Historical evidence, never current radio authority.
    pub fn completed_observations(&self) -> Option<&[ScanObservation]> {
        self.completed_output()?;
        self.records.get(..usize::from(self.output[20]))
    }
    pub async fn cleanup(&mut self) -> Result<(), ScanCallRefusal> {
        if self.cleanup_required {
            if self.provider.now_ms() >= self.request.cleanup_deadline_ms {
                return Err(ScanCallRefusal::Deadline);
            }
            self.provider
                .cleanup(self.request.cleanup_deadline_ms)
                .await
                .map_err(ScanCallRefusal::Provider)?;
            if self.provider.now_ms() >= self.request.cleanup_deadline_ms {
                return Err(ScanCallRefusal::Deadline);
            }
            self.cleanup_required = false;
        }
        Ok(())
    }
    pub fn cleanup_required(&self) -> bool {
        self.cleanup_required
    }
    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), ScanCallRefusal> {
        self.binding(node, call)?;
        self.operation.revoke().map_err(ScanCallRefusal::Capability)
    }
    pub fn inspection(&self) -> &CapabilityInspection {
        self.operation.inspection()
    }
    pub fn observe_provider<R>(&self, f: impl FnOnce(&P) -> R) -> R {
        f(&self.provider)
    }
    pub fn retirement_ready(&self) -> bool {
        let i = self.inspection();
        !self.cleanup_required
            && i.lifecycle == CapabilityLifecycle::Revoked
            && i.in_flight_operations == 0
    }
    pub fn retire_retained(slot: &mut Option<Self>) -> Result<P, ScanCallRefusal> {
        if !slot.as_ref().is_some_and(Self::retirement_ready) {
            return Err(ScanCallRefusal::Retirement);
        }
        Ok(slot.take().expect("validated owner").provider)
    }
    fn binding(&self, node: NodeId, call: HostCallId) -> Result<(), ScanCallRefusal> {
        if node == self.node && call == HostCallId(0) {
            Ok(())
        } else {
            Err(ScanCallRefusal::WrongBinding)
        }
    }
}
