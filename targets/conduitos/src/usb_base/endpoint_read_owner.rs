//! Possession and lifetime for one selected class-neutral inbound USB endpoint call.
#![cfg_attr(not(test), allow(dead_code))] // native composition is not installed yet

use conduit_core::{
    BaseCapabilityHandle, BaseCapabilityRefusal, BaseCapabilityTable, BaseOperationClaim,
    BaseOperationLease, CapabilityLifecycle, StructuredInfoRefusal,
};
use conduit_kernel::{HostCallId, NodeId, RequestId};

use super::{
    endpoint_read_contract::{
        ENDPOINT_READ_CALL, ENDPOINT_READ_DATA_BYTES, ENDPOINT_READ_KIND,
        ENDPOINT_READ_MAXIMUM_BYTES, EndpointReadContract,
    },
    endpoint_read_request::{EndpointReadRequestRefusal, PreparedEndpointReadRequestDecoder},
    endpoint_read_result::{
        EndpointReadDisposition, EndpointReadResultRefusal, PreparedEndpointReadResultEncoder,
    },
};
use crate::machine_membrane::selected_operation::{
    SelectedCallRefusal, SelectedOperationContract, SelectedOperationPlan, bind_selected_operation,
};

#[derive(Debug, PartialEq, Eq)]
pub enum EndpointReadOwnerRefusal {
    WrongBinding,
    Possession,
    Pressure,
    StaleTransfer,
    SequenceExhausted,
    Capability(BaseCapabilityRefusal),
    Decode(EndpointReadRequestRefusal),
    Result(EndpointReadResultRefusal),
    Canonical(StructuredInfoRefusal),
}

/// Trusted physical facts for the native owner, not fields in a plot request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EndpointReadAttachment {
    pub slot: u8,
    pub generation: u64,
    pub endpoint_generation: u64,
    pub endpoint_dci: u8,
    pub maximum_data_bytes: u16,
    pub resource_bytes: u64,
}

/// Move-only native submission. Neither serialized identifiers nor a new
/// request can reconstruct this operation's opaque Base lease.
pub(crate) struct NativeEndpointReadSubmission {
    attachment: EndpointReadAttachment,
    request_id: RequestId,
    lease: BaseOperationLease,
    length: u16,
}

pub(crate) enum NativeEndpointReadObservation<'a> {
    Completed { actual: u16, input: &'a [u8] },
    Disposition(EndpointReadDisposition),
}

pub struct EndpointReadCallOwner {
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    node: NodeId,
    attachment: EndpointReadAttachment,
    decoder: PreparedEndpointReadRequestDecoder,
    encoder: PreparedEndpointReadResultEncoder,
    pending: Option<(RequestId, BaseOperationLease)>,
    next_request: u32,
}

