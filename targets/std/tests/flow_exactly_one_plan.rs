use conduit_composite::KernelOperationFactory;
use conduit_core::*;
use conduit_std_host::flow_exactly_one::{FlowExactlyOneOperationFactory, PreparedFlowExactlyOne};
use std::sync::Arc;
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

#[test]
fn sealed_plan_selects_exact_singleton_schema_and_offer() {
    let value =
        CheckedValueContract::new(kind_id(SCALAR_INFO_ID), SCALAR_ENCODED_LEN as u32, vec![])
            .unwrap();
    let schema = StructuredInfoType::leaf(kind_id(SCALAR_INFO_ID)).unwrap();
    let foreign_schema = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    assert!(PreparedFlowExactlyOne::new(value.clone(), foreign_schema).is_err());
    let retained = Arc::new(PreparedFlowExactlyOne::new(value.clone(), schema.clone()).unwrap());
    let source = endpoint(
        "test/singleton-source",
        "test/singleton-source@1",
        PortDirection::Output,
        PortTemporal::Flow { closes: true },
        &value,
        false,
    );
    let sink = endpoint(
        "test/singleton-sink",
        "test/singleton-sink@1",
        PortDirection::Input,
        PortTemporal::Value,
        &value,
        false,
    );
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    for kind in [&source, &sink] {
        startup
            .insert(conduit_plot::KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profiles.insert_kind(kind.clone()).unwrap();
    }
    conduit_semantic_catalog::install_flow_exactly_one_kind(
        &value,
        &schema,
        &mut startup,
        &mut profiles,
    )
    .unwrap();
    let checked = conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document("plot singleton-proof {\n source: test/singleton-source\n singleton: flow/exactly-one\n sink: test/singleton-sink\n source.out >> singleton.item\n singleton.value >> sink.in\n}\n"), &startup).unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot(&checked, "singleton-proof", &profiles).unwrap();
    let endpoint_offer = |kind: Kind, identity: &str| {
        BackOfferBuilder::new(
            kind,
            Back {
                capability_id: identity.into(),
                execution_profile_id: identity.into(),
                implementation_id: identity.into(),
                artifact_id: identity.into(),
                host_calls: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
            },
        )
        .build()
    };
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "singleton-host".into(),
        boot_id: "singleton-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "singleton-proof".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![
            endpoint_offer(source, "singleton-source"),
            endpoint_offer(sink, "singleton-sink"),
            retained.offer().clone(),
        ],
    };
    let hosts = [host];
    let choices = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &expanded,
        &hosts,
        &choices,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    let gear = plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .find(|g| {
            g.implementation_id.as_str() == conduit_std_offers::FLOW_EXACTLY_ONE_IMPLEMENTATION
        })
        .unwrap();
    let factory =
        FlowExactlyOneOperationFactory::for_plan(&plan, std::slice::from_ref(&retained)).unwrap();
    assert_eq!(
        factory.budget(gear).unwrap().maximum_value_bytes,
        SCALAR_ENCODED_LEN as u32
    );
    let mut store = conduit_kernel::HostedValueStore::new(8, 128, 1024).unwrap();
    factory.prepare(gear, &mut store).unwrap();
    let mut wrong = gear.clone();
    wrong.configuration.push(ConfigurationEntry {
        key: "unexpected".into(),
        value: ConfigurationValue::U64(3),
    });
    assert!(factory.prepare(&wrong, &mut store).is_err());
    wrong = gear.clone();
    wrong.artifact_id = "foreign".into();
    assert!(factory.budget(&wrong).is_err());
    assert!(FlowExactlyOneOperationFactory::for_plan(&plan, &[]).is_err());
    assert!(
        FlowExactlyOneOperationFactory::for_plan(&plan, &[retained.clone(), retained]).is_err()
    );
    let mut unsealed = plan;
    unsealed.fragments[0].placements[0].artifact_id = "changed".into();
    assert!(FlowExactlyOneOperationFactory::for_plan(&unsealed, &[]).is_err());
}
