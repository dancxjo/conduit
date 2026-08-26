use conduit_presentation::PresentationPropertyValue;

#[test]
fn portable_world_carries_the_canonical_state_explanation() {
    let (presentation, parts) = crate::portable_demonstration_with_parts().unwrap();
    let body = presentation
        .basis
        .body_id
        .as_ref()
        .map(|body| format!("body/{}", body.as_str()))
        .unwrap();

    for (name, expected) in [
        ("available-explanation", &parts.truth_explanation.available),
        (
            "line-ready-explanation",
            &parts.truth_explanation.line_ready,
        ),
        (
            "line-unavailable-explanation",
            &parts.truth_explanation.line_unavailable,
        ),
        ("in-plan-explanation", &parts.truth_explanation.in_plan),
        ("playing-explanation", &parts.truth_explanation.playing),
    ] {
        assert!(presentation.properties.iter().any(|property| {
            property.subject == body
                && property.name == name
                && property.value == PresentationPropertyValue::Text(expected.clone())
        }));
    }
}
