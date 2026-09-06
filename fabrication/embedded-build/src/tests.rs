use conduit_core::{
    mandatory_sign_storage_requirement, seal_plan, ArtifactId, BaseImplementationId, BootId,
    CancellationPolicy, CapabilityId, CapabilityLimits, CheckedFormId, ConfigurationEntry,
    ConfigurationValue, ConnectionId, ExecutionProfileId, ExpandedFormId, ExpectedSign,
    ExpectedTerminal, FormIdentity, FragmentId, GearId, HostId, ImplementationId,
    KindContractRevision, KindId, OfferGeneration, PlacementId, PlanFragment, PlanId,
    PlannedConnection, PlannedGear, PortDescriptor, PortDirection, PortId, SignStorageBudget,
    SourceDocumentId, StartupDependency, TerminalPolicy,
};
use conduit_plan_lowering::lowering::{lower_plan_fragment, FIXED_KERNEL_STORAGE_PORTS_PER_NODE};
use conduit_signal::{signal_profile_catalog, SIGNAL_ENCODED_LEN};
use conduit_signal_conformance::{
    pico_local_advertisement, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, PICO_LOCAL_HOST_ID,
};

use crate::validate::validate_range;
use crate::{
    generate_embedded_plan, EmbeddedImageBounds, GeneratedConfigurationEntry,
    GeneratedConfigurationValue, GeneratedEmbeddedPlan, GeneratedPort, GeneratedStaticNode,
    GenerationError, GENERATED_EMBEDDED_PLAN_SCHEMA_VERSION,
};

#[test]
fn current_fragment_lowers_into_one_deterministic_fixed_image() {
    let fragment = sealed_current_fragment();
    let lowered = lower_plan_fragment(&fragment).expect("current fragment lowers");
    let generated = generate_embedded_plan(&fragment, &lowered, EmbeddedImageBounds::HOST_TOOLING)
        .expect("current lowered fragment generates");

    assert_eq!(generated.plan_id, fragment.plan_id.as_str());
    assert_eq!(generated.fragment_id, fragment.fragment_id.as_str());
    assert_eq!(generated.nodes.len(), 2);
    assert_eq!(generated.cords.len(), 1);
    assert_eq!(generated.routes.len(), 1);
    assert_eq!(generated.route_targets.len(), 1);
    assert_eq!(generated.cord_value_slots, 1);
    assert_eq!(generated.cord_value_bytes, 9);
    assert_eq!(
        generated.configuration,
        vec![GeneratedConfigurationEntry {
            node: 0,
            key: "count".to_owned(),
            value: GeneratedConfigurationValue::U64(16),
        }]
    );

    let first = generated.render_rust_module();
    let second = generated.render_rust_module();
    assert_eq!(first, second);
    assert!(first.contains("conduit_kernel::CordEndpoint::local"));
    assert!(first.contains("pub const GENERATED_PLACEMENT_IDS"));
    assert!(!first.contains("ExecutionPlan"));
}

#[test]
fn current_fragment_generation_rejects_a_reviewed_bound_overflow() {
    let fragment = sealed_current_fragment();
    let lowered = lower_plan_fragment(&fragment).expect("current fragment lowers");
    let bounds = EmbeddedImageBounds {
        maximum_nodes: 1,
        ..EmbeddedImageBounds::HOST_TOOLING
    };

    assert_eq!(
        generate_embedded_plan(&fragment, &lowered, bounds),
        Err(GenerationError::BoundExceeded {
            table: "nodes",
            actual: 2,
            maximum: 1,
        })
    );
}

