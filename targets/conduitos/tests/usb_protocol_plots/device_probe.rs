//! Source-owned device request and actual-length framing, before native wiring.
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
        include_str!("../../plots/usb/descriptors.conduit"),
        include_str!("../../plots/usb/device-probe.conduit"),
    ]
    .join("\n")
}

fn program(entry: &str) -> conduit_plot::PortableExpressionProgram {
    use conduit_core::ConfigurationValue;
    use conduit_plot::{
        check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    };
    let contract = ControlContract::prepare().unwrap();
    let (startup, profile) = contract.catalogs();
    let syntax = parse_syntax_document(&source());
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &profile)
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("pure program")
    };
    conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

fn tag(output: &[u8], expected: &str) {
    assert!(
        validate_canonical_structured_value(output)
            .unwrap()
            .variant_payload(expected)
            .unwrap()
            .is_some(),
        "expected {expected}"
    );
}

#[test]
fn source_constructs_exact_device_descriptor_request() {
    let p = program("usb-device-descriptor-request");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let bytes = evaluator.evaluate(&[]).unwrap();
    let value = validate_canonical_structured_value(bytes).unwrap();
    assert_eq!(
        value
            .record_field("setup")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap(),
        [128, 6, 0, 1, 0, 0, 18, 0]
    );
}

#[test]
fn actual_transfer_frames_never_decode_short_padding_and_preserve_refusals() {
    let p = program("usb-device-transfer-frame");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let contract = ControlContract::prepare().unwrap();
    let mut encoder = PreparedControlResultEncoder::new(&contract).unwrap();
    let request = ControlTransferRequest::new([128, 6, 0, 1, 0, 0, 18, 0], &[], 256).unwrap();
    let wire = [18, 1, 0, 2, 0, 0, 0, 64, 0x27, 6, 1, 0, 0, 0, 0, 0, 0, 1];
    for actual in 0..18 {
        let input = encoder
            .completed(&request, actual, &wire[..actual as usize])
            .unwrap();
        tag(evaluator.evaluate(input).unwrap(), "short");
    }
    let input = encoder.completed(&request, 18, &wire).unwrap();
    tag(evaluator.evaluate(input).unwrap(), "frame");
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
fn inconsistent_counts_and_shortness_are_malformed() {
    use conduit_core::{StructuredFieldValue, StructuredInfoValue, StructuredInfoValueShape};
    let p = program("usb-device-transfer-frame");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let contract = ControlContract::prepare().unwrap();
    let mut encoder = PreparedControlResultEncoder::new(&contract).unwrap();
    let request = ControlTransferRequest::new([128, 6, 0, 1, 0, 0, 18, 0], &[], 256).unwrap();
    for (actual, field, changed) in [
        (18, "transferred", 17_u16.to_le_bytes().to_vec()),
        (18, "short", vec![1]),
        (0, "short", vec![0]),
    ] {
        let bytes = encoder
            .completed(&request, actual, &vec![0; actual as usize])
            .unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
        let StructuredInfoValueShape::Variant { payload, .. } = value.shape() else {
            panic!("variant")
        };
        let StructuredInfoValueShape::Record(fields) = payload.shape() else {
            panic!("record")
        };
        let fields = fields
            .iter()
            .map(|member| {
                StructuredFieldValue::new(
                    member.name(),
                    if member.name() == field {
                        StructuredInfoValue::leaf(
                            member.value().value_type().clone(),
                            changed.clone(),
                        )
                        .unwrap()
                    } else {
                        member.value().clone()
                    },
                )
                .unwrap()
            })
            .collect();
        let payload = StructuredInfoValue::record(payload.value_type().clone(), fields).unwrap();
        let invalid =
            StructuredInfoValue::variant(value.value_type().clone(), "completed", payload)
                .unwrap()
                .canonical_bytes()
                .unwrap();
        tag(evaluator.evaluate(&invalid).unwrap(), "malformed");
    }
}

#[test]
fn device_probe_expands_into_control_and_source_operations() {
    use conduitos::protocol_source::{PreparedProtocolSource, ProtocolSourcePackage};
    let package = ProtocolSourcePackage::compile(source(), &[]).unwrap();
    let prepared = PreparedProtocolSource::prepare(package).unwrap();
    let expanded = prepared.expand("usb-device-probe").unwrap().expanded;
    assert_eq!(
        expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == "machine/usb/control")
            .count(),
        1
    );
    assert!(expanded.gears.len() > 3);
}

#[test]
fn usb_source_prepares_without_advertising_transfer_authority() {
    use conduitos::protocol_source::{PreparedProtocolSource, ProtocolSourcePackage};
    let package = ProtocolSourcePackage::compile(source(), &[]).unwrap();
    let prepared = PreparedProtocolSource::prepare(package).unwrap();
    assert!(
        prepared
            .capabilities
            .iter()
            .all(|offer| offer.kind_id.as_str() != "machine/usb/control")
    );
}

#[test]
fn descriptor_exchange_plans_against_explicit_control_root_and_exact_fore() {
    use conduitos::usb_base::{control_proof_plan::ControlProofSubject, device_probe_proof_plan};
    let subject = ControlProofSubject {
        host_id: "host/usb-device-proof",
        boot_id: "boot/usb-device-proof",
        controller_base_id: "base/usb-device-proof",
        device_instance_id: "device/usb-device-proof",
        root_port: 1,
        slot: 1,
        attachment_epoch: 1,
    };
    let prepared = device_probe_proof_plan::prepare(&subject).unwrap();
    let definition = prepared.artifact().definition();
    assert_eq!(definition.boundary.input_fronts.len(), 1);
    assert_eq!(definition.boundary.output_fronts.len(), 2);
    assert_eq!(
        definition.internal_plan.fragments[0]
            .placements
            .iter()
            .filter(|placement| placement.kind_id.as_str() == "machine/usb/control")
            .count(),
        1
    );
}
