//! Source-owned metadata for a finite shipped glyph-notation family.
use super::{Expression, SpannedText};
use crate::Span;

/// The language recognizes one declaration shape. Owner contract resolution
/// and finite metadata validation happen during checked package installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphNotationSyntax {
    pub name: SpannedText,
    pub metadata: Expression,
    pub span: Span,
}
