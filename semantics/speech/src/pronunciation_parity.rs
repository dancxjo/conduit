//! Native spelling outputs retain exact record/variant identity after lowering.
use crate::{differential::program, generated::*};
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
};
use std::{vec, vec::Vec};
fn field<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value_type()
}
fn record(
    ty: &StructuredInfoType,
    values: Vec<(&str, StructuredInfoValue)>,
) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty.clone(),
        values
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value).unwrap())
            .collect(),
    )
    .unwrap()
}
fn variant(
    ty: &StructuredInfoType,
    tag: &str,
    payload: Option<StructuredInfoValue>,
) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    let case = cases.iter().find(|case| case.tag() == tag).unwrap();
    StructuredInfoValue::variant(
        ty.clone(),
        tag,
        payload.unwrap_or_else(|| {
            StructuredInfoValue::leaf(case.payload_type().clone(), vec![]).unwrap()
        }),
    )
    .unwrap()
}
fn phoneme(ty: &StructuredInfoType, value: EnglishPronouncedPhoneme) -> StructuredInfoValue {
    let p = PHONEMES
        .iter()
        .find(|(p, _)| *p == value.phoneme)
        .unwrap()
        .1;
    let stress = STRESSES
        .iter()
        .find(|(stress, _)| *stress == value.stress)
        .unwrap()
        .1;
    record(
        ty,
        vec![
            ("phoneme", variant(field(ty, "phoneme"), p, None)),
            ("stress", variant(field(ty, "stress"), stress, None)),
        ],
    )
}
fn phones(ty: &StructuredInfoType, value: EnglishWordPhonemes) -> StructuredInfoValue {
    let slots = [
        value.phoneme1,
        value.phoneme2,
        value.phoneme3,
        value.phoneme4,
        value.phoneme5,
        value.phoneme6,
        value.phoneme7,
        value.phoneme8,
        value.phoneme9,
        value.phoneme10,
        value.phoneme11,
        value.phoneme12,
    ];
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|field| {
                let ordinal: usize = field
                    .name()
                    .strip_prefix("phoneme")
                    .unwrap()
                    .parse()
                    .unwrap();
                let value = match slots[ordinal - 1] {
                    EnglishPhonemeSlot::absent => variant(field.value_type(), "absent", None),
                    EnglishPhonemeSlot::present(value) => {
                        let StructuredInfoTypeShape::Variant { cases, .. } =
                            field.value_type().shape()
                        else {
                            panic!("slot")
                        };
                        let payload_type = cases
                            .iter()
                            .find(|case| case.tag() == "present")
                            .unwrap()
                            .payload_type();
                        variant(
                            field.value_type(),
                            "present",
                            Some(phoneme(payload_type, value)),
                        )
                    }
                };
                StructuredFieldValue::new(field.name(), value).unwrap()
            })
            .collect(),
    )
    .unwrap()
}
fn payload<'a>(ty: &'a StructuredInfoType, tag: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .unwrap()
        .payload_type()
}
#[test]
fn lexicon_and_spelling_outputs_match_portable_exact_native_values() {
    let lexicon = program("speech_english_lexicon");
    for word in [
        "hello",
        "world",
        "synthesizer",
        "want",
        "chat",
        "a",
        "",
        "Hello",
    ] {
        let StructuredInfoTypeShape::Nominal { representation, .. } = lexicon.input_type.shape()
        else {
            panic!("word")
        };
        let input = StructuredInfoValue::nominal(
            lexicon.input_type.clone(),
            StructuredInfoValue::leaf(representation.clone(), word.as_bytes().to_vec()).unwrap(),
        )
        .unwrap();
        let expected = match speech_english_lexicon(word).unwrap() {
            EnglishLexiconDecision::matched(value) => variant(
                &lexicon.output_type,
                "matched",
                Some(phones(payload(&lexicon.output_type, "matched"), value)),
            ),
            EnglishLexiconDecision::unknown => variant(&lexicon.output_type, "unknown", None),
        };
        assert_eq!(
            lexicon.evaluate(&input.canonical_bytes().unwrap()).unwrap(),
            expected.canonical_bytes().unwrap(),
            "{word}"
        );
    }
    let spelling = program("speech_english_spelling");
    for word in [
        "chat", "three", "quick", "fox", "night", "case", "book", "don't", "summer",
    ] {
        let bytes = word.as_bytes();
        for index in 0..bytes.len() {
            let value = EnglishSpellingContext {
                previous: index
                    .checked_sub(1)
                    .and_then(|i| bytes.get(i))
                    .copied()
                    .unwrap_or(0) as i32,
                current: bytes[index] as i32,
                next: bytes.get(index + 1).copied().unwrap_or(0) as i32,
                after: bytes.get(index + 2).copied().unwrap_or(0) as i32,
                index: index as u32,
                length: bytes.len() as u32,
            };
            let input = record(
                &spelling.input_type,
                vec![
                    (
                        "previous",
                        StructuredInfoValue::leaf(
                            field(&spelling.input_type, "previous").clone(),
                            value.previous.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                    ),
                    (
                        "current",
                        StructuredInfoValue::leaf(
                            field(&spelling.input_type, "current").clone(),
                            value.current.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                    ),
                    (
                        "next",
                        StructuredInfoValue::leaf(
                            field(&spelling.input_type, "next").clone(),
                            value.next.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                    ),
                    (
                        "after",
                        StructuredInfoValue::leaf(
                            field(&spelling.input_type, "after").clone(),
                            value.after.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                    ),
                    (
                        "index",
                        StructuredInfoValue::leaf(
                            field(&spelling.input_type, "index").clone(),
                            value.index.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                    ),
                    (
                        "length",
                        StructuredInfoValue::leaf(
                            field(&spelling.input_type, "length").clone(),
                            value.length.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                    ),
                ],
            );
            let expected = match speech_english_spelling(value).unwrap() {
                EnglishSpellingResult::supported(value) => {
                    let ty = payload(&spelling.output_type, "supported");
                    variant(
                        &spelling.output_type,
                        "supported",
                        Some(record(
                            ty,
                            vec![
                                (
                                    "consumed",
                                    StructuredInfoValue::leaf(
                                        field(ty, "consumed").clone(),
                                        value.consumed.to_le_bytes().to_vec(),
                                    )
                                    .unwrap(),
                                ),
                                ("phonemes", phones(field(ty, "phonemes"), value.phonemes)),
                            ],
                        )),
                    )
                }
                EnglishSpellingResult::unsupported => {
                    variant(&spelling.output_type, "unsupported", None)
                }
            };
            assert_eq!(
                spelling
                    .evaluate(&input.canonical_bytes().unwrap())
                    .unwrap(),
                expected.canonical_bytes().unwrap(),
                "{word} at {index}"
            );
        }
    }
}

#[test]
fn digit_normalization_has_exact_portable_values_and_matches_digit_name_lexicon() {
    let p = program("speech_digit_normalization");
    for scalar in [
        i32::MIN,
        -1,
        0,
        47,
        48,
        49,
        50,
        51,
        52,
        53,
        54,
        55,
        56,
        57,
        58,
        255,
        i32::MAX,
    ] {
        let actual = speech_digit_normalization(scalar).unwrap();
        assert_eq!(
            speech_text_class(scalar) == Some(EnglishTextClass::digit),
            (48..=57).contains(&scalar)
        );
        let expected = match actual {
            EnglishDigitDecision::unsupported => variant(&p.output_type, "unsupported", None),
            EnglishDigitDecision::normalized(value) => {
                let name = [
                    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
                ][(scalar - 48) as usize];
                assert_eq!(
                    speech_english_lexicon(name),
                    Some(EnglishLexiconDecision::matched(value.phonemes))
                );
                assert_eq!(value.origin, EnglishPronunciationOrigin::digit_name);
                assert_eq!(
                    (value.before, value.after),
                    (EnglishTextClass::word_gap, EnglishTextClass::word_gap)
                );
                let ty = payload(&p.output_type, "normalized");
                variant(
                    &p.output_type,
                    "normalized",
                    Some(record(
                        ty,
                        vec![
                            ("phonemes", phones(field(ty, "phonemes"), value.phonemes)),
                            ("origin", variant(field(ty, "origin"), "digit_name", None)),
                            ("before", variant(field(ty, "before"), "word_gap", None)),
                            ("after", variant(field(ty, "after"), "word_gap", None)),
                        ],
                    )),
                )
            }
        }
        .canonical_bytes()
        .unwrap();
        assert_eq!(
            p.evaluate(&scalar.to_le_bytes()).unwrap(),
            expected,
            "{scalar}"
        );
    }
}
