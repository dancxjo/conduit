#![cfg(feature = "semantic-bindings")]
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_speech::ipa_constructors::*;

const SOURCE: &str = include_str!("../examples/ipa/reusable-basis.conduit");
fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_speech::authoring::install(&mut startup).unwrap();
    install(&mut startup, &mut profiles).unwrap();
    (startup, profiles)
}

#[test]
fn one_declared_scope_is_exact_typed_immutable_material_for_three_constructors() {
    let (startup, profiles) = catalogs();
    let syntax = parse_syntax_document(SOURCE);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    validate_source(&syntax, &checked).unwrap();
    assert_eq!(checked.plots[0].local_values.len(), 4);
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "reusable-basis", &profiles).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 3);
    for gear in &expanded.expanded.gears {
        let constructor = IpaConstructor::from_kind(gear.kind_id.as_str()).unwrap();
        prepare_configuration(constructor, &gear.configuration).unwrap();
    }
    assert_eq!(syntax.round_trip(), SOURCE);
}

#[test]
fn referenced_scope_refusals_locate_the_original_declaration_not_the_alias() {
    let (startup, _) = catalogs();
    for (source, expected) in [
        (
            SOURCE.replacen(
                "inventory_id: \"inventory/test\"",
                "inventory_id: \"inventory/foreign\"",
                1,
            ),
            "\"inventory/foreign\"",
        ),
        (
            SOURCE.replacen(
                "revision: \"revision/test\"",
                "revision: \"revision/stale\"",
                1,
            ),
            "\"revision/stale\"",
        ),
        (
            SOURCE.replacen(
                "identity: \"variety/test\"",
                "identity: \"variety/foreign\"",
                1,
            ),
            "{identity: \"variety/foreign\", language: \"language/test\"}",
        ),
    ] {
        let syntax = parse_syntax_document(&source);
        let checked = check_syntax_document(&syntax, &startup).unwrap();
        let diagnostic = validate_source(&syntax, &checked).unwrap_err();
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            expected,
            "{diagnostic:?}"
        );
        assert_eq!(diagnostic.source_document_id, syntax.source_document_id());
    }
}

#[test]
fn quoted_request_alias_chain_retains_escaped_spelling_and_source_identity() {
    let (startup, _) = catalogs();
    let source = SOURCE.replace("    one: speech/phonemic-from-ipa( request = {original: \"ˈt͡ʃaː\", provenance: {method: \"reusable basis one\", source: manual(empty), version: none(empty)}}", "    original-request = {original: \"t͡ʃ\\n\", provenance: {method: \"reusable basis one\", source: manual(empty), version: none(empty)}}\n    request-alias = original-request\n    one: speech/phonemic-from-ipa( request = request-alias");
    let syntax = parse_syntax_document(&source);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let diagnostic = validate_source(&syntax, &checked).unwrap_err();
    assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], r"\n");
    let another = parse_syntax_document(&format!("# another Source\n{source}"));
    let diagnostic = validate_source(&another, &checked).unwrap_err();
    assert!(matches!(
        diagnostic.cause.refusal,
        IpaConstructorRefusal::SourceCorrelation
    ));
}

#[test]
fn missing_conflicting_cyclic_and_substituted_scopes_refuse_in_the_shared_checker() {
    let (startup, _) = catalogs();
    for source in [
        SOURCE.replace(", basis = reviewed-basis", ""),
        SOURCE.replace("inventory = reviewed-inventory", "inventory = missing-inventory"),
        SOURCE.replace("    reviewed-phone-bindings = {values: []}", "    reviewed-phone-bindings = {values: []}\n    reviewed-phone-bindings = {values: []}"),
        SOURCE.replace("    reviewed-phone-bindings = {values: []}", "    reviewed-phone-bindings = binding-alias\n    binding-alias = reviewed-phone-bindings"),
        SOURCE.replace("phoneme-bindings = reviewed-phoneme-bindings", "phoneme-bindings = reviewed-phone-bindings"),
    ] {
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty());
        assert!(check_syntax_document(&syntax, &startup).is_err(), "{source}");
    }
}