impl EndpointReadCallOwner {
    /// Bind existing possession and an actual native attachment before Play.
    ///
    /// # Safety
    /// The native composition root must own this exact controller/slot, inbound interrupt or bulk endpoint and DMA
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
        attachment: EndpointReadAttachment,
        contract: &EndpointReadContract,
        selection: SelectedOperationPlan<'_>,
    ) -> Result<Self, EndpointReadOwnerRefusal> {
        let mut entries = table.inspections();
        let entry = entries.next().ok_or(EndpointReadOwnerRefusal::Possession)?;
        if entries.next().is_some()
            || entry.scope.maximum_in_flight != 1
            || entry.scope.maximum_parameter_bytes < ENDPOINT_READ_MAXIMUM_BYTES
            || entry.scope.maximum_result_bytes < ENDPOINT_READ_MAXIMUM_BYTES
            || claim.operation_contract_id.as_str() != ENDPOINT_READ_CALL
            || claim.subject_kind.as_str() != ENDPOINT_READ_KIND
            || claim.parameter_bytes != ENDPOINT_READ_MAXIMUM_BYTES
            || claim.work_units != 1
            || attachment.slot == 0
            || attachment.generation == 0
            || attachment.endpoint_generation == 0
            || !(3..=31).contains(&attachment.endpoint_dci)
            || attachment.endpoint_dci % 2 != 1
            || attachment.maximum_data_bytes == 0
            || attachment.maximum_data_bytes > ENDPOINT_READ_DATA_BYTES
            || attachment.resource_bytes < u64::from(attachment.maximum_data_bytes)
        {
            return Err(EndpointReadOwnerRefusal::Possession);
        }
        drop(entries);
        let node = bind_selected_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: contract.kind(),
                call: ENDPOINT_READ_CALL,
                input_bytes: ENDPOINT_READ_MAXIMUM_BYTES,
                output_bytes: ENDPOINT_READ_MAXIMUM_BYTES,
                resource_bytes: attachment.resource_bytes,
            },
            selection,
        )
        .map_err(|refusal| match refusal {
            SelectedCallRefusal::WrongBinding => EndpointReadOwnerRefusal::WrongBinding,
            SelectedCallRefusal::Possession => EndpointReadOwnerRefusal::Possession,
        })?;
        Ok(Self {
            table,
            handle,
            claim,
            node,
            attachment,
            decoder: PreparedEndpointReadRequestDecoder::new(contract)
                .map_err(EndpointReadOwnerRefusal::Canonical)?,
            encoder: PreparedEndpointReadResultEncoder::new(contract)
                .map_err(EndpointReadOwnerRefusal::Canonical)?,
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
    ) -> Result<NativeEndpointReadSubmission, EndpointReadOwnerRefusal> {
        self.check_binding(node, call)?;
        match self
            .table
            .inspections()
            .next()
            .ok_or(EndpointReadOwnerRefusal::Possession)?
            .lifecycle
        {
            CapabilityLifecycle::Revoked => {
                return Err(EndpointReadOwnerRefusal::Capability(
                    BaseCapabilityRefusal::Revoked,
                ));
            }
            CapabilityLifecycle::Exhausted => {
                return Err(EndpointReadOwnerRefusal::Capability(
                    BaseCapabilityRefusal::Exhausted,
                ));
            }
            CapabilityLifecycle::Issued => {}
        }
        if self.pending.is_some() {
            return Err(EndpointReadOwnerRefusal::Pressure);
        }
        if request_id != RequestId(self.next_request) {
            return Err(EndpointReadOwnerRefusal::StaleTransfer);
        }
        let next = self
            .next_request
            .checked_add(1)
            .ok_or(EndpointReadOwnerRefusal::SequenceExhausted)?;
        let length = self
            .decoder
            .decode(input, self.attachment.maximum_data_bytes)
            .map_err(EndpointReadOwnerRefusal::Decode)?;
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(EndpointReadOwnerRefusal::Capability)?;
        self.next_request = next;
        self.pending = Some((request_id, lease.clone()));
        Ok(NativeEndpointReadSubmission {
            attachment: self.attachment,
            request_id,
            lease,
            length,
        })
    }

    /// Revoke software possession. The pending physical operation is deliberately
    /// retained; this does not claim a successful endpoint stop or DMA release.
    pub fn revoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
    ) -> Result<(), EndpointReadOwnerRefusal> {
        self.check_binding(node, call)?;
        self.table
            .revoke(&self.handle)
            .map_err(EndpointReadOwnerRefusal::Capability)
    }

    /// # Safety
    /// The native owner must have acknowledged quiescence of this exact slot,
    /// attachment generation and transfer: validated completion of this exact transfer, or
    /// acknowledged endpoint/controller stop. A timeout, failed/foreign event,
    /// software cancellation or device-loss observation alone is insufficient.
    #[allow(dead_code)]
    pub(crate) unsafe fn finish_quiesced(
        &mut self,
        submission: &NativeEndpointReadSubmission,
        observation: NativeEndpointReadObservation<'_>,
    ) -> Result<&[u8], EndpointReadOwnerRefusal> {
        if !self.pending.as_ref().is_some_and(|(sequence, lease)| {
            *sequence == submission.request_id && *lease == submission.lease
        }) || self.attachment != submission.attachment
        {
            return Err(EndpointReadOwnerRefusal::StaleTransfer);
        }
        let encoded = match observation {
            NativeEndpointReadObservation::Completed { actual, input } => self
                .encoder
                .completed(submission.length, actual, input)
                .map_err(EndpointReadOwnerRefusal::Result),
            NativeEndpointReadObservation::Disposition(disposition) => self
                .encoder
                .disposition(disposition)
                .map_err(EndpointReadOwnerRefusal::Canonical),
        };
        self.pending = None; // physical acknowledgement, independent of software acceptance
        let result_bytes = encoded.as_ref().map_or(0, |bytes| bytes.len() as u32);
        self.table
            .complete(&mut self.handle, submission.lease.clone(), result_bytes)
            .map_err(EndpointReadOwnerRefusal::Capability)?;
        encoded
    }

    /// # Safety
    /// An acknowledged native endpoint/controller stop must cover this exact
    /// retained submission and attachment. Software revocation alone is insufficient.
    pub(crate) unsafe fn discard_quiesced(
        &mut self,
        submission: &NativeEndpointReadSubmission,
    ) -> Result<(), EndpointReadOwnerRefusal> {
        if !self.pending.as_ref().is_some_and(|(sequence, lease)| {
            *sequence == submission.request_id && *lease == submission.lease
        }) || self.attachment != submission.attachment
        {
            return Err(EndpointReadOwnerRefusal::StaleTransfer);
        }
        self.pending = None;
        self.table
            .complete(&mut self.handle, submission.lease.clone(), 0)
            .map_err(EndpointReadOwnerRefusal::Capability)
    }

    fn check_binding(
        &self,
        node: NodeId,
        call: HostCallId,
    ) -> Result<(), EndpointReadOwnerRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(EndpointReadOwnerRefusal::WrongBinding);
        }
        Ok(())
    }
}

impl NativeEndpointReadSubmission {
    pub(crate) fn length(&self) -> u16 {
        self.length
    }
    pub(crate) fn attachment(&self) -> EndpointReadAttachment {
        self.attachment
    }
}

#[cfg(test)]
mod tests;
