//! Translation fixtures prove typed payloads, not physical input acquisition.
use super::*;
#[test]
fn hid_transitions_preserve_side_specific_modifiers_and_refuse_inconsistent_keys() {
    let expected = KeyEvent::new(
        4,
        KeyTransition::Pressed,
        KeyModifiers::from_bits(
            KeyModifiers::LEFT_SHIFT.bits() | KeyModifiers::RIGHT_CONTROL.bits(),
        ),
    )
    .unwrap();
    assert_eq!(
        key(
            Some(Scancode::A),
            Mod::LSHIFTMOD | Mod::RCTRLMOD,
            KeyTransition::Pressed
        ),
        Ok(NativeInput::Key(expected))
    );
    assert!(matches!(
        key(None, Mod::NOMOD, KeyTransition::Pressed),
        Err(NativeInputRefusal::InvalidKey { scancode: None })
    ));
    assert!(key(Some(Scancode::LShift), Mod::NOMOD, KeyTransition::Pressed).is_err());
    assert!(key(
        Some(Scancode::LShift),
        Mod::LSHIFTMOD,
        KeyTransition::Released
    )
    .is_err());
    assert!(key(Some(Scancode::LShift), Mod::NOMOD, KeyTransition::Released).is_ok());
    assert!(key(Some(Scancode::Mode), Mod::NOMOD, KeyTransition::Pressed).is_err());
}

#[test]
fn held_key_byte_view_checks_full_extent_without_reserved_enum_conversions() {
    assert!(held_key_bytes(&[]).is_err());
    let mut keys = [0u8; sdl2::sys::SDL_Scancode::SDL_NUM_SCANCODES as usize];
    assert_eq!(held_key_bytes(&keys), Ok(false));
    keys[1] = 1;
    assert_eq!(held_key_bytes(&keys), Ok(true));
    keys[1] = 0;
    let end = keys.len() - 1;
    keys[end] = 1;
    assert_eq!(held_key_bytes(&keys), Ok(true));
    assert!(held_key_bytes(&[0; 513]).is_err());
    assert!(held_key_bytes(&[2]).is_err());
}
