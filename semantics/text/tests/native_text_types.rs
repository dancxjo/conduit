use conduit_form::rust_binding::NativeRustBinding;
use conduit_text::{AddressDetectionRefusal, MorseKeyPhase};

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
