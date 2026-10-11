#[path = "../../../architecture/plot/tests/prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_core::{ConfigurationValue, validate_canonical_structured_value};
use conduit_plot::{
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
};
fn source() -> String {
    let audio = include_str!("../../../semantics/audio/types.conduit");
    let start = audio.find("type PcmSampleRepresentation").unwrap();
    let end = audio.find("type AudioRenderDemand").unwrap();
    let usb = include_str!("../plots/usb/protocol.conduit");
    let cadence_start = usb.find("type UsbPcmCadence").unwrap();
    let cadence_end = usb.find("# A stale DMA completion").unwrap();
    [
        &audio[start..end],
        &usb[cadence_start..cadence_end],
        include_str!("../plots/usb/control-types.conduit"),
        include_str!("../plots/usb/isochronous-out-types.conduit"),
        include_str!("../plots/usb/audio-types.conduit"),
        include_str!("../plots/usb/audio-profile.conduit"),
    ]
    .join("\n")
}
fn program(entry: &str) -> PortableExpressionProgram {
    let text = source();
    let syntax = parse_syntax_document(&text);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("pure checked program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
#[test]
fn every_audio_policy_plot_checks_and_expands() {
    let text = source();
    let syntax = parse_syntax_document(&text);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    for plot in &checked.plots {
        expand_canonical_plot_for_authoring(&checked, &plot.name, &ProfileCatalog::new())
            .unwrap_or_else(|error| panic!("{}: {error:?}", plot.name));
    }
}
#[test]
fn class_control_words_are_exact_usb_wire_not_native_policy() {
    for (entry, expected, count) in [
        (
            "usb-audio-configuration-request",
            [0, 9, 1, 0, 0, 0, 0, 0],
            0,
        ),
        ("usb-audio-alternate-request", [1, 11, 1, 0, 1, 0, 0, 0], 0),
        ("usb-audio-set-rate-request", [0x22, 1, 0, 1, 1, 0, 3, 0], 3),
        (
            "usb-audio-get-rate-request",
            [0xa2, 0x81, 0, 1, 1, 0, 3, 0],
            0,
        ),
    ] {
        let p = program(entry);
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        let value = validate_canonical_structured_value(evaluator.evaluate(&[1]).unwrap()).unwrap();
        assert_eq!(
            value
                .record_field("setup")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            expected
        );
        assert_eq!(
            value
                .record_field("output")
                .unwrap()
                .unwrap()
                .collection_length()
                .unwrap(),
            count
        );
    }
}

fn completed(
    program: &PortableExpressionProgram,
    bytes: &[u8],
    transferred: u16,
    short: bool,
) -> Vec<u8> {
    use conduit_core::{
        StructuredFieldValue, StructuredInfoTypeShape as Shape, StructuredInfoValue as Value,
    };
    let Shape::Variant { cases, .. } = program.input_type.shape() else {
        panic!("completion variant")
    };
    let payload = cases
        .iter()
        .find(|case| case.tag() == "completed")
        .unwrap()
        .payload_type();
    let Shape::Record { fields, .. } = payload.shape() else {
        panic!("completed record")
    };
    let values = fields
        .iter()
        .map(|field| {
            let value = match field.name() {
                "transferred" => Value::leaf(
                    field.value_type().clone(),
                    transferred.to_le_bytes().to_vec(),
                )
                .unwrap(),
                "short" => Value::leaf(field.value_type().clone(), vec![u8::from(short)]).unwrap(),
                "input" => {
                    let Shape::Sequence { element, .. } = field.value_type().shape() else {
                        panic!("bytes sequence")
                    };
                    Value::sequence(
                        field.value_type().clone(),
                        bytes
                            .iter()
                            .map(|byte| Value::leaf(element.clone(), vec![*byte]).unwrap())
                            .collect(),
                    )
                    .unwrap()
                }
                _ => panic!("unknown completion field"),
            };
            StructuredFieldValue::new(field.name(), value).unwrap()
        })
        .collect();
    let payload = Value::record(payload.clone(), values).unwrap();
    Value::variant(program.input_type.clone(), "completed", payload)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
#[test]
fn class_rate_negotiation_refuses_rounding_short_and_extent_lies() {
    let p = program("usb-audio-get-rate-ack");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for (bytes, transferred, short, accepted) in [
        (&[128, 187, 0][..], 3, false, true),
        (&[68, 172, 0][..], 3, false, false),
        (&[128, 187][..], 2, true, false),
        (&[128, 187, 0][..], 3, true, false),
        (&[128, 187, 0][..], 2, false, false),
    ] {
        assert_eq!(
            evaluator
                .evaluate(&completed(&p, bytes, transferred, short))
                .unwrap(),
            [u8::from(accepted)]
        );
    }
}
#[test]
fn descriptor_policy_rejects_changed_format_rate_alt_and_endpoint() {
    // Exact external device fixture descriptor; this is a synthetic policy test.
    let descriptor = bytes_from_hex(include_str!("../plots/usb/proof/audio/configuration.hex"));
    let p = program("usb-audio-profile-supported");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    assert_eq!(
        evaluator
            .evaluate(&completed(&p, &descriptor, 100, false))
            .unwrap(),
        [1]
    );
    for index in [4, 5, 12, 59, 62, 77, 78, 79, 81, 86, 87, 88, 90, 96] {
        let mut changed = descriptor.clone();
        changed[index] ^= 1;
        assert_eq!(
            evaluator
                .evaluate(&completed(&p, &changed, 100, false))
                .unwrap(),
            [0],
            "descriptor byte{index}"
        );
    }
    assert_eq!(
        evaluator
            .evaluate(&completed(&p, &descriptor[..99], 99, true))
            .unwrap(),
        [0]
    );
}
fn bytes_from_hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect()
}

#[test]
fn repeated_checked_rate_observation_allocates_nothing() {
    let p = program("usb-audio-get-rate-ack");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let input = completed(&p, &[128, 187, 0], 3, false);
    assert_eq!(
        allocation::allocations(|| {
            for _ in 0..1024 {
                assert_eq!(evaluator.evaluate(&input).unwrap(), [1]);
            }
        }),
        0
    );
}
