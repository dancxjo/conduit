//! An ordinary structured output can exceed ingress without truncation.
use super::*;

#[test]
fn structured_output_above_ingress_limit_is_planned_lowered_and_delivered_exactly() {
    let bytes = ProtocolValue {
        schema: StructuredInfoType::leaf(kind_id("value/bytes")).unwrap(),
        contract: CheckedValueContract::new(kind_id("value/bytes"), 3072, vec![]).unwrap(),
    };
    let mut encoder =
        PreparedTypedTuplePairEncoder::new(bytes.schema.clone(), 3072, bytes.schema.clone(), 3072)
            .unwrap();
    let package = ProtocolSourcePackage {
        schema: PACKAGE_SCHEMA.into(),
        backs: vec![],
        source: alloc::format!(
            "with flow/zip/finite/paired as Pair\nplot paired (\n >> left: Bytes...| <= 3072B\n >> right: Bytes...| <= 3072B\n paired: Pair...| <= {}B >>\n) {{\n join: flow/zip/finite\n left >> join.left\n right >> join.right\n join.paired >> paired\n}}\n",
            encoder.maximum_bytes()
        ),
        specializations: vec![ProtocolSpecialization::Zip {
            left: bytes.clone(),
            right: bytes,
        }],
    };
    let source = PreparedProtocolSource::prepare(package.clone()).unwrap();
    let expanded = source.expand("paired").unwrap();
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/output".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "conduitos/native@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    source.publish_pure_backs(&expanded, &mut host).unwrap();
    let hosts = [host];
    let placements =
        conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let limits = source.queue_limits(&expanded, &hosts, &placements).unwrap();
    for port in ["left", "right"] {
        assert_eq!(
            limits.boundaries[&ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id(port),
                track: ConnectionTrack::Payload
            }]
                .byte_capacity,
            MAXIMUM_PROTOCOL_FORE_BYTES
        );
    }
    let output_limit = limits.boundaries[&ForeBoundaryKey {
        direction: PortDirection::Output,
        front_port_id: port_id("paired"),
        track: ConnectionTrack::Payload,
    }]
        .byte_capacity;
    assert!(output_limit > MAXIMUM_PROTOCOL_FORE_BYTES);
    assert!(output_limit <= MAXIMUM_PROTOCOL_OUTPUT_FORE_BYTES);
    let options = conduit_planner::PlanningOptions {
        connection_bases: &BTreeMap::new(),
        line_candidates: &BTreeMap::new(),
        connection_item_capacity: 1,
        connection_byte_capacity: 1,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let entry =
        PreparedProtocolEntry::prepare(&serde_json::to_vec(&package).unwrap(), "paired").unwrap();
    let prepared = entry
        .plan(
            &hosts,
            &placements,
            &["conduit.base/local@1".into()],
            options,
        )
        .unwrap();
    let definition = prepared.artifact().definition().clone();
    let fragment = &definition.internal_plan.fragments[0];
    let output_boundary = fragment
        .fore_ports
        .iter()
        .find(|boundary| {
            boundary.direction == PortDirection::Output
                && boundary.track == ConnectionTrack::Payload
        })
        .unwrap();
    assert_eq!(output_boundary.byte_capacity, output_limit);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    assert!(
        lowered
            .nodes
            .iter()
            .flat_map(|node| &node.outputs)
            .any(|output| output
                .maximum_value_bytes
                .is_some_and(|maximum| maximum > u64::from(MAXIMUM_PROTOCOL_FORE_BYTES)))
    );
    let operations = PreparedProtocolSource::prepare(package).unwrap().operations;
    let mut registry = conduit_composite::KernelOperationRegistry::new();
    registry.install(operations.joins).unwrap();
    let mut kernel =
        conduit_composite::KernelCompositeHost::prepare(definition, &registry).unwrap();
    let left = vec![0x5a; 3000];
    let right = vec![0xa5; 3000];
    let expected = encoder.encode(&left, &right).unwrap().to_vec();
    assert!(expected.len() > MAXIMUM_PROTOCOL_FORE_BYTES as usize);
    kernel.start().unwrap();
    for (port, encoded) in [("left", left), ("right", right)] {
        assert!(matches!(
            kernel
                .admit_input(
                    &port_id(port),
                    0,
                    &ValuePayload {
                        value_kind: kind_id("value/bytes"),
                        encoded,
                    }
                )
                .unwrap(),
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
        ));
    }
    let mut output = ValuePayload {
        value_kind: encoder.value_type().profile().unwrap().value_kind().clone(),
        encoded: Vec::with_capacity(output_limit as usize),
    };
    let mut observed = false;
    let mut complete = false;
    for _ in 0..32 {
        let status = kernel.step().unwrap();
        if let Some(sequence) = kernel.output_into(&port_id("paired"), &mut output).unwrap() {
            assert!(!observed);
            assert_eq!(output.encoded, expected);
            observed = true;
            kernel
                .complete_output(&port_id("paired"), sequence)
                .unwrap();
            // Finite zip flushes an already formed pair, and discards unmatched
            // ingress when closure arrives first. Close after exact delivery
            // acknowledgement so this regression exercises both output and close.
            for port in ["left", "right"] {
                kernel.close_input(&port_id(port)).unwrap();
            }
        }
        if status == conduit_composite::KernelCompositeStatus::Complete {
            complete = true;
            break;
        }
    }
    assert!(
        observed && complete,
        "observed={observed}, complete={complete}, signs={:?}",
        kernel.signs()
    );
}
