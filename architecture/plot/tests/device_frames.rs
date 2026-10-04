use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
mod common {
    pub mod allocation_probe;
}
#[global_allocator]
static ALLOCATOR: common::allocation_probe::Allocator = common::allocation_probe::Allocator;

fn program(name: &str) -> PortableExpressionProgram {
    let syntax = parse_syntax_document(include_str!(
        "../../../plots/device-protocols/frames.conduit"
    ));
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn field_type<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value_type()
}
fn record(
    ty: &StructuredInfoType,
    values: Vec<(&str, StructuredInfoValue)>,
) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty.clone(),
        values
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value).unwrap())
            .collect(),
    )
    .unwrap()
}
fn byte(ty: &StructuredInfoType, value: u8) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty.clone(), vec![value]).unwrap()
}
fn frame(ty: &StructuredInfoType, data: &[u8], length: u8) -> StructuredInfoValue {
    let bytes_type = field_type(ty, "bytes");
    let StructuredInfoTypeShape::Collection { element, .. } = bytes_type.shape() else {
        panic!("bytes")
    };
    let bytes = StructuredInfoValue::collection(
        bytes_type.clone(),
        data.iter().map(|value| byte(element, *value)).collect(),
    )
    .unwrap();
    record(
        ty,
        vec![
            ("bytes", bytes),
            ("length", byte(field_type(ty, "length"), length)),
        ],
    )
}
fn tag(bytes: &[u8]) -> String {
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("variant")
    };
    tag.into()
}
#[test]
fn concatenation_preserves_actual_bytes_and_excludes_padding() {
    let p = program("binary-frame-concat");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for left in 0..=4 {
        for right in 0..=4 {
            let input = record(
                &p.input_type,
                vec![
                    (
                        "left",
                        frame(field_type(&p.input_type, "left"), &[1, 2, 3, 4], left),
                    ),
                    (
                        "right",
                        frame(field_type(&p.input_type, "right"), &[5, 6, 7, 8], right),
                    ),
                ],
            )
            .canonical_bytes()
            .unwrap();
            let output = prepared.evaluate(&input).unwrap();
            assert_eq!(output, p.evaluate(&input).unwrap());
            let value = StructuredInfoValue::from_canonical_bytes(output).unwrap();
            let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
                panic!("variant")
            };
            assert_eq!(tag, "frame");
            let StructuredInfoValueShape::Record(fields) = payload.shape() else {
                panic!("frame")
            };
            let actual = fields
                .iter()
                .find(|f| f.name() == "length")
                .unwrap()
                .value();
            assert!(
                matches!(actual.shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == [left+right])
            );
            let data = fields.iter().find(|f| f.name() == "bytes").unwrap().value();
            let StructuredInfoValueShape::Collection(values) = data.shape() else {
                panic!("bytes")
            };
            let mut expected = [0_u8; 8];
            expected[..usize::from(left)].copy_from_slice(&[1, 2, 3, 4][..usize::from(left)]);
            expected[usize::from(left)..usize::from(left + right)]
                .copy_from_slice(&[5, 6, 7, 8][..usize::from(right)]);
            for (value, expected) in values.iter().zip(expected) {
                assert!(
                    matches!(value.shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == [expected])
                );
            }
        }
    }
    for length in [5, 255] {
        let input = record(
            &p.input_type,
            vec![
                (
                    "left",
                    frame(field_type(&p.input_type, "left"), &[1, 2, 3, 4], length),
                ),
                (
                    "right",
                    frame(field_type(&p.input_type, "right"), &[5, 6, 7, 8], 4),
                ),
            ],
        )
        .canonical_bytes()
        .unwrap();
        assert_eq!(tag(prepared.evaluate(&input).unwrap()), "malformed");
    }
}
#[test]
fn byte_access_distinguishes_short_from_malformed_and_never_reads_padding() {
    let p = program("binary-frame-byte");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for length in [0, 1, 8, 9, 255] {
        for index in 0..=255 {
            let input = record(
                &p.input_type,
                vec![
                    (
                        "frame",
                        frame(
                            field_type(&p.input_type, "frame"),
                            &[1, 2, 3, 4, 5, 6, 7, 8],
                            length,
                        ),
                    ),
                    ("index", byte(field_type(&p.input_type, "index"), index)),
                ],
            )
            .canonical_bytes()
            .unwrap();
            let output = prepared.evaluate(&input).unwrap();
            assert_eq!(output, p.evaluate(&input).unwrap());
            assert_eq!(
                tag(output),
                if length > 8 {
                    "malformed"
                } else if index >= length {
                    "short"
                } else {
                    "byte"
                }
            );
        }
    }
}

#[test]
fn repeated_frame_composition_uses_only_admitted_storage() {
    let p = program("binary-frame-concat");
    let input = record(
        &p.input_type,
        vec![
            (
                "left",
                frame(field_type(&p.input_type, "left"), &[1, 2, 3, 4], 3),
            ),
            (
                "right",
                frame(field_type(&p.input_type, "right"), &[5, 6, 7, 8], 2),
            ),
        ],
    )
    .canonical_bytes()
    .unwrap();
    let (mut prepared, preparation) =
        common::allocation_probe::observe(|| PreparedPortableExpressionEvaluator::new(&p).unwrap());
    assert!(preparation.peak_bytes < 2 * 1024 * 1024, "{preparation:?}");
    let (_, play) = common::allocation_probe::observe(|| {
        for _ in 0..10_000 {
            assert!(!prepared.evaluate(&input).unwrap().is_empty());
        }
    });
    assert_eq!(play.allocations, 0);
    assert_eq!(play.reallocations, 0);
}
