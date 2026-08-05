use core::mem::size_of;

use super::{NodeId, PortId, RequestId};

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
