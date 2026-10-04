//! Context-preserving authored pitch/DSP composition and portable refusals.
use crate::{
    frame_parity::{field_type, integers, record, result, state},
    generated::*,
};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue};
use conduit_plot::PortableExpressionProgram;
use std::{vec, vec::Vec};

pub(super) fn input(ty: &StructuredInfoType, value: SpeechProsodyInput) -> Vec<u8> {
    let stress_type = field_type(ty, "stress");
    let tag = STRESSES
        .iter()
        .find(|(stress, _)| *stress == value.stress)
        .unwrap()
        .1;
    let StructuredInfoTypeShape::Variant { cases, .. } = stress_type.shape() else {
        panic!("stress")
    };
    let case = cases.iter().find(|case| case.tag() == tag).unwrap();
    record(
        ty,
        &[
            (
                "cycle",
                crate::frame_parity::cycle_value(field_type(ty, "cycle"), value.cycle),
            ),
            (
                "stress",
                StructuredInfoValue::variant(
                    stress_type.clone(),
                    tag,
                    StructuredInfoValue::leaf(case.payload_type().clone(), vec![]).unwrap(),
                )
                .unwrap(),
            ),
            (
                "attack",
                StructuredInfoValue::leaf(
                    field_type(ty, "attack").clone(),
                    vec![u8::from(value.attack)],
                )
                .unwrap(),
            ),
            (
                "release",
                StructuredInfoValue::leaf(
                    field_type(ty, "release").clone(),
                    vec![u8::from(value.release)],
                )
                .unwrap(),
            ),
            (
                "target",
                integers(field_type(ty, "target"), &target_fields(value.target)),
            ),
            (
                "frame",
                StructuredInfoValue::leaf(
                    field_type(ty, "frame").clone(),
                    value.frame.to_le_bytes().to_vec(),
                )
                .unwrap(),
            ),
            ("state", state(field_type(ty, "state"), value.state)),
        ],
    )
    .canonical_bytes()
    .unwrap()
}

pub(super) fn graph(name: &str) -> (Vec<(usize, PortableExpressionProgram)>, usize) {
    let graph = GRAPHS.iter().find(|graph| graph.name == name).unwrap();
    (
        graph
            .steps
            .iter()
            .map(|(source, encoded)| {
                (
                    *source,
                    PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
                )
            })
            .collect(),
        graph.result,
    )
}
pub(super) fn evaluate(
    programs: &[(usize, PortableExpressionProgram)],
    result: usize,
    input: &[u8],
) -> Option<Vec<u8>> {
    let mut values: Vec<Vec<u8>> = Vec::new();
    for (source, program) in programs {
        let argument = if *source == usize::MAX {
            input
        } else {
            &values[*source]
        };
        values.push(program.evaluate(argument).ok()?);
    }
    Some(values[result].clone())
}

pub(super) fn context(
    stress: EnglishStress,
    target: SpeechAcousticTarget,
    frame: i32,
) -> SpeechProsodyInput {
    SpeechProsodyInput {
        cycle: crate::frame_parity::profile_cycle(),
        stress,
        target,
        frame,
        attack: true,
        release: true,
        state: speech_initial_state(SpeechStart::begin).unwrap(),
    }
}

#[test]
fn authored_pitch_and_dsp_chain_matches_portable_graph_with_exact_context() {
    let (programs, result_index) = graph("speech_prosodic_frame");
    assert_eq!(programs.len(), 14); // three intensity stages, two pitch stages and nine DSP stages
    let ty = &programs[0].1.input_type;
    let output_type = &programs[result_index].1.output_type;
    for (phone, _) in PHONES {
        let target = speech_voice_target(*phone).unwrap();
        for (stress, _) in STRESSES {
            for frame in [0, 63, target.closure, target.frames - 1] {
                let mut value = context(*stress, target, frame);
                value.attack = frame != 63;
                value.release = frame != target.frames - 1;
                value.state = SpeechFrameState {
                    voicing: -1234,
                    phase: 57,
                    noise: 65535,
                    first1: 32767,
                    first2: -32767,
                    second1: -32000,
                    second2: 32000,
                    third1: 12345,
                    third2: -12345,
                };
                assert_eq!(
                    evaluate(&programs, result_index, &input(ty, value)),
                    speech_prosodic_frame(value).map(|out| result(output_type, out)),
                    "{phone:?}, {stress:?}, {frame}"
                );
                let pitched = speech_pitch(value).unwrap();
                assert_eq!(pitched.target, value.target);
                assert_eq!(pitched.state, value.state);
                assert_eq!(pitched.frame, value.frame);
                assert_eq!(pitched.attack, value.attack);
                assert_eq!(pitched.release, value.release);
                assert_eq!(
                    speech_prosodic_frame(value),
                    speech_frame(speech_pitch(speech_stress_intensity(value).unwrap()).unwrap()),
                );
            }
        }
    }
    let target = speech_voice_target(EnglishPhone::iy).unwrap();
    let mut cases = [context(EnglishStress::primary, target, 0); 3];
    cases[0].state.noise = i32::MAX;
    cases[1].frame = i32::MAX;
    cases[2].target.frames = 1;
    for value in cases {
        assert_eq!(speech_prosodic_frame(value), None);
        assert_eq!(evaluate(&programs, result_index, &input(ty, value)), None);
    }
}

#[test]
fn pitch_policy_stays_bounded_monotone_and_retains_uncertainty() {
    let (programs, result_index) = graph("speech_pitch");
    let ty = &programs[0].1.input_type;
    for (stress, _) in STRESSES {
        for total in [640, 960] {
            let mut target = speech_voice_target(EnglishPhone::iy).unwrap();
            target.frames = total;
            let mut previous = 0;
            for frame in 0..total {
                let value = context(*stress, target, frame);
                let period = speech_pitch(value).unwrap().period;
                let expected = match stress {
                    EnglishStress::primary => 58 + frame * 8 / (total - 1),
                    EnglishStress::secondary => 62 + frame * 5 / (total - 1),
                    EnglishStress::unstressed | EnglishStress::reduced => {
                        67 + frame * 2 / (total - 1)
                    }
                    EnglishStress::unknown | EnglishStress::unspecified => 67,
                };
                assert_eq!(period, expected);
                assert!((58..=69).contains(&period));
                assert!(period >= previous);
                previous = period;
            }
        }
        for (frame, total) in [(0, 1), (i32::MAX, 960), (0, i32::MIN)] {
            let mut target = speech_voice_target(EnglishPhone::iy).unwrap();
            target.frames = total;
            let value = context(*stress, target, frame);
            assert_eq!(
                evaluate(&programs, result_index, &input(ty, value)).is_some(),
                speech_pitch(value).is_some(),
                "{stress:?}, {frame}/{total}"
            );
            if matches!(stress, EnglishStress::unknown | EnglishStress::unspecified) {
                assert_eq!(speech_pitch(value).unwrap().period, 67);
            } else {
                assert!(speech_pitch(value).is_none());
            }
        }
    }
}
