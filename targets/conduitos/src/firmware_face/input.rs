//! Typed input exits through the existing Mask and Crèche semantic owner.
use super::*;
use crate::native_face_mask::NativeFaceMaskInput;
impl<D: PixelTarget> FirmwareFace<D> {
    /// # Safety
    /// Root retains an independently admitted live keyboard provider and its
    /// original native endpoint custody throughout delivery, including loss.
    pub unsafe fn admit_keyboard(&mut self) -> Result<(), &'static str> {
        if self.keyboard_admitted {
            return Err("firmware-keyboard-already-admitted");
        }
        self._mask.suspend().map_err(|error| error.as_str())?;
        unsafe { self._mask.admit_local_input_for_root() }.map_err(|error| error.as_str())?;
        self.keyboard_admitted = true;
        self.present_pointer_basis()?;
        self._mask
            .focus_named(
                &crate::native_face_scene::FaceFocusRequest {
                    action_id: "creche.name".into(),
                    argument_name: Some("value".into()),
                },
                &mut self._display,
            )
            .map_err(|error| error.as_str())?;
        Ok(())
    }
    pub fn retire_keyboard(&mut self) -> Result<(), &'static str> {
        if !self.keyboard_admitted {
            return Ok(());
        }
        self.keyboard_admitted = false;
        if self
            .pointer
            .as_ref()
            .is_none_or(|pointer| pointer.is_retired())
        {
            self._mask.suspend().map_err(|error| error.as_str())?;
            self._mask.retire_local_input_for_root();
            if !self.born {
                self.present_pointer_basis()?;
            }
        }
        Ok(())
    }
    pub fn observe_key(&mut self, event: conduit_human::KeyEvent) -> Result<(), &'static str> {
        if !self.keyboard_admitted {
            return Err("firmware-keyboard-not-admitted");
        }
        if self.born {
            let revision = self._door.revision();
            if self
                ._door
                .accept(event, revision)
                .map_err(|error| error.as_str())?
            {
                self.presenter
                    .present(&self._door, &mut self._display)
                    .map_err(|error| error.as_str())?;
            }
            return Ok(());
        }
        self.pointer_sequence = self
            .pointer_sequence
            .checked_add(1)
            .ok_or("firmware-input-sequence")?;
        let show = self
            ._mask
            .show()
            .ok_or("firmware-face-show-absent")?
            .clone();
        let input = match self
            ._mask
            .key(event, self.pointer_sequence, &mut self._display)
        {
            Ok(input) => input,
            Err(_) if self._mask.acknowledged_input_refusal().is_some() => return Ok(()),
            Err(error) => return Err(error.as_str()),
        };
        self.accept_input(input, show)
    }
    pub(super) fn accept_input(
        &mut self,
        input: NativeFaceMaskInput,
        show: conduit_face::MaskShow,
    ) -> Result<(), &'static str> {
        let NativeFaceMaskInput::Submitted {
            correlation,
            next_focus,
        } = input
        else {
            return Ok(());
        };
        let basis = self._mask.birth_basis("conduitos/creche/current");
        match self
            ._door
            .apply_creche_face_interaction(&basis, &show, &correlation.interaction)
            .map_err(|error| error.as_str())?
        {
            conduit_birth_plot::BirthActionOutcome::Changed => {
                self.present_pointer_basis()?;
                let focus =
                    next_focus.unwrap_or_else(|| crate::native_face_scene::FaceFocusRequest {
                        action_id: correlation.interaction.action_id.clone(),
                        argument_name: correlation
                            .interaction
                            .arguments
                            .first()
                            .map(|argument| argument.name.clone()),
                    });
                match self._mask.focus_named(&focus, &mut self._display) {
                    Ok(_) => {}
                    Err(crate::native_face_mask::NativeFaceMaskError::Scene(
                        crate::native_face_scene::FaceSceneError::UnknownControl
                        | crate::native_face_scene::FaceSceneError::UnavailableAction,
                    )) => {
                        self._mask
                            .show_local_notice(
                                "Focus changed: control unavailable",
                                &mut self._display,
                            )
                            .map_err(|error| error.as_str())?;
                    }
                    Err(error) => return Err(error.as_str()),
                }
            }
            conduit_birth_plot::BirthActionOutcome::Birth(selection) => {
                self._mask.suspend().map_err(|error| error.as_str())?;
                crate::creche_arrival::birth_and_arrive(
                    selection,
                    &mut self._door,
                    &mut self._journey,
                    &mut self.presenter,
                    &mut self._display,
                )?;
                self.born = true;
            }
        }
        Ok(())
    }
}
