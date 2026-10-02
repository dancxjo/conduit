use conduit_form::rust_binding::NativeRustBinding;
use conduit_text::{
    AddressDetection, AddressDetectionRefusal, MorseKeyPhase, MorseKeyTransition, MorsePattern,
    MorseSegment,
};

#[test]
fn morse_key_phase_round_trips_through_its_exact_native_type() {
    for phase in [MorseKeyPhase::Pressed, MorseKeyPhase::Released] {
        let structured = phase.into_structured().unwrap();
        assert_eq!(
            structured.value_type(),
            &MorseKeyPhase::semantic_type().unwrap()
        );
        assert_eq!(MorseKeyPhase::from_structured(structured).unwrap(), phase);
    }
}

#[test]
fn morse_pattern_is_one_bounded_native_value() {
    let pattern = MorsePattern::from_text("SOS", 120).unwrap();
    let structured = pattern.clone().into_structured().unwrap();
    assert_eq!(MorsePattern::from_structured(structured), Ok(pattern));
}

#[test]
fn morse_segment_round_trips_through_its_exact_native_record() {
    let segment = MorseSegment::new(true, 3).unwrap();
    let structured = segment.clone().into_structured().unwrap();
    assert_eq!(
        structured.value_type(),
        &MorseSegment::semantic_type().unwrap()
    );
    assert_eq!(MorseSegment::from_structured(structured).unwrap(), segment);
}

#[test]
fn morse_key_transition_round_trips_through_its_exact_native_record() {
    let transition = MorseKeyTransition::new(
        "fixture/boot-monotonic-us@1".into(),
        1_250_000,
        MorseKeyPhase::Released,
        7,
    )
    .unwrap();
    let structured = transition.clone().into_structured().unwrap();
    assert_eq!(
        structured.value_type(),
        &MorseKeyTransition::semantic_type().unwrap()
    );
    assert_eq!(
        MorseKeyTransition::from_structured(structured).unwrap(),
        transition
    );
    assert!(MorseKeyTransition::new("x".repeat(97), 0, MorseKeyPhase::Pressed, 0,).is_err());
}

#[test]
fn address_detection_terminal_round_trips_through_its_exact_native_type() {
    let refusal = AddressDetectionRefusal::RecognizedTextTooLarge;
    let structured = refusal.into_structured().unwrap();
    assert_eq!(
        structured.value_type(),
        &AddressDetectionRefusal::semantic_type().unwrap()
    );
    assert_eq!(
        AddressDetectionRefusal::from_structured(structured).unwrap(),
        refusal
    );
}

#[test]
fn address_detection_value_round_trips_and_owns_its_bounds() {
    for detection in [
        AddressDetection::NotAddressed,
        AddressDetection::addressed(7, "status".into()).unwrap(),
    ] {
        let structured = detection.clone().into_structured().unwrap();
        assert_eq!(
            structured.value_type(),
            &AddressDetection::semantic_type().unwrap()
        );
        assert_eq!(
            AddressDetection::from_structured(structured).unwrap(),
            detection
        );
    }

    assert!(AddressDetection::addressed(8, "status".into()).is_err());
    assert!(AddressDetection::addressed(0, "x".repeat(257)).is_err());
}
