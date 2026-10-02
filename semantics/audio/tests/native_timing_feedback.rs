use conduit_audio::{BeatReference, RhythmRecoveryState, TimingClassification, TimingFeedback};
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn complete_timing_feedback_family_round_trips_through_native_types() {
    for classification in [
        TimingClassification::OnTime,
        TimingClassification::Early,
        TimingClassification::Late,
        TimingClassification::Missed,
    ] {
        let structured = classification.into_structured().unwrap();
        assert_eq!(
            TimingClassification::from_structured(structured).unwrap(),
            classification
        );
    }
    for recovery in [
        RhythmRecoveryState::Interrupted,
        RhythmRecoveryState::Recovered,
        RhythmRecoveryState::OnBeat,
        RhythmRecoveryState::Recovering,
        RhythmRecoveryState::Displaced,
    ] {
        let structured = recovery.into_structured().unwrap();
        assert_eq!(
            RhythmRecoveryState::from_structured(structured).unwrap(),
            recovery
        );
    }

    let reference = BeatReference::new(u64::MAX, u64::MAX).unwrap();
    assert_eq!(
        BeatReference::from_structured(reference.into_structured().unwrap()).unwrap(),
        reference
    );

    let feedback = TimingFeedback::new(
        u64::MAX,
        TimingClassification::Early,
        i64::MIN,
        u64::MAX,
        true,
        0,
        RhythmRecoveryState::Recovering,
    )
    .unwrap();
    assert_eq!(
        TimingFeedback::from_structured(feedback.into_structured().unwrap()).unwrap(),
        feedback
    );
}

#[test]
fn timing_records_keep_the_exact_fields_and_primitive_bounds() {
    let beat = BeatReference::new(7, 1_000).unwrap();
    assert_eq!((beat.beat(), beat.expected_time_micros()), (7, 1_000));

    let feedback = TimingFeedback::new(
        7,
        TimingClassification::Late,
        25,
        1_000,
        false,
        0,
        RhythmRecoveryState::Interrupted,
    )
    .unwrap();
    assert_eq!(feedback.beat(), 7);
    assert_eq!(feedback.classification(), TimingClassification::Late);
    assert_eq!(feedback.delta_micros(), 25);
    assert_eq!(feedback.expected_time_micros(), 1_000);
    assert!(!feedback.observed());
    assert_eq!(feedback.observed_time_micros(), 0);
    assert_eq!(feedback.recovery_state(), RhythmRecoveryState::Interrupted);
}
