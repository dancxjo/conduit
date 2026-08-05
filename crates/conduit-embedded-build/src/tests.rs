use conduit_core::{
    mandatory_evidence_storage_requirement, seal_plan, ArtifactId, BootId, CancellationPolicy,
    CapabilityId, CheckedFormId, ConfigurationEntry, ConfigurationValue, ConnectionId,
    ConnectionProvider, EvidenceStorageBudget, ExecutionProfileId, ExpandedFormId,
    ExpectedEvidence, ExpectedTerminal, FormIdentity, FragmentId, HostId, ImplementationId,
    KindContractRevision, KindId, OfferGeneration, OperationId, PlacementId, PlanFragment, PlanId,
    PlannedConnection, PlannedOperation, PortDescriptor, PortDirection, PortId, SourceDocumentId,
    StartupDependency, TerminalPolicy,
};
use conduit_runtime::lowering::{lower_plan_fragment, MAXIMUM_KERNEL_PORTS_PER_NODE};

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
    assert!(first.contains("conduit_kernel::scheduler::CordSpec::local"));
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
            input_cords: [None; MAXIMUM_KERNEL_PORTS_PER_NODE],
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
        routes: Vec::new(),
        route_targets: Vec::new(),
        host_operations: Vec::new(),
        resources: Vec::new(),
        evidence: Vec::new(),
        startup_dependencies: Vec::new(),
        startup_order: vec![0],
        expected_terminals: Vec::new(),
        cord_value_slots: 0,
        cord_value_bytes: 0,
        evidence_items: 1,
        evidence_bytes: 16,
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
    };
    let input = PortDescriptor {
        port_id: PortId::from("in"),
        value_kind: value_kind.clone(),
        direction: PortDirection::Input,
    };
    let expected_evidence = vec![
        ExpectedEvidence::PlanFragmentReceived,
        ExpectedEvidence::PlacementPrepared(source.clone()),
        ExpectedEvidence::PlacementPrepared(sink.clone()),
        ExpectedEvidence::PlacementTerminal(source.clone()),
        ExpectedEvidence::PlacementTerminal(sink.clone()),
        ExpectedEvidence::ConnectionTerminal(connection.clone()),
        ExpectedEvidence::PlanTerminal,
    ];
    let evidence_storage_budget = mandatory_evidence_storage_requirement(&expected_evidence)
        .unwrap_or(EvidenceStorageBudget {
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
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        offer_generation: OfferGeneration(1),
        placements: vec![
            PlannedOperation {
                placement_id: source.clone(),
                operation_id: OperationId::from("source"),
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
                inputs: Vec::new(),
                outputs: vec![output],
                host_operations: Vec::new(),
                resources: Vec::new(),
                authority: Vec::new(),
            },
            PlannedOperation {
                placement_id: sink.clone(),
                operation_id: OperationId::from("sink"),
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
                inputs: vec![input],
                outputs: Vec::new(),
                host_operations: Vec::new(),
                resources: Vec::new(),
                authority: Vec::new(),
            },
        ],
        connections: vec![PlannedConnection {
            connection_id: connection.clone(),
            source_placement_id: source.clone(),
            source_port_id: PortId::from("out"),
            sink_placement_id: sink.clone(),
            sink_port_id: PortId::from("in"),
            value_kind,
            provider: ConnectionProvider::Local,
            link_binding: None,
            item_capacity: 1,
            byte_capacity: 9,
        }],
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
        expected_evidence,
        evidence_storage_budget,
        plan_fragments: Vec::new(),
    };

    seal_plan(form_identity, vec![fragment])
        .fragments
        .into_iter()
        .next()
        .expect("sealed plan retains its fragment")
}
