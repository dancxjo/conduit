/// Maximum bytes retained for one unfinished logical line.
pub const LINES_MAX_RETAINED_PREFIX_BYTES: usize = 1024;
/// Maximum bytes in one emitted line, excluding an LF or CRLF delimiter.
pub const LINES_MAX_LINE_BYTES: usize = 1024;
/// Maximum finite items retained by the reference join provider.
pub const JOIN_MAX_ITEMS: usize = 8;
/// Maximum bytes in one join item.
pub const JOIN_MAX_ITEM_BYTES: usize = 1024;
/// Maximum separator size.
pub const JOIN_MAX_SEPARATOR_BYTES: usize = 64;
/// Maximum joined output size.
pub const JOIN_MAX_OUTPUT_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineError {
    RetainedPrefixOverflow,
    InvalidUtf8,
    OutputTooSmall,
    TooManyItems,
    ItemTooLarge,
    SeparatorTooLarge,
    JoinedOutputTooLarge,
}

/// Allocator-free incremental UTF-8 validator.
///
/// Each call validates exactly one byte, allowing hosted drivers to account
/// scanning work before performing it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Utf8State {
    remaining: u8,
    next_min: u8,
    next_max: u8,
}

impl Utf8State {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            remaining: 0,
            next_min: 0x80,
            next_max: 0xbf,
        }
    }

    pub fn push_byte(&mut self, byte: u8) -> Result<(), LineError> {
        if self.remaining > 0 {
            if byte < self.next_min || byte > self.next_max {
                return Err(LineError::InvalidUtf8);
            }
            self.remaining -= 1;
            self.next_min = 0x80;
            self.next_max = 0xbf;
            return Ok(());
        }
        let (remaining, next_min, next_max) = match byte {
            0x00..=0x7f => (0, 0x80, 0xbf),
            0xc2..=0xdf => (1, 0x80, 0xbf),
            0xe0 => (2, 0xa0, 0xbf),
            0xe1..=0xec | 0xee..=0xef => (2, 0x80, 0xbf),
            0xed => (2, 0x80, 0x9f),
            0xf0 => (3, 0x90, 0xbf),
            0xf1..=0xf3 => (3, 0x80, 0xbf),
            0xf4 => (3, 0x80, 0x8f),
            _ => return Err(LineError::InvalidUtf8),
        };
        self.remaining = remaining;
        self.next_min = next_min;
        self.next_max = next_max;
        Ok(())
    }

    pub fn finish(&self) -> Result<(), LineError> {
        if self.remaining == 0 {
            Ok(())
        } else {
            Err(LineError::InvalidUtf8)
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl LineError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::RetainedPrefixOverflow => "CND-TXT-001",
            Self::InvalidUtf8 => "CND-TXT-002",
            Self::OutputTooSmall => "CND-TXT-003",
            Self::TooManyItems => "CND-TXT-004",
            Self::ItemTooLarge => "CND-TXT-005",
            Self::SeparatorTooLarge => "CND-TXT-006",
            Self::JoinedOutputTooLarge => "CND-TXT-007",
        }
    }
}

/// Allocator-free incremental LF/CRLF line splitter.
///
/// Delimiters are removed. Empty lines are emitted. A final unterminated,
/// non-empty prefix is emitted by [`Self::finish`]. UTF-8 is validated per
/// accepted byte, so a code point may cross input chunk boundaries.
pub struct LinesState {
    retained: [u8; LINES_MAX_RETAINED_PREFIX_BYTES],
    retained_len: usize,
    ready_len: Option<usize>,
    finished: bool,
    utf8: Utf8State,
}

impl Default for LinesState {
    fn default() -> Self {
        Self::new()
    }
}

