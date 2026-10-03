//! Possession and lifetime for one selected class-neutral USB control call.
#![cfg_attr(not(test), allow(dead_code))] // native composition is not installed yet

use conduit_core::{
    BaseCapabilityHandle, BaseCapabilityRefusal, BaseCapabilityTable, BaseOperationClaim,
    BaseOperationLease, CapabilityLifecycle, StructuredInfoRefusal,
};
use conduit_kernel::{HostCallId, NodeId, RequestId};

use super::{
    control_contract::{CONTROL_CALL, CONTROL_KIND, CONTROL_MAXIMUM_BYTES, ControlContract},
    control_decode::{ControlDecodeRefusal, DecodedControlRequest, PreparedControlRequestDecoder},
    control_request::{ControlRequestRefusal, ControlTransferRequest},
    control_result::{
        ControlResultRefusal, ControlTransferDisposition, PreparedControlResultEncoder,
    },
};
use crate::machine_membrane::selected_operation::{
    SelectedCallRefusal, SelectedOperationContract, SelectedOperationPlan, bind_selected_operation,
};

#[derive(Debug, PartialEq, Eq)]
pub enum ControlOwnerRefusal {
    WrongBinding,
    Possession,
    Pressure,
    StaleTransfer,
    SequenceExhausted,
    Capability(BaseCapabilityRefusal),
    Decode(ControlDecodeRefusal),
    Result(ControlResultRefusal),
    Canonical(StructuredInfoRefusal),
}

/// Trusted physical facts for the native owner, not fields in a plot request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ControlAttachment {
    pub slot: u8,
    pub generation: u64,
    pub maximum_data_bytes: u16,
    pub resource_bytes: u64,
}

/// Move-only native submission. Neither serialized identifiers nor a new
/// request can reconstruct this operation's opaque Base lease.
pub(crate) struct NativeControlSubmission {
    attachment: ControlAttachment,
    request_id: RequestId,
    lease: BaseOperationLease,
    request: DecodedControlRequest,
}

pub(crate) enum NativeControlObservation<'a> {
    Completed { actual: u16, input: &'a [u8] },
    Disposition(ControlTransferDisposition),
}

pub struct ControlCallOwner {
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    node: NodeId,
    attachment: ControlAttachment,
    decoder: PreparedControlRequestDecoder,
    encoder: PreparedControlResultEncoder,
    pending: Option<(RequestId, BaseOperationLease)>,
    next_request: u32,
}

