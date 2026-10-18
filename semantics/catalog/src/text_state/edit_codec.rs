//! Canonical bounded forms for committed edits and authoritative documents.
use super::{TextEdit, TextEditRefusal, MAXIMUM_EDITED_TEXT_BYTES};

pub const TEXT_EDIT_VALUE_KIND: &str = "text/edit@1";
pub const TEXT_DOCUMENT_VALUE_KIND: &str = "text/document@1";
pub const MAXIMUM_TEXT_EDIT_BYTES: u32 = 16 + MAXIMUM_EDITED_TEXT_BYTES;
pub const MAXIMUM_TEXT_DOCUMENT_BYTES: u32 = 8 + MAXIMUM_EDITED_TEXT_BYTES;

/// A borrowed view of one committed document. IME preedit is never a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextDocument<'a> {
    pub revision: u64,
    pub text: &'a [u8],
}

fn validate_text(text: &[u8]) -> Result<(), TextEditRefusal> {
    if text.len() > MAXIMUM_EDITED_TEXT_BYTES as usize {
        return Err(TextEditRefusal::CapacityExhausted);
    }
    core::str::from_utf8(text).map_err(|_| TextEditRefusal::InvalidUtf8)?;
    Ok(())
}

impl<'a> TextEdit<'a> {
    /// Little-endian revision/start/end followed by the exact inserted UTF-8 bytes.
    pub fn decode(bytes: &'a [u8]) -> Result<Self, TextEditRefusal> {
        let header = bytes.get(..16).ok_or(TextEditRefusal::InvalidEncoding)?;
        let edit = Self {
            base_revision: u64::from_le_bytes(header[..8].try_into().unwrap()),
            start: u32::from_le_bytes(header[8..12].try_into().unwrap()),
            end: u32::from_le_bytes(header[12..16].try_into().unwrap()),
            insert_text: &bytes[16..],
        };
        edit.validate()?;
        Ok(edit)
    }

    fn validate(&self) -> Result<(), TextEditRefusal> {
        if self.start > self.end || self.end > MAXIMUM_EDITED_TEXT_BYTES {
            return Err(TextEditRefusal::InvalidRange);
        }
        validate_text(self.insert_text)
    }

    pub fn encode_into(&self, output: &mut [u8]) -> Result<usize, TextEditRefusal> {
        self.validate()?;
        let length = 16 + self.insert_text.len();
        let output = output
            .get_mut(..length)
            .ok_or(TextEditRefusal::InvalidStorage)?;
        output[..8].copy_from_slice(&self.base_revision.to_le_bytes());
        output[8..12].copy_from_slice(&self.start.to_le_bytes());
        output[12..16].copy_from_slice(&self.end.to_le_bytes());
        output[16..].copy_from_slice(self.insert_text);
        Ok(length)
    }
}

impl<'a> TextDocument<'a> {
    /// Little-endian revision followed by the exact authoritative UTF-8 bytes.
    pub fn decode(bytes: &'a [u8]) -> Result<Self, TextEditRefusal> {
        let header = bytes.get(..8).ok_or(TextEditRefusal::InvalidEncoding)?;
        let document = Self {
            revision: u64::from_le_bytes(header.try_into().unwrap()),
            text: &bytes[8..],
        };
        validate_text(document.text)?;
        Ok(document)
    }

    pub fn encode_into(&self, output: &mut [u8]) -> Result<usize, TextEditRefusal> {
        validate_text(self.text)?;
        let length = 8 + self.text.len();
        let output = output
            .get_mut(..length)
            .ok_or(TextEditRefusal::InvalidStorage)?;
        output[..8].copy_from_slice(&self.revision.to_le_bytes());
        output[8..].copy_from_slice(self.text);
        Ok(length)
    }
}
