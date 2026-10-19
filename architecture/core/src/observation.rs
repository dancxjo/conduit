//! Shared execution observations, distinct from Source, Plan and Play identities.
use crate::{
    ActivePlayId, BootId, ConnectionId, ConnectionTerminalDisposition, ExecutionRegionId, FaceId,
    FailureReason, HostId, PlacementId, PlanId, PortId, SignId, TerminalDisposition, ValuePayload,
};
use alloc::string::String;
use serde::{Deserialize, Serialize};

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
    /// Bytes committed to an exact planned Cord, retained after execution.
    /// The native event sequence is distinct from the enclosing Sign sequence.
    RoutedContent {
        evidence: RoutedContentEvidence,
    },
    /// Transient execution-kernel events evicted from this Play's native
    /// history. These are not missing Core Sign identities or snapshot slots.
    ExecutionEventGap {
        first_sequence: u32,
        last_sequence: u32,
        entries: u32,
    },
    /// Bytes committed toward a sealed output Fore, without inventing a Cord.
    /// The producing placement/port and actual native event remain explicit.
    ForeOutputContent {
        front_port_id: PortId,
        track: crate::ConnectionTrack,
        evidence: RoutedContentEvidence,
    },
}

/// Shared bounded routing evidence. It describes retained bytes, not a new
/// Gear-produced value or a mathematical projection certificate. Plan/Play,
/// producer and exact Cord/Fore context belong to the enclosing Observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutedContentEvidence {
    pub native_event_sequence: u32,
    pub output_port: PortId,
    pub byte_length: u32,
    pub digest_domain: String,
    pub digest_scheme: String,
    pub digest: [u8; 32],
}
