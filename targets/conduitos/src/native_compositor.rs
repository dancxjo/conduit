//! Persistent finite native compositor service above the scanout mechanism.

mod frame_composition;
mod input_routing;
mod surface_buffer;

use crate::display::{DisplayError, DisplayReceipt, PixelTarget, render_scene};
use alloc::{string::String, vec::Vec};
use conduit_core::{
    ActivePlayId, ArtifactId, BootId, CapabilityId, HostBaseId, HostId, ImplementationId,
    OfferGeneration, PlacementId, Plan, PlanId,
};
use conduit_presentation::{
    GraphicsScene, LayoutRect, Manifestation, ManifestationError, ManifestationId,
    ManifestationLifecycle, Presentation, PresentationContentId,
};
use frame_composition::{blit_surface, clear_target};
use surface_buffer::{SurfaceBuffer, surface_pixels};

pub use input_routing::{InputRoute, RoutedKeyboard, RoutedPointer};

pub const NATIVE_COMPOSITOR_FACILITY: &str = "compositor/native@1";
pub const NATIVE_PRESENTER_IMPLEMENTATION: &str = "presenter/native-graphical@1";
pub const MAX_COMPOSITOR_SURFACES: usize = 8;
pub const MAX_SURFACE_ID_BYTES: usize = 160;
pub const MAX_COMPOSITOR_PIXELS: usize = 8 * 1024 * 1024;

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
    /// Work used to render this revision into its retained offscreen buffer.
    pub display: DisplayReceipt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameReceipt {
    pub frame_sequence: u64,
    pub surfaces_composed: u8,
    pub pixels_written: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCompositorError {
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

#[derive(Clone)]
struct SurfaceBinding {
    manifestation_id: ManifestationId,
    plan_id: PlanId,
    placement_id: PlacementId,
    face_subject: String,
    last_revision: u64,
}
pub(super) struct CompositorSurface {
    surface_id: String,
    bounds: LayoutRect,
    z: u8,
    visible: bool,
    buffer: SurfaceBuffer,
    binding: Option<SurfaceBinding>,
    receipt: Option<CompositionReceipt>,
}

pub struct NativeCompositor {
    admission: CompositorAdmission,
    surfaces: Vec<CompositorSurface>,
    focused_surface: Option<String>,
    frame_sequence: u64,
    admitted_pixels: usize,
}

impl NativeCompositor {
    pub fn admitted(admission: CompositorAdmission) -> Self {
        Self {
            admission,
            surfaces: Vec::new(),
            focused_surface: None,
            frame_sequence: 0,
            admitted_pixels: 0,
        }
    }
    pub fn admission(&self) -> &CompositorAdmission {
        &self.admission
    }
    pub const fn frame_sequence(&self) -> u64 {
        self.frame_sequence
    }
    pub fn focused_surface(&self) -> Option<&str> {
        self.focused_surface.as_deref()
    }
    pub fn receipts(&self) -> impl Iterator<Item = &CompositionReceipt> {
        self.surfaces
            .iter()
            .filter_map(|surface| surface.receipt.as_ref())
    }

    pub fn admit_surface(
        &mut self,
        surface_id: &str,
        bounds: LayoutRect,
        z: u8,
    ) -> Result<(), NativeCompositorError> {
        self.validate_surface_id(surface_id)?;
        if self
            .surfaces
            .iter()
            .any(|surface| surface.surface_id == surface_id)
        {
            return Err(NativeCompositorError::SurfaceAlreadyAdmitted);
        }
        let pixels = surface_pixels(bounds)?;
        let admitted_pixels = self
            .admitted_pixels
            .checked_add(pixels)
            .ok_or(NativeCompositorError::SurfaceCapacityExceeded)?;
        if self.surfaces.len() == MAX_COMPOSITOR_SURFACES || admitted_pixels > MAX_COMPOSITOR_PIXELS
        {
            return Err(NativeCompositorError::SurfaceCapacityExceeded);
        }
        self.surfaces.push(CompositorSurface {
            surface_id: surface_id.into(),
            bounds,
            z,
            visible: true,
            buffer: SurfaceBuffer::new(bounds.width, bounds.height)?,
            binding: None,
            receipt: None,
        });
        self.admitted_pixels = admitted_pixels;
        Ok(())
    }

    pub fn update_surface(
        &mut self,
        presentation: &Presentation,
        manifestation: &Manifestation,
        plan: &Plan,
        surface_id: &str,
        display_base_id: &HostBaseId,
        scene: &GraphicsScene,
    ) -> Result<&CompositionReceipt, NativeCompositorError> {
        self.validate_manifestation(
            presentation,
            manifestation,
            plan,
            surface_id,
            display_base_id,
        )?;
        let surface = self
            .surfaces
            .iter_mut()
            .find(|surface| surface.surface_id == surface_id)
            .ok_or(NativeCompositorError::SurfaceNotAdmitted)?;
        if let Some(binding) = &surface.binding {
            if binding.plan_id != manifestation.plan_id
                || binding.placement_id != manifestation.placement_id
                || binding.face_subject != manifestation.face_subject
            {
                return Err(NativeCompositorError::SurfaceAlreadyBound);
            }
            if presentation.revision <= binding.last_revision {
                return Err(NativeCompositorError::StaleSurfaceRevision);
            }
        }
        surface.buffer.clear();
        let display = render_scene(&mut surface.buffer, scene)?;
        surface.binding = Some(SurfaceBinding {
            manifestation_id: manifestation.manifestation_id.clone(),
            plan_id: manifestation.plan_id.clone(),
            placement_id: manifestation.placement_id.clone(),
            face_subject: manifestation.face_subject.clone(),
            last_revision: presentation.revision,
        });
        surface.receipt = Some(CompositionReceipt {
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
        surface
            .receipt
            .as_ref()
            .ok_or(NativeCompositorError::SurfaceNotAdmitted)
    }

    pub fn place_surface(
        &mut self,
        surface_id: &str,
        bounds: LayoutRect,
        z: u8,
    ) -> Result<(), NativeCompositorError> {
        let pixels = surface_pixels(bounds)?;
        let surface = self
            .surfaces
            .iter_mut()
            .find(|surface| surface.surface_id == surface_id)
            .ok_or(NativeCompositorError::SurfaceNotAdmitted)?;
        let admitted_pixels = self
            .admitted_pixels
            .checked_sub(surface.buffer.pixels.len())
            .and_then(|value| value.checked_add(pixels))
            .ok_or(NativeCompositorError::SurfaceCapacityExceeded)?;
        if admitted_pixels > MAX_COMPOSITOR_PIXELS {
            return Err(NativeCompositorError::SurfaceCapacityExceeded);
        }
        if bounds.width != surface.bounds.width || bounds.height != surface.bounds.height {
            surface.buffer = SurfaceBuffer::new(bounds.width, bounds.height)?;
            surface.receipt = None;
        }
        surface.bounds = bounds;
        surface.z = z;
        self.admitted_pixels = admitted_pixels;
        Ok(())
    }

    pub fn set_surface_visible(
        &mut self,
        surface_id: &str,
        visible: bool,
    ) -> Result<(), NativeCompositorError> {
        self.surface_mut(surface_id)?.visible = visible;
        Ok(())
    }
    pub fn focus_surface(&mut self, surface_id: &str) -> Result<(), NativeCompositorError> {
        let surface = self.surface_mut(surface_id)?;
        if !surface.visible || surface.binding.is_none() {
            return Err(NativeCompositorError::SurfaceNotAdmitted);
        }
        self.focused_surface = Some(surface_id.into());
        Ok(())
    }
    pub fn remove_surface(&mut self, surface_id: &str) -> Result<(), NativeCompositorError> {
        let index = self
            .surfaces
            .iter()
            .position(|surface| surface.surface_id == surface_id)
            .ok_or(NativeCompositorError::SurfaceNotAdmitted)?;
        let removed = self.surfaces.remove(index);
        self.admitted_pixels -= removed.buffer.pixels.len();
        if self.focused_surface.as_deref() == Some(surface_id) {
            self.focused_surface = None;
        }
        Ok(())
    }

    pub fn compose_frame(
        &mut self,
        target: &mut impl PixelTarget,
    ) -> Result<FrameReceipt, NativeCompositorError> {
        let format = target.format().validate()?;
        let mut pixels_written = clear_target(target, format)?;
        let mut order: Vec<usize> = self
            .surfaces
            .iter()
            .enumerate()
            .filter(|(_, surface)| surface.visible && surface.binding.is_some())
            .map(|(index, _)| index)
            .collect();
        order.sort_by_key(|index| (self.surfaces[*index].z, *index));
        for index in &order {
            pixels_written = pixels_written
                .checked_add(blit_surface(target, format, &self.surfaces[*index])?)
                .ok_or(NativeCompositorError::Display(DisplayError::InvalidExtent))?;
        }
        self.frame_sequence = self
            .frame_sequence
            .checked_add(1)
            .ok_or(NativeCompositorError::Display(DisplayError::InvalidExtent))?;
        Ok(FrameReceipt {
            frame_sequence: self.frame_sequence,
            surfaces_composed: u8::try_from(order.len())
                .map_err(|_| NativeCompositorError::SurfaceCapacityExceeded)?,
            pixels_written,
        })
    }

    fn validate_surface_id(&self, surface_id: &str) -> Result<(), NativeCompositorError> {
        if self.admission.surface_ids.iter().any(|id| id == surface_id) {
            Ok(())
        } else {
            Err(NativeCompositorError::UnadmittedSurface)
        }
    }
    fn validate_manifestation(
        &self,
        presentation: &Presentation,
        manifestation: &Manifestation,
        plan: &Plan,
        surface_id: &str,
        display_base_id: &HostBaseId,
    ) -> Result<(), NativeCompositorError> {
        self.validate_surface_id(surface_id)?;
        if !self
            .admission
            .placement_ids
            .contains(&manifestation.placement_id)
        {
            return Err(NativeCompositorError::UnadmittedPlacement);
        }
        if manifestation.host_id != self.admission.host_id
            || manifestation.boot_id != self.admission.boot_id
            || manifestation.offer_generation != self.admission.offer_generation
            || manifestation.presenter_implementation_id
                != self.admission.presenter_implementation_id
            || display_base_id != &self.admission.display_base_id
            || manifestation.target_subject != surface_id
            || manifestation.lifecycle != ManifestationLifecycle::Available
        {
            return Err(NativeCompositorError::StaleIdentity);
        }
        manifestation
            .validate_against(presentation, plan)
            .map_err(map_manifestation_error)?;
        Ok(())
    }
    fn surface_mut(
        &mut self,
        surface_id: &str,
    ) -> Result<&mut CompositorSurface, NativeCompositorError> {
        self.validate_surface_id(surface_id)?;
        self.surfaces
            .iter_mut()
            .find(|surface| surface.surface_id == surface_id)
            .ok_or(NativeCompositorError::SurfaceNotAdmitted)
    }
}

fn map_manifestation_error(error: ManifestationError) -> NativeCompositorError {
    match error {
        ManifestationError::StaleIdentity => NativeCompositorError::StaleIdentity,
        _ => NativeCompositorError::ManifestationInvalid,
    }
}
