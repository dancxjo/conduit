use conduit_chat::{BodyConversationalSummary, BODY_CONVERSATIONAL_SUMMARY_SCHEMA};
use conduit_plot::rust_binding::NativeRustBinding;

fn summary() -> BodyConversationalSummary {
    BodyConversationalSummary::new(
        1,
        "Fixture Body".into(),
        "playing".into(),
        "awake".into(),
        1,
        2,
        3,
        BODY_CONVERSATIONAL_SUMMARY_SCHEMA.into(),
        4,
        5,
    )
    .unwrap()
}

#[test]
fn conversational_summary_has_exact_native_and_json_round_trips() {
    let summary = summary();
    let structured = summary.clone().into_structured().unwrap();
    assert_eq!(
        structured.value_type(),
        &BodyConversationalSummary::semantic_type().unwrap()
    );
    assert_eq!(
        BodyConversationalSummary::from_structured(structured).unwrap(),
        summary
    );

    let json = summary.canonical_bytes().unwrap();
    assert_eq!(
        core::str::from_utf8(&json).unwrap(),
        "{\"schema\":\"conduit.body/conversational-summary@1\",\"display_name\":\"Fixture Body\",\"lifecycle\":\"awake\",\"present_hosts\":2,\"offline_hosts\":1,\"active_plots\":1,\"execution\":\"playing\",\"ready_lines\":3,\"unavailable_lines\":4,\"unknown_lines\":5}"
    );
    assert_eq!(
        serde_json::from_slice::<BodyConversationalSummary>(&json).unwrap(),
        summary
    );
}

#[test]
fn conversational_summary_text_bounds_belong_to_the_native_type() {
    assert!(BodyConversationalSummary::new(
        0,
        "x".repeat(129),
        "idle".into(),
        "awake".into(),
        0,
        0,
        0,
        BODY_CONVERSATIONAL_SUMMARY_SCHEMA.into(),
        0,
        0,
    )
    .is_err());
}
