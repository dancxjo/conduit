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
        value: if key == "to" {
            ConfigurationValue::Unit(conduit_core::UnitConfigurationValue::parse(value).unwrap())
        } else {
            ConfigurationValue::TemperatureDifference(
                conduit_core::ExactTemperatureDifferenceConfigurationValue::new(
                    conduit_core::ExactTemperatureDifference::parse_plot_literal(&delta_source(
                        value,
                    ))
                    .unwrap(),
                    delta_source(value),
                )
                .unwrap(),
            )
        },
    })
}
fn delta_source(literal: &str) -> String {
    let start = literal
        .char_indices()
        .find_map(|(i, c)| (!(c.is_ascii_digit() || c == '.' || (i == 0 && c == '-'))).then_some(i))
        .unwrap();
    format!(
        "TemperatureDelta({}, {})",
        &literal[..start],
        &literal[start..]
    )
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
        let authored = delta_source(original);
        let source = format!("# µ before original source\nplot difference (\n receipt: ExactTemperatureDifferenceConversionReceipt <= 8192B >>\n) {{\n converted: units/convert-temperature-difference(source = {authored}, to = {target})\n converted.receipt >> receipt\n}}.\n");
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
        assert_eq!(bytes(field(&receipt, "original")), authored.as_bytes());
        assert_eq!(
            bytes(field(&receipt, "profile")),
            conduit_core::temperature_delta_info_id().as_bytes()
        );
        let delta = conduit_core::Quantity::decode(bytes(field(&receipt, "source"))).unwrap();
        assert_eq!(delta.role(), conduit_core::QuantityRole::Delta);
        let target_unit =
            conduit_core::Unit::decode(bytes(field(&receipt, "target-unit"))).unwrap();
        assert_eq!(
            target_unit
                .definition()
                .exact_offset(delta.role())
                .unwrap()
                .numerator,
            0
        );
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
        let point =
            quantity_conversion::prepare_configuration(&point_configuration(original, target))
                .unwrap();
        assert!(difference::validate_receipt(&point).is_err());
    }
    let source = "plot bad (\n receipt: ExactQuantityConversionReceipt <= 8192B >>\n) {\n converted: units/convert-temperature-difference(source = TemperatureDelta(9, °F), to = K)\n converted.receipt >> receipt\n}.\n";
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
        ("TemperatureDelta(1, °C)", "mkg", "mkg"),
        ("21C", "K", "21C"),
    ] {
        let source = format!("# µ original\nplot invalid {{\n converted: units/convert-temperature-difference(source = {original}, to = {target})\n}}\n");
        let syntax = parse_syntax_document(&source);
        let error = check_syntax_document(&syntax, &startup).unwrap_err();
        assert_eq!(&source[error.span.start..error.span.end], offending);
    }
}

#[test]
fn difference_receipt_readmission_rejects_changed_target_definition_and_point_source_type() {
    let receipt = difference::prepare_configuration(&configuration("9°F", "°F")).unwrap();
    let point =
        quantity_conversion::prepare_configuration(&point_configuration("9°F", "°F")).unwrap();
    let StructuredInfoValueShape::Record(fields) = receipt.shape() else {
        panic!("record")
    };
    let wrong_target = conduit_core::StructuredInfoValue::leaf(
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::UNIT_INFO_ID))
            .unwrap(),
        conduit_core::Unit::Celsius.encode().to_vec(),
    )
    .unwrap();
    let forged_fields = fields
        .iter()
        .map(|entry| {
            if entry.name() == "target-unit" {
                StructuredFieldValue::new("target-unit", wrong_target.clone()).unwrap()
            } else {
                entry.clone()
            }
        })
        .collect();
    let forged = StructuredInfoValue::record(difference::receipt_type(), forged_fields).unwrap();
    assert!(difference::validate_receipt(&forged).is_err());
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
    let invalid_coordinate = conduit_core::Quantity::parse_plot_literal("1Hz").unwrap();
    assert!(StructuredInfoValue::leaf(
        difference::source_type(),
        invalid_coordinate.encode().to_vec(),
    )
    .is_err());
}

#[test]
fn expected_delta_type_never_reinterprets_a_bare_temperature_point() {
    let mut startup = StartupCatalog::new();
    quantity_conversion::install(&mut startup, &mut ProfileCatalog::new()).unwrap();
    for literal in ["9°F", "1°C", "1K"] {
        let source = format!("plot wrong {{\n converted: units/convert-temperature-difference(source = {literal}, to = K)\n}}\n");
        assert!(check_syntax_document(&parse_syntax_document(&source), &startup).is_err());
    }
}

fn point_configuration(source: &str, target: &str) -> [ConfigurationEntry; 2] {
    [
        ConfigurationEntry {
            key: "source".into(),
            value: ConfigurationValue::Quantity(
                conduit_core::QuantityConfigurationValue::parse(source).unwrap(),
            ),
        },
        ConfigurationEntry {
            key: "to".into(),
            value: ConfigurationValue::Unit(
                conduit_core::UnitConfigurationValue::parse(target).unwrap(),
            ),
        },
    ]
}
