use conduit_core::{StructuredConfigurationValue, StructuredInfoValue};
use conduit_human::{
    default_keymap_configuration, keymap_layout_type, KeyEvent, KeyModifiers, KeyTransition,
    PreparedKeymap,
};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CanonicalStartupValue, KindSignature,
    StartupCatalog, StartupParameterSignature,
};

const SHIFT: u8 = 2;
const ALTGR: u8 = 64;
fn event(usage: u8, modifiers: u8) -> KeyEvent {
    KeyEvent::new(
        usage,
        KeyTransition::Pressed,
        KeyModifiers::from_bits(modifiers),
    )
    .unwrap()
}
fn apply(map: &mut PreparedKeymap, usage: u8, modifiers: u8) -> Option<String> {
    map.apply(event(usage, modifiers))
        .unwrap()
        .map(|value| String::from_utf8(value.as_slice().to_vec()).unwrap())
}
fn default_map() -> PreparedKeymap {
    PreparedKeymap::new(&default_keymap_configuration()).unwrap()
}
fn configuration(source: &str) -> Result<StructuredConfigurationValue, String> {
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_structured_type("KeymapLayout", keymap_layout_type())
        .unwrap();
    catalog
        .insert(KindSignature {
            kind: "input/keymap".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "layout".into(),
                value_type: "KeymapLayout".into(),
                default: None,
            }],
        })
        .unwrap();
    let syntax = parse_syntax_document(source);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("{:?}", syntax.diagnostics));
    }
    let checked = check_syntax_document(&syntax, &catalog).map_err(|e| format!("{e:?}"))?;
    let CanonicalStartupValue::Structured(value) =
        &checked.plots[0].gears[0].startup_bindings[0].value
    else {
        panic!("typed layout")
    };
    let value = value.try_concrete().unwrap();
    Ok(StructuredConfigurationValue::new(
        keymap_layout_type().profile().unwrap().value_kind().clone(),
        value.canonical_bytes().unwrap(),
    )
    .unwrap())
}
fn custom(action: &str) -> Result<StructuredConfigurationValue, String> {
    configuration(&format!("plot test {{\n map: input/keymap(layout = {{layers: [[{{usage:4,shift:false,altgr:false,action:{action}}}]],compositions:[]}})\n}}\n"))
}

#[test]
fn independent_us_ascii_reference_covers_every_base_and_shift_position() {
    let rows: &[(&[u8], &str, &str)] = &[
        (
            &[30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 45, 46],
            "1234567890-=",
            "!@#$%^&*()_+",
        ),
        (
            &[20, 26, 8, 21, 23, 28, 24, 12, 18, 19, 47, 48],
            "qwertyuiop[]",
            "QWERTYUIOP{}",
        ),
        (
            &[4, 22, 7, 9, 10, 11, 13, 14, 15, 51, 52],
            "asdfghjkl;'",
            "ASDFGHJKL:\"",
        ),
        (
            &[29, 27, 6, 25, 5, 17, 16, 54, 55, 56],
            "zxcvbnm,./",
            "ZXCVBNM<>?",
        ),
        (&[53, 49, 100], r"`\\", "~||"),
    ];
    let mut map = default_map();
    for &(usages, plain, shifted) in rows {
        for (modifiers, expected) in [(0, plain), (SHIFT, shifted)] {
            assert_eq!(usages.len(), expected.chars().count());
            for (&usage, character) in usages.iter().zip(expected.chars()) {
                assert_eq!(
                    apply(&mut map, usage, modifiers),
                    Some(character.to_string()),
                    "usage {usage}, modifiers {modifiers}"
                );
            }
        }
    }
    for modifiers in [0, SHIFT, ALTGR, SHIFT | ALTGR] {
        assert_eq!(apply(&mut map, 40, modifiers), Some("\n".into()));
        assert_eq!(apply(&mut map, 42, modifiers), Some("\u{8}".into()));
        assert_eq!(apply(&mut map, 43, modifiers), None);
        assert_eq!(apply(&mut map, 44, modifiers), Some(" ".into()));
    }
}

