use conduit_form::rust_binding::NativeRustBinding;
use conduit_presentation::GeneratedActionAffordance;

#[test]
fn generated_action_affordance_round_trips_and_enforces_identity_bounds() {
    let value = GeneratedActionAffordance::new("lesson/answer".into(), 4).unwrap();
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(
        GeneratedActionAffordance::from_structured(structured).unwrap(),
        value
    );
    assert!(GeneratedActionAffordance::new("x".repeat(128), u64::MAX).is_ok());
    assert!(GeneratedActionAffordance::new(String::new(), 0).is_err());
    assert!(GeneratedActionAffordance::new("x".repeat(129), 0).is_err());
}

#[test]
fn generated_action_affordance_preserves_exact_serde_and_unknown_field_refusal() {
    let value = GeneratedActionAffordance::new("lesson/answer".into(), 4).unwrap();
    let json = r#"{"action_identity":"lesson/answer","source_presentation_revision":4}"#;
    assert_eq!(serde_json::to_string(&value).unwrap(), json);
    assert_eq!(
        serde_json::from_str::<GeneratedActionAffordance>(json).unwrap(),
        value
    );
    assert!(serde_json::from_str::<GeneratedActionAffordance>(
        r#"{"action_identity":"lesson/answer","source_presentation_revision":4,"extra":true}"#,
    )
    .is_err());
    let bytes = [
        13, b'l', b'e', b's', b's', b'o', b'n', b'/', b'a', b'n', b's', b'w', b'e', b'r', 4,
    ];
    assert_eq!(postcard::to_allocvec(&value).unwrap(), bytes);
    assert_eq!(
        postcard::from_bytes::<GeneratedActionAffordance>(&bytes).unwrap(),
        value
    );
}
