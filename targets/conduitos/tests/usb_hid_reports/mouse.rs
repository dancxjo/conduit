use super::common::*;
use conduit_core::validate_canonical_structured_value;
use conduit_plot::PreparedPortableExpressionEvaluator;

#[test]
fn mouse_prefix_preserves_signed_motion_and_uninterpreted_extensions() {
    let p = program("usb-hid-mouse-frame");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for octet in 0_u8..=255 {
        let input = request(&p, &[0xff, octet, 255, 0xab], 4, false);
        let ordinary = p.evaluate(&input).unwrap();
        let result = prepared.evaluate(&input).unwrap();
        assert_eq!(result, ordinary);
        if octet == 128 {
            tag(result, "invalid-motion");
            continue;
        }
        let value = validate_canonical_structured_value(result)
            .unwrap()
            .variant_payload("mouse")
            .unwrap()
            .unwrap();
        assert_eq!(
            value
                .record_field("buttons")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u8")
                .unwrap(),
            [7]
        );
        assert_eq!(
            value
                .record_field("x")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/i16")
                .unwrap(),
            i16::from(octet as i8).to_le_bytes()
        );
        assert_eq!(
            value
                .record_field("y")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/i16")
                .unwrap(),
            (-1_i16).to_le_bytes()
        );
        assert_eq!(
            value
                .record_field("wire")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bytes")
                .unwrap()
                .len(),
            4
        );
    }
}

#[test]
fn mouse_extent_and_explicit_strict_profile_refuse_without_reading_padding() {
    let p = program("usb-hid-mouse-frame");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for (wire, actual, strict, expected) in [
        (&[][..], 0, false, "short"),
        (&[0; 9][..], 9, false, "malformed"),
        (&[0; 1025][..], 1025, false, "malformed"),
        (&[0; 2048][..], 2048, false, "malformed"),
        (&[0, 0][..], 2, false, "short"),
        (&[0, 0, 0][..], 2, false, "malformed"),
        (&[0xf8, 0, 0][..], 3, true, "reserved"),
        (&[0xf8, 0, 0][..], 3, false, "mouse"),
        (&[0, 0, 128][..], 3, false, "invalid-motion"),
        (&[7, 0, 0, 1, 2, 3, 4, 5][..], 8, false, "mouse"),
    ] {
        let input = request(&p, wire, actual, strict);
        assert!(
            input.len() <= 4096,
            "packed report exceeds the admitted frame surface"
        );
        tag(prepared.evaluate(&input).unwrap(), expected);
    }
}
