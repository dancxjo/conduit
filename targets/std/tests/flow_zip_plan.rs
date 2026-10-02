use conduit_core::{
    kind_id, port_id, AbnormalTerminalTransduction, Back, BackOfferBuilder, BaseImplementationId,
    CancellationTransduction, CapabilityLimits, CheckedValueContract, FrontValueContract,
    FrontValueLocation, HostAdvertisement, HostId, HostProfileId, Kind, KindSemanticLaw,
    NormalCloseTransduction, OfferGeneration, PortDescriptor, PortDirection, PortTemporal,
    TerminalTransductionProfile, PROTOCOL_VERSION, TERMINAL_INFO_ID, UNIT_INFO_ID,
};

fn endpoint(
    kind: &str,
    revision: &str,
    direction: PortDirection,
    value: &CheckedValueContract,
) -> Kind {
    let port = PortDescriptor {
        port_id: port_id(if direction == PortDirection::Input {
            "in"
        } else {
            "out"
        }),
        value_kind: value.value_kind.clone(),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: Some(kind_id(TERMINAL_INFO_ID)),
    };
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(kind),
        kind_contract_revision: revision.into(),
        inputs: (direction == PortDirection::Input)
            .then(|| port.clone())
            .into_iter()
            .collect(),
        outputs: (direction == PortDirection::Output)
            .then(|| port.clone())
            .into_iter()
            .collect(),
        configuration: Vec::new(),
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: if direction == PortDirection::Input {
                FrontValueLocation::Input(port.port_id)
            } else {
                FrontValueLocation::Output(port.port_id)
            },
            contract: value.clone(),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: value.maximum_bytes.max(1),
        },
    }
}

fn recovery(kind: &str) -> Kind {
    let terminal = PortDescriptor {
        port_id: port_id("terminal"),
        value_kind: kind_id(TERMINAL_INFO_ID),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
        abnormal_kind: Some(kind_id(TERMINAL_INFO_ID)),
    };
    let recovered = PortDescriptor {
        port_id: port_id("recovered"),
        value_kind: kind_id(UNIT_INFO_ID),
        direction: PortDirection::Output,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(kind),
        kind_contract_revision: format!("{kind}@1").into(),
        inputs: vec![terminal.clone()],
        outputs: vec![recovered.clone()],
        configuration: Vec::new(),
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Input(terminal.port_id.clone()),
                    contract: CheckedValueContract::new(
                        kind_id(TERMINAL_INFO_ID),
                        conduit_core::TERMINAL_INFO_ENCODED_LEN as u32,
                        vec![],
                    )
                    .unwrap(),
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(recovered.port_id.clone()),
                    contract: CheckedValueContract::new(kind_id(UNIT_INFO_ID), 0, vec![]).unwrap(),
                },
            ]),
            KindSemanticLaw::TerminalTransduction(TerminalTransductionProfile {
                input_port_id: terminal.port_id,
                output_port_id: recovered.port_id,
                normal_close: NormalCloseTransduction::NotAccepted,
                abnormal: AbnormalTerminalTransduction::Recover,
                cancellation: CancellationTransduction::NotCancellable,
            }),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_core::TERMINAL_INFO_ENCODED_LEN as u32,
        },
    }
}

