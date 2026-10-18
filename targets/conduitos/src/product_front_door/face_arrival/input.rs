//! Crèche keyboard submission through its acknowledged Mask and owner.
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
        let show = self.mask.show().cloned().ok_or("creche-face-show-absent")?;
        let sequence = self.next_interaction;
        self.next_interaction = sequence.checked_add(1).ok_or("creche-face-input-bound")?;
        match self
            .mask
            .key(event, sequence, display)
            .map_err(|error| error.as_str())?
        {
            NativeFaceMaskInput::Unchanged => Ok(FaceArrivalInput::Continue),
            NativeFaceMaskInput::Redrawn(_) => {
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
