//! Temporal acoustics preserve segment identity and checked graph semantics.
use crate::{
    frame_parity::{field_type, integers, record},
    generated::*,
};
use conduit_core::{StructuredInfoTypeShape, StructuredInfoValue};
use conduit_plot::PortableExpressionProgram;
use std::{vec, vec::Vec};

#[test]
fn temporal_targets_match_portable_graph_for_every_phone_and_transition_edge() {
    let graph = GRAPHS
        .iter()
        .find(|g| g.name == "speech_phone_frame_target")
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
    let input_type = &programs[0].1.input_type;
    let phone_type = field_type(input_type, "phone");
    let StructuredInfoTypeShape::Variant { cases, .. } = phone_type.shape() else {
        panic!("phone")
    };
    for (phone, tag) in PHONES {
        let target = speech_voice_target(*phone).unwrap();
        for frame in [
            0,
            179,
            180,
            239,
            240,
            275,
            276,
            479,
            480,
            719,
            720,
            target.frames - 1,
        ] {
            if frame >= target.frames {
                continue;
            }
            let case = cases.iter().find(|c| c.tag() == *tag).unwrap();
            let input = record(
                input_type,
                &[
                    (
                        "phone",
                        StructuredInfoValue::variant(
                            phone_type.clone(),
                            *tag,
                            StructuredInfoValue::leaf(case.payload_type().clone(), vec![]).unwrap(),
                        )
                        .unwrap(),
                    ),
                    (
                        "frame",
                        StructuredInfoValue::leaf(
                            field_type(input_type, "frame").clone(),
                            frame.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                    ),
                    (
                        "target",
                        integers(field_type(input_type, "target"), &target_fields(target)),
                    ),
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
            let actual = speech_phone_frame_target(SpeechTrajectoryInput {
                phone: *phone,
                frame,
                target,
            })
            .unwrap();
            assert_eq!(
                values[graph.result],
                integers(
                    &programs[graph.result].1.output_type,
                    &target_fields(actual)
                )
                .canonical_bytes()
                .unwrap(),
                "{tag} at {frame}"
            );
        }
    }
}

#[test]
fn diphthongs_move_spectrum_without_changing_duration_or_segment_identity() {
    for (phone, destination) in [
        (EnglishPhone::ey, EnglishPhone::iy),
        (EnglishPhone::ay, EnglishPhone::iy),
        (EnglishPhone::oy, EnglishPhone::iy),
        (EnglishPhone::ow, EnglishPhone::uw),
        (EnglishPhone::aw, EnglishPhone::uw),
    ] {
        let target = speech_voice_target(phone).unwrap();
        let at = |frame| {
            speech_phone_frame_target(SpeechTrajectoryInput {
                phone,
                frame,
                target,
            })
            .unwrap()
        };
        let end = speech_voice_target(destination).unwrap();
        assert_eq!(at(0), target);
        assert_ne!(at(target.frames / 2).b2, target.b2);
        assert_eq!(at(target.frames - 1).b2, end.b2);
        assert_eq!(at(target.frames - 1).frames, target.frames);
    }
    // A monophthong remains stationary, rather than receiving an inferred glide.
    let target = speech_voice_target(EnglishPhone::eh).unwrap();
    assert_eq!(
        speech_phone_frame_target(SpeechTrajectoryInput {
            phone: EnglishPhone::eh,
            frame: 800,
            target
        }),
        Some(target)
    );
}

#[test]
fn plain_stops_have_silent_closure_and_audible_finite_release() {
    for phone in [EnglishPhone::p, EnglishPhone::t, EnglishPhone::k] {
        let base = speech_voice_target(phone).unwrap();
        let mut state = speech_initial_state(SpeechStart::begin).unwrap();
        let mut release_energy = 0_i64;
        for frame in 0..base.frames {
            let target = speech_phone_frame_target(SpeechTrajectoryInput {
                phone,
                frame,
                target: base,
            })
            .unwrap();
            assert_eq!(
                target.frication,
                i64::from(frame >= base.closure && frame < base.closure + 96)
            );
            let result = speech_frame(SpeechFrameInput {
                target,
                frame,
                period: 61,
                state,
            })
            .unwrap();
            if frame < base.closure {
                assert_eq!(result.sample, 0);
            } else if frame < base.closure + 96 {
                release_energy += result.sample.abs();
            }
            state = result.state;
        }
        assert!(release_energy > 0, "{phone:?}");
    }
}
