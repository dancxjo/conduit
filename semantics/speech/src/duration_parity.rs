//! Exact cumulative-time arithmetic agrees with the checked portable graph.
use crate::{generated::*, timing_parity::record};

#[test]
fn sum_preserves_exact_values_and_checked_refusals() {
    let (programs, result) = crate::prosody_parity::graph("speech_duration_sum");
    for (left_numerator, left_denominator, right_numerator, right_denominator) in [
        (0, 1, 1, 3),
        (1, 3, 1, 3),
        (2, 3, 1, 3),
        (1, 3, 1, 8000),
        (8003, 24000, 120, 1000),
        (120, 1000, 1, 3),
        (u64::MAX, u64::MAX, 0, u64::MAX),
        (u64::MAX, 1, 1, 1),
        (1, u64::MAX, 1, 2),
        (0, 0, 1, 3),
        (1, 3, 0, 0),
    ] {
        let input = record(
            &programs[0].1.input_type,
            &[
                ("left_numerator", left_numerator),
                ("left_denominator", left_denominator),
                ("right_numerator", right_numerator),
                ("right_denominator", right_denominator),
            ],
        );
        let compiled = speech_duration_sum(SpeechDurationSumInput {
            left_numerator,
            left_denominator,
            right_numerator,
            right_denominator,
        })
        .map(|value| {
            record(
                &programs[result].1.output_type,
                &[
                    ("numerator_seconds", value.numerator_seconds),
                    ("denominator", value.denominator),
                ],
            )
        });
        assert_eq!(
            crate::prosody_parity::evaluate(&programs, result, &input),
            compiled
        );
    }
}

#[test]
fn frame_span_uses_checked_subtraction() {
    let program = crate::differential::program("speech_frame_span");
    for (start_frame, end_frame) in [
        (0, 2666),
        (2666, 5333),
        (5333, 8000),
        (1, 1),
        (1, 0),
        (0, u64::MAX),
    ] {
        let input = record(
            &program.input_type,
            &[("start_frame", start_frame), ("end_frame", end_frame)],
        );
        let actual = speech_frame_span(SpeechFrameSpanInput {
            start_frame,
            end_frame,
        })
        .map(|value| value.to_le_bytes().to_vec());
        assert_eq!(actual, program.evaluate(&input).ok());
    }
}

#[test]
fn admitted_grid_domain_and_duration_targets_match_portable_plots() {
    let admission = crate::differential::program("speech_event_duration_admitted");
    for boundary in [false, true] {
        for frames in [-1_i32, 0, 1, 2, 240000, 240001, i32::MAX] {
            let input = crate::frame_parity::record(
                &admission.input_type,
                &[
                    (
                        "boundary",
                        conduit_core::StructuredInfoValue::leaf(
                            crate::frame_parity::field_type(&admission.input_type, "boundary")
                                .clone(),
                            std::vec![u8::from(boundary)],
                        )
                        .unwrap(),
                    ),
                    (
                        "frames",
                        conduit_core::StructuredInfoValue::leaf(
                            crate::frame_parity::field_type(&admission.input_type, "frames")
                                .clone(),
                            frames.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                    ),
                ],
            )
            .canonical_bytes()
            .unwrap();
            assert_eq!(
                admission.evaluate(&input).ok(),
                speech_event_duration_admitted(SpeechEventDurationCheck { boundary, frames })
                    .map(|value| std::vec![u8::from(value)])
            );
        }
    }
    let checked = crate::differential::program("speech_duration_target");
    for (phone, _) in PHONES {
        let target = speech_voice_target(*phone).unwrap();
        for frames in [2_i32, 64, 320, 640, 960, 1600, 240000] {
            let input = crate::frame_parity::record(
                &checked.input_type,
                &[
                    (
                        "target",
                        crate::frame_parity::integers(
                            crate::frame_parity::field_type(&checked.input_type, "target"),
                            &target_fields(target),
                        ),
                    ),
                    (
                        "frames",
                        conduit_core::StructuredInfoValue::leaf(
                            crate::frame_parity::field_type(&checked.input_type, "frames").clone(),
                            frames.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                    ),
                ],
            )
            .canonical_bytes()
            .unwrap();
            let actual =
                speech_duration_target(SpeechDurationTargetInput { target, frames }).unwrap();
            let output =
                crate::frame_parity::integers(&checked.output_type, &target_fields(actual))
                    .canonical_bytes()
                    .unwrap();
            assert_eq!(checked.evaluate(&input).unwrap(), output);
            assert_eq!(actual.frames, frames);
            if frames == target.frames {
                assert_eq!(actual, target);
            }
            if target.closure > 0 {
                assert!(actual.closure > 0 && actual.closure < frames);
            } else {
                assert_eq!(actual.closure, 0);
            }
        }
    }
}
