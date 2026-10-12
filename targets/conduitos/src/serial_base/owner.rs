//! Exact admitted lifetime for a transport-neutral duplex serial provider.
//! This owner never chooses a device, class request, retry or transfer schedule.
#![cfg_attr(not(test), allow(dead_code))] // Root installation is a separate integration.
use super::{
    contract::{
        SERIAL_DUPLEX_CALL, SERIAL_DUPLEX_KIND, SERIAL_DUPLEX_MAXIMUM_BYTES, SerialDuplexContract,
    },
    request::{PreparedSerialRequestDecoder, SerialRequest, SerialRequestRefusal},
    result::{PreparedSerialResultEncoder, SerialDisposition, SerialResultRefusal},
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
pub enum SerialOwnerRefusal {
    Terminal,
    WrongBinding,
    Possession,
    Pressure,
    StaleRequest,
    SequenceExhausted,
    Capability(BaseCapabilityRefusal),
    Request(SerialRequestRefusal),
    Result(SerialResultRefusal),
    Canonical(StructuredInfoRefusal),
}
// Minimum semantic input/output staging reservation. Root must separately
// admit the actual implementation and prepared owner storage; this minimum
// does not prove a provider allocation or any underlying physical attachment.
pub const SERIAL_DUPLEX_RESOURCE_BYTES: u64 = 8192;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Transmit,
    Receive,
}

