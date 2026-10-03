//! Checked contour arithmetic and preservation of stress uncertainty.
use crate::{
    differential::program,
    frame_parity::{field_type, record},
    generated::*,
};
use conduit_core::{StructuredInfoTypeShape, StructuredInfoValue};
use std::vec;

#[test]
fn pitch_contours_match_portable_arithmetic_for_all_stress_states() {
    let p = program("speech_pitch_contour");
    let ty = &p.input_type;
    let stress_type = field_type(ty, "stress");
    let StructuredInfoTypeShape::Variant { cases, .. } = stress_type.shape() else {
        panic!("stress")
    };
    for (stress, tag) in STRESSES {
        let case = cases.iter().find(|c| c.tag() == *tag).unwrap();
        for (base_period, frame, total) in [
            (speech_pitch_period(*stress).unwrap(), 0, 960),
            (61, 479, 960),
            (61, 959, 960),
            (67, 639, 640),
            (61, 0, 1),
            (i32::MAX, 959, 960),
            (61, i32::MAX, 960),
        ] {
            let value = SpeechPitchInput {
                stress: *stress,
                base_period,
                frame,
                total,
            };
            let input = record(
                ty,
                &[
                    (
                        "stress",
                        StructuredInfoValue::variant(
                            stress_type.clone(),
                            *tag,
                            StructuredInfoValue::leaf(case.payload_type().clone(), vec![]).unwrap(),
                        )
                        .unwrap(),
                    ),
                    (
                        "base_period",
                        StructuredInfoValue::leaf(
                            field_type(ty, "base_period").clone(),
                            base_period.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
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
                        "total",
                        StructuredInfoValue::leaf(
                            field_type(ty, "total").clone(),
                            total.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                    ),
                ],
            )
            .canonical_bytes()
            .unwrap();
            assert_eq!(
                p.evaluate(&input)
                    .ok()
                    .map(|out| i32::from_le_bytes(out.try_into().unwrap())),
                speech_pitch_contour(value),
                "{stress:?}, {frame}/{total}"
            );
        }
    }
}

#[test]
fn admitted_contours_are_bounded_monotone_and_uncertainty_remains_neutral() {
    for (stress, _) in STRESSES {
        let base_period = speech_pitch_period(*stress).unwrap();
        for total in [640, 960] {
            let mut previous = 0;
            for frame in 0..total {
                let period = speech_pitch_contour(SpeechPitchInput {
                    stress: *stress,
                    base_period,
                    frame,
                    total,
                })
                .unwrap();
                assert!((58..=69).contains(&period));
                assert!(period >= previous);
                if matches!(stress, EnglishStress::unknown | EnglishStress::unspecified) {
                    assert_eq!(period, base_period);
                }
                previous = period;
            }
        }
    }
    for (stress, start, end) in [
        (EnglishStress::primary, 58, 66),
        (EnglishStress::secondary, 62, 67),
        (EnglishStress::unstressed, 67, 69),
        (EnglishStress::reduced, 67, 69),
    ] {
        let base_period = speech_pitch_period(stress).unwrap();
        assert_eq!(
            speech_pitch_contour(SpeechPitchInput {
                stress,
                base_period,
                frame: 0,
                total: 960
            }),
            Some(start)
        );
        assert_eq!(
            speech_pitch_contour(SpeechPitchInput {
                stress,
                base_period,
                frame: 959,
                total: 960
            }),
            Some(end)
        );
    }
}

#[test]
fn authored_pitch_chain_preserves_context_and_portable_refusals() {
    use conduit_plot::PortableExpressionProgram;
    use std::vec::Vec;
    let graph = GRAPHS
        .iter()
        .find(|graph| graph.name == "speech_pitch")
        .unwrap();
    assert_eq!(graph.steps.len(), 2);
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
    let ty = &programs[0].1.input_type;
    let stress_type = field_type(ty, "stress");
    let StructuredInfoTypeShape::Variant { cases, .. } = stress_type.shape() else {
        panic!("stress")
    };
    for (stress, tag) in STRESSES {
        let case = cases.iter().find(|c| c.tag() == *tag).unwrap();
        for (frame, total) in [
            (0, 960),
            (479, 960),
            (959, 960),
            (639, 640),
            (0, 1),
            (i32::MAX, 960),
            (0, i32::MIN),
        ] {
            let input = record(
                ty,
                &[
                    (
                        "stress",
                        StructuredInfoValue::variant(
                            stress_type.clone(),
                            *tag,
                            StructuredInfoValue::leaf(case.payload_type().clone(), vec![]).unwrap(),
                        )
                        .unwrap(),
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
                        "total",
                        StructuredInfoValue::leaf(
                            field_type(ty, "total").clone(),
                            total.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                    ),
                ],
            )
            .canonical_bytes()
            .unwrap();
            let mut values = Vec::new();
            let portable = (|| {
                for (source, program) in &programs {
                    let argument = if *source == usize::MAX {
                        &input
                    } else {
                        &values[*source]
                    };
                    values.push(program.evaluate(argument).ok()?);
                }
                Some(i32::from_le_bytes(
                    values[graph.result].as_slice().try_into().unwrap(),
                ))
            })();
            let actual = speech_pitch(SpeechPitchContext {
                stress: *stress,
                frame,
                total,
            });
            assert_eq!(actual, portable, "{stress:?}, {frame}/{total}");
            assert_eq!(
                actual,
                speech_pitch_period(*stress).and_then(|base_period| {
                    speech_pitch_contour(SpeechPitchInput {
                        stress: *stress,
                        base_period,
                        frame,
                        total,
                    })
                })
            );
        }
    }
}
