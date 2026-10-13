//! Bounded tokenizer and annotation realizations for portable linguistic Info.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use conduit_core::StructuredInfoValue;
use conduit_plot::rust_binding::NativeRustBinding;

use crate::*;

struct Lexeme {
    surface: String,
    start: u64,
    end: u64,
    word: bool,
}

pub fn tokenize_four(source: &LanguageText) -> Result<StructuredInfoValue, LinguisticRefusal> {
    let text = source.text().as_str();
    if text.len() > MAXIMUM_LINGUISTIC_TEXT_BYTES as usize {
        return Err(LinguisticRefusal::TextTooLarge);
    }
    let mut lexemes = Vec::new();
    let mut word_start: Option<(usize, u64)> = None;
    let mut scalar = 0_u64;
    for (byte, character) in text.char_indices() {
        if character.is_alphabetic() {
            word_start.get_or_insert((byte, scalar));
        } else {
            if let Some((start_byte, start_scalar)) = word_start.take() {
                push_lexeme(
                    &mut lexemes,
                    &text[start_byte..byte],
                    start_scalar,
                    scalar,
                    true,
                )?;
            }
            if !character.is_whitespace() {
                push_lexeme(
                    &mut lexemes,
                    &text[byte..byte + character.len_utf8()],
                    scalar,
                    scalar + 1,
                    false,
                )?;
            }
        }
        scalar += 1;
    }
    if let Some((start_byte, start_scalar)) = word_start {
        push_lexeme(
            &mut lexemes,
            &text[start_byte..],
            start_scalar,
            scalar,
            true,
        )?;
    }
    if lexemes.len() != usize::from(LINGUISTIC_TOKEN_COUNT) {
        return Err(LinguisticRefusal::WrongTokenCount {
            expected: LINGUISTIC_TOKEN_COUNT,
            actual: lexemes.len(),
        });
    }

    let tokens = lexemes
        .iter()
        .enumerate()
        .map(|(ordinal, lexeme)| token(source, ordinal as u64, lexeme))
        .collect::<Result<Vec<_>, _>>()?;
    let tokens: [LinguisticToken; 4] =
        tokens.try_into().map_err(|tokens: Vec<LinguisticToken>| {
            LinguisticRefusal::WrongTokenCount {
                expected: LINGUISTIC_TOKEN_COUNT,
                actual: tokens.len(),
            }
        })?;
    let segment = LinguisticSegment::new(
        "segment/0".to_string(),
        LinguisticSegmentKind::sentence(),
        span(source, 0, scalar)?,
    )?;
    Ok(LinguisticTokensFour::new(
        provenance(
            "deterministic_rule",
            "conduit/std-tokenizer",
            "unicode-scalar@1",
        )?,
        [segment],
        source.clone(),
        tokens,
    )?
    .into_structured()?)
}

fn push_lexeme(
    lexemes: &mut Vec<Lexeme>,
    surface: &str,
    start: u64,
    end: u64,
    word: bool,
) -> Result<(), LinguisticRefusal> {
    if lexemes.len() == usize::from(LINGUISTIC_TOKEN_COUNT) {
        return Err(LinguisticRefusal::WrongTokenCount {
            expected: LINGUISTIC_TOKEN_COUNT,
            actual: lexemes.len() + 1,
        });
    }
    lexemes.push(Lexeme {
        surface: surface.to_string(),
        start,
        end,
        word,
    });
    Ok(())
}

fn token(
    source: &LanguageText,
    ordinal: u64,
    lexeme: &Lexeme,
) -> Result<LinguisticToken, LinguisticRefusal> {
    Ok(LinguisticToken::new(
        if lexeme.word {
            LinguisticTokenCategory::word()
        } else {
            LinguisticTokenCategory::punctuation()
        },
        [
            LinguisticTokenFeatureSlot::unused(),
            LinguisticTokenFeatureSlot::unused(),
        ],
        token_identity(source, ordinal)?,
        LinguisticOptionalText::absent(),
        span(source, lexeme.start, lexeme.end)?,
        lexeme.surface.clone(),
    )?)
}