impl ControlCallOwner {
    /// Bind existing possession and an actual native attachment before Play.
    ///
    /// # Safety
    /// The native composition root must own this exact controller/slot and DMA
    /// storage under the claim's resource generation/envelope. No request may
    /// choose another slot or buffer. The physical owner must retain DMA/rings
    /// until acknowledged hardware quiescence, including on software revocation,
    /// provider loss and timeout. This constructor grants no resource authority
    /// and does not provide hostile-code containment in a shared address space.
    #[allow(dead_code)] // native composition installation remains a separate step
    pub(crate) unsafe fn bind_admitted(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        attachment: ControlAttachment,
        contract: &ControlContract,
        selection: SelectedOperationPlan<'_>,
    ) -> Result<Self, ControlOwnerRefusal> {
        let mut entries = table.inspections();
        let entry = entries.next().ok_or(ControlOwnerRefusal::Possession)?;
        if entries.next().is_some()
            || entry.scope.maximum_in_flight != 1
            || entry.scope.maximum_parameter_bytes < CONTROL_MAXIMUM_BYTES
            || entry.scope.maximum_result_bytes < CONTROL_MAXIMUM_BYTES
            || claim.operation_contract_id.as_str() != CONTROL_CALL
            || claim.subject_kind.as_str() != CONTROL_KIND
            || claim.parameter_bytes != CONTROL_MAXIMUM_BYTES
            || claim.work_units != 1
            || attachment.slot == 0
            || attachment.generation == 0
            || attachment.maximum_data_bytes == 0
            || attachment.maximum_data_bytes > 256
            || attachment.resource_bytes < u64::from(attachment.maximum_data_bytes)
        {
            return Err(ControlOwnerRefusal::Possession);
        }
        drop(entries);
        let node = bind_selected_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: contract.kind(),
                call: CONTROL_CALL,
                input_bytes: CONTROL_MAXIMUM_BYTES,
                output_bytes: CONTROL_MAXIMUM_BYTES,
                resource_bytes: attachment.resource_bytes,
            },
            selection,
        )
        .map_err(|refusal| match refusal {
            SelectedCallRefusal::WrongBinding => ControlOwnerRefusal::WrongBinding,
            SelectedCallRefusal::Possession => ControlOwnerRefusal::Possession,
        })?;
        Ok(Self {
            table,
            handle,
            claim,
            node,
            attachment,
            decoder: PreparedControlRequestDecoder::new(contract)
                .map_err(ControlOwnerRefusal::Canonical)?,
            encoder: PreparedControlResultEncoder::new(contract)
                .map_err(ControlOwnerRefusal::Canonical)?,
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
    ) -> Result<NativeControlSubmission, ControlOwnerRefusal> {
        self.check_binding(node, call)?;
        match self
            .table
            .inspections()
            .next()
            .ok_or(ControlOwnerRefusal::Possession)?
            .lifecycle
        {
            CapabilityLifecycle::Revoked => {
                return Err(ControlOwnerRefusal::Capability(
                    BaseCapabilityRefusal::Revoked,
                ));
            }
            CapabilityLifecycle::Exhausted => {
                return Err(ControlOwnerRefusal::Capability(
                    BaseCapabilityRefusal::Exhausted,
                ));
            }
            CapabilityLifecycle::Issued => {}
        }
        if self.pending.is_some() {
            return Err(ControlOwnerRefusal::Pressure);
        }
        if request_id != RequestId(self.next_request) {
            return Err(ControlOwnerRefusal::StaleTransfer);
        }
        let next = self
            .next_request
            .checked_add(1)
            .ok_or(ControlOwnerRefusal::SequenceExhausted)?;
        let request = self
            .decoder
            .decode(input)
            .map_err(ControlOwnerRefusal::Decode)?;
        request
            .request(self.attachment.maximum_data_bytes)
            .map_err(|reason| {
                ControlOwnerRefusal::Decode(ControlDecodeRefusal::Transfer(reason))
            })?;
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(ControlOwnerRefusal::Capability)?;
        self.next_request = next;
        self.pending = Some((request_id, lease.clone()));
        Ok(NativeControlSubmission {
            attachment: self.attachment,
            request_id,
            lease,
            request,
        })
    }

    /// Revoke software possession. The pending physical operation is deliberately
    /// retained; this does not claim a successful endpoint stop or DMA release.
    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), ControlOwnerRefusal> {
        self.check_binding(node, call)?;
        self.table
            .revoke(&self.handle)
            .map_err(ControlOwnerRefusal::Capability)
    }

    /// # Safety
    /// The native owner must have acknowledged quiescence of this exact slot,
    /// attachment generation and transfer: successful final Status Stage, or
    /// acknowledged endpoint/controller stop. A timeout, failed/foreign event,
    /// software cancellation or device-loss observation alone is insufficient.
    #[allow(dead_code)]
    pub(crate) unsafe fn finish_quiesced(
        &mut self,
        submission: &NativeControlSubmission,
        observation: NativeControlObservation<'_>,
    ) -> Result<&[u8], ControlOwnerRefusal> {
        if !self.pending.as_ref().is_some_and(|(sequence, lease)| {
            *sequence == submission.request_id && *lease == submission.lease
        }) || self.attachment != submission.attachment
        {
            return Err(ControlOwnerRefusal::StaleTransfer);
        }
        let request = submission
            .request
            .request(self.attachment.maximum_data_bytes)
            .map_err(|reason| {
                ControlOwnerRefusal::Decode(ControlDecodeRefusal::Transfer(reason))
            })?;
        let encoded = match observation {
            NativeControlObservation::Completed { actual, input } => self
                .encoder
                .completed(&request, actual, input)
                .map_err(ControlOwnerRefusal::Result),
            NativeControlObservation::Disposition(disposition) => self
                .encoder
                .disposition(disposition)
                .map_err(ControlOwnerRefusal::Canonical),
        };
        self.pending = None; // physical acknowledgement, independent of software acceptance
        let result_bytes = encoded.as_ref().map_or(0, |bytes| bytes.len() as u32);
        self.table
            .complete(&mut self.handle, submission.lease.clone(), result_bytes)
            .map_err(ControlOwnerRefusal::Capability)?;
        encoded
    }

    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), ControlOwnerRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(ControlOwnerRefusal::WrongBinding);
        }
        Ok(())
    }
}

impl NativeControlSubmission {
    #[allow(dead_code)]
    pub(crate) fn request(&self) -> Result<ControlTransferRequest<'_>, ControlRequestRefusal> {
        self.request.request(self.attachment.maximum_data_bytes)
    }
}

#[cfg(test)]
mod tests;
