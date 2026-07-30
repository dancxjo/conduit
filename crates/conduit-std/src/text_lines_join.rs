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
/// complete logical line, so a code point may cross input chunk boundaries.
pub struct LinesState {
    retained: [u8; LINES_MAX_RETAINED_PREFIX_BYTES],
    retained_len: usize,
    ready_len: Option<usize>,
    finished: bool,
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
        }
    }

    /// Accepts one byte. Call [`Self::take_ready`] before accepting another
    /// byte after this returns `true`.
    pub fn push_byte(&mut self, byte: u8) -> Result<bool, LineError> {
        if self.ready_len.is_some() || self.finished {
            return Err(LineError::OutputTooSmall);
        }
        if byte == b'\n' {
            let length = if self.retained_len > 0 && self.retained[self.retained_len - 1] == b'\r' {
                self.retained_len - 1
            } else {
                self.retained_len
            };
            core::str::from_utf8(&self.retained[..length]).map_err(|_| LineError::InvalidUtf8)?;
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
        if self.retained_len == 0 {
            return Ok(false);
        }
        core::str::from_utf8(&self.retained[..self.retained_len])
            .map_err(|_| LineError::InvalidUtf8)?;
        self.ready_len = Some(self.retained_len);
        Ok(true)
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
        self.retained_len = 0;
        self.ready_len = None;
        Ok(Some(length))
    }

    /// Cancellation discards the finite retained prefix.
    pub fn cancel(&mut self) {
        self.retained_len = 0;
        self.ready_len = None;
        self.finished = true;
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
