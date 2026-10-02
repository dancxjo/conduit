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
    temporal: PortTemporal,
    value: &CheckedValueContract,
    abnormal: bool,
) -> Kind {
    let port = PortDescriptor {
        port_id: port_id(if direction == PortDirection::Input {
            "in"
        } else {
            "out"
        }),
        value_kind: value.value_kind.clone(),
        direction,
        temporal,
        abnormal_kind: abnormal.then(|| kind_id(TERMINAL_INFO_ID)),
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

fn recovery(kind: &str, revision: &str) -> Kind {
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
        kind_contract_revision: revision.into(),
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
            execution_profile_id: "std/current-sample-plan-proof@1".into(),
            implementation_id: identity.into(),
            artifact_id: "conduit-std-host/current-sample-plan-proof@1".into(),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[test]
fn plan_seals_one_exact_4096_byte_triggered_current_sample_specialization() {
    let text = CheckedValueContract::new(kind_id("value/text"), 4_096, vec![]).unwrap();
    let request = CheckedValueContract::new(kind_id("data/save-request@1"), 0, vec![]).unwrap();
    let source = endpoint(
        "test/current-text-source",
        "test/current-text-source@1",
        PortDirection::Output,
        PortTemporal::Current,
        &text,
        false,
    );
    let trigger = endpoint(
        "test/save-trigger",
        "test/save-trigger@1",
        PortDirection::Output,
        PortTemporal::Flow { closes: false },
        &request,
        true,
    );
    let sink = endpoint(
        "test/sampled-text-sink",
        "test/sampled-text-sink@1",
        PortDirection::Input,
        PortTemporal::Flow { closes: false },
        &text,
        true,
    );
    let trigger_recovery = recovery("test/trigger-recovery", "test/trigger-recovery@1");
    let sample_recovery = recovery("test/sample-recovery", "test/sample-recovery@1");
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    for kind in [
        &source,
        &trigger,
        &sink,
        &trigger_recovery,
        &sample_recovery,
    ] {
        startup
            .insert(conduit_plot::KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: Vec::new(),
            })
            .unwrap();
        profile.insert_kind((*kind).clone()).unwrap();
    }
    conduit_semantic_catalog::install_current_sample_kind(
        &text,
        &request,
        &mut startup,
        &mut profile,
    )
    .unwrap();
    let syntax = conduit_plot::parse_syntax_document(
        "plot sample-current-text {\n source: test/current-text-source\n trigger: test/save-trigger\n sample: current/sample\n sink: test/sampled-text-sink\n trigger-recovery: test/trigger-recovery\n sample-recovery: test/sample-recovery\n source.out >> sample.current\n trigger.out >> sample.trigger\n sample.value >> sink.in\n trigger.out! >> trigger-recovery.terminal\n sample.value! >> sample-recovery.terminal\n}\n",
    );
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot(&checked, "sample-current-text", &profile).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/current-sample-proof"),
        boot_id: "boot/current-sample-proof".into(),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/current-sample-proof@1"),
        bases: Vec::new(),
        resources: Vec::new(),
        capabilities: vec![
            offer(source, "test/current-text-source-back@1"),
            offer(trigger, "test/save-trigger-back@1"),
            conduit_std_offers::current_sample_offer(&text, &request).unwrap(),
            offer(sink, "test/sampled-text-sink-back@1"),
            offer(trigger_recovery, "test/trigger-recovery-back@1"),
            offer(sample_recovery, "test/sample-recovery-back@1"),
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
            placement.kind_id.as_str() == conduit_semantic_catalog::CURRENT_SAMPLE_KIND
        })
        .unwrap();
    assert_eq!(placement.inputs[0].temporal, PortTemporal::Current);
    assert_eq!(
        placement.inputs[1].temporal,
        PortTemporal::Flow { closes: false }
    );
    assert_eq!(
        placement.outputs[0].temporal,
        PortTemporal::Flow { closes: false }
    );
    assert_eq!(
        placement
            .semantic_contract
            .value_contracts()
            .iter()
            .find(|contract| { contract.location == FrontValueLocation::Output(port_id("value")) })
            .unwrap()
            .contract,
        text
    );
}
