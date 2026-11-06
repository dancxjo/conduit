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
        include_str!("../../plots/usb/hub-port-power.conduit"),
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
fn hub_power_requests_cover_every_nonzero_port_without_play_allocations() {
    for (entry, opcode) in [
        ("usb-hub-port-power-on-request", 3),
        ("usb-hub-port-power-off-request", 1),
    ] {
        let program = program(entry);
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        tag(evaluator.evaluate(&[0]).unwrap(), "invalid-port");
        let allocations = super::allocation::allocations(|| {
            for port in 1..=255_u8 {
                let value =
                    validate_canonical_structured_value(evaluator.evaluate(&[port]).unwrap())
                        .unwrap();
                let request = value.variant_payload("request").unwrap().unwrap();
                assert_eq!(
                    request
                        .record_field("setup")
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u64")
                        .unwrap(),
                    [0x23, opcode, 8, 0, port, 0, 0, 0]
                );
                assert_eq!(
                    request
                        .record_field("output")
                        .unwrap()
                        .unwrap()
                        .collection_length()
                        .unwrap(),
                    0
                );
            }
        });
        assert_eq!(allocations, 0);
    }
}
#[test]
fn hub_power_acknowledgement_preserves_failure_and_requires_empty_completion() {
    let program = program("usb-hub-port-power-result");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let contract = ControlContract::prepare().unwrap();
    let mut encoder = PreparedControlResultEncoder::new(&contract).unwrap();
    let request = ControlTransferRequest::new([0x23, 3, 8, 0, 2, 0, 0, 0], &[], 64).unwrap();
    let completed = encoder.completed(&request, 0, &[]).unwrap().to_vec();
    tag(evaluator.evaluate(&completed).unwrap(), "acknowledged");
    for (field, value) in [("short", vec![1]), ("transferred", vec![1, 0])] {
        let invalid = super::control_reply::replace_completed_leaf(&completed, field, &value);
        tag(evaluator.evaluate(&invalid).unwrap(), "malformed");
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
fn hub_port_power_source_checks_and_expands_through_existing_usb_offer() {
    use conduitos::protocol_source::{PreparedProtocolSource, ProtocolSourcePackage};
    let package = ProtocolSourcePackage::compile(source(), &[]).unwrap();
    let prepared = PreparedProtocolSource::prepare(package).unwrap();
    let expanded = prepared.expand("usb-hub-power-port").unwrap().expanded;
    assert_eq!(
        expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == "machine/usb/control")
            .count(),
        1
    );
}
