//! Checked descriptor interpretation consumes the exact control-result contract.
use super::*;
use conduit_core::validate_canonical_structured_value;
use conduitos::usb_base::{
    control_contract::ControlContract,
    control_request::ControlTransferRequest,
    control_result::{ControlTransferDisposition, PreparedControlResultEncoder},
};

fn decoder() -> PortableExpressionProgram {
    let contract = ControlContract::prepare().unwrap();
    let (startup, profile) = contract.catalogs();
    let source = format!(
        "{}\n{}",
        include_str!("../../plots/usb/descriptors.conduit"),
        include_str!("../../plots/usb/device-read.conduit")
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "usb-device-read", &profile)
        .unwrap()
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("checked expression")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    assert_eq!(program.input_type, *contract.result_type());
    program
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
fn acknowledged_control_results_decode_without_padding_or_failure_translation() {
    let contract = ControlContract::prepare().unwrap();
    let program = decoder();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let capacity = prepared.output_capacity();
    let mut encoder = PreparedControlResultEncoder::new(&contract).unwrap();
    let request = ControlTransferRequest::new([128, 6, 0, 1, 0, 0, 64, 0], &[], 256).unwrap();
    let wire = [
        18, 1, 0, 2, 0, 0, 0, 64, 0x34, 0x12, 0xcd, 0xab, 0, 1, 1, 2, 3, 1,
    ];
    for actual in 0..18 {
        let bytes = encoder
            .completed(&request, actual, &wire[..usize::from(actual)])
            .unwrap();
        tag(prepared.evaluate(bytes).unwrap(), "short");
    }
    for (index, value) in [(0, 17), (1, 2)] {
        let mut bad = wire;
        bad[index] = value;
        tag(
            prepared
                .evaluate(encoder.completed(&request, 18, &bad).unwrap())
                .unwrap(),
            "malformed",
        );
    }
    let input = encoder.completed(&request, 18, &wire).unwrap().to_vec();
    let expected = program.evaluate(&input).unwrap();
    tag(&expected, "device");
    let device = validate_canonical_structured_value(&expected)
        .unwrap()
        .variant_payload("device")
        .unwrap()
        .unwrap();
    assert_eq!(
        device
            .record_field("vendor_id")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap(),
        0x1234_u64.to_le_bytes()
    );
    for _ in 0..10_000 {
        assert_eq!(prepared.evaluate(&input).unwrap(), expected);
        assert_eq!(prepared.output_capacity(), capacity);
    }
    for (case, disposition) in [
        ("stalled", ControlTransferDisposition::Stalled),
        ("provider-lost", ControlTransferDisposition::ProviderLost),
        ("unsupported", ControlTransferDisposition::Unsupported),
    ] {
        tag(
            prepared
                .evaluate(encoder.disposition(disposition).unwrap())
                .unwrap(),
            case,
        );
    }
    assert!(prepared.evaluate(&input[..input.len() - 1]).is_err());
}
