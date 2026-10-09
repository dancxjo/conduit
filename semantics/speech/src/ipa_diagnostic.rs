//! Exact half-open UTF-8 byte and Unicode scalar locations in authored IPA.
use crate::ipa_notation::IpaNotationRefusal;

/// Coordinates are relative to the exact supplied source, including any display
/// delimiters. A scalar span is a source location, never a phone ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IpaSourceSpan {
    pub byte_start: usize,
    pub byte_end: usize,
    pub scalar_start: usize,
    pub scalar_end: usize,
}
impl IpaSourceSpan {
    pub fn from_bytes(source: &str, start: usize, end: usize) -> Option<Self> {
        if start > end
            || end > source.len()
            || !source.is_char_boundary(start)
            || !source.is_char_boundary(end)
        {
            return None;
        }
        Some(Self {
            byte_start: start,
            byte_end: end,
            scalar_start: source[..start].chars().count(),
            scalar_end: source[..end].chars().count(),
        })
    }
}

/// A refusal and the exact source region to underline. No normalization occurs
/// before these coordinates are established.
#[derive(Debug)]
pub struct IpaNotationDiagnostic {
    pub refusal: IpaNotationRefusal,
    pub span: IpaSourceSpan,
}
