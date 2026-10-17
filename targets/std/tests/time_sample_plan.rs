use conduit_core::{
    kind_id, port_id, Back, BackOfferBuilder, BaseImplementationId, CapabilityLimits,
    CheckedValueContract, FrontValueContract, FrontValueLocation, HostAdvertisement, HostId,
    HostProfileId, Kind, KindSemanticLaw, OfferGeneration, PortDescriptor, PortDirection,
    PortTemporal, PROTOCOL_VERSION,
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
            execution_profile_id: "std/time-sample-plan-proof@1".into(),
            implementation_id: identity.into(),
            artifact_id: "conduit-std-host/time-sample-plan-proof@1".into(),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[test]
fn plan_retains_the_exact_non_authored_sample_specialization() {
    let value = CheckedValueContract::new(kind_id("value/text"), 73, Vec::new()).unwrap();
    let source = endpoint(
        "test/text-source",
        "test/text-source@1",
        PortDirection::Output,
        &value,
    );
    let tick = CheckedValueContract::new(
        kind_id(conduit_time::TICK_VALUE_KIND),
        conduit_time::TICK_ENCODED_LEN,
        Vec::new(),
    )
    .unwrap();
    let cadence = endpoint(
        "test/cadence-source",
        "test/cadence-source@1",
        PortDirection::Output,
        &tick,
    );
    let sink = endpoint(
        "test/text-sink",
        "test/text-sink@1",
        PortDirection::Input,
        &value,
    );
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    for kind in [&source, &cadence, &sink] {
        startup
            .insert(conduit_plot::KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: Vec::new(),
            })
            .unwrap();
        profile.insert_kind((*kind).clone()).unwrap();
    }
    conduit_semantic_catalog::install_time_sample_kind(&value, &mut startup, &mut profile).unwrap();
    let syntax = conduit_plot::parse_syntax_document(
        "plot sampled-text {\n source: test/text-source\n cadence: test/cadence-source\n sampler: time/sample\n sink: test/text-sink\n source.out >> sampler.value\n cadence.out >> sampler.cadence\n sampler.sample >> sink.in\n}.\n",
    );
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let expanded = conduit_plot::expand_canonical_plot(&checked, "sampled-text", &profile).unwrap();

    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/sample-proof"),
        boot_id: "boot/sample-proof".into(),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/sample-proof@1"),
        bases: Vec::new(),
        resources: Vec::new(),
        capabilities: vec![
            offer(source, "test/text-source-back@1"),
            offer(cadence, "test/cadence-source-back@1"),
            conduit_std_offers::time_sample_offer(&value).unwrap(),
            offer(sink, "test/text-sink-back@1"),
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
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::TIME_SAMPLE_KIND)
        .unwrap();
    assert_eq!(placement.semantic_contract.value_contracts().len(), 2);
    assert!(placement
        .semantic_contract
        .value_contracts()
        .iter()
        .all(|entry| entry.contract == value));
}

#[test]
fn installed_sampler_preserves_physical_capsules_in_actual_plan_play() {
    use conduit_core::{ConnectionTrack, Quantity, Unit};
    use conduit_std_host::{
        ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost,
        StdHostComposition, StdHostConfig, ThreadTimer,
    };
    use std::collections::BTreeMap;
    #[derive(Default)]
    struct Collector(Vec<Vec<u8>>);
    impl ExternalForeOutputAdapter for Collector {
        fn deliver(&mut self, delivery: ExternalForeDelivery) -> Result<(), String> {
            if delivery.track == ConnectionTrack::Payload {
                self.0.push(delivery.bytes);
            }
            Ok(())
        }
    }
    for (name, info, bytes) in [
        (
            "Quantity",
            conduit_core::QUANTITY_INFO_ID,
            Quantity::new(1, Unit::Kilohertz).encode().to_vec(),
        ),
        (
            "Unit",
            conduit_core::UNIT_INFO_ID,
            Unit::Kilohertz.encode().to_vec(),
        ),
    ] {
        let value =
            CheckedValueContract::new(kind_id(info), bytes.len() as u32, Vec::new()).unwrap();
        let mut startup = conduit_plot::StartupCatalog::new();
        let mut profile = conduit_plot::ProfileCatalog::new();
        conduit_time::install_tick_catalog(&mut startup, &mut profile).unwrap();
        conduit_semantic_catalog::install_time_sample_kind(&value, &mut startup, &mut profile)
            .unwrap();
        let source = format!("plot physical-sample (\n >> value: {name}...| <= {}B\n sample: {name}...| <= {}B >>\n) {{\n cadence: time/tick(count = 1, period-ms = 1)\n sampler: time/sample\n value >> sampler.value\n cadence.tick >> sampler.cadence\n sampler.sample >> sample\n}}.\n", bytes.len(), bytes.len());
        let checked = conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(&source),
            &startup,
        )
        .unwrap();
        let authored = conduit_plot::expand_canonical_plot_for_authoring(
            &checked,
            "physical-sample",
            &profile,
        )
        .unwrap();
        let mut host = StdHost::new_with_composition(
            StdHostConfig {
                host_id: "physical-sample".into(),
                boot_id: "sample-boot".into(),
                offer_generation: OfferGeneration(1),
            },
            StdHostComposition::minimal().with_time(),
        );
        let mut advertisement = host.advertisement().clone();
        advertisement
            .capabilities
            .push(conduit_std_offers::time_sample_offer(&value).unwrap());
        host = StdHost::from_advertisement(advertisement).unwrap();
        let hosts = [host.advertisement().clone()];
        let placements =
            conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
        let boundaries = [
            (PortDirection::Input, "value"),
            (PortDirection::Output, "sample"),
        ]
        .into_iter()
        .map(|(direction, name)| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: name.into(),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes.len() as u32,
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
                connection_byte_capacity: conduit_time::TICK_ENCODED_LEN,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .unwrap();
        let mut collector = Collector::default();
        host.run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes: bytes.clone(),
            }],
            &mut collector,
            &mut Vec::new(),
            &mut ThreadTimer,
        )
        .unwrap();
        assert_eq!(collector.0, vec![bytes]);
    }
}