#[test]
fn hosted_text_configuration_is_not_promoted_to_embedded_execution() {
    let mut fragment = sealed_current_fragment();
    fragment.placements[0].configuration[0].value =
        ConfigurationValue::Text("hosted only".to_owned());
    let identity = FormIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_form_id: fragment.checked_form_id.clone(),
        expanded_form_id: fragment.expanded_form_id.clone(),
    };
    let fragment = seal_plan(identity, vec![fragment])
        .fragments
        .into_iter()
        .next()
        .unwrap();
    let lowered = lower_plan_fragment(&fragment).expect("text configuration still lowers");
    assert_eq!(
        generate_embedded_plan(&fragment, &lowered, EmbeddedImageBounds::HOST_TOOLING),
        Err(GenerationError::Unsupported(
            crate::UnsupportedPlanFeature::TextConfiguration
        ))
    );
}

#[test]
fn signed_scalar_configuration_remains_exact_in_a_fixed_image() {
    let mut fragment = sealed_current_fragment();
    fragment.placements[0].configuration[0].value = ConfigurationValue::I64(-7);
    let identity = FormIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_form_id: fragment.checked_form_id.clone(),
        expanded_form_id: fragment.expanded_form_id.clone(),
    };
    let fragment = seal_plan(identity, vec![fragment])
        .fragments
        .into_iter()
        .next()
        .unwrap();
    let lowered = lower_plan_fragment(&fragment).expect("signed configuration lowers");
    let generated =
        generate_embedded_plan(&fragment, &lowered, EmbeddedImageBounds::HOST_TOOLING).unwrap();
    assert_eq!(
        generated.configuration[0].value,
        GeneratedConfigurationValue::I64(-7)
    );
    assert!(generated
        .render_rust_module()
        .contains("conduit_core::ConfigurationValue::I64(-7)"));
}

#[test]
fn unchanged_signal_form_plans_lowers_and_generates_one_fixed_image() {
    let form = conduit_form::parse_with_startup(
        include_str!("../../../proof/fixtures/forms/signal-demo.conduit"),
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .expect("unchanged Signal form checks");
    let host = pico_local_advertisement();
    let placements = conduit_planner::default_placements(&form, std::slice::from_ref(&host))
        .expect("Pico profile covers the unchanged Signal form");
    let plan = conduit_planner::plan_with_connection_limits(
        &form,
        std::slice::from_ref(&host),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        SIGNAL_ENCODED_LEN,
    )
    .expect("unchanged Signal form plans locally");
    let fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == PICO_LOCAL_HOST_ID)
        .expect("Pico plan contains its exact fragment");
    let lowered = lower_plan_fragment(fragment).expect("Pico fragment lowers");
    let generated = generate_embedded_plan(
        fragment,
        &lowered,
        EmbeddedImageBounds {
            maximum_nodes: 2,
            maximum_cords: 1,
            maximum_routes: 1,
            maximum_route_targets: 1,
            maximum_host_operations: 2,
            maximum_resources: 2,
            maximum_sign_expectations: 8,
            maximum_configuration_entries: 3,
            maximum_ports_per_node: FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            maximum_remote_endpoints: 0,
            maximum_cord_value_slots: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            maximum_cord_value_bytes: SIGNAL_ENCODED_LEN,
            maximum_sign_items: 16,
            maximum_sign_bytes: 1024,
        },
    )
    .expect("Pico fragment fits the firmware image contract");

    assert_eq!(generated.plan_id, plan.plan_id.as_str());
    assert_eq!(generated.fragment_id, fragment.fragment_id.as_str());
    assert_eq!(generated.nodes.len(), 2);
    assert_eq!(generated.cords.len(), 1);
    assert_eq!(generated.routes.len(), 1);
    assert_eq!(generated.route_targets.len(), 1);
    assert_eq!(
        generated.cord_value_slots,
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS
    );
    assert_eq!(generated.cord_value_bytes, SIGNAL_ENCODED_LEN);
    let rendered = generated.render_no_alloc_firmware_module();
    assert!(rendered.contains("pub const GENERATED_NODES"));
    assert!(rendered.contains("pub const GENERATED_CORDS"));
    assert!(rendered.contains("pub const GENERATED_ROUTES"));
    assert!(rendered.contains("pub const GENERATED_HOST_OPERATIONS"));
    assert!(!rendered.contains("ExecutionPlan"));
}

