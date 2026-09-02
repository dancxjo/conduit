//! Narrow bounded execution control owned by the production scheduler.

use crate::NodeId;

use super::{DebugExecutionIdentity, DebugSubject};

pub const DEBUG_CONTROL_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugBreakpointKind {
    BeforeGearStart,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebugBreakpoint {
    pub schema_version: u16,
    pub execution: DebugExecutionIdentity,
    pub subject: DebugSubject,
    pub kind: DebugBreakpointKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebugSuspension {
    pub execution: DebugExecutionIdentity,
    pub subject: DebugSubject,
    pub kind: DebugBreakpointKind,
    pub node: NodeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugControlRefusal {
    UnsupportedSchemaVersion,
    StaleExecution,
    UnsupportedBreakpoint,
    UnknownSubject,
    BreakpointAlreadyArmed,
    NotSuspended,
    StaleSuspension,
    DistributedSuspensionUnsupported,
}

/// Validation supplied by the exact execution-bound debugger adapter.
///
/// The scheduler remains the sole owner of suspension and resume. This trait
/// only proves that a requested exact subject belongs to its current Play.
pub trait DebugRuntimeControl {
    fn validate_breakpoint(
        &self,
        breakpoint: DebugBreakpoint,
    ) -> Result<NodeId, DebugControlRefusal>;
}
