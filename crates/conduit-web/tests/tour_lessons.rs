use std::collections::BTreeSet;

use conduit_runtime::{Registry, RunIo};
use serde_json::Value;

const REQUIRED_FOUNDATION_LESSONS: [&str; 15] = [
    "welcome.hello-panel",
    "welcome.pull-the-cord",
    "welcome.change-message",
    "welcome.nothing-up-our-sleeves",
    "nodes.more-than-one-port",
    "nodes.direction-matters",
    "nodes.types-mean-promises",
    "nodes.fan-out-is-a-choice",
    "nodes.empty-is-not-never",
    "panels.put-a-panel-in-a-panel",
    "panels.jacks-on-the-front",
    "panels.inside-outside",
    "panels.reuse-without-copying",
    "panels.tiny-instrument",
    "patchbay.observes-patchbay",
];

#[test]
fn tour_lessons_use_the_production_parser_and_runtime() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/v1.json"))
        .expect("Tour lesson manifest is valid JSON");
    assert_eq!(manifest["schema"], "conduit.tour-lessons/v1");

    let lessons = manifest["lessons"]
        .as_array()
        .expect("Tour lesson manifest contains lessons");
    let actual_ids = lessons
        .iter()
        .map(|lesson| lesson["id"].as_str().expect("lesson has an id"))
        .collect::<BTreeSet<_>>();
    let required_ids = REQUIRED_FOUNDATION_LESSONS
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual_ids, required_ids,
        "Tour foundation contains every Chapter 0-2 lesson exactly once"
    );
    let browser_plan: Value =
        serde_json::from_str(include_str!("../../../tour/public/browser-plan.json"))
            .expect("Tour browser plan is valid JSON");
    assert_eq!(
        browser_plan["schema"], "conduit.tour-browser-plan/v1",
        "lessons consume the exact browser-host plan"
    );
    let maximum_message_bytes = browser_plan["bounds"]["maximum_message_bytes"]
        .as_u64()
        .expect("browser plan bounds message bytes");
    let maximum_evidence_events = browser_plan["bounds"]["maximum_evidence_events"]
        .as_u64()
        .expect("browser plan bounds evidence");

    for lesson in lessons {
        let id = lesson["id"].as_str().expect("lesson has an id");
        assert!(lesson["chapter"].as_u64().is_some(), "{id} has a chapter");
        for field in ["title", "objective", "prose", "solution"] {
            assert!(
                lesson[field]
                    .as_str()
                    .is_some_and(|value| !value.is_empty()),
                "{id} has {field} lesson text"
            );
        }
        for field in ["prerequisites", "vocabulary", "hints"] {
            assert!(lesson[field].is_array(), "{id} has {field}");
        }
        assert!(
            lesson["presentation"].is_object(),
            "{id} separates presentation"
        );
        assert!(
            lesson["accessibility"].is_object(),
            "{id} has an accessible alternative"
        );
        assert!(
            lesson["command"]
                .as_str()
                .expect("lesson has a command")
                .starts_with("conduct "),
            "{id} uses the canonical conduct command"
        );
        assert!(
            lesson["commands"].is_null() || lesson["commands"].is_array(),
            "{id} commands are an optional canonical command list"
        );
        assert_eq!(
            lesson["profile"], "browser-dedicated-worker",
            "{id} uses the exact bounded browser-host placement"
        );

        let source = lesson["source"].as_str().expect("lesson has source");
        assert!(
            source.len() as u64 <= maximum_message_bytes,
            "{id} fits the exact worker message bound"
        );
        assert!(
            lesson["budgets"]["queue_bytes"]
                .as_u64()
                .is_some_and(|value| value <= maximum_message_bytes),
            "{id} has a plan-visible queue budget"
        );
        assert!(
            lesson["budgets"]["evidence_events"]
                .as_u64()
                .is_some_and(|value| value <= maximum_evidence_events),
            "{id} has bounded evidence"
        );
        for prerequisite in lesson["prerequisites"]
            .as_array()
            .expect("prerequisites are an array")
        {
            assert!(
                actual_ids.contains(prerequisite.as_str().expect("prerequisite is an id")),
                "{id} names an existing prerequisite"
            );
        }
        let panel = conduit_panel::parse(source)
            .unwrap_or_else(|error| panic!("{id} must parse through conduit-panel: {error}"));
        let registry = Registry::default();

        if let Some(expected_stdout) = lesson["expected_stdout"].as_str() {
            assert_eq!(lesson["validation"]["kind"], "stdout");
            assert_eq!(lesson["validation"]["value"], expected_stdout);
            let resolved = registry.resolve(&panel).unwrap_or_else(|error| {
                panic!("{id} must resolve through conduit-runtime: {error}")
            });
            let mut input = std::io::empty();
            let mut output = Vec::new();
            let mut error = Vec::new();
            resolved
                .run(&mut RunIo {
                    input: &mut input,
                    output: &mut output,
                    error: &mut error,
                })
                .unwrap_or_else(|run_error| panic!("{id} must run: {run_error}"));
            assert_eq!(
                String::from_utf8(output).expect("runtime stdout is UTF-8"),
                expected_stdout,
                "{id} expected stdout stays in sync with its source"
            );
            assert!(error.is_empty(), "{id} does not emit stderr");
        } else {
            let expected = lesson["expected_diagnostic"]
                .as_str()
                .expect("non-running lesson declares a diagnostic");
            let error = registry
                .resolve(&panel)
                .expect_err("lesson must produce its declared diagnostic");
            assert_eq!(error.code, expected, "{id} diagnostic stays in sync");
            assert_eq!(lesson["validation"]["kind"], "diagnostic");
            assert_eq!(lesson["validation"]["value"], expected);
        }
    }
}
