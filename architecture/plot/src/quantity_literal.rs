//! Preserve semantic quantity recognition and selected-profile refusals during
//! startup resolution. Numeric eligibility must not fall back to opaque text.

use crate::SyntaxCheckError;

pub(crate) fn startup_quantity(
    text: &str,
) -> Result<Option<conduit_core::QuantityConfigurationValue>, SyntaxCheckError> {
    match conduit_core::QuantityConfigurationValue::parse(text) {
        Ok(value) => Ok(Some(value)),
        Err(_) => Ok(None),
    }
}

/// Resolve the canonical physical Types without an import.
pub(crate) fn selected_profile<'a>(
    catalog: &'a crate::StartupCatalog,
    source_type: &str,
) -> Option<alloc::borrow::Cow<'a, conduit_core::StructuredInfoType>> {
    if let Some(existing) = catalog.structured_type(source_type) {
        return Some(alloc::borrow::Cow::Borrowed(existing));
    }
    let kind = crate::value_type::canonical_value_kind(source_type);
    if !matches!(
        kind.as_str(),
        conduit_core::QUANTITY_INFO_ID | conduit_core::UNIT_INFO_ID
    ) && conduit_core::quantity_info_dimension(kind.as_str()).is_none()
    {
        return None;
    }
    conduit_core::StructuredInfoType::leaf(kind)
        .ok()
        .map(alloc::borrow::Cow::Owned)
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

/// Punctuation inside a complete reviewed Unit token belongs to that value.
/// Numeric remainder and unreviewed divisions retain their ordinary grammar.
pub(crate) fn unit_token_length(tail: &str) -> Option<usize> {
    if tail.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let end = tail
        .char_indices()
        .find_map(|(i, c)| {
            (c.is_whitespace()
                || matches!(
                    c,
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
                        | '<'
                        | '>'
                        | '='
                        | '&'
                        | '^'
                        | '|'
                ))
            .then_some(i)
        })
        .unwrap_or(tail.len());
    let candidate = &tail[..end];
    if !candidate.contains('/') && candidate != "%" {
        return None;
    }
    conduit_core::Unit::resolve(candidate).ok()?;
    Some(end)
}
