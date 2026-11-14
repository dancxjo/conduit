//! The real successful Source/Form result must fit its ordinary planned Cord.
use conduit_core as c;
use conduit_dht11_observation::{ObservationContract, PreparedSourceFrameDecoder};
use conduit_environment_runtime::publication::{PublicationChannel, PublicationContract};

#[test]
fn complete_sensor_result_fits_the_default_checked_publication_connection() {
    let observation = ObservationContract::prepare().unwrap();
    let publication = PublicationContract::prepare(PublicationChannel::Reading).unwrap();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    observation.install(&mut startup, &mut profiles).unwrap();
    publication.install(&mut startup, &mut profiles).unwrap();
    let syntax = conduit_plot::parse_syntax_document(
        "plot result-route {\n convert: device/dht11-observation\n publish: environment/observe-dht11\n convert.result >> publish.value\n}\n",
    );
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot(&checked, "result-route", &profiles).unwrap();
    let offers = [observation.kind(), publication.kind()]
        .into_iter()
        .enumerate()
        .map(|(index, kind)| {
            c::BackOfferBuilder::new(
                kind.clone(),
                c::Back {
                    capability_id: format!("fixture/capability/{index}").into(),
                    execution_profile_id: "fixture/pure@1".into(),
                    implementation_id: format!("fixture/implementation/{index}").into(),
                    artifact_id: "fixture/pure".into(),
                    host_calls: vec![],
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                },
            )
            .build()
        })
        .collect();
    let hosts = [c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "fixture/host".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/pure@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: offers,
        planner_capabilities: vec![],
    }];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let queues =
        conduit_planner::default_expanded_connection_limits(&expanded, &hosts, &placements)
            .unwrap();
    assert_eq!(queues.len(), 1);
    let capacity = queues.values().next().unwrap().byte_capacity;
    let mut decoder = PreparedSourceFrameDecoder::prepare(&observation).unwrap();
    // Five real data octets, including the Source-checked checksum, pass through
    // the authored decoder and the exact native quantity Form conversion.
    let encoded = decoder.decode(5, [55, 0, 24, 0, 79]).unwrap();
    assert_eq!(encoded.len(), 2274);
    assert_eq!(capacity, encoded.len() as u32);
    assert!(capacity <= observation.kind().limits.max_queue_bytes);
    assert!(capacity <= publication.kind().limits.max_queue_bytes);
    let value = c::validate_canonical_structured_value(encoded).unwrap();
    assert!(value.variant_payload("observation").unwrap().is_some());
}

#[test]
fn publication_fronts_retain_their_exact_checked_type_storage_bounds() {
    for channel in PublicationChannel::ALL {
        let publication = PublicationContract::prepare(channel).unwrap();
        let front = publication.kind().checked_front();
        let value = front
            .value_contract(&c::FrontValueLocation::Input(c::port_id("value")))
            .unwrap();
        assert_eq!(
            value.maximum_bytes,
            conduit_plot::maximum_prepared_canonical_value_bytes(publication.value_type())
                .unwrap()
                .min(channel.maximum_bytes() as u32),
        );
        assert_eq!(
            &value.value_kind,
            publication.value_type().profile().unwrap().value_kind(),
        );
    }
}