impl LinesState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            retained: [0; LINES_MAX_RETAINED_PREFIX_BYTES],
            retained_len: 0,
            ready_len: None,
            finished: false,
            utf8: Utf8State::new(),
        }
    }

    /// Accepts one byte. Call [`Self::take_ready`] before accepting another
    /// byte after this returns `true`.
    pub fn push_byte(&mut self, byte: u8) -> Result<bool, LineError> {
        if self.ready_len.is_some() || self.finished {
            return Err(LineError::OutputTooSmall);
        }
        self.utf8.push_byte(byte)?;
        if byte == b'\n' {
            self.utf8.finish()?;
            let length = if self.retained_len > 0 && self.retained[self.retained_len - 1] == b'\r' {
                self.retained_len - 1
            } else {
                self.retained_len
            };
            self.ready_len = Some(length);
            return Ok(true);
        }
        if self.retained_len >= self.retained.len() {
            return Err(LineError::RetainedPrefixOverflow);
        }
        self.retained[self.retained_len] = byte;
        self.retained_len += 1;
        Ok(false)
    }

    /// Marks terminal input and exposes a final unterminated line when one is
    /// retained. Empty input does not invent a line.
    pub fn finish(&mut self) -> Result<bool, LineError> {
        if self.ready_len.is_some() {
            return Ok(true);
        }
        self.finished = true;
        self.utf8.finish()?;
        if self.retained_len == 0 {
            return Ok(false);
        }
        self.ready_len = Some(self.retained_len);
        Ok(true)
    }

    /// Length of the ready logical line, if any.
    #[must_use]
    pub const fn ready_len(&self) -> Option<usize> {
        self.ready_len
    }

    /// Reads one ready byte without copying the complete line.
    #[must_use]
    pub fn ready_byte(&self, index: usize) -> Option<u8> {
        let length = self.ready_len?;
        (index < length).then(|| self.retained[index])
    }

    /// Clears a completely consumed ready line.
    pub fn clear_ready(&mut self) -> Result<(), LineError> {
        if self.ready_len.is_none() {
            return Err(LineError::OutputTooSmall);
        }
        self.retained_len = 0;
        self.ready_len = None;
        self.utf8.reset();
        Ok(())
    }

    /// Copies the ready line to caller-owned storage and clears retained
    /// state. The copied bytes never include LF or the CR in CRLF.
    pub fn take_ready(&mut self, output: &mut [u8]) -> Result<Option<usize>, LineError> {
        let Some(length) = self.ready_len else {
            return Ok(None);
        };
        if output.len() < length {
            return Err(LineError::OutputTooSmall);
        }
        output[..length].copy_from_slice(&self.retained[..length]);
        self.clear_ready()?;
        Ok(Some(length))
    }

    /// Cancellation discards the finite retained prefix.
    pub fn cancel(&mut self) {
        self.retained_len = 0;
        self.ready_len = None;
        self.finished = true;
        self.utf8.reset();
    }
}

