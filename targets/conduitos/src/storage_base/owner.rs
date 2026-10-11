//! Exact admitted lifetime for a transport-neutral read-only block provider.
//! This owner never chooses a device, class request, retry or transfer schedule.
#![cfg_attr(not(test), allow(dead_code))] // Root installation is a separate integration.
use super::{
    contract::{
        BLOCK_READ_CALL, BLOCK_READ_KIND, BLOCK_READ_MAXIMUM_BYTES, BLOCK_READ_RESOURCE_BYTES,
        BlockReadContract,
    },
    request::{BlockReadRequest, BlockReadRequestRefusal, PreparedBlockReadRequestDecoder},
    result::{BlockReadDisposition, BlockReadResultRefusal, PreparedBlockReadResultEncoder},
};
use crate::machine_membrane::selected_operation::{
    SelectedCallRefusal, SelectedOperationContract, SelectedOperationPlan, bind_selected_operation,
};
use conduit_core::{
    BaseCapabilityHandle, BaseCapabilityRefusal, BaseCapabilityTable, BaseOperationClaim,
    BaseOperationLease, CapabilityLifecycle, StructuredInfoRefusal,
};
use conduit_kernel::{HostCallId, NodeId, RequestId};

#[derive(Debug, PartialEq, Eq)]
pub enum BlockReadOwnerRefusal {
    Terminal,
    WrongBinding,
    Possession,
    Pressure,
    StaleRequest,
    SequenceExhausted,
    Capability(BaseCapabilityRefusal),
    Request(BlockReadRequestRefusal),
    Result(BlockReadResultRefusal),
    Canonical(StructuredInfoRefusal),
}
/// Move-only possession receipt. A serialized request cannot reconstruct it.
pub(crate) struct BlockReadSubmission {
    request_id: RequestId,
    lease: BaseOperationLease,
    request: BlockReadRequest,
}
pub(crate) enum BlockReadObservation<'a> {
    Read { bytes: &'a [u8], residue: u16 },
    Disposition(BlockReadDisposition),
}
pub struct BlockReadCallOwner {
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    node: NodeId,
    decoder: PreparedBlockReadRequestDecoder,
    encoder: PreparedBlockReadResultEncoder,
    pending: Option<(RequestId, BaseOperationLease, BlockReadRequest)>,
    next_request: u32,
    terminal: bool,
}
impl BlockReadCallOwner {
    /// Bind exact existing possession and selected storage before Play.
    ///
    /// # Safety
    /// Root must own the actual block provider for this Base/provider/resource
    /// generation and reserve all bounded storage before Play. A checked BOT
    /// Plot may realize the semantic Back, but this owner is not a BOT driver.
    /// Provider cancellation must retain any outstanding underlying operations
    /// until their exact completions or acknowledged physical stop; software
    /// revocation, timeout and disconnect alone cannot assert quiescence.
    pub(crate) unsafe fn bind_admitted(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &BlockReadContract,
        selection: SelectedOperationPlan<'_>,
    ) -> Result<Self, BlockReadOwnerRefusal> {
        let mut entries = table.inspections();
        let entry = entries.next().ok_or(BlockReadOwnerRefusal::Possession)?;
        if entries.next().is_some()
            || entry.scope.maximum_in_flight != 1
            || entry.scope.maximum_parameter_bytes < BLOCK_READ_MAXIMUM_BYTES
            || entry.scope.maximum_result_bytes < BLOCK_READ_MAXIMUM_BYTES
            || claim.base_provider_generation == 0
            || claim.operation_contract_id.as_str() != BLOCK_READ_CALL
            || claim.subject_kind.as_str() != BLOCK_READ_KIND
            || claim.parameter_bytes != BLOCK_READ_MAXIMUM_BYTES
            || claim.work_units != 1
        {
            return Err(BlockReadOwnerRefusal::Possession);
        }
        drop(entries);
        let node = bind_selected_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: contract.kind(),
                call: BLOCK_READ_CALL,
                input_bytes: BLOCK_READ_MAXIMUM_BYTES,
                output_bytes: BLOCK_READ_MAXIMUM_BYTES,
                resource_bytes: BLOCK_READ_RESOURCE_BYTES,
            },
            selection,
        )
        .map_err(|error| match error {
            SelectedCallRefusal::WrongBinding => BlockReadOwnerRefusal::WrongBinding,
            SelectedCallRefusal::Possession => BlockReadOwnerRefusal::Possession,
        })?;
        Ok(Self {
            table,
            handle,
            claim,
            node,
            decoder: PreparedBlockReadRequestDecoder::new(contract)
                .map_err(BlockReadOwnerRefusal::Canonical)?,
            encoder: PreparedBlockReadResultEncoder::new(contract)
                .map_err(BlockReadOwnerRefusal::Canonical)?,
            pending: None,
            next_request: 0,
            terminal: false,
        })
    }
    pub(crate) fn begin(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request_id: RequestId,
        input: &[u8],
    ) -> Result<BlockReadSubmission, BlockReadOwnerRefusal> {
        self.check_binding(node, call)?;
        match self
            .table
            .inspections()
            .next()
            .ok_or(BlockReadOwnerRefusal::Possession)?
            .lifecycle
        {
            CapabilityLifecycle::Revoked => {
                return Err(BlockReadOwnerRefusal::Capability(
                    BaseCapabilityRefusal::Revoked,
                ));
            }
            CapabilityLifecycle::Exhausted => {
                return Err(BlockReadOwnerRefusal::Capability(
                    BaseCapabilityRefusal::Exhausted,
                ));
            }
            CapabilityLifecycle::Issued => {}
        }
        if self.terminal {
            return Err(BlockReadOwnerRefusal::Terminal);
        }
        if self.pending.is_some() {
            return Err(BlockReadOwnerRefusal::Pressure);
        }
        if request_id != RequestId(self.next_request) {
            return Err(BlockReadOwnerRefusal::StaleRequest);
        }
        let next = self
            .next_request
            .checked_add(1)
            .ok_or(BlockReadOwnerRefusal::SequenceExhausted)?;
        let request = self
            .decoder
            .decode(input)
            .map_err(BlockReadOwnerRefusal::Request)?;
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(BlockReadOwnerRefusal::Capability)?;
        self.pending = Some((request_id, lease.clone(), request));
        self.next_request = next;
        Ok(BlockReadSubmission {
            request_id,
            lease,
            request,
        })
    }
    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), BlockReadOwnerRefusal> {
        self.check_binding(node, call)?;
        self.terminal = true;
        self.table
            .revoke(&self.handle)
            .map_err(BlockReadOwnerRefusal::Capability)
    }
    /// # Safety
    /// The provider must acknowledge completion of this exact semantic request
    /// and all underlying operations that could still access retained storage.
    /// A typed terminal disposition alone does not establish this fact.
    pub(crate) unsafe fn finish_acknowledged(
        &mut self,
        submission: &BlockReadSubmission,
        observation: BlockReadObservation<'_>,
    ) -> Result<&[u8], BlockReadOwnerRefusal> {
        self.check_submission(submission)?;
        let ordinal = u64::from(submission.request_id.0);
        let terminal = matches!(observation, BlockReadObservation::Disposition(_));
        let encoded = match observation {
            BlockReadObservation::Read { bytes, residue } => self
                .encoder
                .observed(submission.request, ordinal, bytes, residue)
                .map_err(BlockReadOwnerRefusal::Result)?,
            BlockReadObservation::Disposition(disposition) => self
                .encoder
                .disposition(submission.request, ordinal, disposition)
                .map_err(BlockReadOwnerRefusal::Result)?,
        };
        self.pending = None;
        self.terminal |= terminal;
        self.table
            .complete(
                &mut self.handle,
                submission.lease.clone(),
                encoded.len() as u32,
            )
            .map_err(BlockReadOwnerRefusal::Capability)?;
        Ok(encoded)
    }
    /// # Safety
    /// An actual acknowledged provider stop must cover this exact request and
    /// every retained underlying operation before storage/lease retirement.
    pub(crate) unsafe fn discard_acknowledged(
        &mut self,
        submission: &BlockReadSubmission,
    ) -> Result<(), BlockReadOwnerRefusal> {
        self.check_submission(submission)?;
        self.pending = None;
        self.terminal = true;
        self.table
            .complete(&mut self.handle, submission.lease.clone(), 0)
            .map_err(BlockReadOwnerRefusal::Capability)
    }
    fn check_submission(
        &self,
        submission: &BlockReadSubmission,
    ) -> Result<(), BlockReadOwnerRefusal> {
        if !self.pending.as_ref().is_some_and(|(id, lease, request)| {
            *id == submission.request_id
                && *lease == submission.lease
                && *request == submission.request
        }) {
            return Err(BlockReadOwnerRefusal::StaleRequest);
        }
        Ok(())
    }
    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), BlockReadOwnerRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(BlockReadOwnerRefusal::WrongBinding);
        }
        Ok(())
    }
}
impl BlockReadSubmission {
    pub(crate) fn request(&self) -> &BlockReadRequest {
        &self.request
    }
}
#[cfg(test)]
mod tests;