fn offer(kind: Kind, identity: &str) -> conduit_core::CapabilityOffer {
    BackOfferBuilder::new(
        kind,
        Back {
            capability_id: identity.into(),
            execution_profile_id: "std/flow-zip-plan-proof@1".into(),
            implementation_id: identity.into(),
            artifact_id: "conduit-std-host/flow-zip-plan-proof@1".into(),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[test]
fn plan_seals_both_exact_inputs_pair_output_and_terminal_profiles() {
    let text = CheckedValueContract::new(kind_id("value/text"), 64, vec![]).unwrap();
    let count = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
    let zip = conduit_semantic_catalog::flow_zip_semantic_contract(&text, &count).unwrap();
    let paired = zip
        .value_contracts()
        .iter()
        .find(|contract| contract.location == FrontValueLocation::Output(port_id("paired")))
        .unwrap()
        .contract
        .clone();
    let left = endpoint(
        "test/zip-left-source",
        "test/zip-left-source@1",
        PortDirection::Output,
        &text,
    );
    let right = endpoint(
        "test/zip-right-source",
        "test/zip-right-source@1",
        PortDirection::Output,
        &count,
    );
    let sink = endpoint(
        "test/zip-pair-sink",
        "test/zip-pair-sink@1",
        PortDirection::Input,
        &paired,
    );
    let left_recovery = recovery("test/zip-left-recovery");
    let right_recovery = recovery("test/zip-right-recovery");
    let pair_recovery = recovery("test/zip-pair-recovery");
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    for kind in [
        &left,
        &right,
        &sink,
        &left_recovery,
        &right_recovery,
        &pair_recovery,
    ] {
        startup
            .insert(conduit_plot::KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: Vec::new(),
            })
            .unwrap();
        profile.insert_kind((*kind).clone()).unwrap();
    }
    conduit_semantic_catalog::install_flow_zip_kind(&text, &count, &mut startup, &mut profile)
        .unwrap();
    let syntax = conduit_plot::parse_syntax_document(
        "plot zip-two-flows {\n left: test/zip-left-source\n right: test/zip-right-source\n zip: flow/zip\n sink: test/zip-pair-sink\n left-recovery: test/zip-left-recovery\n right-recovery: test/zip-right-recovery\n pair-recovery: test/zip-pair-recovery\n left.out >> zip.left\n right.out >> zip.right\n zip.paired >> sink.in\n left.out! >> left-recovery.terminal\n right.out! >> right-recovery.terminal\n zip.paired! >> pair-recovery.terminal\n}\n",
    );
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot(&checked, "zip-two-flows", &profile).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/flow-zip-proof"),
        boot_id: "boot/flow-zip-proof".into(),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/flow-zip-proof@1"),
        bases: Vec::new(),
        resources: Vec::new(),
        capabilities: vec![
            offer(left, "test/zip-left-source-back@1"),
            offer(right, "test/zip-right-source-back@1"),
            conduit_std_offers::flow_zip_offer(&text, &count).unwrap(),
            offer(sink, "test/zip-pair-sink-back@1"),
            offer(left_recovery, "test/zip-left-recovery-back@1"),
            offer(right_recovery, "test/zip-right-recovery-back@1"),
            offer(pair_recovery, "test/zip-pair-recovery-back@1"),
        ],
        planner_capabilities: Vec::new(),
    };
    let placements =
        conduit_planner::default_expanded_placements(&expanded, core::slice::from_ref(&host))
            .unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &expanded,
        core::slice::from_ref(&host),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    let placement = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::FLOW_ZIP_KIND)
        .unwrap();
    assert_eq!(placement.inputs[0].value_kind, text.value_kind);
    assert_eq!(placement.inputs[1].value_kind, count.value_kind);
    assert_eq!(placement.outputs[0].value_kind, paired.value_kind);
    assert_eq!(placement.terminal_transductions.len(), 2);
    assert_eq!(
        placement.terminal_transductions[0].input_port_id.as_str(),
        "left"
    );
    assert_eq!(
        placement.terminal_transductions[1].input_port_id.as_str(),
        "right"
    );
}

#[test]
fn plan_seals_combine_latest_state_and_all_close_terminal_profiles() {
    let text = CheckedValueContract::new(kind_id("value/text"), 64, vec![]).unwrap();
    let count = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
    let combine =
        conduit_semantic_catalog::combine_latest_semantic_contract(&text, &count).unwrap();
    let latest = combine
        .value_contracts()
        .iter()
        .find(|contract| contract.location == FrontValueLocation::Output(port_id("latest")))
        .unwrap()
        .contract
        .clone();
    let left = endpoint(
        "test/latest-left-source",
        "test/latest-left-source@1",
        PortDirection::Output,
        &text,
    );
    let right = endpoint(
        "test/latest-right-source",
        "test/latest-right-source@1",
        PortDirection::Output,
        &count,
    );
    let sink = endpoint(
        "test/latest-pair-sink",
        "test/latest-pair-sink@1",
        PortDirection::Input,
        &latest,
    );
    let left_recovery = recovery("test/latest-left-recovery");
    let right_recovery = recovery("test/latest-right-recovery");
    let pair_recovery = recovery("test/latest-pair-recovery");
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    for kind in [
        &left,
        &right,
        &sink,
        &left_recovery,
        &right_recovery,
        &pair_recovery,
    ] {
        startup
            .insert(conduit_plot::KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: Vec::new(),
            })
            .unwrap();
        profile.insert_kind((*kind).clone()).unwrap();
    }
    conduit_semantic_catalog::install_combine_latest_kind(
        &text,
        &count,
        &mut startup,
        &mut profile,
    )
    .unwrap();
    let syntax = conduit_plot::parse_syntax_document(
        "plot combine-two-flows {\n left: test/latest-left-source\n right: test/latest-right-source\n combine: state/combine-latest\n sink: test/latest-pair-sink\n left-recovery: test/latest-left-recovery\n right-recovery: test/latest-right-recovery\n pair-recovery: test/latest-pair-recovery\n left.out >> combine.left\n right.out >> combine.right\n combine.latest >> sink.in\n left.out! >> left-recovery.terminal\n right.out! >> right-recovery.terminal\n combine.latest! >> pair-recovery.terminal\n}\n",
    );
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot(&checked, "combine-two-flows", &profile).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/combine-latest-proof"),
        boot_id: "boot/combine-latest-proof".into(),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/combine-latest-proof@1"),
        bases: Vec::new(),
        resources: Vec::new(),
        capabilities: vec![
            offer(left, "test/latest-left-source-back@1"),
            offer(right, "test/latest-right-source-back@1"),
            conduit_std_offers::combine_latest_offer(&text, &count).unwrap(),
            offer(sink, "test/latest-pair-sink-back@1"),
            offer(left_recovery, "test/latest-left-recovery-back@1"),
            offer(right_recovery, "test/latest-right-recovery-back@1"),
            offer(pair_recovery, "test/latest-pair-recovery-back@1"),
        ],
        planner_capabilities: Vec::new(),
    };
    let placements =
        conduit_planner::default_expanded_placements(&expanded, core::slice::from_ref(&host))
            .unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &expanded,
        core::slice::from_ref(&host),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    let placement = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::COMBINE_LATEST_KIND
        })
        .unwrap();
    assert_eq!(placement.inputs[0].value_kind, text.value_kind);
    assert_eq!(placement.inputs[1].value_kind, count.value_kind);
    assert_eq!(placement.outputs[0].value_kind, latest.value_kind);
    assert_eq!(placement.terminal_transductions.len(), 2);
    assert!(placement
        .terminal_transductions
        .iter()
        .all(|profile| matches!(
            profile.normal_close,
            NormalCloseTransduction::FlushThenPropagateWhenAllClose(_)
        )));
}