/// A deterministic hosted-library realization using Rust's Unicode character tables.
pub fn annotate_with_unicode_library(
    tokens: &StructuredInfoValue,
) -> Result<StructuredInfoValue, LinguisticRefusal> {
    annotation_bundle(
        tokens,
        provenance("library", "rust/core-char", "unicode-alphabetic@1")?,
    )
}

/// Deterministic fixture for proving that model evidence remains distinct and portable.
pub fn annotate_with_model_fixture(
    tokens: &StructuredInfoValue,
    model_identity: &str,
) -> Result<StructuredInfoValue, LinguisticRefusal> {
    annotation_bundle(
        tokens,
        provenance("model", model_identity, "fixture-output@1")?,
    )
}

fn annotation_bundle(
    tokens: &StructuredInfoValue,
    provenance: LinguisticDerivationProvenance,
) -> Result<StructuredInfoValue, LinguisticRefusal> {
    let tokens = LinguisticTokensFour::from_structured(tokens.clone())
        .map_err(|_| LinguisticRefusal::MalformedInfo)?;
    crate::validate_linguistic_source(&tokens)?;
    let annotations = tokens
        .tokens()
        .iter()
        .map(|token| {
            let label = if token.surface().chars().all(char::is_alphabetic) {
                "lexical-item"
            } else {
                "sentence-terminal"
            };
            Ok(LinguisticAnnotation::new(
                label.to_string(),
                token.span().clone(),
            )?)
        })
        .collect::<Result<Vec<_>, LinguisticRefusal>>()?;
    let annotations: [LinguisticAnnotation; 4] = annotations
        .try_into()
        .map_err(|_| LinguisticRefusal::MalformedInfo)?;
    let token_values = tokens.tokens();
    let dependencies = [
        dependency(
            token_values[0].identity().clone(),
            token_values[1].identity().clone(),
            LinguisticDependencyRelation::modifier(),
        )?,
        dependency(
            token_values[1].identity().clone(),
            token_values[2].identity().clone(),
            LinguisticDependencyRelation::subject(),
        )?,
        dependency(
            token_values[3].identity().clone(),
            token_values[2].identity().clone(),
            LinguisticDependencyRelation::punctuation(),
        )?,
    ];
    Ok(AnnotationBundleFour::new(
        annotations,
        dependencies,
        provenance,
        tokens.source().clone(),
    )?
    .into_structured()?)
}

fn dependency(
    dependent: LinguisticTokenIdentity,
    governor: LinguisticTokenIdentity,
    relation: LinguisticDependencyRelation,
) -> Result<LinguisticDependencyEdge, LinguisticRefusal> {
    Ok(LinguisticDependencyEdge::new(
        dependent, governor, relation,
    )?)
}

fn span(source: &LanguageText, start: u64, end: u64) -> Result<TextSpan, LinguisticRefusal> {
    Ok(TextSpan::new(
        LinguisticOffsetBasis::unicode_scalar(),
        end,
        start,
        source.identity().clone(),
        source.revision().clone(),
    )?)
}

fn token_identity(
    source: &LanguageText,
    ordinal: u64,
) -> Result<LinguisticTokenIdentity, LinguisticRefusal> {
    Ok(LinguisticTokenIdentity::new(
        ordinal,
        source.identity().clone(),
        source.revision().clone(),
    )?)
}

fn provenance(
    tag: &str,
    implementation: &str,
    revision: &str,
) -> Result<LinguisticDerivationProvenance, LinguisticRefusal> {
    let implementation = implementation.to_string();
    let revision = revision.to_string();
    Ok(match tag {
        "deterministic_rule" => {
            LinguisticDerivationProvenance::deterministic_rule(implementation, revision)?
        }
        "library" => LinguisticDerivationProvenance::library(implementation, revision)?,
        "model" => LinguisticDerivationProvenance::model(implementation, revision)?,
        _ => return Err(LinguisticRefusal::MalformedInfo),
    })
}
