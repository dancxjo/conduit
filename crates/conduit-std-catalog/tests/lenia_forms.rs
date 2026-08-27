use conduit_alife::{
    install_lenia_catalogs, LENIA_STEP_KIND, SCALAR_FIELD2_INFO_ID, SCALAR_FIELD_PRESENTATION_KIND,
};
use conduit_core::{
    resource_requirement, wait_host_operation_requirement, BaseImplementationId, BootId,
    HostAdvertisement, HostId, HostProfileId, OfferGeneration, PortTemporal, PROTOCOL_VERSION,
    TIMER_RESOURCE_CLASS,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_catalog::{
    install_tick_presentation_catalog, realization_offer, RealizationOfferIdentity,
};
use std::collections::BTreeMap;

const SOURCE: &str = include_str!("../../../examples/lenia-orbium.conduit");

#[test]
fn portable_demo_checks_expands_and_plans_on_one_truthful_host() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_lenia_catalogs(&mut startup, &mut profile).unwrap();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
    install_tick_presentation_catalog(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "lenia-orbium-demo", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 4);
    assert_eq!(authored.expanded.connections.len(), 3);

    let mut capabilities = vec![
        realization_offer(
            conduit_std_catalog::orbium_seed_contract(),
            conduit_alife::ORBIUM_SEED_REVISION,
            RealizationOfferIdentity {
                capability: "lenia-proof-orbium-seed",
                execution_profile: "proof/lenia-orbium-seed@1",
                implementation: "proof/lenia-orbium-seed@1",
                artifact: "proof/lenia-orbium-seed@1",
            },
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        realization_offer(
            conduit_std_catalog::lenia_step_contract(),
            conduit_alife::LENIA_STEP_REVISION,
            RealizationOfferIdentity {
                capability: "lenia-proof-step",
                execution_profile: "proof/lenia-step@1",
                implementation: "proof/lenia-step@1",
                artifact: "proof/lenia-step@1",
            },
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        realization_offer(
            conduit_std_catalog::scalar_field_presentation_contract(),
            conduit_alife::SCALAR_FIELD_PRESENTATION_REVISION,
            RealizationOfferIdentity {
                capability: "lenia-proof-presentation",
                execution_profile: "proof/lenia-presentation@1",
                implementation: "proof/lenia-presentation@1",
                artifact: "proof/lenia-presentation@1",
            },
            Vec::new(),
            vec![resource_requirement(
                conduit_core::PRESENTATION_RESOURCE_CLASS,
                1,
            )],
            Vec::new(),
        ),
    ];
    let mut every = conduit_std_catalog::realization_offer(
        conduit_std_catalog::time_every_contract(),
        conduit_time::TIME_EVERY_CONTRACT_REVISION,
        conduit_std_catalog::RealizationOfferIdentity {
            capability: "lenia-proof-time-every",
            execution_profile: "proof/lenia-time-every@1",
            implementation: "proof/lenia-time-every@1",
            artifact: "proof/lenia-time-every@1",
        },
        vec![wait_host_operation_requirement()],
        vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)],
        Vec::new(),
    );
    every.startup_parameters[0].value_type = "Duration".into();
    every.startup_parameters[0].has_default = false;
    capabilities.push(every);
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/lenia-proof"),
        boot_id: BootId::from("boot/lenia-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/lenia-proof@1"),
        resources: vec![
            conduit_core::resource_offer(
                "presentation/lenia-proof",
                conduit_core::PRESENTATION_RESOURCE_CLASS,
                1,
            ),
            conduit_core::resource_offer(
                "timer/lenia-proof",
                conduit_core::TIMER_RESOURCE_CLASS,
                1,
            ),
        ],
        planner_capabilities: vec![],
        capabilities,
    };
    let placements = conduit_planner::default_expanded_placements(
        &authored.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let field_limits = authored
        .expanded
        .connections
        .iter()
        .filter(|connection| connection.value_kind.as_str() == SCALAR_FIELD2_INFO_ID)
        .map(|connection| {
            (
                (
                    connection.source_gear_id.clone(),
                    connection.source_port_id.clone(),
                    connection.sink_gear_id.clone(),
                    connection.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 4,
                    byte_capacity: conduit_alife::LENIA_MAXIMUM_FIELD_BYTES,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let plan = conduit_planner::plan_expanded_canonical_with_connection_limits(
        &authored.expanded,
        &[host],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 4,
            connection_byte_capacity: 64,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &field_limits,
    )
    .unwrap();
    assert_eq!(plan.fragments.len(), 1);
    assert_eq!(plan.fragments[0].placements.len(), 4);
    let evolve = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == LENIA_STEP_KIND)
        .unwrap();
    assert_eq!(evolve.inputs[0].value_kind.as_str(), SCALAR_FIELD2_INFO_ID);
    assert_eq!(
        evolve.outputs[0].temporal,
        PortTemporal::Flow { closes: true }
    );
    assert!(plan.fragments[0]
        .placements
        .iter()
        .any(|placement| placement.kind_id.as_str() == SCALAR_FIELD_PRESENTATION_KIND));
    for connection in &plan.fragments[0].connections {
        let expected_bytes = if connection.value_kind.as_str() == SCALAR_FIELD2_INFO_ID {
            conduit_alife::LENIA_MAXIMUM_FIELD_BYTES
        } else {
            64
        };
        assert_eq!(connection.byte_capacity, expected_bytes);
        assert_eq!(connection.item_capacity, 4);
    }
}

#[test]
fn authored_meaning_contains_no_realization_or_partition_facts() {
    let lowered = SOURCE.to_ascii_lowercase();
    for forbidden in [
        "host/",
        "boot/",
        "pico",
        "esp32",
        "partition",
        "websocket",
        "framebuffer",
        "gpio",
    ] {
        assert!(!lowered.contains(forbidden), "leaked {forbidden}");
    }
}
