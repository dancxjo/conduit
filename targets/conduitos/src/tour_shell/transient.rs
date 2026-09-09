//! Transient Presentation admission, focus, and dismissal lifecycle.

use crate::display::PixelTarget;

use super::*;

impl TourShellPresenter {
    /// Resolve a current chooser row to its exact semantic Gear identity.
    pub fn chooser_gear(
        &self,
        route: &RoutedPointer,
    ) -> Result<Option<&'static str>, TourShellError> {
        self.validate_pointer_route(route)?;
        let Some(state) = self
            .surfaces
            .iter()
            .find(|state| state.slot == Slot::Transient)
        else {
            return Ok(None);
        };
        if route.surface_id != TRANSIENT_SURFACE
            || state.face_subject.as_deref() != Some(TourTransientKind::Chooser.subject_identity())
        {
            return Ok(None);
        }
        let bounds = state.bounds.ok_or(TourShellError::Identity)?;
        if route.local_y < 70
            || route.local_x < 12
            || route.local_x >= bounds.width.saturating_sub(12)
        {
            return Ok(None);
        }
        Ok(self.scroll_hit_subject(route)?.and_then(|row| {
            conduit_tour_model::CANONICAL_PATCHBAY_GEARS
                .get(usize::from(row))
                .copied()
        }))
    }

    pub fn show_transient(
        &mut self,
        tour: &TourProduct,
        kind: TourTransientKind,
        detail: &str,
        display: &mut impl PixelTarget,
    ) -> Result<ShellTransientReceipt, TourShellError> {
        if self.has_transient() {
            self.dismiss(Slot::Transient)?;
        }
        let format = display
            .format()
            .validate()
            .map_err(NativeCompositorError::from)
            .map_err(TourShellError::Compositor)?;
        let layout = ShellLayout::new(
            u16::try_from(format.width).map_err(|_| TourShellError::Identity)?,
            u16::try_from(format.height).map_err(|_| TourShellError::Identity)?,
        )?;
        let presentation = tour
            .controller()
            .state()
            .transient_presentation(kind, detail)
            .map_err(|_| TourShellError::Identity)?;
        let receipt = self.present_surface(
            Slot::Transient,
            &presentation,
            kind.subject_identity(),
            layout.transient,
            3,
            &transient_scene(layout.transient, &presentation, 0)?,
        )?;
        let frame = self
            .compositor
            .compose_frame(display)
            .map_err(TourShellError::Compositor)?;
        self.compositor
            .focus_surface(TRANSIENT_SURFACE)
            .map_err(TourShellError::Compositor)?;
        let parent_manifestation_id = self
            .surfaces
            .iter()
            .find(|state| state.slot == Slot::Workspace)
            .and_then(|state| state.manifestation_id.clone())
            .ok_or(TourShellError::Identity)?;
        let parent_presentation_id = tour
            .controller()
            .state()
            .workspace_presentation()
            .map_err(|_| TourShellError::Identity)?
            .identity;
        Ok(ShellTransientReceipt {
            kind,
            parent_presentation_id,
            parent_manifestation_id,
            transient: receipt,
            frame,
        })
    }

    pub fn dismiss_transient(
        &mut self,
        display: &mut impl PixelTarget,
    ) -> Result<ShellTransientDismissalReceipt, TourShellError> {
        let manifestation_id = self
            .surfaces
            .iter()
            .find(|state| state.slot == Slot::Transient)
            .and_then(|state| state.manifestation_id.clone())
            .ok_or(TourShellError::Identity)?;
        self.dismiss(Slot::Transient)?;
        let frame = self
            .compositor
            .compose_frame(display)
            .map_err(TourShellError::Compositor)?;
        self.compositor
            .focus_surface(WORKSPACE_SURFACE)
            .map_err(TourShellError::Compositor)?;
        Ok(ShellTransientDismissalReceipt {
            surface_id: TRANSIENT_SURFACE.into(),
            manifestation_id,
            frame,
        })
    }
}
