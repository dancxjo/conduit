use conduit_core::{Quantity, Unit};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CanonicalStartupValue, StartupCatalog,
};

#[test]
fn reviewed_composed_suffixes_check_as_existing_quantity_values_without_source_rewrite() {
    let source = "plot prefixes {\n distance = 1dam\n tiny_area = 1dam²\n old_pitch = 440Hz\n old_delay = 250ms\n}\n";
    let parsed = parse_syntax_document(source);
    assert_eq!(parsed.round_trip(), source);
    let checked = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap();
    for (name, literal) in [
        ("distance", "1dam"),
        ("tiny_area", "1dam²"),
        ("old_pitch", "440Hz"),
        ("old_delay", "250ms"),
    ] {
        assert!(checked.plots[0]
            .local_values
            .iter()
            .any(|(actual_name, value)| actual_name == name
                && value
                    == &CanonicalStartupValue::Quantity(
                        conduit_core::QuantityConfigurationValue::parse(literal).unwrap()
                    )));
    }
    assert_eq!(
        Quantity::parse_plot_literal("440Hz"),
        Ok(Quantity::new(440, Unit::Hertz))
    );
    assert_eq!(
        Quantity::parse_plot_literal("250ms"),
        Ok(Quantity::new(250, Unit::Millisecond))
    );
}

#[test]
fn recognized_extreme_units_use_the_canonical_quantity_without_source_rewrite() {
    for literal in ["1Qm", "1qm", "1Qm³", "1qm³", "-1Qm", "-1qm"] {
        let source = format!("plot prefixes {{\n # Unicode: µ\n distance = {literal}\n}}\n");
        let parsed = parse_syntax_document(&source);
        let checked = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap();
        let value = &checked.plots[0].local_values[0].1;
        let CanonicalStartupValue::Quantity(value) = value else {
            panic!("Quantity");
        };
        assert_eq!(value.source(), literal);
        assert_eq!(
            value.value().encode().len(),
            conduit_core::QUANTITY_ENCODED_LEN
        );
        assert_eq!(parsed.round_trip(), source);
    }
}

#[test]
fn default_and_highlighting_preserve_known_unit_identity_and_original_bytes() {
    use conduit_plot::{highlight_syntax, SyntaxHighlightKind};
    let source = "plot prefixes (\n distance: Distance = 1Qm\n) {\n}\n";
    let parsed = parse_syntax_document(source);
    let checked = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap();
    assert!(matches!(
        checked.plots[0].startup_parameters[0].default,
        Some(CanonicalStartupValue::Quantity(_))
    ));
    assert_eq!(parsed.round_trip(), source);
    let highlighted = highlight_syntax(source).unwrap();
    assert!(highlighted
        .iter()
        .any(|item| item.kind == SyntaxHighlightKind::Number
            && &source[item.start..item.end] == "1Qm"));
}
