use conduit_web::{json_boolean_summary, JsonSummaryRefusal, JsonValue};

#[test]
fn arbitrary_boolean_field_counts_both_outcomes_without_mutating_records() {
    let value = JsonValue::decode_text(br#"[{"enabled":true},{"enabled":false},{"enabled":true}]"#)
        .unwrap();
    let before = value.encode_info().unwrap();
    let summary = json_boolean_summary(&value, "enabled").unwrap();
    assert_eq!(
        summary.encode_text().unwrap(),
        br#"{"false":1,"total":3,"true":2}"#
    );
    assert_eq!(value.encode_info().unwrap(), before);
    let empty = JsonValue::decode_text(b"[]").unwrap();
    assert_eq!(
        json_boolean_summary(&empty, "enabled")
            .unwrap()
            .encode_text()
            .unwrap(),
        br#"{"false":0,"total":0,"true":0}"#
    );
}

#[test]
fn shape_and_field_failures_are_distinct() {
    for (input, field, expected) in [
        ("[]", "", JsonSummaryRefusal::InvalidField),
        ("{}", "enabled", JsonSummaryRefusal::NotCollection),
        ("[1]", "enabled", JsonSummaryRefusal::NotRecord),
        ("[{}]", "enabled", JsonSummaryRefusal::MissingField),
        (
            r#"[{"enabled":1}]"#,
            "enabled",
            JsonSummaryRefusal::NotBoolean,
        ),
    ] {
        let value = JsonValue::decode_text(input.as_bytes()).unwrap();
        assert_eq!(json_boolean_summary(&value, field), Err(expected));
    }
}
