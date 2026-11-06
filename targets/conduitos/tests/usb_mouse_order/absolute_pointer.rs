//! Absolute HID meaning feeds the same normalized mouse pointer contract.
use super::{allocation, common::*};
use conduit_core::*;
fn input(
    ty: &StructuredInfoType,
    position: [i64; 2],
    sequence: u64,
    point: [u16; 4],
    pressed: bool,
    capacity: u64,
) -> Vec<u8> {
    let state = field_type(ty, "state");
    StructuredInfoValue::record(
        ty.clone(),
        vec![
            StructuredFieldValue::new(
                "state",
                StructuredInfoValue::record(
                    state.clone(),
                    vec![
                        leaf_field(&state, "position-x", &position[0].to_le_bytes()),
                        leaf_field(&state, "position-y", &position[1].to_le_bytes()),
                        leaf_field(&state, "sequence", &sequence.to_le_bytes()),
                    ],
                )
                .unwrap(),
            )
            .unwrap(),
            leaf_field(ty, "ordinal", &17_u64.to_le_bytes()),
            leaf_field(ty, "x", &point[0].to_le_bytes()),
            leaf_field(ty, "y", &point[1].to_le_bytes()),
            leaf_field(ty, "maximum-x", &point[2].to_le_bytes()),
            leaf_field(ty, "maximum-y", &point[3].to_le_bytes()),
            leaf_field(ty, "primary-pressed", &[u8::from(pressed)]),
            leaf_field(ty, "queue-capacity", &capacity.to_le_bytes()),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
#[test]
fn absolute_hid_reuses_mouse_output_and_normalizes_full_unsigned_axes_without_allocations() {
    let [absolute, mouse] = programs([
        "usb-hid-absolute-pointer-advance",
        "usb-hid-mouse-pointer-advance",
    ]);
    assert_eq!(absolute.output_type, mouse.output_type);
    let mut evaluator = conduit_plot::PreparedPortableExpressionEvaluator::new(&absolute).unwrap();
    for (point, expected, pressed) in [
        ([0, 65535, 65535, 65535], [0_i64, 1000000_i64], true),
        ([65535, 0, 65535, 65535], [1000000, 0], false),
        ([500, 300, 1000, 600], [500000, 500000], true),
    ] {
        let bytes = input(&absolute.input_type, [500000; 2], 8, point, pressed, 8);
        let allocations = allocation::allocations(|| {
            let value =
                validate_canonical_structured_value(evaluator.evaluate(&bytes).unwrap()).unwrap();
            let ready = value.variant_payload("ready").unwrap().unwrap();
            let sample = ready.record_field("sample").unwrap().unwrap();
            let state = ready.record_field("state").unwrap().unwrap();
            for (field, coordinate) in [("position-x", expected[0]), ("position-y", expected[1])] {
                assert_eq!(
                    state
                        .record_field(field)
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/i64")
                        .unwrap(),
                    coordinate.to_le_bytes()
                );
            }
            for (field, expected) in [
                ("position-x", expected[0]),
                ("position-y", expected[1]),
                ("delta-x", expected[0] - 500000),
                ("delta-y", expected[1] - 500000),
            ] {
                assert_eq!(
                    sample
                        .record_field(field)
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/i64")
                        .unwrap(),
                    expected.to_le_bytes()
                );
            }
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
                9_u64.to_le_bytes()
            );
            assert_eq!(
                ready
                    .record_field("ordinal")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                17_u64.to_le_bytes()
            );
        });
        assert_eq!(allocations, 0);
    }
}
#[test]
fn absolute_hid_refuses_invalid_bounds_state_and_sequence_without_clamping() {
    let absolute = program("usb-hid-absolute-pointer-advance");
    let mut evaluator = conduit_plot::PreparedPortableExpressionEvaluator::new(&absolute).unwrap();
    for (position, sequence, point, capacity, tag) in [
        ([500000; 2], 0, [0, 1, 0, 100], 8, "invalid-motion"),
        ([500000; 2], 0, [101, 0, 100, 100], 8, "invalid-motion"),
        ([-1, 0], 0, [0, 0, 100, 100], 8, "invalid-state"),
        ([0; 2], 0, [0, 0, 100, 100], 0, "invalid-state"),
        ([0; 2], u64::MAX, [0, 0, 100, 100], 8, "sequence-exhausted"),
    ] {
        let bytes = input(
            &absolute.input_type,
            position,
            sequence,
            point,
            false,
            capacity,
        );
        assert!(
            validate_canonical_structured_value(evaluator.evaluate(&bytes).unwrap())
                .unwrap()
                .variant_payload(tag)
                .unwrap()
                .is_some()
        );
    }
}