/// Move-only possession receipt. A serialized request cannot reconstruct it.
pub(crate) struct SerialSubmission<'a> {
    request_id: RequestId,
    lease: BaseOperationLease,
    request: SerialRequest<'a>,
}
pub(crate) enum SerialObservation<'a> {
    Transmitted { actual: u16 },
    Received { actual: u16, bytes: &'a [u8] },
    Disposition(SerialDisposition),
}
pub struct SerialDuplexCallOwner {
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    node: NodeId,
    decoder: PreparedSerialRequestDecoder,
    encoder: PreparedSerialResultEncoder,
    pending: Option<(RequestId, BaseOperationLease, Direction, u16)>,
    next_request: u32,
    terminal: bool,
}
impl SerialDuplexCallOwner {
    /// Bind exact existing possession and selected storage before Play.
    ///
    /// # Safety
    /// Root must own the actual serial provider for this Base/provider/resource
    /// generation and reserve all bounded storage before Play. A checked CDC
    /// Plot may realize the semantic Back, but this owner is not a CDC driver.
    /// Provider cancellation must retain any outstanding underlying operations
    /// until their exact completions or acknowledged physical stop; software
    /// revocation, timeout and disconnect alone cannot assert quiescence.
    pub(crate) unsafe fn bind_admitted(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &SerialDuplexContract,
        selection: SelectedOperationPlan<'_>,
    ) -> Result<Self, SerialOwnerRefusal> {
        let mut entries = table.inspections();
        let entry = entries.next().ok_or(SerialOwnerRefusal::Possession)?;
        if entries.next().is_some()
            || entry.scope.maximum_in_flight != 1
            || entry.scope.maximum_parameter_bytes < SERIAL_DUPLEX_MAXIMUM_BYTES
            || entry.scope.maximum_result_bytes < SERIAL_DUPLEX_MAXIMUM_BYTES
            || claim.base_provider_generation == 0
            || claim.operation_contract_id.as_str() != SERIAL_DUPLEX_CALL
            || claim.subject_kind.as_str() != SERIAL_DUPLEX_KIND
            || claim.parameter_bytes != SERIAL_DUPLEX_MAXIMUM_BYTES
            || claim.work_units != 1
        {
            return Err(SerialOwnerRefusal::Possession);
        }
        drop(entries);
        let node = bind_selected_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: contract.kind(),
                call: SERIAL_DUPLEX_CALL,
                input_bytes: SERIAL_DUPLEX_MAXIMUM_BYTES,
                output_bytes: SERIAL_DUPLEX_MAXIMUM_BYTES,
                resource_bytes: SERIAL_DUPLEX_RESOURCE_BYTES,
            },
            selection,
        )
        .map_err(|error| match error {
            SelectedCallRefusal::WrongBinding => SerialOwnerRefusal::WrongBinding,
            SelectedCallRefusal::Possession => SerialOwnerRefusal::Possession,
        })?;
        Ok(Self {
            table,
            handle,
            claim,
            node,
            decoder: PreparedSerialRequestDecoder::new(contract)
                .map_err(SerialOwnerRefusal::Canonical)?,
            encoder: PreparedSerialResultEncoder::new(contract)
                .map_err(SerialOwnerRefusal::Canonical)?,
            pending: None,
            next_request: 0,
            terminal: false,
        })
    }
    pub(crate) fn begin<'a>(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request_id: RequestId,
        input: &'a [u8],
    ) -> Result<SerialSubmission<'a>, SerialOwnerRefusal> {
        self.check_binding(node, call)?;
        match self
            .table
            .inspections()
            .next()
            .ok_or(SerialOwnerRefusal::Possession)?
            .lifecycle
        {
            CapabilityLifecycle::Revoked => {
                return Err(SerialOwnerRefusal::Capability(
                    BaseCapabilityRefusal::Revoked,
                ));
            }
            CapabilityLifecycle::Exhausted => {
                return Err(SerialOwnerRefusal::Capability(
                    BaseCapabilityRefusal::Exhausted,
                ));
            }
            CapabilityLifecycle::Issued => {}
        }
        if self.terminal {
            return Err(SerialOwnerRefusal::Terminal);
        }
        if self.pending.is_some() {
            return Err(SerialOwnerRefusal::Pressure);
        }
        if request_id != RequestId(self.next_request) {
            return Err(SerialOwnerRefusal::StaleRequest);
        }
        let next = self
            .next_request
            .checked_add(1)
            .ok_or(SerialOwnerRefusal::SequenceExhausted)?;
        let request = self
            .decoder
            .decode(input)
            .map_err(SerialOwnerRefusal::Request)?;
        let direction = match request {
            SerialRequest::Transmit(_) => Direction::Transmit,
            SerialRequest::Receive(_) => Direction::Receive,
        };
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(SerialOwnerRefusal::Capability)?;
        self.pending = Some((request_id, lease.clone(), direction, request.requested()));
        self.next_request = next;
        Ok(SerialSubmission {
            request_id,
            lease,
            request,
        })
    }
    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), SerialOwnerRefusal> {
        self.check_binding(node, call)?;
        self.terminal = true;
        self.table
            .revoke(&self.handle)
            .map_err(SerialOwnerRefusal::Capability)
    }
    /// # Safety
    /// The provider must acknowledge completion of this exact semantic request
    /// and all underlying operations that could still access retained storage.
    /// A typed terminal disposition alone does not establish this fact.
    pub(crate) unsafe fn finish_acknowledged(
        &mut self,
        submission: &SerialSubmission<'_>,
        observation: SerialObservation<'_>,
    ) -> Result<&[u8], SerialOwnerRefusal> {
        self.check_submission(submission)?;
        let requested = submission.request.requested();
        let ordinal = u64::from(submission.request_id.0);
        let terminal = matches!(observation, SerialObservation::Disposition(_));
        let encoded = match (&submission.request, observation) {
            (SerialRequest::Transmit(_), SerialObservation::Transmitted { actual }) => self
                .encoder
                .transmitted(ordinal, requested, actual)
                .map_err(SerialOwnerRefusal::Result)?,
            (SerialRequest::Receive(_), SerialObservation::Received { actual, bytes }) => self
                .encoder
                .received(ordinal, requested, actual, bytes)
                .map_err(SerialOwnerRefusal::Result)?,
            (_, SerialObservation::Disposition(disposition)) => self
                .encoder
                .disposition(disposition)
                .map_err(SerialOwnerRefusal::Canonical)?,
            _ => return Err(SerialOwnerRefusal::WrongBinding),
        };
        self.pending = None;
        self.terminal |= terminal;
        self.table
            .complete(
                &mut self.handle,
                submission.lease.clone(),
                encoded.len() as u32,
            )
            .map_err(SerialOwnerRefusal::Capability)?;
        Ok(encoded)
    }
    /// # Safety
    /// An actual acknowledged provider stop must cover this exact request and
    /// every retained underlying operation before storage/lease retirement.
    pub(crate) unsafe fn discard_acknowledged(
        &mut self,
        submission: &SerialSubmission<'_>,
    ) -> Result<(), SerialOwnerRefusal> {
        self.check_submission(submission)?;
        self.pending = None;
        self.terminal = true;
        self.table
            .complete(&mut self.handle, submission.lease.clone(), 0)
            .map_err(SerialOwnerRefusal::Capability)
    }
    fn check_submission(
        &self,
        submission: &SerialSubmission<'_>,
    ) -> Result<(), SerialOwnerRefusal> {
        let direction = match submission.request {
            SerialRequest::Transmit(_) => Direction::Transmit,
            SerialRequest::Receive(_) => Direction::Receive,
        };
        if !self
            .pending
            .as_ref()
            .is_some_and(|(id, lease, dir, length)| {
                *id == submission.request_id
                    && *lease == submission.lease
                    && *dir == direction
                    && *length == submission.request.requested()
            })
        {
            return Err(SerialOwnerRefusal::StaleRequest);
        }
        Ok(())
    }
    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), SerialOwnerRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(SerialOwnerRefusal::WrongBinding);
        }
        Ok(())
    }
}
impl SerialSubmission<'_> {
    pub(crate) fn request(&self) -> &SerialRequest<'_> {
        &self.request
    }
}
#[cfg(test)]
mod tests;
