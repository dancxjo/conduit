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
            target.closure.saturating_sub(1),
            target.closure,
            target.closure + 1,
            target.closure + 95,
            target.closure + 96,
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
                i32::from(frame >= base.closure && frame < base.closure + 96)
            );
            let result = speech_frame(SpeechFrameInput {
                attack: true,
                release: true,
                target,
                frame,
                period: 61,
                state,
            })
            .unwrap();
            if frame < base.closure {
                assert_eq!(result.sample, 0);
            } else if frame < base.closure + 96 {
                release_energy += i64::from(result.sample).abs();
            }
            state = result.state;
        }
        assert!(release_energy > 0, "{phone:?}");
    }
}

#[test]
fn stop_place_and_voicing_have_distinct_bounded_realizations() {
    let p = released_samples(EnglishPhone::p);
    let t = released_samples(EnglishPhone::t);
    let k = released_samples(EnglishPhone::k);
    assert_ne!(p, t);
    assert_ne!(p, k);
    assert_ne!(t, k);
    for (plain, voiced, aspirated) in [
        (EnglishPhone::p, EnglishPhone::b, EnglishPhone::p_aspirated),
        (EnglishPhone::t, EnglishPhone::d, EnglishPhone::t_aspirated),
        (EnglishPhone::k, EnglishPhone::g, EnglishPhone::k_aspirated),
    ] {
        let base = speech_voice_target(plain).unwrap();
        let voiced_base = speech_voice_target(voiced).unwrap();
        let aspirated_base = speech_voice_target(aspirated).unwrap();
        assert_eq!(base.frames, voiced_base.frames);
        assert_eq!(base.frames, aspirated_base.frames);
        assert!(base.frames - base.closure <= 128);
        assert!(voiced_base.closure < base.closure);
        assert!(aspirated_base.frames - aspirated_base.closure > 96);
        assert_ne!(released_samples(plain), released_samples(voiced));
        assert_ne!(released_samples(plain), released_samples(aspirated));
        assert_eq!(
            speech_envelope(EnvelopeInput {
                attack: true,
                release: true,
                frame: base.closure,
                total: base.frames,
                closure: base.closure
            }),
            Some(256)
        );
        assert_eq!(
            speech_envelope(EnvelopeInput {
                attack: true,
                release: true,
                frame: base.frames - 1,
                total: base.frames,
                closure: base.closure
            }),
            Some(4)
        );
    }
}

fn released_samples(phone: EnglishPhone) -> Vec<i64> {
    let base = speech_voice_target(phone).unwrap();
    let mut state = speech_initial_state(SpeechStart::begin).unwrap();
    let mut samples = vec![];
    for frame in 0..base.frames {
        let target = speech_phone_frame_target(SpeechTrajectoryInput {
            phone,
            frame,
            target: base,
        })
        .unwrap();
        let result = speech_frame(SpeechFrameInput {
            attack: true,
            release: true,
            target,
            frame,
            period: 61,
            state,
        })
        .unwrap();
        if frame >= base.closure {
            samples.push(i64::from(result.sample));
        }
        state = result.state;
    }
    samples
}

#[test]
fn unvoiced_stop_spectra_separate_low_mid_and_high_energy_models() {
    // Independent DFT of actual rendered PCM, not a comparison of table fields.
    // This is deterministic profile evidence, not a human intelligibility test.
    let high_fraction = |phone| {
        let samples = released_samples(phone);
        let mut total = 0.0;
        let mut high = 0.0;
        for bin in 1..=samples.len() / 2 {
            let mut real = 0.0;
            let mut imaginary = 0.0;
            for (index, sample) in samples.iter().enumerate() {
                let angle =
                    std::f64::consts::TAU * bin as f64 * index as f64 / samples.len() as f64;
                real += *sample as f64 * angle.cos();
                imaginary += *sample as f64 * angle.sin();
            }
            let power = real * real + imaginary * imaginary;
            total += power;
            if bin * 8000 >= samples.len() * 2500 {
                high += power;
            }
        }
        assert!(total > 0.0);
        high / total
    };
    let p = high_fraction(EnglishPhone::p);
    let t = high_fraction(EnglishPhone::t);
    let k = high_fraction(EnglishPhone::k);
    assert!(p < 0.15, "p: {p}");
    assert!(t > 0.65, "t: {t}");
    assert!(p < k && k < t, "p: {p}, k: {k}, t: {t}");
}
