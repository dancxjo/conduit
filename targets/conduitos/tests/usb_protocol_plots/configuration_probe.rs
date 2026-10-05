//! Source control request and completion framing precede descriptor walking.
use conduit_core::validate_canonical_structured_value;
use conduitos::usb_base::{
    control_contract::ControlContract,
    control_request::ControlTransferRequest,
    control_result::{ControlTransferDisposition, PreparedControlResultEncoder},
};

fn source() -> String {
    format!(
        "{}\n{}\n{}",
        include_str!("../../plots/usb/control-types.conduit"),
        include_str!("../../plots/usb/configuration-walk.conduit"),
        include_str!("../../plots/usb/configuration-probe.conduit")
    )
}
fn program(entry: &str) -> conduit_plot::PortableExpressionProgram {
    let (startup, profile) = ControlContract::prepare().unwrap().catalogs();
    super::program_with_catalog(&source(), entry, &startup, &profile)
}

#[test]
fn request_has_exact_configuration_zero_and_finite_length() {
    let program = program("usb-configuration-descriptor-request");
    let bytes = program.evaluate(&[]).unwrap();
    let request = validate_canonical_structured_value(&bytes).unwrap();
    assert_eq!(
        request
            .record_field("setup")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap(),
        [128, 6, 0, 2, 0, 0, 0, 1]
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
#[test]
fn framing_keeps_actual_extent_and_every_physical_disposition() {
    let program = program("usb-configuration-transfer-frame");
    let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let contract = ControlContract::prepare().unwrap();
    let mut encoder = PreparedControlResultEncoder::new(&contract).unwrap();
    let request = ControlTransferRequest::new([128, 6, 0, 2, 0, 0, 0, 1], &[], 256).unwrap();
    let wire = [0x5a; 256];
    for count in [0, 8, 9, 25, 255, 256] {
        let input = encoder
            .completed(&request, count, &wire[..count as usize])
            .unwrap();
        let output = prepared.evaluate(input).unwrap();
        assert_eq!(output, program.evaluate(input).unwrap());
        let frame = validate_canonical_structured_value(output)
            .unwrap()
            .variant_payload("frame")
            .unwrap()
            .unwrap();
        assert_eq!(
            frame
                .record_field("actual")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            u64::from(count).to_le_bytes()
        );
        assert_eq!(
            frame
                .record_field("wire")
                .unwrap()
                .unwrap()
                .collection_length()
                .unwrap(),
            u32::from(count)
        );
    }
    for (disposition, tag) in [
        (ControlTransferDisposition::Stalled, "stalled"),
        (ControlTransferDisposition::ProviderLost, "provider-lost"),
        (ControlTransferDisposition::Unsupported, "unsupported"),
    ] {
        let input = encoder.disposition(disposition).unwrap();
        assert!(
            validate_canonical_structured_value(prepared.evaluate(input).unwrap())
                .unwrap()
                .variant_payload(tag)
                .unwrap()
                .is_some()
        );
    }
}

#[test]
fn checked_probe_has_one_control_call_and_no_discovered_authority() {
    use conduitos::protocol_source::{PreparedProtocolSource, ProtocolSourcePackage};
    let prepared =
        PreparedProtocolSource::prepare(ProtocolSourcePackage::compile(source(), &[]).unwrap())
            .unwrap();
    let expanded = prepared.expand("usb-configuration-probe").unwrap().expanded;
    assert_eq!(expanded.gears.len(), 60);
    assert_eq!(
        expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == "machine/usb/control")
            .count(),
        1
    );
    assert!(
        prepared
            .capabilities
            .iter()
            .all(|offer| offer.kind_id.as_str() != "machine/usb/control")
    );
}

#[test]
fn contradictory_short_flag_or_count_never_creates_a_configuration_frame() {
    let program = program("usb-configuration-transfer-frame");
    let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let contract = ControlContract::prepare().unwrap();
    let mut encoder = PreparedControlResultEncoder::new(&contract).unwrap();
    let request = ControlTransferRequest::new([128, 6, 0, 2, 0, 0, 0, 1], &[], 256).unwrap();
    let wire = [0; 256];
    for (actual, field, changed) in [
        (0, "short", vec![0]),
        (256, "short", vec![1]),
        (25, "transferred", 26_u16.to_le_bytes().to_vec()),
        (256, "transferred", 257_u16.to_le_bytes().to_vec()),
    ] {
        let bytes = encoder
            .completed(&request, actual, &wire[..actual as usize])
            .unwrap();
        let input = super::control_reply::replace_completed_leaf(bytes, field, &changed);
        let output = prepared.evaluate(&input).unwrap();
        assert_eq!(output, program.evaluate(&input).unwrap());
        assert!(
            validate_canonical_structured_value(output)
                .unwrap()
                .variant_payload("malformed")
                .unwrap()
                .is_some()
        );
    }
}
