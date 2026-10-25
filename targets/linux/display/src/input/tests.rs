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
