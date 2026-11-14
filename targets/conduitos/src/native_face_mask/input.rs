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
        let show = self.show().ok_or(NativeFaceMaskError::Face)?.clone();
        let result = route_acknowledged_native_key(
            &self.compositor,
            self.scene.as_mut().ok_or(NativeFaceMaskError::Face)?,
            &show,
            event,
            sequence,
        );
        let result = match result {
            Ok(result) => self.present_input_result(Ok(result), display)?,
            Err(NativeFaceMaskError::Scene(error)) => {
                self.present_input_result(Err(error), display)?
            }
            Err(error) => return Err(error),
        };
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
        route_acknowledged_native_pointer_motion(&mut self.compositor, show, x, y)?;
        self.compose_and_commit(display)?;
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
        let show = self.show().ok_or(NativeFaceMaskError::Face)?.clone();
        let result = route_acknowledged_native_pointer(
            &mut self.compositor,
            self.scene.as_mut().ok_or(NativeFaceMaskError::Face)?,
            &show,
            x,
            y,
            sequence,
        );
        let result = match result {
            Ok(result) => self.present_input_result(Ok(result), display)?,
            Err(NativeFaceMaskError::Scene(error)) => {
                self.present_input_result(Err(error), display)?
            }
            Err(error) => return Err(error),
        };
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

/// Reusable native compositor routing over this exact physically acknowledged
/// manifestation. The caller still owns current input admission/session/owner.
pub fn route_acknowledged_native_pointer_motion(
    compositor: &mut NativeCompositor,
    show: &MaskShow,
    x: u32,
    y: u32,
) -> Result<(), NativeFaceMaskError> {
    if let InputRoute::Delivered(route) = compositor
        .route_pointer(x, y, false)
        .map_err(NativeFaceMaskError::Compositor)?
    {
        compositor
            .validate_pointer_route(&route)
            .map_err(NativeFaceMaskError::Compositor)?;
        if route.manifestation_id != show.show.manifestation_id {
            return Err(NativeFaceMaskError::Face);
        }
    }
    Ok(())
}
pub fn route_acknowledged_native_pointer(
    compositor: &mut NativeCompositor,
    scene: &mut NativeFaceScene,
    show: &MaskShow,
    x: u32,
    y: u32,
    sequence: u64,
) -> Result<FaceSceneInput, NativeFaceMaskError> {
    let InputRoute::Delivered(route) = compositor
        .route_pointer(x, y, true)
        .map_err(NativeFaceMaskError::Compositor)?
    else {
        return Ok(FaceSceneInput::Unchanged);
    };
    compositor
        .validate_pointer_route(&route)
        .map_err(NativeFaceMaskError::Compositor)?;
    if route.manifestation_id != show.show.manifestation_id {
        return Err(NativeFaceMaskError::Face);
    }
    scene
        .pointer(route.local_x as i16, route.local_y as i16, show, sequence)
        .map_err(NativeFaceMaskError::Scene)
}

/// Route the original normalized HID event through the current acknowledged
/// manifestation, preserving the ordinary Scene focus, editing and submit rules.
pub fn route_acknowledged_native_key(
    compositor: &NativeCompositor,
    scene: &mut NativeFaceScene,
    show: &MaskShow,
    event: KeyEvent,
    sequence: u64,
) -> Result<FaceSceneInput, NativeFaceMaskError> {
    if !matches!(
        compositor
            .route_keyboard(&show.show.manifestation_id)
            .map_err(NativeFaceMaskError::Compositor)?,
        InputRoute::Delivered(_)
    ) {
        return Err(NativeFaceMaskError::Face);
    }
    let cleared = event.transition() == KeyTransition::Pressed
        && !(0xe0..=0xe7).contains(&event.usage())
        && scene.clear_local_notice();
    let result = scene
        .key(event, show, sequence)
        .map_err(NativeFaceMaskError::Scene)?;
    Ok(if cleared && matches!(result, FaceSceneInput::Unchanged) {
        FaceSceneInput::Changed
    } else {
        result
    })
}
