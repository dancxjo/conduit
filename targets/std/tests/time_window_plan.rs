use conduit_core::{
    kind_id, port_id, Back, BackOfferBuilder, BaseImplementationId, CapabilityLimits,
    CheckedValueContract, FrontValueContract, FrontValueLocation, HostAdvertisement, HostId, Kind,
    KindSemanticLaw, OfferGeneration, PortDescriptor, PortDirection, PortTemporal,
    PreparedLeafSequenceEncoder, PROTOCOL_VERSION,
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
        abnormal_kind: None,
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
            max_queue_bytes: value.maximum_bytes,
        },
    }
}

fn offer(kind: Kind, identity: &str) -> conduit_core::CapabilityOffer {
    BackOfferBuilder::new(
        kind,
        Back {
            capability_id: identity.into(),
            execution_profile_id: "std/time-window-plan-proof@1".into(),
            implementation_id: identity.into(),
            artifact_id: "conduit-std-host/time-window-plan-proof@1".into(),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[test]
fn plan_retains_the_exact_non_authored_window_specialization() {
    let value = CheckedValueContract::new(kind_id("value/text"), 73, Vec::new()).unwrap();
    let encoder = PreparedLeafSequenceEncoder::new(
        value.value_kind.clone(),
        value.maximum_bytes,
        conduit_semantic_catalog::TIME_WINDOW_MAXIMUM_ITEMS,
    )
    .unwrap();
    let window = CheckedValueContract::new(
        encoder
            .value_type()
            .unwrap()
            .profile()
            .unwrap()
            .value_kind()
            .clone(),
        encoder.maximum_bytes(),
        Vec::new(),
    )
    .unwrap();
    let source = endpoint(
        "test/window-text-source",
        "test/window-text-source@1",
        PortDirection::Output,
        &value,
    );
    let sink = endpoint(
        "test/window-text-sink",
        "test/window-text-sink@1",
        PortDirection::Input,
        &window,
    );
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    for kind in [&source, &sink] {
        startup
            .insert(conduit_plot::KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: Vec::new(),
            })
            .unwrap();
        profile.insert_kind((*kind).clone()).unwrap();
    }
    conduit_semantic_catalog::install_time_window_kind(
        &value,
        conduit_semantic_catalog::TIME_WINDOW_MAXIMUM_ITEMS,
        &mut startup,
        &mut profile,
    )
    .unwrap();
    let syntax = conduit_plot::parse_syntax_document(
        "plot windowed-text {\n source: test/window-text-source\n window: time/window(duration-ms = 5ms)\n sink: test/window-text-sink\n source.out >> window.value\n window.window >> sink.in\n}.\n",
    );
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot(&checked, "windowed-text", &profile).unwrap();

    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/window-proof"),
        boot_id: "boot/window-proof".into(),
        offer_generation: OfferGeneration(1),
        profile: "std/window-proof@1".into(),
        bases: Vec::new(),
        resources: vec![conduit_core::ResourceOffer {
            pool_id: "host/window-proof/monotonic-timer".into(),
            class_id: conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS.into(),
            capacity_units: 1,
            compute: None,
            content: None,
        }],
        capabilities: vec![
            offer(source, "test/window-text-source-back@1"),
            conduit_std_offers::time_window_offer(
                &value,
                conduit_semantic_catalog::TIME_WINDOW_MAXIMUM_ITEMS,
            )
            .unwrap(),
            offer(sink, "test/window-text-sink-back@1"),
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
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::TIME_WINDOW_KIND)
        .unwrap();
    assert_eq!(placement.semantic_contract.value_contracts().len(), 4);
    assert_eq!(placement.outputs[0].value_kind, window.value_kind);
    assert_eq!(placement.limits.max_queue_items, 10);
    assert_eq!(placement.host_calls.len(), 1);
    assert_eq!(placement.resources.len(), 1);
}
