//! Checked U64 projection parity, including arithmetic refusal boundaries.
use crate::{differential::program, frame_parity, generated::*};
use conduit_core::{StructuredInfoType, StructuredInfoValue};

pub(crate) fn record(ty: &StructuredInfoType, values: &[(&str, u64)]) -> std::vec::Vec<u8> {
    frame_parity::record(
        ty,
        &values
            .iter()
            .map(|(name, value)| {
                (
                    *name,
                    StructuredInfoValue::leaf(
                        frame_parity::field_type(ty, name).clone(),
                        value.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                )
            })
            .collect::<std::vec::Vec<_>>(),
    )
    .canonical_bytes()
    .unwrap()
}

#[test]
fn exact_rate_projection_matches_portable_unsigned_arithmetic() {
    let checked = program("speech_time_at_rate");
    for (numerator_seconds, denominator, sample_rate_hz) in [
        (0, 1, 8000),
        (61, 8000, 8000),
        (61, 8000, 16000),
        (960, 8000, 16000),
        (1, 3, 8000),
        (u64::MAX, u64::MAX, 1),
        (u64::MAX, 1, 192000),
        (1, 0, 8000),
    ] {
        let input = record(
            &checked.input_type,
            &[
                ("numerator_seconds", numerator_seconds),
                ("denominator", denominator),
                ("sample_rate_hz", sample_rate_hz),
            ],
        );
        let compiled = speech_time_at_rate(SpeechTimeProjectionInput {
            numerator_seconds,
            denominator,
            sample_rate_hz,
        })
        .map(|result| {
            record(
                &checked.output_type,
                &[
                    ("whole_frames", result.whole_frames),
                    ("remainder_numerator", result.remainder_numerator),
                ],
            )
        });
        assert_eq!(checked.evaluate(&input).ok(), compiled);
    }
}
