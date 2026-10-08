#![cfg(feature = "semantic-bindings")]
#[path = "common_acoustic/common.rs"]
mod common;
use common::*;
use conduit_audio::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::semantic::*;
use conduit_speech::*;
#[test]
fn original_speech_quantities_bridge_exactly_then_execute_audio_at_three_rates() {
    let duration = SpeechExactDuration::new(10, 1).unwrap();
    let cycle = SpeechFundamentalCycle::new(200, 1).unwrap();
    let intensity = SpeechRelativeIntensity::new(16, 8).unwrap();
    let d = speech_duration_to_audio(&duration.clone().encode().unwrap()).unwrap();
    let c = speech_cycle_to_audio(&cycle.clone().encode().unwrap()).unwrap();
    let a = speech_intensity_to_audio(&intensity.clone().encode().unwrap()).unwrap();
    assert_eq!(d.original(), &duration);
    assert_eq!(c.original(), &cycle);
    assert_eq!(a.original(), &intensity);
    assert_eq!(*a.result().numerator(), 8);
    assert_eq!(*a.result().denominator(), 16);
    reproduce(d.executions());
    reproduce(c.executions());
    reproduce(a.executions());
    let hz = PreparedAcousticReciprocal::new()
        .unwrap()
        .cycle_to_frequency(c.admitted_canonical())
        .unwrap();
    assert_eq!(*hz.result().numerator_hz(), 200);
    assert_eq!(*hz.result().denominator(), 1);
    for rate in [8000, 16000, 48000] {
        let basis =
            AudioSampleRateBasis::new(anchor(), AudioFrameQuantization::Floor, rate).unwrap();
        let request = AudioSampleProjectionRequest::new(
            basis,
            AudioSampleProjectionQuantity::duration(
                *d.result().denominator(),
                *d.result().numerator_seconds(),
            )
            .unwrap(),
        )
        .unwrap();
        let r = PreparedAudioSampleProjection::new()
            .unwrap()
            .project(&request.encode().unwrap())
            .unwrap();
        assert_eq!(
            u128::from(*r.result().raw().whole_frames()),
            u128::from(rate) / 10
        );
    }
    let extreme = SpeechExactDuration::new(u64::MAX, u64::MAX).unwrap();
    assert_eq!(
        speech_duration_to_audio(&extreme.encode().unwrap())
            .unwrap()
            .result(),
        &AudioTimeFraction::new(u64::MAX, u64::MAX).unwrap()
    );
}
#[test]
fn syllabic_rate_has_explicit_scope_and_exact_reciprocal_not_pitch_hz() {
    for (n, d) in [(4, 1), (8, 16), (u64::MAX, u64::MAX)] {
        let original = SpeechSyllabicRate::new(d, n, scope()).unwrap();
        let r = speech_rate_to_syllable_period(&original.clone().encode().unwrap()).unwrap();
        assert_eq!(r.original(), &original);
        assert_eq!(r.result().scope(), original.scope());
        assert_eq!(*r.result().numerator_seconds(), d);
        assert_eq!(*r.result().denominator_syllables(), n);
        assert_eq!(
            u128::from(n) * u128::from(*r.result().numerator_seconds()),
            u128::from(d) * u128::from(*r.result().denominator_syllables())
        );
        reproduce(r.executions());
    }
    let zero = SpeechSyllabicRate::new(1, 0, scope()).unwrap();
    assert!(speech_rate_to_syllable_period(&zero.encode().unwrap()).is_err());
}
#[test]
fn signed_tilt_single_octave_profile_and_overflow_refuse_without_approximation() {
    for n in [i64::MIN, -6, 0, 6, i64::MAX] {
        for octaves in [-1, 0, 1] {
            let tilt = SpeechSpectralTilt::new(
                AudioDecibelConvention::PowerTenLog10,
                AudioDecibelFraction::new(3, n).unwrap(),
            )
            .unwrap();
            let request = SpeechTiltOctaveRequest::new(octaves, tilt).unwrap();
            let result = speech_tilt_single_octave_delta(&request.clone().encode().unwrap());
            if n == i64::MIN && octaves == -1 {
                assert!(result.is_err());
                continue;
            }
            let r = result.unwrap();
            assert_eq!(r.original(), &request);
            assert_eq!(
                i128::from(*r.result().numerator_db()),
                i128::from(n) * i128::from(octaves)
            );
            assert_eq!(*r.result().denominator(), 3);
            reproduce(r.executions());
        }
    }
    let tilt = SpeechSpectralTilt::new(
        AudioDecibelConvention::AmplitudeTwentyLog10,
        AudioDecibelFraction::new(1, -6).unwrap(),
    )
    .unwrap();
    assert!(speech_tilt_single_octave_delta(
        &SpeechTiltOctaveRequest::new(2, tilt)
            .unwrap()
            .encode()
            .unwrap()
    )
    .is_err());
}
