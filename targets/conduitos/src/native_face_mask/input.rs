//! Native input routing and bounded refusal presentation over the current Show.
use super::*;

impl NativeFaceMask {
    /// Key translation and local reading state belong to this Mask. A
    /// submitted semantic action exits through the same admitted Mask Fore.
    pub fn key(
        &mut self,
        event: KeyEvent,
        sequence: u64,
        display: &mut impl PixelTarget,
    ) -> Result<NativeFaceMaskInput, NativeFaceMaskError> {
        if !self.route_keyboard()? {
            return Err(NativeFaceMaskError::Face);
        }
        let show = self.show().ok_or(NativeFaceMaskError::Face)?.clone();
        let scene = self.scene.as_mut().ok_or(NativeFaceMaskError::Face)?;
        let cleared_notice =
            event.transition() == KeyTransition::Pressed && scene.clear_local_notice();
        let result = scene.key(event, &show, sequence);
        let result = self.present_input_result(result, display)?;
        if cleared_notice && matches!(result, FaceSceneInput::Unchanged) {
            return self.repaint(display).map(NativeFaceMaskInput::Redrawn);
        }
        self.accept_scene_input(result, display)
    }

    pub fn pointer(
        &mut self,
        x: u32,
        y: u32,
        sequence: u64,
        display: &mut impl PixelTarget,
    ) -> Result<NativeFaceMaskInput, NativeFaceMaskError> {
        let route = self
            .compositor
            .route_pointer(x, y, true)
            .map_err(NativeFaceMaskError::Compositor)?;
        let InputRoute::Delivered(route) = route else {
            return Ok(NativeFaceMaskInput::Unchanged);
        };
        self.compositor
            .validate_pointer_route(&route)
            .map_err(NativeFaceMaskError::Compositor)?;
        let show = self.show().ok_or(NativeFaceMaskError::Face)?.clone();
        if route.manifestation_id != show.show.manifestation_id {
            return Err(NativeFaceMaskError::Face);
        }
        let result = self
            .scene
            .as_mut()
            .ok_or(NativeFaceMaskError::Face)?
            .pointer(route.local_x as i16, route.local_y as i16, &show, sequence);
        let result = self.present_input_result(result, display)?;
        self.accept_scene_input(result, display)
    }

    fn accept_scene_input(
        &mut self,
        result: FaceSceneInput,
        display: &mut impl PixelTarget,
    ) -> Result<NativeFaceMaskInput, NativeFaceMaskError> {
        match result {
            FaceSceneInput::Unchanged => Ok(NativeFaceMaskInput::Unchanged),
            FaceSceneInput::Changed => self.repaint(display).map(NativeFaceMaskInput::Redrawn),
            FaceSceneInput::Submitted {
                interaction,
                next_focus,
            } => self
                .submit(*interaction)
                .map(|correlation| NativeFaceMaskInput::Submitted {
                    correlation,
                    next_focus,
                }),
        }
    }

    /// Refusal scanout never acknowledges a new semantic Show or consumes input.
    /// Preserve the original machine-readable error after presenting the notice.
    fn present_input_result(
        &mut self,
        result: Result<FaceSceneInput, FaceSceneError>,
        display: &mut impl PixelTarget,
    ) -> Result<FaceSceneInput, NativeFaceMaskError> {
        if matches!(result, Err(FaceSceneError::StaleFace)) {
            self.show_local_notice("Stale: refresh", display)?;
        }
        result.map_err(NativeFaceMaskError::Scene)
    }
}
