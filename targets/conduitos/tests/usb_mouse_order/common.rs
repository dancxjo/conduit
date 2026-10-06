//! Typed fixtures shared by mouse expression and kernel conformance.
// Each integration binary uses the helpers for its own proof surface.
#![allow(dead_code)]
use conduit_core::*;
use conduit_plot::PortableExpressionProgram;

pub(super) fn program(entry: &str) -> PortableExpressionProgram {
    let [program] = programs([entry]);
    program
}

pub(super) fn programs<const N: usize>(entries: [&str; N]) -> [PortableExpressionProgram; N] {
    // Check the exact inert package once; expand and prepare each operation independently.
    let package = conduitos::protocol_source::usb_hid_mouse_order_package().unwrap();
    let source = conduitos::protocol_source::PreparedProtocolSource::prepare(package).unwrap();
    entries.map(|entry| {
        let expanded = source
            .expand(entry)
            .unwrap_or_else(|error| panic!("{entry}: {error:?}"))
            .expanded;
        assert_eq!(expanded.gears.len(), 1);
        let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
            panic!("pure Source expression")
        };
        PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
    })
}

pub(super) fn field_type(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
pub(super) fn insertion(ty: &StructuredInfoType, state: &[u8], ordinal: u64, tag: &str) -> Vec<u8> {
    let observation_type = field_type(ty, "observation");
    let result_type = field_type(&observation_type, "observed");
    let unit = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/unit")).unwrap(),
        vec![],
    )
    .unwrap();
    let observed = StructuredInfoValue::variant(result_type, tag, unit).unwrap();
    let observation = StructuredInfoValue::record(
        observation_type.clone(),
        vec![
            StructuredFieldValue::new(
                "ordinal",
                StructuredInfoValue::leaf(
                    field_type(&observation_type, "ordinal"),
                    ordinal.to_le_bytes().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new("observed", observed).unwrap(),
        ],
    )
    .unwrap();
    StructuredInfoValue::record(
        ty.clone(),
        vec![
            StructuredFieldValue::new(
                "state",
                StructuredInfoValue::from_canonical_bytes(state).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new("observation", observation).unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
pub(super) fn encoded(value: ValidatedCanonicalStructuredValue<'_>) -> Vec<u8> {
    let mut bytes = value.type_bytes().to_vec();
    bytes.extend_from_slice(value.value_node());
    bytes
}
pub(super) fn payload(bytes: &[u8], tag: &str) -> Vec<u8> {
    encoded(
        validate_canonical_structured_value(bytes)
            .unwrap()
            .variant_payload(tag)
            .unwrap()
            .unwrap(),
    )
}

pub(super) fn assert_tag(bytes: &[u8], expected: &str) {
    assert!(
        validate_canonical_structured_value(bytes)
            .unwrap()
            .variant_payload(expected)
            .unwrap()
            .is_some()
    );
}

pub(super) fn leaf_field(
    ty: &StructuredInfoType,
    name: &str,
    bytes: &[u8],
) -> StructuredFieldValue {
    StructuredFieldValue::new(
        name,
        StructuredInfoValue::leaf(field_type(ty, name), bytes.to_vec()).unwrap(),
    )
    .unwrap()
}

pub(super) fn prepared() -> (
    StructuredInfoType,
    conduitos::pure_protocol_play::PreparedPureProtocolPlay,
) {
    use std::collections::BTreeMap;
    let package = conduitos::protocol_source::usb_hid_mouse_order_package().unwrap();
    let entry = conduitos::protocol_source::PreparedProtocolEntry::prepare(
        &serde_json::to_vec(&package).unwrap(),
        "usb-hid-mouse-order-lifecycle",
    )
    .unwrap();
    let schema = entry.input_schema(&PortId::from("command")).unwrap();
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/mouse-source-host".into(),
        boot_id: "fixture/mouse-source-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/mouse-source".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    entry.publish_pure_backs(&mut host).unwrap();
    let hosts = [host];
    let placements = entry.placements(&hosts).unwrap();
    let artifact = entry
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
        .unwrap();
    let run = artifact
        .prepare_pure(conduit_composite::KernelCompositeSignStorage {
            additional_local_items: 60000,
            additional_remote_items: 60000,
        })
        .unwrap();
    (schema, run)
}
