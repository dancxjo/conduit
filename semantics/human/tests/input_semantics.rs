use conduit_human::{
    ChordInfo, ConduitIntlKeymap, CoreChordId, KeyEvent, KeyModifiers, KeyTransition,
    KeymapDisposition, KeymapRefusal,
};

fn event(usage: u8, transition: KeyTransition, modifiers: u8) -> KeyEvent {
    KeyEvent::new(usage, transition, KeyModifiers::from_bits(modifiers)).unwrap()
}

fn press(usage: u8, modifiers: u8) -> KeyEvent {
    event(usage, KeyTransition::Pressed, modifiers)
}

fn text(value: KeymapDisposition) -> String {
    match value {
        KeymapDisposition::Text(value) => String::from_utf8(value.as_bytes().to_vec()).unwrap(),
        other => panic!("expected text, found {other:?}"),
    }
}

fn arm_compose(map: &mut ConduitIntlKeymap) {
    assert_eq!(
        map.apply(press(0xe7, KeyModifiers::RIGHT_GUI.bits())),
        KeymapDisposition::NoText
    );
    assert_eq!(
        map.apply(event(0xe7, KeyTransition::Released, 0)),
        KeymapDisposition::NoText
    );
}

fn start_unicode(map: &mut ConduitIntlKeymap) {
    map.apply(press(0xe7, KeyModifiers::RIGHT_GUI.bits()));
    map.apply(press(0x18, KeyModifiers::RIGHT_GUI.bits()));
    map.apply(event(0xe7, KeyTransition::Released, 0));
}

#[test]
fn reviewed_altgr_table_is_byte_exact() {
    let cases = [
        (0x04, false, "æ"),
        (0x12, false, "ø"),
        (0x16, false, "ß"),
        (0x07, false, "ð"),
        (0x13, false, "þ"),
        (0x11, false, "ñ"),
        (0x08, false, "€"),
        (0x0f, false, "£"),
        (0x1c, false, "¥"),
        (0x06, false, "©"),
        (0x15, false, "®"),
        (0x17, false, "™"),
        (0x1e, true, "¡"),
        (0x38, true, "¿"),
        (0x2d, false, "–"),
        (0x2d, true, "—"),
    ];
    for (usage, shifted, expected) in cases {
        let mut map = ConduitIntlKeymap::new();
        let modifiers = KeyModifiers::RIGHT_ALT.bits()
            | if shifted {
                KeyModifiers::LEFT_SHIFT.bits()
            } else {
                0
            };
        assert_eq!(text(map.apply(press(usage, modifiers))), expected);
    }
}

#[test]
fn reviewed_compose_table_includes_uppercase_and_fallback_prefix() {
    let cases = [
        (0x34, false, 0x08, false, "é"),
        (0x35, false, 0x08, false, "è"),
        (0x23, true, 0x08, false, "ê"),
        (0x34, true, 0x08, true, "Ë"),
        (0x35, true, 0x11, false, "ñ"),
        (0x36, false, 0x06, false, "ç"),
        (0x12, false, 0x04, false, "å"),
        (0x38, false, 0x12, true, "Ø"),
    ];
    for (first, first_shifted, second, second_shifted, expected) in cases {
        let mut map = ConduitIntlKeymap::new();
        arm_compose(&mut map);
        let first_modifiers = if first_shifted {
            KeyModifiers::LEFT_SHIFT.bits()
        } else {
            0
        };
        assert_eq!(
            map.apply(press(first, first_modifiers)),
            KeymapDisposition::NoText
        );
        let second_modifiers = if second_shifted {
            KeyModifiers::LEFT_SHIFT.bits()
        } else {
            0
        };
        assert_eq!(text(map.apply(press(second, second_modifiers))), expected);
    }

    let mut fallback = ConduitIntlKeymap::new();
    assert_eq!(
        fallback.apply(press(0x2c, KeyModifiers::RIGHT_ALT.bits())),
        KeymapDisposition::NoText
    );
    fallback.apply(press(0x34, 0));
    assert_eq!(text(fallback.apply(press(0x08, 0))), "é");
}

#[test]
fn unicode_invalid_empty_overflow_and_cancel_reset_without_retained_growth() {
    let mut map = ConduitIntlKeymap::new();
    start_unicode(&mut map);
    assert_eq!(
        map.apply(press(0x28, 0)),
        KeymapDisposition::Refused(KeymapRefusal::EmptyUnicodeEntry)
    );
    assert_eq!(text(map.apply(press(0x04, 0))), "a");

    start_unicode(&mut map);
    for usage in [0x07, 0x25, 0x27, 0x27] {
        map.apply(press(usage, 0));
    }
    assert_eq!(
        map.apply(press(0x28, 0)),
        KeymapDisposition::Refused(KeymapRefusal::InvalidUnicodeScalar)
    );

    start_unicode(&mut map);
    for _ in 0..6 {
        assert_eq!(map.apply(press(0x27, 0)), KeymapDisposition::NoText);
    }
    assert_eq!(
        map.apply(press(0x27, 0)),
        KeymapDisposition::Refused(KeymapRefusal::UnicodeEntryOverflow)
    );

    arm_compose(&mut map);
    map.apply(press(0x34, 0));
    assert_eq!(map.apply(press(0x29, 0)), KeymapDisposition::Cancelled);
    assert_eq!(text(map.apply(press(0x05, 0))), "b");
}

#[test]
fn chord_table_emits_one_trigger_and_preserves_modifier_identity() {
    let cases = [
        (
            KeyModifiers::LEFT_CONTROL.bits(),
            0x0a,
            CoreChordId::CancelOrEscape,
        ),
        (
            KeyModifiers::RIGHT_CONTROL.bits(),
            0x0f,
            CoreChordId::ClearOrRefresh,
        ),
        (
            KeyModifiers::LEFT_CONTROL.bits(),
            0x15,
            CoreChordId::RepeatOrReplan,
        ),
        (KeyModifiers::LEFT_ALT.bits(), 0x13, CoreChordId::Palette),
        (KeyModifiers::LEFT_ALT.bits(), 0x0c, CoreChordId::Inspect),
        (KeyModifiers::LEFT_GUI.bits(), 0x13, CoreChordId::Plan),
        (KeyModifiers::LEFT_GUI.bits(), 0x2c, CoreChordId::Command),
        (KeyModifiers::LEFT_GUI.bits(), 0x28, CoreChordId::Activate),
    ];
    for (modifiers, usage, expected) in cases {
        let pressed = ChordInfo::from_key_event(press(usage, modifiers)).unwrap();
        assert_eq!(pressed.chord_id(), expected);
        assert_eq!(pressed.modifiers().bits(), modifiers);
        assert_eq!(ChordInfo::decode(&pressed.encode()), Ok(pressed));
        assert!(
            ChordInfo::from_key_event(event(usage, KeyTransition::Released, modifiers)).is_none()
        );
    }
    assert!(ChordInfo::from_key_event(press(0x08, KeyModifiers::RIGHT_ALT.bits())).is_none());
    assert!(ChordInfo::from_key_event(press(0x13, KeyModifiers::RIGHT_GUI.bits())).is_none());
    assert!(ChordInfo::from_key_event(press(
        0x13,
        KeyModifiers::LEFT_ALT.bits() | KeyModifiers::LEFT_GUI.bits()
    ))
    .is_none());
}
