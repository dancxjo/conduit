//! Transactional retained-surface placement and visibility lifecycle.

use conduit_presentation::LayoutRect;

use super::{
    MAX_COMPOSITOR_PIXELS, NativeCompositor, NativeCompositorError, SurfacePlacementReceipt,
    damage::RawDamageRect, surface_pixels,
};

impl NativeCompositor {
    pub fn place_surface(
        &mut self,
        surface_id: &str,
        bounds: LayoutRect,
        z: u8,
    ) -> Result<SurfacePlacementReceipt, NativeCompositorError> {
        let pixels = surface_pixels(bounds)?;
        let index = self
            .surfaces
            .iter()
            .position(|surface| surface.surface_id == surface_id)
            .ok_or(NativeCompositorError::SurfaceNotAdmitted)?;
        let old_bounds = self.surfaces[index].bounds;
        let old_z = self.surfaces[index].z;
        if old_bounds == bounds && old_z == z {
            return Ok(SurfacePlacementReceipt::Unchanged);
        }
        let visible_bound = self.surfaces[index].visible && self.surfaces[index].binding.is_some();
        let old_damage = visible_bound
            .then(|| RawDamageRect::from_layout(old_bounds))
            .transpose()?;
        let new_damage = visible_bound
            .then(|| RawDamageRect::from_layout(bounds))
            .transpose()?;
        let old_pixels = self.surfaces[index].buffer.pixels.len();
        let admitted_pixels = self
            .admitted_pixels
            .checked_sub(old_pixels)
            .and_then(|value| value.checked_add(pixels))
            .ok_or(NativeCompositorError::SurfaceCapacityExceeded)?;
        if admitted_pixels > MAX_COMPOSITOR_PIXELS {
            return Err(NativeCompositorError::SurfaceCapacityExceeded);
        }
        let dimensions_changed =
            bounds.width != old_bounds.width || bounds.height != old_bounds.height;
        let replacement = dimensions_changed
            .then(|| self.buffer_pool.take(bounds))
            .transpose()?;
        if visible_bound && old_bounds == bounds && old_z != z {
            self.damage_z_crossings(index, bounds, old_z, z)?;
        } else if visible_bound {
            self.damage.add(old_damage.expect("visible surface damage"));
            self.damage.add(new_damage.expect("visible surface damage"));
        }
        let prior_manifestation = dimensions_changed
            .then(|| {
                self.surfaces[index]
                    .binding
                    .as_ref()
                    .map(|binding| binding.manifestation_id.clone())
            })
            .flatten();
        if let Some(buffer) = replacement {
            let invalidated = core::mem::replace(&mut self.surfaces[index].buffer, buffer);
            self.buffer_pool.retain(invalidated);
            self.surfaces[index].receipt = None;
        }
        let surface = &mut self.surfaces[index];
        surface.bounds = bounds;
        surface.z = z;
        self.admitted_pixels = admitted_pixels;
        Ok(if dimensions_changed {
            SurfacePlacementReceipt::RasterInvalidated {
                previous: old_bounds,
                current: bounds,
                manifestation_id: prior_manifestation,
            }
        } else {
            SurfacePlacementReceipt::Moved {
                previous: old_bounds,
                current: bounds,
            }
        })
    }

    fn damage_z_crossings(
        &mut self,
        index: usize,
        bounds: LayoutRect,
        old_z: u8,
        z: u8,
    ) -> Result<(), NativeCompositorError> {
        let changed = RawDamageRect::from_layout(bounds)?;
        for (other_index, other) in self.surfaces.iter().enumerate() {
            if other_index == index || !other.visible || other.binding.is_none() {
                continue;
            }
            let crossed = (old_z < other.z && z >= other.z) || (old_z > other.z && z <= other.z);
            if crossed
                && let Some(overlap) =
                    changed.intersection(RawDamageRect::from_layout(other.bounds)?)
            {
                self.damage.add(overlap);
            }
        }
        Ok(())
    }

    pub fn set_surface_visible(
        &mut self,
        surface_id: &str,
        visible: bool,
    ) -> Result<(), NativeCompositorError> {
        let damage = {
            let surface = self.surface_mut(surface_id)?;
            let damage =
                (surface.visible != visible && surface.binding.is_some()).then_some(surface.bounds);
            surface.visible = visible;
            damage
        };
        if let Some(bounds) = damage {
            self.damage.add_layout(bounds)?;
        }
        Ok(())
    }

    pub fn focus_surface(&mut self, surface_id: &str) -> Result<(), NativeCompositorError> {
        let surface = self.surface_mut(surface_id)?;
        if !surface.visible || !surface.is_ready() {
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
        if removed.visible && removed.binding.is_some() {
            self.damage.add_layout(removed.bounds)?;
        }
        self.admitted_pixels -= removed.buffer.pixels.len();
        self.buffer_pool.retain(removed.buffer);
        if self.focused_surface.as_deref() == Some(surface_id) {
            self.focused_surface = None;
        }
        Ok(())
    }
}