#[test]
fn partial_and_conflicting_membership_refuse_before_value_preparation() {
    let (startup, _) = catalogs();
    for source in [
        SOURCE.replace(", {phoneme: \"phoneme/long-a\", units: [\"unit/a\", \"unit/length\"], provenance: {method: \"declared contrast\", source: manual(empty), version: none(empty)}}", ""),
        SOURCE.replace("{phoneme: \"phoneme/long-a\", units: [\"unit/a\", \"unit/length\"], provenance: {method: \"declared contrast\", source: manual(empty), version: none(empty)}}", "{phoneme: \"phoneme/foreign\", units: [\"unit/a\", \"unit/length\"], provenance: {method: \"declared contrast\", source: manual(empty), version: none(empty)}}"),
        SOURCE.replace("notation: \"aː\"", "notation: \"t͡ʃ\"").replace("[\"unit/a\", \"unit/length\"]", "[\"unit/ch\"]"),
    ] {
        let syntax = parse_syntax_document(&source);
        let checked = check_syntax_document(&syntax, &startup).unwrap();
        assert!(validate_source(&syntax, &checked).is_err());
    }
}

#[test]
fn unchanged_alias_spelling_cannot_equate_distinct_reviewed_revisions() {
    let (startup, profiles) = catalogs();
    let configs: Vec<_> = [
        SOURCE.to_owned(),
        SOURCE.replace("revision/test", "revision/next"),
    ]
    .iter()
    .map(|source| {
        let syntax = parse_syntax_document(source);
        let checked = check_syntax_document(&syntax, &startup).unwrap();
        validate_source(&syntax, &checked).unwrap();
        let expanded =
            expand_canonical_plot_for_authoring(&checked, "reusable-basis", &profiles).unwrap();
        let gear = &expanded.expanded.gears[0];
        prepare_configuration(
            IpaConstructor::from_kind(gear.kind_id.as_str()).unwrap(),
            &gear.configuration,
        )
        .unwrap()
        .bytes()
        .to_vec()
    })
    .collect();
    assert_ne!(configs[0], configs[1]);
}

#[test]
fn oversized_original_inventory_refuses_even_when_requested_phoneme_is_present() {
    let (startup, _) = catalogs();
    let definition = "{identity: \"phoneme/extra\", notation: \"a\", features: [], aliases: [], default_phone: none(empty), possible_phones: [], allophones: [], status: core(empty)}";
    let source = SOURCE.replacen(
        "phonemes: [",
        &format!("phonemes: [{},", vec![definition; 65].join(",")),
        1,
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty());
    assert!(check_syntax_document(&syntax, &startup).is_err());
}

#[test]
fn renaming_a_local_preserves_constructor_material_but_retains_new_source_identity() {
    let (startup, profiles) = catalogs();
    let mut expansions = Vec::new();
    for source in [
        SOURCE.to_owned(),
        SOURCE.replace("reviewed-basis", "explicit-basis"),
    ] {
        let syntax = parse_syntax_document(&source);
        let checked = check_syntax_document(&syntax, &startup).unwrap();
        validate_source(&syntax, &checked).unwrap();
        expansions.push(
            expand_canonical_plot_for_authoring(&checked, "reusable-basis", &profiles).unwrap(),
        );
    }
    assert_ne!(
        expansions[0].expanded.source_document_id,
        expansions[1].expanded.source_document_id
    );
    for (first, renamed) in expansions[0]
        .expanded
        .gears
        .iter()
        .zip(&expansions[1].expanded.gears)
    {
        assert_eq!(first.kind_id, renamed.kind_id);
        assert_eq!(first.configuration, renamed.configuration);
    }
}

#[test]
fn inspectable_expansion_retains_exact_source_constructor_and_configuration_types() {
    let (startup, profiles) = catalogs();
    let syntax = parse_syntax_document(SOURCE);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    validate_source(&syntax, &checked).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "reusable-basis", &profiles).unwrap();
    println!("Declared scope; pack: none (all original material is in this Source)");
    println!(
        "Source: {}\nChecked Plot: {}\nExpanded Plot: {}",
        authored.expanded.source_document_id.as_str(),
        authored.expanded.checked_plot_id.as_str(),
        authored.expanded.expanded_plot_id.as_str()
    );
    for gear in &authored.expanded.gears {
        println!(
            "Gear: {} => {}",
            gear.gear_id.as_str(),
            gear.kind_id.as_str()
        );
        for entry in &gear.configuration {
            let conduit_core::ConfigurationValue::Structured(value) = &entry.value else {
                panic!("exact structured scoped arguments")
            };
            println!(
                "  {}: {} ({} canonical bytes)",
                entry.key,
                value.profile().as_str(),
                value.canonical_value().len()
            );
            assert_eq!(entry.value.semantic_kind(), *value.profile());
        }
    }
}
