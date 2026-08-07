#![no_std]

//! Port-aware, allocation-independent execution-kernel contract.
//!
//! This crate is the forward S1 kernel. It does not adapt the reboot runtime:
//! callers lower exact plans into numeric port, host-operation, and route
//! bindings before activation. The fixed and hosted storage profiles implement
//! the same value/evidence contracts.

#[cfg(feature = "alloc")]
extern crate alloc;

use core::mem::size_of;

pub mod scheduler;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct NodeId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct PortId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct CordId(pub u16);

/// Numeric identity for one plan-lowered carrier boundary. The kernel does not
/// interpret provider or transport configuration; the host binds this identity
/// to the exact observed link before activation.
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
pub struct EvidenceExpectationId(pub u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceBinding {
    pub resource: ResourceId,
    pub units: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceExpectationTarget {
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
        if admitted_bytes == 0 || value.byte_len > admitted_bytes {
            return Err(ProtocolError::HostOperationInputExceeded);
        }
        Ok(Self {
            value,
            admitted_bytes,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureCode {
    InvalidInput,
    InvalidPort,
    InvalidLifecycle,
    StorageExhausted,
    HostOperationDenied,
    HostOperationFailed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Failure {
    pub code: FailureCode,
    pub detail: u16,
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
    RequestHostOperation {
        request: RequestId,
        operation: HostOperationId,
        input: BoundedValueRef,
    },
    Complete,
    Fail(Failure),
}

/// Shared state-machine boundary for hosted and fixed-storage execution.
pub trait Operation {
    fn start(&mut self) -> OperationAction;
    fn resume(&mut self, input: OperationInput) -> OperationAction;
    fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }
    fn retains_resumed_value(&self) -> bool {
        false
    }
    fn take_released_value(&mut self) -> Option<ValueRef> {
        None
    }
    fn cancel(&mut self) {}
}

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
    maximum_operations_per_node: u16,
    bindings: [Option<HostOperationBinding>; SLOTS],
    sealed: bool,
}

impl<const SLOTS: usize> FixedHostOperationBindings<SLOTS> {
    pub const fn new(maximum_operations_per_node: u16) -> Self {
        Self {
            maximum_operations_per_node,
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
        if binding.maximum_input_bytes == 0 {
            return Err(ProtocolError::HostOperationTableInvalid);
        }
        let slot = self.slot(node, binding.operation)?;
        if self.bindings[slot].is_some() {
            return Err(ProtocolError::HostOperationTableInvalid);
        }
        self.bindings[slot] = Some(binding);
        Ok(())
    }

    pub fn seal(&mut self) -> Result<(), ProtocolError> {
        if self.maximum_operations_per_node == 0 {
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
        if !self.sealed || self.maximum_operations_per_node == 0 {
            return Err(ProtocolError::HostOperationTableInvalid);
        }
        let operations_per_node = usize::from(self.maximum_operations_per_node);
        for (slot, binding) in self.bindings.iter().enumerate() {
            if binding.is_some() && slot / operations_per_node >= active_nodes {
                return Err(ProtocolError::HostOperationTableInvalid);
            }
        }
        Ok(())
    }

    fn slot(&self, node: NodeId, operation: HostOperationId) -> Result<usize, ProtocolError> {
        if operation.0 >= self.maximum_operations_per_node {
            return Err(ProtocolError::HostOperationMissing);
        }
        usize::from(node.0)
            .checked_mul(usize::from(self.maximum_operations_per_node))
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
/// there is no graph scan, string lookup, provider choice, or allocation.
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
pub enum EvidenceError {
    InvalidBudget,
    ItemCapacityExceeded,
    ByteCapacityExceeded,
    SequenceOverflow,
}

pub trait EvidenceSink {
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
    ) -> Result<KernelEvent, EvidenceError>;
}

pub trait EvidenceQuery {
    fn contains_kind(&self, kind: KernelEventKind) -> bool;
}

pub struct FixedEvidenceLog<const EVENTS: usize> {
    entries: [Option<KernelEvent>; EVENTS],
    len: u16,
    byte_capacity: u32,
    used_bytes: u32,
    next_sequence: u32,
}

impl<const EVENTS: usize> FixedEvidenceLog<EVENTS> {
    pub fn new(byte_capacity: u32) -> Result<Self, EvidenceError> {
        let physical_bytes = EVENTS
            .checked_mul(size_of::<KernelEvent>())
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(EvidenceError::InvalidBudget)?;
        if EVENTS == 0
            || EVENTS > usize::from(u16::MAX)
            || byte_capacity == 0
            || byte_capacity > physical_bytes
        {
            return Err(EvidenceError::InvalidBudget);
        }
        Ok(Self {
            entries: [None; EVENTS],
            len: 0,
            byte_capacity,
            used_bytes: 0,
            next_sequence: 0,
        })
    }

    pub fn events(&self) -> impl Iterator<Item = KernelEvent> + '_ {
        self.entries.iter().copied().flatten()
    }
}

impl<const EVENTS: usize> EvidenceSink for FixedEvidenceLog<EVENTS> {
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
    ) -> Result<KernelEvent, EvidenceError> {
        let charge =
            u32::try_from(size_of::<KernelEvent>()).map_err(|_| EvidenceError::InvalidBudget)?;
        if usize::from(self.len) >= EVENTS {
            return Err(EvidenceError::ItemCapacityExceeded);
        }
        if self
            .used_bytes
            .checked_add(charge)
            .filter(|used| *used <= self.byte_capacity)
            .is_none()
        {
            return Err(EvidenceError::ByteCapacityExceeded);
        }
        let sequence = self.next_sequence;
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(EvidenceError::SequenceOverflow)?;
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
}

impl<const EVENTS: usize> EvidenceQuery for FixedEvidenceLog<EVENTS> {
    fn contains_kind(&self, kind: KernelEventKind) -> bool {
        self.events().any(|event| event.kind == kind)
    }
}

#[cfg(feature = "alloc")]
pub struct HostedEvidenceLog {
    entries: alloc::vec::Vec<Option<KernelEvent>>,
    len: u16,
    byte_capacity: u32,
    used_bytes: u32,
    next_sequence: u32,
}

#[cfg(feature = "alloc")]
impl HostedEvidenceLog {
    pub fn new(item_capacity: u16, byte_capacity: u32) -> Result<Self, EvidenceError> {
        let physical_bytes = usize::from(item_capacity)
            .checked_mul(size_of::<KernelEvent>())
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(EvidenceError::InvalidBudget)?;
        if item_capacity == 0 || byte_capacity == 0 || byte_capacity > physical_bytes {
            return Err(EvidenceError::InvalidBudget);
        }
        let mut entries = alloc::vec::Vec::with_capacity(usize::from(item_capacity));
        entries.resize(usize::from(item_capacity), None);
        Ok(Self {
            entries,
            len: 0,
            byte_capacity,
            used_bytes: 0,
            next_sequence: 0,
        })
    }

    pub fn events(&self) -> impl Iterator<Item = KernelEvent> + '_ {
        self.entries.iter().copied().flatten()
    }

    pub fn allocation_capacity(&self) -> usize {
        self.entries.capacity()
    }
}

#[cfg(feature = "alloc")]
impl EvidenceSink for HostedEvidenceLog {
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
    ) -> Result<KernelEvent, EvidenceError> {
        let charge =
            u32::try_from(size_of::<KernelEvent>()).map_err(|_| EvidenceError::InvalidBudget)?;
        if usize::from(self.len) >= self.entries.len() {
            return Err(EvidenceError::ItemCapacityExceeded);
        }
        if self
            .used_bytes
            .checked_add(charge)
            .filter(|used| *used <= self.byte_capacity)
            .is_none()
        {
            return Err(EvidenceError::ByteCapacityExceeded);
        }
        let sequence = self.next_sequence;
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(EvidenceError::SequenceOverflow)?;
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
}

#[cfg(feature = "alloc")]
impl EvidenceQuery for HostedEvidenceLog {
    fn contains_kind(&self, kind: KernelEventKind) -> bool {
        self.events().any(|event| event.kind == kind)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BoundedValueRef, CordId, EvidenceError, EvidenceSink, FixedEvidenceLog,
        FixedHostOperationBindings, FixedRoutes, FixedValueStore, HostOperationBinding,
        HostOperationDisposition, HostOperationId, HostOperationOutcome, KernelEvent,
        KernelEventKind, NodeId, Operation, OperationAction, OperationInput, PortId, RequestId,
        RouteRange, RouteTarget, StorageError, ValueStorage,
    };

    #[test]
    fn port_aware_actions_and_inputs_preserve_exact_identity() {
        struct Echo {
            input: PortId,
            output: PortId,
        }

        impl Operation for Echo {
            fn start(&mut self) -> OperationAction {
                OperationAction::Await
            }

            fn resume(&mut self, input: OperationInput) -> OperationAction {
                match input {
                    OperationInput::Value { port, value } if port == self.input => {
                        OperationAction::Emit {
                            port: self.output,
                            value,
                        }
                    }
                    OperationInput::Closed { port } if port == self.input => {
                        OperationAction::Complete
                    }
                    _ => OperationAction::Fail(super::Failure {
                        code: super::FailureCode::InvalidPort,
                        detail: 0,
                    }),
                }
            }
        }

        let mut operation = Echo {
            input: PortId(3),
            output: PortId(7),
        };
        let value = super::ValueRef {
            slot: 1,
            generation: 2,
            byte_len: 4,
        };
        assert_eq!(operation.start(), OperationAction::Await);
        assert_eq!(
            operation.resume(OperationInput::Value {
                port: PortId(3),
                value
            }),
            OperationAction::Emit {
                port: PortId(7),
                value
            }
        );
        assert_eq!(
            operation.resume(OperationInput::Closed { port: PortId(3) }),
            OperationAction::Complete
        );
    }

    #[test]
    fn prebound_routes_never_broadcast_between_output_ports() {
        let mut routes = FixedRoutes::<4, 3>::new(2);
        routes
            .install(
                NodeId(0),
                PortId(0),
                RouteRange { start: 0, len: 2 },
                &[
                    RouteTarget {
                        cord: CordId(0),
                        sink: crate::CordEndpoint::local(NodeId(1), PortId(0)),
                    },
                    RouteTarget {
                        cord: CordId(1),
                        sink: crate::CordEndpoint::local(NodeId(2), PortId(0)),
                    },
                ],
            )
            .unwrap();
        routes
            .install(
                NodeId(0),
                PortId(1),
                RouteRange { start: 2, len: 1 },
                &[RouteTarget {
                    cord: CordId(2),
                    sink: crate::CordEndpoint::local(NodeId(3), PortId(4)),
                }],
            )
            .unwrap();
        routes.seal().unwrap();

        let mut left = routes.route(NodeId(0), PortId(0)).unwrap();
        assert_eq!(left.next().unwrap().cord, CordId(0));
        assert_eq!(left.next().unwrap().cord, CordId(1));
        assert_eq!(left.next(), None);
        let mut right = routes.route(NodeId(0), PortId(1)).unwrap();
        let right = right.next().unwrap();
        assert_eq!(right.cord, CordId(2));
        assert_eq!(right.sink, crate::CordEndpoint::local(NodeId(3), PortId(4)));
    }

    #[test]
    fn fixed_value_store_enforces_items_bytes_generation_and_fanout_references() {
        let mut store = FixedValueStore::<2, 8>::new(10).unwrap();
        let first = store.store(b"abcd").unwrap();
        let second = store.store(b"123456").unwrap();
        assert_eq!(store.used_items(), 2);
        assert_eq!(store.used_bytes(), 10);
        assert_eq!(store.store(b"x"), Err(StorageError::ByteCapacityExceeded));
        assert_eq!(store.get(first).unwrap(), b"abcd");

        store.retain(first).unwrap();
        store.release(first).unwrap();
        assert_eq!(store.get(first).unwrap(), b"abcd");
        store.release(first).unwrap();
        assert_eq!(store.get(first), Err(StorageError::StaleReference));
        assert_eq!(store.used_items(), 1);
        assert_eq!(store.used_bytes(), 6);

        let replacement = store.store(b"xy").unwrap();
        assert_eq!(replacement.slot, first.slot);
        assert_ne!(replacement.generation, first.generation);
        assert_eq!(store.get(second).unwrap(), b"123456");
    }

    #[test]
    fn host_operation_completion_is_correlated_and_byte_admitted() {
        let value = super::ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 4,
        };
        let bounded = BoundedValueRef::new(value, 4).unwrap();
        let action = OperationAction::RequestHostOperation {
            request: RequestId(9),
            operation: HostOperationId(2),
            input: bounded,
        };
        assert!(matches!(
            action,
            OperationAction::RequestHostOperation {
                request: RequestId(9),
                operation: HostOperationId(2),
                ..
            }
        ));
        let input = OperationInput::HostOperationCompleted {
            request: RequestId(9),
            outcome: HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(bounded),
                failure: None,
            },
        };
        assert!(matches!(
            input,
            OperationInput::HostOperationCompleted {
                request: RequestId(9),
                ..
            }
        ));
        assert!(BoundedValueRef::new(value, 3).is_err());
    }

    #[test]
    fn only_plan_admitted_host_operations_cross_the_boundary() {
        let value = super::ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 4,
        };
        let mut bindings = FixedHostOperationBindings::<4>::new(2);
        bindings
            .install(
                NodeId(1),
                HostOperationBinding {
                    operation: HostOperationId(0),
                    maximum_input_bytes: 4,
                    maximum_output_bytes: 8,
                },
            )
            .unwrap();
        bindings.seal().unwrap();
        let action = OperationAction::RequestHostOperation {
            request: RequestId(7),
            operation: HostOperationId(0),
            input: BoundedValueRef::new(value, 4).unwrap(),
        };
        assert_eq!(
            bindings
                .admit(NodeId(1), action)
                .unwrap()
                .maximum_output_bytes,
            8
        );
        assert!(bindings.admit(NodeId(0), action).is_err());
    }

    #[test]
    fn admitted_sink_host_operation_may_have_no_output_payload() {
        let mut bindings = FixedHostOperationBindings::<1>::new(1);
        bindings
            .install(
                NodeId(0),
                HostOperationBinding {
                    operation: HostOperationId(0),
                    maximum_input_bytes: 8,
                    maximum_output_bytes: 0,
                },
            )
            .unwrap();
        bindings.seal().unwrap();

        let action = OperationAction::RequestHostOperation {
            request: RequestId(1),
            operation: HostOperationId(0),
            input: BoundedValueRef::new(
                super::ValueRef {
                    slot: 0,
                    generation: 1,
                    byte_len: 4,
                },
                4,
            )
            .unwrap(),
        };
        assert_eq!(
            bindings
                .admit(NodeId(0), action)
                .unwrap()
                .maximum_output_bytes,
            0
        );
    }

    #[test]
    fn fixed_evidence_has_independent_item_and_byte_budgets() {
        let charge = u32::try_from(core::mem::size_of::<KernelEvent>()).unwrap();
        let mut log = FixedEvidenceLog::<3>::new(charge * 2).unwrap();
        log.record(
            NodeId(0),
            Some(PortId(1)),
            None,
            KernelEventKind::ValueRouted,
        )
        .unwrap();
        log.record(
            NodeId(1),
            None,
            Some(RequestId(2)),
            KernelEventKind::HostOperationCompleted,
        )
        .unwrap();
        assert_eq!(
            log.record(NodeId(2), None, None, KernelEventKind::OperationCompleted),
            Err(EvidenceError::ByteCapacityExceeded)
        );
        let mut events = log.events();
        assert_eq!(events.next().unwrap().sequence, 0);
        assert_eq!(events.next().unwrap().sequence, 1);
        assert_eq!(events.next(), None);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn hosted_and_fixed_value_profiles_produce_the_same_storage_vector() {
        use super::{HostedEvidenceLog, HostedValueStore};

        fn vector(storage: &mut impl ValueStorage) -> (u16, u32, [u8; 3]) {
            let value = storage.store(b"abc").unwrap();
            let mut bytes = [0; 3];
            bytes.copy_from_slice(storage.get(value).unwrap());
            storage.retain(value).unwrap();
            storage.release(value).unwrap();
            (storage.used_items(), storage.used_bytes(), bytes)
        }

        let mut fixed = FixedValueStore::<4, 8>::new(16).unwrap();
        let mut hosted = HostedValueStore::new(4, 8, 16).unwrap();
        assert_eq!(vector(&mut fixed), vector(&mut hosted));

        fn evidence_vector(sink: &mut impl EvidenceSink) -> (u16, u32, KernelEvent) {
            let event = sink
                .record(
                    NodeId(1),
                    Some(PortId(2)),
                    Some(RequestId(3)),
                    KernelEventKind::HostOperationCompleted,
                )
                .unwrap();
            (sink.len(), sink.used_bytes(), event)
        }
        let charge = u32::try_from(core::mem::size_of::<KernelEvent>()).unwrap();
        let mut fixed_evidence = FixedEvidenceLog::<2>::new(charge * 2).unwrap();
        let mut hosted_evidence = HostedEvidenceLog::new(2, charge * 2).unwrap();
        assert_eq!(
            evidence_vector(&mut fixed_evidence),
            evidence_vector(&mut hosted_evidence)
        );
    }
}
