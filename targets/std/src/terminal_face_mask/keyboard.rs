//! Incremental bounded terminal-key decoder. The host supplies byte readiness;
//! it calls `escape_timeout` for an isolated Escape, never a hidden blocking read.
use super::{TerminalError, TerminalInput};
#[derive(Default)]
pub struct TerminalKeyboard {
    pending: [u8; 8],
    length: usize,
}
impl TerminalKeyboard {
    pub fn push(&mut self, byte: u8) -> Result<Option<TerminalInput>, TerminalError> {
        if self.length == 0 {
            let key = match byte {
                3 => Some(TerminalInput::Quit),
                9 => Some(TerminalInput::NextControl),
                10 | 13 => Some(TerminalInput::Apply),
                8 | 127 => Some(TerminalInput::Backspace),
                _ => None,
            };
            if key.is_some() {
                return Ok(key);
            }
        }
        if self.length == self.pending.len() {
            self.length = 0;
            return Err(TerminalError::InputPressure);
        }
        self.pending[self.length] = byte;
        self.length += 1;
        let bytes = &self.pending[..self.length];
        if bytes[0] == 27 {
            let keys: &[(&[u8], TerminalInput)] = &[
                (b"\x1b[A", TerminalInput::PreviousClause),
                (b"\x1b[B", TerminalInput::NextClause),
                (b"\x1b[Z", TerminalInput::PreviousControl),
                (b"\x1b[5~", TerminalInput::PageUp),
                (b"\x1b[6~", TerminalInput::PageDown),
                (b"\x1bOQ", TerminalInput::Inspect),
                (b"\x1b[12~", TerminalInput::Inspect),
            ];
            if let Some((_, key)) = keys.iter().find(|(sequence, _)| *sequence == bytes) {
                let key = key.clone();
                self.length = 0;
                return Ok(Some(key));
            }
            if keys.iter().any(|(sequence, _)| sequence.starts_with(bytes)) {
                return Ok(None);
            }
            self.length = 0;
            return Err(TerminalError::UnsupportedInput);
        }
        match std::str::from_utf8(bytes) {
            Ok(text) => {
                let ch = text.chars().next().ok_or(TerminalError::MalformedInput)?;
                self.length = 0;
                if ch.is_control() {
                    Err(TerminalError::UnsupportedInput)
                } else {
                    Ok(Some(TerminalInput::Text(ch)))
                }
            }
            Err(e) if e.error_len().is_none() && self.length < 4 => Ok(None),
            Err(_) => {
                self.length = 0;
                Err(TerminalError::MalformedInput)
            }
        }
    }
    pub fn escape_timeout(&mut self) -> Option<TerminalInput> {
        let escape = self.length == 1 && self.pending[0] == 27;
        self.length = 0;
        escape.then_some(TerminalInput::Escape)
    }
}
