use super::*;

fn source() -> String {
    "type Packet = {\n code: U8\n}\nplot identity (\n >> input: Packet...|\n output: Packet...| >>\n) = (.)".into()
}
fn request(name: &str, maximum_bytes: u32) -> ProtocolSpecializationRequest {
    ProtocolSpecializationRequest::SeededFlow {
        value: ProtocolValueReference {
            type_name: name.into(),
            maximum_bytes,
        },
    }
}
#[test]
fn named_type_preparation_preserves_exact_source_and_checks_the_final_plot_separately() {
    let original = source();
    let package =
        ProtocolSourcePackage::compile(original.clone(), &[request("Packet", 128)]).unwrap();
    assert_eq!(package.source, original);
    let encoded = serde_json::to_vec(&package).unwrap();
    let entry = PreparedProtocolEntry::prepare(&encoded, "identity").unwrap();
    assert_eq!(entry.expanded().expanded.gears.len(), 1);
    let ProtocolSpecialization::SeededFlow { value } = &package.specializations[0] else {
        panic!()
    };
    assert_eq!(value.contract.maximum_bytes, 128);
    assert_eq!(
        value.schema.profile().unwrap().value_kind(),
        &value.contract.value_kind
    );
    let invalid = original.replace(
        "= (.)",
        "{\n missing: nonexistent/kind\n input >> missing >> output\n}",
    );
    let package = ProtocolSourcePackage::compile(invalid, &[request("Packet", 128)]).unwrap();
    assert!(
        PreparedProtocolEntry::prepare(&serde_json::to_vec(&package).unwrap(), "identity").is_err()
    );
}
#[test]
fn unknown_types_invalid_envelopes_and_invalid_declarations_refuse() {
    for request in [
        request("absent", 128),
        request("Packet", 0),
        request("Packet", 4097),
    ] {
        assert!(ProtocolSourcePackage::compile(source(), &[request]).is_err());
    }
    assert!(
        ProtocolSourcePackage::compile("type Packet = Unknown".into(), &[request("Packet", 128)])
            .is_err()
    );
    assert!(
        ProtocolSourcePackage::compile(
            source(),
            &vec![request("Packet", 128); MAXIMUM_SPECIALIZATIONS + 1]
        )
        .is_err()
    );
}

#[test]
fn usb_duplex_plot_reuses_one_boolean_request_feedback_zip_specialization() {
    let source = include_str!("../../../plots/usb/control-endpoint-write-read.conduit");
    let package = ProtocolSourcePackage::compile(
        source.into(),
        &[ProtocolSpecializationRequest::FeedbackZip {
            left: ProtocolValueReference {
                type_name: "UsbTransferCompleted".into(),
                maximum_bytes: 256,
            },
            right: ProtocolValueReference {
                type_name: "UsbWriteReadRequests".into(),
                maximum_bytes: 4096,
            },
        }],
    )
    .unwrap();
    assert_eq!(package.specializations.len(), 1);
    let entry = PreparedProtocolEntry::prepare(
        &serde_json::to_vec(&package).unwrap(),
        "usb-control-endpoint-write-read",
    )
    .unwrap();
    assert!(entry.expanded().expanded.gears.len() >= 5);
}

mod usb_reuse;

#[test]
fn ordinary_literal_retains_value_temporal_contract_in_protocol_source() {
    let source = "type SerialBootstrapState = {\n    phase: U64\n    terminal: Boolean\n}\nplot serial-bootstrap-state (\n    start: Text...| >> state: SerialBootstrapState...|\n) = ({ phase: 0, terminal: false })\nplot bootstrap (\n    ended: SerialBootstrapState...| >>\n) {\n    cell: state/seeded/flow/until\n    \"start\" >> serial-bootstrap-state() >> cell.seed\n    cell.current >> ended\n}\n";
    let package = ProtocolSourcePackage::compile(
        source.into(),
        &[ProtocolSpecializationRequest::SeededUntil {
            value: ProtocolValueReference {
                type_name: "SerialBootstrapState".into(),
                maximum_bytes: 4096,
            },
        }],
    )
    .unwrap();
    let error = PreparedProtocolEntry::prepare(&serde_json::to_vec(&package).unwrap(), "bootstrap")
        .err()
        .expect("Value cannot silently become Flow");
    let ProtocolSourceRefusal::Expansion(error) = error else {
        panic!("expected temporal expansion refusal")
    };
    assert_eq!(error.code, "CND-FRM-045");
}

#[test]
fn ordinary_literal_is_retained_as_exact_inert_source_configuration() {
    let package = ProtocolSourcePackage::compile(
        "plot literal (
    text: Text >>
) {
    \"start\" >> text
}
"
        .into(),
        &[],
    )
    .unwrap();
    let entry =
        PreparedProtocolEntry::prepare(&serde_json::to_vec(&package).unwrap(), "literal").unwrap();
    let literals: Vec<_> = entry
        .expanded()
        .expanded
        .gears
        .iter()
        .filter(|gear| gear.kind_id.as_str() == conduit_text::TEXT_LITERAL_KIND)
        .collect();
    assert_eq!(literals.len(), 1);
    assert!(literals[0].checked_front().inputs().is_empty());
    assert_eq!(literals[0].configuration.len(), 1);
    assert_eq!(literals[0].configuration[0].key, "value");
    assert_eq!(
        literals[0].configuration[0].value,
        conduit_core::ConfigurationValue::Text("start".into())
    );
}

#[test]
fn explicit_singleton_bootstraps_one_checked_state_flow() {
    let source = r#"type BootstrapState = {
 phase: U64
 terminal: Boolean
}
plot initialize (
 start: Text >> state: BootstrapState
) = ({ phase: 0, terminal: true })
plot bootstrap (
 ended: BootstrapState...| >>
) {
 lift: flow/singleton
 cell: state/seeded/flow/until
 "start" >> initialize() >> lift.value
 lift.flow >> cell.seed
 cell.current >> ended
}
"#;
    let reference = || ProtocolValueReference {
        type_name: "BootstrapState".into(),
        maximum_bytes: 4096,
    };
    let package = ProtocolSourcePackage::compile(
        source.into(),
        &[
            ProtocolSpecializationRequest::Singleton { value: reference() },
            ProtocolSpecializationRequest::SeededUntil { value: reference() },
        ],
    )
    .unwrap();
    let entry = PreparedProtocolEntry::prepare(&serde_json::to_vec(&package).unwrap(), "bootstrap")
        .unwrap();
    let lift = entry
        .expanded()
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == "flow/singleton")
        .expect("explicit singleton placement");
    assert_eq!(
        lift.checked_front().inputs()[0].temporal,
        conduit_core::PortTemporal::Value
    );
    assert_eq!(
        lift.checked_front().outputs()[0].temporal,
        conduit_core::PortTemporal::Flow { closes: true }
    );
}
