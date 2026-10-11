//! Possession and lifetime for one selected class-neutral outbound USB endpoint call.
#![cfg_attr(not(test), allow(dead_code))] // native composition is not installed yet

use conduit_core::{
    BaseCapabilityHandle, BaseCapabilityRefusal, BaseCapabilityTable, BaseOperationClaim,
    BaseOperationLease, CapabilityLifecycle, StructuredInfoRefusal,
};
use conduit_kernel::{HostCallId, NodeId, RequestId};

use super::{
    endpoint_write_contract::{
        ENDPOINT_WRITE_CALL, ENDPOINT_WRITE_DATA_BYTES, ENDPOINT_WRITE_KIND,
        ENDPOINT_WRITE_MAXIMUM_BYTES, EndpointWriteContract,
    },
    endpoint_write_request::{EndpointWriteRequestRefusal, PreparedEndpointWriteRequestDecoder},
    endpoint_write_result::{
        EndpointWriteDisposition, EndpointWriteResultRefusal, PreparedEndpointWriteResultEncoder,
    },
};
use crate::machine_membrane::selected_operation::{
    SelectedCallRefusal, SelectedOperationContract, SelectedOperationPlan, bind_selected_operation,
};

#[derive(Debug, PartialEq, Eq)]
pub enum EndpointWriteOwnerRefusal {
    WrongBinding,
    Possession,
    Pressure,
    StaleTransfer,
    SequenceExhausted,
    Capability(BaseCapabilityRefusal),
    Decode(EndpointWriteRequestRefusal),
    Result(EndpointWriteResultRefusal),
    Canonical(StructuredInfoRefusal),
}

/// Trusted physical facts for the native owner, not fields in a plot request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EndpointWriteAttachment {
    pub slot: u8,
    pub generation: u64,
    pub endpoint_generation: u64,
    pub endpoint_dci: u8,
    pub maximum_data_bytes: u16,
    pub resource_bytes: u64,
}

/// Move-only native submission. Neither serialized identifiers nor a new
/// request can reconstruct this operation's opaque Base lease.
pub(crate) struct NativeEndpointWriteSubmission {
    attachment: EndpointWriteAttachment,
    request_id: RequestId,
    lease: BaseOperationLease,
    length: u16,
    wire: [u8; ENDPOINT_WRITE_DATA_BYTES as usize],
}

pub(crate) enum NativeEndpointWriteObservation {
    Completed {
        actual: u16,
    },
    #[allow(dead_code)]
    // Native terminal faults retain DMA until an acknowledged stop path is installed.
    Disposition(EndpointWriteDisposition),
}

pub struct EndpointWriteCallOwner {
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    node: NodeId,
    attachment: EndpointWriteAttachment,
    decoder: PreparedEndpointWriteRequestDecoder,
    encoder: PreparedEndpointWriteResultEncoder,
    pending: Option<(RequestId, BaseOperationLease)>,
    next_request: u32,
}

