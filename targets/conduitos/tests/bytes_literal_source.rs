//! Actual pure Kernel evidence; no USB or storage implementation is offered.
use conduit_composite::{
    KernelCompositeSignStorage, KernelCompositeStatus, KernelCompositeTerminal,
};
use conduit_core::*;
use conduit_planner::PlanningOptions;
use conduitos::protocol_source::{PreparedProtocolEntry, ProtocolSourcePackage};
use std::collections::BTreeMap;
fn package(hex: &str) -> Vec<u8> {
    let source = format!(
        "plot octets (\n    value: Bytes <= 2048B >>\n) {{\n    literal: bytes/literal(\"{hex}\")\n    literal >> value\n}}\n"
    );
    serde_json::to_vec(&ProtocolSourcePackage::compile(source, &[]).unwrap()).unwrap()
}
fn entry(hex: &str) -> PreparedProtocolEntry {
    PreparedProtocolEntry::prepare(&package(hex), "octets").unwrap()
}
#[test]
fn source_roundtrip_retains_authored_hex_provenance_and_exact_pure_fore() {
    let a = entry("00ff80");
    let b = entry("00FF80");
    assert_ne!(a.package_digest(), b.package_digest());
    assert_ne!(a.resident(), b.resident());
    let mut host = host();
    a.publish_pure_backs(&mut host).unwrap();
    assert_eq!(host.capabilities.len(), 1);
    let offer = &host.capabilities[0];
    assert!(offer.host_calls.is_empty());
    assert!(offer.authority_requirements.is_empty());
    assert!(offer.resource_requirements.is_empty());
    assert_eq!(offer.outputs[0].value_kind.as_str(), "value/bytes");
    assert_eq!(
        offer.kind_contract_revision.as_str(),
        conduit_data::bytes_literal::BYTES_LITERAL_REVISION
    );
    assert_eq!(
        offer.semantic_contract.value_contracts()[0]
            .contract
            .maximum_bytes,
        2048
    );
}
fn host() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "literal/host".into(),
        boot_id: "literal/boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "literal/pure".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
#[test]
fn exact_prepared_kernel_emits_once_and_retires_with_no_host_request() {
    let entry = entry("00ff80");
    let mut advertisement = host();
    entry.publish_pure_backs(&mut advertisement).unwrap();
    let hosts = [advertisement];
    let placements = entry.placements(&hosts).unwrap();
    let empty = BTreeMap::new();
    let artifact = entry
        .plan(
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &empty,
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 2048,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .unwrap();
    let mut play = artifact
        .prepare_pure(KernelCompositeSignStorage::default())
        .unwrap();
    play.start().unwrap();
    let mut value = ValuePayload {
        value_kind: kind_id("value/bytes"),
        encoded: Vec::with_capacity(2048),
    };
    let port = port_id("value");
    let mut count = 0;
    let mut closed = false;
    for _ in 0..32 {
        let status = play.step().unwrap();
        if let Some(sequence) = play.output_into(&port, &mut value).unwrap() {
            assert_eq!(value.encoded, [0, 255, 128]);
            count += 1;
            play.complete_output(&port, sequence).unwrap();
        }
        if let Some(terminal) = play.output_terminal_into(&port, &mut value).unwrap() {
            assert_eq!(terminal, KernelCompositeTerminal::Normal);
            closed = true;
        }
        if status == KernelCompositeStatus::Complete && closed {
            assert_eq!(count, 1);
            return;
        }
    }
    panic!("literal did not retire within finite budget");
}
