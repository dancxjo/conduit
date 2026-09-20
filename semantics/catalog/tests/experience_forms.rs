use conduit_core::{
    ArtifactId, BaseImplementationId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    HostAdvertisement, HostId, HostProfileId, ImplementationId, ImplementationOffer,
    OfferGeneration, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

const SOURCE: &str = include_str!("../../../forms/experiencer/main.conduit");

#[test]
fn experiencer_is_one_portable_typed_convergence_without_effect_authority() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_presentation::install_geometry_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_vision_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_experience_catalogs(&mut startup, &mut profile).unwrap();

    let parsed = parse_syntax_document(SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authored = expand_canonical_form_for_authoring(&checked, "experiencer", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 1);
    assert_eq!(
        authored.expanded.gears[0].kind_id.as_str(),
        "experience/relate-current"
    );
    assert_eq!(authored.expanded.gears[0].inputs.len(), 7);
    assert_eq!(authored.expanded.gears[0].outputs.len(), 1);

    for forbidden in [
        "camera",
        "microphone",
        "filesystem",
        "socket",
        "host/",
        "execute-action",
        "fulfill",
    ] {
        assert!(
            !SOURCE.contains(forbidden),
            "portable source leaked {forbidden}"
        );
    }

    let definition = profile
        .get(&conduit_core::kind_id(
            conduit_semantic_catalog::EXPERIENCE_RELATE_KIND,
        ))
        .unwrap();
    assert!(definition
        .inputs
        .iter()
        .all(|port| port.temporal == conduit_core::PortTemporal::Flow { closes: false }));
    assert_eq!(
        definition.outputs[0].temporal,
        conduit_core::PortTemporal::Current
    );
}

#[test]
fn epistemic_source_types_remain_nominally_distinct_and_bounded() {
    let types = [
        conduit_semantic_catalog::experience_human_input_type(),
        conduit_semantic_catalog::experience_body_input_type(),
        conduit_semantic_catalog::experience_memory_input_type(),
        conduit_semantic_catalog::experience_inference_input_type(),
        conduit_semantic_catalog::experience_hypothesis_input_type(),
    ];
    let kinds = types
        .iter()
        .map(|value| value.profile().unwrap().value_kind().clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(kinds.len(), types.len());

    let current = conduit_semantic_catalog::current_experience_type();
    let conduit_core::StructuredInfoTypeShape::Record { fields, .. } = current.shape() else {
        panic!("current experience must remain a nominal record")
    };
    let identities = fields
        .iter()
        .find(|field| field.name() == "item_identities")
        .unwrap();
    let conduit_core::StructuredInfoTypeShape::Collection { length, .. } =
        identities.value_type().shape()
    else {
        panic!("item identities must remain bounded")
    };
    assert_eq!(length, 32);
    assert!(fields
        .iter()
        .any(|field| field.name() == "provenance_revision"));
}

#[test]
fn one_checked_experiencer_moves_between_compatible_hosts_without_changing_meaning() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_presentation::install_geometry_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_vision_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_experience_catalogs(&mut startup, &mut profile).unwrap();

    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    let expanded = expand_canonical_form_for_authoring(&checked, "experiencer", &profile)
        .unwrap()
        .expanded;
    let definition = profile
        .get(&conduit_core::kind_id(
            conduit_semantic_catalog::EXPERIENCE_RELATE_KIND,
        ))
        .unwrap();
    let host = |name: &str| HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(format!("experience/{name}")),
        boot_id: BootId::from(format!("experience/{name}/boot")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from(format!("experience/{name}@1")),
        bases: vec![],
        resources: vec![],
        capabilities: vec![CapabilityOffer {
            startup_parameters: vec![],
            shorthand: None,
            capability_id: CapabilityId::from(format!("experience/{name}/relate-current")),
            kind_id: definition.kind_id.clone(),
            kind_contract_revision: definition.kind_contract_revision.clone(),
            inputs: definition.inputs.clone(),
            outputs: definition.outputs.clone(),
            implementation: ImplementationOffer {
                execution_profile_id: format!("experience/{name}/profile@1").into(),
                implementation_id: ImplementationId::from(format!(
                    "experience/{name}/relate-current@1"
                )),
                artifact_id: ArtifactId::from(format!("experience/{name}/artifact@1")),
            },
            host_operations: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 8,
                max_queue_bytes: 8_192,
            },
        }],
        planner_capabilities: vec![],
    };
    let hosts = [host("browser"), host("native")];
    let bases = [BaseImplementationId::from("conduit.base/experience@1")];

    let browser_placements =
        conduit_planner::default_expanded_placements(&expanded, &hosts[..1]).unwrap();
    let browser_plan =
        conduit_planner::plan_expanded_canonical(&expanded, &hosts, &browser_placements, &bases)
            .unwrap();
    let native_placements =
        conduit_planner::default_expanded_placements(&expanded, &hosts[1..]).unwrap();
    let native_plan =
        conduit_planner::plan_expanded_canonical(&expanded, &hosts, &native_placements, &bases)
            .unwrap();

    assert_eq!(
        browser_plan.source_document_id,
        native_plan.source_document_id
    );
    assert_eq!(browser_plan.checked_form_id, native_plan.checked_form_id);
    assert_eq!(browser_plan.expanded_form_id, native_plan.expanded_form_id);
    assert_ne!(browser_plan.plan_id, native_plan.plan_id);
    assert_eq!(browser_plan.fragments[0].host_id, hosts[0].host_id);
    assert_eq!(native_plan.fragments[0].host_id, hosts[1].host_id);
    assert_eq!(
        browser_plan.fragments[0].placements[0].kind_id,
        native_plan.fragments[0].placements[0].kind_id
    );
}
