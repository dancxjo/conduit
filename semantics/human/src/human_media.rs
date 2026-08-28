//! Portable, finite contracts for permission-gated human media.
//!
//! Acquisition and semantic use are deliberately separate planning events.
//! Browser device identifiers and permission APIs are host-adapter truth and
//! therefore never appear in these contracts.

use conduit_core::{
    AuthorityContractId, AuthorityGrantId, BootId, HostId, HostOperationContractId,
    HostOperationId, ImplementationId, KindId, OfferGeneration, PlanId, PortId, ResourceClassId,
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
    pub output_port: PortId,
    pub class_id: ResourceClassId,
    pub value_kind: KindId,
    pub flow_bounds: MediaFlowBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedMediaResource {
    pub output_port: PortId,
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

pub fn plan_media_acquisition(
    plan_id: PlanId,
    offer: &MediaAcquisitionOffer,
    authority: Option<&MediaAcquisitionAuthority>,
    request: MediaAcquisitionRequest,
    occupied_operation_slots: u16,
) -> Result<MediaAcquisitionPlan, MediaPlanningRefusal> {
    if !request.constraints.is_valid() {
        return Err(MediaPlanningRefusal::InvalidConstraints);
    }
    if !request.flow_bounds.is_finite_and_valid() {
        return Err(MediaPlanningRefusal::InvalidBounds);
    }
    if offer.maximum_in_flight == 0 || offer.maximum_result_bytes == 0 {
        return Err(MediaPlanningRefusal::OfferUnavailable);
    }
    let authority = authority.ok_or(MediaPlanningRefusal::RequestAuthorityMissing)?;
    if authority.contract_id != offer.request_authority_contract
        || authority.host_id != offer.host_id
        || authority.boot_id != offer.boot_id
        || authority.kind != offer.kind
        || request.constraints.kind() != offer.kind
    {
        return Err(MediaPlanningRefusal::RequestAuthorityMismatch);
    }
    if occupied_operation_slots >= offer.maximum_in_flight {
        return Err(MediaPlanningRefusal::CapacityExhausted);
    }
    Ok(MediaAcquisitionPlan {
        plan_id,
        host_id: offer.host_id.clone(),
        boot_id: offer.boot_id.clone(),
        offer_generation: offer.offer_generation,
        operation_contract: offer.operation_contract.clone(),
        request_authority_grant: authority.grant_id.clone(),
        reservation: MediaAcquisitionReservation {
            operation_id: request.operation_id.clone(),
            slot: occupied_operation_slots,
            maximum_result_bytes: offer.maximum_result_bytes,
        },
        request,
    })
}

pub fn select_acquired_media(
    requirement: &MediaUseRequirement,
    resource: &AcquiredMediaResource,
    use_authority_grant: Option<&AuthorityGrantId>,
) -> Result<SelectedMediaResource, MediaPlanningRefusal> {
    match resource.availability {
        MediaResourceAvailability::Lost => return Err(MediaPlanningRefusal::ResourceLost),
        MediaResourceAvailability::Closed => return Err(MediaPlanningRefusal::ResourceClosed),
        MediaResourceAvailability::Available => {}
    }
    let grant = use_authority_grant.ok_or(MediaPlanningRefusal::UseAuthorityMissing)?;
    if *grant != resource.use_authority_grant {
        return Err(MediaPlanningRefusal::UseAuthorityMissing);
    }
    if requirement.kind != resource.settings.kind()
        || requirement.output_port.as_str().is_empty()
        || requirement.class_id != resource.class_id
        || requirement.value_kind != resource.value_kind
    {
        return Err(MediaPlanningRefusal::WrongResourceKind);
    }
    if !requirement.flow_bounds.is_finite_and_valid()
        || requirement.flow_bounds.maximum_value_bytes > resource.flow_bounds.maximum_value_bytes
        || requirement.flow_bounds.maximum_queue_items > resource.flow_bounds.maximum_queue_items
        || requirement.flow_bounds.maximum_queue_bytes > resource.flow_bounds.maximum_queue_bytes
    {
        return Err(MediaPlanningRefusal::BoundsUnsatisfied);
    }
    Ok(SelectedMediaResource {
        output_port: requirement.output_port.clone(),
        handle_id: resource.handle_id.clone(),
        use_authority_grant: grant.clone(),
        host_id: resource.host_id.clone(),
        boot_id: resource.boot_id.clone(),
        flow_bounds: requirement.flow_bounds,
    })
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
