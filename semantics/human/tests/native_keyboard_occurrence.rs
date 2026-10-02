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

#[test]
fn checked_keyboard_constructor_matches_every_native_usage_and_modifier_law() {
    let usages = [
        0, 3, 4, 164, 165, 223, 224, 225, 226, 227, 228, 229, 230, 231, 232, 255,
    ];
    let modifiers = [0, 255, 1, 2, 4, 8, 16, 32, 64, 128];
    for usage in usages {
        for transition in [KeyTransition::Pressed, KeyTransition::Released] {
            for bits in modifiers {
                let checked = KeyEvent::new(usage, transition, KeyModifiers::from_bits(bits));
                let native = KeyEvent::new_native(
                    usage,
                    transition,
                    bits & 1 != 0,
                    bits & 2 != 0,
                    bits & 4 != 0,
                    bits & 8 != 0,
                    bits & 16 != 0,
                    bits & 32 != 0,
                    bits & 64 != 0,
                    bits & 128 != 0,
                );
                assert_eq!(
                    checked.is_ok(),
                    native.is_ok(),
                    "usage={usage} transition={transition:?} bits={bits}"
                );
                if let (Ok(checked), Ok(native)) = (checked, native) {
                    assert_eq!(checked, native);
                }
            }
        }
    }
}

#[test]
fn rust_character_adapter_preserves_native_scalar_bounds() {
    use conduit_human::TextFragment;
    for value in [
        '\0',
        'a',
        '\u{7f}',
        '\u{80}',
        '\u{7ff}',
        '\u{800}',
        '\u{d7ff}',
        '\u{e000}',
        '\u{ffff}',
        '\u{10000}',
        '\u{10ffff}',
    ] {
        let adapted = TextFragment::from_char(value);
        let native = if u32::from(value) <= 0xd7ff {
            TextFragment::basic(u32::from(value))
        } else {
            TextFragment::supplementary(u32::from(value))
        }
        .unwrap();
        assert_eq!(adapted, native);
        let mut actual = [0; 4];
        let mut expected = [0; 4];
        assert_eq!(
            adapted.encode_utf8(&mut actual),
            value.encode_utf8(&mut expected).as_bytes()
        );
    }
    for invalid in [0xd800, 0xdfff, 0x110000] {
        assert!(char::from_u32(invalid).is_none());
        assert!(TextFragment::basic(invalid).is_err());
        assert!(TextFragment::supplementary(invalid).is_err());
    }
}
