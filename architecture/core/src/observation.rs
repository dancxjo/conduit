//! Shared execution observations, distinct from Source, Plan and Play identities.
use alloc::string::String;
use serde::{Deserialize, Serialize};
use crate::{ActivePlayId, BootId, ConnectionId, ConnectionTerminalDisposition,
    ExecutionRegionId, FaceId, FailureReason, HostId, PlacementId, PlanId,
    SignId, TerminalDisposition, ValuePayload};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub sign_id: SignId,
    pub active_play_id: Option<ActivePlayId>,
    pub face_id: Option<FaceId>,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub plan_id: Option<PlanId>,
    pub placement_id: Option<PlacementId>,
    pub connection_id: Option<ConnectionId>,
    pub kind: ObservationKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationKind {
    HostStarted,
    AdvertisementPublished,
    PlanFragmentReceived,
    PlacementPrepared,
    PlanPlayStarted,
    ExecutionRegionOverlap {
        waiting_region_id: ExecutionRegionId,
        progressing_region_id: ExecutionRegionId,
        physical_parallelism: bool,
    },
    ValueProduced {
        value: ValuePayload,
    },
    ValueAccepted {
        value: ValuePayload,
    },
    ValuePresented {
        /// Meaningful data name declared by the authored Face Gear.
        label: String,
        value: ValuePayload,
    },
    PlacementCompleted,
    PlanCompleted,
    PlacementTerminal {
        disposition: TerminalDisposition,
    },
    ConnectionTerminal {
        disposition: ConnectionTerminalDisposition,
    },
    PlanTerminal {
        disposition: TerminalDisposition,
    },
    Failure {
        reason: FailureReason,
        message: Option<String>,
    },
    Cancelled,
    Released,
    SignGap {
        dropped: u64,
    },
}

