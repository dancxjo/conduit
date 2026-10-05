use super::*;
use conduit_core::*;

#[test]
fn source_admission_refuses_substituted_fore_bindings() {
    let source = PreparedProtocolSource::prepare(ProtocolSourcePackage {
        schema: PACKAGE_SCHEMA.into(),
        source: "plot identity (\n >> input: Boolean...|\n output: Boolean...| >>\n) = (.)".into(),
        specializations: vec![],
    })
    .unwrap();
    let expanded = source.expand("identity").unwrap();
    source.validate_expanded(&expanded).unwrap();
    for direction in [PortDirection::Input, PortDirection::Output] {
        let mut substituted = expanded.clone();
        let bindings = match direction {
            PortDirection::Input => &mut substituted.input_bindings,
            PortDirection::Output => &mut substituted.output_bindings,
        };
        bindings[0].gear_port_id = port_id("substituted-port");
        // The canonical expansion remains valid; the external binding does not.
        substituted.expanded.validate_expansion().unwrap();
        assert!(matches!(
            source.validate_expanded(&substituted),
            Err(ProtocolSourceRefusal::Offer)
        ));
    }
}

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
    let entry =
        PreparedProtocolEntry::prepare(&serde_json::to_vec(&package).unwrap(), "fanout").unwrap();
    let exact_artifact = entry.artifact_id().clone();
    let prepared = entry.plan(&hosts, &placements, &bases, options).unwrap();
    let artifact = prepared.artifact();
    assert_eq!(artifact.identity().artifact, exact_artifact);
    let partition = prepared.body_partition();
    assert_eq!(partition.plan, artifact.definition().internal_plan);
    let resident = partition.plot.clone();
    let workset = conduit_body::BodyWorkset::one(resident.clone()).unwrap();
    let (_, wake) = conduit_body::Body::born_with_plots(workset, 1, "fixture/birth".into())
        .unwrap()
        .wake(2, "fixture/wake".into())
        .unwrap();
    let body_plan = conduit_body::BodyPlan::seal(&wake, vec![partition.clone()]).unwrap();
    body_plan.validate_for(&wake).unwrap();
    assert_eq!(body_plan.plots[0], partition);

    // A protocol partition cannot silently omit another resident Plot.
    let complete_workset = conduit_body::BodyWorkset::from_plots([
        resident,
        conduit_body::ResidentPlot::new(
            "fixture/another-source".into(),
            "fixture/another-checked".into(),
        ),
    ])
    .unwrap();
    let (_, complete_wake) =
        conduit_body::Body::born_with_plots(complete_workset, 1, "fixture/complete-birth".into())
            .unwrap()
            .wake(2, "fixture/complete-wake".into())
            .unwrap();
    assert_eq!(
        conduit_body::BodyPlan::seal(&complete_wake, vec![partition]),
        Err(conduit_body::BodyPlanError::MissingPlot)
    );
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

#[test]
fn source_admission_requires_the_actual_retained_concat_owner() {
    let boolean = ProtocolValue {
        schema: StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap(),
        contract: CheckedValueContract::new(kind_id(BOOL_INFO_ID), 1, vec![]).unwrap(),
    };
    let package = ProtocolSourcePackage {
        schema: PACKAGE_SCHEMA.into(),
        source: "plot ordered (\n >> first: Boolean...| <= 1B\n >> last: Boolean...| <= 1B\n output: Boolean...| <= 1B >>\n) {\n join: flow/concat/finite\n first >> join.left\n last >> join.right\n join.concatenated >> output\n}\n".into(),
        specializations: vec![ProtocolSpecialization::Concat { value: boolean }],
    };
    let source = PreparedProtocolSource::prepare(package.clone()).unwrap();
    let expanded = source.expand("ordered").unwrap();
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/concat".into(),
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
    for retain_owner in [true, false] {
        let mut source = PreparedProtocolSource::prepare(package.clone()).unwrap();
        if !retain_owner {
            source.operations.concats =
                crate::flow_concat_finite::FlowConcatFiniteOperationFactory::default();
        }
        let result = source.plan_artifact(
            &expanded,
            ArtifactId::from("fixture/concat-artifact"),
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 1,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        );
        if retain_owner {
            assert!(result.is_ok());
        } else {
            assert!(matches!(result, Err(ProtocolSourceRefusal::Offer)));
        }
    }
}
