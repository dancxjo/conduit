use conduit_web::{
    json_collection_step, json_collection_step_bytes, JsonCollectionRefusal as Refusal,
    JsonRefusal, JsonValue,
};

fn request(text: &str) -> JsonValue {
    JsonValue::decode_text(text.as_bytes()).unwrap()
}

#[test]
fn generic_records_preserve_order_and_refusals_leave_the_input_unchanged() {
    let input = request(
        r#"{"collection":[{"enabled":true,"name":"alpha"},{"enabled":false,"name":"beta"}],"command":{"index":0,"op":"remove"}}"#,
    );
    let before = input.encode_info().unwrap();
    let next = json_collection_step(&input).unwrap();
    assert_eq!(
        next.encode_text().unwrap(),
        br#"[{"enabled":false,"name":"beta"}]"#
    );
    assert_eq!(input.encode_info().unwrap(), before);
    let toggled = request(
        r#"{"collection":[{"enabled":false,"name":"beta"}],"command":{"field":"enabled","index":0,"op":"toggle"}}"#,
    );
    assert_eq!(
        json_collection_step(&toggled)
            .unwrap()
            .encode_text()
            .unwrap(),
        br#"[{"enabled":true,"name":"beta"}]"#
    );
}

#[test]
fn malformed_missing_fractional_wrong_type_and_unknown_commands_remain_distinct() {
    let cases = [
        (
            r#"{"collection":[],"command":{"index":0,"op":"remove"}}"#,
            Refusal::MissingIndex,
        ),
        (
            r#"{"collection":[1],"command":{"index":0.5,"op":"remove"}}"#,
            Refusal::InvalidIndex,
        ),
        (
            r#"{"collection":[1],"command":{"index":-1,"op":"remove"}}"#,
            Refusal::InvalidIndex,
        ),
        (
            r#"{"collection":[{"enabled":1}],"command":{"field":"enabled","index":0,"op":"toggle"}}"#,
            Refusal::NotBoolean,
        ),
        (
            r#"{"collection":[{}],"command":{"field":"enabled","index":0,"op":"toggle"}}"#,
            Refusal::MissingField,
        ),
        (
            r#"{"collection":[],"command":{"op":"invent"}}"#,
            Refusal::UnknownOperation,
        ),
        (
            r#"{"collection":[],"command":{"extra":true,"op":"clear"}}"#,
            Refusal::InvalidCommand,
        ),
    ];
    for (text, expected) in cases {
        let input = request(text);
        let before = input.encode_info().unwrap();
        assert_eq!(json_collection_step(&input), Err(expected));
        assert_eq!(input.encode_info().unwrap(), before);
    }
    assert!(matches!(
        json_collection_step_bytes(&[255]),
        Err(Refusal::InvalidValue(_))
    ));
}

#[test]
fn finite_item_and_string_bounds_refuse_without_a_partial_edit() {
    let entries = ["0"; 32].join(",");
    let input = request(&format!(
        r#"{{"collection":[{entries}],"command":{{"op":"append","value":1}}}}"#
    ));
    assert_eq!(json_collection_step(&input), Err(Refusal::CollectionFull));
    let oversized = JsonValue::String("x".repeat(conduit_web::JSON_MAXIMUM_STRING_BYTES + 1));
    assert_eq!(
        json_collection_step(&oversized),
        Err(Refusal::InvalidValue(JsonRefusal::StringByteOverflow))
    );
}

#[test]
fn replace_and_clear_are_explicit_and_round_trip_canonical_bytes() {
    for (input, expected) in [
        (
            r#"{"collection":[1,2],"command":{"index":0,"op":"replace","value":3}}"#,
            "[3,2]",
        ),
        (r#"{"collection":[1,2],"command":{"op":"clear"}}"#, "[]"),
    ] {
        let encoded = request(input).encode_info().unwrap();
        let output = json_collection_step_bytes(&encoded).unwrap();
        assert_eq!(
            JsonValue::decode_info(&output)
                .unwrap()
                .encode_text()
                .unwrap(),
            expected.as_bytes()
        );
    }
}
