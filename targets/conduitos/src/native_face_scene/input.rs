//! Mask-local replacement buffers. No current value is inferred from Face prose.
use super::*;
use alloc::{string::String, vec};
use conduit_human::{ConduitIntlKeymap, KeyEvent, KeyModifiers, KeyTransition, KeymapDisposition};
use conduit_presentation::{ManifestationLifecycle, UTF8_TEXT_VALUE_KIND};

/// A navigation wish, resolved afresh after the producer accepts an interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceFocusRequest {
    pub action_id: String,
    pub argument_name: Option<String>,
}

#[derive(Debug)]
pub enum FaceSceneInput {
    Unchanged,
    Changed,
    Submitted {
        interaction: FaceInteraction,
        next_focus: Option<FaceFocusRequest>,
    },
}

#[derive(Default)]
pub(super) struct InputState {
    keymap: ConduitIntlKeymap,
    action: Option<usize>,
    values: Vec<Option<Vec<u8>>>,
    submitted: bool,
}

impl InputState {
    pub fn preview(&self, focus: Option<FaceControl>) -> Option<String> {
        let control = focus?;
        if self.action != Some(control.action) {
            return None;
        }
        let bytes = self.values.get(control.argument?)?.as_ref()?;
        let value = match bytes.as_slice() {
            [0] => "false",
            [1] => "true",
            _ => core::str::from_utf8(bytes).ok()?,
        };
        let mut start = value.len().saturating_sub(64);
        while !value.is_char_boundary(start) {
            start += 1;
        }
        Some(alloc::format!(
            "Draft{}: {} · Enter applies",
            if start > 0 { " (end)" } else { "" },
            &value[start..]
        ))
    }
}

impl NativeFaceScene {
    /// Resolve a named navigation request against this fresh Face, not an old
    /// action index. A missing/removed/unavailable target refuses explicitly.
    pub fn focus_named(&mut self, request: &FaceFocusRequest) -> Result<(), FaceSceneError> {
        let (action_index, action) = self
            .face
            .actions
            .iter()
            .enumerate()
            .find(|(_, action)| action.identity == request.action_id)
            .ok_or(FaceSceneError::UnknownControl)?;
        let argument = request
            .argument_name
            .as_ref()
            .map(|name| {
                action
                    .arguments
                    .iter()
                    .position(|argument| &argument.name == name)
                    .ok_or(FaceSceneError::UnknownControl)
            })
            .transpose()?;
        let control = FaceControl {
            action: action_index,
            argument,
        };
        self.resolve(&self.face.identity, self.face.revision, control)?;
        let primary = self.primary.iter().find(|row| row.control == Some(control));
        let page = primary
            .or_else(|| self.details.iter().find(|row| row.control == Some(control)))
            .ok_or(FaceSceneError::UnknownControl)?
            .page;
        self.showing_details = primary.is_none();
        self.focus = Some(control);
        self.page = page;
        Ok(())
    }

    /// Pointer coordinates use the exact currently displayed page. Clicking a
    /// field focuses it; clicking an argument-free action submits it. Dirty
    /// replacement input commits before changing focus, returning a fresh-focus
    /// request instead of applying a second action under the old Show.
    pub fn pointer(
        &mut self,
        x: i16,
        y: i16,
        show: &MaskShow,
        sequence: u64,
    ) -> Result<FaceSceneInput, FaceSceneError> {
        self.check_show(show)?;
        let control = self.frame()?.hit_test(x, y).map(|hit| hit.control);
        let Some(control) = control else {
            return if self.input.action.is_some() {
                self.commit(show, sequence, None)
            } else {
                Ok(FaceSceneInput::Unchanged)
            };
        };
        if self.input.action.is_some()
            && self.focus != Some(control)
            && !(self.input.action == Some(control.action)
                && self.input.values.iter().any(Option::is_none))
        {
            return self.commit(show, sequence, Some(self.focus_request(control)));
        }
        self.focus = Some(control);
        if control.argument.is_none() && self.face.actions[control.action].arguments.is_empty() {
            self.commit(show, sequence, None)
        } else {
            Ok(FaceSceneInput::Changed)
        }
    }

