//! Descriptor-driven bytes feed the shared pointer state; no native effects.
use super::{allocation, common::*};
use conduit_core::*;
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};
fn programs() -> [PortableExpressionProgram; 2] {
    let source = conduitos::protocol_source::PreparedProtocolSource::prepare(
        conduitos::protocol_source::usb_hid_absolute_package().unwrap(),
    )
    .unwrap();
    [
        "usb-hid-absolute-report-prepare",
        "usb-hid-absolute-pointer-advance",
    ]
    .map(|entry| {
        let expanded = source.expand(entry).unwrap().expanded;
        let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
            panic!("program")
        };
        PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
    })
}
fn record(ty: &StructuredInfoType, fields: Vec<StructuredFieldValue>) -> StructuredInfoValue {
    StructuredInfoValue::record(ty.clone(), fields).unwrap()
}
fn input(ty: &StructuredInfoType, wire: &[u8], actual: u64, x_byte: u64) -> Vec<u8> {
    let layout = field_type(ty, "layout");
    let frame = field_type(ty, "frame");
    let state = field_type(ty, "state");
    let layout_value = record(
        &layout,
        vec![
            leaf_field(&layout, "report-id", &[1]),
            leaf_field(&layout, "tip-byte", &1_u64.to_le_bytes()),
            leaf_field(&layout, "tip-mask", &[1]),
            leaf_field(&layout, "x-byte", &x_byte.to_le_bytes()),
            leaf_field(&layout, "y-byte", &5_u64.to_le_bytes()),
            leaf_field(&layout, "x-width", &[2]),
            leaf_field(&layout, "y-width", &[2]),
            leaf_field(&layout, "maximum-x", &1024_u16.to_le_bytes()),
            leaf_field(&layout, "maximum-y", &600_u16.to_le_bytes()),
        ],
    );
    record(
        ty,
        vec![
            StructuredFieldValue::new("layout", layout_value).unwrap(),
            StructuredFieldValue::new(
                "frame",
                record(
                    &frame,
                    vec![
                        leaf_field(&frame, "wire", wire),
                        leaf_field(&frame, "actual", &actual.to_le_bytes()),
                    ],
                ),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "state",
                record(
                    &state,
                    vec![
                        leaf_field(&state, "position-x", &0_i64.to_le_bytes()),
                        leaf_field(&state, "position-y", &0_i64.to_le_bytes()),
                        leaf_field(&state, "sequence", &7_u64.to_le_bytes()),
                    ],
                ),
            )
            .unwrap(),
            leaf_field(ty, "ordinal", &3_u64.to_le_bytes()),
            leaf_field(ty, "queue-capacity", &1_u64.to_le_bytes()),
        ],
    )
    .canonical_bytes()
    .unwrap()
}
#[test]
fn absolute_reports_preserve_press_release_and_use_the_existing_pointer_contract() {
    let [decode, normalize] = programs();
    let mut decode_owner = PreparedPortableExpressionEvaluator::new(&decode).unwrap();
    let mut pointer_owner = PreparedPortableExpressionEvaluator::new(&normalize).unwrap();
    for pressed in [true, false] {
        let bytes = input(
            &decode.input_type,
            &[1, u8::from(pressed), 9, 0, 2, 44, 1],
            7,
            3,
        );
        let prepared = decode_owner.evaluate(&bytes).unwrap();
        let point = payload(prepared, "point");
        let allocations = allocation::allocations(|| {
            let ready =
                validate_canonical_structured_value(pointer_owner.evaluate(&point).unwrap())
                    .unwrap()
                    .variant_payload("ready")
                    .unwrap()
                    .unwrap();
            let sample = ready.record_field("sample").unwrap().unwrap();
            assert_eq!(
                sample
                    .record_field("position-x")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/i64")
                    .unwrap(),
                500000_i64.to_le_bytes()
            );
            assert_eq!(
                sample
                    .record_field("position-y")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/i64")
                    .unwrap(),
                500000_i64.to_le_bytes()
            );
            assert_eq!(
                sample
                    .record_field("primary-pressed")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/bool")
                    .unwrap(),
                [u8::from(pressed)]
            );
            assert_eq!(
                sample
                    .record_field("sequence")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                8_u64.to_le_bytes()
            );
        });
        assert_eq!(allocations, 0);
    }
    for (wire, actual, x_byte, tag) in [
        (vec![1, 1, 9, 0, 2, 44], 6, 3, "short"),
        (vec![1, 1, 9, 0, 2, 44, 1], 6, 3, "malformed"),
        (vec![4; 64], 64, 3, "other-report"),
        (vec![], 0, u64::MAX, "invalid-layout"),
    ] {
        let bytes = input(&decode.input_type, &wire, actual, x_byte);
        let allocations = allocation::allocations(|| {
            assert!(
                validate_canonical_structured_value(decode_owner.evaluate(&bytes).unwrap())
                    .unwrap()
                    .variant_payload(tag)
                    .unwrap()
                    .is_some(),
                "{tag}"
            );
        });
        assert_eq!(allocations, 0);
    }
}
