//! One admitted capability lease covers the complete bounded ISO batch.
#![cfg_attr(not(test), allow(dead_code))] // Root native installation is a separate handoff.
use super::{contract::*, request::*, result::*};
use crate::machine_membrane::selected_operation::{
    SelectedCallRefusal, SelectedOperationContract, SelectedOperationPlan, bind_selected_operation,
};
use conduit_core::{
    BaseCapabilityHandle, BaseCapabilityRefusal, BaseCapabilityTable, BaseOperationClaim,
    BaseOperationLease, CapabilityLifecycle, StructuredInfoRefusal,
};
use conduit_kernel::{HostCallId, NodeId, RequestId};

/// Trusted original native attachment facts. Serializing these does not confer possession.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IsochronousAttachment {
    pub slot: u8,
    pub generation: u64,
    pub endpoint_dci: u8,
    pub endpoint_generation: u64,
    pub maximum_packet_bytes: u16,
    /// Complete admitted envelope, including native DMA and all Root metadata.
    pub resource_bytes: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub enum IsochronousOwnerRefusal {
    WrongBinding,
    Possession,
    Pressure,
    StaleTransfer,
    SequenceExhausted,
    Observation,
    Capability(BaseCapabilityRefusal),
    Decode(IsochronousRequestRefusal),
    Canonical(StructuredInfoRefusal),
}
pub(crate) struct NativeIsochronousSubmission {
    attachment: IsochronousAttachment,
    request_id: RequestId,
    lease: BaseOperationLease,
    request: DecodedIsochronousRequest,
}
impl NativeIsochronousSubmission {
    pub(crate) fn request(&self) -> &DecodedIsochronousRequest {
        &self.request
    }
}
pub struct IsochronousOutCallOwner {
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    node: NodeId,
    attachment: IsochronousAttachment,
    decoder: PreparedIsochronousRequestDecoder,
    encoder: PreparedIsochronousResultEncoder,
    pending: Option<(RequestId, BaseOperationLease)>,
    next_request: u32,
    closed: bool,
}
impl IsochronousOutCallOwner {
    /// # Safety
    /// Root owns the original configured ISO endpoint, controller/event consumer,
    /// eight DMA packets/ring and all decoder/encoder storage under this exact
    /// provider/resource generation and admitted envelope. Retain every buffer
    /// and the original submission until all exact TDs complete or original
    /// Disable Slot acknowledges all-stop; software revocation is insufficient.
    pub(crate) unsafe fn bind_admitted(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        attachment: IsochronousAttachment,
        contract: &IsochronousOutContract,
        selection: SelectedOperationPlan<'_>,
    ) -> Result<Self, IsochronousOwnerRefusal> {
        use IsochronousOwnerRefusal as Error;
        let mut entries = table.inspections();
        let entry = entries.next().ok_or(Error::Possession)?;
        if entries.next().is_some()
            || entry.scope.maximum_in_flight != 1
            || entry.scope.maximum_parameter_bytes < ISOCHRONOUS_OUT_MAXIMUM_BYTES
            || entry.scope.maximum_result_bytes < ISOCHRONOUS_OUT_MAXIMUM_BYTES
            || claim.operation_contract_id.as_str() != ISOCHRONOUS_OUT_CALL
            || claim.subject_kind.as_str() != ISOCHRONOUS_OUT_KIND
            || claim.parameter_bytes != ISOCHRONOUS_OUT_MAXIMUM_BYTES
            || claim.work_units != 8
            || attachment.slot == 0
            || attachment.generation == 0
            || attachment.endpoint_generation == 0
            || !(2..=30).contains(&attachment.endpoint_dci)
            || !attachment.endpoint_dci.is_multiple_of(2)
            || attachment.maximum_packet_bytes == 0
            || attachment.maximum_packet_bytes > 256
            || attachment.resource_bytes < 3072
        {
            return Err(Error::Possession);
        }
        drop(entries);
        let node = bind_selected_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: contract.kind(),
                call: ISOCHRONOUS_OUT_CALL,
                input_bytes: ISOCHRONOUS_OUT_MAXIMUM_BYTES,
                output_bytes: ISOCHRONOUS_OUT_MAXIMUM_BYTES,
                resource_bytes: attachment.resource_bytes,
            },
            selection,
        )
        .map_err(|error| match error {
            SelectedCallRefusal::WrongBinding => Error::WrongBinding,
            SelectedCallRefusal::Possession => Error::Possession,
        })?;
        Ok(Self {
            table,
            handle,
            claim,
            node,
            attachment,
            decoder: PreparedIsochronousRequestDecoder::new(contract).map_err(Error::Canonical)?,
            encoder: PreparedIsochronousResultEncoder::new(contract).map_err(Error::Canonical)?,
            pending: None,
            next_request: 0,
            closed: false,
        })
    }
    pub(crate) fn begin(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request_id: RequestId,
        input: &[u8],
    ) -> Result<NativeIsochronousSubmission, IsochronousOwnerRefusal> {
        use IsochronousOwnerRefusal as Error;
        self.check_binding(node, call)?;
        match self
            .table
            .inspections()
            .next()
            .ok_or(Error::Possession)?
            .lifecycle
        {
            CapabilityLifecycle::Revoked => {
                return Err(Error::Capability(BaseCapabilityRefusal::Revoked));
            }
            CapabilityLifecycle::Exhausted => {
                return Err(Error::Capability(BaseCapabilityRefusal::Exhausted));
            }
            CapabilityLifecycle::Issued => {}
        }
        if self.closed {
            return Err(Error::Possession);
        }
        if self.pending.is_some() {
            return Err(Error::Pressure);
        }
        if request_id != RequestId(self.next_request) {
            return Err(Error::StaleTransfer);
        }
        let next = self
            .next_request
            .checked_add(1)
            .ok_or(Error::SequenceExhausted)?;
        let request = self
            .decoder
            .decode(input, self.attachment.maximum_packet_bytes)
            .map_err(Error::Decode)?;
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(Error::Capability)?;
        self.pending = Some((request_id, lease.clone()));
        self.next_request = next;
        Ok(NativeIsochronousSubmission {
            attachment: self.attachment,
            request_id,
            lease,
            request,
        })
    }
    pub fn revoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
    ) -> Result<(), IsochronousOwnerRefusal> {
        self.check_binding(node, call)?;
        self.closed = true;
        self.table
            .revoke(&self.handle)
            .map_err(IsochronousOwnerRefusal::Capability)
    }
    /// # Safety
    /// Every completed metadata item represents its exact correlated native TD.
    /// Batch success acknowledges every submitted TD. Every fault/Drain requires
    /// acknowledged all-stop from the original sole controller before publication.
    /// Snapshot comes from that same original controller. Never call on begin Ok,
    /// timeout, foreign completion, unplug observation alone or failed stop.
    pub(crate) unsafe fn finish_quiesced(
        &mut self,
        submission: &NativeIsochronousSubmission,
        completed: &[IsochronousCompletion],
        outcome: IsochronousOutcome,
    ) -> Result<&[u8], IsochronousOwnerRefusal> {
        use IsochronousOwnerRefusal as Error;
        self.check_submission(submission)?;
        self.validate_observation(submission, completed, outcome)?;
        let encoded = self
            .encoder
            .encode(completed, outcome)
            .map_err(Error::Canonical)?;
        self.table
            .complete(
                &mut self.handle,
                submission.lease.clone(),
                encoded.len() as u32,
            )
            .map_err(Error::Capability)?;
        self.pending = None;
        self.closed |= !matches!(
            outcome,
            IsochronousOutcome::Completed | IsochronousOutcome::Snapshot(_)
        );
        Ok(encoded)
    }
    /// # Safety
    /// Original controller acknowledges all-stop covering every packet of this
    /// exact retained submission, or Root proves no packet was ever published.
    /// Revocation alone does not authorize dropping this batch or any DMA.
    pub(crate) unsafe fn discard_quiesced(
        &mut self,
        submission: &NativeIsochronousSubmission,
    ) -> Result<(), IsochronousOwnerRefusal> {
        self.check_submission(submission)?;
        match self
            .table
            .complete(&mut self.handle, submission.lease.clone(), 0)
        {
            Ok(()) | Err(BaseCapabilityRefusal::StaleCompletion) => {}
            Err(error) => return Err(IsochronousOwnerRefusal::Capability(error)),
        }
        self.pending = None;
        Ok(())
    }
    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), IsochronousOwnerRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(IsochronousOwnerRefusal::WrongBinding);
        }
        Ok(())
    }
    fn check_submission(
        &self,
        submission: &NativeIsochronousSubmission,
    ) -> Result<(), IsochronousOwnerRefusal> {
        if !self.pending.as_ref().is_some_and(|(request, lease)| {
            *request == submission.request_id && *lease == submission.lease
        }) || self.attachment != submission.attachment
        {
            return Err(IsochronousOwnerRefusal::StaleTransfer);
        }
        Ok(())
    }
    fn validate_observation(
        &self,
        submission: &NativeIsochronousSubmission,
        completed: &[IsochronousCompletion],
        outcome: IsochronousOutcome,
    ) -> Result<(), IsochronousOwnerRefusal> {
        use IsochronousOwnerRefusal as Error;
        match &submission.request {
            DecodedIsochronousRequest::Observe => {
                let IsochronousOutcome::Snapshot(snapshot) = outcome else {
                    return Err(Error::Observation);
                };
                if !completed.is_empty()
                    || snapshot.current_microframe > 0x3fff
                    || snapshot.scheduling_threshold > 15
                    || snapshot.service_interval_microframes != 8
                    || snapshot.maximum_packet_bytes != self.attachment.maximum_packet_bytes
                    || snapshot.pending_packets != 0
                    || snapshot.maximum_pending_packets != 8
                    || snapshot.dma_resource_bytes != 3072
                {
                    return Err(Error::Observation);
                }
            }
            DecodedIsochronousRequest::Drain => {
                if outcome != IsochronousOutcome::Drained || !completed.is_empty() {
                    return Err(Error::Observation);
                }
            }
            DecodedIsochronousRequest::Batch(batch) => {
                if completed.len() > batch.count() {
                    return Err(Error::Observation);
                }
                for (index, completion) in completed.iter().enumerate() {
                    let (frame, wire) = batch.packet(index).ok_or(Error::Observation)?;
                    if completion.frame != frame.number()
                        || usize::from(completion.requested) != wire.len()
                        || completion.actual != completion.requested
                    {
                        return Err(Error::Observation);
                    }
                }
                match outcome {
                    IsochronousOutcome::Completed if completed.len() == batch.count() => {}
                    IsochronousOutcome::Short(packet) => {
                        let (frame, wire) =
                            batch.packet(completed.len()).ok_or(Error::Observation)?;
                        if packet.frame != frame.number()
                            || usize::from(packet.requested) != wire.len()
                            || packet.actual >= packet.requested
                        {
                            return Err(Error::Observation);
                        }
                    }
                    IsochronousOutcome::MissedService { frame } => {
                        if batch
                            .packet(completed.len())
                            .is_none_or(|(expected, _)| expected.number() != frame)
                        {
                            return Err(Error::Observation);
                        }
                    }
                    IsochronousOutcome::Underrun | IsochronousOutcome::ProviderLost
                        if completed.len() < batch.count() => {}
                    _ => return Err(Error::Observation),
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "owner/tests.rs"]
mod tests;
