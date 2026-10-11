//! Checked class policy tests, distinct from external-device/native evidence.
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
        expand_canonical_plot_for_authoring(&checked, &plot.name, &ProfileCatalog::new()).unwrap_or_else(|error| panic!("{}: {error:?}", plot.name));
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
