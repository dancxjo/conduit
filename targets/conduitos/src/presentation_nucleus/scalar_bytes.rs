//! Finite canonical Scalar transport for selected 128-byte host profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ScalarBytes {
    bytes: [u8; 128],
    length: usize,
}
impl ScalarBytes {
    pub(super) fn new(value: &[u8]) -> Result<Self, ()> {
        if value.len() > 128 {
            return Err(());
        }
        let admitted = conduit_core::MathValueRef::validate(value).map_err(|_| ())?;
        let mut profile = conduit_core::PortableNumericProfile::MAXIMAL;
        profile.maximum_encoded_bytes = 128;
        profile.validate(admitted.encoded()).map_err(|_| ())?;
        let mut bytes = [0; 128];
        bytes[..value.len()].copy_from_slice(value);
        Ok(Self {
            bytes,
            length: value.len(),
        })
    }
    pub(super) fn encoded(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}
impl AsRef<[u8]> for ScalarBytes {
    fn as_ref(&self) -> &[u8] {
        self.encoded()
    }
}

pub(super) fn digest_hex(digest: [u8; 32]) -> alloc::string::String {
    use core::fmt::Write;
    let mut text = alloc::string::String::with_capacity(64);
    for byte in digest {
        write!(text, "{byte:02x}").expect("String write");
    }
    text
}
