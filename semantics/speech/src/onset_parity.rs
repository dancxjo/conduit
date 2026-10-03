//! Stop-derived vowel onsets preserve segment identity and finite extent.
use crate::{
    differential::program,
    frame_parity::{field_type, integers, record},
    generated::*,
};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue};
use conduit_plot::PortableExpressionProgram;
use std::{vec, vec::Vec};

fn variant(ty: &StructuredInfoType, tag: &str) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    let case = cases.iter().find(|case| case.tag() == tag).unwrap();
    StructuredInfoValue::variant(
        ty.clone(),
        tag,
        StructuredInfoValue::leaf(case.payload_type().clone(), vec![]).unwrap(),
    )
    .unwrap()
}

#[test]
fn stop_place_is_derived_from_the_exact_selected_phone() {
    let p = program("speech_stop_place");
    for (phone, tag) in PHONES {
        let (expected, expected_tag) = match phone {
            EnglishPhone::p | EnglishPhone::b | EnglishPhone::p_aspirated => {
                (SpeechStopPlace::labial, "labial")
            }
            EnglishPhone::t | EnglishPhone::d | EnglishPhone::t_aspirated => {
                (SpeechStopPlace::alveolar, "alveolar")
            }
            EnglishPhone::k | EnglishPhone::g | EnglishPhone::k_aspirated => {
                (SpeechStopPlace::velar, "velar")
            }
            _ => (SpeechStopPlace::not_stop, "not_stop"),
        };
        assert_eq!(speech_stop_place(*phone), Some(expected));
        assert_eq!(
            p.evaluate(&variant(&p.input_type, tag).canonical_bytes().unwrap())
                .unwrap(),
            variant(&p.output_type, expected_tag)
                .canonical_bytes()
                .unwrap()
        );
    }
}

#[test]
fn onset_graph_matches_portable_models_for_every_phone_place_and_window_edge() {
    let graph = GRAPHS
        .iter()
        .find(|graph| graph.name == "speech_vowel_onset")
        .unwrap();
    let programs: Vec<_> = graph
        .steps
        .iter()
        .map(|(source, encoded)| {
            (
                *source,
                PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
            )
        })
        .collect();
    let ty = &programs[0].1.input_type;
    for (phone, phone_tag) in PHONES {
        let target = speech_voice_target(*phone).unwrap();
        for (place, place_tag) in [
            (SpeechStopPlace::not_stop, "not_stop"),
            (SpeechStopPlace::labial, "labial"),
            (SpeechStopPlace::alveolar, "alveolar"),
            (SpeechStopPlace::velar, "velar"),
        ] {
            for frame in [0i64, 159, 160, i64::MIN, i64::MAX] {
                let input = record(
                    ty,
                    &[
                        ("phone", variant(field_type(ty, "phone"), phone_tag)),
                        (
                            "target",
                            integers(field_type(ty, "target"), &target_fields(target)),
                        ),
                        (
                            "frame",
                            StructuredInfoValue::leaf(
                                field_type(ty, "frame").clone(),
                                frame.to_le_bytes().to_vec(),
                            )
                            .unwrap(),
                        ),
                        (
                            "previous_place",
                            variant(field_type(ty, "previous_place"), place_tag),
                        ),
                        ("relation", variant(field_type(ty, "relation"), "segment")),
                    ],
                )
                .canonical_bytes()
                .unwrap();
                let mut values: Vec<Vec<u8>> = vec![];
                let mut refused = false;
                for (source, program) in &programs {
                    match program.evaluate(if *source == usize::MAX {
                        &input
                    } else {
                        &values[*source]
                    }) {
                        Ok(value) => values.push(value),
                        Err(_) => {
                            refused = true;
                            break;
                        }
                    }
                }
                let actual = speech_vowel_onset(SpeechVowelOnsetInput {
                    phone: *phone,
                    target,
                    frame,
                    previous_place: place,
                    relation: SpeechNeighborRelation::segment,
                });
                assert_eq!(actual.is_none(), refused, "{phone:?}, {place:?}, {frame}");
                let Some(actual) = actual else {
                    continue;
                };
                let out = &programs[graph.result].1.output_type;
                let expected = integers(out, &target_fields(actual))
                    .canonical_bytes()
                    .unwrap();
                assert_eq!(
                    values[graph.result], expected,
                    "{phone:?}, {place:?}, {frame}"
                );
            }
        }
    }
}

#[test]
fn onset_models_restore_targets_keep_stable_poles_and_respect_boundaries() {
    let vowels = [
        EnglishPhone::iy,
        EnglishPhone::ih,
        EnglishPhone::ey,
        EnglishPhone::eh,
        EnglishPhone::ae,
        EnglishPhone::aa,
        EnglishPhone::ao,
        EnglishPhone::ow,
        EnglishPhone::uh,
        EnglishPhone::uw,
        EnglishPhone::ah,
        EnglishPhone::ax,
        EnglishPhone::er,
        EnglishPhone::ay,
        EnglishPhone::aw,
        EnglishPhone::oy,
    ];
    for (phone, _) in PHONES {
        let target = speech_voice_target(*phone).unwrap();
        for place in [
            SpeechStopPlace::labial,
            SpeechStopPlace::alveolar,
            SpeechStopPlace::velar,
        ] {
            let input = SpeechVowelOnsetInput {
                phone: *phone,
                target,
                frame: 0,
                previous_place: place,
                relation: SpeechNeighborRelation::segment,
            };
            let start = speech_vowel_onset(input).unwrap();
            if !vowels.contains(phone) {
                assert_eq!(start, target);
            } else {
                assert_eq!(start.f1, 200);
                match place {
                    SpeechStopPlace::labial => assert_eq!(start.f2, 800),
                    SpeechStopPlace::alveolar => assert_eq!(start.f2, 1800),
                    SpeechStopPlace::velar => {
                        assert_eq!(start.f2, start.f3);
                        assert_eq!(start.b2, start.b3);
                        assert_eq!(start.c2, start.c3);
                    }
                    SpeechStopPlace::not_stop => unreachable!(),
                }
            }
            for frame in 0..=160 {
                let t = speech_vowel_onset(SpeechVowelOnsetInput { frame, ..input }).unwrap();
                assert_eq!(
                    (t.frames, t.closure, t.voiced, t.frication),
                    (
                        target.frames,
                        target.closure,
                        target.voiced,
                        target.frication
                    )
                );
                for (b, c) in [(t.b1, t.c1), (t.b2, t.c2), (t.b3, t.c3)] {
                    // Jury stability conditions for 1 - b*z^-1 - c*z^-2.
                    assert!(16384 - b - c > 0 && 16384 + b - c > 0 && 16384 + c > 0);
                }
            }
            assert_eq!(
                speech_vowel_onset(SpeechVowelOnsetInput {
                    frame: 160,
                    ..input
                })
                .unwrap(),
                target
            );
            for relation in [
                SpeechNeighborRelation::word_boundary,
                SpeechNeighborRelation::phrase_boundary,
                SpeechNeighborRelation::turn_boundary,
                SpeechNeighborRelation::sequence_edge,
            ] {
                assert_eq!(
                    speech_vowel_onset(SpeechVowelOnsetInput { relation, ..input }).unwrap(),
                    target
                );
            }
        }
    }
}
