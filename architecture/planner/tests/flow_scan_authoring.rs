use conduit_core::{
    kind_id, port_id, verify_plan, ArtifactId, BaseImplementationId, BootId, CapabilityId,
    CapabilityLimits, ExecutionProfileId, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, KindIdentity, OfferGeneration, PlannedActivationEntry, PortDescriptor,
    PortDirection, PortTemporal, PROTOCOL_VERSION,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_activations, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CanonicalBackCatalog, KindProjection, KindSignature, ProfileCatalog, StartupCatalog,
};
use std::collections::BTreeMap;

const SOURCE: &str = r#"
plot test/transition (
    >> accumulator: Text
    >> item: Text
    combined: Text >>
) {
    combine: test/combine
    accumulator >> combine.accumulator
    item >> combine.item
    combine.combined >> combined
}

plot test/scan-root (
    >> commands: Text...|
    states: Text...| >>
) {
    reducer: scan("seed", maximum-items = 64) test/transition()
    commands >> reducer.item
    reducer.combined >> states
}
"#;

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id("value/text"),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }
}

#[test]
fn sixty_four_item_scan_keeps_exact_child_and_top_level_fore() {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: "test/combine".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let mut profile = ProfileCatalog::new();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/combine"),
            kind_contract_revision: KindIdentity::from("test/combine@1"),
            inputs: vec![
                port("accumulator", PortDirection::Input),
                port("item", PortDirection::Input),
            ],
            outputs: vec![port("combined", PortDirection::Output)],
            configuration: vec![],
        })
        .unwrap();
    let document = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    let authoring =
        expand_canonical_plot_for_authoring(&document, "test/scan-root", &profile).unwrap();
    let child =
        expand_canonical_plot_for_authoring(&document, "test/transition", &profile).unwrap();
    let capabilities = authoring
        .expanded
        .gears
        .iter()
        .chain(child.expanded.gears.iter())
        .enumerate()
        .map(|(index, gear)| {
            conduit_core::capability_offer_from_parts! {
                semantic_contract: gear.semantic_contract.clone(),
                startup_parameters: gear.startup_parameters.clone(),
                shorthand: gear.shorthand.clone(),
                capability_id: CapabilityId::from(format!("scan-test/{index}")),
                kind_id: gear.kind_id.clone(),
                kind_contract_revision: gear.kind_contract_revision.clone(),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: ExecutionProfileId::from(format!("scan-test/{index}")),
                    implementation_id: ImplementationId::from(format!("scan-test/{index}")),
                    artifact_id: ArtifactId::from(format!("scan-test/{index}")),
                },
                inputs: gear.inputs.clone(),
                outputs: gear.outputs.clone(),
                host_calls: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 64,
                    max_queue_items: 3,
                    max_queue_bytes: 1024,
                },
            }
        })
        .collect();
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("scan-host"),
        boot_id: BootId::from("scan-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("scan-profile"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities,
    }];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let empty_bases = BTreeMap::new();
    let empty_lines = BTreeMap::new();
    let options = PlanningOptions {
        connection_bases: &empty_bases,
        line_candidates: &empty_lines,
        connection_item_capacity: 1,
        connection_byte_capacity: 512,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let bounds = BTreeMap::from([
        (
            ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id("commands"),
                track: conduit_core::ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 256,
            },
        ),
        (
            ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: port_id("states"),
                track: conduit_core::ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 256,
            },
        ),
    ]);
    let plan = plan_expanded_authoring_with_activations(
        &document,
        &authoring,
        &profile,
        &CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        options,
        &bounds,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    assert_eq!(plan.fragments[0].fore_ports.len(), 2);
    let PlannedActivationEntry::Scan(scan) = &plan.activations[0] else {
        panic!("the authored scan must select a scan activation")
    };
    assert_eq!(scan.limits.maximum_items, 64);
    assert_eq!(scan.selected_plan_id, scan.selected_plan.plan_id);
    assert!(scan.selected_plan.fragments.iter().any(|fragment| fragment
        .placements
        .iter()
        .any(|placement| placement.kind_id == kind_id("test/combine"))));
    assert_eq!(scan.selected_plan.fragments[0].fore_ports.len(), 3);

    let mut undersized = hosts.clone();
    undersized[0].capabilities[0].limits.max_queue_bytes = 512;
    let undersized_placements =
        default_expanded_placements(&authoring.expanded, &undersized).unwrap();
    assert!(plan_expanded_authoring_with_activations(
        &document,
        &authoring,
        &profile,
        &CanonicalBackCatalog::new(),
        &undersized,
        &undersized_placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        options,
        &bounds,
    )
    .is_err());
}
