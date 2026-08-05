#![no_std]

//! Port-aware, allocation-independent execution-kernel contract.
//!
//! This crate is the forward S1 kernel. It does not adapt the reboot runtime:
//! callers lower exact plans into numeric port, host-operation, and route
//! bindings before activation. The fixed and hosted storage profiles implement
//! the same value/evidence contracts.

#[cfg(feature = "alloc")]
extern crate alloc;

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

pub mod evidence;
pub mod storage;

pub use evidence::{EvidenceError, EvidenceQuery, EvidenceSink, FixedEvidenceLog, KernelEvent, KernelEventKind};
pub use storage::{FixedValueStore, StorageError, ValueStorage};

#[cfg(feature = "alloc")]
pub use evidence::HostedEvidenceLog;
#[cfg(feature = "alloc")]
pub use storage::HostedValueStore;

#[cfg(test)]
mod tests;
