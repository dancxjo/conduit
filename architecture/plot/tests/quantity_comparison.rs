use conduit_core::{
    ConfigurationEntry, ConfigurationValue, StructuredFieldValue, StructuredInfoValue,
    StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    quantity_conversion::{self, comparison},
    ProfileCatalog, StartupCatalog,
};

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
fn result(receipt: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = field(receipt, "result").shape() else {
        panic!("result")
    };
    tag
}
fn prepare(left: &str, right: &str, difference: bool) -> StructuredInfoValue {
    let (kind, name) = if difference {
        (
            comparison::DIFFERENCE_KIND,
            comparison::DIFFERENCE_RECEIPT_NAME,
        )
    } else {
        (comparison::KIND, comparison::RECEIPT_NAME)
    };
    let delta = |value: &str| {
        let split = value
            .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-' && c != '+')
            .unwrap();
        format!("TemperatureDelta({}, {})", &value[..split], &value[split..])
    };
    let left = if difference {
        delta(left)
    } else {
        left.to_owned()
    };
    let right = if difference {
        delta(right)
    } else {
        right.to_owned()
    };
    let source = format!("# µ original source\nplot compare (\n receipt: {name} <= 8192B >>\n) {{\n compared: {kind}(left = {left}, right = {right})\n compared.receipt >> receipt\n}}.\n");
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    quantity_conversion::install(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty());
    assert_eq!(syntax.round_trip(), source);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    quantity_conversion::validate_source(&syntax, &checked).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "compare", &profile).unwrap();
    let gear = &expanded.expanded.gears[0];
    let receipt =
        quantity_conversion::prepare_operation_configuration(kind, &gear.configuration).unwrap();
    if difference {
        comparison::validate_difference_receipt(&receipt)
    } else {
        comparison::validate_receipt(&receipt)
    }
    .unwrap();
    assert_eq!(
        gear.outputs[0].value_kind,
        *receipt.value_type().profile().unwrap().value_kind()
    );
    assert!(
        receipt.canonical_bytes().unwrap().len()
            <= quantity_conversion::MAXIMUM_RECEIPT_BYTES as usize
    );
    assert_eq!(
        bytes(field(field(&receipt, "left"), "original")),
        left.as_bytes()
    );
    assert_eq!(
        bytes(field(field(&receipt, "right"), "original")),
        right.as_bytes()
    );
    receipt
}

#[test]
fn ordinary_comparisons_match_independent_fraction_reference() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../core/tests/fixtures/quantity_comparison_reference.json"
    ))
    .unwrap();
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 798);
    for case in cases {
        let receipt = prepare(
            case["left"].as_str().unwrap(),
            case["right"].as_str().unwrap(),
            case["role"] == "difference",
        );
        assert_eq!(result(&receipt), case["result"].as_str().unwrap(), "{case}");
    }
}

#[test]
fn comparisons_preserve_exact_laws_and_do_not_project_to_a_repeating_decimal() {
    let receipt = prepare("1°F", "0°C", false);
    assert_eq!(result(&receipt), "less");
    let coordinate =
        conduit_core::Quantity::decode(bytes(field(field(&receipt, "left"), "coordinate")))
            .unwrap();
    assert_ne!(
        coordinate
            .unit()
            .exact_offset(coordinate.role())
            .unwrap()
            .numerator,
        0
    );
    let point = prepare("9°F", "5K", false);
    let difference = prepare("9°F", "5K", true);
    assert_eq!(result(&point), "greater");
    assert_eq!(result(&difference), "equal");
    assert_ne!(point.value_type(), difference.value_type());
    assert!(comparison::validate_receipt(&difference).is_err());
    assert!(comparison::validate_difference_receipt(&point).is_err());
    for (left, right, refusal) in [
        ("1Hz", "1m", "incompatible-dimensions"),
        ("1rad", "1°", "inexact"),
    ] {
        let receipt = prepare(left, right, false);
        let StructuredInfoValueShape::Variant { tag, payload } = field(&receipt, "result").shape()
        else {
            panic!("result")
        };
        assert_eq!(tag, "refused");
        assert_eq!(bytes(payload), refusal.as_bytes());
    }
    let receipt = prepare("1um²", "1m²", false);
    let coordinate =
        conduit_core::Quantity::decode(bytes(field(field(&receipt, "left"), "coordinate")))
            .unwrap();
    assert!(coordinate.unit().matches_source_evidence("um²"));
}

