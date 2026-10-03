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
            (i64::MAX, 959, 960),
            (61, i64::MAX, 960),
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
                    .map(|out| i64::from_le_bytes(out.try_into().unwrap())),
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
