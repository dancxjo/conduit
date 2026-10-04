//! Exact planned possession for a synchronous, finitely bounded I2C Host Call.
use super::{
    contract::{I2C_CALL, I2C_MAXIMUM_BYTES, I2cContract},
    decode::{I2cDecodeRefusal, PreparedI2cDecoder},
    result::{I2cResultRefusal, PreparedI2cResultEncoder},
    transaction::{I2cDisposition, I2cProvider, MAXIMUM_TRANSACTION_BYTES},
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
pub struct I2cAttachment {
    pub generation: u64,
    pub minimum_address: u8,
    pub maximum_address: u8,
    pub resource_bytes: u64,
}

pub struct I2cCallSelection<'a> {
    pub contract: &'a I2cContract,
    pub fragment: &'a PlanFragment,
    pub lowered: &'a LoweredPlanFragment,
    pub active: &'a ActivePlayIdentity,
    pub placement: &'a PlacementId,
}

#[derive(Debug, PartialEq, Eq)]
pub enum I2cCallRefusal {
    WrongBinding,
    Possession,
    StaleRequest,
    SequenceExhausted,
    Capability(BaseCapabilityRefusal),
    Decode(I2cDecodeRefusal),
    Result(I2cResultRefusal),
    Canonical(StructuredInfoRefusal),
}

pub struct I2cHostCall<P> {
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    attachment: I2cAttachment,
    provider: P,
    node: NodeId,
    next_request: u32,
    decoder: PreparedI2cDecoder,
    encoder: PreparedI2cResultEncoder,
    input: [u8; MAXIMUM_TRANSACTION_BYTES],
}

impl<P: I2cProvider> I2cHostCall<P> {
    /// Bind actual native ownership to an already admitted exact Plan and Play.
    ///
    /// # Safety
    /// The trusted native composition root must own this exact controller,
    /// attachment generation and electrical resources under the claim. The
    /// address interval is independently authorized local attachment truth,
    /// never inferred from a plot or successful probe. The provider must enforce
    /// finite termination/quiescence, electrical invariants and mandatory safety
    /// even if ordinary protocol work fails. This cooperative boundary does not
    /// confine hostile native code sharing its address space.
    pub unsafe fn bind_selected(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        attachment: I2cAttachment,
        provider: P,
        selection: I2cCallSelection<'_>,
    ) -> Result<Self, I2cCallRefusal> {
        let mut entries = table.inspections();
        let entry = entries.next().ok_or(I2cCallRefusal::Possession)?;
        if entries.next().is_some()
            || entry.scope.maximum_in_flight != 1
            || entry.scope.maximum_parameter_bytes < I2C_MAXIMUM_BYTES
            || entry.scope.maximum_result_bytes < I2C_MAXIMUM_BYTES
            || claim.parameter_bytes != I2C_MAXIMUM_BYTES
            || claim.work_units != 1
            || attachment.generation == 0
            || attachment.minimum_address < 8
            || attachment.maximum_address > 119
            || attachment.minimum_address > attachment.maximum_address
            || attachment.resource_bytes < MAXIMUM_TRANSACTION_BYTES as u64
        {
            return Err(I2cCallRefusal::Possession);
        }
        drop(entries);
        let node = bind_selected_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: selection.contract.kind(),
                call: I2C_CALL,
                input_bytes: I2C_MAXIMUM_BYTES,
                output_bytes: I2C_MAXIMUM_BYTES,
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
            SelectedCallRefusal::WrongBinding => I2cCallRefusal::WrongBinding,
            SelectedCallRefusal::Possession => I2cCallRefusal::Possession,
        })?;
        Ok(Self {
            table,
            handle,
            claim,
            attachment,
            provider,
            node,
            next_request: 0,
            decoder: PreparedI2cDecoder::new(selection.contract)
                .map_err(I2cCallRefusal::Canonical)?,
            encoder: PreparedI2cResultEncoder::new(selection.contract)
                .map_err(I2cCallRefusal::Canonical)?,
            input: [0; MAXIMUM_TRANSACTION_BYTES],
        })
    }

    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request_id: RequestId,
        input: &[u8],
    ) -> Result<&[u8], I2cCallRefusal> {
        self.check_binding(node, call)?;
        if request_id != RequestId(self.next_request) {
            return Err(I2cCallRefusal::StaleRequest);
        }
        let next = self
            .next_request
            .checked_add(1)
            .ok_or(I2cCallRefusal::SequenceExhausted)?;
        let decoded = self.decoder.decode(input).map_err(I2cCallRefusal::Decode)?;
        let request = decoded
            .transaction()
            .map_err(|reason| I2cCallRefusal::Decode(I2cDecodeRefusal::Geometry(reason)))?;
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(I2cCallRefusal::Capability)?;
        self.next_request = next; // an admitted dispatch cannot be replayed
        self.input.fill(0); // a short read can never expose bytes from a preceding transaction
        let observed = if request.address() < self.attachment.minimum_address
            || request.address() > self.attachment.maximum_address
        {
            Err(I2cDisposition::Refused)
        } else {
            self.provider.transact(
                &request,
                &mut self.input[..usize::from(request.read_length())],
            )
        };
        let encoded = match observed {
            Ok(actual) if actual <= usize::from(request.read_length()) => self
                .encoder
                .completed(request.read_length(), &self.input[..actual])
                .map_err(I2cCallRefusal::Result),
            Ok(_) => Err(I2cCallRefusal::Result(I2cResultRefusal::ActualLength)),
            Err(reason) => self
                .encoder
                .disposition(reason)
                .map_err(I2cCallRefusal::Canonical),
        };
        let result_bytes = encoded.as_ref().map_or(0, |bytes| bytes.len() as u32);
        // A physical refusal or malformed result still consumes the admitted
        // operation; no hidden retry and no leaked in-flight lease.
        self.table
            .complete(&mut self.handle, lease, result_bytes)
            .map_err(I2cCallRefusal::Capability)?;
        encoded
    }

    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), I2cCallRefusal> {
        self.check_binding(node, call)?;
        self.table
            .revoke(&self.handle)
            .map_err(I2cCallRefusal::Capability)?;
        self.provider.revoke();
        Ok(())
    }

    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), I2cCallRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(I2cCallRefusal::WrongBinding);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
