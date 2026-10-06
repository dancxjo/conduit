use conduit_core::{kind_id, ConfigurationValue, PortDirection, PortTemporal};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document,
    validate_startup_configuration, CanonicalStartupValue,
};
use conduit_semantic_catalog::{
    standard_profile_catalog, GearPalette, PaletteProfile, QUANTITY_INFO_WRAP_KIND,
};

#[test]
fn text_palette_matches_the_installed_canonical_text_contracts() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut catalog = conduit_plot::ProfileCatalog::new();
    conduit_text::install_text_catalogs(&mut startup, &mut catalog).unwrap();
    let palette = GearPalette::standard().unwrap();
    for id in ["text/literal", "text/upper", "text/join"] {
        let entry = palette.find(&kind_id(id)).unwrap();
        let kind = catalog.canonical_kind(&kind_id(id)).unwrap();
        assert_eq!(entry.front, kind.checked_front());
        assert_eq!(entry.kind_contract_revision, kind.kind_contract_revision);
        assert_eq!(entry.semantic_laws, kind.semantic_laws);
        assert_eq!(entry.limits, kind.limits);
        assert_eq!(entry.configuration.len(), kind.configuration.len());
        for (projected, field) in entry.configuration.iter().zip(&kind.configuration) {
            assert_eq!(projected.key, field.key);
            assert_eq!(projected.rule, field.rule);
            assert_eq!(projected.default_value, field.default_value);
        }
    }
}

#[test]
fn palette_exact_fore_revision_and_rules_are_source_catalog_truth() {
    let palette = GearPalette::standard().unwrap();
    let catalog = standard_profile_catalog();
    for entry in palette.entries() {
        assert_eq!(entry.profile, PaletteProfile::PortableAuthoring);
        assert_eq!(entry.inputs.len(), entry.front.inputs().len());
        assert_eq!(entry.outputs.len(), entry.front.outputs().len());
        assert!(entry
            .inputs
            .iter()
            .all(|port| entry.front.inputs().contains(port)));
        assert!(entry
            .outputs
            .iter()
            .all(|port| entry.front.outputs().contains(port)));
        if let Some(kind) = catalog.canonical_kind(&entry.kind_id) {
            assert_eq!(entry.kind_contract_revision, kind.kind_contract_revision);
            assert_eq!(entry.front, kind.checked_front());
            assert_eq!(entry.semantic_laws, kind.semantic_laws);
            assert_eq!(entry.limits, kind.limits);
            for (projected, field) in entry.configuration.iter().zip(&kind.configuration) {
                assert_eq!(projected.key, field.key);
                assert_eq!(projected.rule, field.rule);
                assert_eq!(projected.default_value, field.default_value);
            }
        }
    }
    let keyboard = palette.find(&kind_id("input/keyboard")).unwrap();
    assert_eq!(
        keyboard.kind_contract_revision,
        conduit_semantic_catalog::keyboard_contract_revision()
    );
    assert_eq!(
        keyboard.front,
        conduit_semantic_catalog::keyboard_semantic_contract().checked_front()
    );
}

#[test]
fn only_reviewed_exact_conversion_is_suggested() {
    let palette = GearPalette::standard().unwrap();
    let wrapper = palette.find(&kind_id(QUANTITY_INFO_WRAP_KIND)).unwrap();
    let mut source = wrapper.inputs[0].clone();
    source.direction = PortDirection::Output;
    let mut sink = wrapper.outputs[0].clone();
    sink.direction = PortDirection::Input;
    let suggestions = palette.adapter_suggestions(&source, &sink);
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].kind_id, wrapper.kind_id);
    assert_eq!(
        suggestions[0].kind_contract_revision,
        wrapper.kind_contract_revision
    );
    assert_eq!(suggestions[0].input, wrapper.inputs[0]);
    assert_eq!(suggestions[0].output, wrapper.outputs[0]);
    source.value_kind = kind_id("value/quantity-lookalike");
    assert!(palette.adapter_suggestions(&source, &sink).is_empty());
    source.value_kind = wrapper.inputs[0].value_kind.clone();
    sink.temporal = PortTemporal::Current;
    assert!(palette.adapter_suggestions(&source, &sink).is_empty());
    let upper = palette.find(&kind_id("text/upper")).unwrap();
    assert!(palette
        .adapter_suggestions(&upper.outputs[0], &upper.inputs[0])
        .is_empty());
}

#[test]
fn configuration_bounds_and_diagnostics_match_source_expansion() {
    let catalog = standard_profile_catalog();
    let startup = catalog.startup_catalog().unwrap();
    let kind = catalog.canonical_kind(&kind_id("text/join")).unwrap();
    let field = &kind.configuration[0];
    let conduit_core::KindConfigurationRule::TextBytes { maximum } = field.rule else {
        panic!("text/join must expose its finite text rule");
    };
    for size in [0, maximum as usize, maximum as usize + 1] {
        let text = "x".repeat(size);
        let literal = format!("\"{text}\"");
        let source = format!(
            "plot main {{\n join: text/join({} = {literal})\n}}\n",
            field.key
        );
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
        let expanded = expand_canonical_plot(&checked, "main", &catalog);
        let tool = validate_startup_configuration(field, CanonicalStartupValue::Literal(literal));
        assert_eq!(tool.is_ok(), expanded.is_ok());
        match (tool, expanded) {
            (Ok(value), Ok(plot)) => {
                assert_eq!(value, ConfigurationValue::Text(text));
                assert_eq!(plot.gears[0].configuration[0].value, value);
            }
            (Err(tool), Err(source)) => {
                assert_eq!(tool.code, source.code);
                assert_eq!(tool.message, source.message);
            }
            _ => panic!("editor and source disagreed"),
        }
    }
}
