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
    let source = PreparedProtocolSource::prepare(package).unwrap();
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
    let plan = conduit_planner::plan_expanded_authoring_with_connection_limits(
        &expanded,
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
        &limits.connections,
        &limits.boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
}
