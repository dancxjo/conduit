//! Preserve semantic quantity recognition and selected-profile refusals during
//! startup resolution. Numeric eligibility must not fall back to opaque text.

use crate::SyntaxCheckError;
use conduit_core::{Quantity, QuantityLiteralRefusal};

pub(crate) fn startup_quantity(text: &str) -> Result<Option<Quantity>, SyntaxCheckError> {
    match Quantity::parse_plot_literal(text) {
        Ok(value) => Ok(Some(value)),
        Err(QuantityLiteralRefusal::NonCanonicalUnit { canonical }) => {
            Err(SyntaxCheckError::QuantityLiteral(format!(
                "non-canonical quantity unit in '{text}'; use '{canonical}'"
            )))
        }
        Err(
            refusal @ (QuantityLiteralRefusal::RepresentationIneligible { .. }
            | QuantityLiteralRefusal::AmbiguousUnit),
        ) => Err(SyntaxCheckError::QuantityEligibility(
            format!("quantity literal '{text}' refused: {refusal:?}"),
            None,
        )),
        Err(_) => Ok(None),
    }
}

/// A wider representation is an explicit selected Type. Existing catalogs and
/// checked identities receive no extra declarations when it is unused.
pub(crate) fn selected_profile<'a>(
    catalog: &'a crate::StartupCatalog,
    source_type: &str,
) -> Option<alloc::borrow::Cow<'a, conduit_core::StructuredInfoType>> {
    if let Some(existing) = catalog.structured_type(source_type) {
        return Some(alloc::borrow::Cow::Borrowed(existing));
    }
    let kind = crate::value_type::canonical_value_kind(source_type);
    if kind.as_str() != conduit_core::EXACT_DECIMAL_QUANTITY_INFO_ID {
        return None;
    }
    conduit_core::StructuredInfoType::leaf(kind)
        .ok()
        .map(alloc::borrow::Cow::Owned)
}

pub(crate) fn is_exact_profile(expected: &conduit_core::StructuredInfoType) -> bool {
    matches!(expected.shape(), conduit_core::StructuredInfoTypeShape::Leaf(kind)
        if kind.as_str() == conduit_core::EXACT_DECIMAL_QUANTITY_INFO_ID)
}

/// A slash is part of a quantity token only for a complete reviewed suffix
/// immediately following a decimal magnitude, without intervening whitespace.
/// Ordinary division and unknown suffixes keep the existing expression grammar.
pub(crate) fn compound_token_length(tail: &str) -> Option<usize> {
    if !tail.starts_with(|character: char| character.is_ascii_digit()) {
        return None;
    }
    let end = tail
        .char_indices()
        .find_map(|(index, character)| {
            (character.is_whitespace()
                || matches!(
                    character,
                    '(' | ')'
                        | '['
                        | ']'
                        | '{'
                        | '}'
                        | ','
                        | ':'
                        | '?'
                        | '!'
                        | '+'
                        | '-'
                        | '*'
                        | '%'
                        | '<'
                        | '>'
                        | '='
                        | '&'
                        | '^'
                        | '|'
                ))
            .then_some(index)
        })
        .unwrap_or(tail.len());
    let candidate = &tail[..end];
    if !candidate.contains('/') {
        return None;
    }
    let suffix_start = candidate.char_indices().find_map(|(index, character)| {
        (!character.is_ascii_digit() && character != '.').then_some(index)
    })?;
    conduit_core::ResolvedQuantitySuffix::resolve(&candidate[suffix_start..]).ok()?;
    Some(end)
}
