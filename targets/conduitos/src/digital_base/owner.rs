//! Exact planned possession for a synchronous, finitely bounded digital Host Call.
use super::{
    DigitalProvider,
    contract::{DIGITAL_CALL, DIGITAL_MAXIMUM_BYTES, DigitalContract},
    decode::{DigitalDecodeRefusal, PreparedDigitalDecoder},
    result::PreparedDigitalResultEncoder,
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
pub struct DigitalAttachment {
    pub generation: u64,
    pub resource_bytes: u64,
}

pub struct DigitalCallSelection<'a> {
    pub contract: &'a DigitalContract,
    pub fragment: &'a PlanFragment,
    pub lowered: &'a LoweredPlanFragment,
    pub active: &'a ActivePlayIdentity,
    pub placement: &'a PlacementId,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DigitalCallRefusal {
    WrongBinding,
    Possession,
    StaleRequest,
    SequenceExhausted,
    Capability(BaseCapabilityRefusal),
    Decode(DigitalDecodeRefusal),
    Canonical(StructuredInfoRefusal),
}

pub struct DigitalHostCall<P: DigitalProvider> {
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    provider: super::provider_guard::ProviderGuard<P>,
    node: NodeId,
    next_request: u32,
    decoder: PreparedDigitalDecoder,
    encoder: PreparedDigitalResultEncoder,
}

impl<P: DigitalProvider> DigitalHostCall<P> {
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
        attachment: DigitalAttachment,
        provider: P,
        selection: DigitalCallSelection<'_>,
    ) -> Result<Self, DigitalCallRefusal> {
        let provider = super::provider_guard::ProviderGuard::new(provider);
        let mut entries = table.inspections();
        let entry = entries.next().ok_or(DigitalCallRefusal::Possession)?;
        if entries.next().is_some()
            || entry.scope.maximum_in_flight != 1
            || entry.scope.maximum_parameter_bytes < DIGITAL_MAXIMUM_BYTES
            || entry.scope.maximum_result_bytes < DIGITAL_MAXIMUM_BYTES
            || claim.parameter_bytes != DIGITAL_MAXIMUM_BYTES
            || claim.work_units != 1
            || attachment.generation == 0
            || attachment.resource_bytes < 1
        {
            return Err(DigitalCallRefusal::Possession);
        }
        drop(entries);
        let node = bind_selected_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: selection.contract.kind(),
                call: DIGITAL_CALL,
                input_bytes: DIGITAL_MAXIMUM_BYTES,
                output_bytes: DIGITAL_MAXIMUM_BYTES,
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
            SelectedCallRefusal::WrongBinding => DigitalCallRefusal::WrongBinding,
            SelectedCallRefusal::Possession => DigitalCallRefusal::Possession,
        })?;
        Ok(Self {
            table,
            handle,
            claim,
            provider,
            node,
            next_request: 0,
            decoder: PreparedDigitalDecoder::new(selection.contract)
                .map_err(DigitalCallRefusal::Canonical)?,
            encoder: PreparedDigitalResultEncoder::new(selection.contract)
                .map_err(DigitalCallRefusal::Canonical)?,
        })
    }

    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request_id: RequestId,
        input: &[u8],
    ) -> Result<&[u8], DigitalCallRefusal> {
        self.check_binding(node, call)?;
        if request_id != RequestId(self.next_request) {
            return Err(DigitalCallRefusal::StaleRequest);
        }
        let next = self
            .next_request
            .checked_add(1)
            .ok_or(DigitalCallRefusal::SequenceExhausted)?;
        let request = self
            .decoder
            .decode(input)
            .map_err(DigitalCallRefusal::Decode)?;
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(DigitalCallRefusal::Capability)?;
        self.next_request = next; // an admitted dispatch cannot be replayed
        let observed = self.provider.access(request);
        let encoded = match observed {
            Ok(high) => self
                .encoder
                .completed(high)
                .map_err(DigitalCallRefusal::Canonical),
            Err(reason) => self
                .encoder
                .disposition(reason)
                .map_err(DigitalCallRefusal::Canonical),
        };
        let result_bytes = encoded.as_ref().map_or(0, |bytes| bytes.len() as u32);
        // A physical refusal or malformed result still consumes the admitted
        // operation; no hidden retry and no leaked in-flight lease.
        self.table
            .complete(&mut self.handle, lease, result_bytes)
            .map_err(DigitalCallRefusal::Capability)?;
        encoded
    }

    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), DigitalCallRefusal> {
        self.check_binding(node, call)?;
        let result = self
            .table
            .revoke(&self.handle)
            .map_err(DigitalCallRefusal::Capability);
        // Mandatory local safety does not depend on a successful capability update.
        self.provider.revoke();
        result
    }

    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), DigitalCallRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(DigitalCallRefusal::WrongBinding);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