#[test]
fn independent_altgr_reference_pins_letters_punctuation_and_shift_levels() {
    let rows: &[(&[u8], &str, &str)] = &[
        (
            &[20, 26, 8, 21, 23, 28, 24, 12, 18, 19, 47, 48],
            "äåéëþüúíóö«»",
            "ÄÅÉËÞÜÚÍÓÖ“”",
        ),
        (
            &[4, 22, 7, 9, 10, 11, 13, 14, 15, 51],
            "áßðfghïœø¶",
            "Á§ÐFGHÏŒØ°",
        ),
        (
            &[29, 27, 6, 25, 17, 16, 54, 49, 100],
            "æœ©®ñµç¬\\",
            "ÆŒ¢™Ñ±Ç¦|",
        ),
    ];
    let mut map = default_map();
    for &(usages, plain, shifted) in rows {
        for (modifiers, expected) in [(ALTGR, plain), (SHIFT | ALTGR, shifted)] {
            assert_eq!(usages.len(), expected.chars().count());
            for (&usage, character) in usages.iter().zip(expected.chars()) {
                assert_eq!(
                    apply(&mut map, usage, modifiers),
                    Some(character.to_string()),
                    "usage {usage}, modifiers {modifiers}"
                );
            }
        }
    }
    for (usage, modifiers, text) in [
        (30, ALTGR, "¹"),
        (30, ALTGR | SHIFT, "¡"),
        (31, ALTGR, "²"),
        (32, ALTGR, "³"),
        (33, ALTGR, "¤"),
        (33, ALTGR | SHIFT, "£"),
        (34, ALTGR, "€"),
        (35, ALTGR | SHIFT, "¼"),
        (36, ALTGR | SHIFT, "½"),
        (37, ALTGR | SHIFT, "¾"),
        (38, ALTGR, "‘"),
        (39, ALTGR, "’"),
        (45, ALTGR, "¥"),
        (46, ALTGR, "×"),
        (46, ALTGR | SHIFT, "÷"),
        (5, ALTGR, "·"),
        (56, ALTGR, "¿"),
    ] {
        assert_eq!(apply(&mut map, usage, modifiers), Some(text.into()));
    }
}

#[test]
fn all_dead_families_compose_uppercase_and_spacing_independently() {
    // usage, shift on dead key, base usage, expected lowercase, uppercase, spacing.
    let cases = [
        (53, false, 4, "à", "À", "`"),
        (53, true, 17, "ñ", "Ñ", "~"),
        (52, false, 8, "é", "É", "´"),
        (52, true, 24, "ü", "Ü", "¨"),
        (35, false, 4, "â", "Â", "^"),
        (36, false, 18, "ơ", "Ơ", "̛"),
        (37, false, 4, "ą", "Ą", "˛"),
        (31, true, 18, "ő", "Ő", "˝"),
        (32, true, 4, "ā", "Ā", "¯"),
        (34, true, 6, "ç", "Ç", "¸"),
        (38, true, 4, "ă", "Ă", "˘"),
        (39, true, 4, "å", "Å", "˚"),
        (45, true, 4, "ạ", "Ạ", "̣"),
        (55, false, 29, "ż", "Ż", "˙"),
        (55, true, 22, "š", "Š", "ˇ"),
        (56, true, 4, "ả", "Ả", "̉"),
        (5, true, 15, "ł", "Ł", "/"),
    ];
    for (dead, shift, base, lower, upper, spacing) in cases {
        for (modifiers, expected) in [(0, lower), (SHIFT, upper)] {
            let mut map = default_map();
            assert_eq!(
                apply(&mut map, dead, ALTGR | if shift { SHIFT } else { 0 }),
                None
            );
            assert!(map.is_composing());
            assert_eq!(apply(&mut map, base, modifiers), Some(expected.into()));
            assert!(!map.is_composing());
        }
        let mut map = default_map();
        apply(&mut map, dead, ALTGR | if shift { SHIFT } else { 0 });
        assert_eq!(apply(&mut map, 44, 0), Some(spacing.into()));
        assert!(!map.is_composing());
    }
}

