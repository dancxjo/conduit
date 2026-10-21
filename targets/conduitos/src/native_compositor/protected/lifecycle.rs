use super::*;
use crate::native_compositor::SurfacePlacementReceipt;
impl NativeCompositor {
    pub fn admit_surface(
        &mut self,
        id: &str,
        bounds: LayoutRect,
        z: u8,
    ) -> Result<(), NativeCompositorError> {
        if self.failed {
            return Err(NativeCompositorError::ServiceLost);
        }
        self.validate_id(id)?;
        if self.surfaces.iter().any(|s| s.id == id) {
            return Err(NativeCompositorError::SurfaceAlreadyAdmitted);
        }
        let reserved = self
            .admitted_pixels
            .checked_add(pixels(bounds)?)
            .filter(|&n| n <= super::super::MAX_COMPOSITOR_PIXELS)
            .ok_or(NativeCompositorError::SurfaceCapacityExceeded)?;
        if self.surfaces.len() == 8 {
            return Err(NativeCompositorError::TooManySurfaces);
        }
        self.invalidate();
        if self.session.is_none() {
            let owner = self.owner()?.clone();
            let mut owner = owner
                .try_borrow_mut()
                .map_err(|_| NativeCompositorError::ServiceBusy)?;
            self.session = Some(owner.sessions.activate().map_err(owner_error)?.session);
            self.pointer_sample.set(None);
            self.hover_sample.set(false);
        }
        let handle = match self.request(Request::Admit {
            rect: rect(bounds),
            z,
        })? {
            Response::Surface(h) => h,
            _ => return Err(NativeCompositorError::ServiceProtocol),
        };
        self.surfaces.push(Surface {
            id: id.into(),
            handle,
            bounds,
            z,
            binding: None,
            receipt: None,
            recipe: None,
            expression: None,
            upload_sequence: 0,
        });
        self.admitted_pixels = reserved;
        Ok(())
    }
    pub fn place_surface(
        &mut self,
        id: &str,
        bounds: LayoutRect,
        z: u8,
    ) -> Result<SurfacePlacementReceipt, NativeCompositorError> {
        let i = self.index(id)?;
        let old = self.surfaces[i].bounds;
        let previous_z = self.surfaces[i].z;
        if old == bounds && previous_z == z {
            self.scope()?;
            return Ok(SurfacePlacementReceipt::Unchanged);
        }
        let replacement_pixels = pixels(bounds)?;
        let reserved = self
            .admitted_pixels
            .checked_sub(pixels(old)?)
            .and_then(|n| n.checked_add(replacement_pixels))
            .filter(|&n| n <= super::super::MAX_COMPOSITOR_PIXELS)
            .ok_or(NativeCompositorError::SurfaceCapacityExceeded)?;
        self.invalidate();
        let handle = self.surfaces[i].handle;
        let changed = old.width != bounds.width || old.height != bounds.height;
        self.unit(if changed {
            Request::Resize {
                surface: handle,
                rect: rect(bounds),
                z,
            }
        } else {
            Request::Place {
                surface: handle,
                x: bounds.x,
                y: bounds.y,
                z,
            }
        })?;
        self.surfaces[i].bounds = bounds;
        self.surfaces[i].z = z;
        self.admitted_pixels = reserved;
        if changed {
            self.surfaces[i].receipt = None;
            self.surfaces[i].recipe = None;
            Ok(SurfacePlacementReceipt::RasterInvalidated {
                previous: old,
                current: bounds,
                manifestation_id: self.surfaces[i]
                    .binding
                    .as_ref()
                    .map(|b| b.manifestation.clone()),
            })
        } else {
            Ok(SurfacePlacementReceipt::Moved {
                previous: old,
                current: bounds,
            })
        }
    }
    pub fn set_surface_visible(
        &mut self,
        id: &str,
        visible: bool,
    ) -> Result<(), NativeCompositorError> {
        let i = self.index(id)?;
        self.invalidate();
        self.unit(Request::Visible {
            surface: self.surfaces[i].handle,
            visible,
        })
    }
    pub fn focus_surface(&mut self, id: &str) -> Result<(), NativeCompositorError> {
        let i = self.index(id)?;
        self.invalidate();
        self.unit(Request::Focus(self.surfaces[i].handle))
    }
    pub fn remove_surface(&mut self, id: &str) -> Result<(), NativeCompositorError> {
        let i = self.index(id)?;
        self.invalidate();
        // Explicit withdrawal may forget an already-retired local namespace.
        // It never reactivates it or sends stale handles to the current owner.
        let session = self.session.ok_or(NativeCompositorError::StaleIdentity)?;
        let owner = self.owner()?.clone();
        let mut owner = owner
            .try_borrow_mut()
            .map_err(|_| NativeCompositorError::ServiceBusy)?;
        if owner.sessions.is_active(session) {
            response(
                owner
                    .sessions
                    .request(session, Request::Remove(self.surfaces[i].handle))
                    .map_err(owner_error)?,
            )?;
        }
        self.admitted_pixels -= pixels(self.surfaces[i].bounds)?;
        self.surfaces.remove(i);
        if self.surfaces.is_empty() {
            self.session = None;
        }
        Ok(())
    }
    pub fn set_cursor_hover(&mut self, hovered: bool) -> Result<(), NativeCompositorError> {
        // Cursor position is guest-owned. The backend records only the last
        // Root input sample for resending a hover request, never for rendering.
        let position = self.pointer_sample.get();
        self.invalidate();
        self.unit(Request::Cursor {
            position,
            hover: hovered,
        })?;
        self.hover_sample.set(hovered);
        Ok(())
    }
}