#[test]
fn plan_seals_keyed_join_types_capacity_policy_and_terminal_profiles() {
    let key = CheckedValueContract::new(kind_id("value/text"), 32, vec![]).unwrap();
    let left_value = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
    let right_value = CheckedValueContract::new(kind_id("value/bool"), 1, vec![]).unwrap();
    let join = conduit_semantic_catalog::flow_join_by_key_semantic_contract(
        &key,
        &left_value,
        &right_value,
    )
    .unwrap();
    let contract = |location| {
        join.value_contracts()
            .iter()
            .find(|contract| contract.location == location)
            .unwrap()
            .contract
            .clone()
    };
    let left = contract(FrontValueLocation::Input(port_id("left")));
    let right = contract(FrontValueLocation::Input(port_id("right")));
    let joined = contract(FrontValueLocation::Output(port_id("joined")));
    let left_source = endpoint(
        "test/keyed-left-source",
        "test/keyed-left-source@1",
        PortDirection::Output,
        &left,
    );
    let right_source = endpoint(
        "test/keyed-right-source",
        "test/keyed-right-source@1",
        PortDirection::Output,
        &right,
    );
    let sink = endpoint(
        "test/keyed-joined-sink",
        "test/keyed-joined-sink@1",
        PortDirection::Input,
        &joined,
    );
    let left_recovery = recovery("test/keyed-left-recovery");
    let right_recovery = recovery("test/keyed-right-recovery");
    let joined_recovery = recovery("test/keyed-joined-recovery");
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    for kind in [
        &left_source,
        &right_source,
        &sink,
        &left_recovery,
        &right_recovery,
        &joined_recovery,
    ] {
        startup
            .insert(conduit_plot::KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: Vec::new(),
            })
            .unwrap();
        profile.insert_kind((*kind).clone()).unwrap();
    }
    conduit_semantic_catalog::install_flow_join_by_key_kind(
        &key,
        &left_value,
        &right_value,
        &mut startup,
        &mut profile,
    )
    .unwrap();
    let syntax = conduit_plot::parse_syntax_document(
        "plot join-keyed-flows {\n left: test/keyed-left-source\n right: test/keyed-right-source\n join: flow/join/by-key\n sink: test/keyed-joined-sink\n left-recovery: test/keyed-left-recovery\n right-recovery: test/keyed-right-recovery\n joined-recovery: test/keyed-joined-recovery\n left.out >> join.left\n right.out >> join.right\n join.joined >> sink.in\n left.out! >> left-recovery.terminal\n right.out! >> right-recovery.terminal\n join.joined! >> joined-recovery.terminal\n}\n",
    );
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot(&checked, "join-keyed-flows", &profile).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/keyed-join-proof"),
        boot_id: "boot/keyed-join-proof".into(),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/keyed-join-proof@1"),
        bases: Vec::new(),
        resources: Vec::new(),
        capabilities: vec![
            offer(left_source, "test/keyed-left-source-back@1"),
            offer(right_source, "test/keyed-right-source-back@1"),
            conduit_std_offers::flow_join_by_key_offer(&key, &left_value, &right_value).unwrap(),
            offer(sink, "test/keyed-joined-sink-back@1"),
            offer(left_recovery, "test/keyed-left-recovery-back@1"),
            offer(right_recovery, "test/keyed-right-recovery-back@1"),
            offer(joined_recovery, "test/keyed-joined-recovery-back@1"),
        ],
        planner_capabilities: Vec::new(),
    };
    let placements =
        conduit_planner::default_expanded_placements(&expanded, core::slice::from_ref(&host))
            .unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &expanded,
        core::slice::from_ref(&host),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    let placement = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::FLOW_JOIN_BY_KEY_KIND
        })
        .unwrap();
    let law = placement.semantic_contract.keyed_join().unwrap();
    assert_eq!(law.key, key);
    assert_eq!(law.left_value, left_value);
    assert_eq!(law.right_value, right_value);
    assert_eq!(
        law.maximum_pending_per_side,
        conduit_semantic_catalog::FLOW_JOIN_BY_KEY_PENDING_PER_SIDE
    );
    assert_eq!(placement.terminal_transductions.len(), 2);
    assert!(placement
        .terminal_transductions
        .iter()
        .all(|profile| matches!(
            profile.normal_close,
            NormalCloseTransduction::PropagateWhenAllClose
        )));
}
