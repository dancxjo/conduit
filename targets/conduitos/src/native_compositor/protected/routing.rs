use super::*;
use crate::native_compositor::input_routing::ProtectedRouteScope;
use crate::native_compositor::{InputRoute, RoutedKeyboard, RoutedPointer};
impl NativeCompositor {
    pub fn focused_surface(&self) -> Option<&str> {
        let Response::Focused(Some(handle)) = self.request(Request::Focused).ok()? else {
            return None;
        };
        self.surfaces
            .iter()
            .find(|surface| surface.handle == handle)
            .map(|surface| surface.id.as_str())
    }
    pub fn route_keyboard(
        &self,
        expected: &ManifestationId,
    ) -> Result<InputRoute<RoutedKeyboard>, NativeCompositorError> {
        let response = self.request(Request::Keyboard)?;
        let Response::Keyboard(route) = response else {
            return Err(NativeCompositorError::ServiceProtocol);
        };
        let Some((handle, revision)) = route else {
            return Ok(InputRoute::NoTarget);
        };
        let surface = self.routed_surface(handle, revision)?;
        let binding = surface
            .binding
            .as_ref()
            .ok_or(NativeCompositorError::StaleSurfaceBinding)?;
        if &binding.manifestation != expected {
            return Err(NativeCompositorError::StaleSurfaceBinding);
        }
        Ok(InputRoute::Delivered(RoutedKeyboard {
            surface_id: surface.id.clone(),
            manifestation_id: binding.manifestation.clone(),
            scope: ProtectedRouteScope {
                session: self.scope()?,
                handle,
                upload_sequence: revision,
            },
        }))
    }
    pub fn route_pointer(
        &mut self,
        x: u32,
        y: u32,
        activate: bool,
    ) -> Result<InputRoute<RoutedPointer>, NativeCompositorError> {
        self.invalidate();
        self.unit(Request::Cursor {
            position: Some((x, y)),
            hover: self.hover_sample.get(),
        })?;
        self.pointer_sample.set(Some((x, y)));
        let Response::Pointer(route) = self.request(Request::Pointer { x, y })? else {
            return Err(NativeCompositorError::ServiceProtocol);
        };
        let Some(route) = route else {
            self.set_cursor_hover(false)?;
            return Ok(InputRoute::NoTarget);
        };
        let surface = self.routed_surface(route.surface, route.revision)?;
        if route.x >= surface.bounds.width || route.y >= surface.bounds.height {
            return Err(NativeCompositorError::ServiceProtocol);
        }
        let binding = surface
            .binding
            .as_ref()
            .ok_or(NativeCompositorError::StaleSurfaceBinding)?;
        let result = RoutedPointer {
            surface_id: surface.id.clone(),
            manifestation_id: binding.manifestation.clone(),
            local_x: route.x,
            local_y: route.y,
            scope: ProtectedRouteScope {
                session: self.scope()?,
                handle: route.surface,
                upload_sequence: route.revision,
            },
        };
        if activate {
            self.unit(Request::Focus(route.surface))?;
        }
        Ok(InputRoute::Delivered(result))
    }
    pub fn validate_pointer_route(
        &self,
        route: &RoutedPointer,
    ) -> Result<(), NativeCompositorError> {
        let session = self.scope()?;
        if route.scope.session != session {
            return Err(NativeCompositorError::StaleSurfaceBinding);
        }
        let surface = self.routed_surface(route.scope.handle, route.scope.upload_sequence)?;
        if surface.id != route.surface_id
            || surface
                .binding
                .as_ref()
                .is_none_or(|b| b.manifestation != route.manifestation_id)
            || route.local_x >= surface.bounds.width
            || route.local_y >= surface.bounds.height
        {
            return Err(NativeCompositorError::StaleSurfaceBinding);
        }
        let x = u32::try_from(i64::from(surface.bounds.x) + i64::from(route.local_x))
            .map_err(|_| NativeCompositorError::InvalidBounds)?;
        let y = u32::try_from(i64::from(surface.bounds.y) + i64::from(route.local_y))
            .map_err(|_| NativeCompositorError::InvalidBounds)?;
        match self.request(Request::Pointer { x, y })? {
            Response::Pointer(Some(current))
                if current.surface == surface.handle
                    && current.revision == surface.upload_sequence
                    && current.x == route.local_x
                    && current.y == route.local_y =>
            {
                Ok(())
            }
            Response::Pointer(_) => Err(NativeCompositorError::StaleSurfaceBinding),
            _ => Err(NativeCompositorError::ServiceProtocol),
        }
    }
    fn routed_surface(
        &self,
        handle: Handle,
        revision: u64,
    ) -> Result<&Surface, NativeCompositorError> {
        self.surfaces
            .iter()
            .find(|s| s.handle == handle && s.upload_sequence == revision && s.receipt.is_some())
            .ok_or(NativeCompositorError::StaleSurfaceBinding)
    }
}
