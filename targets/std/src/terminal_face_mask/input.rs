use super::*;
use conduit_presentation::{FaceInteractionArgument, UTF8_TEXT_VALUE_KIND};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalControl {
    pub action: usize,
    pub argument: Option<usize>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInput {
    NextControl,
    PreviousControl,
    NextClause,
    PreviousClause,
    PageDown,
    PageUp,
    Inspect,
    Text(char),
    Backspace,
    Toggle,
    Apply,
    Escape,
    Quit,
}
#[derive(Debug, PartialEq, Eq)]
pub enum TerminalInputOutcome {
    Unchanged,
    Redraw,
    Cancelled,
    Quit,
    Interaction(FaceInteraction),
}

impl TerminalFaceMask {
    pub fn input(
        &mut self,
        input: TerminalInput,
        expected: &MaskShow,
        sequence: u64,
    ) -> Result<TerminalInputOutcome, TerminalError> {
        self.check_show(expected)?;
        match input {
            TerminalInput::Apply => {
                let control = self.focus.ok_or(TerminalError::UnknownControl)?;
                let action = &self.face.actions[control.action];
                let arguments = action
                    .arguments
                    .iter()
                    .enumerate()
                    .map(|(i, a)| {
                        let value = self.drafts[control.action][i].clone().ok_or(
                            TerminalError::Interaction(FaceInteractionRefusal::MissingArgument),
                        )?;
                        Ok(FaceInteractionArgument {
                            name: a.name.clone(),
                            value_kind: a.contract.value_kind.as_str().into(),
                            value,
                        })
                    })
                    .collect::<Result<_, TerminalError>>()?;
                let interaction = FaceInteraction::new(
                    &self.face,
                    expected,
                    &action.identity,
                    &action.target,
                    arguments,
                    sequence,
                )
                .map_err(TerminalError::Interaction)?;
                self.submitted = true;
                return Ok(TerminalInputOutcome::Interaction(interaction));
            }
            TerminalInput::Quit => {
                self.active_show = None;
                return Ok(TerminalInputOutcome::Quit);
            }
            TerminalInput::Escape => {
                self.active_show = None;
                for args in &mut self.drafts {
                    args.fill(None);
                }
                return Ok(TerminalInputOutcome::Cancelled);
            }
            TerminalInput::Inspect => {
                self.inspect = !self.inspect;
                self.top = 0;
                self.reading = 0;
                self.focus = None;
            }
            TerminalInput::NextControl | TerminalInput::PreviousControl => {
                self.inspect = false;
                let mut controls = Vec::new();
                for (i, row) in self.document.iter().enumerate() {
                    if let Some(c) = row.control {
                        if self.face.actions[c.action].availability.is_available()
                            && !controls.iter().any(|(_, v)| *v == c)
                        {
                            controls.push((i, c));
                        }
                    }
                }
                if controls.is_empty() {
                    return Ok(TerminalInputOutcome::Unchanged);
                }
                let old = controls.iter().position(|(_, c)| Some(*c) == self.focus);
                let next = if input == TerminalInput::NextControl {
                    old.map_or(0, |i| (i + 1) % controls.len())
                } else {
                    old.map_or(controls.len() - 1, |i| {
                        (i + controls.len() - 1) % controls.len()
                    })
                };
                self.reading = controls[next].0;
                self.focus = Some(controls[next].1);
                self.reveal();
            }
            TerminalInput::NextClause | TerminalInput::PreviousClause => {
                let rows = self.current_rows();
                let clause = rows.get(self.reading).and_then(|r| r.clause);
                let next = if input == TerminalInput::NextClause {
                    ((self.reading + 1)..rows.len())
                        .find(|&i| rows[i].clause != clause || clause.is_none())
                } else {
                    (0..self.reading)
                        .rev()
                        .find(|&i| rows[i].clause != clause || clause.is_none())
                };
                if let Some(next) = next {
                    self.reading = next;
                    self.focus = self.current_rows()[next].control;
                    self.reveal();
                }
            }
            TerminalInput::PageDown | TerminalInput::PageUp => {
                self.top = if input == TerminalInput::PageDown {
                    (self.top + self.page_rows()).min(self.current_rows().len().saturating_sub(1))
                } else {
                    self.top.saturating_sub(self.page_rows())
                };
                self.reading = self.top;
                self.focus = None;
            }
            TerminalInput::Text(_) | TerminalInput::Backspace | TerminalInput::Toggle => {
                self.edit(input)?
            }
        }
        // The changed frame must cross a new renderer effect before its input
        // mapping is available. Existing drafts survive only this SAME Face.
        self.active_show = None;
        Ok(TerminalInputOutcome::Redraw)
    }
    fn reveal(&mut self) {
        if self.reading < self.top {
            self.top = self.reading;
        }
        if self.reading >= self.top + self.page_rows() {
            self.top = self.reading + 1 - self.page_rows();
        }
    }
    fn edit(&mut self, input: TerminalInput) -> Result<(), TerminalError> {
        let control = self.focus.ok_or(TerminalError::UnknownControl)?;
        let action = &self.face.actions[control.action];
        let index = control
            .argument
            .or_else(|| (action.arguments.len() == 1).then_some(0))
            .ok_or(TerminalError::UnknownControl)?;
        let declaration = &action.arguments[index];
        let old = self.drafts[control.action][index].as_deref().unwrap_or(&[]);
        let mut value = old.to_vec();
        match declaration.contract.value_kind.as_str() {
            UTF8_TEXT_VALUE_KIND => match input {
                TerminalInput::Text(ch) if !ch.is_control() => {
                    let mut bytes = [0; 4];
                    value.extend_from_slice(ch.encode_utf8(&mut bytes).as_bytes());
                }
                TerminalInput::Backspace => {
                    let text =
                        std::str::from_utf8(&value).map_err(|_| TerminalError::MalformedInput)?;
                    value.truncate(text.char_indices().last().map_or(0, |(i, _)| i));
                }
                _ => return Err(TerminalError::UnsupportedInput),
            },
            "value/bool" => {
                value = match input {
                    TerminalInput::Toggle | TerminalInput::Text(' ') => vec![u8::from(old != [1])],
                    TerminalInput::Text('0') => vec![0],
                    TerminalInput::Text('1') => vec![1],
                    _ => return Err(TerminalError::UnsupportedInput),
                }
            }
            _ => return Err(TerminalError::UnsupportedInput),
        }
        let retained: usize = self.drafts.iter().flatten().flatten().map(Vec::len).sum();
        if value.len() > declaration.contract.maximum_bytes as usize
            || retained - old.len() + value.len() > MAX_TERMINAL_DRAFT_BYTES
        {
            return Err(TerminalError::InputPressure);
        }
        self.drafts[control.action][index] = Some(value);
        Ok(())
    }
}
