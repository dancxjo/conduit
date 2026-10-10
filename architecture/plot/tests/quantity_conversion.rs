use conduit_core::{StructuredInfoValue, StructuredInfoValueShape, QUANTITY_INFO_ID};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    quantity_conversion::*, ProfileCatalog, StartupCatalog,
};

fn prepare(original: &str, target: &str) -> StructuredInfoValue {
    let source = format!("# Preserve Unicode before all source spans: µ\nplot conversion (\n receipt: ExactQuantityConversionReceipt <= 8192B >>\n) {{\n converted: units/convert(source = {original}, to = {target})\n converted.receipt >> receipt\n}}.\n");
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    assert_eq!(syntax.round_trip(), source);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    validate_source(&syntax, &checked).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "conversion", &profile).unwrap();
    let gear = &expanded.expanded.gears[0];
    assert_eq!(gear.kind_id.as_str(), KIND);
    let result = prepare_configuration(&gear.configuration).unwrap();
    validate_receipt(&result).unwrap();
    assert_eq!(
        result.value_type().profile().unwrap().value_kind(),
        &gear.outputs[0].value_kind
    );
    assert!(result.canonical_bytes().unwrap().len() <= MAXIMUM_RECEIPT_BYTES as usize);
    result
}
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record");
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}
fn bytes(value: &StructuredInfoValue) -> &[u8] {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("leaf");
    };
    bytes
}

#[test]
fn ordinary_checked_gear_returns_source_target_law_profile_and_exact_coordinate() {
    for (source, target, coefficient, exponent) in [
        ("1kHz", "Hz", 1_i128, 3_i16),
        ("1µs", "ns", 1, 3),
        ("1cm²", "mm²", 1, 2),
        ("0°C", "K", 27315, -2),
        ("30°C", "°F", 86, 0),
        ("1Qm", "qm", 1, 60),
        ("1qm", "Qm", 1, -60),
        ("1uW", "MW", 1, -12),
    ] {
        let receipt = prepare(source, target);
        assert_eq!(bytes(field(&receipt, "original")), source.as_bytes());
        assert_eq!(bytes(field(&receipt, "target")), target.as_bytes());
        assert_eq!(
            bytes(field(&receipt, "profile")),
            QUANTITY_INFO_ID.as_bytes()
        );
        let StructuredInfoValueShape::Variant { tag, payload } = field(&receipt, "result").shape()
        else {
            panic!("result");
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
        assert!(conduit_core::Quantity::decode(bytes(field(&receipt, "source"))).is_ok());
    }
}

#[test]
fn ordinary_conversion_retains_precise_refusal_in_a_typed_receipt() {
    for (source, target, refusal) in [
        ("1°F", "°C", "inexact"),
        ("1Hz", "m", "incompatible-dimensions"),
        ("1Qm³", "qm³", "overflow"),
    ] {
        let receipt = prepare(source, target);
        let StructuredInfoValueShape::Variant { tag, payload } = field(&receipt, "result").shape()
        else {
            panic!("result");
        };
        assert_eq!(tag, "refused");
        assert_eq!(bytes(payload), refusal.as_bytes());
        assert_eq!(bytes(field(&receipt, "original")), source.as_bytes());
    }
}

#[test]
fn invalid_request_diagnostics_keep_original_unicode_spans_and_local_aliases() {
    for (original, target, expected) in [
        ("21C", "K", "21C"),
        ("1m", "mkg", "mkg"),
        ("1μs", "s", "1μs"),
    ] {
        let source = format!("# Unicode µ before the invalid literal\nplot conversion {{\n original = {original}\n alias = original\n converted: units/convert(source = alias, to = {target})\n}}\n");
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        install(&mut startup, &mut profile).unwrap();
        let syntax = parse_syntax_document(&source);
        let error = check_syntax_document(&syntax, &startup).unwrap_err();
        assert_eq!(&source[error.span.start..error.span.end], expected);
    }
}

#[test]
fn complete_independent_target_matrix_passes_through_authored_checking_and_expansion() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../core/tests/fixtures/quantity_prefix_scales.json"
    ))
    .unwrap();
    for case in reference["cases"].as_array().unwrap() {
        let target = &case["source"].as_str().unwrap()[1..];
        let original = format!("1{}", case["base"].as_str().unwrap());
        let receipt = prepare(&original, target);
        let StructuredInfoValueShape::Variant { tag, payload } = field(&receipt, "result").shape()
        else {
            panic!("result");
        };
        assert_eq!(tag, "converted");
        let expected = case["relative_denominator"].as_str().unwrap().len() as i16
            - case["relative_numerator"].as_str().unwrap().len() as i16;
        assert_eq!(
            i128::from_le_bytes(bytes(field(payload, "coefficient")).try_into().unwrap()),
            1
        );
        assert_eq!(
            i16::from_le_bytes(bytes(field(payload, "exponent")).try_into().unwrap()),
            expected,
            "{target}"
        );
    }
}

#[test]
fn readmission_refuses_forged_facts_even_when_the_record_shape_is_valid() {
    use conduit_core::{kind_id, StructuredFieldValue, StructuredInfoType, TEXT_INFO_ID};
    let receipt = prepare("0°C", "K");
    let StructuredInfoValueShape::Record(fields) = receipt.shape() else {
        panic!("record");
    };
    for name in [
        "profile",
        "source-dimension",
        "catalogue",
        "target",
        "original",
    ] {
        let fields = fields
            .iter()
            .map(|field| {
                if field.name() == name {
                    StructuredFieldValue::new(
                        name,
                        StructuredInfoValue::leaf(
                            StructuredInfoType::leaf(kind_id(TEXT_INFO_ID)).unwrap(),
                            b"forged".to_vec(),
                        )
                        .unwrap(),
                    )
                    .unwrap()
                } else {
                    field.clone()
                }
            })
            .collect();
        let forged = StructuredInfoValue::record(receipt_type(), fields).unwrap();
        assert!(validate_receipt(&forged).is_err(), "{name}");
    }
}

#[test]
fn installing_conversion_keeps_intrinsic_literal_checked_identities() {
    use conduit_plot::{CanonicalStartupValue, KindSignature, StartupParameterSignature};
    let mut intrinsic = StartupCatalog::new();
    intrinsic
        .insert(KindSignature {
            kind: "test/intrinsic-literal".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "value".into(),
                value_type: "Quantity".into(),
                default: None,
            }],
        })
        .unwrap();
    let mut extended = intrinsic.clone();
    install(&mut extended, &mut ProfileCatalog::new()).unwrap();
    for literal in ["440Hz", "250ms", "3.2m", "21°C", "640px"] {
        let source = format!("plot intrinsic {{\n value: test/intrinsic-literal({literal})\n}}\n");
        let syntax = parse_syntax_document(&source);
        let before = check_syntax_document(&syntax, &intrinsic).unwrap();
        let after = check_syntax_document(&syntax, &extended).unwrap();
        assert_eq!(
            before.plots[0].checked_plot_id,
            after.plots[0].checked_plot_id
        );
        let before = &before.plots[0].gears[0].startup_bindings[0].value;
        let after = &after.plots[0].gears[0].startup_bindings[0].value;
        assert_eq!(before, after);
        let CanonicalStartupValue::Quantity(value) = after else {
            panic!("intrinsic quantity");
        };
        assert_eq!(
            value.value().encode().len(),
            conduit_core::QUANTITY_ENCODED_LEN
        );
        assert_eq!(
            value.value(),
            conduit_core::Quantity::parse_plot_literal(literal).unwrap()
        );
        assert_eq!(syntax.round_trip(), source);
    }
}
