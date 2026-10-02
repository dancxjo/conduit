use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn keyboard_occurrences_round_trip_through_exact_native_types() {
    let event = KeyEvent::new(0x04, KeyTransition::Pressed, KeyModifiers::LEFT_SHIFT).unwrap();
    let structured = event.into_structured().unwrap();
    assert_eq!(KeyEvent::from_structured(structured).unwrap(), event);
    assert_eq!(KeyEvent::decode(&event.encode()), Ok(event));
}

#[test]
fn native_keyboard_contracts_preserve_usage_and_modifier_laws() {
    let native = |usage, transition, left_control| {
        KeyEvent::new_native(
            usage,
            transition,
            left_control,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
        )
    };
    assert!(native(3, KeyTransition::Pressed, false).is_err());
    assert!(native(165, KeyTransition::Pressed, false).is_err());
    assert!(native(224, KeyTransition::Pressed, false).is_err());
    assert!(native(224, KeyTransition::Released, true).is_err());
    assert!(native(224, KeyTransition::Pressed, true).is_ok());
}

#[test]
fn keyboard_occurrences_have_no_handwritten_duplicate_records() {
    let event_source = include_str!("../src/key_event.rs");
    assert!(!event_source.contains("pub struct KeyEvent {"));

    let types = include_str!("../types.conduit");
    assert!(types.contains("type KeyEvent ="));
}
