use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring,
    expand_canonical_form_for_authoring_with_backs, parse_syntax_document, CanonicalBackCatalog,
    ProfileCatalog, StartupCatalog,
};
use conduit_semantic_catalog::{
    garden_clock_observation_type, garden_contact_observation_type, install_signal_garden_backs,
    install_signal_garden_catalog, GARDEN_ENRICHED_REDUCER_KIND, GARDEN_ENRICHED_STEP_KIND,
    GARDEN_FIXTURE_KIND, GARDEN_MINIMAL_STEP_KIND, GARDEN_OBSERVATION_COMBINE_KIND,
    GARDEN_STATE_PRESENTATION_KIND,
};

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
    let garden = parse_syntax_document(GARDEN);
    assert!(garden.diagnostics.is_empty(), "{:?}", garden.diagnostics);
    let garden = check_syntax_document(&garden, &startup).unwrap();
    let minimal =
        expand_canonical_form_for_authoring(&garden, "signal-garden-minimal", &profile).unwrap();
    let enriched =
        expand_canonical_form_for_authoring(&garden, "signal-garden-interactive", &profile)
            .unwrap();
    assert_eq!(garden.forms.len(), 5);
    assert_eq!(minimal.expanded.gears.len(), 2);
    assert_eq!(enriched.expanded.gears.len(), 3);
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
    assert!(!enriched
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == GARDEN_ENRICHED_STEP_KIND));
    assert!(enriched
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == GARDEN_OBSERVATION_COMBINE_KIND));
    assert!(enriched
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == GARDEN_ENRICHED_REDUCER_KIND));

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
fn displayed_garden_keeps_presentation_downstream_of_semantic_state() {
    let (startup, profile) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(GARDEN), &startup).unwrap();
    let displayed = expand_canonical_form_for_authoring(
        &checked,
        "signal-garden-interactive-display",
        &profile,
    )
    .unwrap()
    .expanded;
    assert_eq!(displayed.gears.len(), 4);
    for expected in [
        GARDEN_FIXTURE_KIND,
        GARDEN_OBSERVATION_COMBINE_KIND,
        GARDEN_ENRICHED_REDUCER_KIND,
        GARDEN_STATE_PRESENTATION_KIND,
    ] {
        assert!(displayed
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str() == expected));
    }
    let presentation = profile
        .get(&conduit_core::kind_id(GARDEN_STATE_PRESENTATION_KIND))
        .unwrap();
    assert_eq!(presentation.inputs.len(), 1);
    assert!(presentation.outputs.is_empty());
    assert_eq!(
        presentation.inputs[0].value_kind,
        conduit_semantic_catalog::garden_state_type()
            .profile()
            .unwrap()
            .value_kind()
            .clone()
    );
}

#[test]
fn enriched_face_has_a_canonical_back_with_two_input_reusable_leaves() {
    let (startup, profile) = catalogs();
    let source = "form main (\n > prior: GardenState\n > clock: GardenClockObservation\n > contact: GardenContactObservation\n next: GardenState >\n) {\n evolve: state/garden-step-contact\n prior > evolve.prior\n clock > evolve.clock\n contact > evolve.contact\n evolve.next > next\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let mut backs = CanonicalBackCatalog::new();
    install_signal_garden_backs(&startup, &profile, &mut backs).unwrap();
    let expanded =
        expand_canonical_form_for_authoring_with_backs(&checked, "main", &profile, &backs)
            .unwrap()
            .expanded;

    assert_eq!(expanded.realization_backs.len(), 1);
    assert_eq!(
        expanded.realization_backs[0].kind_id.as_str(),
        GARDEN_ENRICHED_STEP_KIND
    );
    assert_eq!(expanded.gears.len(), 2);
    assert!(expanded
        .gears
        .iter()
        .all(|gear| gear.kind_id.as_str() != GARDEN_ENRICHED_STEP_KIND));
    for expected in [
        GARDEN_OBSERVATION_COMBINE_KIND,
        GARDEN_ENRICHED_REDUCER_KIND,
    ] {
        assert!(expanded
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str() == expected));
    }
    assert!(expanded.gears.iter().all(|gear| {
        profile
            .get(&gear.kind_id)
            .is_some_and(|definition| definition.inputs.len() <= 2)
    }));
}

#[test]
fn authored_meaning_is_host_and_mechanism_neutral() {
    for source in [GARDEN] {
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
