use conduit_core::ConfigurationValue;
use conduit_plot::{
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
};

const SOURCE: &str = include_str!("../plots/usb/protocol.conduit");

#[path = "usb_protocol_plots/descriptors.rs"]
mod descriptors;

#[path = "usb_protocol_plots/device_probe.rs"]
mod device_probe;

#[path = "usb_protocol_plots/device_probe_execution.rs"]
mod device_probe_execution;

fn program(entry: &str) -> PortableExpressionProgram {
    program_from(SOURCE, entry)
}

fn program_from(source: &str, entry: &str) -> PortableExpressionProgram {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 1, "{entry}");
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("expected checked pure plot program");
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

#[test]
fn every_usb_wire_plot_checks_and_expands() {
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    for plot in &checked.plots {
        expand_canonical_plot_for_authoring(&checked, &plot.name, &ProfileCatalog::new()).unwrap();
    }
}

#[test]
fn endpoint_identity_is_wire_arithmetic_not_authority() {
    let p = program("usb-endpoint-dci");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for number in 1_u8..=15 {
        assert_eq!(
            evaluator
                .evaluate(&u64::from(number).to_le_bytes())
                .unwrap(),
            &u64::from(number * 2).to_le_bytes()
        );
        assert_eq!(
            evaluator
                .evaluate(&u64::from(number | 128).to_le_bytes())
                .unwrap(),
            &u64::from(number * 2 + 1).to_le_bytes()
        );
    }
}

#[test]
fn octet_extraction_and_endpoint_validation_do_not_accept_hidden_high_bits() {
    let p = program("usb-word-high");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    assert_eq!(
        evaluator
            .evaluate(&0xfeed_cafe_1234_5678_u64.to_le_bytes())
            .unwrap(),
        &0x56_u64.to_le_bytes()
    );
    let p = program("usb-endpoint-address-valid");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for address in [0_u64, 16, 0x101, u64::MAX] {
        assert_eq!(evaluator.evaluate(&address.to_le_bytes()).unwrap(), &[0]);
    }
    assert_eq!(evaluator.evaluate(&0x81_u64.to_le_bytes()).unwrap(), &[1]);
}

#[test]
fn boot_protocol_request_keeps_interface_in_windex() {
    let p = program("usb-hid-select-boot");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for interface in 0_u8..=255 {
        assert_eq!(
            evaluator.evaluate(&[interface]).unwrap(),
            &[0x21, 11, 0, 0, interface, 0, 0, 0]
        );
    }
}

fn record_input(p: &PortableExpressionProgram, fields: &[(&str, u64)]) -> Vec<u8> {
    use conduit_core::{StructuredFieldValue, StructuredInfoValue, kind_id};
    StructuredInfoValue::record(
        p.input_type.clone(),
        fields
            .iter()
            .map(|(name, value)| {
                StructuredFieldValue::new(
                    *name,
                    StructuredInfoValue::leaf(
                        conduit_core::StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
                        value.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

#[test]
fn pcm_fractional_cadence_delivers_exact_rate_without_lifetime_growth() {
    let p = program("usb-pcm-packet-bytes");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let capacity = evaluator.output_capacity();
    for rate in [8000_u64, 44100, 48000] {
        let mut total_bytes = 0_u64;
        for phase in 0..1000 {
            let input = record_input(
                &p,
                &[
                    ("phase", phase),
                    ("sample_rate", rate),
                    ("channels", 2),
                    ("sample_bytes", 2),
                ],
            );
            let bytes = u64::from_le_bytes(evaluator.evaluate(&input).unwrap().try_into().unwrap());
            assert!(bytes <= 192);
            total_bytes += bytes;
            assert_eq!(evaluator.output_capacity(), capacity);
        }
        assert_eq!(total_bytes, rate * 4);
    }
}

#[test]
fn reusable_ring_cycles_preserve_geometry_beyond_initial_capacity() {
    let p = program("usb-ring-cycle");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let capacity = evaluator.output_capacity();
    for sequence in 0..10000 {
        let input = record_input(&p, &[("sequence", sequence), ("slots", 63), ("buffers", 8)]);
        assert_eq!(
            evaluator.evaluate(&input).unwrap(),
            &(1_u64 ^ ((sequence / 63) & 1)).to_le_bytes()
        );
        assert_eq!(evaluator.output_capacity(), capacity);
    }
}

#[test]
fn class_requests_have_exact_wire_bytes() {
    for (entry, index, expected) in [
        (
            "usb-audio-frequency-control",
            1_u8,
            [0x22, 1, 0, 1, 1, 0, 3, 0],
        ),
        (
            "usb-ecm-directed-broadcast",
            7,
            [0x21, 0x43, 12, 0, 7, 0, 0, 0],
        ),
    ] {
        let p = program(entry);
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        assert_eq!(evaluator.evaluate(&index.to_le_bytes()).unwrap(), expected);
    }
}

#[test]
fn mouse_octets_preserve_signed_edges() {
    let p = program("usb-hid-signed-delta");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for input in 0_i16..=255 {
        let expected = if input < 128 { input } else { input - 256 };
        assert_eq!(
            evaluator.evaluate(&input.to_le_bytes()).unwrap(),
            &expected.to_le_bytes()
        );
    }
}

#[test]
fn each_completion_identity_must_match_and_attachment_must_be_current() {
    let p = program("usb-completion-current");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let original = [
        ("expected_slot", 1),
        ("observed_slot", 1),
        ("expected_endpoint", 3),
        ("observed_endpoint", 3),
        ("expected_descriptor", 4096),
        ("observed_descriptor", 4096),
        ("expected_attachment", 7),
        ("observed_attachment", 7),
    ];
    assert_eq!(
        evaluator.evaluate(&record_input(&p, &original)).unwrap(),
        &[1]
    );
    for field in [1, 3, 5, 7] {
        let mut changed = original;
        changed[field].1 += 1;
        assert_eq!(
            evaluator.evaluate(&record_input(&p, &changed)).unwrap(),
            &[0]
        );
    }
    let mut absent = original;
    absent[6].1 = 0;
    absent[7].1 = 0;
    assert_eq!(
        evaluator.evaluate(&record_input(&p, &absent)).unwrap(),
        &[0]
    );
}

#[test]
fn class_recognition_is_specific_and_does_not_publish_offers() {
    for (entry, fields) in [
        ("usb-hid-keyboard-interface", 0x010103_u64),
        ("usb-hid-mouse-interface", 0x020103),
        ("usb-audio-streaming-interface", 0x000201),
        ("usb-cdc-ecm-interface", 0x000602),
        ("usb-storage-bulk-only-interface", 0x500608),
    ] {
        let p = program(entry);
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        assert_eq!(evaluator.evaluate(&fields.to_le_bytes()).unwrap(), &[1]);
        assert_eq!(
            evaluator.evaluate(&(fields ^ 1).to_le_bytes()).unwrap(),
            &[0]
        );
    }
}
