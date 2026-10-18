//! Retained editing over caller-owned, pre-admitted storage; no allocator.

pub const MAXIMUM_EDITED_TEXT_BYTES: u32 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStateMode {
    Edit,
    Submit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStateRefusal {
    InvalidCapacity,
    InvalidUtf8,
    CapacityExhausted,
}

/// One committed replacement; coordinates count Unicode scalars, never bytes or UTF-16 units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextEdit<'a> {
    pub base_revision: u64,
    pub start: u32,
    pub end: u32,
    pub insert_text: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEditRefusal {
    StaleRevision,
    InvalidEncoding,
    InvalidRange,
    InvalidUtf8,
    CapacityExhausted,
    RevisionExhausted,
    InvalidStorage,
    UnsupportedMode,
}

pub struct RetainedText {
    mode: TextStateMode,
    maximum: usize,
    length: usize,
    pending_clear: bool,
    revision: u64,
}

impl RetainedText {
    pub fn new(mode: TextStateMode, maximum: usize) -> Result<Self, TextStateRefusal> {
        if maximum == 0 || maximum > MAXIMUM_EDITED_TEXT_BYTES as usize {
            return Err(TextStateRefusal::InvalidCapacity);
        }
        Ok(Self {
            mode,
            maximum,
            length: 0,
            pending_clear: false,
            revision: 0,
        })
    }

    pub fn current<'a>(&self, storage: &'a [u8]) -> Result<&'a [u8], TextEditRefusal> {
        let storage = storage
            .get(..self.maximum)
            .ok_or(TextEditRefusal::InvalidStorage)?;
        let text = &storage[..self.length];
        core::str::from_utf8(text).map_err(|_| TextEditRefusal::InvalidUtf8)?;
        Ok(text)
    }

    /// Revision of the authoritative document, shared by both input ramps.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Replace Unicode scalar offsets atomically in caller-admitted storage.
    /// All checks precede mutation; navigation and grapheme policy belong to input.
    pub fn replace<'a>(
        &mut self,
        storage: &'a mut [u8],
        edit: TextEdit<'_>,
    ) -> Result<&'a [u8], TextEditRefusal> {
        if self.mode != TextStateMode::Edit {
            return Err(TextEditRefusal::UnsupportedMode);
        }
        if edit.base_revision != self.revision {
            return Err(TextEditRefusal::StaleRevision);
        }
        let next_revision = self
            .revision
            .checked_add(1)
            .ok_or(TextEditRefusal::RevisionExhausted)?;
        let text = storage
            .get_mut(..self.maximum)
            .ok_or(TextEditRefusal::InvalidStorage)?;
        let current =
            core::str::from_utf8(&text[..self.length]).map_err(|_| TextEditRefusal::InvalidUtf8)?;
        core::str::from_utf8(edit.insert_text).map_err(|_| TextEditRefusal::InvalidUtf8)?;
        let offset = |scalar: u32| {
            current
                .char_indices()
                .map(|(byte, _)| byte)
                .chain(core::iter::once(current.len()))
                .nth(scalar as usize)
        };
        if edit.start > edit.end {
            return Err(TextEditRefusal::InvalidRange);
        }
        let start = offset(edit.start).ok_or(TextEditRefusal::InvalidRange)?;
        let end = offset(edit.end).ok_or(TextEditRefusal::InvalidRange)?;
        let next_length = (self.length - (end - start))
            .checked_add(edit.insert_text.len())
            .filter(|length| *length <= self.maximum)
            .ok_or(TextEditRefusal::CapacityExhausted)?;
        text.copy_within(end..self.length, start + edit.insert_text.len());
        text[start..start + edit.insert_text.len()].copy_from_slice(edit.insert_text);
        self.length = next_length;
        self.revision = next_revision;
        Ok(&text[..self.length])
    }

    pub fn apply<'a>(
        &mut self,
        storage: &'a mut [u8],
        fragment: &[u8],
    ) -> Result<Option<&'a [u8]>, TextStateRefusal> {
        if self.mode == TextStateMode::Edit {
            let admitted = storage
                .get(..self.maximum)
                .ok_or(TextStateRefusal::InvalidCapacity)?;
            let current = admitted
                .get(..self.length)
                .and_then(|bytes| core::str::from_utf8(bytes).ok())
                .ok_or(TextStateRefusal::InvalidUtf8)?;
            let count = current.chars().count() as u32;
            if fragment == b"\n" {
                return Ok(Some(&storage[..self.length]));
            }
            let (start, insert_text) = if fragment == b"\x08" {
                (count.saturating_sub(1), &b""[..])
            } else {
                (count, fragment)
            };
            return self
                .replace(
                    storage,
                    TextEdit {
                        base_revision: self.revision,
                        start,
                        end: count,
                        insert_text,
                    },
                )
                .map(Some)
                .map_err(|error| match error {
                    TextEditRefusal::CapacityExhausted | TextEditRefusal::RevisionExhausted => {
                        TextStateRefusal::CapacityExhausted
                    }
                    TextEditRefusal::InvalidUtf8 => TextStateRefusal::InvalidUtf8,
                    _ => TextStateRefusal::InvalidCapacity,
                });
        }
        let text = storage
            .get_mut(..self.maximum)
            .ok_or(TextStateRefusal::InvalidCapacity)?;
        if self.pending_clear {
            self.length = 0;
            self.pending_clear = false;
        }
        let fragment = core::str::from_utf8(fragment).map_err(|_| TextStateRefusal::InvalidUtf8)?;
        if fragment == "\n" {
            return match self.mode {
                TextStateMode::Edit => Ok(Some(&text[..self.length])),
                TextStateMode::Submit if self.length == 0 => Ok(None),
                TextStateMode::Submit => {
                    self.pending_clear = true;
                    Ok(Some(&text[..self.length]))
                }
            };
        }
        if fragment == "\u{8}" {
            if let Some((index, _)) = core::str::from_utf8(&text[..self.length])
                .ok()
                .and_then(|text| text.char_indices().next_back())
            {
                self.length = index;
            }
            return Ok(matches!(self.mode, TextStateMode::Edit).then_some(&text[..self.length]));
        }
        let next = self
            .length
            .checked_add(fragment.len())
            .filter(|length| *length <= self.maximum)
            .ok_or(TextStateRefusal::CapacityExhausted)?;
        text[self.length..next].copy_from_slice(fragment.as_bytes());
        self.length = next;
        Ok(matches!(self.mode, TextStateMode::Edit).then_some(&text[..self.length]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_deletion_capacity_and_refusal_preserve_retained_text() {
        let mut state = RetainedText::new(TextStateMode::Edit, 4).unwrap();
        let mut bytes = [0; 4];
        assert_eq!(
            state.apply(&mut bytes, "é".as_bytes()).unwrap(),
            Some("é".as_bytes())
        );
        assert_eq!(
            state.apply(&mut bytes, "ß".as_bytes()).unwrap(),
            Some("éß".as_bytes())
        );
        assert_eq!(
            state.apply(&mut bytes, b"x"),
            Err(TextStateRefusal::CapacityExhausted)
        );
        assert_eq!(
            state.apply(&mut bytes, &[0xff]),
            Err(TextStateRefusal::InvalidUtf8)
        );
        assert_eq!(
            state.apply(&mut bytes, b"\n").unwrap(),
            Some("éß".as_bytes())
        );
        assert_eq!(
            state.apply(&mut bytes, b"\x08").unwrap(),
            Some("é".as_bytes())
        );
        assert_eq!(state.apply(&mut bytes, b"\x08").unwrap(), Some(&b""[..]));
    }

    #[test]
    fn exhausted_revision_refuses_before_touching_storage() {
        let mut state = RetainedText::new(TextStateMode::Edit, 4).unwrap();
        let mut bytes = [0; 4];
        state.apply(&mut bytes, b"ab").unwrap();
        state.revision = u64::MAX;
        let before = bytes;
        assert_eq!(
            state.replace(
                &mut bytes,
                TextEdit {
                    base_revision: u64::MAX,
                    start: 0,
                    end: 2,
                    insert_text: b"x",
                }
            ),
            Err(TextEditRefusal::RevisionExhausted)
        );
        assert_eq!(bytes, before);
        assert_eq!(state.revision(), u64::MAX);
    }

    #[test]
    fn submission_clears_on_next_input_and_instances_are_private() {
        let mut state = RetainedText::new(TextStateMode::Submit, 4).unwrap();
        let mut sibling = RetainedText::new(TextStateMode::Edit, 4).unwrap();
        let mut bytes = [0; 4];
        let mut other = [0; 4];
        assert_eq!(state.apply(&mut bytes, b"ab").unwrap(), None);
        assert_eq!(state.apply(&mut bytes, b"\n").unwrap(), Some(&b"ab"[..]));
        assert_eq!(state.apply(&mut bytes, b"c").unwrap(), None);
        assert_eq!(state.apply(&mut bytes, b"\n").unwrap(), Some(&b"c"[..]));
        assert_eq!(sibling.apply(&mut other, b"z").unwrap(), Some(&b"z"[..]));
        assert_eq!(state.apply(&mut bytes, b"\n").unwrap(), None);
        assert_eq!(sibling.apply(&mut other, b"\n").unwrap(), Some(&b"z"[..]));
        assert_eq!(
            sibling.apply(&mut [], b"x"),
            Err(TextStateRefusal::InvalidCapacity)
        );
    }
}
