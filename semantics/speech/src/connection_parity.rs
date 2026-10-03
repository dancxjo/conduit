//! Neighbor-model joins preserve exact segment extent and pause distinctions.
use crate::{
    frame_parity::{field_type, integers, record},
    generated::*,
    onset_parity::variant,
};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue};
use conduit_plot::PortableExpressionProgram;
use std::{vec, vec::Vec};

pub(super) fn neighbor(
    ty: &StructuredInfoType,
    value: SpeechNeighborModel,
    tag: &str,
) -> StructuredInfoValue {
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

fn isolated_input(phone: EnglishPhone, target: SpeechAcousticTarget) -> SpeechConnectedInput {
    SpeechConnectedInput {
        phone,
        target,
        stress: EnglishStress::unspecified,
        state: speech_initial_state(SpeechStart::begin).unwrap(),
        previous_place: SpeechStopPlace::not_stop,
        frame: 0,
        previous: isolated(target),
        next: isolated(target),
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
    let (composed, composed_result) = crate::prosody_parity::graph("speech_connected_frame");
    assert_eq!(composed.len(), 20);
    let ty = &programs[0].1.input_type;
    let other = speech_voice_target(EnglishPhone::iy).unwrap();
    for (phone, phone_tag) in PHONES {
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
            for (frame_index, frame) in [
                0,
                127,
                128,
                target.frames - 129,
                target.frames - 128,
                target.frames - 1,
            ]
            .into_iter()
            .enumerate()
            {
                let model = SpeechNeighborModel {
                    relation,
                    model: other,
                };
                let value = SpeechConnectedInput {
                    stress: STRESSES[frame_index].0,
                    target,
                    frame,
                    previous: model,
                    next: model,
                    ..isolated_input(*phone, target)
                };
                let input = record(
                    ty,
                    &[
                        ("phone", variant(field_type(ty, "phone"), phone_tag)),
                        (
                            "stress",
                            variant(field_type(ty, "stress"), STRESSES[frame_index].1),
                        ),
                        (
                            "state",
                            crate::frame_parity::state(field_type(ty, "state"), value.state),
                        ),
                        (
                            "previous_place",
                            variant(field_type(ty, "previous_place"), "not_stop"),
                        ),
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
                let final_frame = speech_connected_frame(value).unwrap();
                assert_eq!(
                    crate::prosody_parity::evaluate(&composed, composed_result, &input),
                    Some(crate::frame_parity::result(
                        &composed[composed_result].1.output_type,
                        final_frame
                    )),
                    "full graph: {phone:?}, {tag}, {frame}"
                );
                let paired =
                    speech_connected_onset_input(speech_connected_target(value).unwrap()).unwrap();
                assert_eq!(final_frame, speech_contextual_frame(paired).unwrap());
                assert_eq!(paired.phone, value.phone);
                assert_eq!(paired.prosody.stress, value.stress);
                assert_eq!(paired.prosody.state, value.state);
                assert_eq!(paired.prosody.frame, value.frame);
                assert_eq!(paired.previous_place, value.previous_place);
                assert_eq!(paired.relation, value.previous.relation);
                let actual = speech_connected_target(value).unwrap();
                assert_eq!(actual.input, value);
                let out = &programs[graph.result].1.output_type;
                let expected = record(
                    out,
                    &[
                        (
                            "input",
                            StructuredInfoValue::from_canonical_bytes(&input).unwrap(),
                        ),
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
                ..isolated_input(left, end)
            })
            .unwrap();
            let b = speech_connected_target(SpeechConnectedInput {
                target: start,
                frame: 0,
                previous: segment(end),
                next: isolated(start),
                ..isolated_input(right, start)
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
            ..isolated_input(EnglishPhone::eh, current)
        })
        .unwrap();
        assert_eq!(
            (result.target, result.attack, result.release),
            (current, true, true)
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
            ..isolated_input(EnglishPhone::eh, current)
        })
        .unwrap();
        assert_eq!(
            (result.target, result.attack, result.release),
            (current, true, true)
        );
    }
}

#[test]
fn connected_context_keeps_stop_place_stress_and_history_distinct() {
    let target = speech_voice_target(EnglishPhone::eh).unwrap();
    for (previous_phone, place) in [
        (EnglishPhone::p, SpeechStopPlace::labial),
        (EnglishPhone::t, SpeechStopPlace::alveolar),
        (EnglishPhone::k, SpeechStopPlace::velar),
        (EnglishPhone::iy, SpeechStopPlace::not_stop),
    ] {
        for (stress, _) in STRESSES {
            let mut value = isolated_input(EnglishPhone::eh, target);
            value.stress = *stress;
            value.state.first1 = 12345;
            value.state.second2 = -23456;
            value.previous_place = place;
            value.previous = segment(speech_voice_target(previous_phone).unwrap());
            assert_eq!(speech_stop_place(previous_phone), Some(place));
            let paired =
                speech_connected_onset_input(speech_connected_target(value).unwrap()).unwrap();
            assert_eq!(paired.previous_place, place);
            assert_eq!(paired.prosody.stress, *stress);
            assert_eq!(paired.prosody.state, value.state);
            assert_eq!(paired.phone, value.phone);
            assert_eq!(
                speech_connected_frame(value),
                speech_contextual_frame(paired)
            );
        }
    }
}
