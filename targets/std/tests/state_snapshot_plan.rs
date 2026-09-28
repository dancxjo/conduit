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
            execution_profile_id: "std/state-snapshot-plan-proof@1".into(),
            implementation_id: identity.into(),
            artifact_id: "conduit-std-host/state-snapshot-plan-proof@1".into(),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[test]
fn plan_seals_one_exact_4096_byte_current_snapshot_specialization() {
    let text = CheckedValueContract::new(kind_id("value/text"), 4_096, vec![]).unwrap();
    let unit = CheckedValueContract::new(kind_id(UNIT_INFO_ID), 0, vec![]).unwrap();
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
        PortTemporal::Value,
        &unit,
        true,
    );
    let sink = endpoint(
        "test/snapshot-text-sink",
        "test/snapshot-text-sink@1",
        PortDirection::Input,
        PortTemporal::Value,
        &text,
        true,
    );
    let trigger_recovery = recovery("test/trigger-recovery", "test/trigger-recovery@1");
    let snapshot_recovery = recovery("test/snapshot-recovery", "test/snapshot-recovery@1");
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    for kind in [
        &source,
        &trigger,
        &sink,
        &trigger_recovery,
        &snapshot_recovery,
    ] {
        startup
            .insert(conduit_form::KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: Vec::new(),
            })
            .unwrap();
        profile.insert_kind((*kind).clone()).unwrap();
    }
    conduit_semantic_catalog::install_state_snapshot_kind(&text, &mut startup, &mut profile)
        .unwrap();
    let syntax = conduit_form::parse_syntax_document(
        "form snapshot-text {\n source: test/current-text-source\n trigger: test/save-trigger\n snapshot: state/snapshot\n sink: test/snapshot-text-sink\n trigger-recovery: test/trigger-recovery\n snapshot-recovery: test/snapshot-recovery\n source.out >> snapshot.current\n trigger.out >> snapshot.trigger\n snapshot.value >> sink.in\n trigger.out! >> trigger-recovery.terminal\n snapshot.value! >> snapshot-recovery.terminal\n}.\n",
    );
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_form::expand_canonical_form(&checked, "snapshot-text", &profile).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/snapshot-proof"),
        boot_id: "boot/snapshot-proof".into(),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/snapshot-proof@1"),
        bases: Vec::new(),
        resources: Vec::new(),
        capabilities: vec![
            offer(source, "test/current-text-source-back@1"),
            offer(trigger, "test/save-trigger-back@1"),
            conduit_std_offers::state_snapshot_offer(&text).unwrap(),
            offer(sink, "test/snapshot-text-sink-back@1"),
            offer(trigger_recovery, "test/trigger-recovery-back@1"),
            offer(snapshot_recovery, "test/snapshot-recovery-back@1"),
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
            placement.kind_id.as_str() == conduit_semantic_catalog::STATE_SNAPSHOT_KIND
        })
        .unwrap();
    assert_eq!(placement.inputs[0].temporal, PortTemporal::Current);
    assert_eq!(placement.inputs[1].temporal, PortTemporal::Value);
    assert_eq!(placement.outputs[0].temporal, PortTemporal::Value);
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
