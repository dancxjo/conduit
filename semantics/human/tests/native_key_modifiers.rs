use conduit_form::rust_binding::NativeRustBinding;
use conduit_human::KeyModifiers;

#[test]
fn key_modifiers_own_the_full_native_bit_domain_and_legacy_const_api() {
    for bits in [0, 1, 0x55, 0xaa, u8::MAX] {
        let modifiers = KeyModifiers::from_bits(bits);
        assert_eq!(modifiers.bits(), bits);
        let structured = modifiers.into_structured().unwrap();
        assert_eq!(
            KeyModifiers::from_structured(structured).unwrap(),
            modifiers
        );
    }

    assert_eq!(KeyModifiers::NONE.bits(), 0);
    assert_eq!(KeyModifiers::LEFT_CONTROL.bits(), 1);
    assert_eq!(KeyModifiers::RIGHT_GUI.bits(), 1 << 7);
}
