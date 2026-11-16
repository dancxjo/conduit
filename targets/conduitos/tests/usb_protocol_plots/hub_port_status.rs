use conduit_core::validate_canonical_structured_value;
use conduit_plot::PreparedPortableExpressionEvaluator;
use conduitos::usb_base::{
    control_contract::ControlContract,
    control_request::ControlTransferRequest,
    control_result::{ControlTransferDisposition, PreparedControlResultEncoder},
};
fn source() -> String {
    [
        include_str!("../../plots/usb/control-types.conduit"),
        include_str!("../../plots/usb/hub-port-status.conduit"),
    ]
    .join("\n")
}
fn program(entry: &str) -> conduit_plot::PortableExpressionProgram {
    let (startup, profile) = ControlContract::prepare().unwrap().catalogs();
    super::program_with_catalog(&source(), entry, &startup, &profile)
}
fn tag(bytes: &[u8], expected: &str) {
    assert!(
        validate_canonical_structured_value(bytes)
            .unwrap()
            .variant_payload(expected)
            .unwrap()
            .is_some()
    );
}
#[test]
fn hub_port_status_request_is_exact_for_every_nonzero_port() {
    let program = program("usb-hub-port-status-request");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    tag(evaluator.evaluate(&[0]).unwrap(), "invalid-port");
    for port in 1..=255_u8 {
        let value =
            validate_canonical_structured_value(evaluator.evaluate(&[port]).unwrap()).unwrap();
        let request = value.variant_payload("request").unwrap().unwrap();
        assert_eq!(
            request
                .record_field("setup")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            [0xa3, 0, 0, 0, port, 0, 4, 0]
        );
    }
}
#[test]
fn hub_port_status_decodes_wire_bits_without_allocating_or_clearing_changes() {
    let program = program("usb-hub-port-status-result");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let contract = ControlContract::prepare().unwrap();
    let mut encoder = PreparedControlResultEncoder::new(&contract).unwrap();
    let request = ControlTransferRequest::new([0xa3, 0, 0, 0, 1, 0, 4, 0], &[], 64).unwrap();
    let bytes = encoder
        .completed(&request, 4, &[0x1f, 7, 0x15, 0x80])
        .unwrap()
        .to_vec();
    let allocations = super::allocation::allocations(|| {
        let value =
            validate_canonical_structured_value(evaluator.evaluate(&bytes).unwrap()).unwrap();
        let observed = value.variant_payload("observed").unwrap().unwrap();
        for field in [
            "connected",
            "enabled",
            "suspended",
            "overcurrent",
            "resetting",
            "powered",
            "low-speed",
            "high-speed",
        ] {
            assert_eq!(
                observed
                    .record_field(field)
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/bool")
                    .unwrap(),
                [1]
            );
        }
        assert_eq!(
            observed
                .record_field("status")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u16")
                .unwrap(),
            [0x1f, 7]
        );
        assert_eq!(
            observed
                .record_field("change")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u16")
                .unwrap(),
            [0x15, 0x80]
        );
    });
    assert_eq!(allocations, 0);
    for length in 0..4 {
        tag(
            evaluator
                .evaluate(
                    encoder
                        .completed(&request, length as u16, &bytes[..length])
                        .unwrap(),
                )
                .unwrap(),
            "malformed",
        );
    }
    for (disposition, expected) in [
        (ControlTransferDisposition::Stalled, "stalled"),
        (ControlTransferDisposition::ProviderLost, "provider-lost"),
        (ControlTransferDisposition::Unsupported, "unsupported"),
    ] {
        tag(
            evaluator
                .evaluate(encoder.disposition(disposition).unwrap())
                .unwrap(),
            expected,
        );
    }
}
#[test]
fn hub_port_status_source_checks_and_expands() {
    use conduitos::protocol_source::{PreparedProtocolSource, ProtocolSourcePackage};
    let package = ProtocolSourcePackage::compile(source(), &[]).unwrap();
    let prepared = PreparedProtocolSource::prepare(package).unwrap();
    let expanded = prepared.expand("usb-hub-observe-port").unwrap().expanded;
    assert_eq!(
        expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == "machine/usb/control")
            .count(),
        1
    );
}

#[test]
fn hub_port_status_flags_are_independent_wire_bits() {
    let program = program("usb-hub-port-status-result");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let contract = ControlContract::prepare().unwrap();
    let mut encoder = PreparedControlResultEncoder::new(&contract).unwrap();
    let request = ControlTransferRequest::new([0xa3, 0, 0, 0, 1, 0, 4, 0], &[], 64).unwrap();
    let flags = [
        ("connected", 0),
        ("enabled", 1),
        ("suspended", 2),
        ("overcurrent", 3),
        ("resetting", 4),
        ("powered", 8),
        ("low-speed", 9),
        ("high-speed", 10),
    ];
    for selected in [None]
        .into_iter()
        .chain(flags.iter().map(|(_, bit)| Some(*bit)))
    {
        let wire = selected.map_or(0_u16, |bit| 1 << bit).to_le_bytes();
        let bytes = encoder
            .completed(&request, 4, &[wire[0], wire[1], 0, 0])
            .unwrap();
        let value =
            validate_canonical_structured_value(evaluator.evaluate(bytes).unwrap()).unwrap();
        let observed = value.variant_payload("observed").unwrap().unwrap();
        for (field, bit) in flags {
            assert_eq!(
                observed
                    .record_field(field)
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/bool")
                    .unwrap(),
                [u8::from(selected == Some(bit))]
            );
        }
    }
    let bytes = encoder.completed(&request, 4, &[0; 4]).unwrap().to_vec();
    for (field, value) in [("short", vec![1]), ("transferred", vec![3, 0])] {
        let invalid = super::control_reply::replace_completed_leaf(&bytes, field, &value);
        tag(evaluator.evaluate(&invalid).unwrap(), "malformed");
    }
}
