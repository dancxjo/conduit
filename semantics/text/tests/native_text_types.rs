use conduit_form::rust_binding::NativeRustBinding;
use conduit_text::MorseKeyPhase;

#[test]
fn morse_key_phase_round_trips_through_its_exact_native_type() {
    for phase in [MorseKeyPhase::Pressed, MorseKeyPhase::Released] {
        let structured = phase.into_structured().unwrap();
        assert_eq!(structured.value_type(), &MorseKeyPhase::semantic_type().unwrap());
        assert_eq!(MorseKeyPhase::from_structured(structured).unwrap(), phase);
    }
}
