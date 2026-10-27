//! Exact planned possession for a synchronous, finitely bounded SPI Host Call.
use super::{
    contract::{SPI_CALL, SPI_MAXIMUM_BYTES, SpiContract},
    decode::{PreparedSpiDecoder, SpiDecodeRefusal},
    result::{PreparedSpiResultEncoder, SpiResultRefusal},
    transaction::{MAXIMUM_TRANSACTION_BYTES, SpiProvider},
};
use crate::machine_membrane::selected_operation::{
    SelectedCallRefusal, SelectedOperationContract, SelectedOperationPlan, bind_selected_operation,
};
use conduit_core::{
    ActivePlayIdentity, BaseCapabilityHandle, BaseCapabilityRefusal, BaseCapabilityTable,
    BaseOperationClaim, PlacementId, PlanFragment, StructuredInfoRefusal,
};
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::lowering::LoweredPlanFragment;

/// Native attachment truth supplied independently of an authored request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpiAttachment {
    pub generation: u64,
    pub resource_bytes: u64,
}

pub struct SpiCallSelection<'a> {
    pub contract: &'a SpiContract,
    pub fragment: &'a PlanFragment,
    pub lowered: &'a LoweredPlanFragment,
    pub active: &'a ActivePlayIdentity,
    pub placement: &'a PlacementId,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SpiCallRefusal {
    WrongBinding,
    Possession,
    StaleRequest,
    SequenceExhausted,
    Capability(BaseCapabilityRefusal),
    Decode(SpiDecodeRefusal),
    Result(SpiResultRefusal),
    Canonical(StructuredInfoRefusal),
}

pub struct SpiHostCall<P> {
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    provider: P,
    node: NodeId,
    next_request: u32,
    decoder: PreparedSpiDecoder,
    encoder: PreparedSpiResultEncoder,
    input: [u8; MAXIMUM_TRANSACTION_BYTES],
}

impl<P: SpiProvider> SpiHostCall<P> {
    /// Bind actual native ownership to an already admitted exact Plan and Play.
    ///
    /// # Safety
    /// The trusted native composition root must own this exact controller,
    /// attachment generation and electrical resources under the claim. The
    /// bus configuration and routing are independently authorized attachment
    /// truth, never inferred from a plot or successful probe. The provider must enforce
    /// finite termination/quiescence, electrical invariants and mandatory safety
    /// even if ordinary protocol work fails. This cooperative boundary does not
    /// confine hostile native code sharing its address space.
    pub unsafe fn bind_selected(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        attachment: SpiAttachment,
        provider: P,
        selection: SpiCallSelection<'_>,
    ) -> Result<Self, SpiCallRefusal> {
        let mut entries = table.inspections();
        let entry = entries.next().ok_or(SpiCallRefusal::Possession)?;
        if entries.next().is_some()
            || entry.scope.maximum_in_flight != 1
            || entry.scope.maximum_parameter_bytes < SPI_MAXIMUM_BYTES
            || entry.scope.maximum_result_bytes < SPI_MAXIMUM_BYTES
            || claim.parameter_bytes != SPI_MAXIMUM_BYTES
            || claim.work_units != 1
            || attachment.generation == 0
            || attachment.resource_bytes < MAXIMUM_TRANSACTION_BYTES as u64
        {
            return Err(SpiCallRefusal::Possession);
        }
        drop(entries);
        let node = bind_selected_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: selection.contract.kind(),
                call: SPI_CALL,
                input_bytes: SPI_MAXIMUM_BYTES,
                output_bytes: SPI_MAXIMUM_BYTES,
                resource_bytes: attachment.resource_bytes,
            },
            SelectedOperationPlan {
                fragment: selection.fragment,
                lowered: selection.lowered,
                active: selection.active,
                placement_id: selection.placement,
            },
        )
        .map_err(|reason| match reason {
            SelectedCallRefusal::WrongBinding => SpiCallRefusal::WrongBinding,
            SelectedCallRefusal::Possession => SpiCallRefusal::Possession,
        })?;
        Ok(Self {
            table,
            handle,
            claim,
            provider,
            node,
            next_request: 0,
            decoder: PreparedSpiDecoder::new(selection.contract)
                .map_err(SpiCallRefusal::Canonical)?,
            encoder: PreparedSpiResultEncoder::new(selection.contract)
                .map_err(SpiCallRefusal::Canonical)?,
            input: [0; MAXIMUM_TRANSACTION_BYTES],
        })
    }

    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request_id: RequestId,
        input: &[u8],
    ) -> Result<&[u8], SpiCallRefusal> {
        self.check_binding(node, call)?;
        if request_id != RequestId(self.next_request) {
            return Err(SpiCallRefusal::StaleRequest);
        }
        let next = self
            .next_request
            .checked_add(1)
            .ok_or(SpiCallRefusal::SequenceExhausted)?;
        let request = self.decoder.decode(input).map_err(SpiCallRefusal::Decode)?;
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(SpiCallRefusal::Capability)?;
        self.next_request = next; // an admitted dispatch cannot be replayed
        self.input.fill(0); // no preceding exchange may leak into the next result
        let observed = self
            .provider
            .exchange(&request, &mut self.input[..request.output().len()]);
        let encoded = match observed {
            Ok(()) => self
                .encoder
                .completed(
                    request.output().len(),
                    &self.input[..request.output().len()],
                )
                .map_err(SpiCallRefusal::Result),
            Err(reason) => self
                .encoder
                .disposition(reason)
                .map_err(SpiCallRefusal::Canonical),
        };
        let result_bytes = encoded.as_ref().map_or(0, |bytes| bytes.len() as u32);
        // A physical refusal or malformed result still consumes the admitted
        // operation; no hidden retry and no leaked in-flight lease.
        self.table
            .complete(&mut self.handle, lease, result_bytes)
            .map_err(SpiCallRefusal::Capability)?;
        encoded
    }

    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), SpiCallRefusal> {
        self.check_binding(node, call)?;
        self.table
            .revoke(&self.handle)
            .map_err(SpiCallRefusal::Capability)?;
        self.provider.revoke();
        Ok(())
    }

    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), SpiCallRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(SpiCallRefusal::WrongBinding);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
