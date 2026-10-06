//! Endpoint framing retains physical ordinals without inventing report history.
use super::{allocation, common::*};
use conduit_core::*;
use conduit_plot::PreparedPortableExpressionEvaluator;

#[test]
fn endpoint_decode_retains_ordinals_for_valid_and_invalid_reports_without_allocating() {
    let decode = program("usb-hid-mouse-endpoint-command-message");
    let StructuredInfoTypeShape::Variant { cases, .. } = decode.input_type.shape() else {
        panic!("endpoint result")
    };
    let frame = cases
        .iter()
        .find(|case| case.tag() == "completed")
        .unwrap()
        .payload_type();
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&decode).unwrap();
    let maximum = [0_u8; 2048];
    for (ordinal, wire, actual, short, expected) in [
        (10_u64, &[0xff, 127, 129][..], 3_u64, true, "mouse"),
        (11, &[0, 0][..], 2, true, "short"),
        (12, &[0, 128, 0][..], 3, true, "invalid-motion"),
        (13, &[0, 0, 128][..], 3, true, "invalid-motion"),
        (14, &[0, 0, 0][..], 2, true, "malformed"),
        (15, &[0, 0, 0][..], 3, false, "malformed"),
        (16, &maximum[..], 2048, false, "malformed"),
        (u64::MAX, &[0, 0, 0, 0, 0, 0, 0, 0][..], 8, false, "mouse"),
    ] {
        let input = StructuredInfoValue::variant(
            decode.input_type.clone(),
            "completed",
            StructuredInfoValue::record(
                frame.clone(),
                vec![
                    leaf_field(frame, "ordinal", &ordinal.to_le_bytes()),
                    leaf_field(frame, "actual", &actual.to_le_bytes()),
                    leaf_field(frame, "short", &[u8::from(short)]),
                    leaf_field(frame, "wire", wire),
                ],
            )
            .unwrap(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        assert!(input.len() <= 4096);
        let allocations = allocation::allocations(|| {
            let bytes = evaluator.evaluate(&input).unwrap();
            assert!(bytes.len() <= 4096);
            let message = validate_canonical_structured_value(bytes).unwrap();
            let command = message.variant_payload("command").unwrap().unwrap();
            let observation = command.variant_payload("observation").unwrap().unwrap();
            assert_eq!(
                observation
                    .record_field("ordinal")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                ordinal.to_le_bytes()
            );
            let result = observation.record_field("observed").unwrap().unwrap();
            let motion = result.variant_payload(expected).unwrap().unwrap();
            if expected == "mouse" {
                let (buttons, x, y) = if ordinal == 10 {
                    (7_u8, 127_i16, -127_i16)
                } else {
                    (0, 0, 0)
                };
                assert_eq!(
                    motion
                        .record_field("buttons")
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u8")
                        .unwrap(),
                    [buttons]
                );
                for (name, expected) in [("x", x), ("y", y)] {
                    assert_eq!(
                        motion
                            .record_field(name)
                            .unwrap()
                            .unwrap()
                            .primitive_bytes("value/i16")
                            .unwrap(),
                        expected.to_le_bytes()
                    );
                }
            }
        });
        assert_eq!(allocations, 0);
    }
    // Decode retains MAX as wire truth; the ordering lifecycle refuses its
    // exhausted ordinal rather than fabricating a replacement ordinal here.
    for tag in ["stalled", "provider-lost", "timeout", "unsupported"] {
        let input = StructuredInfoValue::variant(
            decode.input_type.clone(),
            tag,
            StructuredInfoValue::leaf(
                StructuredInfoType::leaf(kind_id("value/unit")).unwrap(),
                vec![],
            )
            .unwrap(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let allocations = allocation::allocations(|| {
            let value =
                validate_canonical_structured_value(evaluator.evaluate(&input).unwrap()).unwrap();
            assert!(
                value
                    .variant_payload("command")
                    .unwrap()
                    .unwrap()
                    .variant_payload(tag)
                    .unwrap()
                    .is_some()
            );
        });
        assert_eq!(allocations, 0);
    }
}
