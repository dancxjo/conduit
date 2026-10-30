//! Checked Source and ordinary planning; pure offer identity is a CPU fixture.
//! No physical provider, execution, membership or native authority is claimed.
use conduit_core as c;
use conduit_embedded_build as b;
use conduit_plot as p;
use std::collections::BTreeMap;
fn plan(bytes: u32) -> c::Plan {
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
fn generate(plan: &c::Plan) -> b::GeneratedForePlan {
    b::generate_fore_native_plan(
        plan,
        &plan.fragments[0].fragment_id,
        b::EmbeddedImageBounds::HOST_TOOLING,
        b::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 65536,
        },
        2,
    )
    .unwrap()
}
#[test]
fn checked_local_fore_projects_exactly_and_legacy_paths_refuse_it() {
    let plan = plan(64);
    let fragment = &plan.fragments[0];
    let generated = generate(&plan);
    assert_eq!(generated.fore().len(), 2);
    assert!(generated.kernel().remote_endpoints.is_empty());
    assert!(generated.kernel().host_calls.is_empty());
    assert!(generated.kernel().resources.is_empty());
    let maxima = c::AssignedPlanMaxima {
        encoded_bytes: 16384,
        runtime_state_bytes: 32768,
        counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
    };
    let bytes = generated.encode(maxima).unwrap();
    assert_eq!(bytes, generated.encode(maxima).unwrap());
    let computations = b::assigned_computation_requirements(generated.kernel()).unwrap();
    let profile = c::AssignedComputationProfile {
        maximum_text_bytes: u16::MAX,
        maximum_total_text_bytes: u32::MAX,
        requirements: &computations,
    };
    let required = c::AssignedPlanRequirements {
        host: c::AssignedIdentity::from_text(fragment.host_id.as_str()),
        boot: c::AssignedIdentity::from_text(fragment.boot_id.as_str()),
        host_calls: &[],
        resources: &[],
        remote_bindings: &[],
    };
    let identity =
        c::decode_assigned_plan_with_fore(&bytes, maxima, required, profile, generated.fore())
            .unwrap();
    assert_eq!(
        identity.plan,
        c::AssignedIdentity::from_text(plan.plan_id.as_str())
    );
    let records = c::AssignedPlanRecords::new(&bytes)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        c::decode_assigned_records_with_fore(
            &bytes[..125],
            &records,
            maxima,
            required,
            profile,
            generated.fore()
        )
        .unwrap(),
        identity
    );
    assert!(c::decode_assigned_plan_with_computations(&bytes, maxima, required, profile).is_err());
    assert!(b::encode_assigned_native_plan(generated.kernel(), maxima).is_err());
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    assert!(matches!(
        b::generate_embedded_plan_with_computations(
            fragment,
            &lowered,
            b::EmbeddedImageBounds::HOST_TOOLING,
            b::EmbeddedComputationBounds {
                maximum_program_bytes: 32768,
                maximum_total_program_bytes: 65536
            }
        ),
        Err(b::GenerationError::Unsupported(
            b::UnsupportedPlanFeature::LocalFore
        ))
    ));
    assert!(b::generate_fore_native_plan(
        &plan,
        &fragment.fragment_id,
        b::EmbeddedImageBounds::HOST_TOOLING,
        b::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 65536
        },
        1
    )
    .is_err());
    let changed = generate(&self::plan(128));
    assert_ne!(generated.fore(), changed.fore());
    assert!(
        c::decode_assigned_plan_with_fore(&bytes, maxima, required, profile, changed.fore())
            .is_err()
    );
    let mut broken = plan.clone();
    broken.fragments[0].fore_ports.clear();
    assert!(b::generate_fore_native_plan(
        &broken,
        &fragment.fragment_id,
        b::EmbeddedImageBounds::HOST_TOOLING,
        b::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 65536
        },
        2
    )
    .is_err());
    assert!(!generated.layout_requirements(maxima).unwrap().is_empty());
}
