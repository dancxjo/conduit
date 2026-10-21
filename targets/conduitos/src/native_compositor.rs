//! Persistent finite native compositor service above the scanout mechanism.

mod contracts;
mod damage;
mod frame_composition;
mod input_routing;
mod interaction_affordance;
mod raster_reuse;
mod scanline;
mod scanout_acknowledgement;
mod surface_buffer;
mod surface_lifecycle;
mod surface_update;
pub use scanout_acknowledgement::ScanoutAcknowledgement;

use crate::display::DisplayError;
#[cfg(not(feature = "native-compositor"))]
use crate::display::render_scene;
use alloc::{string::String, vec::Vec};
use conduit_core::{HostBaseId, PlacementId, Plan, PlanId};
use conduit_face::{
    Face, GraphicsScene, LayoutRect, Manifestation, ManifestationError, ManifestationId,
    ManifestationLifecycle,
};
use damage::DamageState;
use surface_buffer::{SurfaceBuffer, SurfaceBufferPool, surface_pixels};

pub use contracts::{
    CompositionReceipt, CompositorAdmission, FrameReceipt, MAX_COMPOSITOR_PIXELS,
    MAX_COMPOSITOR_SURFACES, MAX_SURFACE_ID_BYTES, NATIVE_COMPOSITOR_FACILITY,
    NATIVE_PRESENTER_IMPLEMENTATION, NativeCompositorError, SurfacePlacementReceipt,
};
pub use damage::{DamageRect, MAX_DAMAGE_RECTS};
pub use input_routing::{InputRoute, RoutedKeyboard, RoutedPointer};

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
    raster_scene: Option<GraphicsScene>,
    visual_expression: Option<conduit_face::AdmittedVisualExpression>,
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
    /// a newer Face revision is manifested.
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
            raster_scene: None,
            visual_expression: None,
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
