//! The recorded ordinary graph separates updates, samples, and selected effects.
use super::*;
pub(super) fn assert_replay(harness: &common::Harness, report: &conduit_std_host::StdRunReport) {
    use conduit_face::FacePropertyValue as Value;
    let snapshot = super::pete_replay::snapshot(
        &harness.fragment,
        &harness.authoring,
        &common::context(),
        report,
    )
    .unwrap();
    let timeline = snapshot.timeline.as_ref().unwrap();
    assert!(timeline.gap.is_none());
    assert_eq!(timeline.evicted_events, 0);
    let count = |event: &str, suffix: &str| {
        timeline
            .events
            .iter()
            .filter(|item| item.event == event && item.subject.ends_with(suffix))
            .count()
    };
    assert_eq!(count("value-sent", "/front/output/reactions"), 1);
    assert_eq!(count("value-sent", "/front/output/attention"), 3);
    assert_eq!(count("value-received", "/sample/input/trigger"), 2);
    assert_eq!(count("value-received", "/model/input/request"), 2);
    assert_eq!(count("value-sent", "/front/output/episodes"), 2);
    let trace = timeline.trace.as_ref().unwrap();
    assert!(trace.missing_parent_sequences.is_empty());
    assert_eq!(
        trace
            .steps
            .iter()
            .filter(|item| item.event == "value-received"
                && item.subject.contains("front/input/frames"))
            .count(),
        2
    );
    for port in [
        "/experience/input/inputs",
        "/sample/input/current",
        "/recall/input/experience",
        "/retrieve/input/window",
        "/retrieve/input/intent",
        "/contextualize/input/capture",
        "/contextualize/input/outcome",
        "/model/input/request",
        "/retain/input/response",
        "/memory/input/update",
        "/recall/input/memory",
        "/front/output/episodes",
    ] {
        assert!(
            trace.steps.iter().any(|item| item.subject.ends_with(port)),
            "missing {port}"
        );
    }
    let properties = &snapshot.face.properties;
    let pressure = properties
        .iter()
        .find(|property| {
            property.name == "pressure"
                && property.value == Value::Text("policy: coalesce-latest".into())
        })
        .unwrap();
    assert!(properties
        .iter()
        .any(|property| property.subject == pressure.subject
            && property.name == "maximum-queue-items"
            && property.value == Value::Count(1)));
    let model = properties
        .iter()
        .find(|property| {
            property.name == "implementation-id"
                && property.value == Value::Identity(conduit_ai::LOCAL_MODEL_IMPLEMENTATION.into())
        })
        .unwrap();
    for (name, expected) in [("maximum-input-bytes", 4096), ("maximum-in-flight", 1)] {
        assert!(properties
            .iter()
            .any(|property| property.subject == model.subject
                && property.name == name
                && property.value == Value::Count(expected)));
    }
    assert!(properties
        .iter()
        .any(|property| property.subject == model.subject
            && property.name == "host-id"
            && property.value == Value::Identity(harness.fragment.host_id.as_str().into())));
    if let Some(path) = std::env::var_os("CONDUIT_PETE_CADENCE_REPLAY_SNAPSHOT") {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap()
            .write_all(&snapshot.encode().unwrap())
            .unwrap();
    }
}
