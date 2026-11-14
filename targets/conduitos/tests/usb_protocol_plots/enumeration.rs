//! Enumeration wire meaning stays independent of the native controller.
use conduit_core::*;
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};
fn program(entry: &str) -> PortableExpressionProgram {
    let source = [
        include_str!("../../plots/usb/control-types.conduit"),
        include_str!("../../plots/usb/enumeration-requests.conduit"),
        include_str!("../../plots/usb/enumeration-descriptors.conduit"),
    ]
    .join("\n");
    let (startup, profile) = conduitos::usb_base::control_contract::ControlContract::prepare()
        .unwrap()
        .catalogs();
    super::program_with_catalog(&source, entry, &startup, &profile)
}
fn field(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
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
fn leaf(ty: &StructuredInfoType, name: &str, bytes: &[u8]) -> StructuredFieldValue {
    StructuredFieldValue::new(
        name,
        StructuredInfoValue::leaf(field(ty, name), bytes.to_vec()).unwrap(),
    )
    .unwrap()
}
fn frame(ty: &StructuredInfoType, wire: &[u8], actual: u64) -> Vec<u8> {
    StructuredInfoValue::record(
        ty.clone(),
        vec![
            StructuredFieldValue::new(
                "wire",
                StructuredInfoValue::sequence(
                    field(ty, "wire"),
                    wire.iter()
                        .map(|b| {
                            StructuredInfoValue::leaf(
                                StructuredInfoType::leaf(kind_id("value/u8")).unwrap(),
                                vec![*b],
                            )
                            .unwrap()
                        })
                        .collect(),
                )
                .unwrap(),
            )
            .unwrap(),
            leaf(ty, "actual", &actual.to_le_bytes()),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn tag(bytes: &[u8], expected: &str) {
    assert!(
        validate_canonical_structured_value(bytes)
            .unwrap()
            .variant_payload(expected)
            .unwrap()
            .is_some(),
        "{expected}"
    );
}
fn setup(bytes: &[u8], wrapped: bool) -> [u8; 8] {
    let value = validate_canonical_structured_value(bytes).unwrap();
    let request = if wrapped {
        value.variant_payload("request").unwrap().unwrap()
    } else {
        value
    };
    request
        .record_field("setup")
        .unwrap()
        .unwrap()
        .primitive_bytes("value/u64")
        .unwrap()
        .try_into()
        .unwrap()
}
#[test]
fn enumeration_requests_bound_wire_values_and_descriptor_prefixes() {
    for (entry, expected) in [
        (
            "usb-device-initial-descriptor-request",
            [0x80, 6, 0, 1, 0, 0, 8, 0],
        ),
        ("usb-hub-descriptor-request", [0xa0, 6, 0, 0x29, 0, 0, 9, 0]),
    ] {
        assert_eq!(
            setup(&program(entry).evaluate(&[]).unwrap(), false),
            expected
        );
    }
    for (entry, opcode, maximum) in [
        ("usb-device-address-request", 5, 127_u8),
        ("usb-device-configuration-request", 9, 255_u8),
    ] {
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&program(entry)).unwrap();
        tag(evaluator.evaluate(&[0]).unwrap(), "invalid-value");
        for value in [1, maximum] {
            assert_eq!(
                setup(evaluator.evaluate(&[value]).unwrap(), true),
                [0, opcode, value, 0, 0, 0, 0, 0]
            );
        }
        if maximum < 255 {
            tag(evaluator.evaluate(&[maximum + 1]).unwrap(), "invalid-value");
        }
    }
    let report = program("usb-hid-report-descriptor-request");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&report).unwrap();
    for (length, expected) in [(66_u64, 66_u16), (426, 256), (u64::MAX, 256)] {
        let bytes = StructuredInfoValue::record(
            report.input_type.clone(),
            vec![
                leaf(&report.input_type, "interface", &[7]),
                leaf(&report.input_type, "length", &length.to_le_bytes()),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let allocations = super::allocation::allocations(|| {
            let n = expected.to_le_bytes();
            assert_eq!(
                setup(evaluator.evaluate(&bytes).unwrap(), true),
                [0x81, 6, 0, 0x22, 7, 0, n[0], n[1]]
            );
        });
        assert_eq!(allocations, 0);
    }
}
#[test]
fn initial_geometry_and_hub_power_timing_come_from_actual_descriptor_bytes() {
    for (entry, cases) in [
        (
            "usb-device-initial-descriptor",
            vec![
                (vec![18, 1, 0, 2, 0, 0, 0, 64], 8, "device"),
                (vec![18, 1, 0, 2, 0, 0, 0, 8], 8, "device"),
                (vec![18, 1, 0, 2, 0, 0, 0, 7], 8, "unsupported"),
                (vec![18, 1], 2, "short"),
                (vec![18, 1], 8, "malformed"),
            ],
        ),
        (
            "usb-hub-descriptor",
            vec![
                (vec![9, 41, 5, 9, 0, 50, 0, 0, 255], 9, "hub"),
                (vec![9, 41, 9, 9, 0, 50, 0, 0, 255], 9, "unsupported"),
                (vec![9, 41, 0, 9, 0, 50, 0, 0, 255], 9, "malformed"),
                (vec![9, 41], 2, "short"),
            ],
        ),
    ] {
        let program = program(entry);
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        for (wire, actual, expected) in cases {
            let bytes = frame(&program.input_type, &wire, actual);
            let allocations = super::allocation::allocations(|| {
                tag(evaluator.evaluate(&bytes).unwrap(), expected);
            });
            assert_eq!(allocations, 0);
            if expected == "hub" {
                let hub = validate_canonical_structured_value(evaluator.evaluate(&bytes).unwrap())
                    .unwrap()
                    .variant_payload("hub")
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    hub.record_field("ports")
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u8")
                        .unwrap(),
                    [5]
                );
                assert_eq!(
                    hub.record_field("power-good-delay-ms")
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u64")
                        .unwrap(),
                    100_u64.to_le_bytes()
                );
            }
        }
    }
}
