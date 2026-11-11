use conduit_human::{ExperienceEvidenceSource, ExperienceTemporalPolicy};
use conduit_time::{NativeTemporalInstant, NativeTemporalScale};

pub fn source() -> ExperienceEvidenceSource {
    ExperienceEvidenceSource::temporal_context(
        NativeTemporalInstant::new(
            "clock/observation\"\n".into(),
            2,
            NativeTemporalScale::Milliseconds,
            100,
            7,
        )
        .unwrap(),
        ExperienceTemporalPolicy::new(20, 50).unwrap(),
        NativeTemporalInstant::new(
            "clock/reference".into(),
            3,
            NativeTemporalScale::Milliseconds,
            200,
            11,
        )
        .unwrap(),
    )
    .unwrap()
}

/// Forge a scalar inside otherwise exact native canonical bytes. This exercises
/// projection's semantic checks without asking a generated constructor to lie.
pub fn set_u64(encoded: &mut [u8], field: &str, value: u64) {
    let start = encoded
        .windows(field.len())
        .rposition(|bytes| bytes == field.as_bytes())
        .unwrap()
        + field.len();
    assert_eq!(&encoded[start..start + 5], &[0, 8, 0, 0, 0]);
    encoded[start + 5..start + 13].copy_from_slice(&value.to_le_bytes());
}
