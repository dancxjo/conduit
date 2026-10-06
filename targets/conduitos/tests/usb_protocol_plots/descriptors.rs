//! Deterministic descriptor framing; neither enumeration nor device proof.
use super::{descriptor_frame::frame, program_from, record_input};
use conduit_core::validate_canonical_structured_value;
use conduit_plot::{
    PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog, check_syntax_document,
    expand_canonical_plot_for_authoring, parse_syntax_document,
};

const SOURCE: &str = include_str!("../../plots/usb/descriptors.conduit");

#[test]
fn descriptor_sources_check_and_expand() {
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    for plot in &checked.plots {
        expand_canonical_plot_for_authoring(&checked, &plot.name, &ProfileCatalog::new()).unwrap();
    }
}

fn tag(bytes: &[u8], expected: &str) {
    assert!(
        validate_canonical_structured_value(bytes)
            .unwrap()
            .variant_payload(expected)
            .unwrap()
            .is_some(),
        "expected {expected}"
    );
}

#[test]
fn cursor_distinguishes_end_truncation_malformed_and_storage_overflow() {
    let p = program_from(SOURCE, "usb-descriptor-advance");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for (offset, available, length, descriptor_type, expected) in [
        (0, 0, 0, 0, "end"),
        (256, 256, u64::MAX, u64::MAX, "end"),
        (0, 1, 18, 1, "short"),
        (0, 17, 18, 1, "short"),
        (0, 18, 0, 1, "malformed"),
        (0, 18, 1, 1, "malformed"),
        (0, 18, 256, 1, "malformed"),
        (0, 18, 18, 256, "malformed"),
        (19, 18, 18, 1, "malformed"),
        (u64::MAX, 256, 2, 1, "malformed"),
        (0, u64::MAX, 18, 1, "oversized"),
        (0, 257, 18, 1, "oversized"),
        (0, 18, 18, 1, "descriptor"),
        (254, 256, 2, 0xff, "descriptor"),
    ] {
        let input = record_input(
            &p,
            &[
                ("offset", offset),
                ("available", available),
                ("length", length),
                ("descriptor_type", descriptor_type),
            ],
        );
        let ordinary = p.evaluate(&input).unwrap();
        let output = prepared.evaluate(&input).unwrap();
        assert_eq!(output, ordinary);
        tag(output, expected);
        if expected == "descriptor" {
            let value = validate_canonical_structured_value(output)
                .unwrap()
                .variant_payload("descriptor")
                .unwrap()
                .unwrap();
            assert_eq!(
                value
                    .record_field("next")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                (offset + length).to_le_bytes()
            );
        }
    }
}

#[test]
fn device_decode_preserves_fields_and_cannot_read_padding_as_received_data() {
    let p = program_from(SOURCE, "usb-device-descriptor");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let wire = [
        18, 1, 0x10, 0x03, 0, 0, 0, 9, 0x34, 0x12, 0xcd, 0xab, 0xff, 0x01, 1, 2, 3, 1,
    ];
    for actual in 0..18 {
        tag(
            prepared
                .evaluate(&frame(&p.input_type, &wire, actual))
                .unwrap(),
            "short",
        );
    }
    for actual in [19, u64::MAX] {
        tag(
            prepared
                .evaluate(&frame(&p.input_type, &wire, actual))
                .unwrap(),
            "malformed",
        );
    }
    for (index, value) in [(0, 0), (0, 17), (0, 19), (1, 2)] {
        let mut changed = wire;
        changed[index] = value;
        tag(
            prepared
                .evaluate(&frame(&p.input_type, &changed, 18))
                .unwrap(),
            "malformed",
        );
    }
    let input = frame(&p.input_type, &wire, 18);
    let expected = p.evaluate(&input).unwrap();
    assert!(prepared.evaluate(&input[..input.len() - 1]).is_err());
    let mut trailing = input.clone();
    trailing.push(0);
    assert!(prepared.evaluate(&trailing).is_err());
    let capacity = prepared.output_capacity();
    let allocations = super::allocation::allocations(|| {
        for _ in 0..10_000 {
            assert_eq!(prepared.evaluate(&input).unwrap(), expected);
            assert_eq!(prepared.output_capacity(), capacity);
        }
    });
    assert_eq!(allocations, 0, "prepared descriptor decoding must reuse admitted storage");
    let device = validate_canonical_structured_value(&expected)
        .unwrap()
        .variant_payload("device")
        .unwrap()
        .unwrap();
    for (name, value) in [
        ("usb_version", 0x0310_u64),
        ("vendor_id", 0x1234),
        ("product_id", 0xabcd),
        ("device_version", 0x01ff),
    ] {
        assert_eq!(
            device
                .record_field(name)
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            value.to_le_bytes()
        );
    }
    assert_eq!(
        device
            .record_field("ep0_packet_field")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u8")
            .unwrap(),
        [9]
    );
}