#[test]
fn pending_marks_survive_release_modifier_and_shortcut_events_and_cancel_explicitly() {
    let mut map = default_map();
    apply(&mut map, 52, ALTGR);
    assert!(map
        .apply(KeyEvent::new(52, KeyTransition::Released, KeyModifiers::NONE).unwrap())
        .unwrap()
        .is_none());
    assert_eq!(apply(&mut map, 230, ALTGR), None);
    assert_eq!(apply(&mut map, 8, 1), None);
    assert!(map.is_composing());
    assert_eq!(apply(&mut map, 8, 0), Some("é".into()));
    apply(&mut map, 52, ALTGR);
    assert_eq!(apply(&mut map, 52, ALTGR), Some("´".into()));
    assert!(!map.is_composing());
    apply(&mut map, 52, ALTGR);
    assert_eq!(apply(&mut map, 20, 0), Some("´q".into()));
    apply(&mut map, 52, ALTGR);
    assert_eq!(apply(&mut map, 42, 0), None);
    assert!(!map.is_composing());
    apply(&mut map, 52, ALTGR);
    assert_eq!(apply(&mut map, 41, 0), None);
    assert_eq!(apply(&mut map, 8, 0), Some("e".into()));
}

#[test]
fn conduitese_aliases_compose_layers_with_exact_override_and_unicode_text() {
    let config=configuration("plot test {\n base = [{usage:4,shift:false,altgr:false,action:text(\"a\")},{usage:8,shift:false,altgr:false,action:text(\"ơ\")} ]\n override = [{usage:4,shift:false,altgr:false,action:text(\"λ\")},{usage:52,shift:false,altgr:true,action:dead({spacing:\"´\",combining:769})}]\n layout = {layers:[base,override],compositions:[]}\n map: input/keymap(layout=layout)\n}\n").unwrap();
    let mut map = PreparedKeymap::new(&config).unwrap();
    assert_eq!(apply(&mut map, 4, 0), Some("λ".into()));
    apply(&mut map, 52, ALTGR);
    assert_eq!(apply(&mut map, 8, 0), Some("ớ".into()));
}

#[test]
fn preparation_refuses_non_scalar_marks_and_invalid_spacing() {
    for action in [
        "dead({spacing:\"´\",combining:55296})",
        "dead({spacing:\"´\",combining:1114112})",
        "dead({spacing:\"\",combining:769})",
        "dead({spacing:\"ab\",combining:769})",
    ] {
        let config = custom(action).unwrap();
        assert!(PreparedKeymap::new(&config).is_err(), "{action}");
    }
    let config = custom("text(\"abcdefgh\")").unwrap();
    let mut map = PreparedKeymap::new(&config).unwrap();
    assert_eq!(apply(&mut map, 4, 0), Some("abcdefgh".into()));
    if let Ok(config) = custom("text(\"abcdefghi\")") {
        assert!(PreparedKeymap::new(&config).is_err());
    }
}

#[test]
fn authored_bounds_refuse_excess_layers_bindings_and_composition_entries() {
    for layers in [
        "[]".to_owned(),
        "[[],[],[],[],[]]".to_owned(),
        format!(
            "[[{}]]",
            vec!["{usage:4,shift:false,altgr:false,action:none(empty)}"; 129].join(",")
        ),
    ] {
        assert!(configuration(&format!(
            "plot test {{\n map: input/keymap(layout={{layers:{layers},compositions:[]}})\n}}\n"
        ))
        .is_err());
    }
    let compositions = vec!["{combining:769,base:97,result:225}"; 65].join(",");
    assert!(configuration(&format!("plot test {{\n map: input/keymap(layout={{layers:[[]],compositions:[{compositions}]}})\n}}\n")).is_err());
    // Canonical configuration retains the exact finite layout Type.
    let value =
        StructuredInfoValue::from_canonical_bytes(default_keymap_configuration().canonical_value())
            .unwrap();
    assert_eq!(value.value_type(), &keymap_layout_type());
}

