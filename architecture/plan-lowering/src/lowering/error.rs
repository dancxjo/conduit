//! Typed refusal at the Plan-to-kernel preparation boundary.
use conduit_core::{ConnectionId, PlacementId, PortDirection, PortId as PlanPortId, SharedPoolId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    InvalidFragment,
    UnsupportedExecutionRegion(conduit_core::ExecutionRegionId),
    UnsupportedState(conduit_core::StateId),
    StateStorageExceeded,
    EmptyFragment,
    CapacityOverflow,
    ProfileCapacityExceeded {
        placement_id: PlacementId,
        direction: PortDirection,
        required: usize,
        available: usize,
    },
    DuplicatePlacement(PlacementId),
    DuplicateConnection(ConnectionId),
    DuplicatePort {
        placement_id: PlacementId,
        port_id: PlanPortId,
    },
    UnknownConnectionEndpoint(ConnectionId),
    UnknownConnectionPort(ConnectionId),
    ConnectionContractMismatch(ConnectionId),
    InvalidConnectionBudget(ConnectionId),
    InvalidRemoteConnection(ConnectionId),
    MultipleConnectionsToInput {
        placement_id: PlacementId,
        port_id: PlanPortId,
    },
    PortDirectionMismatch {
        placement_id: PlacementId,
        port_id: PlanPortId,
    },
    InvalidTerminalTransduction(PlacementId),
    UnsupportedHostCallConcurrency(PlacementId),
    ResourceBindingInvalid(PlacementId),
    ResourceAuthorityTransferUnsupported(ConnectionId),
    SignBudgetInvalid,
    SignReferenceMissing,
    SharedPoolInvalid(SharedPoolId),
    SharedPoolConsumerMissing(SharedPoolId),
}