#[test]
fn generation_rejects_lowering_with_a_different_fragment_identity() {
    let fragment = sealed_current_fragment();
    let mut lowered = lower_plan_fragment(&fragment).expect("current fragment lowers");
    lowered.identity.fragment_id = FragmentId::from("mutated-fragment");

    assert_eq!(
        generate_embedded_plan(&fragment, &lowered, EmbeddedImageBounds::HOST_TOOLING),
        Err(GenerationError::IdentityMismatch)
    );
}

#[test]
fn generation_rejects_inconsistent_lowered_node_identity() {
    let fragment = sealed_current_fragment();
    let mut lowered = lower_plan_fragment(&fragment).expect("current fragment lowers");
    lowered.nodes[0].placement_id = PlacementId::from("mutated-placement");

    assert!(matches!(
        generate_embedded_plan(&fragment, &lowered, EmbeddedImageBounds::HOST_TOOLING),
        Err(GenerationError::InconsistentLowering(_))
    ));
}

#[test]
fn generation_rejects_an_unsupported_remote_cord() {
    let fragment = sealed_current_fragment();
    let mut lowered = lower_plan_fragment(&fragment).expect("current fragment lowers");
    lowered.cords[0].spec.sink =
        conduit_kernel::CordEndpoint::Remote(conduit_kernel::RemoteEndpointId(0));

    assert_eq!(
        generate_embedded_plan(&fragment, &lowered, EmbeddedImageBounds::HOST_TOOLING),
        Err(GenerationError::Unsupported(
            crate::UnsupportedPlanFeature::RemoteConnection
        ))
    );
}

#[test]
fn renderer_emits_fixed_current_kernel_tables() {
    let generated = GeneratedEmbeddedPlan {
        schema_version: GENERATED_EMBEDDED_PLAN_SCHEMA_VERSION,
        plan_id: "plan-1".to_owned(),
        fragment_id: "fragment-1".to_owned(),
        host_id: "host-1".to_owned(),
        boot_id: "boot-1".to_owned(),
        offer_generation: 7,
        cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
        terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
        nodes: vec![GeneratedStaticNode {
            node: 0,
            placement_id: "pulse".to_owned(),
            kind_id: "flow/pulse".to_owned(),
            implementation_id: "signal/pulse".to_owned(),
            artifact_id: "artifact/pulse".to_owned(),
            input_cords: [None; FIXED_KERNEL_STORAGE_PORTS_PER_NODE],
            maximum_step_work: 2,
        }],
        input_ports: Vec::new(),
        output_ports: vec![GeneratedPort {
            node: 0,
            port: 0,
            port_id: "out".to_owned(),
            value_kind: "value/signal".to_owned(),
        }],
        configuration: vec![GeneratedConfigurationEntry {
            node: 0,
            key: "count".to_owned(),
            value: GeneratedConfigurationValue::U64(16),
        }],
        cords: Vec::new(),
        remote_endpoints: Vec::new(),
        routes: Vec::new(),
        route_targets: Vec::new(),
        host_operations: Vec::new(),
        resources: Vec::new(),
        signs: Vec::new(),
        startup_dependencies: Vec::new(),
        startup_order: vec![0],
        expected_terminals: Vec::new(),
        cord_value_slots: 0,
        cord_value_bytes: 0,
        sign_items: 1,
        sign_bytes: 16,
    };

    let module = generated.render_rust_module();
    assert!(module.contains("pub const PLAN_ID: &str = \"plan-1\";"));
    assert!(module.contains("conduit_kernel::scheduler::NodeSpec<16>"));
    assert!(module.contains("conduit_core::ConfigurationValue::U64(16)"));
    assert!(module.contains("pub const GENERATED_STARTUP_ORDER"));
    assert!(module.contains("CancelAllAndRejectLateCompletion"));
    assert!(!module.contains("ExecutionPlan"));
}