#[test]
fn comparison_diagnostics_keep_the_correct_operand_original_unicode_and_alias_span() {
    let mut startup = StartupCatalog::new();
    quantity_conversion::install(&mut startup, &mut ProfileCatalog::new()).unwrap();
    for (kind, left, right, expected) in [
        (comparison::KIND, "21C", "1K", "21C"),
        (comparison::KIND, "1m", "1mkg", "1mkg"),
        (
            comparison::DIFFERENCE_KIND,
            "TemperatureDelta(1, °C)",
            "1Hz",
            "1Hz",
        ),
        (comparison::KIND, "1m", "1μs", "1μs"),
    ] {
        let source = format!("# µ before operands\nplot invalid {{\n compared: {kind}(left = {left}, right = {right})\n}}\n");
        let syntax = parse_syntax_document(&source);
        let error = check_syntax_document(&syntax, &startup).unwrap_err();
        assert_eq!(&source[error.span.start..error.span.end], expected);
    }
}

#[test]
fn comparison_receipt_readmission_refuses_shape_valid_forged_results_and_operand_facts() {
    let receipt = prepare("1m", "1000mm", false);
    let unequal = prepare("1m", "2m", false);
    let StructuredInfoValueShape::Record(fields) = receipt.shape() else {
        panic!("receipt")
    };
    for name in ["result", "right"] {
        let fields = fields
            .iter()
            .map(|entry| {
                if entry.name() == name {
                    StructuredFieldValue::new(name, field(&unequal, name).clone()).unwrap()
                } else {
                    entry.clone()
                }
            })
            .collect();
        let forged = StructuredInfoValue::record(comparison::receipt_type(), fields).unwrap();
        if name == "result" {
            assert!(comparison::validate_receipt(&forged).is_err());
        }
        // A complete different operand is a new truthful receipt only with its recomputed result.
        if name == "right" {
            assert!(comparison::validate_receipt(&forged).is_err());
        }
    }
    let temperature = prepare("0°C", "273.15K", false);
    let original_left = field(&receipt, "left");
    let StructuredInfoValueShape::Record(operand_fields) = original_left.shape() else {
        panic!("operand")
    };
    for name in ["original", "coordinate"] {
        let replacement = field(field(&temperature, "left"), name);
        let operand = StructuredInfoValue::record(
            original_left.value_type().clone(),
            operand_fields
                .iter()
                .map(|entry| {
                    if entry.name() == name {
                        StructuredFieldValue::new(name, replacement.clone()).unwrap()
                    } else {
                        entry.clone()
                    }
                })
                .collect(),
        )
        .unwrap();
        let forged = StructuredInfoValue::record(
            comparison::receipt_type(),
            fields
                .iter()
                .map(|entry| {
                    if entry.name() == "left" {
                        StructuredFieldValue::new("left", operand.clone()).unwrap()
                    } else {
                        entry.clone()
                    }
                })
                .collect(),
        )
        .unwrap();
        assert!(comparison::validate_receipt(&forged).is_err(), "{name}");
    }
    let configuration = [
        ConfigurationEntry {
            key: "left".into(),
            value: ConfigurationValue::Quantity(
                conduit_core::QuantityConfigurationValue::parse("1m").unwrap(),
            ),
        },
        ConfigurationEntry {
            key: "left".into(),
            value: ConfigurationValue::Quantity(
                conduit_core::QuantityConfigurationValue::parse("1m").unwrap(),
            ),
        },
    ];
    assert!(comparison::prepare_configuration(&configuration).is_err());
}
