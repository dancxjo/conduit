use alloc::string::String;

use conduit_presentation::ManifestationId;

use super::{NativeCompositor, NativeCompositorError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedPointer {
    pub surface_id: String,
    pub manifestation_id: ManifestationId,
    pub local_x: u16,
    pub local_y: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedKeyboard {
    pub surface_id: String,
    pub manifestation_id: ManifestationId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputRoute<T> {
    Delivered(T),
    NoTarget,
}

impl NativeCompositor {
    pub fn route_pointer(
        &mut self,
        display_x: u32,
        display_y: u32,
        activate: bool,
    ) -> Result<InputRoute<RoutedPointer>, NativeCompositorError> {
        let candidate = self
            .surfaces
            .iter()
            .enumerate()
            .filter(|(_, surface)| surface.visible && surface.is_ready())
            .filter(|(_, surface)| contains(surface.bounds, display_x, display_y))
            .max_by_key(|(index, surface)| (surface.z, *index));
        let Some((_, surface)) = candidate else {
            return Ok(InputRoute::NoTarget);
        };
        let binding = surface
            .binding
            .as_ref()
            .ok_or(NativeCompositorError::SurfaceNotAdmitted)?;
        let local_x = local_coordinate(display_x, surface.bounds.x)?;
        let local_y = local_coordinate(display_y, surface.bounds.y)?;
        let routed = RoutedPointer {
            surface_id: surface.surface_id.clone(),
            manifestation_id: binding.manifestation_id.clone(),
            local_x,
            local_y,
        };
        if activate {
            self.focused_surface = Some(routed.surface_id.clone());
        }
        Ok(InputRoute::Delivered(routed))
    }

    pub fn route_keyboard(
        &self,
        expected_manifestation_id: &ManifestationId,
    ) -> Result<InputRoute<RoutedKeyboard>, NativeCompositorError> {
        let Some(focused) = self.focused_surface.as_deref() else {
            return Ok(InputRoute::NoTarget);
        };
        let Some(surface) = self
            .surfaces
            .iter()
            .find(|surface| surface.surface_id == focused)
        else {
            return Ok(InputRoute::NoTarget);
        };
        if !surface.visible || !surface.is_ready() {
            return Ok(InputRoute::NoTarget);
        }
        let Some(binding) = surface.binding.as_ref() else {
            return Ok(InputRoute::NoTarget);
        };
        if &binding.manifestation_id != expected_manifestation_id {
            return Err(NativeCompositorError::StaleSurfaceBinding);
        }
        Ok(InputRoute::Delivered(RoutedKeyboard {
            surface_id: surface.surface_id.clone(),
            manifestation_id: binding.manifestation_id.clone(),
        }))
    }

    pub fn validate_pointer_route(
        &self,
        route: &RoutedPointer,
    ) -> Result<(), NativeCompositorError> {
        let surface = self
            .surfaces
            .iter()
            .find(|surface| surface.surface_id == route.surface_id)
            .ok_or(NativeCompositorError::StaleSurfaceBinding)?;
        let binding = surface
            .binding
            .as_ref()
            .ok_or(NativeCompositorError::StaleSurfaceBinding)?;
        if !surface.visible
            || !surface.is_ready()
            || binding.manifestation_id != route.manifestation_id
        {
            return Err(NativeCompositorError::StaleSurfaceBinding);
        }
        Ok(())
    }
}

fn contains(bounds: conduit_presentation::LayoutRect, x: u32, y: u32) -> bool {
    let left = i64::from(bounds.x);
    let top = i64::from(bounds.y);
    let right = left + i64::from(bounds.width);
    let bottom = top + i64::from(bounds.height);
    let x = i64::from(x);
    let y = i64::from(y);
    x >= left && x < right && y >= top && y < bottom
}

fn local_coordinate(value: u32, origin: i16) -> Result<u16, NativeCompositorError> {
    let local = i64::from(value)
        .checked_sub(i64::from(origin))
        .ok_or(NativeCompositorError::InvalidBounds)?;
    u16::try_from(local).map_err(|_| NativeCompositorError::InvalidBounds)
}
