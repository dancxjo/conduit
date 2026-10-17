use conduit_composite::KernelCompositeSignStorage;
use conduit_core::*;
use conduitos::{
    protocol_source::{
        PreparedProtocolArtifact, PreparedProtocolEntry, ProtocolSourcePackage,
        ProtocolSpecializationRequest, ProtocolValueReference,
    },
    pure_protocol_play::PreparedPureProtocolPlay,
};
use std::collections::BTreeMap;

pub(super) fn artifact(entry_name: &str) -> PreparedProtocolArtifact {
    let package = ProtocolSourcePackage::compile(
        include_str!("counter.conduit").into(),
        &[ProtocolSpecializationRequest::SeededUntil {
            value: ProtocolValueReference {
                type_name: "PureCounterFrame".into(),
                maximum_bytes: 4096,
            },
        }],
    )
    .unwrap();
    let entry =
        PreparedProtocolEntry::prepare(&serde_json::to_vec(&package).unwrap(), entry_name).unwrap();
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/pure-source-host".into(),
        boot_id: "fixture/pure-source-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/pure-source".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    entry.publish_pure_backs(&mut host).unwrap();
    let hosts = [host];
    let placements = entry.placements(&hosts).unwrap();
    entry
        .plan(
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 4096,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .unwrap()
}

pub(super) fn storage() -> KernelCompositeSignStorage {
    KernelCompositeSignStorage {
        additional_local_items: 8192,
        additional_remote_items: 1024,
    }
}

pub(super) fn prepare(entry: &str) -> PreparedPureProtocolPlay {
    artifact(entry).prepare_pure(storage()).unwrap()
}

pub(super) fn input(run: &PreparedPureProtocolPlay, value: u64) -> (PortId, ValuePayload) {
    let port = &run.kernel().definition().boundary.input_fronts[0].external_port;
    (
        port.port_id.clone(),
        ValuePayload {
            value_kind: port.value_kind.clone(),
            encoded: value.to_le_bytes().to_vec(),
        },
    )
}

pub(super) fn output(run: &PreparedPureProtocolPlay) -> (PortId, ValuePayload) {
    let port = &run.kernel().definition().boundary.output_fronts[0].external_port;
    (
        port.port_id.clone(),
        ValuePayload {
            value_kind: port.value_kind.clone(),
            encoded: Vec::with_capacity(4096),
        },
    )
}

pub(super) fn count(bytes: &[u8]) -> u64 {
    let value = validate_canonical_structured_value(bytes).unwrap();
    u64::from_le_bytes(
        value
            .record_field("value")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap()
            .try_into()
            .unwrap(),
    )
}
