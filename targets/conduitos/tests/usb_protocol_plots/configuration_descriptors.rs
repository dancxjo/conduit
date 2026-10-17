//! Shared wire-prefix conformance, distinct from topology and device evidence.
use super::{descriptor_frame::frame, program_from};
use conduit_core::validate_canonical_structured_value;
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};

const SOURCE: &str = include_str!("../../plots/usb/configuration-descriptors.conduit");

fn run(entry: &str, wire: &[u8], actual: u64, expected: &str) -> Vec<u8> {
    let program = program_from(SOURCE, entry);
    let input = frame(&program.input_type, wire, actual);
    let ordinary = program.evaluate(&input).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let capacity = prepared.output_capacity();
    let output = prepared.evaluate(&input).unwrap().to_vec();
    assert_eq!(output, ordinary);
    assert_eq!(prepared.output_capacity(), capacity);
    assert!(
        validate_canonical_structured_value(&output)
            .unwrap()
            .variant_payload(expected)
            .unwrap()
            .is_some(),
        "{entry}: {expected}"
    );
    output
}
fn field(bytes: &[u8], tag: &str, name: &str, kind: &str) -> Vec<u8> {
    validate_canonical_structured_value(bytes)
        .unwrap()
        .variant_payload(tag)
        .unwrap()
        .unwrap()
        .record_field(name)
        .unwrap()
        .unwrap()
        .primitive_bytes(kind)
        .unwrap()
        .to_vec()
}

#[test]
fn configuration_header_preserves_advertised_geometry_and_power_wire_field() {
    let wire = [9, 2, 0, 1, 4, 7, 3, 0xe0, 50];
    for actual in [9, 256] {
        let output = run("usb-configuration-prefix", &wire, actual, "configuration");
        assert_eq!(
            field(&output, "configuration", "total_length", "value/u64"),
            256_u64.to_le_bytes()
        );
        for (name, expected) in [
            ("interfaces", 4),
            ("configuration_value", 7),
            ("string_index", 3),
            ("attributes", 0xe0),
            ("maximum_power_field", 50),
        ] {
            assert_eq!(
                field(&output, "configuration", name, "value/u8"),
                [expected]
            );
        }
    }
    for actual in 0..9 {
        run("usb-configuration-prefix", &wire, actual, "short");
    }
    for actual in [257, u64::MAX] {
        run("usb-configuration-prefix", &wire, actual, "oversized");
    }
    for total in [0_u16, 8, 257, u16::MAX] {
        let mut changed = wire;
        changed[2..4].copy_from_slice(&total.to_le_bytes());
        run(
            "usb-configuration-prefix",
            &changed,
            9,
            if total < 9 { "malformed" } else { "oversized" },
        );
    }
    for (index, value) in [(0, 8), (0, 10), (1, 4)] {
        let mut changed = wire;
        changed[index] = value;
        run("usb-configuration-prefix", &changed, 9, "malformed");
    }
}

#[test]
fn interface_prefix_keeps_alternate_setting_and_zero_endpoint_class_facts() {
    let wire = [9, 4, 11, 2, 0, 1, 2, 0, 6];
    let output = run("usb-interface-prefix", &wire, 9, "interface");
    for (name, expected) in [
        ("descriptor_length", 9),
        ("number", 11),
        ("alternate_setting", 2),
        ("endpoints", 0),
        ("class", 1),
        ("subclass", 2),
        ("protocol", 0),
        ("string_index", 6),
    ] {
        assert_eq!(field(&output, "interface", name, "value/u8"), [expected]);
    }
    for actual in 0..9 {
        run("usb-interface-prefix", &wire, actual, "short");
    }
    let mut extended = wire;
    extended[0] = 17;
    run("usb-interface-prefix", &extended, 9, "short");
    run("usb-interface-prefix", &extended, 17, "interface");
    for (index, value) in [(0, 8), (1, 2)] {
        let mut changed = wire;
        changed[index] = value;
        run("usb-interface-prefix", &changed, 9, "malformed");
    }
    run("usb-interface-prefix", &wire, 257, "oversized");
}

#[test]
fn endpoint_prefix_preserves_packet_bits_and_checks_extended_record_extent() {
    let wire = [7, 5, 0x81, 3, 0, 0x14, 4];
    let output = run("usb-endpoint-prefix", &wire, 7, "endpoint");
    for (name, expected) in [
        ("packet_field", 0x1400_u64),
        ("maximum_packet_size", 1024),
        ("additional_transactions_field", 2),
        ("transfer_type", 3),
    ] {
        assert_eq!(
            field(&output, "endpoint", name, "value/u64"),
            expected.to_le_bytes()
        );
    }
    assert_eq!(
        field(&output, "endpoint", "direction_in", "value/bool"),
        [1]
    );
    assert_eq!(field(&output, "endpoint", "address", "value/u8"), [0x81]);
    assert_eq!(
        field(&output, "endpoint", "interval_field", "value/u8"),
        [4]
    );
    for actual in 0..7 {
        run("usb-endpoint-prefix", &wire, actual, "short");
    }
    let mut extended = wire;
    extended[0] = 9;
    extended[2] = 2;
    extended[3] = 0x29;
    run("usb-endpoint-prefix", &extended, 7, "short");
    let output = run("usb-endpoint-prefix", &extended, 9, "endpoint");
    assert_eq!(field(&output, "endpoint", "attributes", "value/u8"), [0x29]);
    assert_eq!(
        field(&output, "endpoint", "direction_in", "value/bool"),
        [0]
    );
    for (index, value) in [(0, 6), (1, 4)] {
        let mut changed = wire;
        changed[index] = value;
        run("usb-endpoint-prefix", &changed, 7, "malformed");
    }
    run("usb-endpoint-prefix", &wire, u64::MAX, "oversized");
}

#[test]
fn descriptor_prepared_storage_reuses_one_shape_across_ten_thousand_records() {
    let program: PortableExpressionProgram = program_from(SOURCE, "usb-endpoint-prefix");
    let input = frame(&program.input_type, &[7, 5, 0x81, 3, 8, 0, 10], 7);
    let expected = program.evaluate(&input).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let capacity = prepared.output_capacity();
    for _ in 0..10_000 {
        assert_eq!(prepared.evaluate(&input).unwrap(), expected);
        assert_eq!(prepared.output_capacity(), capacity);
    }
}
