use conduit_form::rust_binding::NativeRustBinding;
use conduit_tutorial_form::TutorialPlayback;

#[test]
fn every_tutorial_playback_phase_round_trips_through_its_native_type() {
    for playback in [
        TutorialPlayback::Lulled,
        TutorialPlayback::Preparing,
        TutorialPlayback::Playing,
        TutorialPlayback::Idle,
        TutorialPlayback::Completed,
        TutorialPlayback::Cancelled,
        TutorialPlayback::Failed,
        TutorialPlayback::Refused,
        TutorialPlayback::Stopped,
        TutorialPlayback::Fulfilled,
    ] {
        let structured = playback.into_structured().unwrap();
        assert_eq!(
            TutorialPlayback::from_structured(structured).unwrap(),
            playback
        );
    }
}
