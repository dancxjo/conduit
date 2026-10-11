//! The installed product owns catalogue admission and identical human/JSON spans.
use serde_json::Value;
use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn source(original: &str, target: &str) -> String {
    format!("# Unicode µ before owned spans\nplot conversion (\n receipt: ExactQuantityConversionReceipt <= 8192B >>\n) {{\n converted: units/convert(source = {original}, to = {target})\n converted.receipt >> receipt\n}}.\n")
}
fn check(source: &str, json: bool) -> std::process::Output {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "conduit-quantity-conversion-{}-{nonce}.conduit",
        std::process::id()
    ));
    fs::write(&path, source).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_conduit"));
    command.arg("check").arg(&path);
    if json {
        command.arg("--json");
    }
    let result = command.output().unwrap();
    fs::remove_file(path).unwrap();
    result
}

#[test]
fn standard_product_catalogue_checks_explicit_conversion_and_retained_refusal_values() {
    for (original, target) in [
        ("1kHz", "Hz"),
        ("1Qm", "qm"),
        ("1qm", "Qm"),
        ("0°C", "K"),
        ("1°F", "°C"),
    ] {
        let result = check(&source(original, target), true);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        let diagnostics: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(diagnostics, serde_json::json!([]));
    }
}

#[test]
fn invalid_conversion_requests_keep_exact_original_spans_in_both_product_presentations() {
    for (original, target, expected) in [
        ("21C", "K", "21C"),
        ("1m", "mkg", "mkg"),
        ("1μs", "s", "1μs"),
    ] {
        let source = source(original, target);
        let human = check(&source, false);
        let machine = check(&source, true);
        assert!(!human.status.success());
        assert!(!machine.status.success());
        let diagnostics: Value = serde_json::from_slice(&machine.stdout).unwrap();
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic["code"], "CND-FRM-051");
        let start = diagnostic["primary_span"]["start"].as_u64().unwrap() as usize;
        let end = diagnostic["primary_span"]["end"].as_u64().unwrap() as usize;
        assert_eq!(&source[start..end], expected);
        let human = String::from_utf8(human.stdout).unwrap();
        assert!(human.contains("CND-FRM-051"));
        assert!(human.contains(diagnostic["summary"].as_str().unwrap()));
        assert_eq!(diagnostic["source_document_id"].as_str().unwrap().len(), 64);
    }
}

#[test]
fn product_checks_distinct_temperature_difference_contract_and_owned_refusals() {
    let difference = |original: &str, target: &str| {
        source(original, target)
            .replace(
                "ExactQuantityConversionReceipt",
                "ExactTemperatureDifferenceConversionReceipt",
            )
            .replace("units/convert(", "units/convert-temperature-difference(")
    };
    for (original, target) in [
        ("TemperatureDelta(9, °F)", "K"),
        ("TemperatureDelta(1, m°C)", "K"),
        ("TemperatureDelta(1, QK)", "qK"),
        ("TemperatureDelta(1, °F)", "K"),
        ("TemperatureDelta(1, °C)", "m"),
    ] {
        let result = check(&difference(original, target), true);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&result.stdout).unwrap(),
            serde_json::json!([])
        );
    }
    for (original, target, expected) in [
        ("1Hz", "K", "1Hz"),
        ("TemperatureDelta(1, °C)", "mkg", "mkg"),
    ] {
        let source = difference(original, target);
        let human = check(&source, false);
        let machine = check(&source, true);
        assert!(!human.status.success());
        assert!(!machine.status.success());
        let diagnostics: Value = serde_json::from_slice(&machine.stdout).unwrap();
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic["code"], "CND-FRM-051");
        let start = diagnostic["primary_span"]["start"].as_u64().unwrap() as usize;
        let end = diagnostic["primary_span"]["end"].as_u64().unwrap() as usize;
        assert_eq!(&source[start..end], expected);
        assert!(String::from_utf8(human.stdout)
            .unwrap()
            .contains(diagnostic["summary"].as_str().unwrap()));
    }
}

#[test]
fn product_checks_exact_comparisons_and_preserves_the_offending_operand_span() {
    let source = |kind: &str, name: &str, left: &str, right: &str| {
        format!("# µ before original operands\nplot compare (\n receipt: {name} <= 8192B >>\n) {{\n compared: {kind}(left = {left}, right = {right})\n compared.receipt >> receipt\n}}.\n")
    };
    for (kind, name, left, right) in [
        (
            "units/compare",
            "ExactQuantityComparisonReceipt",
            "1000mm",
            "0.001km",
        ),
        (
            "units/compare",
            "ExactQuantityComparisonReceipt",
            "1°F",
            "0°C",
        ),
        (
            "units/compare",
            "ExactQuantityComparisonReceipt",
            "1Hz",
            "1m",
        ),
        (
            "units/compare-temperature-differences",
            "ExactTemperatureDifferenceComparisonReceipt",
            "TemperatureDelta(9, °F)",
            "TemperatureDelta(5, K)",
        ),
    ] {
        let result = check(&source(kind, name, left, right), true);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&result.stdout).unwrap(),
            serde_json::json!([])
        );
    }
    for (kind, name, left, right, expected) in [
        (
            "units/compare",
            "ExactQuantityComparisonReceipt",
            "21C",
            "1K",
            "21C",
        ),
        (
            "units/compare",
            "ExactQuantityComparisonReceipt",
            "1m",
            "1mkg",
            "1mkg",
        ),
        (
            "units/compare-temperature-differences",
            "ExactTemperatureDifferenceComparisonReceipt",
            "TemperatureDelta(1, °C)",
            "1Hz",
            "1Hz",
        ),
    ] {
        let source = source(kind, name, left, right);
        let human = check(&source, false);
        let machine = check(&source, true);
        assert!(!human.status.success());
        assert!(!machine.status.success());
        let diagnostics: Value = serde_json::from_slice(&machine.stdout).unwrap();
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic["code"], "CND-FRM-051");
        let start = diagnostic["primary_span"]["start"].as_u64().unwrap() as usize;
        let end = diagnostic["primary_span"]["end"].as_u64().unwrap() as usize;
        assert_eq!(&source[start..end], expected);
        assert!(String::from_utf8(human.stdout)
            .unwrap()
            .contains(diagnostic["summary"].as_str().unwrap()));
    }
}
