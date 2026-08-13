//! Optional finite native compositor facility above the scanout mechanism.

use alloc::{string::String, vec::Vec};
use conduit_core::{
    ActivePlayId, ArtifactId, BootId, CapabilityId, HostBaseId, HostId, ImplementationId,
    OfferGeneration, PlacementId, Plan, PlanId,
};
use conduit_presentation::{
    GraphicsScene, Manifestation, ManifestationError, ManifestationId, ManifestationLifecycle,
    Presentation, PresentationContentId,
};

use crate::display::{DisplayError, DisplayReceipt, PixelTarget, render_scene};

pub const NATIVE_COMPOSITOR_FACILITY: &str = "compositor/native@1";
pub const NATIVE_PRESENTER_IMPLEMENTATION: &str = "presenter/native-graphical@1";
pub const MAX_COMPOSITOR_SURFACES: usize = 8;
pub const MAX_SURFACE_ID_BYTES: usize = 160;

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
    pub presentation_id: PresentationContentId,
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
    pub face_subject: String,
    pub display_base_id: HostBaseId,
    pub surface_id: String,
    pub display: DisplayReceipt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCompositorError {
    EmptyAdmission,
    TooManySurfaces,
    InvalidSurface,
    DuplicateSurface,
    DuplicatePlacement,
    UnadmittedSurface,
    UnadmittedPlacement,
    SurfaceOccupied,
    StaleIdentity,
    ManifestationInvalid,
    Display(DisplayError),
}

impl NativeCompositorError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmptyAdmission => "compositor-admission-empty",
            Self::TooManySurfaces => "compositor-surface-capacity-exceeded",
            Self::InvalidSurface => "compositor-surface-invalid",
            Self::DuplicateSurface => "compositor-surface-duplicate",
            Self::DuplicatePlacement => "compositor-placement-duplicate",
            Self::UnadmittedSurface => "compositor-surface-unadmitted",
            Self::UnadmittedPlacement => "compositor-placement-unadmitted",
            Self::SurfaceOccupied => "compositor-surface-occupied",
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

pub struct NativeCompositor<T> {
    admission: CompositorAdmission,
    target: T,
    receipts: Vec<CompositionReceipt>,
}

impl<T: PixelTarget> NativeCompositor<T> {
    pub fn admitted(admission: CompositorAdmission, target: T) -> Self {
        Self {
            admission,
            target,
            receipts: Vec::new(),
        }
    }

    pub fn admission(&self) -> &CompositorAdmission {
        &self.admission
    }

    pub fn receipts(&self) -> &[CompositionReceipt] {
        &self.receipts
    }

    pub fn compose(
        &mut self,
        presentation: &Presentation,
        manifestation: &Manifestation,
        plan: &Plan,
        surface_id: &str,
        display_base_id: &HostBaseId,
        scene: &GraphicsScene,
    ) -> Result<&CompositionReceipt, NativeCompositorError> {
        if !self
            .admission
            .surface_ids
            .iter()
            .any(|admitted| admitted == surface_id)
        {
            return Err(NativeCompositorError::UnadmittedSurface);
        }
        if !self
            .admission
            .placement_ids
            .contains(&manifestation.placement_id)
        {
            return Err(NativeCompositorError::UnadmittedPlacement);
        }
        if self
            .receipts
            .iter()
            .any(|receipt| receipt.surface_id == surface_id)
        {
            return Err(NativeCompositorError::SurfaceOccupied);
        }
        if manifestation.host_id != self.admission.host_id
            || manifestation.boot_id != self.admission.boot_id
            || manifestation.offer_generation != self.admission.offer_generation
            || manifestation.presenter_implementation_id
                != self.admission.presenter_implementation_id
            || display_base_id != &self.admission.display_base_id
            || manifestation.lifecycle != ManifestationLifecycle::Available
        {
            return Err(NativeCompositorError::StaleIdentity);
        }
        manifestation
            .validate_against(presentation, plan)
            .map_err(map_manifestation_error)?;
        let display = render_scene(&mut self.target, scene)?;
        self.receipts.push(CompositionReceipt {
            presentation_id: presentation.identity.clone(),
            manifestation_id: manifestation.manifestation_id.clone(),
            plan_id: manifestation.plan_id.clone(),
            active_play_id: manifestation.active_play_id.clone(),
            play_sequence: manifestation.play_sequence,
            placement_id: manifestation.placement_id.clone(),
            host_id: manifestation.host_id.clone(),
            boot_id: manifestation.boot_id.clone(),
            offer_generation: manifestation.offer_generation,
            presenter_implementation_id: manifestation.presenter_implementation_id.clone(),
            presenter_capability_id: manifestation.presenter_capability_id.clone(),
            presenter_artifact_id: manifestation.presenter_artifact_id.clone(),
            face_subject: manifestation.face_subject.clone(),
            display_base_id: display_base_id.clone(),
            surface_id: surface_id.into(),
            display,
        });
        self.receipts
            .last()
            .ok_or(NativeCompositorError::TooManySurfaces)
    }
}

fn map_manifestation_error(error: ManifestationError) -> NativeCompositorError {
    match error {
        ManifestationError::StaleIdentity => NativeCompositorError::StaleIdentity,
        _ => NativeCompositorError::ManifestationInvalid,
    }
}
