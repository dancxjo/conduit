use super::*;
use alloc::vec;

#[test]
fn canonical_vectors_cover_nested_unicode_numbers_and_member_order() {
    let value =
        JsonValue::decode_text(br#" { "z": [null,true,"\ud83c\udf0d"], "a": -12.3400 } "#).unwrap();
    assert_eq!(
        value.encode_text().unwrap(),
        "{\"a\":-12.34,\"z\":[null,true,\"🌍\"]}".as_bytes()
    );
    let info = value.encode_info().unwrap();
    assert_eq!(JsonValue::decode_info(&info).unwrap(), value);
}

#[test]
fn refusal_classes_remain_distinct() {
    assert_eq!(
        JsonValue::decode_text(&[0xff]),
        Err(JsonRefusal::InvalidUtf8)
    );
    assert_eq!(
        JsonValue::decode_text(b"["),
        Err(JsonRefusal::MalformedSyntax)
    );
    assert_eq!(
        JsonValue::decode_text(b"{\"a\":1,\"a\":2}"),
        Err(JsonRefusal::DuplicateKey)
    );
    assert_eq!(
        JsonValue::decode_text(b"0.0000001"),
        Err(JsonRefusal::NumericOverflow)
    );
    let mut deep = JsonValue::Null;
    for _ in 0..JSON_MAXIMUM_DEPTH {
        deep = JsonValue::Array(vec![deep]);
    }
    assert_eq!(deep.validate(), Err(JsonRefusal::DepthOverflow));
}

#[test]
fn every_declared_capacity_has_its_own_refusal() {
    let array = alloc::format!(
        "[{}]",
        core::iter::repeat_n("null", JSON_MAXIMUM_ARRAY_ITEMS + 1)
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(
        JsonValue::decode_text(array.as_bytes()),
        Err(JsonRefusal::ArrayItemOverflow)
    );
    let object = alloc::format!(
        "{{{}}}",
        (0..=JSON_MAXIMUM_OBJECT_MEMBERS)
            .map(|index| alloc::format!("\"k{index}\":null"))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(
        JsonValue::decode_text(object.as_bytes()),
        Err(JsonRefusal::ObjectMemberOverflow)
    );
    assert_eq!(
        JsonValue::String("x".repeat(JSON_MAXIMUM_STRING_BYTES + 1)).validate(),
        Err(JsonRefusal::StringByteOverflow)
    );
    assert_eq!(
        JsonValue::Object(vec![(
            "x".repeat(JSON_MAXIMUM_KEY_BYTES + 1),
            JsonValue::Null
        )])
        .validate(),
        Err(JsonRefusal::KeyByteOverflow)
    );
    let strings = JsonValue::Array(vec![
        JsonValue::String("a".repeat(700)),
        JsonValue::String("b".repeat(700)),
        JsonValue::String("c".repeat(700)),
    ]);
    assert_eq!(
        strings.validate(),
        Err(JsonRefusal::TotalStringByteOverflow)
    );
    assert_eq!(
        JsonValue::decode_text(&vec![b' '; JSON_MAXIMUM_ENCODED_BYTES + 1]),
        Err(JsonRefusal::EncodedByteOverflow)
    );

    let leaves = JsonValue::Array(vec![JsonValue::Null; JSON_MAXIMUM_ARRAY_ITEMS]);
    let branches = JsonValue::Array(vec![leaves; 4]);
    assert_eq!(branches.validate(), Err(JsonRefusal::NodeOverflow));
}

#[test]
fn fixed_six_decimal_number_profile_is_canonical_and_checked() {
    for (source, canonical) in [
        ("1e2", "100"),
        ("1.230000", "1.23"),
        ("-0", "0"),
        ("0.000001", "0.000001"),
    ] {
        assert_eq!(
            JsonValue::decode_text(source.as_bytes())
                .unwrap()
                .encode_text()
                .unwrap(),
            canonical.as_bytes()
        );
    }
    assert_eq!(
        JsonValue::decode_text(b"1e1000"),
        Err(JsonRefusal::NumericOverflow)
    );
    assert_eq!(
        JsonValue::decode_text(b"9223372036855"),
        Err(JsonRefusal::NumericOverflow)
    );
    assert_eq!(
        JsonValue::decode_text(b"\x0cnull"),
        Err(JsonRefusal::MalformedSyntax)
    );
}
