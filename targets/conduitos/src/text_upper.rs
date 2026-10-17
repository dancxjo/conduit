//! Allocation-free Unicode uppercase realization with an exact output bound.

pub const MAXIMUM_BYTES: usize = conduit_text::MAX_TEXT_BYTES as usize;

pub use crate::text_transform::UppercaseError;

pub struct UppercaseText {
    pub(crate) bytes: [u8; MAXIMUM_BYTES],
    pub(crate) len: usize,
}

impl UppercaseText {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

pub fn uppercase(input: &[u8]) -> Result<UppercaseText, UppercaseError> {
    let mut output = UppercaseText {
        bytes: [0; MAXIMUM_BYTES],
        len: 0,
    };
    output.len = crate::text_transform::uppercase_into(input, &mut output.bytes)?;
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
