//! Checked Source with an explicitly named CPU-only pure offer fixture.
use conduit_core as c;
use conduit_plot as p;
use std::collections::BTreeMap;
pub fn plan(bytes: u32) -> c::Plan {
    let source = "plot increment (\n >> reading: U8\n result: U8 >>\n) = (reading + 1)\n";
    let checked =
        p::check_syntax_document(&p::parse_syntax_document(source), &p::StartupCatalog::new())
            .unwrap();
    let plot =
        p::expand_canonical_plot_for_authoring(&checked, "increment", &p::ProfileCatalog::new())
            .unwrap();
    let gear = &plot.expanded.gears[0];
    let c::ConfigurationValue::Text(program) = &gear.configuration[0].value else {
        panic!("program")
    };
    let program = p::PortableExpressionProgram::from_canonical_hex(program).unwrap();
    let _initialized = p::PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let offer = c::capability_offer_from_parts! {
        semantic_contract:gear.semantic_contract.clone(),startup_parameters:gear.startup_parameters.clone(),shorthand:gear.shorthand.clone(),
        capability_id:"fixture/increment".into(),kind_id:gear.kind_id.clone(),kind_contract_revision:gear.kind_contract_revision.clone(),
        implementation:c::ImplementationOffer {execution_profile_id:"fixture/pure".into(),implementation_id:"fixture/increment".into(),artifact_id:"fixture/increment".into()},
        inputs:gear.inputs.clone(),outputs:gear.outputs.clone(),host_calls:vec![],resource_requirements:vec![],authority_requirements:vec![],
        limits:c::CapabilityLimits {max_active_instances:1,max_queue_items:1,max_queue_bytes:1024},
    };
    let host = c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "fixture/host".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/pure".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![offer],
        planner_capabilities: vec![],
    };
    let hosts = [host];
    let choices = conduit_planner::default_expanded_placements(&plot.expanded, &hosts).unwrap();
    let mut fore = BTreeMap::new();
    for (direction, bindings) in [
        (c::PortDirection::Input, &plot.input_bindings),
        (c::PortDirection::Output, &plot.output_bindings),
    ] {
        for binding in bindings {
            fore.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                },
            );
        }
    }
    conduit_planner::plan_expanded_authoring_with_options(
        &plot,
        &hosts,
        &choices,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: bytes,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &fore,
    )
    .unwrap()
}
