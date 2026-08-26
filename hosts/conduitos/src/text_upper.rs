//! Allocation-free Unicode uppercase realization with an exact output bound.

pub const MAXIMUM_BYTES: usize = conduit_text::MAX_TEXT_BYTES as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UppercaseError {
    MalformedUtf8,
    OutputOverflow,
}

pub struct UppercaseText {
    bytes: [u8; MAXIMUM_BYTES],
    len: usize,
}

impl UppercaseText {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

pub fn uppercase(input: &[u8]) -> Result<UppercaseText, UppercaseError> {
    let text = core::str::from_utf8(input).map_err(|_| UppercaseError::MalformedUtf8)?;
    let mut output = UppercaseText {
        bytes: [0; MAXIMUM_BYTES],
        len: 0,
    };
    for character in text.chars().flat_map(char::to_uppercase) {
        let mut encoded = [0_u8; 4];
        let bytes = character.encode_utf8(&mut encoded).as_bytes();
        let end = output
            .len
            .checked_add(bytes.len())
            .filter(|end| *end <= MAXIMUM_BYTES)
            .ok_or(UppercaseError::OutputOverflow)?;
        output.bytes[output.len..end].copy_from_slice(bytes);
        output.len = end;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_expansion_has_an_independent_output_length() {
        let output = uppercase("ǰ".as_bytes()).unwrap();
        assert_eq!(core::str::from_utf8(output.as_bytes()).unwrap(), "J\u{30c}");
        assert!(output.as_bytes().len() > "ǰ".len());
    }

    #[test]
    fn malformed_and_expanding_overflow_are_distinct_and_never_truncated() {
        assert_eq!(
            uppercase(&[0xff]).err(),
            Some(UppercaseError::MalformedUtf8)
        );
        let mut input = [0_u8; MAXIMUM_BYTES];
        for chunk in input.as_chunks_mut::<2>().0 {
            chunk.copy_from_slice("ǰ".as_bytes());
        }
        assert_eq!(
            uppercase(&input).err(),
            Some(UppercaseError::OutputOverflow)
        );
    }
}
