//! Neighbor-model joins preserve exact segment extent and pause distinctions.
use crate::{
    frame_parity::{field_type, integers, record},
    generated::*,
};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue};
use conduit_plot::PortableExpressionProgram;
use std::{vec, vec::Vec};

fn neighbor(ty: &StructuredInfoType, value: SpeechNeighborModel, tag: &str) -> StructuredInfoValue {
    let relation = field_type(ty, "relation");
    let StructuredInfoTypeShape::Variant { cases, .. } = relation.shape() else {
        panic!("relation")
    };
    let case = cases.iter().find(|case| case.tag() == tag).unwrap();
    record(
        ty,
        &[
            (
                "relation",
                StructuredInfoValue::variant(
                    relation.clone(),
                    tag,
                    StructuredInfoValue::leaf(case.payload_type().clone(), vec![]).unwrap(),
                )
                .unwrap(),
            ),
            (
                "model",
                integers(field_type(ty, "model"), &target_fields(value.model)),
            ),
        ],
    )
}
fn isolated(model: SpeechAcousticTarget) -> SpeechNeighborModel {
    SpeechNeighborModel {
        relation: SpeechNeighborRelation::sequence_edge,
        model,
    }
}
fn segment(model: SpeechAcousticTarget) -> SpeechNeighborModel {
    SpeechNeighborModel {
        relation: SpeechNeighborRelation::segment,
        model,
    }
}

#[test]
fn connected_target_graph_matches_portable_models_at_every_transition_edge() {
    let graph = GRAPHS
        .iter()
        .find(|graph| graph.name == "speech_connected_target")
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
    let other = speech_voice_target(EnglishPhone::iy).unwrap();
    for (phone, _) in PHONES {
        let target = speech_voice_target(*phone).unwrap();
        for (relation, tag) in [
            (SpeechNeighborRelation::segment, "segment"),
            (SpeechNeighborRelation::sequence_edge, "sequence_edge"),
            (SpeechNeighborRelation::word_boundary, "word_boundary"),
            (SpeechNeighborRelation::phrase_boundary, "phrase_boundary"),
            (SpeechNeighborRelation::turn_boundary, "turn_boundary"),
        ] {
            // Cover every phone at connected edges; boundary discriminator
            // coverage uses one voiced and one noise model to keep proof finite.
            if tag != "segment" && !matches!(phone, EnglishPhone::eh | EnglishPhone::h) {
                continue;
            }
            for frame in [
                0,
                127,
                128,
                target.frames - 129,
                target.frames - 128,
                target.frames - 1,
            ] {
                let model = SpeechNeighborModel {
                    relation,
                    model: other,
                };
                let value = SpeechConnectedInput {
                    target,
                    frame,
                    previous: model,
                    next: model,
                };
                let input = record(
                    ty,
                    &[
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
                        ("previous", neighbor(field_type(ty, "previous"), model, tag)),
                        ("next", neighbor(field_type(ty, "next"), model, tag)),
                    ],
                )
                .canonical_bytes()
                .unwrap();
                let mut values: Vec<Vec<u8>> = vec![];
                for (source, program) in &programs {
                    values.push(
                        program
                            .evaluate(if *source == usize::MAX {
                                &input
                            } else {
                                &values[*source]
                            })
                            .unwrap(),
                    );
                }
                let actual = speech_connected_target(value).unwrap();
                let out = &programs[graph.result].1.output_type;
                let expected = record(
                    out,
                    &[
                        (
                            "target",
                            integers(field_type(out, "target"), &target_fields(actual.target)),
                        ),
                        (
                            "attack",
                            StructuredInfoValue::leaf(
                                field_type(out, "attack").clone(),
                                vec![u8::from(actual.attack)],
                            )
                            .unwrap(),
                        ),
                        (
                            "release",
                            StructuredInfoValue::leaf(
                                field_type(out, "release").clone(),
                                vec![u8::from(actual.release)],
                            )
                            .unwrap(),
                        ),
                    ],
                )
                .canonical_bytes()
                .unwrap();
                assert_eq!(values[graph.result], expected, "{phone:?}, {tag}, {frame}");
            }
        }
    }
}

#[test]
fn sonorant_joins_share_an_exact_midpoint_without_merging_durations() {
    for left in [
        EnglishPhone::eh,
        EnglishPhone::ay,
        EnglishPhone::m,
        EnglishPhone::l,
    ] {
        let base = speech_voice_target(left).unwrap();
        let end = speech_phone_frame_target(SpeechTrajectoryInput {
            phone: left,
            frame: base.frames - 1,
            target: base,
        })
        .unwrap();
        for right in [
            EnglishPhone::iy,
            EnglishPhone::ow,
            EnglishPhone::n,
            EnglishPhone::r,
        ] {
            let start = speech_voice_target(right).unwrap();
            let a = speech_connected_target(SpeechConnectedInput {
                target: end,
                frame: end.frames - 1,
                previous: isolated(end),
                next: segment(start),
            })
            .unwrap();
            let b = speech_connected_target(SpeechConnectedInput {
                target: start,
                frame: 0,
                previous: segment(end),
                next: isolated(start),
            })
            .unwrap();
            assert!(!a.release && !b.attack);
            assert_eq!(a.target.frames, end.frames);
            assert_eq!(b.target.frames, start.frames);
            for ((name, a), (_, b)) in target_fields(a.target)
                .into_iter()
                .zip(target_fields(b.target))
            {
                if name != "frames" {
                    assert_eq!(a, b, "{left:?} -> {right:?}: {name}");
                }
            }
        }
    }
}

#[test]
fn boundaries_and_noise_or_stop_neighbors_keep_isolated_envelopes() {
    let current = speech_voice_target(EnglishPhone::eh).unwrap();
    let sonorant = speech_voice_target(EnglishPhone::iy).unwrap();
    for relation in [
        SpeechNeighborRelation::word_boundary,
        SpeechNeighborRelation::phrase_boundary,
        SpeechNeighborRelation::turn_boundary,
        SpeechNeighborRelation::sequence_edge,
    ] {
        let model = SpeechNeighborModel {
            relation,
            model: sonorant,
        };
        let result = speech_connected_target(SpeechConnectedInput {
            target: current,
            frame: 0,
            previous: model,
            next: model,
        })
        .unwrap();
        assert_eq!(
            result,
            SpeechConnectedResult {
                target: current,
                attack: true,
                release: true
            }
        );
    }
    for phone in [
        EnglishPhone::p,
        EnglishPhone::b,
        EnglishPhone::sh,
        EnglishPhone::h,
    ] {
        let model = segment(speech_voice_target(phone).unwrap());
        let result = speech_connected_target(SpeechConnectedInput {
            target: current,
            frame: 0,
            previous: model,
            next: model,
        })
        .unwrap();
        assert_eq!(
            result,
            SpeechConnectedResult {
                target: current,
                attack: true,
                release: true
            }
        );
    }
}
