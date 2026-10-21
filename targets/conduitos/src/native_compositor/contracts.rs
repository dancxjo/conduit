//! Shared native compositor admission, receipts and refusal contracts.
use super::{DamageRect, MAX_DAMAGE_RECTS};
use crate::display::{DisplayError, DisplayReceipt};
use alloc::{string::String, vec::Vec};
use conduit_core::{
    ActivePlayId, ArtifactId, BootId, CapabilityId, HostBaseId, HostId, ImplementationId,
    OfferGeneration, PlacementId, PlanId,
};
use conduit_face::{FaceContentId, LayoutRect, ManifestationId};

pub const NATIVE_COMPOSITOR_FACILITY: &str = "compositor/native@1";
pub const NATIVE_PRESENTER_IMPLEMENTATION: &str = "presenter/native-graphical@1";
pub const MAX_COMPOSITOR_SURFACES: usize = 8;
pub const MAX_SURFACE_ID_BYTES: usize = 160;
pub const MAX_COMPOSITOR_PIXELS: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositorAdmission {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub presenter_implementation_id: ImplementationId,
    pub display_base_id: HostBaseId,
    pub placement_ids: Vec<PlacementId>,
    pub surface_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionReceipt {
    pub face_id: FaceContentId,
    pub manifestation_id: ManifestationId,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub play_sequence: u64,
    pub placement_id: PlacementId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub presenter_implementation_id: ImplementationId,
    pub presenter_capability_id: CapabilityId,
    pub presenter_artifact_id: ArtifactId,
    pub front_subject: String,
    pub display_base_id: HostBaseId,
    pub surface_id: String,
    /// Work used to render this revision into its retained offscreen buffer.
    pub display: DisplayReceipt,
    pub visual_expression: Option<conduit_face::AdmittedVisualExpression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfacePlacementReceipt {
    Unchanged,
    Moved {
        previous: LayoutRect,
        current: LayoutRect,
    },
    RasterInvalidated {
        previous: LayoutRect,
        current: LayoutRect,
        manifestation_id: Option<ManifestationId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameReceipt {
    pub frame_sequence: u64,
    pub surfaces_composed: u8,
    pub pixels_written: u32,
    pub damage_count: u8,
    pub damage_rects: [DamageRect; MAX_DAMAGE_RECTS],
    pub conservative_fallback: bool,
    pub cursor_visible: bool,
    pub focus_visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCompositorError {
    ServiceUnavailable,
    ServiceBusy,
    ServiceLost,
    ServiceProtocol,
    EmptyAdmission,
    TooManySurfaces,
    SurfaceCapacityExceeded,
    InvalidSurface,
    InvalidBounds,
    DuplicateSurface,
    DuplicatePlacement,
    UnadmittedSurface,
    UnadmittedPlacement,
    SurfaceAlreadyAdmitted,
    SurfaceNotAdmitted,
    SurfaceAlreadyBound,
    StaleSurfaceBinding,
    StaleSurfaceRevision,
    StaleIdentity,
    ManifestationInvalid,
    Display(DisplayError),
}

impl NativeCompositorError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ServiceUnavailable => "compositor-service-unavailable",
            Self::ServiceBusy => "compositor-service-busy",
            Self::ServiceLost => "compositor-service-lost",
            Self::ServiceProtocol => "compositor-service-protocol-invalid",
            Self::EmptyAdmission => "compositor-admission-empty",
            Self::TooManySurfaces | Self::SurfaceCapacityExceeded => {
                "compositor-surface-capacity-exceeded"
            }
            Self::InvalidSurface => "compositor-surface-invalid",
            Self::InvalidBounds => "compositor-surface-bounds-invalid",
            Self::DuplicateSurface => "compositor-surface-duplicate",
            Self::DuplicatePlacement => "compositor-placement-duplicate",
            Self::UnadmittedSurface => "compositor-surface-unadmitted",
            Self::UnadmittedPlacement => "compositor-placement-unadmitted",
            Self::SurfaceAlreadyAdmitted => "compositor-surface-already-admitted",
            Self::SurfaceNotAdmitted => "compositor-surface-not-admitted",
            Self::SurfaceAlreadyBound => "compositor-surface-already-bound",
            Self::StaleSurfaceBinding => "compositor-surface-binding-stale",
            Self::StaleSurfaceRevision => "compositor-surface-revision-stale",
            Self::StaleIdentity => "compositor-identity-stale",
            Self::ManifestationInvalid => "compositor-manifestation-invalid",
            Self::Display(error) => error.as_str(),
        }
    }
}
impl From<DisplayError> for NativeCompositorError {
    fn from(value: DisplayError) -> Self {
        Self::Display(value)
    }
}

impl CompositorAdmission {
    pub fn new(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        presenter_implementation_id: ImplementationId,
        display_base_id: HostBaseId,
        mut placement_ids: Vec<PlacementId>,
        mut surface_ids: Vec<String>,
    ) -> Result<Self, NativeCompositorError> {
        if placement_ids.is_empty() || surface_ids.is_empty() {
            return Err(NativeCompositorError::EmptyAdmission);
        }
        if placement_ids.len() > MAX_COMPOSITOR_SURFACES
            || surface_ids.len() > MAX_COMPOSITOR_SURFACES
        {
            return Err(NativeCompositorError::TooManySurfaces);
        }
        if surface_ids
            .iter()
            .any(|id| id.is_empty() || id.len() > MAX_SURFACE_ID_BYTES)
        {
            return Err(NativeCompositorError::InvalidSurface);
        }
        placement_ids.sort();
        surface_ids.sort();
        if placement_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(NativeCompositorError::DuplicatePlacement);
        }
        if surface_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(NativeCompositorError::DuplicateSurface);
        }
        Ok(Self {
            host_id,
            boot_id,
            offer_generation,
            presenter_implementation_id,
            display_base_id,
            placement_ids,
            surface_ids,
        })
    }
}
