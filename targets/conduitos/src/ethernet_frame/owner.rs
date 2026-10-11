//! Exact selected network possession; USB discovery supplies no such authority.
use super::{contract::*, factory::FRAME_AUTHORITY, queues::*};
use crate::machine_membrane::selected_operation::{
    SelectedOperationContract, SelectedOperationPlan, bind_selected_operation,
};
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
#[derive(Debug)]
pub(crate) enum FrameCallRefusal {
    Binding,
    Possession,
    StaleRequest,
    Capability(BaseCapabilityRefusal),
    Canonical(StructuredInfoRefusal),
}
/// The Root owns the same fixed queues borrowed by the existing stack Device.
/// No frame request selects a provider, host address, generation or authority.
pub(crate) struct EthernetFrameCallOwner<'a> {
    queues: &'a EthernetFrameQueues,
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    node: NodeId,
    next: u32,
    validator: PreparedStructuredValueValidator,
    result: PreparedStructuredComposer,
    transmit: PreparedStructuredComposer,
    ordinal: PreparedStructuredComposer,
    wire: PreparedStructuredComposer,
    unit: alloc::vec::Vec<u8>,
    scratch: [u8; FRAME_BYTES],
}
impl<'a> EthernetFrameCallOwner<'a> {
    /// # Safety
    /// Root owns these exact queues under the selected independent network Base,
    /// generation and envelope, and binds one stack Device to the same storage.
    /// This grants no USB, network service, interface or destination authority.
    pub(crate) unsafe fn bind_admitted(
        queues: &'a EthernetFrameQueues,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &EthernetFrameContract,
        selection: SelectedOperationPlan<'_>,
    ) -> Result<Self, FrameCallRefusal> {
        let mut entries = table.inspections();
        let entry = entries.next().ok_or(FrameCallRefusal::Possession)?;
        if entries.next().is_some()
            || entry.scope.authority_contract_id.as_str() != FRAME_AUTHORITY
            || entry.scope.maximum_in_flight != 1
            || claim.base_provider_generation != queues.generation()
            || claim.operation_contract_id.as_str() != FRAME_CALL
            || claim.subject_kind.as_str() != FRAME_KIND
            || claim.parameter_bytes != FRAME_MAXIMUM_BYTES
            || claim.work_units != 1
            || entry.scope.maximum_parameter_bytes < FRAME_MAXIMUM_BYTES
            || entry.scope.maximum_result_bytes < FRAME_MAXIMUM_BYTES
        {
            return Err(FrameCallRefusal::Possession);
        }
        drop(entries);
        let node = bind_selected_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: contract.kind(),
                call: FRAME_CALL,
                input_bytes: FRAME_MAXIMUM_BYTES,
                output_bytes: FRAME_MAXIMUM_BYTES,
                resource_bytes: core::mem::size_of::<EthernetFrameQueues>() as u64,
            },
            selection,
        )
        .map_err(|_| FrameCallRefusal::Binding)?;
        let StructuredInfoTypeShape::Variant { cases, .. } = contract.result_type().shape() else {
            return Err(FrameCallRefusal::Binding);
        };
        let transmit = cases
            .iter()
            .find(|case| case.tag() == "transmit")
            .ok_or(FrameCallRefusal::Binding)?
            .payload_type();
        let StructuredInfoTypeShape::Record { fields, .. } = transmit.shape() else {
            return Err(FrameCallRefusal::Binding);
        };
        let wire = fields
            .iter()
            .find(|field| field.name() == "wire")
            .ok_or(FrameCallRefusal::Binding)?
            .value_type();
        let prepared = |ty: &StructuredInfoType, max| {
            PreparedStructuredComposer::new(ty, max).map_err(FrameCallRefusal::Canonical)
        };
        let mut empty = prepared(
            &StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID))
                .map_err(FrameCallRefusal::Canonical)?,
            64,
        )?;
        let unit = empty
            .leaf(&[])
            .map_err(FrameCallRefusal::Canonical)?
            .to_vec();
        Ok(Self {
            queues,
            table,
            handle,
            claim,
            node,
            next: 0,
            validator: contract
                .request_validator()
                .map_err(FrameCallRefusal::Canonical)?,
            result: prepared(contract.result_type(), 4096)?,
            transmit: prepared(transmit, 4096)?,
            ordinal: prepared(
                &StructuredInfoType::leaf(kind_id("value/u64"))
                    .map_err(FrameCallRefusal::Canonical)?,
                64,
            )?,
            wire: prepared(wire, 4096)?,
            unit,
            scratch: [0; FRAME_BYTES],
        })
    }
    pub(crate) fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<&[u8], FrameCallRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(FrameCallRefusal::Binding);
        }
        if request != RequestId(self.next) {
            return Err(FrameCallRefusal::StaleRequest);
        }
        let next = self
            .next
            .checked_add(1)
            .ok_or(FrameCallRefusal::StaleRequest)?;
        self.validator
            .validate(input)
            .map_err(FrameCallRefusal::Canonical)?;
        let value =
            validate_canonical_structured_value(input).map_err(FrameCallRefusal::Canonical)?;
        let tag = value.variant_tag().map_err(FrameCallRefusal::Canonical)?;
        let payload = value
            .variant_payload(tag)
            .map_err(FrameCallRefusal::Canonical)?
            .ok_or(FrameCallRefusal::Binding)?;
        // Decode every projection before possession creates a live lease.
        enum Request<'a> {
            Take,
            Deliver(&'a [u8]),
            Acknowledge(u64, u64),
        }
        let decoded = match tag {
            "take-transmit" => Request::Take,
            "deliver-receive" => Request::Deliver(
                payload
                    .record_field("wire")
                    .map_err(FrameCallRefusal::Canonical)?
                    .ok_or(FrameCallRefusal::Binding)?
                    .primitive_bytes("value/bytes")
                    .map_err(FrameCallRefusal::Canonical)?,
            ),
            "acknowledge-transmit" => {
                let field = |name| -> Result<u64, FrameCallRefusal> {
                    let bytes = payload
                        .record_field(name)
                        .map_err(FrameCallRefusal::Canonical)?
                        .ok_or(FrameCallRefusal::Binding)?
                        .primitive_bytes("value/u64")
                        .map_err(FrameCallRefusal::Canonical)?;
                    Ok(u64::from_le_bytes(
                        bytes.try_into().map_err(|_| FrameCallRefusal::Binding)?,
                    ))
                };
                Request::Acknowledge(field("ordinal")?, field("actual")?)
            }
            _ => return Err(FrameCallRefusal::Binding),
        };
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(FrameCallRefusal::Capability)?;
        self.next = next;
        let generation = self.queues.generation();
        let result = match decoded {
            Request::Take => self
                .queues
                .take_transmit(generation, &mut self.scratch)
                .map(Some),
            Request::Deliver(wire) => self.queues.deliver_receive(generation, wire).map(|()| None),
            Request::Acknowledge(ordinal, actual) => self
                .queues
                .acknowledge_transmit(generation, ordinal, actual)
                .map(|()| None),
        };
        let output = match result {
            Ok(Some((ordinal, length))) => {
                let ordinal = self
                    .ordinal
                    .leaf(&ordinal.to_le_bytes())
                    .map_err(FrameCallRefusal::Canonical)?;
                let wire = self
                    .wire
                    .leaf(&self.scratch[..length])
                    .map_err(FrameCallRefusal::Canonical)?;
                let transmit = self
                    .transmit
                    .record(&[
                        validate_canonical_structured_value(ordinal)
                            .map_err(FrameCallRefusal::Canonical)?,
                        validate_canonical_structured_value(wire)
                            .map_err(FrameCallRefusal::Canonical)?,
                    ])
                    .map_err(FrameCallRefusal::Canonical)?;
                self.result.variant(
                    "transmit",
                    validate_canonical_structured_value(transmit)
                        .map_err(FrameCallRefusal::Canonical)?,
                )
            }
            other => {
                let tag = match other {
                    Ok(None) => {
                        if tag == "deliver-receive" {
                            "accepted"
                        } else {
                            "acknowledged"
                        }
                    }
                    Err(FrameRefusal::Pressure) => "pressure",
                    Err(FrameRefusal::Cancelled) => "cancelled",
                    Err(FrameRefusal::Stale) => "stale",
                    Err(FrameRefusal::Sequence) => "sequence-exhausted",
                    Err(FrameRefusal::ProviderLost) => "provider-lost",
                    Err(_) => "malformed",
                    Ok(Some(_)) => unreachable!(),
                };
                self.result.variant(
                    tag,
                    validate_canonical_structured_value(&self.unit)
                        .map_err(FrameCallRefusal::Canonical)?,
                )
            }
        }
        .map_err(FrameCallRefusal::Canonical)?;
        self.table
            .complete(&mut self.handle, lease, output.len() as u32)
            .map_err(FrameCallRefusal::Capability)?;
        Ok(output)
    }
    pub(crate) fn cancel(&mut self) -> Result<(), FrameCallRefusal> {
        self.table
            .revoke(&self.handle)
            .map_err(FrameCallRefusal::Capability)?;
        self.queues.cancel();
        Ok(())
    }
}

#[cfg(test)]
mod tests;
