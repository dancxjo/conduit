//! Exact source material for linguistic occurrences; source and analysis revisions differ.
use crate::*;
use conduit_core::{ConfigurationEntry, ConfigurationValue, KindId, StructuredConfigurationValue};
use conduit_plot::rust_binding::NativeRustBinding;

pub fn language_text_profile() -> KindId {
    LanguageText::semantic_type()
        .expect("checked Language Type")
        .profile()
        .expect("finite source profile")
        .value_kind()
        .clone()
}

pub fn language_material_configuration(
    source: LanguageText,
) -> Result<ConfigurationValue, LinguisticRefusal> {
    StructuredConfigurationValue::new(language_text_profile(), source.encode()?)
        .map(ConfigurationValue::Structured)
        .ok_or(LinguisticRefusal::MalformedInfo)
}

pub fn language_material_request(source: &LanguageText) -> LanguageRequest {
    LanguageRequest::new(
        source.language().clone(),
        None,
        LanguageVarietyPolicy::LanguageSufficient,
    )
    .expect("validated native source identity")
}

pub fn configured_language_material(
    configuration: &[ConfigurationEntry],
) -> Result<LanguageText, LinguisticRefusal> {
    let Some(ConfigurationEntry {
        value: ConfigurationValue::Structured(value),
        ..
    }) = configuration.iter().find(|entry| entry.key == "material")
    else {
        return Err(LinguisticRefusal::MalformedInfo);
    };
    if value.profile() != &language_text_profile() {
        return Err(LinguisticRefusal::MalformedInfo);
    }
    Ok(LanguageText::decode(value.canonical_value())?)
}

/// Convert an explicit span basis to the canonical Unicode-scalar occurrence reference.
/// No registry, language inference or source-revision substitution participates.
pub fn language_source_occurrence(
    source: &LanguageText,
    span: &TextSpan,
    kind: LanguageTextSegmentKind,
) -> Result<LanguageTextSegmentRef, LinguisticRefusal> {
    if span.text_identity() != source.identity() {
        return Err(LinguisticRefusal::SourceIdentity);
    }
    if span.text_revision() != source.revision() {
        return Err(LinguisticRefusal::SourceRevision);
    }
    let text = source.text().as_str();
    let (start, end) = match span.basis() {
        LinguisticOffsetBasis::UnicodeScalar => (*span.start(), *span.end()),
        LinguisticOffsetBasis::Utf8Byte => {
            let start =
                usize::try_from(*span.start()).map_err(|_| LinguisticRefusal::SourceRange)?;
            let end = usize::try_from(*span.end()).map_err(|_| LinguisticRefusal::SourceRange)?;
            if end > text.len() || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
                return Err(LinguisticRefusal::SourceRange);
            }
            (
                text[..start].chars().count() as u64,
                text[..end].chars().count() as u64,
            )
        }
    };
    let scalar_count = text.chars().count() as u32;
    let start = u32::try_from(start).map_err(|_| LinguisticRefusal::SourceRange)?;
    let end = u32::try_from(end).map_err(|_| LinguisticRefusal::SourceRange)?;
    if start > end || end > scalar_count {
        return Err(LinguisticRefusal::SourceRange);
    }
    let reference = LanguageTextSegmentRef::new(
        kind,
        source.language().clone(),
        LanguageTextRange::new(end, start)?,
        source.revision().clone(),
        source.identity().clone(),
    )?;
    LanguageTextReferenceMatch::new(source.clone(), reference.clone(), scalar_count)?;
    Ok(reference)
}

pub fn validate_linguistic_source(tokens: &LinguisticTokensFour) -> Result<(), LinguisticRefusal> {
    let source = tokens.source();
    for segment in tokens.segments() {
        language_source_occurrence(source, segment.span(), LanguageTextSegmentKind::Utterance)?;
    }
    for (ordinal, token) in tokens.tokens().iter().enumerate() {
        if token.identity().text_identity() != source.identity() {
            return Err(LinguisticRefusal::SourceIdentity);
        }
        if token.identity().text_revision() != source.revision() {
            return Err(LinguisticRefusal::SourceRevision);
        }
        if *token.identity().ordinal() != ordinal as u64 {
            return Err(LinguisticRefusal::MalformedInfo);
        }
        let reference =
            language_source_occurrence(source, token.span(), LanguageTextSegmentKind::Word)?;
        let start = *reference.range().start() as usize;
        let end = *reference.range().end() as usize;
        let text = source.text().as_str();
        let boundary = |scalar| {
            text.char_indices()
                .nth(scalar)
                .map_or(text.len(), |(byte, _)| byte)
        };
        if &text[boundary(start)..boundary(end)] != token.surface() {
            return Err(LinguisticRefusal::SourceSurface);
        }
    }
    Ok(())
}

pub fn validate_linguistic_request(
    request: &LanguageRequest,
    tokens: &LinguisticTokensFour,
) -> Result<(), LinguisticRefusal> {
    validate_linguistic_source(tokens)?;
    if request.language() != tokens.source().language() {
        return Err(LinguisticRefusal::SourceLanguage);
    }
    Ok(())
}

pub fn configured_language_request(
    configuration: &[ConfigurationEntry],
) -> Result<LanguageRequest, LinguisticRefusal> {
    let Some(ConfigurationEntry {
        value: ConfigurationValue::Structured(value),
        ..
    }) = configuration
        .iter()
        .find(|entry| entry.key == "language-request")
    else {
        return Err(LinguisticRefusal::MalformedInfo);
    };
    if value.profile() != &language_request_profile() {
        return Err(LinguisticRefusal::MalformedInfo);
    }
    Ok(LanguageRequest::decode(value.canonical_value())?)
}
