//! Whole transition parity, including wire order and retained history.
use crate::generated::*;
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
};
use conduit_plot::PortableExpressionProgram;
use std::{vec, vec::Vec};

pub(super) fn record(
    ty: &StructuredInfoType,
    values: &[(&str, StructuredInfoValue)],
) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|field| {
                StructuredFieldValue::new(
                    field.name(),
                    values
                        .iter()
                        .find(|(name, _)| *name == field.name())
                        .unwrap()
                        .1
                        .clone(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
pub(super) fn field_type<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value_type()
}
pub(super) fn integers(ty: &StructuredInfoType, values: &[(&str, i32)]) -> StructuredInfoValue {
    record(
        ty,
        &values
            .iter()
            .map(|(name, value)| {
                (
                    *name,
                    StructuredInfoValue::leaf(
                        field_type(ty, name).clone(),
                        value.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                )
            })
            .collect::<Vec<_>>(),
    )
}
pub(super) fn state(ty: &StructuredInfoType, value: SpeechFrameState) -> StructuredInfoValue {
    integers(
        ty,
        &[
            ("voicing", value.voicing),
            ("phase", value.phase),
            ("noise", value.noise),
            ("first1", value.first1),
            ("first2", value.first2),
            ("second1", value.second1),
            ("second2", value.second2),
            ("third1", value.third1),
            ("third2", value.third2),
        ],
    )
}
pub(super) fn profile_cycle() -> SpeechFrameCycleControl {
    SpeechFrameCycleControl {
        mode: SpeechCycleControlMode::profile,
        phase_q8: 0,
        period_q8: 0,
    }
}
pub(super) fn cycle_value(
    ty: &StructuredInfoType,
    value: SpeechFrameCycleControl,
) -> StructuredInfoValue {
    let tag = match value.mode {
        SpeechCycleControlMode::profile => "profile",
        SpeechCycleControlMode::resolved => "resolved",
    };
    record(
        ty,
        &[
            (
                "mode",
                crate::onset_parity::variant(field_type(ty, "mode"), tag),
            ),
            (
                "phase_q8",
                StructuredInfoValue::leaf(
                    field_type(ty, "phase_q8").clone(),
                    value.phase_q8.to_le_bytes().to_vec(),
                )
                .unwrap(),
            ),
            (
                "period_q8",
                StructuredInfoValue::leaf(
                    field_type(ty, "period_q8").clone(),
                    value.period_q8.to_le_bytes().to_vec(),
                )
                .unwrap(),
            ),
        ],
    )
}
pub(super) fn input(ty: &StructuredInfoType, value: SpeechFrameInput) -> Vec<u8> {
    record(
        ty,
        &[
            ("cycle", cycle_value(field_type(ty, "cycle"), value.cycle)),
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
            ("state", state(field_type(ty, "state"), value.state)),
            (
                "period",
                StructuredInfoValue::leaf(
                    field_type(ty, "period").clone(),
                    value.period.to_le_bytes().to_vec(),
                )
                .unwrap(),
            ),
            (
                "frame",
                StructuredInfoValue::leaf(
                    field_type(ty, "frame").clone(),
                    value.frame.to_le_bytes().to_vec(),
                )
                .unwrap(),
            ),
        ],
    )
    .canonical_bytes()
    .unwrap()
}
pub(super) fn result(ty: &StructuredInfoType, value: SpeechFrameResult) -> Vec<u8> {
    record(
        ty,
        &[
            (
                "phase_q8",
                StructuredInfoValue::leaf(
                    field_type(ty, "phase_q8").clone(),
                    value.phase_q8.to_le_bytes().to_vec(),
                )
                .unwrap(),
            ),
            ("state", state(field_type(ty, "state"), value.state)),
            (
                "sample",
                StructuredInfoValue::leaf(
                    field_type(ty, "sample").clone(),
                    value.sample.to_le_bytes().to_vec(),
                )
                .unwrap(),
            ),
        ],
    )
    .canonical_bytes()
    .unwrap()
}

#[test]
fn composed_frame_agrees_with_portable_graph_at_all_phone_and_envelope_edges() {
    let graph = GRAPHS
        .iter()
        .find(|graph| graph.name == "speech_frame")
        .unwrap();
    let result_index = graph.result;
    let programs = graph
        .steps
        .iter()
        .map(|(source, encoded)| {
            (
                *source,
                PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let input_type = &programs
        .iter()
        .find(|(source, _)| *source == usize::MAX)
        .unwrap()
        .1
        .input_type;
    let output_type = &programs[result_index].1.output_type;
    for (phone, _) in PHONES {
        let target = speech_voice_target(*phone).unwrap();
        for frame in [
            0,
            63,
            64,
            target.closure.saturating_sub(1),
            target.closure,
            target.closure + 1,
            target.frames - 1,
        ] {
            for period in [61, 67] {
                let modes: &[(bool, bool)] = if matches!(phone, EnglishPhone::eh | EnglishPhone::p)
                {
                    &[(true, true), (false, true), (true, false), (false, false)]
                } else {
                    &[(true, true)]
                };
                for &(attack, release) in modes {
                    let value = SpeechFrameInput {
                        cycle: crate::frame_parity::profile_cycle(),
                        attack,
                        release,
                        target,
                        frame,
                        period,
                        state: SpeechFrameState {
                            voicing: -1234,
                            phase: frame % period,
                            noise: (frame * 25173) % 65536,
                            first1: 32767,
                            first2: -32767,
                            second1: -32000,
                            second2: 32000,
                            third1: 12345,
                            third2: -12345,
                        },
                    };
                    let input = input(input_type, value);
                    let mut values: Vec<Vec<u8>> = vec![];
                    for (source, program) in &programs {
                        let argument = if *source == usize::MAX {
                            &input
                        } else {
                            &values[*source]
                        };
                        values.push(program.evaluate(argument).unwrap());
                    }
                    assert_eq!(
                    values[result_index],
                    result(output_type, speech_frame(value).unwrap()),
                    "{phone:?}, frame {frame}, period {period}, attack {attack}, release {release}"
                );
                }
            }
        }
    }
    let target = speech_voice_target(EnglishPhone::iy).unwrap();
    for (noise, period) in [(i32::MAX, 61), (1, 0)] {
        let value = SpeechFrameInput {
            cycle: profile_cycle(),
            attack: true,
            release: true,
            target,
            period,
            frame: 0,
            state: SpeechFrameState {
                noise,
                ..speech_initial_state(SpeechStart::begin).unwrap()
            },
        };
        assert_eq!(speech_frame(value), None);
        assert_eq!(
            crate::prosody_parity::evaluate(&programs, result_index, &input(input_type, value)),
            None,
        );
    }
}

#[test]
fn frame_history_matches_the_original_checked_scalar_composition() {
    let mut history = speech_initial_state(SpeechStart::begin).unwrap();
    for (phone, _) in PHONES {
        let target = speech_voice_target(*phone).unwrap();
        for frame in 0..target.frames {
            let target = speech_phone_frame_target(SpeechTrajectoryInput {
                phone: *phone,
                frame,
                target,
            })
            .unwrap();
            let noise = speech_noise(history.noise).unwrap();
            let excitation = speech_excitation(ExcitationInput {
                phase: history.phase,
                period: 61,
                noise,
                voiced: target.voiced,
                frication: target.frication,
            })
            .unwrap();
            let voicing = speech_excitation(ExcitationInput {
                phase: history.phase,
                period: 61,
                noise,
                voiced: target.voiced,
                frication: 0,
            })
            .unwrap();
            let upper = if target.voiced == 1 && target.frication == 0 {
                voicing - history.voicing
            } else if target.voiced == 1 && target.frication == 1 && target.closure == 0 {
                speech_excitation(ExcitationInput {
                    phase: history.phase,
                    period: 61,
                    noise,
                    voiced: 0,
                    frication: 1,
                })
                .unwrap()
            } else {
                excitation
            };
            let lower = if target.voiced == 1 && target.frication == 1 && target.closure == 0 {
                voicing
            } else {
                excitation
            };
            let mut current = [0; 3];
            for (index, (gain, b, c, y1, y2)) in [
                (
                    target.gain1,
                    target.b1,
                    target.c1,
                    history.first1,
                    history.first2,
                ),
                (
                    target.gain2,
                    target.b2,
                    target.c2,
                    history.second1,
                    history.second2,
                ),
                (
                    target.gain3,
                    target.b3,
                    target.c3,
                    history.third1,
                    history.third2,
                ),
            ]
            .into_iter()
            .enumerate()
            {
                current[index] = speech_filter_limit(ResonatorInput {
                    drive: speech_drive(DriveInput {
                        sample: if index == 0 { lower } else { upper },
                        gain,
                    })
                    .unwrap(),
                    b,
                    c,
                    y1,
                    y2,
                })
                .unwrap();
            }
            let envelope = speech_envelope(EnvelopeInput {
                attack: true,
                release: true,
                frame,
                total: target.frames,
                closure: target.closure,
            })
            .unwrap();
            let expected = SpeechFrameResult {
                phase_q8: speech_phase(PhaseInput {
                    phase: history.phase,
                    period: 61,
                })
                .unwrap()
                    * 256,
                state: SpeechFrameState {
                    voicing,
                    phase: speech_phase(PhaseInput {
                        phase: history.phase,
                        period: 61,
                    })
                    .unwrap(),
                    noise,
                    first1: current[0],
                    first2: history.first1,
                    second1: current[1],
                    second2: history.second1,
                    third1: current[2],
                    third2: history.third1,
                },
                sample: speech_limit(
                    speech_mix(MixInput {
                        first: current[0],
                        second: current[1],
                        third: current[2],
                        bypass: if target.bypass_gain == 0 {
                            0
                        } else {
                            upper * target.bypass_gain / 256
                        },
                        envelope,
                    })
                    .unwrap(),
                )
                .unwrap(),
            };
            let actual = speech_frame(SpeechFrameInput {
                cycle: crate::frame_parity::profile_cycle(),
                attack: true,
                release: true,
                target,
                frame,
                period: 61,
                state: history,
            })
            .unwrap();
            assert_eq!(actual, expected, "{phone:?}, frame {frame}");
            history = actual.state;
        }
        assert_eq!(
            speech_boundary_frame(SpeechBoundaryFrameInput {
                state: history,
                phase_q8: history.phase * 256
            })
            .unwrap(),
            SpeechFrameResult {
                phase_q8: history.phase * 256,
                state: history,
                sample: 0
            }
        );
    }
}
