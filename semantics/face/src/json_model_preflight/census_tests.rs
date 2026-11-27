use super::*;
use alloc::{format, string::String};
#[test]
fn counts_immediate_elements_and_conservative_escaped_string_bytes() {
    let bytes = br#"{"items":[{"key":"a\u0062"},"xyz",[1,2],true,null],"empty":[]}"#;
    let shape = json_shape(bytes).unwrap();
    assert_eq!(
        shape,
        JsonModelShape {
            objects: 2,
            object_fields: 3,
            arrays: 3,
            object_or_string_array_elements: 3,
            numeric_array_elements: 4,
            string_source_bytes: 5 + 3 + 7 + 3 + 5
        }
    );
    assert!(serde_json::from_slice::<serde_json::Value>(bytes).is_ok());
}
#[test]
fn refuses_malformed_structure_and_fences_nesting_without_allocating_scan() {
    for bad in [
        b"[1,]".as_slice(),
        b"{\"x\":}",
        b"[01]",
        b"truefalse",
        b"\"\\u123X\"",
        b"{",
        b"[] []",
        b"[+1]",
    ] {
        assert_eq!(json_shape(bad), Err(JsonModelRefusal::Json));
    }
    let nested = format!("{}0{}", "[".repeat(128), "]".repeat(128));
    assert!(json_shape(nested.as_bytes()).is_ok());
    let deeper = format!("[{nested}]");
    assert_eq!(json_shape(deeper.as_bytes()), Err(JsonModelRefusal::Json));
    let too_large = String::from_utf8(alloc::vec![b' '; MAX_JSON_MODEL_BYTES + 1]).unwrap();
    assert_eq!(
        json_shape(too_large.as_bytes()),
        Err(JsonModelRefusal::Capacity)
    );
}

#[test]
fn selected_line_base_identity_is_distinct_from_owning_placement_binding() {
    assert!(generic_mask_plan_json_shape(
        br#"{"selected_line":{"binding":{"base":"conduit.base/websocket@1"}}}"#
    )
    .is_ok());
    for value in [
        br#"{"base":{"provider":"host"}}"#.as_slice(),
        br#"{"base":[]}"#,
    ] {
        assert_eq!(
            generic_mask_plan_json_shape(value),
            Err(JsonModelRefusal::Unsupported)
        );
    }
}
