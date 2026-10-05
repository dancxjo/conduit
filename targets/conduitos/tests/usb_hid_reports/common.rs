use conduit_core::*;
use conduit_plot::*;

pub const SOURCE: &str = include_str!("../../plots/usb/hid-reports.conduit");

pub fn program(entry: &str) -> PortableExpressionProgram {
    let checked =
        check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("pure program");
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

pub fn request(
    program: &PortableExpressionProgram,
    wire: &[u8],
    actual: u64,
    zero_reserved: bool,
) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("request record");
    };
    let frame_type = fields
        .iter()
        .find(|field| field.name() == "frame")
        .unwrap()
        .value_type();
    let frame = crate::descriptor_frame::frame(frame_type, wire, actual);
    StructuredInfoValue::record(
        program.input_type.clone(),
        vec![
            StructuredFieldValue::new(
                "frame",
                StructuredInfoValue::from_canonical_bytes(&frame).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "zero_reserved",
                StructuredInfoValue::leaf(
                    fields
                        .iter()
                        .find(|field| field.name() == "zero_reserved")
                        .unwrap()
                        .value_type()
                        .clone(),
                    vec![u8::from(zero_reserved)],
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

pub fn tag(bytes: &[u8], expected: &str) {
    assert!(
        validate_canonical_structured_value(bytes)
            .unwrap()
            .variant_payload(expected)
            .unwrap()
            .is_some(),
        "expected {expected}"
    );
}
