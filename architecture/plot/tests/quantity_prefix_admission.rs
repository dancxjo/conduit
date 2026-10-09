use conduit_core::{Quantity, QuantityUnit};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CanonicalStartupValue, StartupCatalog,
};

#[test]
fn reviewed_composed_suffixes_check_as_existing_quantity_values_without_source_rewrite() {
    let source = "plot prefixes {\n distance = 1dam\n tiny_area = 1dam2\n old_pitch = 440Hz\n old_delay = 250ms\n}\n";
    let parsed = parse_syntax_document(source);
    assert_eq!(parsed.round_trip(), source);
    let checked = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap();
    for (name, literal) in [
        ("distance", "1dam"),
        ("tiny_area", "1dam2"),
        ("old_pitch", "440Hz"),
        ("old_delay", "250ms"),
    ] {
        assert!(checked.plots[0]
            .local_values
            .iter()
            .any(|(actual_name, value)| actual_name == name
                && value
                    == &CanonicalStartupValue::Quantity(
                        Quantity::parse_plot_literal(literal).unwrap()
                    )));
    }
    assert_eq!(
        Quantity::parse_plot_literal("440Hz"),
        Ok(Quantity::new(440, QuantityUnit::Hertz))
    );
    assert_eq!(
        Quantity::parse_plot_literal("250ms"),
        Ok(Quantity::new(250, QuantityUnit::Millisecond))
    );
}

#[test]
fn recognized_extreme_units_refuse_the_selected_legacy_profile_at_original_source_span() {
    for literal in ["1Qm", "1qm", "1Qm³", "1qm³", "-1Qm", "-1qm"] {
        let source = format!("plot prefixes {{\n # Original Unicode survives before the token: µ\n distance = {literal}\n}}\n");
        let parsed = parse_syntax_document(&source);
        assert_eq!(parsed.round_trip(), source);
        let refusal = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap_err();
        assert_eq!(refusal.code, "CND-FRM-055", "{}", refusal.message);
        assert!(
            refusal.message.contains("RepresentationIneligible"),
            "{}",
            refusal.message
        );
        assert!(refusal.message.contains("value/quantity"));
        assert!(!refusal.message.contains("UnknownUnit"));
        assert_eq!(&source[refusal.span.start..refusal.span.end], literal);
    }
}

#[test]
fn default_and_highlighting_preserve_known_unit_identity_and_original_bytes() {
    use conduit_plot::{highlight_syntax, SyntaxHighlightKind};
    let source = "plot prefixes (\n distance: Distance = 1Qm\n) {\n}\n";
    let parsed = parse_syntax_document(source);
    let refusal = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap_err();
    assert_eq!(refusal.code, "CND-FRM-055");
    assert!(refusal.message.contains("RepresentationIneligible"));
    assert_eq!(&source[refusal.span.start..refusal.span.end], "1Qm");
    assert_eq!(parsed.round_trip(), source);
    let highlighted = highlight_syntax(source).unwrap();
    assert!(highlighted
        .iter()
        .any(|item| item.kind == SyntaxHighlightKind::Number
            && &source[item.start..item.end] == "1Qm"));
}
