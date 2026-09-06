use conduit_form::{check_syntax_document, expand_canonical_form, parse_syntax_document};
use conduit_semantic_catalog::{
    install_button_indicator_catalogs, BUTTON_INDICATOR_STATE_KIND, BUTTON_SOURCE_KIND,
    INDICATOR_STATE_PRESENTATION_KIND,
};

const SOURCE: &str = include_str!("../../../forms/button-across-room/main.conduit");

#[test]
fn canonical_form_is_only_the_semantic_button_to_indicator_chain() {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    install_button_indicator_catalogs(&mut startup, &mut profile).unwrap();

    let parsed = parse_syntax_document(SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "button_across_room", &profile).unwrap();

    assert_eq!(expanded.gears.len(), 3);
    assert_eq!(expanded.connections.len(), 2);
    let mut kinds = expanded
        .gears
        .iter()
        .map(|gear| gear.kind_id.as_str())
        .collect::<Vec<_>>();
    kinds.sort_unstable();
    let mut expected = vec![
        BUTTON_SOURCE_KIND,
        BUTTON_INDICATOR_STATE_KIND,
        INDICATOR_STATE_PRESENTATION_KIND,
    ];
    expected.sort_unstable();
    assert_eq!(kinds, expected);

    let source = SOURCE.to_ascii_lowercase();
    for forbidden in [
        "host",
        "device",
        "base",
        "resource",
        "line",
        "transport",
        "socket",
        "address",
        "dom",
        "gpio",
        "board",
        "pin",
        "browser",
        "usb",
    ] {
        assert!(
            !source.contains(forbidden),
            "authored Form leaked {forbidden}"
        );
    }
}

#[test]
fn authored_button_transition_count_is_finite_and_defaults_to_one_press_release() {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    install_button_indicator_catalogs(&mut startup, &mut profile).unwrap();
    for (argument, expected) in [
        ("", 2),
        ("(maximum-transitions = 1)", 1),
        ("(maximum-transitions = 5)", 5),
        ("(maximum-transitions = 8)", 8),
    ] {
        let source = format!(
            "form bounded {{
 button: input/button{argument}
}}"
        );
        let parsed = parse_syntax_document(&source);
        let checked = check_syntax_document(&parsed, &startup).unwrap();
        let expanded = expand_canonical_form(&checked, "bounded", &profile).unwrap();
        assert_eq!(
            expanded.gears[0].configuration[0].value,
            conduit_core::ConfigurationValue::U64(expected)
        );
    }
    for count in [0, 9, u64::MAX] {
        let source = format!(
            "form bounded {{
 button: input/button(maximum-transitions = {count})
}}"
        );
        let parsed = parse_syntax_document(&source);
        let checked = check_syntax_document(&parsed, &startup).unwrap();
        assert!(
            expand_canonical_form(&checked, "bounded", &profile).is_err(),
            "out-of-bound count {count} must refuse before Play"
        );
    }
}
