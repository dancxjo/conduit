//! The installed product owns catalogue admission and identical human/JSON spans.
use serde_json::Value;
use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn source(original: &str, target: &str) -> String {
    format!("# Unicode µ before owned spans\nplot conversion (\n receipt: ExactQuantityConversionReceipt <= 8192B >>\n) {{\n original = \"{original}\"\n alias = original\n converted: units/convert(source = alias, to = \"{target}\")\n converted.receipt >> receipt\n}}.\n")
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
        assert_eq!(diagnostic["code"], "CND-QTY-001");
        let start = diagnostic["primary_span"]["start"].as_u64().unwrap() as usize;
        let end = diagnostic["primary_span"]["end"].as_u64().unwrap() as usize;
        assert_eq!(&source[start..end], expected);
        let human = String::from_utf8(human.stdout).unwrap();
        assert!(human.contains("CND-QTY-001"));
        assert!(human.contains(diagnostic["summary"].as_str().unwrap()));
        assert_eq!(diagnostic["source_document_id"].as_str().unwrap().len(), 64);
    }
}
