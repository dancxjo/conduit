use super::*;
use conduit_core::*;

#[test]
fn an_atomic_input_fanout_uses_the_smallest_selected_queue_envelope() {
    let boolean = ProtocolValue {
        schema: StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap(),
        contract: CheckedValueContract::new(kind_id(BOOL_INFO_ID), 1, vec![]).unwrap(),
    };
    let paired_maximum =
        PreparedTypedTuplePairEncoder::new(boolean.schema.clone(), 1, boolean.schema.clone(), 1)
            .unwrap()
            .maximum_bytes();
    let package = ProtocolSourcePackage {
        schema: PACKAGE_SCHEMA.into(),
        source: alloc::format!(
            "with flow/zip/finite/paired as Pair\nplot fanout (\n >> input: Boolean...| <= 1B\n >> other: Boolean...| <= 1B\n flag: Boolean...| <= 1B >>\n paired: Pair...| <= {paired_maximum}B >>\n) {{\n zip: flow/zip/finite\n merge: flow/merge/finite\n input >> zip.left\n other >> zip.right\n input >> merge.left\n other >> merge.right\n merge.merged >> flag\n zip.paired >> paired\n}}\n"
        ),
        specializations: vec![
            ProtocolSpecialization::Zip {
                left: boolean.clone(),
                right: boolean.clone(),
            },
            ProtocolSpecialization::Merge { value: boolean },
        ],
    };
    let source = PreparedProtocolSource::prepare(package.clone()).unwrap();
    let expanded = source.expand("fanout").unwrap();
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/fanout".into(),
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
    let envelopes: Vec<_> = expanded
        .input_bindings
        .iter()
        .filter(|binding| binding.front_port_id.as_str() == "input")
        .map(|binding| {
            let selected = &placements.by_gear[&binding.gear_id];
            hosts[0]
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == selected.capability_id)
                .unwrap()
                .limits
                .max_queue_bytes
        })
        .collect();
    assert_eq!(envelopes.len(), 2);
    assert_ne!(envelopes[0], envelopes[1]);
    let key = ForeBoundaryKey {
        direction: PortDirection::Input,
        front_port_id: port_id("input"),
        track: ConnectionTrack::Payload,
    };
    assert_eq!(
        limits.boundaries[&key].byte_capacity,
        *envelopes.iter().min().unwrap()
    );
    let options = conduit_planner::PlanningOptions {
        connection_bases: &BTreeMap::new(),
        line_candidates: &BTreeMap::new(),
        connection_item_capacity: 1,
        connection_byte_capacity: 1,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let bases = [BaseImplementationId::from("conduit.base/local@1")];
    assert!(matches!(
        PreparedProtocolSource::prepare(package.clone())
            .unwrap()
            .plan_artifact(
                &expanded,
                ArtifactId::from(""),
                &hosts,
                &placements,
                &bases,
                options,
            ),
        Err(ProtocolSourceRefusal::Admission(
            crate::protocol_host_calls::ProtocolCallRefusal::InvalidPlan
        ))
    ));
    let prepared = source
        .plan_artifact(
            &expanded,
            ArtifactId::from("fixture/fanout-source@1"),
            &hosts,
            &placements,
            &bases,
            options,
        )
        .unwrap();
    let artifact = prepared.artifact();
    assert!(verify_plan(&artifact.definition().internal_plan));
    assert_eq!(
        artifact.identity().source,
        expanded.expanded.source_document_id
    );
    assert_eq!(
        artifact.identity().checked,
        expanded.expanded.checked_plot_id
    );
    assert_eq!(
        artifact.identity().expanded,
        expanded.expanded.expanded_plot_id
    );
    let retained = PreparedProtocolSource::prepare(package).unwrap().operations;
    let mut registry = conduit_composite::KernelOperationRegistry::new();
    registry.install(retained.joins).unwrap();
    registry.install(retained.merges).unwrap();
    let mut kernel =
        conduit_composite::KernelCompositeHost::prepare(artifact.definition().clone(), &registry)
            .unwrap();
    kernel.start().unwrap();
    for (port, byte) in [("input", 1), ("other", 0)] {
        let value = ValuePayload {
            value_kind: kind_id(BOOL_INFO_ID),
            encoded: vec![byte],
        };
        assert!(matches!(
            kernel.admit_input(&port_id(port), 0, &value).unwrap(),
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { sequence: 0 }
        ));
    }
    let maximum = artifact
        .definition()
        .external_capability
        .limits
        .max_queue_bytes as usize;
    let mut outputs: Vec<_> = artifact
        .definition()
        .external_capability
        .outputs
        .iter()
        .map(|port| {
            (
                port.port_id.clone(),
                ValuePayload {
                    value_kind: port.value_kind.clone(),
                    encoded: Vec::with_capacity(maximum),
                },
            )
        })
        .collect();
    let mut flags = Vec::new();
    let mut pairs = 0;
    let mut complete = false;
    let mut inputs_closed = false;
    for step in 0..64 {
        let status = kernel.step().unwrap_or_else(|error| {
            panic!("step {step}, flags {flags:?}, pairs {pairs}: {error:?}")
        });
        for (port, output) in &mut outputs {
            if let Some(sequence) = kernel.output_into(port, output).unwrap() {
                if port.as_str() == "flag" {
                    flags.push(output.encoded.clone());
                } else {
                    pairs += 1;
                }
                kernel.complete_output(port, sequence).unwrap();
            }
        }
        if !inputs_closed && flags.len() == 2 && pairs == 1 {
            for port in ["input", "other"] {
                kernel.close_input(&port_id(port)).unwrap();
            }
            inputs_closed = true;
        }
        if status == conduit_composite::KernelCompositeStatus::Complete {
            complete = true;
            break;
        }
    }
    assert_eq!(flags, vec![vec![1], vec![0]]);
    assert_eq!(pairs, 1);
    assert!(complete, "retained signs: {:?}", kernel.signs());
}
