//! Source-level timing and shared Audio conformance, before owner integration.
#![cfg(feature = "semantic-bindings")]
use conduit_audio::*;
use conduit_core::ConfigurationValue;
use conduit_plot::{rust_binding::NativeRustBinding, *};
use conduit_speech::semantic::*;

fn document() -> CheckedSyntaxDocument {
    let source = format!(
        "{}\n{}",
        include_str!("../timing.conduit"),
        include_str!("../phone_gestures.conduit")
    );
    let mut catalog = StartupCatalog::new();
    conduit_audio::authoring::install(&mut catalog).unwrap();
    check_syntax_document(&parse_syntax_document(&source), &catalog).unwrap()
}
fn program(document: &CheckedSyntaxDocument, name: &str) -> PortableExpressionProgram {
    let expanded =
        expand_canonical_plot_for_authoring(document, name, &ProfileCatalog::new()).unwrap();
    let [gear] = expanded.expanded.gears.as_slice() else {
        panic!("one checked expression Gear")
    };
    let [entry] = gear.configuration.as_slice() else {
        panic!("one retained Source program")
    };
    let ConfigurationValue::Text(bytes) = &entry.value else {
        panic!("canonical program bytes")
    };
    PortableExpressionProgram::from_canonical_hex(bytes).unwrap()
}
fn duration(n: u64) -> SpeechExactDuration {
    SpeechExactDuration::new(1000, n).unwrap()
}
fn timing(
    start: u64,
    current: u64,
    previous: Option<u64>,
    next: Option<u64>,
) -> SpeechPhoneGestureTiming {
    SpeechPhoneGestureTiming::new(
        duration(current),
        next.map(duration),
        previous.map(duration),
        AudioExactTimeOffset::new(1000, start).unwrap(),
    )
    .unwrap()
}
fn phases(
    program: &PortableExpressionProgram,
    timing: SpeechPhoneGestureTiming,
) -> SpeechPhoneGesturePhases {
    let raw =
        SpeechPhoneGestureRawPhases::decode(&program.evaluate(&timing.encode().unwrap()).unwrap())
            .unwrap();
    // Admission copies executed Source fields; no Rust arithmetic or retagging.
    let time = |n| AudioExactTimeOffset::new(*raw.denominator(), n).unwrap();
    SpeechPhoneGesturePhases::new(
        time(*raw.closure_end()),
        time(*raw.leading_end()),
        time(*raw.leading_start()),
        time(*raw.nominal_end()),
        time(*raw.nominal_start()),
        time(*raw.release_end()),
        time(*raw.trailing_end()),
        time(*raw.trailing_start()),
    )
    .unwrap()
}
fn anchor() -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
fn provenance() -> AudioTrajectoryProvenance {
    AudioTrajectoryProvenance::new(
        AudioTrajectoryProvenanceKind::Authored,
        "phone-gesture-source-conformance".into(),
        Some("v1".into()),
    )
    .unwrap()
}

#[test]
fn unequal_neighbor_durations_share_exact_overlapping_transition_and_rate_independent_phases() {
    let doc = document();
    let source = program(&doc, "speech/phone-gesture-phases");
    let first = phases(&source, timing(100, 200, Some(100), Some(300)));
    let second = phases(&source, timing(300, 300, Some(200), None));
    assert_eq!(first.trailing_start(), second.leading_start());
    assert_eq!(first.trailing_end(), second.leading_end());
    assert!(first.trailing_start().numerator_seconds() < first.nominal_end().numerator_seconds());
    assert!(first.trailing_end().numerator_seconds() > first.nominal_end().numerator_seconds());
    assert_eq!(*first.nominal_end().denominator(), 20000);
    assert_eq!(*first.nominal_end().numerator_seconds(), 6000);
    let exact = SpeechExactDuration::new(
        *first.nominal_end().denominator(),
        *first.nominal_end().numerator_seconds(),
    )
    .unwrap();
    for (rate, frames) in [(8000, 2400), (48000, 14400)] {
        let projected = conduit_speech::timing::duration_at_rate(
            &SpeechDurationAtRateRequest::new(exact.clone(), rate).unwrap(),
        )
        .unwrap();
        assert_eq!(*projected.whole_frames(), frames);
        assert_eq!(projected.request().duration(), &exact);
    }
}

#[test]
fn resonance_and_energy_overlap_are_evaluated_by_the_shared_audio_source() {
    let doc = document();
    let phase_source = program(&doc, "speech/phone-gesture-phases");
    let curve_source = program(&doc, "speech/phone-gesture-curve");
    let first = phases(&phase_source, timing(100, 200, Some(100), Some(300)));
    let second = phases(&phase_source, timing(300, 300, Some(200), None));
    for (earlier, later) in [
        (
            AudioTrajectoryQuantity::frequency(1, 270).unwrap(),
            AudioTrajectoryQuantity::frequency(1, 2290).unwrap(),
        ),
        (
            AudioTrajectoryQuantity::amplitude(4, 1).unwrap(),
            AudioTrajectoryQuantity::amplitude(4, 3).unwrap(),
        ),
    ] {
        let mut results = Vec::new();
        for (phase, previous, current, next) in [
            (
                first.clone(),
                earlier.clone(),
                earlier.clone(),
                later.clone(),
            ),
            (
                second.clone(),
                earlier.clone(),
                later.clone(),
                later.clone(),
            ),
        ] {
            let request = SpeechPhoneGestureCurve::new(
                anchor(),
                current,
                next,
                phase,
                previous,
                provenance(),
            )
            .unwrap();
            let output = curve_source.evaluate(&request.encode().unwrap()).unwrap();
            let prepared = PreparedAudioQuantityTrajectory::new_with_linear_profile(
                &output,
                AudioLinearTrajectoryProfile::CommonBasisU32,
            )
            .unwrap();
            // Exact midpoint of the shared 250..375 ms overlap, in its retained basis.
            let query = AudioTrajectoryQuery::new(
                anchor(),
                AudioExactTimeOffset::new(20000, 6250).unwrap(),
            )
            .unwrap();
            let evaluated = prepared.evaluate(&query.encode().unwrap()).unwrap();
            results.push(evaluated.result().clone());
        }
        assert_eq!(results[0], results[1]);
        match &results[0] {
            AudioTrajectoryQuantity::Frequency(value) => {
                assert_eq!(*value.numerator_hz(), *value.denominator() * 1280);
            }
            AudioTrajectoryQuantity::Amplitude(value) => {
                assert_eq!(*value.numerator() * 2, *value.denominator());
            }
            _ => panic!("declared frequency or amplitude domain"),
        }
    }
}

#[test]
fn invalid_duration_foreign_fraction_basis_and_capacity_refuse_before_source() {
    let time = || AudioExactTimeOffset::new(1000, 100).unwrap();
    assert!(SpeechPhoneGestureTiming::new(duration(0), None, None, time()).is_err());
    assert!(SpeechPhoneGestureTiming::new(
        duration(100),
        None,
        Some(SpeechExactDuration::new(999, 10).unwrap()),
        time(),
    )
    .is_err());
    assert!(
        SpeechPhoneGestureTiming::new(duration(100), None, Some(duration(101)), time()).is_err()
    );
    assert!(SpeechPhoneGestureTiming::new(duration(65536), None, None, time()).is_err());
}
