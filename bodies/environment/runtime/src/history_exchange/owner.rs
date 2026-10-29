//! Exact selected Plan/Play custody for finite signed evidence reads.
use super::{
    contract::{HistoryReadContract, HISTORY_CALL, HISTORY_MAXIMUM_BYTES},
    HistoryProvider, HistoryProviderRefusal, MAXIMUM_SIGNED_BATCH_BYTES,
};
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::{
    lowering::LoweredPlanFragment,
    selected_operation::{
        bind_selected_operation, SelectedOperationContract, SelectedOperationPlan,
    },
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryAttachment {
    pub generation: u64,
    pub resource_bytes: u64,
}
pub struct HistoryCallSelection<'a> {
    pub contract: &'a HistoryReadContract,
    pub fragment: &'a PlanFragment,
    pub lowered: &'a LoweredPlanFragment,
    pub active: &'a ActivePlayIdentity,
    pub placement: &'a PlacementId,
}
#[derive(Debug, PartialEq, Eq)]
pub enum HistoryCallRefusal {
    WrongBinding,
    Possession,
    StaleRequest,
    SequenceExhausted,
    Capability(BaseCapabilityRefusal),
    Canonical(StructuredInfoRefusal),
    Provider(HistoryProviderRefusal),
    ResultExtent,
}
pub struct HistoryHostCall<P> {
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    provider: P,
    node: NodeId,
    next_request: u32,
    validator: PreparedStructuredValueValidator,
    encoder: super::codec::PreparedSignedBatchEncoder,
    output: [u8; MAXIMUM_SIGNED_BATCH_BYTES],
}
impl<P: HistoryProvider> HistoryHostCall<P> {
    /// # Safety
    /// The trusted composition root owns the exact retained evidence provider,
    /// generation and independently authorized scope represented by this claim.
    pub unsafe fn bind_selected(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        attachment: HistoryAttachment,
        provider: P,
        selection: HistoryCallSelection<'_>,
    ) -> Result<Self, HistoryCallRefusal> {
        let mut entries = table.inspections();
        let entry = entries.next().ok_or(HistoryCallRefusal::Possession)?;
        if entries.next().is_some()
            || entry.scope.maximum_in_flight != 1
            || entry.scope.maximum_parameter_bytes < HISTORY_MAXIMUM_BYTES
            || entry.scope.maximum_result_bytes < HISTORY_MAXIMUM_BYTES
            || claim.parameter_bytes != HISTORY_MAXIMUM_BYTES
            || claim.work_units != 1
            || attachment.generation == 0
            || attachment.resource_bytes < MAXIMUM_SIGNED_BATCH_BYTES as u64
        {
            return Err(HistoryCallRefusal::Possession);
        }
        drop(entries);
        let node = bind_selected_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: selection.contract.kind(),
                call: HISTORY_CALL,
                input_bytes: HISTORY_MAXIMUM_BYTES,
                output_bytes: HISTORY_MAXIMUM_BYTES,
                resource_bytes: attachment.resource_bytes,
            },
            SelectedOperationPlan {
                fragment: selection.fragment,
                lowered: selection.lowered,
                active: selection.active,
                placement_id: selection.placement,
            },
        )
        .map_err(|_| HistoryCallRefusal::WrongBinding)?;
        Ok(Self {
            table,
            handle,
            claim,
            provider,
            node,
            next_request: 0,
            validator: selection
                .contract
                .request_validator()
                .map_err(HistoryCallRefusal::Canonical)?,
            encoder: super::codec::PreparedSignedBatchEncoder::new(selection.contract)
                .map_err(HistoryCallRefusal::Canonical)?,
            output: [0; MAXIMUM_SIGNED_BATCH_BYTES],
        })
    }
    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<&[u8], HistoryCallRefusal> {
        self.check_binding(node, call)?;
        if request != RequestId(self.next_request) {
            return Err(HistoryCallRefusal::StaleRequest);
        }
        let next = self
            .next_request
            .checked_add(1)
            .ok_or(HistoryCallRefusal::SequenceExhausted)?;
        self.validator
            .validate(input)
            .map_err(HistoryCallRefusal::Canonical)?;
        let cursor = decode_request(input).map_err(HistoryCallRefusal::Canonical)?;
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(HistoryCallRefusal::Capability)?;
        self.next_request = next;
        self.output.fill(0);
        let result = match self.provider.read_signed_batch(cursor, &mut self.output) {
            Ok(n) if n <= MAXIMUM_SIGNED_BATCH_BYTES => self
                .encoder
                .encode(&self.output[..n])
                .map_err(HistoryCallRefusal::Canonical),
            Ok(_) => Err(HistoryCallRefusal::ResultExtent),
            Err(e) => Err(HistoryCallRefusal::Provider(e)),
        };
        let bytes = result.as_ref().map_or(0, |b| b.len() as u32);
        self.table
            .complete(&mut self.handle, lease, bytes)
            .map_err(HistoryCallRefusal::Capability)?;
        result
    }
    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), HistoryCallRefusal> {
        self.check_binding(node, call)?;
        self.table
            .revoke(&self.handle)
            .map_err(HistoryCallRefusal::Capability)?;
        self.provider.revoke();
        Ok(())
    }
    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), HistoryCallRefusal> {
        if node != self.node || call != HostCallId(0) {
            Err(HistoryCallRefusal::WrongBinding)
        } else {
            Ok(())
        }
    }
}
/// Call only after exact request-schema validation at the admitted boundary.
pub fn decode_request(input: &[u8]) -> Result<u32, StructuredInfoRefusal> {
    let value = validate_canonical_structured_value(input)?;
    let bytes = value
        .record_field("cursor")?
        .ok_or(StructuredInfoRefusal::WrongRecordFields)?
        .primitive_bytes("value/u32")?;
    let bytes: [u8; 4] = bytes
        .try_into()
        .map_err(|_| StructuredInfoRefusal::WrongType)?;
    Ok(u32::from_le_bytes(bytes))
}
