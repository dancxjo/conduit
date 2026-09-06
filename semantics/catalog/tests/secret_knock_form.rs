use conduit_core::PortTemporal;
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    structured_selector_definition, KindDefinition, KindSignature, ProfileCatalog, StartupCatalog,
};

const SOURCE: &str = include_str!("../../../forms/secret-knock/main.conduit");

#[test]
fn canonical_secret_knock_is_a_host_free_composition_of_reusable_forms() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_generalized_input_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_timed_pattern_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_timed_button_attempt_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_sequence_normalization_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_template_storage_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_final_normalized_pattern_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_pattern_comparison_catalogs(&mut startup, &mut profile)
        .unwrap();
    startup
        .insert(KindSignature {
            kind: conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND.into(),
            startup_parameters: Vec::new(),
        })
        .unwrap();
    let presentation = conduit_semantic_catalog::structured_presentation_contract(
        conduit_semantic_catalog::PATTERN_COMPARISON_TYPE,
        &conduit_semantic_catalog::pattern_comparison_type(),
    );
    profile
        .insert(KindDefinition {
            kind_id: presentation.kind_id,
            kind_contract_revision: presentation.kind_contract_revision,
            inputs: presentation.inputs,
            outputs: presentation.outputs,
            configuration: Vec::new(),
        })
        .unwrap();

    let syntax = parse_syntax_document(SOURCE);
    assert_eq!(syntax.round_trip(), SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let namesake = checked
        .forms
        .iter()
        .find(|form| form.name == "secret-knock")
        .unwrap();
    assert!(namesake.gears.iter().any(|gear| {
        gear.name.as_deref() == Some("normalize") && gear.kind == "normalize-durations"
    }));
    assert!(namesake.gears.iter().any(|gear| {
        gear.name.as_deref() == Some("intervals") && gear.kind == "derive-intervals"
    }));
    let intervals = expand_canonical_form_for_authoring(&checked, "derive-intervals", &profile)
        .expect("canonical interval derivation expands independently");
    assert_eq!(intervals.expanded.gears.len(), 1);
    assert_eq!(
        intervals.expanded.gears[0].kind_id.as_str(),
        conduit_semantic_catalog::ORDERED_EVENT_INTERVALS_KIND
    );
    let reusable = expand_canonical_form_for_authoring(&checked, "normalize-durations", &profile)
        .expect("the canonical normalization Face expands independently");
    assert_eq!(reusable.expanded.gears.len(), 1);
    assert_eq!(
        reusable.expanded.gears[0].kind_id.as_str(),
        conduit_semantic_catalog::NORMALIZE_SEQUENCE_KIND
    );
    for selector in checked
        .forms
        .iter()
        .flat_map(|form| &form.cords)
        .flat_map(|cord| &cord.stages)
        .filter_map(|stage| match stage {
            conduit_form::CheckedCordStage::StructuredSelector { selector, .. } => Some(selector),
            _ => None,
        })
    {
        profile
            .insert(structured_selector_definition(
                selector,
                PortTemporal::Flow { closes: true },
            ))
            .unwrap();
    }
    let expanded = expand_canonical_form_for_authoring(&checked, "secret-knock", &profile).unwrap();
    for kind in [
        conduit_semantic_catalog::TIMED_BUTTON_ATTEMPT_KIND,
        conduit_semantic_catalog::ORDERED_EVENT_INTERVALS_KIND,
        conduit_semantic_catalog::NORMALIZE_SEQUENCE_KIND,
        conduit_semantic_catalog::TEMPLATE_STORAGE_KIND,
        conduit_semantic_catalog::FINAL_NORMALIZED_PATTERN_KIND,
        conduit_semantic_catalog::COMPARE_PATTERN_KIND,
        conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND,
    ] {
        assert!(expanded
            .expanded
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str() == kind));
    }
    assert_eq!(expanded.expanded.connections.len(), 8);

    let lowercase = SOURCE.to_ascii_lowercase();
    for forbidden in [
        "browser",
        "host/",
        "implementation",
        "localstorage",
        "performance.now",
        "settimeout",
        "socket",
        "dom",
        "gpio",
    ] {
        assert!(
            !lowercase.contains(forbidden),
            "authored source contains {forbidden}"
        );
    }
}
