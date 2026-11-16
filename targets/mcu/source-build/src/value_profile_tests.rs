//! Physical graph conformance, not Plan or provider admission.
use super::*;
use conduit_embedded_build::*;
use conduit_kernel::scheduler::{AssignedConnectionTrack, AssignedPressurePolicy};
fn graph(schema: &[u8]) -> GeneratedEmbeddedPlan {
    let ty = StructuredInfoType::from_canonical_bytes(schema).unwrap();
    let kind = ty.profile().unwrap().value_kind().as_str().to_owned();
    let nodes = (0..3)
        .map(|node| GeneratedStaticNode {
            node,
            placement_id: format!("placement/{node}"),
            kind_id: "kind".into(),
            implementation_id: "implementation".into(),
            artifact_id: format!("artifact/{node}"),
            input_cords: [None;
                conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE],
            terminal_transductions: [None;
                conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE],
            maximum_step_fuel: 1,
        })
        .collect();
    let cords = (1..3)
        .map(|node| GeneratedStaticCord {
            cord: node - 1,
            connection_id: format!("cord/{node}"),
            source: Endpoint::Local { node: 0, port: 0 },
            sink: Endpoint::Local { node, port: 0 },
            slot_start: node - 1,
            item_capacity: 1,
            byte_capacity: schema.len() as u32 + 8,
            maximum_value_bytes: schema.len() as u32 + 8,
            pressure_policy: AssignedPressurePolicy::PreserveOrder,
            track: AssignedConnectionTrack::Payload,
        })
        .collect();
    GeneratedEmbeddedPlan {
        schema_version: 1,
        plan_id: "plan".into(),
        fragment_id: "fragment".into(),
        host_id: "host".into(),
        boot_id: "boot".into(),
        offer_generation: 1,
        cancellation_policy: conduit_core::CancellationPolicy::DrainBeforeCancel,
        terminal_policy: conduit_core::TerminalPolicy::RequireAllPlacementsAndConnections,
        nodes,
        cords,
        input_ports: (1..3)
            .map(|node| GeneratedPort {
                node,
                port: 0,
                port_id: "in".into(),
                value_kind: kind.clone(),
            })
            .collect(),
        output_ports: vec![GeneratedPort {
            node: 0,
            port: 0,
            port_id: "out".into(),
            value_kind: kind,
        }],
        configuration: vec![],
        remote_endpoints: vec![],
        routes: vec![],
        route_targets: vec![],
        host_calls: vec![],
        native_bindings: vec![],
        resources: vec![],
        resource_cords: vec![],
        signs: vec![],
        startup_dependencies: vec![],
        startup_order: vec![],
        expected_terminals: vec![],
        cord_value_slots: 2,
        cord_value_bytes: (schema.len() as u32 + 8) * 2,
        sign_items: 1,
        sign_bytes: 1,
    }
}
fn schema() -> Vec<u8> {
    conduit_core::StructuredInfoType::record(
        conduit_core::KindId::from("test/reading"),
        vec![conduit_core::StructuredFieldType::new(
            "value",
            conduit_core::StructuredInfoType::leaf(conduit_core::KindId::from("value/u64"))
                .unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn declarations(schema: &[u8]) -> [PortSchema<'_>; 3] {
    [
        PortSchema {
            node: 0,
            port: 0,
            direction: PortDirection::Output,
            artifact: "artifact/0",
            schema,
        },
        PortSchema {
            node: 1,
            port: 0,
            direction: PortDirection::Input,
            artifact: "artifact/1",
            schema,
        },
        PortSchema {
            node: 2,
            port: 0,
            direction: PortDirection::Input,
            artifact: "artifact/2",
            schema,
        },
    ]
}
#[test]
fn fanout_requires_every_exact_receiver_and_preserves_counts_and_extents() {
    let schema = schema();
    let mut graph = graph(&schema);
    let declarations = declarations(&schema);
    graph.cords[0].item_capacity = 3;
    let selected = select(&graph, &declarations).unwrap();
    let typed = Slot::Schema {
        canonical: &schema,
        maximum_bytes: schema.len() as u32 + 8,
    };
    assert_eq!(selected.cords, vec![typed; 4]);
    assert_eq!(selected.steps, vec![typed]);
    let mixed = select(&graph, &declarations[..2]).unwrap();
    assert_eq!(
        mixed.cords,
        vec![Slot::Canonical(schema.len() as u32 + 8); 4]
    );
    assert_eq!(mixed.steps, vec![Slot::Canonical(schema.len() as u32 + 8)]);
    graph.cords[1].sink = Endpoint::Remote { endpoint: 0 };
    let external = select(&graph, &declarations).unwrap();
    assert_eq!(external.cords, mixed.cords);
    assert_eq!(external.steps, mixed.steps);
}
#[test]
fn altered_artifacts_schemas_ports_duplicates_and_extents_refuse() {
    let schema = schema();
    let mut graph = graph(&schema);
    let mut declarations = declarations(&schema);
    declarations[0].artifact = "other";
    assert!(matches!(
        select(&graph, &declarations),
        Err("schema-artifact")
    ));
    declarations[0].artifact = "artifact/0";
    declarations[0].port = 99;
    assert!(matches!(select(&graph, &declarations), Err("schema-port")));
    declarations[0].port = 0;
    declarations[0].schema = &[];
    assert!(matches!(select(&graph, &declarations), Err("schema-bytes")));
    declarations[0].schema = &schema;
    declarations[2].node = 1;
    declarations[2].artifact = "artifact/1";
    assert!(matches!(
        select(&graph, &declarations),
        Err("schema-duplicate")
    ));
    declarations[2].node = 2;
    declarations[2].artifact = "artifact/2";
    graph.output_ports[0].value_kind = "foreign".into();
    assert!(matches!(select(&graph, &declarations), Err("schema-kind")));
    graph.output_ports[0].value_kind = StructuredInfoType::from_canonical_bytes(&schema)
        .unwrap()
        .profile()
        .unwrap()
        .value_kind()
        .as_str()
        .into();
    graph.cords[0].maximum_value_bytes = 1;
    assert!(matches!(
        select(&graph, &declarations),
        Err("schema-extent")
    ));
}

#[test]
fn topology_associations_preserve_fanout_multioutput_queue_order_and_external_sinks() {
    let schema = schema();
    let mut graph = graph(&schema);
    graph.cords[0].item_capacity = 3;
    graph.cords[1].slot_start = 3;
    let mut second = graph.cords[1].clone();
    second.cord = 2;
    second.source = Endpoint::Local { node: 0, port: 1 };
    second.sink = Endpoint::Remote { endpoint: 7 };
    second.slot_start = 4;
    second.item_capacity = 2;
    second.maximum_value_bytes += 9;
    graph.cords.push(second);
    let mut output = graph.output_ports[0].clone();
    output.port = 1;
    graph.output_ports.push(output);
    // Unrouted outputs must not invent either a Step slot or an association.
    let mut unused = graph.output_ports[0].clone();
    unused.port = 2;
    graph.output_ports.push(unused);
    let selected = select(&graph, &declarations(&schema)).unwrap();
    assert_eq!(
        selected.cord_sinks,
        vec![
            Endpoint::Local { node: 1, port: 0 },
            Endpoint::Local { node: 1, port: 0 },
            Endpoint::Local { node: 1, port: 0 },
            Endpoint::Local { node: 2, port: 0 },
            Endpoint::Remote { endpoint: 7 },
            Endpoint::Remote { endpoint: 7 },
        ]
    );
    assert_eq!(selected.step_outputs, vec![(0, 0), (0, 1)]);
    assert_eq!(selected.cord_sinks.len(), selected.cords.len());
    assert_eq!(selected.step_outputs.len(), selected.steps.len());
    assert_eq!(selected.steps[1], Slot::Canonical(schema.len() as u32 + 17));
    let emitted = selected.emit_rust();
    assert!(emitted.contains("pub const CORD_VALUE_SINKS:&[Option<(u16,u16)>]=&[\nSome((1,0)),\nSome((1,0)),\nSome((1,0)),\nSome((2,0)),\nNone,\nNone,\n];"));
    assert!(emitted.contains("pub const STEP_VALUE_OUTPUTS:&[(u16,u16)]=&[(0, 0), (0, 1)];"));
}

#[test]
fn abnormal_topology_stock_fact_counts_tracks_without_changing_value_profiles() {
    let schema = schema();
    let mut graph = graph(&schema);
    let ordinary = select(&graph, &declarations(&schema)).unwrap();
    assert_eq!(ordinary.abnormal_cord_count, 0);
    assert!(ordinary
        .emit_rust()
        .contains("pub const ABNORMAL_CORD_COUNT:usize=0;"));
    graph.cords[1].track = AssignedConnectionTrack::AbnormalTerminal;
    let abnormal = select(&graph, &declarations(&schema)).unwrap();
    assert_eq!(abnormal.abnormal_cord_count, 1);
    assert_eq!(abnormal.cords, ordinary.cords);
    assert_eq!(abnormal.steps, ordinary.steps);
    assert!(abnormal
        .emit_rust()
        .contains("pub const ABNORMAL_CORD_COUNT:usize=1;"));
}

#[test]
fn external_ingress_stock_counts_logical_boundaries_without_granting_authority() {
    let bytes = schema();
    let mut plan = graph(&bytes);
    assert_eq!(select(&plan, &[]).unwrap().external_ingress_cord_count, 0);
    plan.cords[0].source = Endpoint::Remote { endpoint: 77 };
    let profiles = select(&plan, &[]).unwrap();
    assert_eq!(profiles.external_ingress_cord_count, 1);
    assert!(profiles
        .emit_rust()
        .contains("pub const EXTERNAL_INGRESS_CORD_COUNT:usize=1;"));
    assert_eq!(profiles.cords.len(), 2);
    plan.cords[1].source = Endpoint::Remote { endpoint: 78 };
    assert_eq!(select(&plan, &[]).unwrap().external_ingress_cord_count, 2);
    // External egress is not ingress stock. Local Fore and Line distinctions
    // belong to actual lowering/custody, so all logical ingress is counted.
    plan.cords[0].source = Endpoint::Local { node: 0, port: 0 };
    plan.cords[0].sink = Endpoint::Remote { endpoint: 79 };
    assert_eq!(select(&plan, &[]).unwrap().external_ingress_cord_count, 1);
}
