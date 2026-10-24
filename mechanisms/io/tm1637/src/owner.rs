//! Exact selected possession around the finite electrical writer. Formatting,
//! page timing and retry policy are not electrical driver responsibilities.
use crate::{
    codec::PreparedPackets, Failure, OpenDrain, PacketRefusal, WriteContract, MAXIMUM_BYTES,
    WRITE_CALL,
};
use conduit_core as c;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::selected_operation::{
    bind_selected_operation, SelectedOperationContract, SelectedOperationPlan,
};

#[derive(Clone, Copy, Debug)]
pub struct WriteAttachment {
    pub generation: u64,
    /// Independently admitted retained packet/provider storage, not pin identity.
    pub resource_bytes: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub enum WriteCallRefusal<E> {
    WrongBinding,
    Possession,
    StaleRequest,
    SequenceExhausted,
    Packets(PacketRefusal),
    Capability(c::BaseCapabilityRefusal),
    Electrical(Failure<E>),
}
#[derive(Debug, PartialEq, Eq)]
pub enum RetirementRefusal {
    Unavailable,
    InFlight,
    NotRevoked,
}

pub struct WriteHostCall<P> {
    table: c::BaseCapabilityTable,
    handle: c::BaseCapabilityHandle,
    claim: c::BaseOperationClaim,
    attachment: WriteAttachment,
    provider: P,
    node: NodeId,
    next_request: u32,
    packets: PreparedPackets,
}
impl<P: OpenDrain> WriteHostCall<P> {
    /// # Safety
    /// Root must independently own the exact initialized electrical pins and
    /// generation, authorize this Plan/Play, and admit provider storage and work.
    /// Each synchronous edge/half-period must terminate within its admitted
    /// bound. This cooperative possession check is not hostile-code isolation.
    pub unsafe fn bind_selected(
        table: c::BaseCapabilityTable,
        handle: c::BaseCapabilityHandle,
        claim: c::BaseOperationClaim,
        attachment: WriteAttachment,
        provider: P,
        contract: &WriteContract,
        selection: SelectedOperationPlan<'_>,
    ) -> Result<Self, WriteCallRefusal<P::Error>> {
        let prepared = Self::prepare_selected(&table, &claim, attachment, contract, selection)?;
        Ok(Self::from_prepared(
            table, handle, claim, attachment, provider, prepared,
        ))
    }
    pub(crate) fn prepare_selected(
        table: &c::BaseCapabilityTable,
        claim: &c::BaseOperationClaim,
        attachment: WriteAttachment,
        contract: &WriteContract,
        selection: SelectedOperationPlan<'_>,
    ) -> Result<PreparedWriteBinding, WriteCallRefusal<P::Error>> {
        use WriteCallRefusal as E;
        check_possession(table, claim, attachment)?;
        let node = bind_selected_operation(
            table,
            claim,
            SelectedOperationContract {
                kind: contract.kind(),
                call: WRITE_CALL,
                input_bytes: MAXIMUM_BYTES,
                output_bytes: 0,
                resource_bytes: attachment.resource_bytes,
            },
            selection,
        )
        .map_err(|reason| match reason {
            conduit_plan_lowering::selected_operation::SelectedCallRefusal::WrongBinding => {
                E::WrongBinding
            }
            conduit_plan_lowering::selected_operation::SelectedCallRefusal::Possession => {
                E::Possession
            }
        })?;
        let packets = PreparedPackets::prepare(contract)
            .map_err(|reason| E::Packets(PacketRefusal::Canonical(reason)))?;
        Ok(PreparedWriteBinding { node, packets })
    }
    pub(crate) fn from_prepared(
        table: c::BaseCapabilityTable,
        handle: c::BaseCapabilityHandle,
        claim: c::BaseOperationClaim,
        attachment: WriteAttachment,
        provider: P,
        prepared: PreparedWriteBinding,
    ) -> Self {
        Self {
            table,
            handle,
            claim,
            attachment,
            provider,
            node: prepared.node,
            next_request: 0,
            packets: prepared.packets,
        }
    }
    /// Decode and authorize before any pin operation. One request consumes one
    /// admitted operation and returns no value. No automatic retries occur.
    /// Electrical failure (including cancellation) releases both lines through
    /// the driver, completes the lease and revokes further use of this binding.
    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
        cancelled: impl FnMut() -> bool,
    ) -> Result<(), WriteCallRefusal<P::Error>> {
        use WriteCallRefusal as E;
        self.check_binding(node, call)?;
        if request != RequestId(self.next_request) {
            return Err(E::StaleRequest);
        }
        let next = self
            .next_request
            .checked_add(1)
            .ok_or(E::SequenceExhausted)?;
        let decoded = self.packets.decode(input).map_err(E::Packets)?;
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(E::Capability)?;
        self.next_request = next;
        // The guard fences a panic/unwind too. Normal electrical failures have
        // an explicit completion before revocation; uncertain work is not reused.
        let pending = Pending {
            table: &mut self.table,
            handle: &mut self.handle,
            lease: Some(lease),
        };
        let result = crate::transmit(&mut self.provider, &decoded.packets(), cancelled);
        pending.complete().map_err(E::Capability)?;
        if result.is_err() {
            self.table.revoke(&self.handle).map_err(E::Capability)?;
        }
        result.map_err(E::Electrical)
    }
    pub fn revoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
    ) -> Result<(), WriteCallRefusal<P::Error>> {
        self.check_binding(node, call)?;
        self.table
            .revoke(&self.handle)
            .map_err(WriteCallRefusal::Capability)
    }
    pub fn inspection(&self) -> &c::CapabilityInspection {
        self.table
            .inspections()
            .next()
            .expect("single prepared capability")
    }
    pub fn retire_retained(
        owner: &mut Option<Self>,
    ) -> Result<(P, WriteAttachment), RetirementRefusal> {
        let current = owner.as_ref().ok_or(RetirementRefusal::Unavailable)?;
        if current.inspection().in_flight_operations != 0 {
            return Err(RetirementRefusal::InFlight);
        }
        if current.inspection().lifecycle != c::CapabilityLifecycle::Revoked {
            return Err(RetirementRefusal::NotRevoked);
        }
        let current = owner.take().expect("preflighted owner");
        Ok((current.provider, current.attachment))
    }
    fn check_binding(
        &self,
        node: NodeId,
        call: HostCallId,
    ) -> Result<(), WriteCallRefusal<P::Error>> {
        if node != self.node || call != HostCallId(0) {
            return Err(WriteCallRefusal::WrongBinding);
        }
        Ok(())
    }
}
struct Pending<'a> {
    table: &'a mut c::BaseCapabilityTable,
    handle: &'a mut c::BaseCapabilityHandle,
    lease: Option<c::BaseOperationLease>,
}
impl Pending<'_> {
    fn complete(mut self) -> Result<(), c::BaseCapabilityRefusal> {
        self.table.complete(
            self.handle,
            self.lease.as_ref().expect("pending").clone(),
            0,
        )?;
        self.lease = None;
        Ok(())
    }
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if self.lease.is_some() {
            let _ = self.table.revoke(self.handle);
        }
    }
}

pub(crate) struct PreparedWriteBinding {
    pub(crate) node: NodeId,
    pub(crate) packets: PreparedPackets,
}
pub(crate) fn check_possession<E>(
    table: &c::BaseCapabilityTable,
    claim: &c::BaseOperationClaim,
    attachment: WriteAttachment,
) -> Result<(), WriteCallRefusal<E>> {
    let mut inspections = table.inspections();
    let scope = &inspections
        .next()
        .ok_or(WriteCallRefusal::Possession)?
        .scope;
    if inspections.next().is_some()
        || scope.maximum_in_flight != 1
        || scope.maximum_parameter_bytes < MAXIMUM_BYTES
        || scope.maximum_result_bytes != 0
        || claim.parameter_bytes != MAXIMUM_BYTES
        || claim.work_units != 1
        || !attachment.valid()
    {
        return Err(WriteCallRefusal::Possession);
    }
    Ok(())
}
impl WriteAttachment {
    pub(crate) fn valid(self) -> bool {
        self.generation != 0 && self.resource_bytes >= 24
    }
}
