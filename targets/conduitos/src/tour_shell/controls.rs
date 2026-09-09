//! Native hit geometry for semantic shell controls.
use super::{INSPECTOR_SURFACE, Slot, TourShellError, TourShellPresenter};
use crate::{display::PixelTarget, native_compositor::RoutedPointer, tour_product::TourProduct};
use conduit_presentation::{LayoutRect, PresentationRole};
use conduit_tour_model::INSPECTOR_CLOSE_ACTION_ID;

pub(super) fn inspector_close_bounds(width: u16) -> LayoutRect {
    LayoutRect {
        x: width.saturating_sub(80) as i16,
        y: 8,
        width: 72,
        height: 24,
    }
}

impl TourShellPresenter {
    pub fn inspector_close_hit(&self, route: &RoutedPointer) -> Result<bool, TourShellError> {
        self.validate_pointer_route(route)?;
        if route.surface_id != INSPECTOR_SURFACE {
            return Ok(false);
        }
        let state = self
            .surfaces
            .iter()
            .find(|state| state.slot == Slot::Inspector)
            .ok_or(TourShellError::Identity)?;
        let presentation = state
            .presentation
            .as_ref()
            .ok_or(TourShellError::Identity)?;
        if !presentation.subjects.iter().any(|subject| {
            subject.identity == INSPECTOR_CLOSE_ACTION_ID
                && subject.role == PresentationRole::Action
        }) {
            return Ok(false);
        }
        let bounds = inspector_close_bounds(state.bounds.ok_or(TourShellError::Identity)?.width);
        Ok(i32::from(route.local_x) >= i32::from(bounds.x)
            && i32::from(route.local_x) < i32::from(bounds.x) + i32::from(bounds.width)
            && i32::from(route.local_y) >= i32::from(bounds.y)
            && i32::from(route.local_y) < i32::from(bounds.y) + i32::from(bounds.height))
    }

    pub fn activate_inspector_close(
        &mut self,
        route: &RoutedPointer,
        tour: &mut TourProduct,
        display: &mut impl PixelTarget,
    ) -> Result<bool, TourShellError> {
        if !self.inspector_close_hit(route)? {
            return Ok(false);
        }
        tour.dismiss_inspector()
            .map_err(|_| TourShellError::Identity)?;
        self.present(tour, display)?;
        Ok(true)
    }
}
