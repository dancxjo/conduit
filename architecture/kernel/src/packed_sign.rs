//! Internal lossless Sign backing. Logical admission still charges KernelEvent.
//! Byte arrays avoid target-dependent padding; no identifier value is a sentinel.
use crate::{KernelEvent, KernelEventKind, NodeId, PortId, RequestId};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PackedKernelEvent {
    sequence: [u8; 4],
    node: [u8; 2],
    port: [u8; 2],
    request: [u8; 4],
    present: u8,
    kind: KernelEventKind,
}
impl From<KernelEvent> for PackedKernelEvent {
    fn from(event: KernelEvent) -> Self {
        Self {
            sequence: event.sequence.to_le_bytes(),
            node: event.node.0.to_le_bytes(),
            port: event.port.map_or(0, |port| port.0).to_le_bytes(),
            request: event.request.map_or(0, |request| request.0).to_le_bytes(),
            present: u8::from(event.port.is_some()) | (u8::from(event.request.is_some()) << 1),
            kind: event.kind,
        }
    }
}
impl From<PackedKernelEvent> for KernelEvent {
    fn from(event: PackedKernelEvent) -> Self {
        Self {
            sequence: u32::from_le_bytes(event.sequence),
            node: NodeId(u16::from_le_bytes(event.node)),
            port: (event.present & 1 != 0).then(|| PortId(u16::from_le_bytes(event.port))),
            request: (event.present & 2 != 0).then(|| RequestId(u32::from_le_bytes(event.request))),
            kind: event.kind,
        }
    }
}
#[cfg(test)]
#[path = "packed_sign_tests.rs"]
mod tests;
