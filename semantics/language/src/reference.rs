//! Bounded tokenizer and annotation realizations for portable linguistic Info.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoValue, StructuredInfoValueShape,
};

use crate::info::*;

struct Lexeme {
    surface: String,
    start: u64,
    end: u64,
    word: bool,
}

pub fn tokenize_four(
    text_identity: &str,
    text: &str,
) -> Result<StructuredInfoValue, LinguisticRefusal> {
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
        .map(|(ordinal, lexeme)| token_value(text_identity, ordinal as u64, lexeme))
        .collect::<Result<Vec<_>, _>>()?;
    let segment = record_value(
        linguistic_segment_type(),
        vec![
            ("identity", text_value("segment/0")),
            ("kind", unit_variant(segment_kind_type(), "sentence")?),
            ("span", span_value(text_identity, 0, scalar)?),
        ],
    )?;
    record_value(
        linguistic_tokens_four_type(),
        vec![
            (
                "provenance",
                provenance_value(
                    "deterministic_rule",
                    "conduit/std-tokenizer",
                    "unicode-scalar@1",
                )?,
            ),
            (
                "segments",
                collection_value(linguistic_segment_type(), vec![segment])?,
            ),
            ("tokens", collection_value(linguistic_token_type(), tokens)?),
        ],
    )
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

fn token_value(
    text_identity: &str,
    ordinal: u64,
    lexeme: &Lexeme,
) -> Result<StructuredInfoValue, LinguisticRefusal> {
    let features = (0..LINGUISTIC_FEATURE_SLOTS)
        .map(|_| unit_variant(feature_slot_type(), "unused"))
        .collect::<Result<Vec<_>, _>>()?;
    record_value(
        linguistic_token_type(),
        vec![
            (
                "category",
                unit_variant(
                    token_category_type(),
                    if lexeme.word { "word" } else { "punctuation" },
                )?,
            ),
            ("features", collection_value(feature_slot_type(), features)?),
            ("identity", token_identity_value(text_identity, ordinal)?),
            ("lemma", unit_variant(optional_text_type(), "absent")?),
            ("span", span_value(text_identity, lexeme.start, lexeme.end)?),
            ("surface", text_value(&lexeme.surface)),
        ],
    )
}

/// A deterministic hosted-library realization using Rust's Unicode character tables.
pub fn annotate_with_unicode_library(
    tokens: &StructuredInfoValue,
) -> Result<StructuredInfoValue, LinguisticRefusal> {
    annotation_bundle(
        tokens,
        provenance_value("library", "rust/core-char", "unicode-alphabetic@1")?,
    )
}

/// Deterministic fixture for proving that model evidence remains distinct and portable.
pub fn annotate_with_model_fixture(
    tokens: &StructuredInfoValue,
    model_identity: &str,
) -> Result<StructuredInfoValue, LinguisticRefusal> {
    annotation_bundle(
        tokens,
        provenance_value("model", model_identity, "fixture-output@1")?,
    )
}

fn annotation_bundle(
    tokens: &StructuredInfoValue,
    provenance: StructuredInfoValue,
) -> Result<StructuredInfoValue, LinguisticRefusal> {
    if tokens.value_type() != &linguistic_tokens_four_type() {
        return Err(LinguisticRefusal::MalformedInfo);
    }
    let token_values = collection_field(tokens, "tokens")?;
    let annotations = token_values
        .iter()
        .map(|token| {
            let surface = leaf_text(record_field(token, "surface")?)?;
            let label = if surface.chars().all(char::is_alphabetic) {
                "lexical-item"
            } else {
                "sentence-terminal"
            };
            record_value(
                linguistic_annotation_type(),
                vec![
                    ("label", leaf_value("value/text@1", label.as_bytes())),
                    ("span", record_field(token, "span")?.clone()),
                ],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let identity = |index| record_field(&token_values[index], "identity").cloned();
    let dependencies = vec![
        dependency_value(identity(0)?, identity(1)?, "modifier")?,
        dependency_value(identity(1)?, identity(2)?, "subject")?,
        dependency_value(identity(3)?, identity(2)?, "punctuation")?,
    ];
    record_value(
        annotation_bundle_four_type(),
        vec![
            (
                "annotations",
                collection_value(linguistic_annotation_type(), annotations)?,
            ),
            (
                "dependencies",
                collection_value(dependency_edge_type(), dependencies)?,
            ),
            ("provenance", provenance),
        ],
    )
}

fn dependency_value(
    dependent: StructuredInfoValue,
    governor: StructuredInfoValue,
    relation: &str,
) -> Result<StructuredInfoValue, LinguisticRefusal> {
    record_value(
        dependency_edge_type(),
        vec![
            ("dependent", dependent),
            ("governor", governor),
            (
                "relation",
                unit_variant(dependency_relation_type(), relation)?,
            ),
        ],
    )
}

fn span_value(
    text_identity: &str,
    start: u64,
    end: u64,
) -> Result<StructuredInfoValue, LinguisticRefusal> {
    record_value(
        text_span_type(),
        vec![
            (
                "basis",
                unit_variant(offset_basis_type(), "unicode_scalar")?,
            ),
            ("end", count_value(end)),
            ("start", count_value(start)),
            ("text_identity", text_value(text_identity)),
        ],
    )
}

fn token_identity_value(
    text_identity: &str,
    ordinal: u64,
) -> Result<StructuredInfoValue, LinguisticRefusal> {
    record_value(
        token_identity_type(),
        vec![
            ("ordinal", count_value(ordinal)),
            ("text_identity", text_value(text_identity)),
        ],
    )
}

fn provenance_value(
    tag: &str,
    implementation: &str,
    revision: &str,
) -> Result<StructuredInfoValue, LinguisticRefusal> {
    let value_type = provenance_type();
    let payload_type = variant_payload_type(&value_type, tag)?;
    let payload = record_value(
        payload_type,
        vec![
            ("implementation", text_value(implementation)),
            ("revision", text_value(revision)),
        ],
    )?;
    Ok(StructuredInfoValue::variant(value_type, tag, payload)?)
}

fn unit_variant(
    value_type: StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoValue, LinguisticRefusal> {
    Ok(StructuredInfoValue::variant(
        value_type,
        tag,
        leaf_value("value/unit@1", &[]),
    )?)
}

fn variant_payload_type(
    value_type: &StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoType, LinguisticRefusal> {
    let conduit_core::StructuredInfoTypeShape::Variant { cases, .. } = value_type.shape() else {
        return Err(LinguisticRefusal::MalformedInfo);
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .map(|case| case.payload_type().clone())
        .ok_or(LinguisticRefusal::MalformedInfo)
}

fn record_value(
    value_type: StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> Result<StructuredInfoValue, LinguisticRefusal> {
    Ok(StructuredInfoValue::record(
        value_type,
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value))
            .collect::<Result<Vec<_>, _>>()?,
    )?)
}

fn collection_value(
    element_type: StructuredInfoType,
    values: Vec<StructuredInfoValue>,
) -> Result<StructuredInfoValue, LinguisticRefusal> {
    let length = u16::try_from(values.len()).map_err(|_| LinguisticRefusal::MalformedInfo)?;
    Ok(StructuredInfoValue::collection(
        bounded(element_type, length),
        values,
    )?)
}

fn text_value(value: &str) -> StructuredInfoValue {
    leaf_value("value/text@1", value.as_bytes())
}

fn count_value(value: u64) -> StructuredInfoValue {
    leaf_value("value/count@1", value.to_string().as_bytes())
}

fn leaf_value(kind: &str, value: &[u8]) -> StructuredInfoValue {
    StructuredInfoValue::leaf(leaf(kind), value.to_vec()).expect("bounded linguistic leaf")
}

fn record_field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, LinguisticRefusal> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(LinguisticRefusal::MalformedInfo);
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(LinguisticRefusal::MalformedInfo)
}

fn collection_field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a [StructuredInfoValue], LinguisticRefusal> {
    let StructuredInfoValueShape::Collection(values) = record_field(value, name)?.shape() else {
        return Err(LinguisticRefusal::MalformedInfo);
    };
    Ok(values)
}

fn leaf_text(value: &StructuredInfoValue) -> Result<&str, LinguisticRefusal> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(LinguisticRefusal::MalformedInfo);
    };
    core::str::from_utf8(bytes).map_err(|_| LinguisticRefusal::MalformedInfo)
}
