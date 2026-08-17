use conduit_core::{
    BootId, ConfigurationValue, ConnectionBase, HostAdvertisement, HostId, HostProfileId,
    OfferGeneration, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_planner::{default_expanded_placements, plan_expanded_canonical};
use conduit_std_catalog::{
    install_structured_music_form_catalogs, instrument_map_std_offer, instrument_mapping_type,
    INSTRUMENT_MAP_KIND, INSTRUMENT_MAP_STD_IMPLEMENTATION,
};

const SOURCE: &str = include_str!("../../../examples/breadboard-instrument.conduit");
const LESSON_SOURCE: &str = include_str!("../../../examples/rhythm-lesson.conduit");

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_structured_music_form_catalogs(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

#[test]
fn separate_rhythm_lesson_is_hardware_neutral_and_expands_with_portable_music() {
    let (startup, profile) = catalogs();
    let parsed = parse_syntax_document(LESSON_SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "rhythm-lesson", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 1);
    assert_eq!(
        authored.expanded.gears[0].kind_id.as_str(),
        conduit_std_catalog::RHYTHM_COMPARE_KIND
    );
    assert_eq!(authored.input_bindings.len(), 2);
    assert_eq!(authored.output_bindings.len(), 1);

    let meaning = LESSON_SOURCE
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    for forbidden in [
        "pico",
        "gpio",
        "adc",
        "usb",
        "midi",
        "host",
        "instrument-map",
    ] {
        assert!(!meaning.contains(forbidden), "lesson leaked {forbidden}");
    }
}

#[test]
fn lesson_types_and_configuration_refuse_before_any_runtime_exists() {
    let (startup, profile) = catalogs();
    let wrong_reference = LESSON_SOURCE.replace("BeatReference", "InstrumentControl");
    let checked =
        check_syntax_document(&parse_syntax_document(&wrong_reference), &startup).unwrap();
    let error =
        expand_canonical_form_for_authoring(&checked, "rhythm-lesson", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-045");
    assert!(error.message.contains("runtime face port 'reference'"));

    let excessive_tolerance =
        LESSON_SOURCE.replace("tolerance-micros = 30000", "tolerance-micros = 1000001");
    let checked =
        check_syntax_document(&parse_syntax_document(&excessive_tolerance), &startup).unwrap();
    let error =
        expand_canonical_form_for_authoring(&checked, "rhythm-lesson", &profile).unwrap_err();
    assert!(error.to_string().contains("tolerance-micros"));

    let feedback = conduit_std_catalog::timing_feedback_type();
    let conduit_core::StructuredInfoTypeShape::Record { fields, .. } = feedback.shape() else {
        panic!("timing feedback must remain a record")
    };
    assert_eq!(
        fields.iter().map(|field| field.name()).collect::<Vec<_>>(),
        [
            "beat",
            "classification",
            "delta_micros",
            "expected_time_micros",
            "observed",
            "observed_time_micros",
            "recovery_state",
        ]
    );
}

#[test]
fn ordinary_instrument_form_carries_one_typed_finite_mapping_into_expansion() {
    let (startup, profile) = catalogs();
    let parsed = parse_syntax_document(SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "breadboard-instrument", &profile).unwrap();

    assert_eq!(authored.expanded.gears.len(), 1);
    let mapper = &authored.expanded.gears[0];
    assert_eq!(mapper.kind_id.as_str(), INSTRUMENT_MAP_KIND);
    let ConfigurationValue::Structured(mapping) = &mapper.configuration[0].value else {
        panic!("mapping must remain structured semantic configuration");
    };
    let expected = instrument_mapping_type();
    assert_eq!(mapping.profile(), expected.profile().unwrap().value_kind());
    let decoded =
        conduit_core::StructuredInfoValue::from_canonical_bytes(mapping.canonical_value()).unwrap();
    assert_eq!(decoded.value_type(), &expected);
    assert_eq!(authored.input_bindings.len(), 1);
    assert_eq!(authored.output_bindings.len(), 2);
    let semantic_source = SOURCE
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    for forbidden in ["pico", "gpio", "adc", "usb", "midi", "host"] {
        assert!(!semantic_source.contains(forbidden));
    }
}

#[test]
fn exact_mapping_reaches_an_ordinary_plan_and_shape_changes_change_identity() {
    let (startup, profile) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "breadboard-instrument", &profile).unwrap();
    let host = host();
    let placements =
        default_expanded_placements(&authored.expanded, core::slice::from_ref(&host)).unwrap();
    let plan = plan_expanded_canonical(
        &authored.expanded,
        &[host],
        &placements,
        &[ConnectionBase::Local],
    )
    .unwrap();
    let planned_mapping = &plan.fragments[0].placements[0].configuration[0].value;
    assert_eq!(
        planned_mapping,
        &authored.expanded.gears[0].configuration[0].value
    );
    assert_eq!(
        plan.fragments[0].placements[0].implementation_id.as_str(),
        INSTRUMENT_MAP_STD_IMPLEMENTATION
    );

    let changed = SOURCE.replace(
        "[261626, 293665, 329628, 349228, 391995, 440000, 493883, 523251]",
        "[261626, 293665, 330000, 349228, 391995, 440000, 493883, 523251]",
    );
    let changed = check_syntax_document(&parse_syntax_document(&changed), &startup).unwrap();
    assert_ne!(
        checked.forms[0].checked_form_id,
        changed.forms[0].checked_form_id
    );
}

#[test]
fn mapping_length_and_control_schema_refuse_before_expansion() {
    let (startup, _) = catalogs();
    let short = SOURCE.replace(
        "[261626, 293665, 329628, 349228, 391995, 440000, 493883, 523251]",
        "[261626, 293665, 329628, 349228, 391995, 440000, 493883]",
    );
    let error = check_syntax_document(&parse_syntax_document(&short), &startup).unwrap_err();
    assert_eq!(error.code, "CND-FRM-051");
    assert!(error.message.contains("exact type requires 8"));

    let wrong = SOURCE.replace("sustain_button", "sustain_control");
    let error = check_syntax_document(&parse_syntax_document(&wrong), &startup).unwrap_err();
    assert_eq!(error.code, "CND-FRM-051");
    assert!(error.message.contains("unknown structured field"));
}

fn host() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("instrument-host"),
        boot_id: BootId::from("instrument-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/instrument-host@1"),
        resources: vec![],
        capabilities: vec![instrument_map_std_offer()],
        planner_capabilities: vec![],
    }
}
