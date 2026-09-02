//! Finite detachable observation of authoritative kernel execution.
//!
//! Observations are a lossy debugger projection beside mandatory Signs. They
//! never participate in Form meaning, scheduling, admission, or execution
//! success. When the observer cannot retain another record, it overwrites the
//! oldest record and exposes the exact resulting gap.

use crate::{CordId, NodeId, PortId};

mod buffer;
mod sink;

pub use buffer::DebugObservationBuffer;
pub use sink::{DebugObserverControl, ObservedSignSink};

pub const DEBUG_OBSERVATION_SCHEMA_VERSION: u16 = 1;
pub const MAX_DEBUG_VALUE_PREVIEW_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebugExecutionIdentity {
    pub body: [u8; 32],
    pub plan: [u8; 32],
    pub play: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebugNodeBinding {
    pub form: u16,
    pub host: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugSubject {
    Gear(NodeId),
    Port { gear: NodeId, port: PortId },
    Cord(CordId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugEventKind {
    GearStarted,
    GearCompleted,
    ValueSent,
    ValueReceived,
    Fault,
    Unsupported(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebugRuntimeEvent<'a> {
    pub node: NodeId,
    pub port: Option<PortId>,
    pub cord: Option<CordId>,
    pub kind: DebugEventKind,
    pub type_identity: Option<u16>,
    pub value: Option<&'a [u8]>,
    pub fault_code: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebugObservationRecord {
    pub schema_version: u16,
    pub execution: DebugExecutionIdentity,
    /// Monotonic sequence assigned by this bounded collector.
    pub sequence: u64,
    /// Monotonic sequence at the originating Host before collection.
    pub host_sequence: u64,
    pub host: u16,
    pub form: u16,
    pub subject: DebugSubject,
    pub related_subject: Option<DebugSubject>,
    pub kind: DebugEventKind,
    pub type_identity: Option<u16>,
    pub value_bytes: u32,
    pub preview_len: u8,
    pub preview_truncated: bool,
    pub preview: [u8; MAX_DEBUG_VALUE_PREVIEW_BYTES],
    pub fault_code: Option<u16>,
}

impl DebugObservationRecord {
    pub fn preview(&self) -> &[u8] {
        &self.preview[..usize::from(self.preview_len)]
    }

    pub fn validate_for(
        &self,
        execution: DebugExecutionIdentity,
        maximum_preview_bytes: u8,
    ) -> Result<(), DebugObservationRefusal> {
        if self.schema_version != DEBUG_OBSERVATION_SCHEMA_VERSION {
            return Err(DebugObservationRefusal::UnsupportedSchemaVersion);
        }
        if matches!(self.kind, DebugEventKind::Unsupported(_)) {
            return Err(DebugObservationRefusal::UnsupportedEventKind);
        }
        if self.execution != execution {
            return Err(DebugObservationRefusal::StaleExecution);
        }
        if usize::from(self.preview_len) > usize::from(maximum_preview_bytes)
            || usize::from(self.preview_len) > MAX_DEBUG_VALUE_PREVIEW_BYTES
            || u32::from(self.preview_len) > self.value_bytes
        {
            return Err(DebugObservationRefusal::InvalidPreview);
        }
        if self.preview_truncated != (u32::from(self.preview_len) < self.value_bytes) {
            return Err(DebugObservationRefusal::InvalidPreview);
        }
        if matches!(
            self.kind,
            DebugEventKind::ValueSent | DebugEventKind::ValueReceived
        ) && self.type_identity.is_none()
            && self.value_bytes == 0
        {
            return Err(DebugObservationRefusal::InvalidValueObservation);
        }
        if self.kind == DebugEventKind::Fault && self.fault_code.is_none() {
            return Err(DebugObservationRefusal::InvalidFault);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebugObservationGap {
    pub dropped_records: u64,
    pub first_retained_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugObservationRefusal {
    InvalidBounds,
    ObserverAlreadyAttached,
    ObserverDetached,
    UnsupportedSchemaVersion,
    UnsupportedEventKind,
    StaleExecution,
    InvalidSequence,
    InvalidPreview,
    InvalidValueObservation,
    InvalidFault,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebugObservationInput<'a> {
    pub execution: DebugExecutionIdentity,
    pub host_sequence: u64,
    pub host: u16,
    pub form: u16,
    pub subject: DebugSubject,
    pub related_subject: Option<DebugSubject>,
    pub kind: DebugEventKind,
    pub type_identity: Option<u16>,
    pub value: Option<&'a [u8]>,
    pub fault_code: Option<u16>,
}