#[test]
fn stacked_dead_marks_compose_vietnamese_in_both_canonical_orders() {
    for order in [[(36, ALTGR), (52, ALTGR)], [(52, ALTGR), (36, ALTGR)]] {
        for (modifiers, expected) in [(0, "ớ"), (SHIFT, "Ớ")] {
            let mut map = default_map();
            for (usage, modifiers) in order {
                assert_eq!(apply(&mut map, usage, modifiers), None);
            }
            assert_eq!(apply(&mut map, 18, modifiers), Some(expected.into()));
        }
    }
    // Circumflex and belowdot have different canonical combining classes.
    for order in [
        [(35, ALTGR), (45, ALTGR | SHIFT)],
        [(45, ALTGR | SHIFT), (35, ALTGR)],
    ] {
        let mut map = default_map();
        for (usage, modifiers) in order {
            assert_eq!(apply(&mut map, usage, modifiers), None);
        }
        assert_eq!(apply(&mut map, 4, 0), Some("ậ".into()));
    }
}

#[test]
fn finite_pending_prefix_overflow_emits_spacing_and_restarts() {
    let mut map = default_map();
    for (usage, modifiers) in [(53, ALTGR), (52, ALTGR), (52, ALTGR | SHIFT), (35, ALTGR)] {
        assert_eq!(apply(&mut map, usage, modifiers), None);
    }
    assert_eq!(apply(&mut map, 53, ALTGR | SHIFT), Some("`´¨^".into()));
    assert!(map.is_composing());
    assert_eq!(apply(&mut map, 17, 0), Some("ñ".into()));
    assert!(!map.is_composing());
}

#[test]
fn static_default_and_prepared_authored_default_agree_for_all_portable_keys() {
    let mut prepared = default_map();
    let mut fixed = PreparedKeymap::default_layout();
    for usage in (4..=164).chain(224..=231) {
        // Modifier key events must retain their matching after-state.
        let modifiers = if usage >= 224 {
            1u8 << (usage - 224)
        } else {
            0
        };
        for additional in [0, SHIFT, ALTGR, SHIFT | ALTGR] {
            let modifiers = modifiers | additional;
            prepared.reset();
            fixed.reset();
            assert_eq!(
                apply(&mut prepared, usage, modifiers),
                apply(&mut fixed, usage, modifiers)
            );
            assert_eq!(apply(&mut prepared, 8, 0), apply(&mut fixed, 8, 0));
        }
    }
}

#[test]
fn authored_caps_lock_toggles_case_pairs_without_shifting_symbols() {
    let mut map = default_map();
    assert_eq!(apply(&mut map, 4, 0), Some("a".into()));
    assert_eq!(apply(&mut map, 57, 0), None);
    assert_eq!(apply(&mut map, 4, 0), Some("A".into()));
    assert_eq!(apply(&mut map, 4, SHIFT), Some("a".into()));
    assert_eq!(apply(&mut map, 4, ALTGR), Some("Á".into()));
    assert_eq!(apply(&mut map, 4, ALTGR | SHIFT), Some("á".into()));
    assert_eq!(apply(&mut map, 30, 0), Some("1".into()));
    assert_eq!(apply(&mut map, 30, SHIFT), Some("!".into()));
    assert_eq!(apply(&mut map, 22, ALTGR), Some("ß".into()));
    assert_eq!(apply(&mut map, 22, ALTGR | SHIFT), Some("§".into()));
    apply(&mut map, 52, ALTGR);
    assert_eq!(apply(&mut map, 8, 0), Some("É".into()));
    assert_eq!(apply(&mut map, 57, SHIFT | ALTGR), None);
    assert_eq!(apply(&mut map, 4, 0), Some("a".into()));
}

