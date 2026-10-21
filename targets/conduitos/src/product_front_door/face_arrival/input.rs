//! Crèche submission through its acknowledged Mask and owner.
use super::*;
use conduit_birth_plot::BirthActionOutcome;

impl FaceArrival {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::product_front_door) fn accept_key(
        &mut self,
        event: KeyEvent,
        door: &mut FrontDoor,
        journey: &mut ProductJourney,
        presenter: &mut FrontDoorPresenter,
        display: &mut impl PixelTarget,
        make: &MakeRecord,
    ) -> Result<FaceArrivalInput, &'static str> {
        match self.select_local_look(event, door, display) {
            Ok(true) => return Ok(FaceArrivalInput::Continue),
            Ok(false) => {}
            Err(reason) => {
                super::super::native_action_evidence::emit_with_limit(
                    b"CONDUIT_NATIVE_COMPONENT_SELECTION ",
                    &serde_json::json!({"status":"refused", "code":reason}),
                    1_024,
                )?;
                if self.mask.show().is_none() {
                    return Err(reason);
                }
                return Ok(FaceArrivalInput::Continue);
            }
        }
        let show = self.mask.show().cloned().ok_or("creche-face-show-absent")?;
        let sequence = self.next_interaction;
        self.next_interaction = sequence.checked_add(1).ok_or("creche-face-input-bound")?;
        let input = self
            .mask
            .key(event, sequence, display)
            .map_err(|error| error.as_str())?;
        self.accept_input(input, show, door, journey, presenter, display, make)
    }

    pub(in crate::product_front_door) fn pointer_motion(
        &mut self,
        x: u32,
        y: u32,
        display: &mut impl PixelTarget,
    ) -> Result<(), &'static str> {
        self.mask
            .pointer_motion(x, y, display)
            .map_err(|error| error.as_str())
    }

    pub(in crate::product_front_door) fn observe_pointer(
        &self,
        sample: conduit_semantic_catalog::NormalizedPointerSample,
        x: u32,
        y: u32,
        press_edge: bool,
    ) -> Result<(), &'static str> {
        let show = self.mask.show().ok_or("creche-face-show-absent")?;
        super::super::native_action_evidence::pointer(show, sample, x, y, press_edge)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::product_front_door) fn accept_pointer(
        &mut self,
        x: u32,
        y: u32,
        door: &mut FrontDoor,
        journey: &mut ProductJourney,
        presenter: &mut FrontDoorPresenter,
        display: &mut impl PixelTarget,
        make: &MakeRecord,
    ) -> Result<FaceArrivalInput, &'static str> {
        let show = self.mask.show().cloned().ok_or("creche-face-show-absent")?;
        let sequence = self.next_interaction;
        self.next_interaction = sequence.checked_add(1).ok_or("creche-face-input-bound")?;
        let input = self
            .mask
            .pointer(x, y, sequence, display)
            .map_err(|error| error.as_str())?;
        self.accept_input(input, show, door, journey, presenter, display, make)
    }

    #[allow(clippy::too_many_arguments)]
    fn accept_input(
        &mut self,
        input: NativeFaceMaskInput,
        show: MaskShow,
        door: &mut FrontDoor,
        journey: &mut ProductJourney,
        presenter: &mut FrontDoorPresenter,
        display: &mut impl PixelTarget,
        make: &MakeRecord,
    ) -> Result<FaceArrivalInput, &'static str> {
        match input {
            NativeFaceMaskInput::Unchanged => Ok(FaceArrivalInput::Continue),
            NativeFaceMaskInput::Redrawn(_) => {
                self.record_focus()?;
                arch::early_write(b"CONDUIT_CRECHE_CHECKPOINT edited\n");
                Ok(FaceArrivalInput::Continue)
            }
            NativeFaceMaskInput::Submitted {
                correlation,
                next_focus,
            } => {
                let source_face = door
                    .creche_face(&self.basis)
                    .map_err(|error| error.as_str())?;
                let before = journey.projection();
                match door
                    .apply_creche_face_interaction(&self.basis, &show, &correlation.interaction)
                    .map_err(|error| error.as_str())?
                {
                    BirthActionOutcome::Changed => {
                        self.present_current(door, display)?;
                        let fresh_face = door
                            .creche_face(&self.basis)
                            .map_err(|error| error.as_str())?;
                        let focus = next_focus.unwrap_or_else(|| FaceFocusRequest {
                            action_id: correlation.interaction.action_id.clone(),
                            argument_name: correlation
                                .interaction
                                .arguments
                                .first()
                                .map(|argument| argument.name.clone()),
                        });
                        match self.mask.focus_named(&focus, display) {
                            Ok(_) => {}
                            Err(crate::native_face_mask::NativeFaceMaskError::Scene(
                                crate::native_face_scene::FaceSceneError::UnknownControl
                                | crate::native_face_scene::FaceSceneError::UnavailableAction,
                            )) => {
                                self.mask
                                    .show_local_notice(
                                        "Focus changed: control unavailable",
                                        display,
                                    )
                                    .map_err(|error| error.as_str())?;
                            }
                            Err(error) => return Err(error.as_str()),
                        }
                        self.record_focus()?;
                        arch::early_write(b"CONDUIT_CRECHE_CHECKPOINT edited\n");
                        let fresh_show = self.mask.show().ok_or("creche-face-show-absent")?;
                        super::super::native_action_evidence::creche_change(
                            &source_face,
                            &show,
                            &correlation.interaction,
                            &fresh_face,
                            fresh_show,
                        )?;
                        Ok(FaceArrivalInput::Continue)
                    }
                    BirthActionOutcome::Birth(selection) => {
                        self.mask.suspend().map_err(|error| error.as_str())?;
                        super::super::arrival::birth_and_arrive(
                            selection, door, journey, presenter, display, make,
                        )?;
                        let after = journey.projection();
                        let action = source_face
                            .resolve_action(
                                source_face.revision,
                                &correlation.interaction.action_id,
                            )
                            .map_err(|_| "creche-birth-source-action-absent")?;
                        super::super::native_action_evidence::action(
                            &source_face,
                            &show,
                            Some(&correlation.interaction),
                            &action.intent,
                            &before,
                            &after,
                            None,
                        )?;
                        Ok(FaceArrivalInput::Born)
                    }
                }
            }
        }
    }
}
