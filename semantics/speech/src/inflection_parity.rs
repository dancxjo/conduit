//! Compact inflection carriers preserve exact portable native meaning.
use crate::{
    differential::program,
    frame_parity::{field_type, record},
    generated::*,
    onset_parity::variant,
};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue};
use std::{vec, vec::Vec};
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
fn slot(ty: &StructuredInfoType, value: EnglishPhonemeSlot) -> StructuredInfoValue {
    let EnglishPhonemeSlot::present(value) = value else {
        return variant(ty, "absent");
    };
    let inner = payload(ty, "present");
    let phoneme = PHONEMES
        .iter()
        .find(|(p, _)| *p == value.phoneme)
        .unwrap()
        .1;
    let stress = STRESSES.iter().find(|(s, _)| *s == value.stress).unwrap().1;
    StructuredInfoValue::variant(
        ty.clone(),
        "present",
        record(
            inner,
            &[
                ("phoneme", variant(field_type(inner, "phoneme"), phoneme)),
                ("stress", variant(field_type(inner, "stress"), stress)),
            ],
        ),
    )
    .unwrap()
}
#[test]
fn every_final_phoneme_and_suffix_matches_portable_ending_meaning() {
    let checked = program("speech_english_inflection_ending");
    for (suffix, tag) in [
        (EnglishRegularSuffix::s, "s"),
        (EnglishRegularSuffix::ed, "ed"),
    ] {
        for &(final_phoneme, phone_tag) in PHONEMES {
            let input = record(
                &checked.input_type,
                &[
                    (
                        "suffix",
                        variant(field_type(&checked.input_type, "suffix"), tag),
                    ),
                    (
                        "final_phoneme",
                        variant(field_type(&checked.input_type, "final_phoneme"), phone_tag),
                    ),
                ],
            );
            let ending = speech_english_inflection_ending(EnglishInflectionEndingInput {
                suffix,
                final_phoneme,
            })
            .unwrap();
            let output = record(
                &checked.output_type,
                &[
                    (
                        "first",
                        slot(field_type(&checked.output_type, "first"), ending.first),
                    ),
                    (
                        "second",
                        slot(field_type(&checked.output_type, "second"), ending.second),
                    ),
                ],
            );
            assert_eq!(
                checked.evaluate(&input.canonical_bytes().unwrap()).unwrap(),
                output.canonical_bytes().unwrap()
            );
        }
    }
}
#[test]
fn scanner_length_and_character_equivalence_classes_match_portable_candidates() {
    let checked = program("speech_english_inflection_scan");
    for length in 0_u32..=32 {
        for last in [0_i32, 97, 100, 101, 115, 127] {
            for before in [0_i32, 97, 100, 101, 115, 127] {
                let number = |name, bytes: Vec<u8>| {
                    StructuredInfoValue::leaf(field_type(&checked.input_type, name).clone(), bytes)
                        .unwrap()
                };
                let input = record(
                    &checked.input_type,
                    &[
                        ("length", number("length", length.to_le_bytes().to_vec())),
                        ("last", number("last", last.to_le_bytes().to_vec())),
                        ("before", number("before", before.to_le_bytes().to_vec())),
                    ],
                );
                let actual = speech_english_inflection_scan(EnglishInflectionScanInput {
                    length,
                    last,
                    before,
                })
                .unwrap();
                let output = match actual {
                    EnglishInflectionScanResult::absent => variant(&checked.output_type, "absent"),
                    EnglishInflectionScanResult::candidate(value) => {
                        let ty = payload(&checked.output_type, "candidate");
                        let second_ty = field_type(ty, "second_strip");
                        let second = match value.second_strip {
                            EnglishStemStrip::absent => variant(second_ty, "absent"),
                            EnglishStemStrip::present(n) => StructuredInfoValue::variant(
                                second_ty.clone(),
                                "present",
                                StructuredInfoValue::leaf(
                                    payload(second_ty, "present").clone(),
                                    n.to_le_bytes().to_vec(),
                                )
                                .unwrap(),
                            )
                            .unwrap(),
                        };
                        let suffix = match value.suffix {
                            EnglishRegularSuffix::s => "s",
                            EnglishRegularSuffix::ed => "ed",
                        };
                        StructuredInfoValue::variant(
                            checked.output_type.clone(),
                            "candidate",
                            record(
                                ty,
                                &[
                                    ("suffix", variant(field_type(ty, "suffix"), suffix)),
                                    (
                                        "first_strip",
                                        StructuredInfoValue::leaf(
                                            field_type(ty, "first_strip").clone(),
                                            value.first_strip.to_le_bytes().to_vec(),
                                        )
                                        .unwrap(),
                                    ),
                                    ("second_strip", second),
                                ],
                            ),
                        )
                        .unwrap()
                    }
                };
                assert_eq!(
                    checked.evaluate(&input.canonical_bytes().unwrap()).unwrap(),
                    output.canonical_bytes().unwrap()
                );
            }
        }
    }
}
#[test]
fn every_stem_class_and_suffix_matches_portable_eligibility() {
    let checked = program("speech_english_inflection_eligible");
    for (stem_class, class_tag) in [
        (EnglishRegularStemClass::ineligible, "ineligible"),
        (EnglishRegularStemClass::s_only, "s_only"),
        (EnglishRegularStemClass::regular_both, "regular_both"),
    ] {
        for (suffix, tag) in [
            (EnglishRegularSuffix::s, "s"),
            (EnglishRegularSuffix::ed, "ed"),
        ] {
            let input = record(
                &checked.input_type,
                &[
                    (
                        "stem_class",
                        variant(field_type(&checked.input_type, "stem_class"), class_tag),
                    ),
                    (
                        "suffix",
                        variant(field_type(&checked.input_type, "suffix"), tag),
                    ),
                ],
            );
            let result =
                speech_english_inflection_eligible(EnglishInflectionBasis { stem_class, suffix })
                    .unwrap();
            // Primitive Boolean Fores use raw canonical value bytes; the record
            // input retains its structured envelope.
            assert_eq!(
                checked.evaluate(&input.canonical_bytes().unwrap()).unwrap(),
                vec![u8::from(result)]
            );
        }
    }
}

#[test]
fn every_declared_stem_and_unknown_controls_match_portable_stem_class() {
    let checked = program("speech_english_inflection_stem_class");
    for word in [
        "listen",
        "thank",
        "please",
        "voice",
        "want",
        "world",
        "device",
        "conduit",
        "synthesizer",
        "speech",
        "speak",
        "read",
        "as",
        "fox",
        "speeche",
        "readed",
        "Voice",
        "",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        let StructuredInfoTypeShape::Nominal { representation, .. } = checked.input_type.shape()
        else {
            panic!("word")
        };
        let input = StructuredInfoValue::nominal(
            checked.input_type.clone(),
            StructuredInfoValue::leaf(representation.clone(), word.as_bytes().to_vec()).unwrap(),
        )
        .unwrap();
        let tag = match speech_english_inflection_stem_class(word).unwrap() {
            EnglishRegularStemClass::ineligible => "ineligible",
            EnglishRegularStemClass::s_only => "s_only",
            EnglishRegularStemClass::regular_both => "regular_both",
        };
        assert_eq!(
            checked.evaluate(&input.canonical_bytes().unwrap()).unwrap(),
            variant(&checked.output_type, tag)
                .canonical_bytes()
                .unwrap()
        );
    }
}