    pub fn key(
        &mut self,
        event: KeyEvent,
        show: &MaskShow,
        sequence: u64,
    ) -> Result<FaceSceneInput, FaceSceneError> {
        self.check_show(show)?;
        if event.transition() != KeyTransition::Pressed {
            self.input.keymap.apply(event);
            return Ok(FaceSceneInput::Unchanged);
        }
        if !self.input.keymap.is_composing() {
            match event.usage() {
                41 => {
                    self.input = InputState::default();
                    return Ok(FaceSceneInput::Changed);
                }
                43 => {
                    let forward = event.modifiers_after().bits()
                        & (KeyModifiers::LEFT_SHIFT.bits() | KeyModifiers::RIGHT_SHIFT.bits())
                        == 0;
                    let old = self.focus;
                    let old_page = self.page;
                    let next = self
                        .focus_next(forward)
                        .map(|control| self.focus_request(control));
                    if self.input.action.is_some()
                        && !(self
                            .focus
                            .is_some_and(|control| self.input.action == Some(control.action))
                            && self.input.values.iter().any(Option::is_none))
                    {
                        self.focus = old;
                        self.page = old_page;
                        return self.commit(show, sequence, next);
                    }
                    self.input.keymap.reset();
                    return Ok(FaceSceneInput::Changed);
                }
                59 | 75 | 78 => {
                    if self.input.action.is_some() {
                        return self.commit(show, sequence, None);
                    }
                    if event.usage() == 59 {
                        self.show_details(!self.showing_details);
                    } else {
                        self.turn_page(event.usage() == 78);
                    }
                    return Ok(FaceSceneInput::Changed);
                }
                40 => return self.commit(show, sequence, None),
                _ => {}
            }
        }
        let Some(control) = self.focus else {
            return Ok(FaceSceneInput::Unchanged);
        };
        let action = &self.face.actions[control.action];
        let argument_index = control
            .argument
            .or_else(|| (action.arguments.len() == 1).then_some(0))
            .ok_or(FaceSceneError::UnsupportedInput)?;
        let argument = &action.arguments[argument_index];
        let kind = argument.contract.value_kind.as_str();
        if !matches!(kind, UTF8_TEXT_VALUE_KIND | "value/bool") {
            return Err(FaceSceneError::UnsupportedInput);
        }
        let composing = self.input.keymap.is_composing();
        let disposition = self.input.keymap.apply(event);
        let mut value = if self.input.action == Some(control.action) {
            self.input
                .values
                .get(argument_index)
                .and_then(Clone::clone)
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if event.usage() == 42 && kind == UTF8_TEXT_VALUE_KIND && !composing {
            let text =
                core::str::from_utf8(&value).map_err(|_| FaceSceneError::UnsupportedInput)?;
            let end = text.char_indices().last().map_or(0, |(index, _)| index);
            value.truncate(end);
        } else if let KeymapDisposition::Text(fragment) = disposition {
            if kind == "value/bool" {
                value = match fragment.as_char() {
                    '0' => vec![0],
                    '1' => vec![1],
                    _ => return Err(FaceSceneError::UnsupportedInput),
                };
            } else {
                let mut bytes = [0; 4];
                value.extend_from_slice(fragment.encode_utf8(&mut bytes));
            }
        } else {
            return match disposition {
                KeymapDisposition::Refused(_) => Err(FaceSceneError::UnsupportedInput),
                _ => Ok(FaceSceneInput::Unchanged),
            };
        }
        if value.len() > argument.contract.maximum_bytes as usize {
            return Err(FaceSceneError::InputBound);
        }
        if self.input.action != Some(control.action) {
            self.input.action = Some(control.action);
            self.input.values = vec![None; action.arguments.len()];
        }
        let total = self
            .input
            .values
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != argument_index)
            .map(|(_, value)| value.as_ref().map_or(0, Vec::len))
            .sum::<usize>()
            + value.len();
        if total > conduit_presentation::MAX_FACE_INTERACTION_BYTES {
            return Err(FaceSceneError::InputBound);
        }
        self.input.values[argument_index] = Some(value);
        self.focus = Some(FaceControl {
            action: control.action,
            argument: Some(argument_index),
        });
        Ok(FaceSceneInput::Changed)
    }

    fn commit(
        &mut self,
        show: &MaskShow,
        sequence: u64,
        next_focus: Option<FaceFocusRequest>,
    ) -> Result<FaceSceneInput, FaceSceneError> {
        let control = self.focus.ok_or(FaceSceneError::UnknownControl)?;
        let action = &self.face.actions[control.action];
        let arguments = action
            .arguments
            .iter()
            .enumerate()
            .map(|(index, argument)| {
                if self.input.action != Some(control.action) {
                    return Err(FaceSceneError::IncompleteInput);
                }
                Ok(FaceInteractionArgument {
                    name: argument.name.clone(),
                    value_kind: argument.contract.value_kind.as_str().into(),
                    value: self
                        .input
                        .values
                        .get(index)
                        .and_then(Clone::clone)
                        .ok_or(FaceSceneError::IncompleteInput)?,
                })
            })
            .collect::<Result<Vec<_>, FaceSceneError>>()?;
        let interaction = self.interaction(
            &self.face.identity,
            self.face.revision,
            control,
            show,
            arguments,
            sequence,
        )?;
        // A submitted occurrence must be accepted/refused by the producer and
        // receive a fresh acknowledged Show before any further inward action.
        self.input.submitted = true;
        Ok(FaceSceneInput::Submitted {
            interaction,
            next_focus,
        })
    }

    fn check_show(&self, show: &MaskShow) -> Result<(), FaceSceneError> {
        if self.input.submitted || show.show.lifecycle != ManifestationLifecycle::Available {
            return Err(FaceSceneError::StaleFace);
        }
        show.validate(&self.face)
            .map_err(|_| FaceSceneError::StaleFace)
    }

    fn focus_request(&self, control: FaceControl) -> FaceFocusRequest {
        let action = &self.face.actions[control.action];
        FaceFocusRequest {
            action_id: action.identity.clone(),
            argument_name: control
                .argument
                .map(|index| action.arguments[index].name.clone()),
        }
    }
}