#[test]
fn range_validation_rejects_out_of_bounds_ranges() {
    assert_eq!(
        validate_range("slots", u16::MAX, 1, u16::MAX),
        Err(GenerationError::InvalidRange {
            table: "slots",
            start: u64::from(u16::MAX),
            length: 1,
            limit: u64::from(u16::MAX),
        })
    );
}

fn sealed_current_fragment() -> PlanFragment {
    let source = PlacementId::from("source");
    let sink = PlacementId::from("sink");
    let connection = ConnectionId::from("source-to-sink");
    let value_kind = KindId::from("value/test");
    let host_id = HostId::from("host/test");
    let boot_id = BootId::from("boot/test");
    let output = PortDescriptor {
        port_id: PortId::from("out"),
        value_kind: value_kind.clone(),
        direction: PortDirection::Output,
        temporal: conduit_core::PortTemporal::Value,
    };
    let input = PortDescriptor {
        port_id: PortId::from("in"),
        value_kind: value_kind.clone(),
        direction: PortDirection::Input,
        temporal: conduit_core::PortTemporal::Value,
    };
    let expected_sign = vec![
        ExpectedSign::PlanFragmentReceived,
        ExpectedSign::PlacementPrepared(source.clone()),
        ExpectedSign::PlacementPrepared(sink.clone()),
        ExpectedSign::PlacementTerminal(source.clone()),
        ExpectedSign::PlacementTerminal(sink.clone()),
        ExpectedSign::ConnectionTerminal(connection.clone()),
        ExpectedSign::PlanTerminal,
    ];
    let sign_storage_budget =
        mandatory_sign_storage_requirement(&expected_sign).unwrap_or(SignStorageBudget {
            item_capacity: 0,
            byte_capacity: 0,
        });
    let form_identity = FormIdentity {
        source_document_id: SourceDocumentId::from("source/test"),
        checked_form_id: CheckedFormId::from("checked/test"),
        expanded_form_id: ExpandedFormId::from("expanded/test"),
    };
    let fragment = PlanFragment {
        plan_id: PlanId::from(""),
        fragment_id: FragmentId::from(""),
        source_document_id: form_identity.source_document_id.clone(),
        checked_form_id: form_identity.checked_form_id.clone(),
        expanded_form_id: form_identity.expanded_form_id.clone(),
        realization_backs: Vec::new(),
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        offer_generation: OfferGeneration(1),
        placements: vec![
            PlannedGear {
                placement_id: source.clone(),
                gear_id: GearId::from("source"),
                kind_id: KindId::from("test/source"),
                kind_contract_revision: KindContractRevision::from("test/source@1"),
                execution_profile_id: ExecutionProfileId::from("test/source-fixed@1"),
                configuration: vec![ConfigurationEntry {
                    key: "count".to_owned(),
                    value: ConfigurationValue::U64(16),
                }],
                host_id: host_id.clone(),
                boot_id: boot_id.clone(),
                offer_generation: OfferGeneration(1),
                capability_id: CapabilityId::from("source-capability"),
                implementation_id: ImplementationId::from("test/source-impl"),
                artifact_id: ArtifactId::from("test/source-artifact"),
                realization_characteristics: Vec::new(),
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 16,
                    max_queue_bytes: 16,
                },
                inputs: Vec::new(),
                outputs: vec![output],
                host_operations: Vec::new(),
                resources: Vec::new(),
                authority: Vec::new(),
                pool_references: Vec::new(),
            },
            PlannedGear {
                placement_id: sink.clone(),
                gear_id: GearId::from("sink"),
                kind_id: KindId::from("test/sink"),
                kind_contract_revision: KindContractRevision::from("test/sink@1"),
                execution_profile_id: ExecutionProfileId::from("test/sink-fixed@1"),
                configuration: Vec::new(),
                host_id: host_id.clone(),
                boot_id: boot_id.clone(),
                offer_generation: OfferGeneration(1),
                capability_id: CapabilityId::from("sink-capability"),
                implementation_id: ImplementationId::from("test/sink-impl"),
                artifact_id: ArtifactId::from("test/sink-artifact"),
                realization_characteristics: Vec::new(),
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 16,
                    max_queue_bytes: 16,
                },
                inputs: vec![input],
                outputs: Vec::new(),
                host_operations: Vec::new(),
                resources: Vec::new(),
                authority: Vec::new(),
                pool_references: Vec::new(),
            },
        ],
        execution_regions: vec![],
        execution_fusions: vec![],
        states: Vec::new(),
        connections: vec![PlannedConnection {
            connection_id: connection.clone(),
            source_placement_id: source.clone(),
            source_port_id: PortId::from("out"),
            sink_placement_id: sink.clone(),
            sink_port_id: PortId::from("in"),
            value_kind,
            temporal: conduit_core::PortTemporal::Value,
            selected_line: None,
            admitted_lines: vec![],
            item_capacity: 1,
            byte_capacity: 9,
        }],
        shared_pools: Vec::new(),
        startup_dependencies: vec![StartupDependency {
            prerequisite_placement_id: source.clone(),
            dependent_placement_id: sink.clone(),
        }],
        startup_order: vec![source.clone(), sink.clone()],
        cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
        terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
        expected_terminals: vec![
            ExpectedTerminal::PlacementCompleted(source),
            ExpectedTerminal::PlacementCompleted(sink),
            ExpectedTerminal::ConnectionCompleted(connection),
            ExpectedTerminal::PlanCompleted,
        ],
        expected_sign,
        sign_storage_budget,
        plan_fragments: Vec::new(),
    };

    seal_plan(form_identity, vec![fragment])
        .fragments
        .into_iter()
        .next()
        .expect("sealed plan retains its fragment")
}

