use conduit_audio::{PcmChannelLayout, PcmClipProfile, PcmSampleRepresentation};
use conduit_form::rust_binding::NativeRustBinding;

#[test]
fn pcm_clip_profile_round_trips_exact_values() {
    let profile = PcmClipProfile::new(
        u64::MAX,
        PcmChannelLayout::StereoLeftRight,
        PcmSampleRepresentation::Signed24LittleEndian,
        48_000,
    )
    .unwrap();
    let structured = profile.into_structured().unwrap();
    let decoded = PcmClipProfile::from_structured(structured).unwrap();
    assert_eq!(
        *decoded.representation(),
        PcmSampleRepresentation::Signed24LittleEndian
    );
    assert_eq!(*decoded.sample_rate_hz(), 48_000);
    assert_eq!(*decoded.layout(), PcmChannelLayout::StereoLeftRight);
    assert_eq!(*decoded.clock_id(), u64::MAX);
}

#[test]
fn pcm_clip_profile_enforces_sample_rate_and_clock_bounds() {
    let make = |sample_rate_hz, clock_id| {
        PcmClipProfile::new(
            clock_id,
            PcmChannelLayout::Mono,
            PcmSampleRepresentation::Signed16LittleEndian,
            sample_rate_hz,
        )
    };
    assert!(make(8_000, 1).is_ok());
    assert!(make(192_000, u64::MAX).is_ok());
    assert!(make(7_999, 1).is_err());
    assert!(make(192_001, 1).is_err());
    assert!(make(48_000, 0).is_err());
}
