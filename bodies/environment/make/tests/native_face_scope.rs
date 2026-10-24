//! Actual MCU scope conversion over opaque Body/Wake newtypes, with CPU IDs.
#[path = "../../../../targets/esp32/proof/tm1637/src/native_face_scope.rs"]
mod scope;
#[test]
fn preserves_both_lifecycle_newtypes_and_rejects_unbounded_or_unframed_text() {
    let text = "a".repeat(64);
    let body: conduit_body::BodyId = scope::identity(&text).unwrap();
    let wake: conduit_body::WakeId = scope::identity(&text).unwrap();
    assert_eq!(body.as_str(), text);
    assert_eq!(wake.as_str(), text);
    assert_eq!(serde_json::to_string(&body).unwrap(), format!("\"{text}\""));
    for invalid in [
        "a".repeat(63),
        "a".repeat(65),
        "z".repeat(64),
        "\"".repeat(64),
        "é".repeat(32),
    ] {
        assert!(matches!(
            scope::identity::<conduit_body::BodyId>(&invalid),
            Err("face-scope-bound")
        ));
        assert!(matches!(
            scope::identity::<conduit_body::WakeId>(&invalid),
            Err("face-scope-bound")
        ));
    }
}