#[test]
fn authored_num_lock_and_shift_select_keypad_numeric_policy() {
    let mut map = default_map();
    for (usage, digit) in (89..=99).zip("1234567890.".chars()) {
        assert_eq!(apply(&mut map, usage, 0), None);
        assert_eq!(apply(&mut map, usage, ALTGR), None);
        assert_eq!(apply(&mut map, usage, SHIFT), Some(digit.to_string()));
        assert_eq!(
            apply(&mut map, usage, SHIFT | ALTGR),
            Some(digit.to_string())
        );
    }
    assert_eq!(apply(&mut map, 83, ALTGR | SHIFT), None);
    for (usage, digit) in (89..=99).zip("1234567890.".chars()) {
        assert_eq!(apply(&mut map, usage, 0), Some(digit.to_string()));
        assert_eq!(apply(&mut map, usage, ALTGR), Some(digit.to_string()));
        assert_eq!(apply(&mut map, usage, SHIFT), None);
        assert_eq!(apply(&mut map, usage, SHIFT | ALTGR), None);
    }
    assert!(map
        .apply(KeyEvent::new(83, KeyTransition::Released, KeyModifiers::NONE).unwrap())
        .unwrap()
        .is_none());
    assert_eq!(apply(&mut map, 89, 0), Some("1".into()));
    apply(&mut map, 83, 0);
    assert_eq!(apply(&mut map, 89, 0), None);
    for _ in 0..2 {
        for (usage, text) in [
            (84, "/"),
            (85, "*"),
            (86, "-"),
            (87, "+"),
            (88, "\n"),
            (103, "="),
        ] {
            for modifiers in [0, SHIFT, ALTGR, SHIFT | ALTGR] {
                assert_eq!(apply(&mut map, usage, modifiers), Some(text.into()));
            }
        }
        apply(&mut map, 83, 0);
    }
}

fn preset(bytes: &[u8]) -> PreparedKeymap {
    let config = StructuredConfigurationValue::new(
        keymap_layout_type().profile().unwrap().value_kind().clone(),
        bytes.to_vec(),
    )
    .unwrap();
    PreparedKeymap::new(&config).unwrap()
}
#[test]
fn authored_language_presets_pin_physical_positions_and_native_dead_keys() {
    use conduit_human::{
        FRENCH_KEYMAP_BYTES, GERMAN_KEYMAP_BYTES, SPANISH_KEYMAP_BYTES, UK_KEYMAP_BYTES,
    };
    let mut uk = preset(UK_KEYMAP_BYTES);
    for (usage, mods, text) in [
        (31, SHIFT, "\""),
        (32, SHIFT, "£"),
        (52, SHIFT, "@"),
        (49, 0, "#"),
        (49, SHIFT, "~"),
        (33, ALTGR, "€"),
        (100, 0, "\\"),
    ] {
        assert_eq!(apply(&mut uk, usage, mods), Some(text.into()));
    }
    apply(&mut uk, 52, ALTGR);
    assert_eq!(apply(&mut uk, 4, 0), Some("â".into()));
    let mut german = preset(GERMAN_KEYMAP_BYTES);
    for (usage, mods, text) in [
        (28, 0, "z"),
        (29, 0, "y"),
        (47, 0, "ü"),
        (51, 0, "ö"),
        (52, 0, "ä"),
        (45, 0, "ß"),
        (8, ALTGR, "€"),
        (20, ALTGR, "@"),
        (36, ALTGR, "{"),
    ] {
        assert_eq!(apply(&mut german, usage, mods), Some(text.into()));
    }
    apply(&mut german, 46, 0);
    assert_eq!(apply(&mut german, 8, 0), Some("é".into()));
    apply(&mut german, 83, 0);
    assert_eq!(apply(&mut german, 99, 0), Some(",".into()));
    let mut french = preset(FRENCH_KEYMAP_BYTES);
    for (usage, mods, text) in [
        (20, 0, "a"),
        (26, 0, "z"),
        (4, 0, "q"),
        (29, 0, "w"),
        (51, 0, "m"),
        (30, 0, "&"),
        (31, 0, "é"),
        (31, SHIFT, "2"),
        (36, ALTGR, "`"),
        (38, ALTGR, "^"),
        (8, ALTGR, "€"),
    ] {
        assert_eq!(apply(&mut french, usage, mods), Some(text.into()));
    }
    apply(&mut french, 47, 0);
    assert_eq!(apply(&mut french, 8, 0), Some("ê".into()));
    let mut spanish = preset(SPANISH_KEYMAP_BYTES);
    for (usage, mods, text) in [
        (51, 0, "ñ"),
        (49, 0, "ç"),
        (46, 0, "¡"),
        (46, SHIFT, "¿"),
        (53, 0, "º"),
        (53, SHIFT, "ª"),
        (31, ALTGR, "@"),
        (8, ALTGR, "€"),
    ] {
        assert_eq!(apply(&mut spanish, usage, mods), Some(text.into()));
    }
    apply(&mut spanish, 52, 0);
    assert_eq!(apply(&mut spanish, 4, 0), Some("á".into()));
}