impl EndpointWriteCallOwner {
    /// Bind existing possession and an actual native attachment before Play.
    ///
    /// # Safety
    /// The native composition root must own this exact controller/slot, outbound interrupt or bulk endpoint and DMA
    /// storage under the claim's resource generation/envelope. No request may
    /// choose another slot, endpoint, generation or buffer. The physical owner must retain DMA/rings
    /// until acknowledged hardware quiescence, including on software revocation,
    /// provider loss and timeout. This constructor grants no resource authority
    /// and does not provide hostile-code containment in a shared address space.
    #[allow(dead_code)] // native composition installation remains a separate step
    pub(crate) unsafe fn bind_admitted(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        attachment: EndpointWriteAttachment,
        contract: &EndpointWriteContract,
        selection: SelectedOperationPlan<'_>,
    ) -> Result<Self, EndpointWriteOwnerRefusal> {
        let mut entries = table.inspections();
        let entry = entries
            .next()
            .ok_or(EndpointWriteOwnerRefusal::Possession)?;
        if entries.next().is_some()
            || entry.scope.maximum_in_flight != 1
            || entry.scope.maximum_parameter_bytes < ENDPOINT_WRITE_MAXIMUM_BYTES
            || entry.scope.maximum_result_bytes < ENDPOINT_WRITE_MAXIMUM_BYTES
            || claim.operation_contract_id.as_str() != ENDPOINT_WRITE_CALL
            || claim.subject_kind.as_str() != ENDPOINT_WRITE_KIND
            || claim.parameter_bytes != ENDPOINT_WRITE_MAXIMUM_BYTES
            || claim.work_units != 1
            || attachment.slot == 0
            || attachment.generation == 0
            || attachment.endpoint_generation == 0
            || !(2..=30).contains(&attachment.endpoint_dci)
            || !attachment.endpoint_dci.is_multiple_of(2)
            || attachment.maximum_data_bytes == 0
            || attachment.maximum_data_bytes > ENDPOINT_WRITE_DATA_BYTES
            || attachment.resource_bytes < u64::from(attachment.maximum_data_bytes)
        {
            return Err(EndpointWriteOwnerRefusal::Possession);
        }
        drop(entries);
        let node = bind_selected_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: contract.kind(),
                call: ENDPOINT_WRITE_CALL,
                input_bytes: ENDPOINT_WRITE_MAXIMUM_BYTES,
                output_bytes: ENDPOINT_WRITE_MAXIMUM_BYTES,
                resource_bytes: attachment.resource_bytes,
            },
            selection,
        )
        .map_err(|refusal| match refusal {
            SelectedCallRefusal::WrongBinding => EndpointWriteOwnerRefusal::WrongBinding,
            SelectedCallRefusal::Possession => EndpointWriteOwnerRefusal::Possession,
        })?;
        Ok(Self {
            table,
            handle,
            claim,
            node,
            attachment,
            decoder: PreparedEndpointWriteRequestDecoder::new(contract)
                .map_err(EndpointWriteOwnerRefusal::Canonical)?,
            encoder: PreparedEndpointWriteResultEncoder::new(contract)
                .map_err(EndpointWriteOwnerRefusal::Canonical)?,
            pending: None,
            next_request: 0,
        })
    }

    /// Accept the next request from the shared single-call kernel Back. A
    /// replay after physical completion cannot silently repeat an effect.
    #[allow(dead_code)]
    pub(crate) fn begin(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request_id: RequestId,
        input: &[u8],
    ) -> Result<NativeEndpointWriteSubmission, EndpointWriteOwnerRefusal> {
        self.check_binding(node, call)?;
        match self
            .table
            .inspections()
            .next()
            .ok_or(EndpointWriteOwnerRefusal::Possession)?
            .lifecycle
        {
            CapabilityLifecycle::Revoked => {
                return Err(EndpointWriteOwnerRefusal::Capability(
                    BaseCapabilityRefusal::Revoked,
                ));
            }
            CapabilityLifecycle::Exhausted => {
                return Err(EndpointWriteOwnerRefusal::Capability(
                    BaseCapabilityRefusal::Exhausted,
                ));
            }
            CapabilityLifecycle::Issued => {}
        }
        if self.pending.is_some() {
            return Err(EndpointWriteOwnerRefusal::Pressure);
        }
        if request_id != RequestId(self.next_request) {
            return Err(EndpointWriteOwnerRefusal::StaleTransfer);
        }
        let next = self
            .next_request
            .checked_add(1)
            .ok_or(EndpointWriteOwnerRefusal::SequenceExhausted)?;
        let input_wire = self
            .decoder
            .decode(input, self.attachment.maximum_data_bytes)
            .map_err(EndpointWriteOwnerRefusal::Decode)?;
        let length = input_wire.len() as u16;
        let mut wire = [0; ENDPOINT_WRITE_DATA_BYTES as usize];
        wire[..input_wire.len()].copy_from_slice(input_wire);
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(EndpointWriteOwnerRefusal::Capability)?;
        self.next_request = next;
        self.pending = Some((request_id, lease.clone()));
        Ok(NativeEndpointWriteSubmission {
            attachment: self.attachment,
            request_id,
            lease,
            length,
            wire,
        })
    }

    /// Revoke software possession. The pending physical operation is deliberately
    /// retained; this does not claim a successful endpoint stop or DMA release.
    pub fn revoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
    ) -> Result<(), EndpointWriteOwnerRefusal> {
        self.check_binding(node, call)?;
        self.table
            .revoke(&self.handle)
            .map_err(EndpointWriteOwnerRefusal::Capability)
    }

    /// # Safety
    /// The native owner must have acknowledged quiescence of this exact slot,
    /// attachment generation and transfer: validated completion of this exact transfer, or
    /// acknowledged endpoint/controller stop. A timeout, failed/foreign event,
    /// software cancellation or device-loss observation alone is insufficient.
    #[allow(dead_code)]
    pub(crate) unsafe fn finish_quiesced(
        &mut self,
        submission: &NativeEndpointWriteSubmission,
        observation: NativeEndpointWriteObservation,
    ) -> Result<&[u8], EndpointWriteOwnerRefusal> {
        if !self.pending.as_ref().is_some_and(|(sequence, lease)| {
            *sequence == submission.request_id && *lease == submission.lease
        }) || self.attachment != submission.attachment
        {
            return Err(EndpointWriteOwnerRefusal::StaleTransfer);
        }
        let encoded = match observation {
            NativeEndpointWriteObservation::Completed { actual } => self
                .encoder
                .completed(submission.length, actual)
                .map_err(EndpointWriteOwnerRefusal::Result),
            NativeEndpointWriteObservation::Disposition(disposition) => self
                .encoder
                .disposition(disposition)
                .map_err(EndpointWriteOwnerRefusal::Canonical),
        };
        self.pending = None; // physical acknowledgement, independent of software acceptance
        let result_bytes = encoded.as_ref().map_or(0, |bytes| bytes.len() as u32);
        self.table
            .complete(&mut self.handle, submission.lease.clone(), result_bytes)
            .map_err(EndpointWriteOwnerRefusal::Capability)?;
        encoded
    }

    /// # Safety
    /// An acknowledged native endpoint/controller stop must cover this exact
    /// retained submission and attachment. Software revocation alone is insufficient.
    pub(crate) unsafe fn discard_quiesced(
        &mut self,
        submission: &NativeEndpointWriteSubmission,
    ) -> Result<(), EndpointWriteOwnerRefusal> {
        if !self.pending.as_ref().is_some_and(|(sequence, lease)| {
            *sequence == submission.request_id && *lease == submission.lease
        }) || self.attachment != submission.attachment
        {
            return Err(EndpointWriteOwnerRefusal::StaleTransfer);
        }
        self.pending = None;
        self.table
            .complete(&mut self.handle, submission.lease.clone(), 0)
            .map_err(EndpointWriteOwnerRefusal::Capability)
    }

    fn check_binding(
        &self,
        node: NodeId,
        call: HostCallId,
    ) -> Result<(), EndpointWriteOwnerRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(EndpointWriteOwnerRefusal::WrongBinding);
        }
        Ok(())
    }
}

impl NativeEndpointWriteSubmission {
    pub(crate) fn length(&self) -> u16 {
        self.length
    }
    /// Exact bounded payload; an empty slice represents a real zero-length transfer.
    pub(crate) fn wire(&self) -> &[u8] {
        &self.wire[..usize::from(self.length)]
    }
}

#[cfg(test)]
pub(crate) mod tests;
