//! Native input routing and bounded refusal presentation over the current Show.
use super::*;

impl NativeFaceMask {
    /// The exact interaction returned by this Mask Plot is still a request to
    /// the caller's semantic owner. It does not mutate Birth or Body state.
    pub fn submit(
        &mut self,
        interaction: FaceInteraction,
    ) -> Result<MaskInteractionCorrelation, NativeFaceMaskError> {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        self.compositor
            .validate_active_session()
            .map_err(NativeFaceMaskError::Compositor)?;
        if !self.interaction_admitted {
            return Err(NativeFaceMaskError::InputUnavailable);
        }
        self.session
            .as_ref()
            .ok_or(NativeFaceMaskError::Face)?
            .validate_interaction(&interaction)
            .map_err(NativeFaceMaskError::Mask)?;
        self.session
            .take()
            .ok_or(NativeFaceMaskError::Face)?
            .interact(interaction)
            .map_err(NativeFaceMaskError::Mask)
    }

    /// A stale occurrence cannot consume the current session. Its notice is a
    /// local repaint, not a new semantic Show or an owner action acceptance.
    pub fn submit_with_display(
        &mut self,
        interaction: FaceInteraction,
        display: &mut impl PixelTarget,
    ) -> Result<MaskInteractionCorrelation, NativeFaceMaskError> {
        self.input_refusal = None;
        let result = self.submit(interaction);
        if matches!(
            result.as_ref(),
            Err(NativeFaceMaskError::Mask(NativeMaskPlayError::Interaction(
                conduit_face::FaceInteractionRefusal::StaleShow
                    | conduit_face::FaceInteractionRefusal::StaleFace
            )))
        ) {
            self.show_local_notice("Stale: refresh", display)?;
        }
        result
    }

    /// Key translation and local reading state belong to this Mask. A
    /// submitted semantic action exits through the same admitted Mask Fore.
    pub fn key(
        &mut self,
        event: KeyEvent,
        sequence: u64,
        display: &mut impl PixelTarget,
    ) -> Result<NativeFaceMaskInput, NativeFaceMaskError> {
        if !self.local_input_available {
            return Err(NativeFaceMaskError::InputUnavailable);
        }
        self.input_refusal = None;
        if !self.route_keyboard()? {
            return Err(NativeFaceMaskError::Face);
        }
        let show = self.show().ok_or(NativeFaceMaskError::Face)?.clone();
        let scene = self.scene.as_mut().ok_or(NativeFaceMaskError::Face)?;
        let cleared_notice = event.transition() == KeyTransition::Pressed
            && !(0xe0..=0xe7).contains(&event.usage())
            && scene.clear_local_notice();
        let result = scene.key(event, &show, sequence);
        let result = self.present_input_result(result, display)?;
        if cleared_notice && matches!(result, FaceSceneInput::Unchanged) {
            return self.repaint(display).map(NativeFaceMaskInput::Redrawn);
        }
        self.accept_scene_input(result, display)
    }

    /// Move the admitted cursor over this acknowledged Show without making a
    /// semantic input occurrence or changing action focus.
    pub fn pointer_motion(
        &mut self,
        x: u32,
        y: u32,
        display: &mut impl PixelTarget,
    ) -> Result<(), NativeFaceMaskError> {
        if !self.local_input_available {
            return Err(NativeFaceMaskError::InputUnavailable);
        }
        let show = self
            .session
            .as_ref()
            .ok_or(NativeFaceMaskError::Face)?
            .show();
        let route = self
            .compositor
            .route_pointer(x, y, false)
            .map_err(NativeFaceMaskError::Compositor)?;
        if let InputRoute::Delivered(route) = route {
            self.compositor
                .validate_pointer_route(&route)
                .map_err(NativeFaceMaskError::Compositor)?;
            if route.manifestation_id != show.show.manifestation_id {
                return Err(NativeFaceMaskError::Face);
            }
        }
        if let Err(error) = self.compositor.compose_frame(display) {
            self.session.take();
            return Err(NativeFaceMaskError::Compositor(error));
        }
        Ok(())
    }

    pub fn pointer(
        &mut self,
        x: u32,
        y: u32,
        sequence: u64,
        display: &mut impl PixelTarget,
    ) -> Result<NativeFaceMaskInput, NativeFaceMaskError> {
        if !self.local_input_available {
            return Err(NativeFaceMaskError::InputUnavailable);
        }
        self.input_refusal = None;
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
                .submit_with_display(*interaction, display)
                .map(|correlation| NativeFaceMaskInput::Submitted {
                    correlation,
                    next_focus,
                }),
        }
    }

    /// Custody of this input's completed local notice repaint, never an owner
    /// action acceptance. Reset on input and semantic publication replacement.
    pub fn acknowledged_input_refusal(&self) -> Option<(FaceSceneError, &'static str)> {
        self.show()?;
        let reason = self.input_refusal?;
        Some((reason, refusal_notice(reason)?))
    }

    /// Refusal scanout never acknowledges a new semantic Show or consumes input.
    /// Preserve the original machine-readable error after presenting the notice.
    fn present_input_result(
        &mut self,
        result: Result<FaceSceneInput, FaceSceneError>,
        display: &mut impl PixelTarget,
    ) -> Result<FaceSceneInput, NativeFaceMaskError> {
        self.input_refusal = None;
        if let Some(reason) = result.as_ref().err().copied()
            && let Some(notice) = refusal_notice(reason)
        {
            self.show_local_notice(notice, display)?;
            self.input_refusal = Some(reason);
        }
        result.map_err(NativeFaceMaskError::Scene)
    }
}

fn refusal_notice(error: FaceSceneError) -> Option<&'static str> {
    match error {
        FaceSceneError::StaleFace => Some("Stale: refresh"),
        FaceSceneError::UnknownControl => Some("Refresh"),
        FaceSceneError::UnavailableAction => Some("Unavailable"),
        FaceSceneError::UnsupportedInput => Some("Unsupported"),
        FaceSceneError::InputBound => Some("Too long"),
        FaceSceneError::IncompleteInput => Some("Enter value"),
        _ => None,
    }
}

#[cfg(test)]
#[path = "refusal_tests.rs"]
mod refusal_tests;
