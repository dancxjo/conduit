use conduit_core::InfoDecodeError;
use conduit_human::{
    KeyEvent, KeyModifiers, KeyTransition, KEY_EVENT_CONFORMANCE_VECTORS, KEY_EVENT_ENCODED_LEN,
    KEY_EVENT_INFO_ID,
};

fn event(usage: u8, transition: KeyTransition, modifiers: u8) -> KeyEvent {
    KeyEvent::new(usage, transition, KeyModifiers::from_bits(modifiers)).unwrap()
}

#[test]
fn canonical_vectors_pin_transition_and_modifier_after_semantics() {
    let vectors = [
        event(0x04, KeyTransition::Pressed, 0),
        event(0x04, KeyTransition::Released, 0),
        event(0xe1, KeyTransition::Pressed, 0x02),
        event(0x04, KeyTransition::Pressed, 0x02),
        event(0x04, KeyTransition::Released, 0x02),
        event(0xe1, KeyTransition::Released, 0),
        event(0xe5, KeyTransition::Pressed, 0x20),
        event(0x05, KeyTransition::Pressed, 0),
    ];
    for value in vectors {
        assert_eq!(KeyEvent::decode(&value.encode()), Ok(value));
        assert_eq!(value.encode().len(), KEY_EVENT_ENCODED_LEN);
    }
    assert_eq!(KEY_EVENT_INFO_ID, "input/key-event@1");
    assert_ne!(vectors[2], vectors[6]);
    assert_ne!(vectors[0].semantic_digest(), vectors[1].semantic_digest());
    for vector in KEY_EVENT_CONFORMANCE_VECTORS {
        KeyEvent::decode(&vector.encoded).expect(vector.name);
    }
    assert_eq!(KEY_EVENT_CONFORMANCE_VECTORS[6].encoded[0], 0x04);
    assert_eq!(KEY_EVENT_CONFORMANCE_VECTORS[7].encoded[0], 0x05);
}

#[test]
fn malformed_reserved_and_inconsistent_values_refuse() {
    assert!(matches!(
        KeyEvent::decode(&[0x04, 2, 0]),
        Err(InfoDecodeError::NonCanonicalEnum(2))
    ));
    assert!(matches!(
        KeyEvent::decode(&[0x04, 0]),
        Err(InfoDecodeError::WrongLength { .. })
    ));
    for usage in [0x00, 0x01, 0x03, 0xa5, 0xdf, 0xe8, 0xff] {
        assert!(matches!(
            KeyEvent::new(usage, KeyTransition::Pressed, KeyModifiers::NONE),
            Err(InfoDecodeError::ReservedValue { .. })
        ));
    }
    assert_ne!(KeyModifiers::LEFT_CONTROL, KeyModifiers::RIGHT_CONTROL);
    assert_ne!(KeyModifiers::LEFT_SHIFT, KeyModifiers::RIGHT_SHIFT);
    assert_ne!(KeyModifiers::LEFT_ALT, KeyModifiers::RIGHT_ALT);
    assert_ne!(KeyModifiers::LEFT_GUI, KeyModifiers::RIGHT_GUI);
    assert!(matches!(
        KeyEvent::new(0xe1, KeyTransition::Pressed, KeyModifiers::NONE),
        Err(InfoDecodeError::InconsistentValue(_))
    ));
    assert!(matches!(
        KeyEvent::new(0xe1, KeyTransition::Released, KeyModifiers::from_bits(0x02)),
        Err(InfoDecodeError::InconsistentValue(_))
    ));
}
