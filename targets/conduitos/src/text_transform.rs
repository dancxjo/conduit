//! Bounded pure Unicode transformation shared with the unprivileged image.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UppercaseError {
    MalformedUtf8,
    OutputOverflow,
}

pub fn uppercase_into(input: &[u8], output: &mut [u8]) -> Result<usize, UppercaseError> {
    let text = core::str::from_utf8(input).map_err(|_| UppercaseError::MalformedUtf8)?;
    let mut len = 0usize;
    for character in text.chars().flat_map(char::to_uppercase) {
        let mut encoded = [0_u8; 4];
        let bytes = character.encode_utf8(&mut encoded).as_bytes();
        let end = len
            .checked_add(bytes.len())
            .filter(|end| *end <= output.len())
            .ok_or(UppercaseError::OutputOverflow)?;
        output[len..end].copy_from_slice(bytes);
        len = end;
    }
    Ok(len)
}
