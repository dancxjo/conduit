//! Presenter-local scrolling of the Tour prose, independent of its live laboratory.
use super::*;
use crate::tour_workspace::lesson;
use conduit_presentation::GraphicsScene;

impl TourShellPresenter {
    pub(super) fn prepare_lesson_scene(
        &mut self,
        scene: GraphicsScene,
    ) -> Result<GraphicsScene, TourShellError> {
        let state = self
            .surfaces
            .iter_mut()
            .find(|state| state.slot == Slot::Workspace)
            .ok_or(TourShellError::Identity)?;
        if let Some((viewport, content)) = lesson::extent(&scene) {
            state.scroll.configure(viewport, content)?;
        }
        let visible =
            lesson::scrolled(&scene, state.scroll.offset()).map_err(|_| TourShellError::Scene)?;
        self.lesson_scene = Some(scene);
        Ok(visible)
    }

    pub(super) fn scroll_lesson(
        &mut self,
        direction: ScrollDirection,
        display: &mut impl PixelTarget,
    ) -> Result<ScrollOutcome, TourShellError> {
        let state = self
            .surfaces
            .iter_mut()
            .find(|state| state.slot == Slot::Workspace)
            .ok_or(TourShellError::Identity)?;
        let previous_offset = state.scroll.offset();
        if !state.scroll.apply(direction) {
            return Ok(ScrollOutcome::Boundary);
        }
        let current_offset = state.scroll.offset();
        let bounds = state.bounds.ok_or(TourShellError::Identity)?;
        let presentation = state.presentation.clone().ok_or(TourShellError::Identity)?;
        let scene = lesson::scrolled(
            self.lesson_scene.as_ref().ok_or(TourShellError::Identity)?,
            current_offset,
        )
        .map_err(|_| TourShellError::Scene)?;
        let composition = self.present_surface(
            Slot::Workspace,
            &presentation,
            TOUR_WORKSPACE_SUBJECT,
            bounds,
            0,
            &scene,
        )?;
        let frame = self
            .compositor
            .compose_frame(display)
            .map_err(TourShellError::Compositor)?;
        Ok(ScrollOutcome::Updated(scroll::ShellScrollReceipt {
            surface_id: WORKSPACE_SURFACE.into(),
            previous_offset,
            current_offset,
            composition,
            frame,
        }))
    }
}
