//! Mandatory Sign publication and query interfaces.
use crate::{
    debug_observation, CordId, KernelEvent, KernelEventKind, NodeId, PortId,
    RemoteLifecycleIdentity, RequestId, SignError, SignRetentionGap,
};
use core::mem::size_of;

pub trait SignSink {
    fn item_capacity(&self) -> u16;
    fn byte_capacity(&self) -> u32;
    fn len(&self) -> u16;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn used_bytes(&self) -> u32;
    fn ensure_capacity(&self, additional: u16) -> Result<(), SignError> {
        if self
            .len()
            .checked_add(additional)
            .filter(|len| *len <= self.item_capacity())
            .is_none()
        {
            return Err(SignError::ItemCapacityExceeded);
        }
        let charge = u32::try_from(size_of::<KernelEvent>())
            .map_err(|_| SignError::InvalidBudget)?
            .checked_mul(u32::from(additional))
            .ok_or(SignError::InvalidBudget)?;
        if self
            .used_bytes()
            .checked_add(charge)
            .filter(|used| *used <= self.byte_capacity())
            .is_none()
        {
            return Err(SignError::ByteCapacityExceeded);
        }
        Ok(())
    }
    /// Pure pre-Play admission for each Cord's maximum payload extent. Sinks
    /// with bounded value custody must refuse incompatible plans here; wrappers
    /// forward this check to their mandatory inner sink.
    fn ensure_value_extent(&self, _maximum_bytes: u32) -> Result<(), SignError> {
        Ok(())
    }
    fn record(
        &mut self,
        node: NodeId,
        port: Option<PortId>,
        request: Option<RequestId>,
        kind: KernelEventKind,
    ) -> Result<KernelEvent, SignError>;

    /// Publish a payload routing Sign with bytes available at commit. Legacy
    /// sinks retain their ordinary Sign only; an explicitly admitted content
    /// profile binds these exact bytes beside that same event.
    fn record_value(
        &mut self,
        node: NodeId,
        port: PortId,
        _cord: CordId,
        _bytes: &[u8],
    ) -> Result<KernelEvent, SignError> {
        self.record(node, Some(port), None, KernelEventKind::ValueRouted)
    }

    fn record_remote(
        &mut self,
        node: NodeId,
        port: PortId,
        kind: KernelEventKind,
        remote: RemoteLifecycleIdentity,
    ) -> Result<KernelEvent, SignError>;

    fn ensure_remote_capacity(&self, additional: u16) -> Result<(), SignError>;

    /// Offers optional debugger telemetry beside mandatory execution Signs.
    /// Implementations must never turn debugger pressure or detachment into an
    /// execution error. The default detached observer deliberately does
    /// nothing.
    fn observe_debug(&mut self, _event: debug_observation::DebugRuntimeEvent<'_>) {}
}

pub trait SignQuery {
    fn contains_kind(&self, kind: KernelEventKind) -> bool;
    fn remote_identity(&self, event_sequence: u32) -> Option<RemoteLifecycleIdentity>;
    fn retention_gap(&self) -> Option<SignRetentionGap> {
        None
    }
}
