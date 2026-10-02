//! Persistent finite native compositor service above the scanout mechanism.

mod damage;
mod frame_composition;
mod input_routing;
mod interaction_affordance;
mod scanout_acknowledgement;
mod surface_buffer;
mod surface_lifecycle;
mod surface_update;
pub use scanout_acknowledgement::ScanoutAcknowledgement;

#[cfg(not(feature = "native-compositor"))]
use crate::display::render_scene;
use crate::display::{DisplayError, DisplayReceipt};
use alloc::{string::String, vec::Vec};
use conduit_core::{
    ActivePlayId, ArtifactId, BootId, CapabilityId, HostBaseId, HostId, ImplementationId,
    OfferGeneration, PlacementId, Plan, PlanId,
};
use conduit_presentation::{
    GraphicsScene, LayoutRect, Manifestation, ManifestationError, ManifestationId,
    ManifestationLifecycle, Presentation, PresentationContentId,
};
use damage::DamageState;
use surface_buffer::{SurfaceBuffer, SurfaceBufferPool, surface_pixels};

pub use damage::{DamageRect, MAX_DAMAGE_RECTS};
pub use input_routing::{InputRoute, RoutedKeyboard, RoutedPointer};

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
    pub front_subject: String,
    pub display_base_id: HostBaseId,
    pub surface_id: String,
    /// Work used to render this revision into its retained offscreen buffer.
    pub display: DisplayReceipt,
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
    front_subject: String,
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
    input_ready: bool,
}

impl CompositorSurface {
    pub(super) const fn is_raster_ready(&self) -> bool {
        self.binding.is_some() && self.receipt.is_some()
    }
    /// A binding is routable and composable only while pixels for that exact
    /// Manifestation revision inhabit the current surface extent. Resizing
    /// retains the binding's revision fence but invalidates those pixels until
    /// a newer Presentation revision is manifested.
    pub(super) const fn is_ready(&self) -> bool {
        self.is_raster_ready() && self.input_ready
    }
}

pub struct NativeCompositor {
    admission: CompositorAdmission,
    surfaces: Vec<CompositorSurface>,
    focused_surface: Option<String>,
    cursor: Option<(u32, u32)>,
    cursor_hover: bool,
    frame_sequence: u64,
    scanout_pixels: [u32; MAX_COMPOSITOR_SURFACES],
    admitted_pixels: usize,
    buffer_pool: SurfaceBufferPool,
    damage: DamageState,
}

impl NativeCompositor {
    pub fn admitted(admission: CompositorAdmission) -> Self {
        Self {
            admission,
            surfaces: Vec::new(),
            focused_surface: None,
            cursor: None,
            cursor_hover: false,
            frame_sequence: 0,
            scanout_pixels: [0; MAX_COMPOSITOR_SURFACES],
            admitted_pixels: 0,
            buffer_pool: SurfaceBufferPool::new(),
            damage: DamageState::new(),
        }
    }
    pub fn admission(&self) -> &CompositorAdmission {
        &self.admission
    }
    pub const fn frame_sequence(&self) -> u64 {
        self.frame_sequence
    }
    pub const fn allocated_pixels(&self) -> usize {
        self.buffer_pool.allocated_pixels()
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
        let buffer = self.buffer_pool.take(bounds)?;
        self.scanout_pixels = [0; MAX_COMPOSITOR_SURFACES];
        self.surfaces.push(CompositorSurface {
            surface_id: surface_id.into(),
            bounds,
            z,
            visible: true,
            buffer,
            binding: None,
            receipt: None,
            input_ready: false,
        });
        self.admitted_pixels = admitted_pixels;
        Ok(())
    }

    fn validate_surface_id(&self, surface_id: &str) -> Result<(), NativeCompositorError> {
        if self.admission.surface_ids.iter().any(|id| id == surface_id) {
            Ok(())
        } else {
            Err(NativeCompositorError::UnadmittedSurface)
        }
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
