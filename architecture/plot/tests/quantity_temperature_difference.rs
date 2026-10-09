use conduit_core::{
    ConfigurationEntry, ConfigurationValue, StructuredFieldValue, StructuredInfoValue,
    StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    quantity_conversion::{self, temperature_difference as difference},
    ProfileCatalog, StartupCatalog,
};

fn configuration(source: &str, target: &str) -> [ConfigurationEntry; 2] {
    [("source", source), ("to", target)].map(|(key, value)| ConfigurationEntry {
        key: key.into(),
        value: ConfigurationValue::Text(value.into()),
    })
}
fn field<'a>(record: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = record.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}
fn bytes(value: &StructuredInfoValue) -> &[u8] {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("leaf")
    };
    bytes
}

#[test]
fn authored_difference_conversion_has_distinct_types_zero_offsets_and_preserved_source() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    quantity_conversion::install(&mut startup, &mut profile).unwrap();
    for (original, target, coefficient, exponent) in [
        ("9°F", "K", 5_i128, 0_i16),
        ("1m°C", "K", 1, -3),
        ("9m°F", "mK", 5, 0),
        ("1QK", "qK", 1, 60),
    ] {
        let source = format!("# µ before original source\nplot difference (\n receipt: ExactTemperatureDifferenceConversionReceipt <= 8192B >>\n) {{\n converted: units/convert-temperature-difference(source = \"{original}\", to = \"{target}\")\n converted.receipt >> receipt\n}}.\n");
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty());
        assert_eq!(syntax.round_trip(), source);
        let checked = check_syntax_document(&syntax, &startup).unwrap();
        quantity_conversion::validate_source(&syntax, &checked).unwrap();
        let expanded =
            expand_canonical_plot_for_authoring(&checked, "difference", &profile).unwrap();
        let gear = &expanded.expanded.gears[0];
        let receipt = difference::prepare_configuration(&gear.configuration).unwrap();
        difference::validate_receipt(&receipt).unwrap();
        assert!(quantity_conversion::validate_receipt(&receipt).is_err());
        assert_ne!(receipt.value_type(), &quantity_conversion::receipt_type());
        assert_eq!(
            field(&receipt, "source").value_type(),
            &difference::source_type()
        );
        assert!(difference::validate_source_value(field(&receipt, "source")).is_ok());
        assert_eq!(bytes(field(&receipt, "original")), original.as_bytes());
        assert_eq!(
            bytes(field(&receipt, "profile")),
            conduit_core::EXACT_TEMPERATURE_DIFFERENCE_INFO_ID.as_bytes()
        );
        for name in ["source-offset", "target-offset"] {
            assert_eq!(
                i128::from_le_bytes(bytes(field(&receipt, name)).try_into().unwrap()),
                0
            );
        }
        let StructuredInfoValueShape::Variant { tag, payload } = field(&receipt, "result").shape()
        else {
            panic!("result")
        };
        assert_eq!(tag, "converted");
        assert_eq!(
            i128::from_le_bytes(bytes(field(payload, "coefficient")).try_into().unwrap()),
            coefficient
        );
        assert_eq!(
            i16::from_le_bytes(bytes(field(payload, "exponent")).try_into().unwrap()),
            exponent
        );
        assert_eq!(
            gear.outputs[0].value_kind,
            *receipt.value_type().profile().unwrap().value_kind()
        );
        let point = quantity_conversion::prepare_configuration(&gear.configuration).unwrap();
        assert!(difference::validate_receipt(&point).is_err());
    }
    let source = "plot bad (\n receipt: ExactQuantityConversionReceipt <= 8192B >>\n) {\n converted: units/convert-temperature-difference(source = \"9°F\", to = \"K\")\n converted.receipt >> receipt\n}.\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    // Kind signatures own startup values; canonical expansion checks exact port Types.
    let error = expand_canonical_plot_for_authoring(&checked, "bad", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-045");
}

#[test]
fn difference_refusals_keep_precision_and_original_diagnostic_spans() {
    let receipt = difference::prepare_configuration(&configuration("1°F", "K")).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = field(&receipt, "result").shape()
    else {
        panic!("result")
    };
    assert_eq!(tag, "refused");
    assert_eq!(bytes(payload), b"inexact");
    let receipt = difference::prepare_configuration(&configuration("1°C", "m")).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = field(&receipt, "result").shape()
    else {
        panic!("result")
    };
    assert_eq!(tag, "refused");
    assert_eq!(bytes(payload), b"incompatible-dimensions");
    let mut startup = StartupCatalog::new();
    quantity_conversion::install(&mut startup, &mut ProfileCatalog::new()).unwrap();
    for (original, target, offending) in [
        ("1Hz", "K", "1Hz"),
        ("1°C", "mkg", "mkg"),
        ("21C", "K", "21C"),
    ] {
        let source = format!("# µ original\nplot invalid {{\n original = \"{original}\"\n alias = original\n converted: units/convert-temperature-difference(source = alias, to = \"{target}\")\n}}\n");
        let syntax = parse_syntax_document(&source);
        let checked = check_syntax_document(&syntax, &startup).unwrap();
        let error = quantity_conversion::validate_source(&syntax, &checked).unwrap_err();
        assert_eq!(&source[error.span.start..error.span.end], offending);
    }
}

#[test]
fn difference_receipt_readmission_rejects_forged_affine_offset_and_point_source_type() {
    let receipt = difference::prepare_configuration(&configuration("9°F", "°F")).unwrap();
    let point = quantity_conversion::prepare_configuration(&configuration("9°F", "°F")).unwrap();
    let StructuredInfoValueShape::Record(fields) = receipt.shape() else {
        panic!("record")
    };
    for name in ["source-offset", "target-offset"] {
        let fields = fields
            .iter()
            .map(|entry| {
                if entry.name() == name {
                    StructuredFieldValue::new(name, field(&point, name).clone()).unwrap()
                } else {
                    entry.clone()
                }
            })
            .collect();
        let forged = StructuredInfoValue::record(difference::receipt_type(), fields).unwrap();
        assert!(difference::validate_receipt(&forged).is_err());
    }
    let fields = fields
        .iter()
        .map(|entry| {
            if entry.name() == "source" {
                StructuredFieldValue::new("source", field(&point, "source").clone()).unwrap()
            } else {
                entry.clone()
            }
        })
        .collect();
    assert!(StructuredInfoValue::record(difference::receipt_type(), fields).is_err());
    let invalid_coordinate = conduit_core::ExactDecimalQuantity::parse_plot_literal("1Hz").unwrap();
    let invalid_source = StructuredInfoValue::record(
        difference::source_type(),
        vec![StructuredFieldValue::new(
            "coordinate",
            StructuredInfoValue::leaf(
                conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
                    conduit_core::EXACT_DECIMAL_QUANTITY_INFO_ID,
                ))
                .unwrap(),
                invalid_coordinate.encode().to_vec(),
            )
            .unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    assert!(difference::validate_source_value(&invalid_source).is_err());
    let fields = match receipt.shape() {
        StructuredInfoValueShape::Record(fields) => fields,
        _ => unreachable!(),
    };
    let fields = fields
        .iter()
        .map(|entry| {
            if entry.name() == "source" {
                StructuredFieldValue::new("source", invalid_source.clone()).unwrap()
            } else {
                entry.clone()
            }
        })
        .collect();
    let forged = StructuredInfoValue::record(difference::receipt_type(), fields).unwrap();
    assert!(difference::validate_receipt(&forged).is_err());
}
