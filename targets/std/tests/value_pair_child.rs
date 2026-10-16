//! Exact schema custody crosses Source, Plan and the existing child kernel.
use conduit_composite::{
    KernelCompositeBoundary, KernelCompositeDefinition, KernelCompositeFrontBinding,
    KernelCompositeHost, KernelOperationFactory, KernelOperationRegistry,
};
use conduit_core::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CompositeFrontTerminal, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{flow_activation::PreparedValuePairFactory, StdHost};
use std::collections::BTreeMap;

#[test]
fn exact_pair_factory_runs_the_authored_value_graph_and_refuses_changed_identity() {
    let left_type = StructuredInfoType::leaf(kind_id("value/i32")).unwrap();
    let right_type = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let left = CheckedValueContract::new(kind_id("value/i32"), 4, vec![]).unwrap();
    let right = CheckedValueContract::new(kind_id("value/u8"), 1, vec![]).unwrap();
    let factory = PreparedValuePairFactory::new(&left, &left_type, &right, &right_type).unwrap();
    let offer = factory.offer().clone();
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_value_pair_kind(
        &left,
        &left_type,
        &right,
        &right_type,
        &mut startup,
        &mut profiles,
    )
    .unwrap();
    let output_maximum = offer
        .semantic_contract
        .value_contracts()
        .iter()
        .find(|v| matches!(v.location, FrontValueLocation::Output(_)))
        .unwrap()
        .contract
        .maximum_bytes;
    let source = format!("with data/pair/paired as Pair\nplot pair (\n >> left: I32 <= 4B\n >> right: U8 <= 1B\n paired: Pair <= {output_maximum}B >>\n) {{\n join: data/pair\n left >> join.left\n right >> join.right\n join.paired >> paired\n}}\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "pair", &profiles).unwrap();
    let mut host = StdHost::new().advertisement().clone();
    host.host_id = "fixture/value-pair-child".into();
    host.capabilities = vec![offer.clone()];
    let hosts = [host];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "left", 4),
        (PortDirection::Input, "right", 1),
        (PortDirection::Output, "paired", output_maximum),
    ]
    .into_iter()
    .map(|(direction, name, maximum_bytes)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: maximum_bytes,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: output_maximum,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    let fragment = &plan.fragments[0];
    let placement = &fragment.placements[0];
    let budget = factory.budget(placement).unwrap();
    assert_eq!(budget.host_requests, 0);
    for field in 0..4 {
        let mut forged = placement.clone();
        match field {
            0 => forged.artifact_id = "forged/artifact".into(),
            1 => forged.capability_id = "forged/capability".into(),
            2 => forged.inputs[0].temporal = PortTemporal::Flow { closes: true },
            3 => forged.semantic_contract = Default::default(),
            _ => unreachable!(),
        }
        assert!(factory.budget(&forged).is_err());
    }
    let bind = |port: &PortDescriptor| KernelCompositeFrontBinding {
        external_port: port.clone(),
        internal_child: fragment.host_id.clone(),
        internal_placement_id: placement.placement_id.clone(),
        internal_port_id: port.port_id.clone(),
        terminal: CompositeFrontTerminal::Independent,
    };
    let inputs = placement.inputs.clone();
    let output = placement.outputs[0].clone();
    let definition = KernelCompositeDefinition {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "fixture/value-pair-child@1".into(),
        boundary: KernelCompositeBoundary {
            input_fronts: inputs.iter().map(bind).collect(),
            output_fronts: vec![bind(&output)],
        },
        external_capability: offer,
        internal_plan: plan,
        failure_translation: FailureReason::CompositeCapabilityFailed,
    };
    let mut registry = KernelOperationRegistry::new();
    registry.install(factory).unwrap();
    let mut child = KernelCompositeHost::prepare(definition, &registry).unwrap();
    child.start().unwrap();
    for (port, encoded) in inputs
        .iter()
        .zip([(-7_i32).to_le_bytes().to_vec(), vec![42]])
    {
        child
            .admit_input(
                &port.port_id,
                0,
                &ValuePayload {
                    value_kind: port.value_kind.clone(),
                    encoded,
                },
            )
            .unwrap();
        child.close_input(&port.port_id).unwrap();
    }
    let mut result = ValuePayload {
        value_kind: output.value_kind.clone(),
        encoded: Vec::with_capacity(budget.maximum_value_bytes as usize),
    };
    let mut delivered = false;
    for _ in 0..16 {
        child.step().unwrap();
        assert!(child.next_host_request().is_none());
        if child
            .output_into(&output.port_id, &mut result)
            .unwrap()
            .is_some()
        {
            assert!(!delivered);
            let view = validate_canonical_structured_value(&result.encoded).unwrap();
            assert_eq!(
                view.record_field("item-00000")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/i32")
                    .unwrap(),
                (-7_i32).to_le_bytes()
            );
            assert_eq!(
                view.record_field("item-00001")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u8")
                    .unwrap(),
                [42]
            );
            child.complete_output(&output.port_id, 0).unwrap();
            delivered = true;
        }
    }
    assert!(delivered);
}
