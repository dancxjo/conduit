//! Portable, finite contracts for permission-gated human media.
//!
//! Acquisition and semantic use are deliberately separate planning events.
//! Browser device identifiers and permission APIs are host-adapter truth and
//! therefore never appear in these contracts.

use crate::{
    AuthorityContractId, AuthorityGrantId, BootId, HostId, HostOperationContractId,
    HostOperationId, ImplementationId, KindId, OfferGeneration, PlanId, ResourceClassId,
    ResourceHandleId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HumanMediaKind {
    Camera,
    Microphone,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaFlowBounds {
    pub maximum_value_bytes: u32,
    pub maximum_queue_items: u16,
    pub maximum_queue_bytes: u32,
}

impl MediaFlowBounds {
    pub const fn is_finite_and_valid(self) -> bool {
        self.maximum_value_bytes > 0
            && self.maximum_queue_items > 0
            && self.maximum_queue_bytes >= self.maximum_value_bytes
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaConstraints {
    Camera {
        minimum_width: u16,
        maximum_width: u16,
        minimum_height: u16,
        maximum_height: u16,
        maximum_frames_per_second: u16,
    },
    Microphone {
        minimum_sample_rate_hz: u32,
        maximum_sample_rate_hz: u32,
        maximum_channels: u8,
    },
}

impl MediaConstraints {
    pub const fn kind(self) -> HumanMediaKind {
        match self {
            Self::Camera { .. } => HumanMediaKind::Camera,
            Self::Microphone { .. } => HumanMediaKind::Microphone,
        }
    }

    pub const fn is_valid(self) -> bool {
        match self {
            Self::Camera {
                minimum_width,
                maximum_width,
                minimum_height,
                maximum_height,
                maximum_frames_per_second,
            } => {
                minimum_width > 0
                    && minimum_width <= maximum_width
                    && minimum_height > 0
                    && minimum_height <= maximum_height
                    && maximum_frames_per_second > 0
            }
            Self::Microphone {
                minimum_sample_rate_hz,
                maximum_sample_rate_hz,
                maximum_channels,
            } => {
                minimum_sample_rate_hz > 0
                    && minimum_sample_rate_hz <= maximum_sample_rate_hz
                    && maximum_channels > 0
            }
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KnownPermissionState {
    Unknown,
    Prompt,
    Granted,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaImplementation {
    pub implementation_id: ImplementationId,
    pub kind: HumanMediaKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitializedMediaImplementation {
    pub implementation_id: ImplementationId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub kind: HumanMediaKind,
}

/// Initialized host capability which may be selected for acquisition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAcquisitionOffer {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub kind: HumanMediaKind,
    pub operation_contract: HostOperationContractId,
    pub request_authority_contract: AuthorityContractId,
    pub known_permission: KnownPermissionState,
    pub maximum_in_flight: u16,
    pub maximum_result_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAcquisitionAuthority {
    pub grant_id: AuthorityGrantId,
    pub contract_id: AuthorityContractId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub kind: HumanMediaKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAcquisitionRequest {
    pub operation_id: HostOperationId,
    pub constraints: MediaConstraints,
    pub flow_bounds: MediaFlowBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAcquisitionReservation {
    pub operation_id: HostOperationId,
    pub slot: u16,
    pub maximum_result_bytes: u32,
}

/// Exact immutable facts sealed before any permission interaction occurs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAcquisitionPlan {
    pub plan_id: PlanId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub operation_contract: HostOperationContractId,
    pub request_authority_grant: AuthorityGrantId,
    pub request: MediaAcquisitionRequest,
    pub reservation: MediaAcquisitionReservation,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaResourceAvailability {
    Available,
    Lost,
    Closed,
}

/// Strongest browser-visible identity: an opaque boot-scoped handle plus
/// exposed settings. It makes no stable physical-device identity claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquiredMediaResource {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub handle_id: ResourceHandleId,
    pub class_id: ResourceClassId,
    pub value_kind: KindId,
    pub settings: MediaConstraints,
    pub flow_bounds: MediaFlowBounds,
    pub use_authority_contract: AuthorityContractId,
    pub use_authority_grant: AuthorityGrantId,
    pub availability: MediaResourceAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaAcquisitionResult {
    Acquired(AcquiredMediaResource),
    Denied,
    Dismissed,
    Cancelled,
    NoMatchingDevice,
    UnsupportedConstraints,
    CapacityExhausted,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaUseRequirement {
    pub kind: HumanMediaKind,
    pub class_id: ResourceClassId,
    pub value_kind: KindId,
    pub flow_bounds: MediaFlowBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedMediaResource {
    pub handle_id: ResourceHandleId,
    pub use_authority_grant: AuthorityGrantId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub flow_bounds: MediaFlowBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveMediaInstance {
    pub plan_id: PlanId,
    pub handle_id: ResourceHandleId,
    pub use_authority_grant: AuthorityGrantId,
    pub operation_id: HostOperationId,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaPlanningRefusal {
    InvalidConstraints,
    InvalidBounds,
    OfferUnavailable,
    RequestAuthorityMissing,
    RequestAuthorityMismatch,
    CapacityExhausted,
    ResourceUnavailable,
    ResourceLost,
    ResourceClosed,
    UseAuthorityMissing,
    WrongResourceKind,
    BoundsUnsatisfied,
}
