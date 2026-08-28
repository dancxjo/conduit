//! Narrow HID-local to portable key-event conversion.

use conduit_core::InfoDecodeError;
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};

pub fn portable_key_event(
    usage: u8,
    pressed: bool,
    modifiers_after: u8,
) -> Result<KeyEvent, InfoDecodeError> {
    KeyEvent::new(
        usage,
        if pressed {
            KeyTransition::Pressed
        } else {
            KeyTransition::Released
        },
        KeyModifiers::from_bits(modifiers_after),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_preserves_semantics_and_has_no_device_identity_output() {
        let pressed = portable_key_event(4, true, 0).unwrap();
        let released = portable_key_event(4, false, 0).unwrap();
        assert_eq!(pressed.encode(), [4, 0, 0]);
        assert_eq!(released.encode(), [4, 1, 0]);
        assert_eq!(pressed.encode().len(), conduit_human::KEY_EVENT_ENCODED_LEN);
    }

    #[test]
    fn usb_bridge_matches_every_shared_peer_conformance_value() {
        for vector in conduit_human::KEY_EVENT_CONFORMANCE_VECTORS {
            let value = portable_key_event(
                vector.encoded[0],
                vector.encoded[1] == KeyTransition::Pressed as u8,
                vector.encoded[2],
            )
            .unwrap();
            assert_eq!(value.encode(), vector.encoded, "{}", vector.name);
        }
    }

    #[test]
    fn invalid_hid_local_transition_cannot_become_a_portable_value() {
        assert!(portable_key_event(0, true, 0).is_err());
        assert!(portable_key_event(0xe1, true, 0).is_err());
    }
}