#[test]
fn preset_special_caps_are_authored_without_changing_shift_symbols() {
    use conduit_human::{FRENCH_KEYMAP_BYTES, GERMAN_KEYMAP_BYTES};
    let mut french = preset(FRENCH_KEYMAP_BYTES);
    apply(&mut french, 57, 0);
    for (usage, upper, shifted) in [
        (31, "É", "2"),
        (36, "È", "7"),
        (38, "Ç", "9"),
        (39, "À", "0"),
        (52, "Ù", "%"),
    ] {
        assert_eq!(apply(&mut french, usage, 0), Some(upper.into()));
        assert_eq!(apply(&mut french, usage, SHIFT), Some(shifted.into()));
    }
    let mut german = preset(GERMAN_KEYMAP_BYTES);
    apply(&mut german, 57, 0);
    assert_eq!(apply(&mut german, 45, 0), Some("ẞ".into()));
    assert_eq!(apply(&mut german, 45, SHIFT), Some("?".into()));
}

#[test]
fn authored_registry_links_six_unique_named_layout_values() {
    use conduit_core::StructuredInfoValueShape as Shape;
    use conduit_human::{
        DEFAULT_KEYMAP_CHOICES_BYTES, DEFAULT_KEYMAP_CHOICES_LITERAL, KEYMAP_CHOICES_SOURCE,
    };
    let value = StructuredInfoValue::from_canonical_bytes(DEFAULT_KEYMAP_CHOICES_BYTES).unwrap();
    let Shape::Collection(choices) = value.shape() else {
        panic!("typed preset choices")
    };
    assert_eq!(choices.len(), 6);
    let mut identities = std::collections::BTreeSet::new();
    for choice in choices {
        let Shape::Record(fields) = choice.shape() else {
            panic!("choice record")
        };
        let identity = fields
            .iter()
            .find(|field| field.name() == "identity")
            .unwrap();
        let Shape::Leaf(bytes) = identity.value().shape() else {
            panic!("identity text")
        };
        assert!(identities.insert(std::str::from_utf8(bytes).unwrap().to_owned()));
        let layout = fields
            .iter()
            .find(|field| field.name() == "layout")
            .unwrap()
            .value();
        assert_eq!(layout.value_type(), &keymap_layout_type());
        let configuration = StructuredConfigurationValue::new(
            keymap_layout_type().profile().unwrap().value_kind().clone(),
            layout.canonical_bytes().unwrap(),
        )
        .unwrap();
        PreparedKeymap::new(&configuration).unwrap();
    }
    assert_eq!(
        identities,
        ["us-intl", "us", "uk", "german", "french", "spanish"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert!(DEFAULT_KEYMAP_CHOICES_LITERAL.contains("US International"));
    assert!(KEYMAP_CHOICES_SOURCE.contains("type KeymapChoices"));
}

#[test]
fn generated_type_only_blobs_preserve_the_exact_authored_value_types() {
    use conduit_human::{choices_type, default_keymap_value, DEFAULT_KEYMAP_CHOICES_BYTES};
    assert_eq!(&keymap_layout_type(), default_keymap_value().value_type());
    let choices = StructuredInfoValue::from_canonical_bytes(DEFAULT_KEYMAP_CHOICES_BYTES).unwrap();
    assert_eq!(&choices_type(), choices.value_type());
}
