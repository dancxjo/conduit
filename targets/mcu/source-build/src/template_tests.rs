use super::*;
#[test]
fn authored_resident_fore_lowers_independently_and_preserves_source_node() {
    let checked = crate::checked_dht11_tm1637_source().unwrap();
    let plot = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "dht11-capture-request",
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap();
    let gear = &plot.expanded.gears[0];
    let c::ConfigurationValue::Text(program) = &gear.configuration[0].value else {
        panic!("checked expression program");
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(program).unwrap();
    let image = e::PreparedResidentExpression::prepare(
        &checked,
        &program,
        gear.inputs[0].temporal,
        e::ResidentExpressionBounds {
            native: conduit_plot::rust_binding::PreparedNativeBounds {
                canonical_bytes: 4096,
                scopes: 64,
                invariants: 64,
                contracts: 64,
            },
            maximum_occurrences: 4096,
            maximum_program_bytes: 512 * 1024,
            maximum_generated_bytes: 1024 * 1024,
            private_work_units: 1113,
        },
    )
    .unwrap();
    let host = c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "template/descriptive-host".into(),
        boot_id: "template/descriptive-boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "template/pure-resident".into(),
        bases: vec![],
        resources: vec![c::ResourceOffer {
            pool_id: "template/reviewed-work".into(),
            class_id: c::COOPERATIVE_STEP_WORK_CLASS_ID.into(),
            capacity_units: 1113,
            compute: None,
            content: None,
        }],
        planner_capabilities: vec![],
        capabilities: vec![image.offer_description().clone()],
    };
    let placements = p::PlacementChoices {
        by_gear: [(
            gear.gear_id.clone(),
            p::PlacementChoice {
                host_id: host.host_id.clone(),
                capability_id: image.offer_description().capability_id.clone(),
            },
        )]
        .into(),
    };
    let queue_bytes = image.offer_description().limits.max_queue_bytes;
    let mut boundaries = BTreeMap::new();
    for (direction, bindings) in [
        (c::PortDirection::Input, &plot.input_bindings),
        (c::PortDirection::Output, &plot.output_bindings),
    ] {
        for binding in bindings {
            boundaries.insert(
                p::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                p::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: queue_bytes,
                },
            );
        }
    }
    let empty = BTreeMap::new();
    let connections = BTreeMap::new();
    let residents = [&image];
    let make = |host, boundaries| {
        compile(Inputs {
            plot: &plot,
            host,
            placements: &placements,
            planning: p::PlanningOptions {
                connection_bases: &empty,
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: queue_bytes,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            connections: &connections,
            boundaries,
            resident: &residents,
            other: &[],
            image: e::EmbeddedImageBounds {
                maximum_nodes: 1,
                maximum_remote_endpoints: 0,
                ..e::EmbeddedImageBounds::HOST_TOOLING
            },
            computations: e::EmbeddedComputationBounds {
                maximum_program_bytes: 512 * 1024,
                maximum_total_program_bytes: 512 * 1024,
            },
            maxima: c::AssignedPlanMaxima::TINY_HOST,
            maximum_fore_ports: 2,
        })
    };
    let layout = make(&host, &boundaries).unwrap();
    let mut restarted = host.clone();
    restarted.boot_id = "template/later-boot".into();
    restarted.offer_generation = c::OfferGeneration(2);
    let later = make(&restarted, &boundaries).unwrap();
    assert_eq!(layout.nodes, later.nodes);
    assert_eq!(
        layout.terminal_transductions_rust,
        later.terminal_transductions_rust
    );
    assert!(layout
        .terminal_transductions_rust
        .contains("GENERATED_TERMINAL_TRANSDUCTIONS"));
    assert_eq!(
        layout.continuation, later.continuation,
        "named Fore/resource pins are immutable across Boot changes"
    );
    assert_eq!(layout.continuation.fore.len(), layout.fore.len());
    for fore in &layout.continuation.fore {
        assert!(!fore.front.is_empty() && !fore.gear.is_empty());
        assert_eq!(
            layout.fragment_seal_placement_ids[layout
                .fragment_seal_to_kernel
                .iter()
                .position(|node| *node == fore.node)
                .unwrap()],
            fore.placement
        );
        assert!(layout.fore.iter().any(|pin| pin.cord() == fore.cord
            && pin.endpoint() == fore.endpoint
            && pin.payload()[5..21] == c::AssignedIdentity::from_text(&fore.front).0));
    }
    let emitted = layout.continuation.emit();
    assert!(emitted.contains("pub static FRESH_FORE"));
    assert!(emitted.contains("pub static CONTINUATION_RESOURCES"));
    assert_eq!(
        layout.fragment_seal_placement_ids,
        later.fragment_seal_placement_ids
    );
    assert_eq!(
        c::fragment_seal_recipe_identity(layout.fragment_seal_recipe.pieces()),
        c::fragment_seal_recipe_identity(later.fragment_seal_recipe.pieces())
    );
    assert_eq!(layout.counts, later.counts);
    assert_eq!(
        layout.projection_template_digest,
        later.projection_template_digest
    );
    c::AssignedLocalProjectionTemplate::check(
        &layout.projection_template,
        &layout.projection_template_digest,
        layout.maxima,
    )
    .unwrap();
    super::continuation_tests::assert_unchanged_fore(&layout, &host, &restarted);
    assert_eq!(layout.records, later.records);
    assert_eq!(layout.configurations, later.configurations);
    assert!(!layout.configurations.is_empty());
    assert!(layout
        .configurations
        .iter()
        .all(|r| r.tag == c::ASSIGNED_CONFIGURATION));
    assert_eq!(layout.nodes, vec![(0, gear.gear_id.clone())]);
    assert_eq!(layout.counts[usize::from(c::ASSIGNED_NODE - 1)], 1);
    assert_eq!(layout.fore.len(), 2);
    assert!(!layout.records.is_empty());
    assert!(layout.records.iter().all(|r| !matches!(
        r.tag,
        c::ASSIGNED_CONFIGURATION | c::ASSIGNED_SIGN | c::ASSIGNED_TERMINAL
    )));
    assert!(
        make(&host, &BTreeMap::new()).is_err(),
        "missing independently selected Fore refuses"
    );
}