/// Joins one complete finite item set into caller-owned storage.
pub fn join_text_into(
    items: &[&str],
    separator: &str,
    output: &mut [u8],
) -> Result<usize, LineError> {
    if items.len() > JOIN_MAX_ITEMS {
        return Err(LineError::TooManyItems);
    }
    if separator.len() > JOIN_MAX_SEPARATOR_BYTES {
        return Err(LineError::SeparatorTooLarge);
    }
    let mut required = separator
        .len()
        .checked_mul(items.len().saturating_sub(1))
        .ok_or(LineError::JoinedOutputTooLarge)?;
    for item in items {
        if item.len() > JOIN_MAX_ITEM_BYTES {
            return Err(LineError::ItemTooLarge);
        }
        required = required
            .checked_add(item.len())
            .ok_or(LineError::JoinedOutputTooLarge)?;
    }
    if required > JOIN_MAX_OUTPUT_BYTES {
        return Err(LineError::JoinedOutputTooLarge);
    }
    if output.len() < required {
        return Err(LineError::OutputTooSmall);
    }
    let mut cursor = 0;
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            let end = cursor + separator.len();
            output[cursor..end].copy_from_slice(separator.as_bytes());
            cursor = end;
        }
        let end = cursor + item.len();
        output[cursor..end].copy_from_slice(item.as_bytes());
        cursor = end;
    }
    Ok(cursor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{borrow::ToOwned, string::String, vec, vec::Vec};

    fn split(chunks: &[&[u8]]) -> Result<Vec<String>, LineError> {
        let mut state = LinesState::new();
        let mut lines = Vec::new();
        for chunk in chunks {
            for byte in *chunk {
                if state.push_byte(*byte)? {
                    let mut output = [0; LINES_MAX_LINE_BYTES];
                    let length = state.take_ready(&mut output)?.unwrap();
                    lines.push(core::str::from_utf8(&output[..length]).unwrap().to_owned());
                }
            }
        }
        if state.finish()? {
            let mut output = [0; LINES_MAX_LINE_BYTES];
            let length = state.take_ready(&mut output)?.unwrap();
            lines.push(core::str::from_utf8(&output[..length]).unwrap().to_owned());
        }
        Ok(lines)
    }

    #[test]
    fn lines_ignore_chunk_boundaries_and_normalize_crlf() {
        let expected = vec!["alpha".to_owned(), "".to_owned(), "omega".to_owned()];
        assert_eq!(split(&[b"alpha\r\n\nomega"]).unwrap(), expected);
        assert_eq!(
            split(&[b"al", b"pha\r", b"\n", b"\n", b"ome", b"ga"]).unwrap(),
            expected
        );
    }

    #[test]
    fn lines_reject_invalid_utf8_and_overflow() {
        assert_eq!(split(&[&[0xff, b'\n']]), Err(LineError::InvalidUtf8));
        assert_eq!(split(&[&[0xe2, 0x82], b"\n"]), Err(LineError::InvalidUtf8));
        assert_eq!(
            split(&[&[0xed, 0xa0, 0x80, b'\n']]),
            Err(LineError::InvalidUtf8)
        );
        assert_eq!(
            split(&[&[0xf4, 0x90, 0x80, 0x80, b'\n']]),
            Err(LineError::InvalidUtf8)
        );
        let oversized = [b'x'; LINES_MAX_LINE_BYTES + 1];
        assert_eq!(split(&[&oversized]), Err(LineError::RetainedPrefixOverflow));
    }

    #[test]
    fn cancellation_discards_a_retained_partial_line() {
        let mut state = LinesState::new();
        for byte in b"partial" {
            assert!(!state.push_byte(*byte).unwrap());
        }
        state.cancel();
        let mut output = [0; LINES_MAX_LINE_BYTES];
        assert_eq!(state.take_ready(&mut output), Ok(None));
        assert_eq!(state.finish(), Ok(false));
    }

    #[test]
    fn join_is_finite_and_bounded() {
        let mut output = [0; JOIN_MAX_OUTPUT_BYTES];
        let length = join_text_into(&["one", "two", "three"], " / ", &mut output).unwrap();
        assert_eq!(&output[..length], b"one / two / three");
        assert_eq!(join_text_into(&[], ",", &mut output), Ok(0));
        assert_eq!(
            join_text_into(&["x"; JOIN_MAX_ITEMS + 1], ",", &mut output),
            Err(LineError::TooManyItems)
        );
        let item = "x".repeat(JOIN_MAX_ITEM_BYTES);
        let final_item = "x".repeat(JOIN_MAX_ITEM_BYTES - 3);
        assert_eq!(
            join_text_into(&[&item, &item, &item, &final_item], "|", &mut output),
            Ok(JOIN_MAX_OUTPUT_BYTES)
        );
        let oversized_final = "x".repeat(JOIN_MAX_ITEM_BYTES - 2);
        assert_eq!(
            join_text_into(&[&item, &item, &item, &oversized_final], "|", &mut output),
            Err(LineError::JoinedOutputTooLarge)
        );
    }
}
