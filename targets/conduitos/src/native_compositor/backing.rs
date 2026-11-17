//! Plan-independent fixed raster stores, consumed by one actual compositor.
use super::*;

/// Physical raster arrays and their finite vector stores only. Identity/Scene/
/// Face heaps and allocator/stack/preparation peak are outside this receipt.
pub struct PreparedCompositorBacking {
    surfaces: Vec<CompositorSurface>,
    pool: SurfaceBufferPool,
    pixels: usize,
    bytes: usize,
}
impl PreparedCompositorBacking {
    pub fn required_bytes(width: u16, height: u16) -> Result<usize, NativeCompositorError> {
        let pixels = surface_pixels(LayoutRect {
            x: 0,
            y: 0,
            width,
            height,
        })?;
        if pixels > MAX_COMPOSITOR_PIXELS {
            return Err(NativeCompositorError::SurfaceCapacityExceeded);
        }
        pixels
            .checked_mul(core::mem::size_of::<u32>())
            .and_then(|bytes| bytes.checked_add(core::mem::size_of::<CompositorSurface>()))
            .and_then(|bytes| {
                bytes.checked_add(MAX_COMPOSITOR_SURFACES * core::mem::size_of::<SurfaceBuffer>())
            })
            .ok_or(NativeCompositorError::SurfaceCapacityExceeded)
    }
    pub fn prepare(
        surface_id: &str,
        width: u16,
        height: u16,
        maximum_bytes: usize,
    ) -> Result<Self, NativeCompositorError> {
        let bytes = Self::required_bytes(width, height)?;
        if bytes > maximum_bytes {
            return Err(NativeCompositorError::SurfaceCapacityExceeded);
        }
        if surface_id.is_empty() || surface_id.len() > MAX_SURFACE_ID_BYTES {
            return Err(NativeCompositorError::InvalidSurface);
        }
        let bounds = LayoutRect {
            x: 0,
            y: 0,
            width,
            height,
        };
        let mut pool = SurfaceBufferPool::new();
        let buffer = pool.take(bounds)?;
        let pixels = surface_pixels(bounds)?;
        let surfaces = alloc::vec![CompositorSurface {
            surface_id: surface_id.into(),
            bounds,
            z: 0,
            visible: true,
            buffer,
            raster_scene: None,
            visual_expression: None,
            visual_look: None,
            binding: None,
            receipt: None,
            input_ready: false,
        }];
        let actual = surfaces.capacity() * core::mem::size_of::<CompositorSurface>()
            + surfaces[0].buffer.pixels.capacity() * core::mem::size_of::<u32>()
            + pool.metadata_backing_bytes();
        if actual > maximum_bytes {
            return Err(NativeCompositorError::SurfaceCapacityExceeded);
        }
        Ok(Self {
            surfaces,
            pool,
            pixels,
            bytes: actual,
        })
    }
    pub fn geometry(&self) -> (u16, u16) {
        (
            self.surfaces[0].bounds.width,
            self.surfaces[0].bounds.height,
        )
    }
    pub fn backing_bytes(&self) -> usize {
        self.bytes
    }
    pub fn bind(
        self,
        admission: CompositorAdmission,
    ) -> Result<NativeCompositor, NativeCompositorError> {
        if admission.surface_ids.len() != 1
            || admission.surface_ids[0] != self.surfaces[0].surface_id
            || admission.placement_ids.len() != 1
        {
            return Err(NativeCompositorError::UnadmittedSurface);
        }
        Ok(NativeCompositor {
            admission,
            surfaces: self.surfaces,
            focused_surface: None,
            cursor: None,
            cursor_hover: false,
            frame_sequence: 0,
            scanout_pixels: [0; MAX_COMPOSITOR_SURFACES],
            admitted_pixels: self.pixels,
            buffer_pool: self.pool,
            damage: DamageState::new(),
        })
    }
}

#[cfg(test)]
#[path = "backing/tests.rs"]
mod tests;
