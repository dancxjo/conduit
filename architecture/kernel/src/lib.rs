#![no_std]

//! Port-aware, allocation-independent execution-kernel contract.
//!
//! This crate is the forward S1 kernel. It does not adapt the reboot runtime:
//! callers lower exact plans into numeric port, host-operation, and route
//! bindings before Play start. The fixed and hosted storage profiles implement
//! the same value/sign contracts.

#[cfg(feature = "alloc")]
extern crate alloc;

use core::mem::size_of;

pub mod debug_observation;
mod failure;
mod operation;
pub use failure::{Failure, FailureCode};
pub mod scheduler;
pub mod shared_flow;
pub mod shared_pool;
mod single_source;
pub mod state_delay;
pub mod static_merge;

pub use single_source::*;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct NodeId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct PortId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct CordId(pub u16);

/// Numeric identity for one plan-lowered line boundary. The kernel does not
/// interpret base or transport configuration; the host binds this identity
/// to the exact observed link before Play start.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct RemoteEndpointId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct RequestId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct HostOperationId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ResourceId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct SignExpectationId(pub u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceBinding {
    pub resource: ResourceId,
    pub units: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignExpectationTarget {
    Fragment,
    Node(NodeId),
    Cord(CordId),
}

/// Opaque reference into a plan-accounted value store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueRef {
    pub slot: u16,
    pub generation: u16,
    pub byte_len: u32,
}

/// A value reference carried across a plan-admitted host-operation boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundedValueRef {
    pub value: ValueRef,
    pub admitted_bytes: u32,
}

