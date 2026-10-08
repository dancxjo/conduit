#![cfg(feature = "semantic-bindings")]
#[path = "common_acoustic/common.rs"]
mod common;
use common::*;
use conduit_audio::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::semantic::*;
use conduit_speech::*;
fn trajectory(
    left: AudioTrajectoryQuantity,
    right: AudioTrajectoryQuantity,
) -> AudioQuantityTrajectory {
    let segment = AudioTrajectorySegment::new(
        AudioExactTimeOffset::new(1, 1).unwrap(),
        AudioTrajectoryInterpolation::Linear,
        left,
        right,
        AudioExactTimeOffset::new(1, 0).unwrap(),
    )
    .unwrap();
    AudioQuantityTrajectory::new(
        anchor(),
        AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
        AudioTrajectoryOutside::Refuse,
        AudioTrajectoryProvenance::new(
            AudioTrajectoryProvenanceKind::Authored,
            "target fixture".into(),
            Some("v1".into()),
        )
        .unwrap(),
        BoundedSequence::try_from_iter(vec![segment]).unwrap(),
    )
    .unwrap()
}
#[test]
fn known_pitch_and_intensity_overlap_and_use_actual_audio_source_interpolation() {
    for (role, left, right, n) in [
        (
            SpeechTargetAudioRole::Pitch,
            AudioTrajectoryQuantity::frequency(1, 100).unwrap(),
            AudioTrajectoryQuantity::frequency(1, 200).unwrap(),
            150u64,
        ),
        (
            SpeechTargetAudioRole::Intensity,
            AudioTrajectoryQuantity::amplitude(4, 1).unwrap(),
            AudioTrajectoryQuantity::amplitude(4, 3).unwrap(),
            2,
        ),
    ] {
        let original =
            SpeechKnownAudioTarget::new(provenance(), role, scope(), trajectory(left, right))
                .unwrap();
        let prepared = PreparedSpeechAudioTarget::new(&original.clone().encode().unwrap()).unwrap();
        let r = prepared.query(&query(1, 2).encode().unwrap()).unwrap();
        assert_eq!(r.original(), &original);
        reproduce(r.domain_executions());
        match r.audio().result() {
            AudioTrajectoryQuantity::Frequency(v) => assert_eq!(
                u128::from(*v.numerator_hz()),
                u128::from(n) * u128::from(*v.denominator())
            ),
            AudioTrajectoryQuantity::Amplitude(v) => {
                assert_eq!(u128::from(*v.numerator()) * 2, u128::from(*v.denominator()))
            }
            _ => panic!("domain"),
        }
        for e in r.audio().executions() {
            assert_eq!(
                conduit_plot::PortableExpressionProgram::from_canonical_hex(e.source_program_hex())
                    .unwrap()
                    .evaluate(e.input_canonical())
                    .unwrap(),
                e.output_canonical()
            );
        }
    }
}
#[test]
fn pitch_does_not_accept_amplitude_and_formant_center_is_not_cycle_or_coefficient() {
    for role in [
        SpeechTargetAudioRole::Pitch,
        SpeechTargetAudioRole::FormantCenter,
        SpeechTargetAudioRole::FormantBandwidth,
    ] {
        let v = AudioTrajectoryQuantity::amplitude(1, 1).unwrap();
        let request =
            SpeechKnownAudioTarget::new(provenance(), role, scope(), trajectory(v.clone(), v))
                .unwrap();
        assert!(matches!(
            PreparedSpeechAudioTarget::new(&request.encode().unwrap()),
            Err(SpeechAudioTargetRefusal::WrongQuantityDomain)
        ));
    }
    let center = SpeechFormantTarget::new(
        SpeechAudioTrajectorySpecification::Unknown,
        SpeechAudioTrajectorySpecification::NotApplicable,
        1,
        provenance(),
    )
    .unwrap();
    assert_ne!(
        center.center().clone().encode().unwrap(),
        center.bandwidth().clone().encode().unwrap()
    );
}
