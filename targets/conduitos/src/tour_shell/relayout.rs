//! Presenter-owned relayout after compositor raster invalidation.

use conduit_presentation::Presentation;

use crate::{
    display::PixelTarget,
    native_compositor::{InputRoute, SurfacePlacementReceipt},
};

use super::*;

impl TourShellPresenter {
    pub fn relayout_inspector(
        &mut self,
        tour: &TourProduct,
        display: &mut impl PixelTarget,
    ) -> Result<ShellRelayoutReceipt, TourShellError> {
        let format = display
            .format()
            .validate()
            .map_err(NativeCompositorError::from)
            .map_err(TourShellError::Compositor)?;
        let layout = ShellLayout::new(
            u16::try_from(format.width).map_err(|_| TourShellError::Identity)?,
            u16::try_from(format.height).map_err(|_| TourShellError::Identity)?,
        )?;
        let previous_bounds = layout.inspector;
        let shrink = 48_u16.min(previous_bounds.width.saturating_sub(160));
        let current_bounds = LayoutRect {
            x: previous_bounds
                .x
                .checked_add(i16::try_from(shrink).map_err(|_| TourShellError::Identity)?)
                .ok_or(TourShellError::Identity)?,
            width: previous_bounds
                .width
                .checked_sub(shrink)
                .ok_or(TourShellError::Identity)?,
            ..previous_bounds
        };
        let placement = self
            .compositor
            .place_surface(INSPECTOR_SURFACE, current_bounds, 2)
            .map_err(TourShellError::Compositor)?;
        let SurfacePlacementReceipt::RasterInvalidated {
            manifestation_id: Some(invalidated_manifestation_id),
            ..
        } = placement
        else {
            return Err(TourShellError::Identity);
        };
        let pointer_refused = matches!(
            self.compositor
                .route_pointer(
                    u32::try_from(current_bounds.x).map_err(|_| TourShellError::Identity)?,
                    u32::try_from(current_bounds.y).map_err(|_| TourShellError::Identity)?,
                    false,
                )
                .map_err(TourShellError::Compositor)?,
            InputRoute::NoTarget
        );
        let keyboard_refused = matches!(self.route_keyboard()?, InputRoute::NoTarget);
        let presentation = tour
            .controller()
            .state()
            .inspector_presentation()
            .map_err(|_| TourShellError::Identity)?
            .ok_or(TourShellError::Identity)?;
        let face = presentation
            .subjects
            .iter()
            .find(|subject| subject.identity.ends_with("/inspection"))
            .ok_or(TourShellError::Identity)?
            .identity
            .clone();
        let revised = Presentation::new(
            presentation
                .revision
                .checked_add(1)
                .ok_or(TourShellError::Identity)?,
            presentation.basis,
            presentation.subjects,
            presentation.relationships,
            presentation.properties,
            presentation.text,
        )
        .map_err(|_| TourShellError::Identity)?;
        let current = self.present_surface(
            Slot::Inspector,
            &revised,
            &face,
            current_bounds,
            2,
            &inspector_scene(current_bounds, &revised)?,
        )?;
        let frame = self
            .compositor
            .compose_frame(display)
            .map_err(TourShellError::Compositor)?;
        Ok(ShellRelayoutReceipt {
            surface_id: INSPECTOR_SURFACE.into(),
            previous_bounds,
            current_bounds,
            invalidated_manifestation_id,
            current,
            input_refused_while_invalidated: pointer_refused && keyboard_refused,
            frame,
        })
    }
}
