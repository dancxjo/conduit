//! Snapshot text inspection preserves syntax without constructing value trees.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_web::{validate_json_text, JsonRefusal as E, JsonValue};

#[test]
fn snapshot_text_and_canonical_tree_have_separate_admission() {
    let full = format!(r#"{{"records":"{}"}}"#, "00".repeat(64 * 21));
    assert_eq!(validate_json_text(full.as_bytes()), Ok(()));
    assert_eq!(
        JsonValue::decode_text(full.as_bytes()),
        Err(E::StringByteOverflow)
    );
    let many = format!("[{}]", vec!["0"; 64].join(","));
    assert_eq!(validate_json_text(many.as_bytes()), Ok(()));
    assert_eq!(
        JsonValue::decode_text(many.as_bytes()),
        Err(E::ArrayItemOverflow)
    );
    for text in [
        r#"{"a":1,"a":2}"#,
        "1e999999999999999999999999999",
        "0.0000001",
    ] {
        assert_eq!(validate_json_text(text.as_bytes()), Ok(()));
        assert!(JsonValue::decode_text(text.as_bytes()).is_err());
    }
}
#[test]
fn syntax_bounds_unicode_and_failures_require_no_allocations() {
    let full = format!(r#"{{"records":"{}"}}"#, "00".repeat(64 * 21));
    let exact = format!("\"{}\"", "x".repeat(4094));
    let over = format!("\"{}\"", "x".repeat(4095));
    let nested = format!("{}0{}", "[".repeat(7), "]".repeat(7));
    let too_deep = format!("{}0{}", "[".repeat(8), "]".repeat(8));
    let ((), memory) = allocation_probe::observe(|| {
        for valid in [
            full.as_str(),
            exact.as_str(),
            nested.as_str(),
            r#" {"a":[true,false,null,"\ud83c\udf0d","🌍"],"b":-1.23e+4} "#,
        ] {
            assert_eq!(validate_json_text(valid.as_bytes()), Ok(()));
        }
        assert_eq!(
            validate_json_text(over.as_bytes()),
            Err(E::EncodedByteOverflow)
        );
        assert_eq!(
            validate_json_text(too_deep.as_bytes()),
            Err(E::DepthOverflow)
        );
        assert_eq!(validate_json_text(&[0xff]), Err(E::InvalidUtf8));
        for invalid in [
            "",
            "01",
            "1.",
            "+1",
            ".1",
            "-",
            "1e",
            "1e+",
            "[1,]",
            "{\"a\":1,}",
            "{}[]",
            "{1:2}",
            "[tru]",
            r#""\x""#,
            r#""\ud800""#,
            r#""\udc00""#,
            r#""\ud800\u0041""#,
            "\"a\nb\"",
        ] {
            assert_eq!(
                validate_json_text(invalid.as_bytes()),
                Err(E::MalformedSyntax),
                "{invalid:?}"
            );
        }
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