impl BoundedValueRef {
    pub const fn new(value: ValueRef, admitted_bytes: u32) -> Result<Self, ProtocolError> {
        if value.byte_len > admitted_bytes {
            return Err(ProtocolError::HostOperationInputExceeded);
        }
        Ok(Self {
            value,
            admitted_bytes,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostOperationDisposition {
    Completed,
    Denied,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostOperationOutcome {
    pub disposition: HostOperationDisposition,
    pub output: Option<BoundedValueRef>,
    pub failure: Option<Failure>,
}

/// Every value, closure, and host completion carries its exact correlation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationInput {
    Value {
        port: PortId,
        value: ValueRef,
    },
    Closed {
        port: PortId,
    },
    HostOperationCompleted {
        request: RequestId,
        outcome: HostOperationOutcome,
    },
}

/// Operations cannot emit without naming the exact semantic output port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationAction {
    Await,
    Emit {
        port: PortId,
        value: ValueRef,
    },
    EmitCanonical {
        port: PortId,
        value: CanonicalValue,
    },
    RequestHostOperation {
        request: RequestId,
        operation: HostOperationId,
        input: BoundedValueRef,
    },
    Complete,
    Fail(Failure),
}

pub use operation::Operation;

pub use scheduler::CanonicalValue;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    HostOperationInputExceeded,
    HostOperationMissing,
    HostOperationTableInvalid,
    HostOperationTableSealed,
    RouteTableInvalid,
    RouteTableSealed,
    RouteMissing,
    RouteTargetExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CordEndpoint {
    Local { node: NodeId, port: PortId },
    Remote(RemoteEndpointId),
}

impl CordEndpoint {
    pub const fn local(node: NodeId, port: PortId) -> Self {
        Self::Local { node, port }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostOperationBinding {
    pub operation: HostOperationId,
    pub maximum_input_bytes: u32,
    pub maximum_output_bytes: u32,
}

/// Plan-lowered, numeric host-operation admission table.
pub struct FixedHostOperationBindings<const SLOTS: usize> {
    maximum_gears_per_node: u16,
    bindings: [Option<HostOperationBinding>; SLOTS],
    sealed: bool,
}

impl<const SLOTS: usize> FixedHostOperationBindings<SLOTS> {
    pub const fn new(maximum_gears_per_node: u16) -> Self {
        Self {
            maximum_gears_per_node,
            bindings: [None; SLOTS],
            sealed: false,
        }
    }

    pub fn install(
        &mut self,
        node: NodeId,
        binding: HostOperationBinding,
    ) -> Result<(), ProtocolError> {
        if self.sealed {
            return Err(ProtocolError::HostOperationTableSealed);
        }
        let slot = self.slot(node, binding.operation)?;
        if self.bindings[slot].is_some() {
            return Err(ProtocolError::HostOperationTableInvalid);
        }
        self.bindings[slot] = Some(binding);
        Ok(())
    }

    pub fn seal(&mut self) -> Result<(), ProtocolError> {
        if self.maximum_gears_per_node == 0 {
            return Err(ProtocolError::HostOperationTableInvalid);
        }
        self.sealed = true;
        Ok(())
    }

    pub fn admit(
        &self,
        node: NodeId,
        action: OperationAction,
    ) -> Result<HostOperationBinding, ProtocolError> {
        if !self.sealed {
            return Err(ProtocolError::HostOperationTableInvalid);
        }
        let OperationAction::RequestHostOperation {
            operation, input, ..
        } = action
        else {
            return Err(ProtocolError::HostOperationMissing);
        };
        let binding = self.bindings[self.slot(node, operation)?]
            .ok_or(ProtocolError::HostOperationMissing)?;
        if input.value.byte_len > binding.maximum_input_bytes
            || input.admitted_bytes > binding.maximum_input_bytes
        {
            return Err(ProtocolError::HostOperationInputExceeded);
        }
        Ok(binding)
    }

    pub const fn is_sealed(&self) -> bool {
        self.sealed
    }

    pub(crate) fn validate_active_nodes(&self, active_nodes: usize) -> Result<(), ProtocolError> {
        if !self.sealed || self.maximum_gears_per_node == 0 {
            return Err(ProtocolError::HostOperationTableInvalid);
        }
        let operations_per_node = usize::from(self.maximum_gears_per_node);
        for (slot, binding) in self.bindings.iter().enumerate() {
            if binding.is_some() && slot / operations_per_node >= active_nodes {
                return Err(ProtocolError::HostOperationTableInvalid);
            }
        }
        Ok(())
    }

    fn slot(&self, node: NodeId, operation: HostOperationId) -> Result<usize, ProtocolError> {
        if operation.0 >= self.maximum_gears_per_node {
            return Err(ProtocolError::HostOperationMissing);
        }
        usize::from(node.0)
            .checked_mul(usize::from(self.maximum_gears_per_node))
            .and_then(|base| base.checked_add(usize::from(operation.0)))
            .filter(|slot| *slot < SLOTS)
            .ok_or(ProtocolError::HostOperationMissing)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteRange {
    pub start: u16,
    pub len: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteTarget {
    pub cord: CordId,
    pub sink: CordEndpoint,
}

/// Precomputed numeric routing table. Route lookup is direct after sealing:
/// there is no graph scan, string lookup, base choice, or allocation.
pub struct FixedRoutes<const ROUTE_SLOTS: usize, const TARGETS: usize> {
    maximum_ports_per_node: u16,
    ranges: [Option<RouteRange>; ROUTE_SLOTS],
    targets: [Option<RouteTarget>; TARGETS],
    sealed: bool,
}

impl<const ROUTE_SLOTS: usize, const TARGETS: usize> FixedRoutes<ROUTE_SLOTS, TARGETS> {
    pub const fn new(maximum_ports_per_node: u16) -> Self {
        Self {
            maximum_ports_per_node,
            ranges: [None; ROUTE_SLOTS],
            targets: [None; TARGETS],
            sealed: false,
        }
    }

    pub fn install(
        &mut self,
        source_node: NodeId,
        source_port: PortId,
        range: RouteRange,
        targets: &[RouteTarget],
    ) -> Result<(), ProtocolError> {
        if self.sealed {
            return Err(ProtocolError::RouteTableSealed);
        }
        if targets.len() != usize::from(range.len) {
            return Err(ProtocolError::RouteTableInvalid);
        }
        let slot = self.slot(source_node, source_port)?;
        let start = usize::from(range.start);
        let end = start
            .checked_add(usize::from(range.len))
            .ok_or(ProtocolError::RouteTargetExceeded)?;
        let destination = self
            .targets
            .get_mut(start..end)
            .ok_or(ProtocolError::RouteTargetExceeded)?;
        if self.ranges[slot].is_some() || destination.iter().any(Option::is_some) {
            return Err(ProtocolError::RouteTableInvalid);
        }
        for (destination, target) in destination.iter_mut().zip(targets.iter().copied()) {
            *destination = Some(target);
        }
        self.ranges[slot] = Some(range);
        Ok(())
    }

    pub fn seal(&mut self) -> Result<(), ProtocolError> {
        if self.maximum_ports_per_node == 0 {
            return Err(ProtocolError::RouteTableInvalid);
        }
        for range in self.ranges.iter().flatten() {
            let start = usize::from(range.start);
            let end = start
                .checked_add(usize::from(range.len))
                .ok_or(ProtocolError::RouteTargetExceeded)?;
            let targets = self
                .targets
                .get(start..end)
                .ok_or(ProtocolError::RouteTargetExceeded)?;
            if targets.iter().any(Option::is_none) {
                return Err(ProtocolError::RouteTableInvalid);
            }
        }
        self.sealed = true;
        Ok(())
    }

    pub fn route(
        &self,
        source_node: NodeId,
        source_port: PortId,
    ) -> Result<impl Iterator<Item = RouteTarget> + '_, ProtocolError> {
        if !self.sealed {
            return Err(ProtocolError::RouteTableInvalid);
        }
        let slot = self.slot(source_node, source_port)?;
        let range = self.ranges[slot].ok_or(ProtocolError::RouteMissing)?;
        let start = usize::from(range.start);
        let end = start + usize::from(range.len);
        Ok(self.targets[start..end].iter().copied().flatten())
    }

    pub const fn is_sealed(&self) -> bool {
        self.sealed
    }

    pub(crate) fn validate_active_prefix(
        &self,
        active_nodes: usize,
        active_cords: usize,
    ) -> Result<(), ProtocolError> {
        if !self.sealed || self.maximum_ports_per_node == 0 {
            return Err(ProtocolError::RouteTableInvalid);
        }
        let ports_per_node = usize::from(self.maximum_ports_per_node);
        for (slot, range) in self.ranges.iter().enumerate() {
            let Some(range) = range else {
                continue;
            };
            if slot / ports_per_node >= active_nodes {
                return Err(ProtocolError::RouteTableInvalid);
            }
            let start = usize::from(range.start);
            let end = start
                .checked_add(usize::from(range.len))
                .ok_or(ProtocolError::RouteTargetExceeded)?;
            for target in self
                .targets
                .get(start..end)
                .ok_or(ProtocolError::RouteTargetExceeded)?
                .iter()
                .flatten()
            {
                if usize::from(target.cord.0) >= active_cords
                    || match target.sink {
                        CordEndpoint::Local { node, .. } => usize::from(node.0) >= active_nodes,
                        CordEndpoint::Remote(_) => false,
                    }
                {
                    return Err(ProtocolError::RouteTableInvalid);
                }
            }
        }
        Ok(())
    }

    fn slot(&self, node: NodeId, port: PortId) -> Result<usize, ProtocolError> {
        if port.0 >= self.maximum_ports_per_node {
            return Err(ProtocolError::RouteMissing);
        }
        usize::from(node.0)
            .checked_mul(usize::from(self.maximum_ports_per_node))
            .and_then(|base| base.checked_add(usize::from(port.0)))
            .filter(|slot| *slot < ROUTE_SLOTS)
            .ok_or(ProtocolError::RouteMissing)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageError {
    InvalidBudget,
    ItemCapacityExceeded,
    ByteCapacityExceeded,
    ValueTooLarge,
    StaleReference,
    ReferenceOverflow,
}

pub trait ValueStorage {
    fn item_capacity(&self) -> u16;
    fn byte_capacity(&self) -> u32;
    fn used_items(&self) -> u16;
    fn used_bytes(&self) -> u32;
    fn store(&mut self, bytes: &[u8]) -> Result<ValueRef, StorageError>;
    fn get(&self, value: ValueRef) -> Result<&[u8], StorageError>;
    fn reference_count(&self, value: ValueRef) -> Result<u16, StorageError>;
    fn retain(&mut self, value: ValueRef) -> Result<(), StorageError>;
    fn release(&mut self, value: ValueRef) -> Result<(), StorageError>;
    fn clear(&mut self);
}

#[derive(Clone, Copy)]
struct FixedValueSlot<const MAX_VALUE_BYTES: usize> {
    generation: u16,
    references: u16,
    len: u32,
    bytes: [u8; MAX_VALUE_BYTES],
}

impl<const MAX_VALUE_BYTES: usize> FixedValueSlot<MAX_VALUE_BYTES> {
    const EMPTY: Self = Self {
        generation: 0,
        references: 0,
        len: 0,
        bytes: [0; MAX_VALUE_BYTES],
    };
}

/// Fixed-storage profile suitable for an embedded static allocation.
pub struct FixedValueStore<const SLOTS: usize, const MAX_VALUE_BYTES: usize> {
    slots: [FixedValueSlot<MAX_VALUE_BYTES>; SLOTS],
    byte_capacity: u32,
    used_items: u16,
    used_bytes: u32,
}

impl<const SLOTS: usize, const MAX_VALUE_BYTES: usize> FixedValueStore<SLOTS, MAX_VALUE_BYTES> {
    pub fn new(byte_capacity: u32) -> Result<Self, StorageError> {
        let physical_bytes = SLOTS
            .checked_mul(MAX_VALUE_BYTES)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(StorageError::InvalidBudget)?;
        if SLOTS == 0
            || SLOTS > usize::from(u16::MAX)
            || MAX_VALUE_BYTES == 0
            || byte_capacity == 0
            || byte_capacity > physical_bytes
        {
            return Err(StorageError::InvalidBudget);
        }
        Ok(Self {
            slots: [FixedValueSlot::EMPTY; SLOTS],
            byte_capacity,
            used_items: 0,
            used_bytes: 0,
        })
    }

    fn slot(&self, value: ValueRef) -> Result<&FixedValueSlot<MAX_VALUE_BYTES>, StorageError> {
        let slot = self
            .slots
            .get(usize::from(value.slot))
            .ok_or(StorageError::StaleReference)?;
        if slot.references == 0 || slot.generation != value.generation || slot.len != value.byte_len
        {
            return Err(StorageError::StaleReference);
        }
        Ok(slot)
    }

    fn slot_mut(
        &mut self,
        value: ValueRef,
    ) -> Result<&mut FixedValueSlot<MAX_VALUE_BYTES>, StorageError> {
        let slot = self
            .slots
            .get_mut(usize::from(value.slot))
            .ok_or(StorageError::StaleReference)?;
        if slot.references == 0 || slot.generation != value.generation || slot.len != value.byte_len
        {
            return Err(StorageError::StaleReference);
        }
        Ok(slot)
    }
}

impl<const SLOTS: usize, const MAX_VALUE_BYTES: usize> ValueStorage
    for FixedValueStore<SLOTS, MAX_VALUE_BYTES>
{
    fn item_capacity(&self) -> u16 {
        u16::try_from(SLOTS).unwrap_or(u16::MAX)
    }

    fn byte_capacity(&self) -> u32 {
        self.byte_capacity
    }

    fn used_items(&self) -> u16 {
        self.used_items
    }

    fn used_bytes(&self) -> u32 {
        self.used_bytes
    }

    fn store(&mut self, bytes: &[u8]) -> Result<ValueRef, StorageError> {
        if bytes.len() > MAX_VALUE_BYTES {
            return Err(StorageError::ValueTooLarge);
        }
        let byte_len = u32::try_from(bytes.len()).map_err(|_| StorageError::ValueTooLarge)?;
        if self
            .used_bytes
            .checked_add(byte_len)
            .filter(|used| *used <= self.byte_capacity)
            .is_none()
        {
            return Err(StorageError::ByteCapacityExceeded);
        }
        let (slot_index, slot) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.references == 0)
            .ok_or(StorageError::ItemCapacityExceeded)?;
        slot.generation = slot.generation.wrapping_add(1);
        if slot.generation == 0 {
            slot.generation = 1;
        }
        slot.references = 1;
        slot.len = byte_len;
        slot.bytes[..bytes.len()].copy_from_slice(bytes);
        self.used_items = self
            .used_items
            .checked_add(1)
            .ok_or(StorageError::ItemCapacityExceeded)?;
        self.used_bytes += byte_len;
        Ok(ValueRef {
            slot: u16::try_from(slot_index).map_err(|_| StorageError::ItemCapacityExceeded)?,
            generation: slot.generation,
            byte_len,
        })
    }

    fn get(&self, value: ValueRef) -> Result<&[u8], StorageError> {
        let slot = self.slot(value)?;
        Ok(&slot.bytes[..usize::try_from(slot.len).map_err(|_| StorageError::StaleReference)?])
    }

    fn reference_count(&self, value: ValueRef) -> Result<u16, StorageError> {
        Ok(self.slot(value)?.references)
    }

    fn retain(&mut self, value: ValueRef) -> Result<(), StorageError> {
        let slot = self.slot_mut(value)?;
        slot.references = slot
            .references
            .checked_add(1)
            .ok_or(StorageError::ReferenceOverflow)?;
        Ok(())
    }

    fn release(&mut self, value: ValueRef) -> Result<(), StorageError> {
        let slot = self.slot_mut(value)?;
        slot.references -= 1;
        if slot.references == 0 {
            let len = slot.len;
            slot.len = 0;
            self.used_items -= 1;
            self.used_bytes -= len;
        }
        Ok(())
    }

    fn clear(&mut self) {
        for slot in &mut self.slots {
            slot.references = 0;
            slot.len = 0;
        }
        self.used_items = 0;
        self.used_bytes = 0;
    }
}

#[cfg(feature = "alloc")]
mod hosted;

#[cfg(feature = "alloc")]
pub use hosted::Store as HostedValueStore;

mod remote_sign;
use remote_sign::RemoteLifecycleSign;
pub use remote_sign::{remote_sign_storage_bytes, RemoteCordDirection, RemoteLifecycleIdentity};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelEventKind {
    Decision,
    ValueStored,
    ValueRouted,
    ValueConsumed,
    RemoteValueOffered,
    RemoteValueAccepted,
    RemoteValueDelivered,
    RemoteOutputClosed,
    RemoteInputAdmitted,
    RemoteInputClosed,
    InputClosed,
    HostOperationRequested,
    HostOperationCancellationRequested,
    HostOperationCompleted,
    OperationCompleted,
    OperationFailed,
    CancellationRequested,
    RunCancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelEvent {
    pub sequence: u32,
    pub node: NodeId,
    pub port: Option<PortId>,
    pub request: Option<RequestId>,
    pub kind: KernelEventKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignError {
    InvalidBudget,
    ItemCapacityExceeded,
    ByteCapacityExceeded,
    RemoteItemCapacityExceeded,
    RemoteByteCapacityExceeded,
    SequenceOverflow,
}

pub trait SignSink {
    fn item_capacity(&self) -> u16;
    fn byte_capacity(&self) -> u32;
    fn len(&self) -> u16;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn used_bytes(&self) -> u32;
    fn record(
        &mut self,
        node: NodeId,
        port: Option<PortId>,
        request: Option<RequestId>,
        kind: KernelEventKind,
    ) -> Result<KernelEvent, SignError>;

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
}

pub struct FixedSignLog<const EVENTS: usize> {
    entries: [Option<KernelEvent>; EVENTS],
    len: u16,
    byte_capacity: u32,
    used_bytes: u32,
    next_sequence: u32,
    remote_entries: [Option<RemoteLifecycleSign>; EVENTS],
    remote_item_capacity: u16,
    remote_byte_capacity: u32,
    remote_len: u16,
    remote_used_bytes: u32,
}

impl<const EVENTS: usize> FixedSignLog<EVENTS> {
    pub fn new(byte_capacity: u32) -> Result<Self, SignError> {
        Self::new_with_remote_storage(byte_capacity, 0, 0)
    }

    pub fn new_with_remote_storage(
        byte_capacity: u32,
        remote_item_capacity: u16,
        remote_byte_capacity: u32,
    ) -> Result<Self, SignError> {
        let physical_bytes = EVENTS
            .checked_mul(size_of::<KernelEvent>())
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(SignError::InvalidBudget)?;
        let remote_physical_bytes =
            remote_sign_storage_bytes(remote_item_capacity).ok_or(SignError::InvalidBudget)?;
        if EVENTS == 0
            || EVENTS > usize::from(u16::MAX)
            || byte_capacity == 0
            || byte_capacity > physical_bytes
            || usize::from(remote_item_capacity) > EVENTS
            || (remote_item_capacity == 0 && remote_byte_capacity != 0)
            || (remote_item_capacity != 0
                && (remote_byte_capacity == 0 || remote_byte_capacity > remote_physical_bytes))
        {
            return Err(SignError::InvalidBudget);
        }
        Ok(Self {
            entries: [None; EVENTS],
            len: 0,
            byte_capacity,
            used_bytes: 0,
            next_sequence: 0,
            remote_entries: [None; EVENTS],
            remote_item_capacity,
            remote_byte_capacity,
            remote_len: 0,
            remote_used_bytes: 0,
        })
    }

    pub fn events(&self) -> impl Iterator<Item = KernelEvent> + '_ {
        self.entries.iter().copied().flatten()
    }

    pub fn remote_item_capacity(&self) -> u16 {
        self.remote_item_capacity
    }

    pub fn remote_byte_capacity(&self) -> u32 {
        self.remote_byte_capacity
    }

    pub fn remote_len(&self) -> u16 {
        self.remote_len
    }

    pub fn remote_used_bytes(&self) -> u32 {
        self.remote_used_bytes
    }
}

impl<const EVENTS: usize> SignSink for FixedSignLog<EVENTS> {
    fn item_capacity(&self) -> u16 {
        u16::try_from(EVENTS).unwrap_or(u16::MAX)
    }

    fn byte_capacity(&self) -> u32 {
        self.byte_capacity
    }

    fn len(&self) -> u16 {
        self.len
    }

    fn used_bytes(&self) -> u32 {
        self.used_bytes
    }

    fn record(
        &mut self,
        node: NodeId,
        port: Option<PortId>,
        request: Option<RequestId>,
        kind: KernelEventKind,
    ) -> Result<KernelEvent, SignError> {
        let charge =
            u32::try_from(size_of::<KernelEvent>()).map_err(|_| SignError::InvalidBudget)?;
        if usize::from(self.len) >= EVENTS {
            return Err(SignError::ItemCapacityExceeded);
        }
        if self
            .used_bytes
            .checked_add(charge)
            .filter(|used| *used <= self.byte_capacity)
            .is_none()
        {
            return Err(SignError::ByteCapacityExceeded);
        }
        let sequence = self.next_sequence;
        let next_sequence = sequence.checked_add(1).ok_or(SignError::SequenceOverflow)?;
        let event = KernelEvent {
            sequence,
            node,
            port,
            request,
            kind,
        };
        self.entries[usize::from(self.len)] = Some(event);
        self.len += 1;
        self.used_bytes += charge;
        self.next_sequence = next_sequence;
        Ok(event)
    }

    fn record_remote(
        &mut self,
        node: NodeId,
        port: PortId,
        kind: KernelEventKind,
        remote: RemoteLifecycleIdentity,
    ) -> Result<KernelEvent, SignError> {
        let charge =
            u32::try_from(size_of::<KernelEvent>()).map_err(|_| SignError::InvalidBudget)?;
        let remote_charge = u32::try_from(size_of::<RemoteLifecycleSign>())
            .map_err(|_| SignError::InvalidBudget)?;
        if usize::from(self.len) >= EVENTS {
            return Err(SignError::ItemCapacityExceeded);
        }
        if self
            .used_bytes
            .checked_add(charge)
            .filter(|used| *used <= self.byte_capacity)
            .is_none()
        {
            return Err(SignError::ByteCapacityExceeded);
        }
        self.ensure_remote_capacity(1)?;
        let sequence = self.next_sequence;
        let next_sequence = sequence.checked_add(1).ok_or(SignError::SequenceOverflow)?;
        let event = KernelEvent {
            sequence,
            node,
            port: Some(port),
            request: None,
            kind,
        };
        let index = usize::from(self.len);
        self.entries[index] = Some(event);
        self.remote_entries[usize::from(self.remote_len)] = Some(RemoteLifecycleSign {
            event_sequence: sequence,
            identity: remote,
        });
        self.len += 1;
        self.used_bytes += charge;
        self.remote_len += 1;
        self.remote_used_bytes += remote_charge;
        self.next_sequence = next_sequence;
        Ok(event)
    }

    fn ensure_remote_capacity(&self, additional: u16) -> Result<(), SignError> {
        if self
            .remote_len
            .checked_add(additional)
            .filter(|len| *len <= self.remote_item_capacity)
            .is_none()
        {
            return Err(SignError::RemoteItemCapacityExceeded);
        }
        let charge = u32::try_from(size_of::<RemoteLifecycleSign>())
            .map_err(|_| SignError::InvalidBudget)?
            .checked_mul(u32::from(additional))
            .ok_or(SignError::InvalidBudget)?;
        if self
            .remote_used_bytes
            .checked_add(charge)
            .filter(|used| *used <= self.remote_byte_capacity)
            .is_none()
        {
            return Err(SignError::RemoteByteCapacityExceeded);
        }
        Ok(())
    }
}

impl<const EVENTS: usize> SignQuery for FixedSignLog<EVENTS> {
    fn contains_kind(&self, kind: KernelEventKind) -> bool {
        self.events().any(|event| event.kind == kind)
    }

    fn remote_identity(&self, event_sequence: u32) -> Option<RemoteLifecycleIdentity> {
        self.remote_entries
            .iter()
            .copied()
            .flatten()
            .find(|entry| entry.event_sequence == event_sequence)
            .map(|entry| entry.identity)
    }
}

#[cfg(feature = "alloc")]
pub struct HostedSignLog {
    entries: alloc::vec::Vec<Option<KernelEvent>>,
    len: u16,
    byte_capacity: u32,
    used_bytes: u32,
    next_sequence: u32,
    remote_entries: alloc::vec::Vec<Option<RemoteLifecycleSign>>,
    remote_item_capacity: u16,
    remote_byte_capacity: u32,
    remote_len: u16,
    remote_used_bytes: u32,
}

#[cfg(feature = "alloc")]
impl HostedSignLog {
    pub fn new(item_capacity: u16, byte_capacity: u32) -> Result<Self, SignError> {
        Self::new_with_remote_storage(item_capacity, byte_capacity, 0, 0)
    }

    pub fn new_with_remote_storage(
        item_capacity: u16,
        byte_capacity: u32,
        remote_item_capacity: u16,
        remote_byte_capacity: u32,
    ) -> Result<Self, SignError> {
        let physical_bytes = usize::from(item_capacity)
            .checked_mul(size_of::<KernelEvent>())
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(SignError::InvalidBudget)?;
        let remote_physical_bytes =
            remote_sign_storage_bytes(remote_item_capacity).ok_or(SignError::InvalidBudget)?;
        if item_capacity == 0
            || byte_capacity == 0
            || byte_capacity > physical_bytes
            || remote_item_capacity > item_capacity
            || (remote_item_capacity == 0 && remote_byte_capacity != 0)
            || (remote_item_capacity != 0
                && (remote_byte_capacity == 0 || remote_byte_capacity > remote_physical_bytes))
        {
            return Err(SignError::InvalidBudget);
        }
        let mut entries = alloc::vec::Vec::with_capacity(usize::from(item_capacity));
        entries.resize(usize::from(item_capacity), None);
        let mut remote_entries = alloc::vec::Vec::with_capacity(usize::from(remote_item_capacity));
        remote_entries.resize(usize::from(remote_item_capacity), None);
        Ok(Self {
            entries,
            len: 0,
            byte_capacity,
            used_bytes: 0,
            next_sequence: 0,
            remote_entries,
            remote_item_capacity,
            remote_byte_capacity,
            remote_len: 0,
            remote_used_bytes: 0,
        })
    }

    pub fn events(&self) -> impl Iterator<Item = KernelEvent> + '_ {
        self.entries.iter().copied().flatten()
    }

    pub fn allocation_capacity(&self) -> usize {
        self.entries.capacity() + self.remote_entries.capacity()
    }
}

#[cfg(feature = "alloc")]
impl SignSink for HostedSignLog {
    fn item_capacity(&self) -> u16 {
        u16::try_from(self.entries.len()).unwrap_or(u16::MAX)
    }

    fn byte_capacity(&self) -> u32 {
        self.byte_capacity
    }

    fn len(&self) -> u16 {
        self.len
    }

    fn used_bytes(&self) -> u32 {
        self.used_bytes
    }

    fn record(
        &mut self,
        node: NodeId,
        port: Option<PortId>,
        request: Option<RequestId>,
        kind: KernelEventKind,
    ) -> Result<KernelEvent, SignError> {
        let charge =
            u32::try_from(size_of::<KernelEvent>()).map_err(|_| SignError::InvalidBudget)?;
        if usize::from(self.len) >= self.entries.len() {
            return Err(SignError::ItemCapacityExceeded);
        }
        if self
            .used_bytes
            .checked_add(charge)
            .filter(|used| *used <= self.byte_capacity)
            .is_none()
        {
            return Err(SignError::ByteCapacityExceeded);
        }
        let sequence = self.next_sequence;
        let next_sequence = sequence.checked_add(1).ok_or(SignError::SequenceOverflow)?;
        let event = KernelEvent {
            sequence,
            node,
            port,
            request,
            kind,
        };
        self.entries[usize::from(self.len)] = Some(event);
        self.len += 1;
        self.used_bytes += charge;
        self.next_sequence = next_sequence;
        Ok(event)
    }

    fn record_remote(
        &mut self,
        node: NodeId,
        port: PortId,
        kind: KernelEventKind,
        remote: RemoteLifecycleIdentity,
    ) -> Result<KernelEvent, SignError> {
        let charge =
            u32::try_from(size_of::<KernelEvent>()).map_err(|_| SignError::InvalidBudget)?;
        let remote_charge = u32::try_from(size_of::<RemoteLifecycleSign>())
            .map_err(|_| SignError::InvalidBudget)?;
        if usize::from(self.len) >= self.entries.len() {
            return Err(SignError::ItemCapacityExceeded);
        }
        if self
            .used_bytes
            .checked_add(charge)
            .filter(|used| *used <= self.byte_capacity)
            .is_none()
        {
            return Err(SignError::ByteCapacityExceeded);
        }
        self.ensure_remote_capacity(1)?;
        let sequence = self.next_sequence;
        let next_sequence = sequence.checked_add(1).ok_or(SignError::SequenceOverflow)?;
        let event = KernelEvent {
            sequence,
            node,
            port: Some(port),
            request: None,
            kind,
        };
        let index = usize::from(self.len);
        self.entries[index] = Some(event);
        self.remote_entries[usize::from(self.remote_len)] = Some(RemoteLifecycleSign {
            event_sequence: sequence,
            identity: remote,
        });
        self.len += 1;
        self.used_bytes += charge;
        self.remote_len += 1;
        self.remote_used_bytes += remote_charge;
        self.next_sequence = next_sequence;
        Ok(event)
    }

    fn ensure_remote_capacity(&self, additional: u16) -> Result<(), SignError> {
        if self
            .remote_len
            .checked_add(additional)
            .filter(|len| *len <= self.remote_item_capacity)
            .is_none()
        {
            return Err(SignError::RemoteItemCapacityExceeded);
        }
        let charge = u32::try_from(size_of::<RemoteLifecycleSign>())
            .map_err(|_| SignError::InvalidBudget)?
            .checked_mul(u32::from(additional))
            .ok_or(SignError::InvalidBudget)?;
        if self
            .remote_used_bytes
            .checked_add(charge)
            .filter(|used| *used <= self.remote_byte_capacity)
            .is_none()
        {
            return Err(SignError::RemoteByteCapacityExceeded);
        }
        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl SignQuery for HostedSignLog {
    fn contains_kind(&self, kind: KernelEventKind) -> bool {
        self.events().any(|event| event.kind == kind)
    }

    fn remote_identity(&self, event_sequence: u32) -> Option<RemoteLifecycleIdentity> {
        self.remote_entries
            .iter()
            .copied()
            .flatten()
            .find(|entry| entry.event_sequence == event_sequence)
            .map(|entry| entry.identity)
    }
}

#[cfg(test)]
mod tests;
