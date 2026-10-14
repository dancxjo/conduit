//! Authored exact affine laws, explicit roles, and source-linked refusals.
use conduit_core::{Quantity, QuantityRole, StructuredInfoValue, StructuredInfoValueShape};
use conduit_plot::{quantity_conversion, *};
const DECLARATIONS: &str = include_str!("fixtures/affine_temperature.conduit");
fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    quantity_conversion::install(&mut startup, &mut profile).unwrap();
    (startup, profile)
}
fn source(value: &str, target: &str) -> String {
    format!("{DECLARATIONS}\nplot convert (\n receipt: ExactQuantityConversionReceipt <= 8192B >>\n) {{\n convert: units/convert(source = {value}, to = {target})\n convert.receipt >> receipt\n}}\n")
}
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
fn bytes(value: &StructuredInfoValue) -> &[u8] {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("leaf")
    };
    bytes
}
#[test]
fn celsius_kelvin_fahrenheit_laws_preserve_exact_points_and_differences() {
    let (startup, profile) = catalogs();
    for (value, target, expected, role) in [
        (
            "0celsiusPoint",
            "kelvinPoint",
            (27315_i128, -2_i16),
            QuantityRole::Point,
        ),
        (
            "32fahrenheitPoint",
            "celsiusPoint",
            (0, 0),
            QuantityRole::Point,
        ),
        (
            "30celsiusPoint",
            "fahrenheitPoint",
            (86, 0),
            QuantityRole::Point,
        ),
        (
            "ThermalDelta(9, fahrenheitPoint)",
            "kelvinPoint",
            (5, 0),
            QuantityRole::Delta,
        ),
        (
            "ThermalDelta(1, celsiusPoint)",
            "fahrenheitPoint",
            (18, -1),
            QuantityRole::Delta,
        ),
    ] {
        let source = source(value, target);
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &startup).unwrap();
        let authored = expand_canonical_plot_for_authoring(&checked, "convert", &profile).unwrap();
        let receipt =
            quantity_conversion::prepare_configuration(&authored.expanded.gears[0].configuration)
                .unwrap();
        quantity_conversion::validate_receipt(&receipt).unwrap();
        let quantity = Quantity::decode(bytes(field(&receipt, "source"))).unwrap();
        assert_eq!(quantity.role(), role);
        let StructuredInfoValueShape::Variant { tag, payload } = field(&receipt, "result").shape()
        else {
            panic!("result")
        };
        assert_eq!(tag, "converted");
        assert_eq!(
            i128::from_le_bytes(bytes(field(payload, "coefficient")).try_into().unwrap()),
            expected.0
        );
        assert_eq!(
            i16::from_le_bytes(bytes(field(payload, "exponent")).try_into().unwrap()),
            expected.1
        );
    }
}
#[test]
fn point_plus_point_and_point_delta_substitution_refuse_in_source() {
    let (startup, profile) = catalogs();
    for fragment in [
        "plot wrong (\n >> point: ThermalPoint\n result: ThermalPoint >>\n) = (. + .)\n",
        "plot wrong (\n value: ThermalDelta = 1celsiusPoint\n) {\n convert: units/convert(source = value, to = kelvinPoint)\n}.\n",
        "plot wrong (\n value: ThermalPoint = ThermalDelta(1, celsiusPoint)\n) {\n convert: units/convert(source = value, to = kelvinPoint)\n}.\n",
        "type OtherDelta = quantity { dimension: temperature }\ntype OtherPoint = quantity { point: OtherDelta }\nplot wrong (\n value: OtherPoint = 1celsiusPoint\n) {\n convert: units/convert(source = value, to = kelvinPoint)\n}.\n",
    ] {
        let source = format!("{DECLARATIONS}\n{fragment}");
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        match check_syntax_document(&syntax, &startup) {
            Err(error) => assert!(error.span.end > error.span.start),
            Ok(checked) => {
                let error =
                    expand_canonical_plot_for_authoring(&checked, "wrong", &profile).unwrap_err();
                assert!(error.message.contains("at "), "{:?}", error);
            }
        }
    }
}
#[test]
fn malformed_affine_laws_refuse_with_owned_source_spans() {
    let (startup, _) = catalogs();
    for altered in [
        DECLARATIONS.replace("scale: 5/9", "scale: 5/0"),
        DECLARATIONS.replace("point: ThermalDelta", "point: Distance"),
        DECLARATIONS.replace("quantity: ThermalDelta", "quantity: Distance"),
    ] {
        let syntax = parse_syntax_document(&altered);
        if syntax.diagnostics.is_empty() {
            let error = check_syntax_document(&syntax, &startup).unwrap_err();
            assert!(error.span.end > error.span.start);
        } else {
            assert!(syntax
                .diagnostics
                .iter()
                .all(|error| error.span.end > error.span.start));
        }
    }
}
