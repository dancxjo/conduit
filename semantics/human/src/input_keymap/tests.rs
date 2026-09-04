use super::*;

fn press(usage: u8, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(usage, KeyTransition::Pressed, modifiers).unwrap()
}

fn release(usage: u8, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(usage, KeyTransition::Released, modifiers).unwrap()
}

fn text(disposition: KeymapDisposition) -> alloc::string::String {
    match disposition {
        KeymapDisposition::Text(value) => {
            alloc::string::String::from_utf8(value.as_bytes().to_vec()).unwrap()
        }
        other => panic!("expected text, found {other:?}"),
    }
}

#[test]
fn base_shift_altgr_and_non_text_are_exact() {
    let mut map = ConduitIntlKeymap::new();
    assert_eq!(text(map.apply(press(0x04, KeyModifiers::NONE))), "a");
    assert_eq!(text(map.apply(press(0x04, KeyModifiers::LEFT_SHIFT))), "A");
    assert_eq!(text(map.apply(press(0x08, KeyModifiers::RIGHT_ALT))), "€");
    assert_eq!(text(map.apply(press(0x2d, KeyModifiers::RIGHT_ALT))), "–");
    assert_eq!(
        text(map.apply(press(0x2d, KeyModifiers::from_bits(0x42)))),
        "—"
    );
    assert_eq!(
        map.apply(press(0x04, KeyModifiers::LEFT_CONTROL)),
        KeymapDisposition::NoText
    );
    assert_eq!(
        map.apply(press(0x04, KeyModifiers::LEFT_ALT)),
        KeymapDisposition::NoText
    );
    assert_eq!(
        map.apply(press(0x4f, KeyModifiers::NONE)),
        KeymapDisposition::NoText
    );
    assert_eq!(
        map.apply(release(0x04, KeyModifiers::NONE)),
        KeymapDisposition::NoText
    );
}

#[test]
fn compose_and_unicode_entry_are_finite_and_reset() {
    let mut map = ConduitIntlKeymap::new();
    assert_eq!(
        map.apply(press(0xe7, KeyModifiers::RIGHT_GUI)),
        KeymapDisposition::NoText
    );
    assert_eq!(
        map.apply(release(0xe7, KeyModifiers::NONE)),
        KeymapDisposition::NoText
    );
    assert_eq!(
        map.apply(press(0x34, KeyModifiers::NONE)),
        KeymapDisposition::NoText
    );
    assert_eq!(text(map.apply(press(0x08, KeyModifiers::NONE))), "é");

    assert_eq!(
        map.apply(press(0xe7, KeyModifiers::RIGHT_GUI)),
        KeymapDisposition::NoText
    );
    assert_eq!(
        map.apply(press(0x18, KeyModifiers::RIGHT_GUI)),
        KeymapDisposition::NoText
    );
    assert_eq!(
        map.apply(release(0xe7, KeyModifiers::NONE)),
        KeymapDisposition::NoText
    );
    for usage in [0x27, 0x20, 0x05, 0x05] {
        assert_eq!(
            map.apply(press(usage, KeyModifiers::NONE)),
            KeymapDisposition::NoText
        );
    }
    assert_eq!(text(map.apply(press(0x28, KeyModifiers::NONE))), "λ");

    assert_eq!(
        map.apply(press(0xe7, KeyModifiers::RIGHT_GUI)),
        KeymapDisposition::NoText
    );
    assert_eq!(
        map.apply(press(0x18, KeyModifiers::RIGHT_GUI)),
        KeymapDisposition::NoText
    );
    assert_eq!(
        map.apply(release(0xe7, KeyModifiers::NONE)),
        KeymapDisposition::NoText
    );
    for usage in [0x1e, 0x09, 0x26, 0x08, 0x27] {
        assert_eq!(
            map.apply(press(usage, KeyModifiers::NONE)),
            KeymapDisposition::NoText
        );
    }
    assert_eq!(text(map.apply(press(0x28, KeyModifiers::NONE))), "🧠");

    assert_eq!(
        map.apply(press(0xe7, KeyModifiers::RIGHT_GUI)),
        KeymapDisposition::NoText
    );
    assert_eq!(
        map.apply(press(0x18, KeyModifiers::RIGHT_GUI)),
        KeymapDisposition::NoText
    );
    assert_eq!(
        map.apply(release(0xe7, KeyModifiers::NONE)),
        KeymapDisposition::NoText
    );
    for usage in [0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e] {
        let result = map.apply(press(usage, KeyModifiers::NONE));
        if matches!(
            result,
            KeymapDisposition::Refused(KeymapRefusal::UnicodeEntryOverflow)
        ) {
            break;
        }
    }
    assert_eq!(text(map.apply(press(0x04, KeyModifiers::NONE))), "a");
}
