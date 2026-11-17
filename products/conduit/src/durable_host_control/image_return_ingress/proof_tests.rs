use super::*;
use conduit_body::MemberReturnProofDocument;

fn proof() -> serde_json::Value {
    serde_json::json!({
        "admission_id":"return/test", "body_id":"body/test", "part_id":"part/test",
        "host_id":"host/test", "boot_id":"boot/test", "nonce":vec![1;32],
        "signature":vec![7;64]
    })
}
fn legacy(input: &str) -> bool {
    serde_json::from_str::<MemberReturnProofDocument>(input)
        .ok()
        .and_then(|proof| proof.decode().ok())
        .is_some()
}
fn compare(input: &str) {
    let view = inspect(input);
    assert_eq!(view.is_ok(), legacy(input), "{input}");
    if let Ok(view) = view {
        let expected = serde_json::from_str::<MemberReturnProofDocument>(input)
            .unwrap()
            .decode()
            .unwrap();
        let actual = view.decode().unwrap().decode().unwrap();
        assert!(actual == expected);
    }
}

#[test]
fn exact_borrowed_bytes_and_original_consuming_proof_correspond() {
    let input = proof().to_string();
    let view = inspect(&input).unwrap();
    assert_eq!(view.identifier_bytes, [11, 9, 9, 9, 9]);
    assert_eq!(view.raw.get().as_ptr(), input.as_ptr());
    compare(&input);
    let mut value = proof();
    value["body_id"] = "creek/é🌙".into();
    let input = value
        .to_string()
        .replace("é", "\\u00e9")
        .replace("🌙", "\\ud83c\\udf19");
    let input = input.replace("admission_id", "admission_\\u0069d");
    let view = inspect(&input).unwrap();
    assert_eq!(view.identifier_bytes[1], "creek/é🌙".len());
    compare(&input);
}

#[test]
fn map_order_and_positional_struct_follow_original_decode() {
    let value = proof();
    let sequence: Vec<_> = FIELDS.iter().map(|field| value[*field].clone()).collect();
    compare(&serde_json::to_string(&sequence).unwrap());
    let reversed = FIELDS
        .iter()
        .rev()
        .map(|field| {
            format!(
                "{}:{}",
                serde_json::to_string(field).unwrap(),
                value[*field]
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    compare(&format!("{{{reversed}}}"));
}

#[test]
fn finite_profile_matches_document_plus_decode_identifier_and_byte_bounds() {
    for field in &FIELDS[..5] {
        for text in [
            String::new(),
            "x".repeat(256),
            "x".repeat(257),
            "é".repeat(128),
            "é".repeat(129),
        ] {
            let mut value = proof();
            value[*field] = text.into();
            compare(&value.to_string());
        }
    }
    for (field, size) in [("nonce", 32), ("signature", 64)] {
        for length in [0, size - 1, size, size + 1] {
            let mut value = proof();
            value[field] = serde_json::json!(vec![255; length]);
            compare(&value.to_string());
        }
        for invalid in [
            serde_json::Value::Null,
            serde_json::json!("1"),
            serde_json::json!([256]),
            serde_json::json!([[-1]]),
        ] {
            let mut value = proof();
            value[field] = invalid;
            compare(&value.to_string());
        }
    }
}

#[test]
fn malformed_numeric_leaves_refuse_before_typed_numeric_or_string_dispatch() {
    let value = proof();
    for field in FIELDS {
        let mut fields = value.as_object().unwrap().clone();
        fields.remove(*field);
        let remaining = serde_json::to_string(&fields).unwrap();
        let remaining = &remaining[1..remaining.len() - 1];
        let raw = if *field == "nonce" || *field == "signature" {
            let size = if *field == "nonce" { 32 } else { 64 };
            let zeros = vec!["0"; size - 1].join(",");
            format!("[1e100000,{zeros}]")
        } else {
            "1e100000".into()
        };
        let input = format!("{{{remaining},\"{field}\":{raw}}}");
        let error = inspect(&input).err().unwrap().to_string();
        assert!(!error.contains("1e100000"));
    }
    for raw in ["-0", "-1", "1.0", "1e0", "true", "null", "\"1\""] {
        let zeros = vec!["0"; 31].join(",");
        let mut value = proof();
        value.as_object_mut().unwrap().remove("nonce");
        let remaining = value.to_string();
        let input = format!(
            "{{{},\"nonce\":[{raw},{zeros}]}}",
            &remaining[1..remaining.len() - 1]
        );
        compare(&input);
    }
}

#[test]
fn duplicate_unknown_missing_and_extra_fields_remain_refusals() {
    let ordinary = proof().to_string();
    for input in [
        ordinary.replacen('{', "{\"nonce\":[],", 1),
        ordinary.replacen('{', "{\"unknown\":null,", 1),
        ordinary.replace("\"admission_id\":\"return/test\",", ""),
        "null".into(),
        "1e100000".into(),
        "[]".into(),
    ] {
        compare(&input);
        assert!(inspect(&input).is_err());
    }
    let value = proof();
    let mut sequence: Vec<_> = FIELDS.iter().map(|field| value[*field].clone()).collect();
    sequence.push(serde_json::Value::Null);
    compare(&serde_json::to_string(&sequence).unwrap());
}

#[test]
fn structural_profile_does_not_assert_signature_or_membership_authority() {
    let mut value = proof();
    value["nonce"] = serde_json::json!(vec![0; 32]);
    value["signature"] = serde_json::json!(vec![0; 64]);
    let input = value.to_string();
    // Ordinary admission verification must still reject stale/unsigned scope.
    // This layer establishes only the finite document construction profile.
    compare(&input);
    assert!(inspect(&input).is_ok());
}

#[test]
fn escaped_duplicate_keys_each_missing_field_and_trailing_bytes_refuse() {
    let ordinary = proof().to_string();
    let duplicate = ordinary.replacen('{', "{\"body_\\u0069d\":\"body/other\",", 1);
    compare(&duplicate);
    assert!(inspect(&duplicate).is_err());
    for field in FIELDS {
        let mut value = proof();
        value.as_object_mut().unwrap().remove(*field);
        compare(&value.to_string());
        assert!(inspect(&value.to_string()).is_err());
    }
    for suffix in ["null", "{}", "0", "garbage"] {
        let input = format!("{ordinary}{suffix}");
        compare(&input);
        assert!(inspect(&input).is_err());
    }
}

#[test]
fn malformed_unicode_and_large_invalid_text_do_not_escape_in_diagnostics() {
    for text in ["\\ud800", "\\udfff", "\\ud800x", "\\x41"] {
        let input = proof().to_string().replace("return/test", text);
        compare(&input);
        assert!(inspect(&input).is_err());
    }
    let input = proof()
        .to_string()
        .replace("return/test", &"secret".repeat(1024));
    let error = inspect(&input).err().unwrap().to_string();
    assert!(!error.contains("secret"));
    assert!(error.len() < 256);
    let mut value = proof();
    value["nonce"] = "secret".repeat(1024).into();
    let error = inspect(&value.to_string()).err().unwrap().to_string();
    assert!(!error.contains("secret"));
    assert!(error.len() < 256);
}
