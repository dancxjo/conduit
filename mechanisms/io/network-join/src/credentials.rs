//! One selected read of a Root-owned volatile credential source.
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_net::*;
use conduit_plan_lowering::selected_assigned_operation::{
    bind_selected_assigned_operation_for_subject, AssignedNativeIdentity,
    SelectedAssignedOperationPlan,
};
use conduit_plan_lowering::selected_operation::*;

pub const CREDENTIAL_RESOURCE_CLASS: &str = "conduit.network/private-credentials@1";
#[derive(Debug, PartialEq, Eq)]
pub enum CredentialSourceRefusal {
    Unavailable,
    Unacknowledged,
    OutputCapacity,
    Invalid,
}
/// The retained implementation owns the actual private mailbox. It must consume
/// only acknowledged input, transfer it once, and obey its admitted finite work
/// and storage. Refusals contain no credential bytes. Its Drop owns secret erasure.
pub trait CredentialSource {
    fn take_acknowledged(&mut self, output: &mut [u8]) -> Result<usize, CredentialSourceRefusal>;
}
pub struct PreparedCredentialContract {
    kind: Kind,
    subject: KindId,
}
impl PreparedCredentialContract {
    pub fn prepare() -> Self {
        Self {
            kind: network_credentials_contract(),
            subject: NETWORK_CREDENTIALS_SUBJECT.into(),
        }
    }
    pub(crate) fn kind(&self) -> &Kind {
        &self.kind
    }
    fn definition(&self) -> SelectedOperationContract<'_> {
        SelectedOperationContract {
            kind: &self.kind,
            call: NETWORK_CREDENTIALS_HOST_CALL,
            input_bytes: 1,
            output_bytes: MAXIMUM_JOIN_INPUT_BYTES,
            resource_bytes: MAXIMUM_JOIN_INPUT_BYTES.into(),
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum CredentialCallRefusal {
    OwnerUnavailable,
    Binding(SelectedCallRefusal),
    Possession,
    WrongRequest,
    OutputCapacity,
    Capability(BaseCapabilityRefusal),
    Source(CredentialSourceRefusal),
    InvalidOutput,
}
pub struct NetworkCredentialsHostCall<P> {
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    source: P,
    node: NodeId,
    invoked: bool,
}
impl<P: CredentialSource> NetworkCredentialsHostCall<P> {
    /// # Safety
    /// Root must retain the actual private source and current Base/resource
    /// generation, admit this Plan/Play and its finite work/storage, and issue
    /// authority specifically for this read. Serialized metadata is insufficient.
    /// This cooperative owner does not confine hostile native code.
    pub unsafe fn bind_selected(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        source: P,
        contract: &PreparedCredentialContract,
        selected: SelectedOperationPlan<'_>,
    ) -> Result<Self, CredentialCallRefusal> {
        let node = Self::validate_selected(&table, &claim, contract, selected)?;
        Ok(Self::bound(table, handle, claim, source, node))
    }
    pub(crate) fn validate_selected(
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &PreparedCredentialContract,
        selected: SelectedOperationPlan<'_>,
    ) -> Result<NodeId, CredentialCallRefusal> {
        Self::check_possession(table, claim)?;
        let gear = selected
            .fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selected.placement_id)
            .ok_or(CredentialCallRefusal::WrongRequest)?;
        let [resource] = gear.resources.as_slice() else {
            return Err(CredentialCallRefusal::WrongRequest);
        };
        if resource.class_id.as_str() != CREDENTIAL_RESOURCE_CLASS || resource.units != 1 {
            return Err(CredentialCallRefusal::WrongRequest);
        }
        let node = bind_selected_operation_for_subject(
            table,
            claim,
            contract.definition(),
            &contract.subject,
            selected,
        )
        .map_err(CredentialCallRefusal::Binding)?;
        Ok(node)
    }
    pub(crate) fn validate_direct(
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &PreparedCredentialContract,
        selected: conduit_plan_lowering::selected_operation::SelectedDirectOperationPlan<
            '_,
            3,
            2,
            1,
            3,
            2,
            3,
        >,
    ) -> Result<NodeId, CredentialCallRefusal> {
        Self::check_possession(table, claim)?;
        let (_, fragment) = selected.tables.preparation_scope();
        let gear = fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selected.placement_id)
            .ok_or(CredentialCallRefusal::WrongRequest)?;
        let [resource] = gear.resources.as_slice() else {
            return Err(CredentialCallRefusal::WrongRequest);
        };
        if resource.class_id.as_str() != CREDENTIAL_RESOURCE_CLASS || resource.units != 1 {
            return Err(CredentialCallRefusal::WrongRequest);
        }
        let node =
            conduit_plan_lowering::selected_operation::bind_selected_direct_operation_for_subject(
                table,
                claim,
                contract.definition(),
                &contract.subject,
                selected,
            )
            .map_err(CredentialCallRefusal::Binding)?;
        Ok(node)
    }
    pub(crate) fn bound(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        source: P,
        node: NodeId,
    ) -> Self {
        Self {
            table,
            handle,
            claim,
            source,
            node,
            invoked: false,
        }
    }
    fn check_possession(
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
    ) -> Result<(), CredentialCallRefusal> {
        let mut entries = table.inspections();
        let entry = entries.next().ok_or(CredentialCallRefusal::Possession)?;
        if entries.next().is_some()
            || entry.scope.authority_contract_id.as_str() != NETWORK_CREDENTIALS_AUTHORITY
            || entry.scope.maximum_operations != 1
            || entry.scope.maximum_in_flight != 1
            || entry.scope.maximum_parameter_bytes < 1
            || entry.scope.maximum_result_bytes < MAXIMUM_JOIN_INPUT_BYTES
            || claim.parameter_bytes != 1
            || claim.work_units != 1
        {
            return Err(CredentialCallRefusal::Possession);
        }
        Ok(())
    }
    /// # Safety
    /// Same source ownership and admission requirements as `bind_selected`.
    /// Root must retain native identities independently of incoming bytes.
    pub unsafe fn bind_assigned(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        source: P,
        contract: &PreparedCredentialContract,
        native: AssignedNativeIdentity<'_>,
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<Self, CredentialCallRefusal> {
        Self::check_possession(&table, &claim)?;
        if native.resource_class != CREDENTIAL_RESOURCE_CLASS {
            return Err(CredentialCallRefusal::WrongRequest);
        }
        let node = bind_selected_assigned_operation_for_subject(
            &table,
            &claim,
            contract.definition(),
            &contract.subject,
            native,
            selected,
        )
        .map_err(CredentialCallRefusal::Binding)?;
        Ok(Self {
            table,
            handle,
            claim,
            source,
            node,
            invoked: false,
        })
    }
    /// Output is caller-owned admitted secret storage. Root must erase it after
    /// custody passes to the selected join call, including failure/cancellation.
    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, CredentialCallRefusal> {
        if node != self.node
            || call != HostCallId(0)
            || request != RequestId(0)
            || self.invoked
            || !input.is_empty()
        {
            return Err(CredentialCallRefusal::WrongRequest);
        }
        if output.len() < MAXIMUM_JOIN_INPUT_BYTES as usize {
            return Err(CredentialCallRefusal::OutputCapacity);
        }
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(CredentialCallRefusal::Capability)?;
        self.invoked = true;
        let lease = crate::lease::Pending::new(&mut self.table, &mut self.handle, lease);
        let result = self
            .source
            .take_acknowledged(&mut output[..MAXIMUM_JOIN_INPUT_BYTES as usize])
            .map_err(CredentialCallRefusal::Source)
            .and_then(|length| {
                let bytes = output
                    .get(..length)
                    .ok_or(CredentialCallRefusal::InvalidOutput)?;
                decode_network_join_request(bytes)
                    .map_err(|_| CredentialCallRefusal::InvalidOutput)?;
                Ok(length)
            });
        lease
            .complete(result.as_ref().copied().unwrap_or(0) as u32)
            .map_err(CredentialCallRefusal::Capability)?;
        result
    }
    pub fn revoke(&mut self) -> Result<(), CredentialCallRefusal> {
        self.table
            .revoke(&self.handle)
            .map_err(CredentialCallRefusal::Capability)
    }
    pub fn retirement_ready(&self) -> bool {
        let mut entries = self.table.inspections();
        entries.next().is_some_and(|entry| {
            entry.lifecycle == CapabilityLifecycle::Revoked && entry.in_flight_operations == 0
        }) && entries.next().is_none()
    }
    /// Recover the actual private source after its exact selected capability
    /// was revoked. Invocation counters are not reset by returning custody.
    pub fn retire_retained(slot: &mut Option<Self>) -> Result<P, CredentialCallRefusal> {
        if !slot
            .as_ref()
            .ok_or(CredentialCallRefusal::OwnerUnavailable)?
            .retirement_ready()
        {
            return Err(CredentialCallRefusal::Possession);
        }
        Ok(slot
            .take()
            .expect("preflighted retained credential source")
            .source)
    }
}
