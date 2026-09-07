//! Planned, finite disposition for an exact admitted failure scope.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureScope {
    Gear(NodeId),
    Cord(CordId),
    Form(u16),
    Play,
    HostOperation(HostOperationId),
    Resource(ResourceId),
    Line(RemoteEndpointId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlannedFaultDisposition {
    TerminateScope,
    Degrade { alternative: NodeId },
    WaitForChange { maximum_events: u16 },
    ReplaceRealization,
    LullBody,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FaultDisposition {
    pub scope: FailureScope,
    pub policy: PlannedFaultDisposition,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FaultDispositionRefusal {
    InvalidBound,
    MissingSemanticAlternative,
    StaleCompletion,
}

impl FaultDisposition {
    pub const fn admit(self) -> Result<Self, FaultDispositionRefusal> {
        match self.policy {
            PlannedFaultDisposition::WaitForChange { maximum_events: 0 } => {
                Err(FaultDispositionRefusal::InvalidBound)
            }
            PlannedFaultDisposition::Degrade {
                alternative: NodeId(id),
            } if id == u16::MAX => Err(FaultDispositionRefusal::MissingSemanticAlternative),
            _ => Ok(self),
        }
    }
    pub const fn accepts_completion(
        self,
        completion_scope: FailureScope,
    ) -> Result<(), FaultDispositionRefusal> {
        if same_scope(self.scope, completion_scope)
            && matches!(
                self.policy,
                PlannedFaultDisposition::TerminateScope
                    | PlannedFaultDisposition::ReplaceRealization
                    | PlannedFaultDisposition::LullBody
            )
        {
            Err(FaultDispositionRefusal::StaleCompletion)
        } else {
            Ok(())
        }
    }
}

const fn same_scope(a: FailureScope, b: FailureScope) -> bool {
    match (a, b) {
        (FailureScope::Gear(NodeId(a)), FailureScope::Gear(NodeId(b))) => a == b,
        (FailureScope::Cord(CordId(a)), FailureScope::Cord(CordId(b))) => a == b,
        (FailureScope::Form(a), FailureScope::Form(b)) => a == b,
        (FailureScope::Play, FailureScope::Play) => true,
        (
            FailureScope::HostOperation(HostOperationId(a)),
            FailureScope::HostOperation(HostOperationId(b)),
        ) => a == b,
        (FailureScope::Resource(ResourceId(a)), FailureScope::Resource(ResourceId(b))) => a == b,
        (FailureScope::Line(RemoteEndpointId(a)), FailureScope::Line(RemoteEndpointId(b))) => {
            a == b
        }
        _ => false,
    }
}

use crate::{CordId, HostOperationId, NodeId, RemoteEndpointId, ResourceId};
