use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationValue,
    ConnectionBase, ExecutionProfileId, HostAdvertisement, HostId, HostProfileId, ImplementationId,
    OfferGeneration, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    KindDefinition, ProfileCatalog, StartupCatalog,
};
use conduit_planner::{default_expanded_placements, plan_expanded_canonical};
use conduit_std_catalog::{
    install_structured_music_form_catalogs, instrument_mapping_type, INSTRUMENT_MAP_KIND,
};

const SOURCE: &str = include_str!("../../../examples/breadboard-instrument.conduit");

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_structured_music_form_catalogs(&mut startup, &mut profile).unwrap();
    (startup, profile)
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
    let definition = profile
        .get(&conduit_core::KindId::from(INSTRUMENT_MAP_KIND))
        .unwrap()
        .clone();
    let host = host(&definition);
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

    let changed = SOURCE.replace(
        "[60, 62, 64, 65, 67, 69, 71, 72]",
        "[60, 62, 63, 65, 67, 69, 71, 72]",
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
        "[60, 62, 64, 65, 67, 69, 71, 72]",
        "[60, 62, 64, 65, 67, 69, 71]",
    );
    let error = check_syntax_document(&parse_syntax_document(&short), &startup).unwrap_err();
    assert_eq!(error.code, "CND-FRM-051");
    assert!(error.message.contains("exact type requires 8"));

    let wrong = SOURCE.replace("sustain_button", "sustain_control");
    let error = check_syntax_document(&parse_syntax_document(&wrong), &startup).unwrap_err();
    assert_eq!(error.code, "CND-FRM-051");
    assert!(error.message.contains("unknown structured field"));
}

fn host(definition: &KindDefinition) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("instrument-host"),
        boot_id: BootId::from("instrument-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/instrument-host@1"),
        resources: vec![],
        capabilities: vec![offer(definition)],
        planner_capabilities: vec![],
    }
}

fn offer(definition: &KindDefinition) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![conduit_core::FaceStartupParameter {
            name: "mapping".into(),
            value_type: "InstrumentMapping".into(),
            has_default: false,
        }],
        shorthand: None,
        capability_id: CapabilityId::from("instrument-map"),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: definition.kind_contract_revision.clone(),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("test/instrument-map@1"),
            implementation_id: ImplementationId::from("test/instrument-map@1"),
            artifact_id: ArtifactId::from("test/instrument-map@1"),
        },
        inputs: definition.inputs.clone(),
        outputs: definition.outputs.clone(),
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 16,
            max_queue_bytes: 16_384,
        },
    }
}
