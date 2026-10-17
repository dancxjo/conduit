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

pub struct RetainedText {
    mode: TextStateMode,
    maximum: usize,
    length: usize,
    pending_clear: bool,
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
        })
    }

    pub fn apply<'a>(
        &mut self,
        storage: &'a mut [u8],
        fragment: &[u8],
    ) -> Result<Option<&'a [u8]>, TextStateRefusal> {
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