#[test]
fn signal_demo_remote_usb_cdc_ingress_generates_embedded_plan() {
    use conduit_core::{
        seal_plan, AdmittedLine, BaseInstanceId, FormIdentity, LineContinuation, LineContract,
        LineDuplex, LineId, LineOrdering, LineReliability, LineScope, LineSecurity,
        LineTrafficShape, LinkAuthorityReference, LinkBinding, LinkCredentialReference,
        LinkEndpoint, LinkEndpointId, LinkLimits,
    };
    use conduit_plan_lowering::lowering::RemoteCordDirection;

    let source_fragment = sealed_current_fragment();
    let form_identity = FormIdentity {
        source_document_id: source_fragment.source_document_id.clone(),
        checked_form_id: source_fragment.checked_form_id.clone(),
        expanded_form_id: source_fragment.expanded_form_id.clone(),
    };

    let pico_host_id = HostId::from("pico-host");
    let pico_boot_id = BootId::from("pico-boot");
    let mut pico_fragment = source_fragment.clone();
    pico_fragment.host_id = pico_host_id.clone();
    pico_fragment.boot_id = pico_boot_id.clone();
    pico_fragment
        .placements
        .retain(|p| p.placement_id.as_str() == "sink");
    pico_fragment.placements[0].host_id = pico_host_id.clone();
    pico_fragment.placements[0].boot_id = pico_boot_id.clone();
    pico_fragment.connections[0].sink_placement_id = PlacementId::from("sink");
    let binding = LinkBinding {
        binding_id: conduit_core::LinkBindingId::from("link/usb-cdc"),
        source: LinkEndpoint {
            host_id: source_fragment.host_id.clone(),
            boot_id: source_fragment.boot_id.clone(),
            endpoint_id: LinkEndpointId::from("std-out"),
        },
        sink: LinkEndpoint {
            host_id: pico_host_id.clone(),
            boot_id: pico_boot_id.clone(),
            endpoint_id: LinkEndpointId::from("pico-in"),
        },
        base: BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
        base_instance_id: BaseInstanceId::from("pico-usb-cdc-0"),
        credential: LinkCredentialReference::None,
        authority: LinkAuthorityReference::ProcessOwned,
        limits: LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 9,
            maximum_buffered_bytes: 9,
            maximum_frame_bytes: 512,
        },
    };
    let admitted = AdmittedLine {
        line_id: LineId::from("line/usb-cdc"),
        binding: binding.bound_link(),
        contract: LineContract {
            scope: LineScope::Machine,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::PhysicalPossession,
        },
    };
    pico_fragment.connections[0].selected_line = Some(admitted.clone());
    pico_fragment.connections[0].admitted_lines = vec![admitted.clone()];

    let sink = PlacementId::from("sink");
    let connection = ConnectionId::from("source-to-sink");
    let expected_sign = vec![
        ExpectedSign::PlanFragmentReceived,
        ExpectedSign::PlacementPrepared(sink.clone()),
        ExpectedSign::PlacementTerminal(sink.clone()),
        ExpectedSign::ConnectionTerminal(connection.clone()),
        ExpectedSign::PlanTerminal,
    ];
    let sign_storage_budget =
        mandatory_sign_storage_requirement(&expected_sign).expect("sign budget");
    pico_fragment.expected_sign = expected_sign;
    pico_fragment.sign_storage_budget = sign_storage_budget;
    pico_fragment.expected_terminals = vec![
        ExpectedTerminal::PlacementCompleted(sink),
        ExpectedTerminal::ConnectionCompleted(connection),
        ExpectedTerminal::PlanCompleted,
    ];
    pico_fragment.startup_dependencies.clear();
    pico_fragment.startup_order = vec![PlacementId::from("sink")];

    let mut std_fragment = source_fragment;
    std_fragment
        .placements
        .retain(|p| p.placement_id.as_str() == "source");
    std_fragment.connections[0].selected_line = Some(admitted.clone());
    std_fragment.connections[0].admitted_lines = vec![admitted];
    std_fragment.startup_dependencies.clear();
    std_fragment.startup_order = vec![PlacementId::from("source")];
    let source_placement = PlacementId::from("source");
    let connection_id = ConnectionId::from("source-to-sink");
    let std_sign = vec![
        ExpectedSign::PlanFragmentReceived,
        ExpectedSign::PlacementPrepared(source_placement.clone()),
        ExpectedSign::PlacementTerminal(source_placement.clone()),
        ExpectedSign::ConnectionTerminal(connection_id.clone()),
        ExpectedSign::PlanTerminal,
    ];
    let std_budget = mandatory_sign_storage_requirement(&std_sign).expect("std budget");
    std_fragment.expected_sign = std_sign;
    std_fragment.sign_storage_budget = std_budget;
    std_fragment.expected_terminals = vec![
        ExpectedTerminal::PlacementCompleted(source_placement),
        ExpectedTerminal::ConnectionCompleted(connection_id),
        ExpectedTerminal::PlanCompleted,
    ];

    let sealed_plan = seal_plan(form_identity, vec![std_fragment, pico_fragment]);
    let sealed_pico = sealed_plan
        .fragments
        .into_iter()
        .find(|f| f.host_id == pico_host_id)
        .expect("sealed plan retains pico fragment");

    let lowered = lower_plan_fragment(&sealed_pico).expect("remote pico fragment lowers");
    assert_eq!(lowered.remote_endpoints.len(), 1);
    assert_eq!(
        lowered.remote_endpoints[0].direction,
        RemoteCordDirection::Ingress
    );

    let generated =
        generate_embedded_plan(&sealed_pico, &lowered, EmbeddedImageBounds::HOST_TOOLING)
            .expect("remote pico fragment generates embedded plan");

    assert_eq!(generated.remote_endpoints.len(), 1);
    assert_eq!(
        generated.remote_endpoints[0].base,
        BaseImplementationId::from("conduit.base/usb-cdc-acm@1")
    );
    assert_eq!(
        generated.remote_endpoints[0].link_binding_id,
        "link/usb-cdc"
    );
}
