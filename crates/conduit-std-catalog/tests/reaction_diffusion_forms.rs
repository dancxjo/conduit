use conduit_core::{
    BootId, ConnectionBase, GrayScottParameters, HostAdvertisement, HostId, HostProfileId,
    OfferGeneration, ReactionDiffusionEvolveRequest, ReactionDiffusionFieldId,
    ReactionDiffusionFieldState, PROTOCOL_VERSION, REACTION_DIFFUSION_MAXIMUM_STATE_BYTES,
    REACTION_DIFFUSION_NUMERIC_PROFILE,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    CanonicalBackCatalog, ProfileCatalog, StartupCatalog,
};
use conduit_std_catalog::{
    evolve_reaction_diffusion_hosted, install_reaction_diffusion_catalogs,
    reaction_diffusion_std_offer, HOSTED_REACTION_DIFFUSION_LIMITS, REACTION_DIFFUSION_EVOLVE_KIND,
    REACTION_DIFFUSION_HOST_OPERATION,
};

const SOURCE: &str = r#"
form field-step {
    evolve: field/evolve
}
"#;
const FIELD_ID: ReactionDiffusionFieldId = ReactionDiffusionFieldId(*b"field-a0-hosted1");

#[test]
fn ordinary_form_checks_and_plans_on_one_truthful_finite_host_offer() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_reaction_diffusion_catalogs(&mut startup, &mut profile).unwrap();
    let parsed = parse_syntax_document(SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authored = expand_canonical_form_for_authoring(&checked, "field-step", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 1);

    let host = host();
    let placements = conduit_planner::default_expanded_placements(
        &authored.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &authored.expanded,
        &[host],
        &placements,
        &[ConnectionBase::Local],
    )
    .unwrap();
    let placement = &plan.fragments[0].placements[0];
    assert_eq!(placement.kind_id.as_str(), REACTION_DIFFUSION_EVOLVE_KIND);
    assert_eq!(
        placement.host_operations[0].contract_id.as_str(),
        REACTION_DIFFUSION_HOST_OPERATION
    );
    assert!(placement.resources.is_empty());
    assert!(placement.authority.is_empty());

    let offer = reaction_diffusion_std_offer();
    assert_eq!(offer.inputs[0].port_id.as_str(), "state");
    assert_eq!(offer.inputs[1].port_id.as_str(), "request");
    assert_eq!(offer.outputs[0].port_id.as_str(), "next-state");
    assert_eq!(offer.inputs[0].value_kind, offer.outputs[0].value_kind);
    assert_eq!(offer.limits.max_active_instances, 1);
    assert_eq!(offer.limits.max_queue_items, 1);
    assert_eq!(
        offer.host_operations[0].maximum_output_bytes,
        REACTION_DIFFUSION_MAXIMUM_STATE_BYTES
    );
}

#[test]
fn direction_distinct_face_accepts_one_exact_canonical_back() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_reaction_diffusion_catalogs(&mut startup, &mut profile).unwrap();
    let back = check_syntax_document(
        &parse_syntax_document(&format!(
            "form field/evolve (\n > state: {}\n > request: {}\n next-state: {} >\n) {{\n}}\n",
            conduit_core::REACTION_DIFFUSION_STATE_INFO_ID,
            conduit_core::REACTION_DIFFUSION_REQUEST_INFO_ID,
            conduit_core::REACTION_DIFFUSION_STATE_INFO_ID,
        )),
        &startup,
    )
    .unwrap();
    let definition = profile
        .get(&conduit_core::kind_id(REACTION_DIFFUSION_EVOLVE_KIND))
        .unwrap();
    assert_eq!(
        back.forms[0].checked_face(),
        reaction_diffusion_std_offer().checked_face()
    );
    let mut backs = CanonicalBackCatalog::new();
    backs
        .insert(definition, &back, REACTION_DIFFUSION_EVOLVE_KIND)
        .unwrap();
}

#[test]
fn hosted_reference_is_repeatable_and_contains_no_realization_identity() {
    let initial = ReactionDiffusionFieldState::initialized(
        FIELD_ID,
        12,
        10,
        GrayScottParameters::REFERENCE,
        42,
    )
    .unwrap();
    let request = ReactionDiffusionEvolveRequest {
        field_id: FIELD_ID,
        expected_generation: 0,
        generations: 3,
        admitted_cell_generations: 360,
    };
    let first = evolve_reaction_diffusion_hosted(&initial, request).unwrap();
    let second = evolve_reaction_diffusion_hosted(&initial, request).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.generation, 3);

    let portable_truth = format!("{initial:?} {request:?} {REACTION_DIFFUSION_NUMERIC_PROFILE}")
        .to_ascii_lowercase();
    for forbidden in [
        "host/",
        "boot/",
        "esp32",
        "pico",
        "partition",
        "websocket",
        "http://",
        "dom",
        "gpio",
    ] {
        assert!(!portable_truth.contains(forbidden), "leaked {forbidden}");
    }
    assert_eq!(HOSTED_REACTION_DIFFUSION_LIMITS.maximum_active_instances, 1);
}

fn host() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/field-proof"),
        boot_id: BootId::from("boot/field-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/field-proof@1"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![reaction_diffusion_std_offer()],
    }
}
