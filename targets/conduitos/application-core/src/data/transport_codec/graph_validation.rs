//! Bounded UTF-8 and grammar phases preserve whole-validator refusal precedence.
use super::{
    Refusal,
    streaming_preflight::{STEP_BYTES, Scanner},
};
/// A phase cursor bound to the frozen serialization's exact length.
pub(crate) struct GraphValidation {
    length: usize,
    utf_cursor: usize,
    scan: Scanner,
    terminal: bool,
}
impl GraphValidation {
    pub(crate) fn new(length: usize) -> Result<Self, Refusal> {
        if length == 0 || length > super::MAX_BYTES {
            return Err(Refusal::Bounds);
        }
        Ok(Self {
            length,
            utf_cursor: 0,
            scan: Scanner::new(super::MAX_BYTES),
            terminal: false,
        })
    }
    pub(crate) fn cursor(&self) -> usize {
        self.utf_cursor + self.scan.offset()
    }
    pub(crate) fn step(&mut self, bytes: &[u8]) -> Result<bool, Refusal> {
        if self.terminal {
            return Err(Refusal::Order);
        }
        let result = self.advance(bytes);
        if !matches!(result, Ok(false)) {
            self.terminal = true;
        }
        result
    }
    fn advance(&mut self, bytes: &[u8]) -> Result<bool, Refusal> {
        if bytes.len() != self.length {
            return Err(Refusal::Bounds);
        }
        // Constant-size authentication, not a whole-input pass.
        if !bytes.starts_with(super::PREFIX) {
            return Err(Refusal::Malformed);
        }
        // Preserve the old validator's error precedence: UTF-8 is authenticated
        // before any grammar/depth/count error. This remains a bounded pass,
        // never a whole-buffer scan in one protected entry.
        if self.utf_cursor < self.length {
            let end = self.utf_cursor.saturating_add(STEP_BYTES).min(self.length);
            for &byte in &bytes[self.utf_cursor..end] {
                self.scan.utf8(byte)?;
            }
            self.utf_cursor = end;
            if end == self.length && !self.scan.utf8_finished() {
                return Err(Refusal::Malformed);
            }
            return Ok(false);
        }
        self.scan.step(bytes, STEP_BYTES)
    }
}
