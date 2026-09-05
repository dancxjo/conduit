use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_semantic_catalog::{
    garden_clock_observation_type, garden_contact_observation_type, install_signal_garden_catalog,
    GARDEN_ENRICHED_STEP_KIND, GARDEN_FIXTURE_KIND, GARDEN_MINIMAL_STEP_KIND,
};

const REUSABLE: &str = include_str!("../../../forms/garden-state-step/main.conduit");
const GARDEN: &str = include_str!("../../../forms/signal-garden/main.conduit");

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_signal_garden_catalog(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

#[test]
fn minimal_and_enriched_forms_preserve_distinct_mandatory_source_types() {
    let (startup, profile) = catalogs();
    let reusable = parse_syntax_document(REUSABLE);
    assert!(
        reusable.diagnostics.is_empty(),
        "{:?}",
        reusable.diagnostics
    );
    let reusable = check_syntax_document(&reusable, &startup).unwrap();
    assert_eq!(reusable.forms.len(), 1);
    assert_eq!(reusable.forms[0].name, "garden-state-step");

    let garden = parse_syntax_document(GARDEN);
    assert!(garden.diagnostics.is_empty(), "{:?}", garden.diagnostics);
    let garden = check_syntax_document(&garden, &startup).unwrap();
    let minimal =
        expand_canonical_form_for_authoring(&garden, "signal-garden-minimal", &profile).unwrap();
    let enriched =
        expand_canonical_form_for_authoring(&garden, "signal-garden-interactive", &profile)
            .unwrap();
    assert_eq!(minimal.expanded.gears.len(), 2);
    assert_eq!(enriched.expanded.gears.len(), 2);
    assert!(minimal
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == GARDEN_FIXTURE_KIND));
    assert!(minimal
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == GARDEN_MINIMAL_STEP_KIND));
    assert!(enriched
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == GARDEN_ENRICHED_STEP_KIND));

    let minimal_step = profile
        .get(&conduit_core::kind_id(GARDEN_MINIMAL_STEP_KIND))
        .unwrap();
    let enriched_step = profile
        .get(&conduit_core::kind_id(GARDEN_ENRICHED_STEP_KIND))
        .unwrap();
    assert_eq!(minimal_step.inputs.len(), 2);
    assert_eq!(enriched_step.inputs.len(), 3);
    assert_eq!(
        &minimal_step.inputs[1].value_kind,
        garden_clock_observation_type()
            .profile()
            .unwrap()
            .value_kind()
    );
    assert_eq!(
        &enriched_step.inputs[2].value_kind,
        garden_contact_observation_type()
            .profile()
            .unwrap()
            .value_kind()
    );
    assert_ne!(
        minimal_step.inputs[1].value_kind,
        enriched_step.inputs[2].value_kind
    );
}

#[test]
fn authored_meaning_is_host_and_mechanism_neutral() {
    for source in [REUSABLE, GARDEN] {
        let source = source.to_ascii_lowercase();
        for forbidden in [
            "browser",
            "dom",
            "canvas",
            "websocket",
            "device",
            "sensor",
            "host",
            "random",
            "storage",
            "nullable",
            "anyobservation",
        ] {
            assert!(
                !source.contains(forbidden),
                "authored source leaked {forbidden}"
            );
        }
    }
}
