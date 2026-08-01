use std::collections::{BTreeMap, BTreeSet};

use bumpalo::Bump;
use conduit_compile::{InstalledProfile, compile_source};
use conduit_core::NodeContract;
use conduit_runtime::{
    CompiledInHostService, Handler, Registry, RegistryError, RunIo, RuntimeError,
    Value as RuntimeValue,
};
use conduit_web::{cancel_panel, run_panel};
use serde_json::Value;

const REQUIRED_TOUR_LESSONS: [&str; 37] = [
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
    "platform.panel-capsules",
    "library.typed-text-format",
    "library.bounded-media-values",
    "library.bounded-media-codecs",
    "library.bounded-learned-inference",
    "library.bounded-spatial-foundation",
    "library.bounded-brainstem-network",
    "library.standard-flow-control",
    "library.bounded-state",
    "library.bounded-supervision",
    "library.explicit-time",
    "library.explicit-data-boundaries",
    "library.bounded-process-exec",
    "library.bounded-sockets",
    "library.bounded-http-client",
    "library.bounded-filesystem",
    "library.evictable-storage-cache",
    "library.contract-package-imports",
    "platform.value-envelope-clock-feedback",
    "platform.resource-lease-effect-commit",
    "platform.workload-admission-deadline",
    "platform.cross-host-provider-conformance",
];

fn assert_current_panel_source(id: &str, source: &str) {
    let panel = conduit_panel::parse(source)
        .unwrap_or_else(|error| panic!("{id} must parse through conduit-panel: {error}"));
    assert_eq!(
        panel.version,
        conduit_panel::SOURCE_AST_SCHEMA_VERSION,
        "{id} must teach the current Panel grammar"
    );
}

#[test]
fn tour_lessons_declare_verified_browser_runnability() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    assert_eq!(manifest["schema"], "conduit.tour-lessons");

    let lessons = manifest["lessons"]
        .as_array()
        .expect("Tour lesson manifest contains lessons");
    let actual_ids = lessons
        .iter()
        .map(|lesson| lesson["id"].as_str().expect("lesson has an id"))
        .collect::<BTreeSet<_>>();
    let required_ids = REQUIRED_TOUR_LESSONS.into_iter().collect::<BTreeSet<_>>();
    assert_eq!(
        actual_ids, required_ids,
        "Tour contains every checked lesson exactly once"
    );
    let browser_plan: Value =
        serde_json::from_str(include_str!("../../../tour/public/browser-plan.json"))
            .expect("Tour browser plan is valid JSON");
    assert_eq!(
        browser_plan["schema"], "conduit.tour-browser-plan",
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
            lesson["accessibility"]["non_audio"]
                .as_str()
                .is_some_and(|alternative| !alternative.is_empty()),
            "{id} retains a non-audio proof"
        );
        assert!(
            lesson.get("expected_audio").is_none(),
            "{id} cannot claim audible proof without a bound provider"
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
        let runnability = &lesson["runnability"];
        assert_eq!(runnability["profile"], "browser", "{id} names its profile");

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
        assert_current_panel_source(id, source);
        let panel = conduit_panel::parse(source).expect("current lesson source already parsed");
        let registry = Registry::compatibility_demo();

        if lesson["validation"]["kind"] == "watch" {
            assert_eq!(runnability["state"], "runnable");
            assert_eq!(runnability["proof"], "browser-worker-exact-plan");
            assert!(lesson["expected_display"].is_null());
            assert!(
                lesson["validation"]["value"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty()),
                "{id} names its initial observed Watch value"
            );
            registry
                .resolve(&panel)
                .unwrap_or_else(|error| panic!("{id} resolves its Watch lesson: {error}"));
        } else if let Some(expected_display) = lesson["expected_display"].as_str() {
            assert_eq!(lesson["validation"]["value"], expected_display);
            if runnability["state"] == "runnable" {
                assert_eq!(runnability["proof"], "browser-worker-exact-plan");
                assert_eq!(lesson["validation"]["kind"], "display");
            } else {
                assert_eq!(runnability["state"], "illustrative/unavailable");
                assert_eq!(runnability["proof"], "canonical-run-rejection");
                assert_eq!(lesson["validation"]["kind"], "pedagogical-display");
                let expected = runnability["code"]
                    .as_str()
                    .filter(|code| code.starts_with("CND-"))
                    .expect("illustrative lesson names the exact production rejection");
                let raw = run_panel(source.to_owned());
                let result: Value = serde_json::from_str(&raw)
                    .unwrap_or_else(|error| panic!("{id}: {error}: {raw}"));
                assert_eq!(result["ok"], false, "{id} must remain non-runnable");
                let diagnostic = result["diagnostic"]
                    .as_str()
                    .or_else(|| result["stderr"].as_str())
                    .unwrap_or_default();
                assert!(
                    diagnostic.contains(expected),
                    "{id} must retain its intended rejection {expected}: {result}"
                );
            }
        } else {
            assert_eq!(runnability["state"], "contract-only");
            assert_eq!(runnability["proof"], "resolver-rejection");
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

#[test]
fn panel_capsule_lesson_keeps_program_plan_live_and_evidence_distinct() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "platform.panel-capsules")
        .expect("panel capsule lesson is selectable");
    let prose = lesson["prose"].as_str().unwrap();
    for term in ["capsule", "exact plan", "live epoch", "evidence"] {
        assert!(prose.contains(term), "lesson explains {term}");
    }
    let fields = lesson["presentation"]["patchbay_fields"]
        .as_array()
        .unwrap();
    for field in [
        "program_identity",
        "presentation_identity",
        "exact_plan_identity",
        "run_epoch",
        "evidence_cursor",
    ] {
        assert!(fields.iter().any(|value| value == field), "shows {field}");
    }
}

#[test]
fn bounded_media_lesson_exposes_exact_time_layout_pressure_and_terminal_facts() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-media-values")
        .expect("bounded media lesson is selectable");

    assert_eq!(lesson["runnability"]["state"], "runnable");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    for field in [
        "time_base",
        "timestamp",
        "duration",
        "planes_strides",
        "pressure",
        "cancellation",
        "error",
        "terminal",
    ] {
        assert!(
            lesson["presentation"]["patchbay_fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == field),
            "Patchbay exposes {field}"
        );
    }
    let contracts = lesson["library"]["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|contract| contract["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts,
        [
            "conduit.media/audio-frame/inspect",
            "conduit.media/audio-frame/literal",
            "conduit.media/video-frame/inspect",
            "conduit.media/video-frame/literal",
        ]
        .into_iter()
        .collect()
    );
    let scenarios = lesson["library"]["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 2);
    assert!(scenarios.iter().all(|scenario| {
        scenario["panel"]
            .as_str()
            .is_some_and(|panel| panel.starts_with("../../examples/media-"))
            && scenario["semantics"]
                .as_str()
                .is_some_and(|semantics| !semantics.is_empty())
    }));
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);

    let raw = run_panel(lesson["source"].as_str().unwrap().to_owned());
    let result: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(
        result["display"],
        "audio:s16le:48000:stereo:192video:rgb24:2x2"
    );
    assert_eq!(result["stdout"], "");
    assert_eq!(result["stderr"], "");

    let cancelled: Value =
        serde_json::from_str(&cancel_panel(lesson["source"].as_str().unwrap().to_owned())).unwrap();
    assert_eq!(cancelled["ok"], true, "{cancelled}");
    assert_eq!(cancelled["terminal"], "cancelled");
    assert_eq!(cancelled["stdout"], "");
    assert_eq!(cancelled["stderr"], "");
}

#[test]
fn bounded_media_codec_lesson_exposes_exact_provider_framing_and_flush_facts() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-media-codecs")
        .expect("bounded media codec lesson is selectable");

    assert_eq!(lesson["runnability"]["state"], "runnable");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    for field in [
        "provider_artifact",
        "profile_identity",
        "extradata_identity",
        "tracks",
        "packets",
        "reorder_depth",
        "retained_bytes",
        "metadata_entries",
        "output_bytes",
        "work",
        "pressure",
        "flush",
        "cancellation",
        "error",
        "terminal",
    ] {
        assert!(
            lesson["presentation"]["patchbay_fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == field),
            "Patchbay exposes {field}"
        );
    }
    let contracts = lesson["library"]["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|contract| contract["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts,
        [
            "conduit.media/audio/decode",
            "conduit.media/audio/encode",
            "conduit.media/container/demux",
            "conduit.media/container/mux",
            "conduit.media/container/probe",
            "conduit.media/wave/literal",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(lesson["library"]["scenarios"].as_array().unwrap().len(), 2);
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);

    let raw = run_panel(lesson["source"].as_str().unwrap().to_owned());
    let result: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(
        result["display"],
        "wave:pcm-s16le:48000:2:1-track:192-frames:812-bytes"
    );
    assert_eq!(result["stdout"], "");
    assert_eq!(result["stderr"], "");

    let cancelled: Value =
        serde_json::from_str(&cancel_panel(lesson["source"].as_str().unwrap().to_owned())).unwrap();
    assert_eq!(cancelled["ok"], true, "{cancelled}");
    assert_eq!(cancelled["terminal"], "cancelled");
    assert_eq!(cancelled["stdout"], "");
    assert_eq!(cancelled["stderr"], "");
}

#[test]
fn bounded_learned_inference_lesson_exposes_exact_model_and_runtime_facts() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-learned-inference")
        .expect("bounded learned inference lesson is selectable");

    assert_eq!(lesson["runnability"]["state"], "runnable");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    for field in [
        "semantic_model_identity",
        "artifact_identity",
        "input_schema_identity",
        "output_schema_identity",
        "runtime_identity",
        "device_identity",
        "resource_identity",
        "provider_artifact",
        "batch",
        "state_bytes",
        "retained_bytes",
        "work",
        "determinism",
        "tolerance",
        "pressure",
        "cancellation",
        "provider_loss",
        "error",
        "terminal",
    ] {
        assert!(
            lesson["presentation"]["patchbay_fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == field),
            "Patchbay exposes {field}"
        );
    }
    let contracts = lesson["library"]["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|contract| contract["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts,
        [
            "learned/infer",
            "learned/model/literal",
            "learned/tensor/inspect",
            "learned/tensor/literal",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(lesson["library"]["scenarios"].as_array().unwrap().len(), 2);
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);

    let raw = run_panel(lesson["source"].as_str().unwrap().to_owned());
    let result: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["display"], "learned:i16:1x2:[35,-3]");
    assert_eq!(result["stdout"], "");
    assert_eq!(result["stderr"], "");

    let cancelled: Value =
        serde_json::from_str(&cancel_panel(lesson["source"].as_str().unwrap().to_owned())).unwrap();
    assert_eq!(cancelled["ok"], true, "{cancelled}");
    assert_eq!(cancelled["terminal"], "cancelled");
    assert_eq!(cancelled["stdout"], "");
    assert_eq!(cancelled["stderr"], "");
}

#[test]
fn bounded_spatial_lesson_exposes_frames_calibration_and_finite_history() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-spatial-foundation")
        .expect("bounded spatial lesson is selectable");

    assert_eq!(lesson["runnability"]["state"], "runnable");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    for field in [
        "source_frame",
        "target_frame",
        "unit",
        "handedness",
        "axes",
        "clock",
        "validity",
        "uncertainty",
        "calibration_identity",
        "provenance_identity",
        "history_values",
        "interpolation_window",
        "numeric_work",
        "queue_bytes",
        "pressure",
        "cancellation",
        "error",
        "terminal",
    ] {
        assert!(
            lesson["presentation"]["patchbay_fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == field),
            "Patchbay exposes {field}"
        );
    }
    let contracts = lesson["library"]["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|contract| contract["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts,
        [
            "spatial/camera/project",
            "spatial/camera/unproject",
            "spatial/point/inspect",
            "spatial/point/literal",
            "spatial/transform/apply",
            "spatial/transform/compose",
            "spatial/transform/interpolate",
            "spatial/transform/invert",
            "spatial/transform/literal",
            "spatial/transform/lookup",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(lesson["library"]["scenarios"].as_array().unwrap().len(), 2);
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);

    let raw = run_panel(lesson["source"].as_str().unwrap().to_owned());
    let result: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(
        result["display"],
        "spatial:point:camera:[1010,520,10030]:clock/fixture@11:uncertainty=0"
    );
    assert_eq!(result["stdout"], "");
    assert_eq!(result["stderr"], "");

    let cancelled: Value =
        serde_json::from_str(&cancel_panel(lesson["source"].as_str().unwrap().to_owned())).unwrap();
    assert_eq!(cancelled["ok"], true, "{cancelled}");
    assert_eq!(cancelled["terminal"], "cancelled");
    assert_eq!(cancelled["stdout"], "");
    assert_eq!(cancelled["stderr"], "");
}

#[test]
fn bounded_brainstem_network_lesson_keeps_observation_and_robot_authority_separate() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-brainstem-network")
        .expect("brainstem network lesson is selectable");

    assert_eq!(lesson["runnability"]["state"], "runnable");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    for field in [
        "compiled_inventory",
        "provider_observation",
        "observation_validity",
        "lease_generation",
        "lease_expiry",
        "icmp_rate",
        "record_expiry",
        "pressure",
        "cancellation",
        "provider_loss",
        "routing",
        "nat",
        "robot_authority",
        "terminal",
    ] {
        assert!(
            lesson["presentation"]["patchbay_fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == field),
            "Patchbay exposes {field}"
        );
    }
    let contracts = lesson["library"]["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|contract| contract["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts,
        [
            "net/dhcp/server",
            "net/dns-sd",
            "net/reachability",
            "net/wifi/access-point",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(lesson["library"]["scenarios"].as_array().unwrap().len(), 5);
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);
    for forbidden in ["CreateUart", "motor_grant", "possession_grant"] {
        assert!(!lesson["source"].as_str().unwrap().contains(forbidden));
    }

    let raw = run_panel(lesson["source"].as_str().unwrap().to_owned());
    let result: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(
        result["display"],
        "wifi-ap:pete-fixture:192.168.4.1:clients=8:no-route:no-bridge:no-nat"
    );
    assert_eq!(result["stdout"], "");
    assert_eq!(result["stderr"], "");
}

#[test]
fn standard_flow_lesson_exposes_exact_semantics_and_accessible_evidence() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.standard-flow-control")
        .expect("standard flow lesson is selectable");

    assert_eq!(lesson["runnability"]["state"], "runnable");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    for field in [
        "pressure",
        "occupancy",
        "ordering",
        "retained_values",
        "terminal",
    ] {
        assert!(
            lesson["presentation"]["patchbay_fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == field),
            "Patchbay exposes {field}"
        );
    }
    let contracts = lesson["library"]["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|contract| contract["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts,
        [
            "conduit.std/gate",
            "conduit.std/merge",
            "conduit.std/select",
            "conduit.std/tee",
            "conduit.std/zip",
        ]
        .into_iter()
        .collect()
    );
    let scenarios = lesson["library"]["scenarios"]
        .as_array()
        .expect("standalone and composition scenarios are selectable");
    assert_eq!(scenarios.len(), 6);
    assert!(scenarios.iter().all(|scenario| {
        scenario["panel"]
            .as_str()
            .is_some_and(|panel| panel.starts_with("../../examples/flow-"))
            && scenario["semantics"]
                .as_str()
                .is_some_and(|semantics| !semantics.is_empty())
    }));
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);
    assert!(
        lesson["accessibility"]["non_audio"]
            .as_str()
            .is_some_and(|text| text.contains("pressure") && text.contains("terminal"))
    );

    let raw = run_panel(lesson["source"].as_str().unwrap().to_owned());
    let result: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["display"], "rightright");
    assert_eq!(result["stdout"], "");
    assert_eq!(result["stderr"], "");
}

#[test]
fn bounded_state_lesson_exposes_bounds_eviction_restart_and_terminal_facts() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-state")
        .expect("bounded state lesson is selectable");

    assert_eq!(lesson["runnability"]["state"], "runnable");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    for field in [
        "state_schema",
        "retained_values",
        "retained_bytes",
        "eviction",
        "hit_miss",
        "terminal",
    ] {
        assert!(
            lesson["presentation"]["patchbay_fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == field),
            "Patchbay exposes {field}"
        );
    }
    let contracts = lesson["library"]["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|contract| contract["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts,
        ["state/cache", "state/cell", "state/deduplicate"]
            .into_iter()
            .collect()
    );
    let scenarios = lesson["library"]["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 4);
    assert!(scenarios.iter().all(|scenario| {
        scenario["panel"]
            .as_str()
            .is_some_and(|panel| panel.starts_with("../../examples/state-"))
            && scenario["semantics"]
                .as_str()
                .is_some_and(|semantics| !semantics.is_empty())
    }));

    let raw = run_panel(lesson["source"].as_str().unwrap().to_owned());
    let result: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["display"], "stored,alpha");
    assert_eq!(result["stdout"], "");
    assert_eq!(result["stderr"], "");
}

#[test]
fn bounded_supervision_lesson_exposes_attempt_backoff_breaker_and_authority_facts() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-supervision")
        .expect("bounded supervision lesson is selectable");

    assert_eq!(lesson["runnability"]["state"], "runnable");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    for field in [
        "terminal_schema",
        "attempt",
        "generation",
        "deadline_tick",
        "backoff",
        "breaker_state",
        "observation_window",
        "decision",
        "terminal",
    ] {
        assert!(
            lesson["presentation"]["patchbay_fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == field),
            "Patchbay exposes {field}"
        );
    }
    let contracts = lesson["library"]["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|contract| contract["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts,
        ["supervision/circuit-breaker", "supervision/retry"]
            .into_iter()
            .collect()
    );
    assert!(lesson["library"]["wrong"].as_str().is_some_and(|wrong| {
        wrong.contains("provider fallback") && wrong.contains("separate backoff box")
    }));
    let scenarios = lesson["library"]["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 3);
    assert!(scenarios.iter().all(|scenario| {
        scenario["panel"]
            .as_str()
            .is_some_and(|panel| panel.starts_with("../../examples/supervision-"))
            && scenario["semantics"]
                .as_str()
                .is_some_and(|semantics| !semantics.is_empty())
    }));

    let raw = run_panel(lesson["source"].as_str().unwrap().to_owned());
    let result: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["display"], "first,probe");
    assert_eq!(result["stdout"], "");
    assert_eq!(result["stderr"], "");
}

#[test]
fn explicit_time_lesson_exposes_clock_timer_loss_and_terminal_facts() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.explicit-time")
        .expect("explicit time lesson is selectable");

    assert_eq!(lesson["runnability"]["state"], "runnable");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    for field in [
        "clock",
        "deadline_tick",
        "pressure",
        "retained_values",
        "terminal",
    ] {
        assert!(
            lesson["presentation"]["patchbay_fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == field),
            "Patchbay exposes {field}"
        );
    }
    let contracts = lesson["library"]["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|contract| contract["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts,
        [
            "time/debounce",
            "time/delay",
            "time/throttle",
            "time/timeout",
        ]
        .into_iter()
        .collect()
    );
    let scenarios = lesson["library"]["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 5);
    assert!(scenarios.iter().all(|scenario| {
        scenario["panel"]
            .as_str()
            .is_some_and(|panel| panel.starts_with("../../examples/time-"))
            && scenario["semantics"]
                .as_str()
                .is_some_and(|semantics| !semantics.is_empty())
    }));
    assert!(
        lesson["accessibility"]["non_audio"]
            .as_str()
            .is_some_and(|text| text.contains("timer") && text.contains("terminal"))
    );

    let raw = run_panel(lesson["source"].as_str().unwrap().to_owned());
    let result: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["display"], "second");
    assert_eq!(result["stdout"], "");
    assert_eq!(result["stderr"], "");
}

#[test]
fn live_ticker_lesson_covers_standalone_composition_and_accessible_lifecycle() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "panels.tiny-instrument")
        .expect("live ticker lesson is selectable");

    assert_eq!(lesson["execution"], "continuous-watch");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);
    assert!(
        lesson["prose"].as_str().is_some_and(|prose|
            prose.contains("monotonic cursors")
                && prose.contains("explicit gap")
                && prose.contains("Strict audit")
                && prose.contains("does not execute the graph or own the evidence store"))
    );
    assert!(
        lesson["accessibility"]["keyboard"]
            .as_str()
            .is_some_and(|text| text.contains("Shift+Enter") && text.contains("ArrowRight"))
    );
    assert_eq!(lesson["library"]["contracts"][0]["id"], "time/ticker");

    let scenarios = lesson["library"]["scenarios"].as_array().unwrap();
    for required in ["ticker-standalone", "ticker-watch-composition"] {
        assert!(
            scenarios.iter().any(|scenario| scenario["id"] == required),
            "lesson includes {required}"
        );
    }
    let fields = lesson["presentation"]["patchbay_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for required in [
        "run_state",
        "timer_deadline",
        "host_wake",
        "latest_public_text",
        "evidence_cursor",
        "retention_gap",
        "reconnect",
        "cleanup",
        "terminal",
    ] {
        assert!(fields.contains(required), "ticker table exposes {required}");
    }
}

#[test]
fn contract_package_import_lesson_separates_names_meaning_and_availability() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.contract-package-imports")
        .expect("contract-package import lesson is selectable");
    assert_eq!(lesson["runnability"]["state"], "contract-only");
    assert_eq!(lesson["expected_diagnostic"], "CND-IMP-001");
    assert!(lesson["prose"].as_str().unwrap().contains(
        "imports name the part; the plan determines whether a particular machine can provide it"
    ));
    let imports = &lesson["imports"];
    assert_eq!(imports["alias"], "split");
    assert_eq!(imports["canonical_id"], "conduit.dev/std/tee");
    assert!(
        imports["descriptor_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert_eq!(imports["availability"], "contract-only");
    assert_eq!(imports["error"]["code"], "CND-IPK-004");
    let lock: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/contract-package-imports/contract-package-lock.json"
    ))
    .unwrap();
    let package = &lock["packages"][0];
    assert_eq!(imports["package_id"], package["package_id"]);
    assert_eq!(imports["package_digest"], package["artifact_digest"]);
    assert_eq!(
        imports["canonical_id"],
        package["exports"][0]["canonical_id"]
    );
    assert_eq!(
        imports["descriptor_hash"],
        package["exports"][0]["descriptor_hash"]
    );
}

#[test]
fn typed_text_format_library_lesson_runs_every_checked_scenario() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let browser_plan: Value =
        serde_json::from_str(include_str!("../../../tour/public/browser-plan.json"))
            .expect("Tour browser plan is valid JSON");
    let maximum_evidence = browser_plan["bounds"]["maximum_evidence_events"]
        .as_u64()
        .unwrap();
    let maximum_scheduler_events = browser_plan["bounds"]["maximum_scheduler_events"]
        .as_u64()
        .unwrap();
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.typed-text-format")
        .expect("typed text format is selectable");
    let lesson_tick_budget = lesson["budgets"]["runtime_ticks"].as_u64().unwrap();
    let library = &lesson["library"];
    assert_eq!(library["schema"], "conduit.tour-library-lesson");
    assert_eq!(library["profile"], "browser-dedicated-worker");
    for field in ["summary", "what", "when", "wrong", "provider"] {
        assert!(
            library[field]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "library lesson has {field}"
        );
    }
    let contracts = library["contracts"]
        .as_array()
        .expect("library contracts are selectable")
        .iter()
        .map(|contract| contract["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts,
        [
            "std/format-values/literal",
            "std/text/format",
            "std/text/join",
            "std/text/lines",
        ]
        .into_iter()
        .collect()
    );
    assert!(
        library["contracts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|contract| {
                contract["instance"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty())
            }),
        "each exported contract selects a concrete standalone instance"
    );
    assert!(
        library["docs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|link| link.as_str().is_some_and(|link| !link.is_empty()))
    );
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for link in library["docs"].as_array().unwrap() {
        let path = link.as_str().unwrap().split('#').next().unwrap();
        let path = path
            .strip_prefix("../../")
            .expect("library documentation is repository-relative");
        assert!(
            root.join(path).is_file(),
            "library documentation {path} exists"
        );
    }

    let scenarios = library["scenarios"].as_array().expect("library scenarios");
    assert_eq!(scenarios.len(), 6);
    assert_eq!(lesson["source"], scenarios[0]["source"]);
    assert!(
        scenarios[1]["source"]
            .as_str()
            .unwrap()
            .contains("text/uppercase"),
        "composition scenario uses another node"
    );

    for scenario in scenarios {
        let id = scenario["id"].as_str().unwrap();
        let source = scenario["source"].as_str().unwrap();
        assert_current_panel_source(id, source);
        let raw = if scenario["execution"] == "cancel-before-first-step" {
            cancel_panel(source.to_owned())
        } else {
            run_panel(source.to_owned())
        };
        let result: Value =
            serde_json::from_str(&raw).unwrap_or_else(|error| panic!("{id}: {error}: {raw}"));
        let validation = &scenario["validation"];
        let expected = validation["value"].as_str().unwrap();
        match validation["kind"].as_str().unwrap() {
            "display" => {
                assert_eq!(result["ok"], true, "{id}: {result}");
                assert_eq!(result["display"], expected, "{id}: {result}");
            }
            "terminal" => {
                assert_eq!(result["ok"], true, "{id}: {result}");
                assert_eq!(result["terminal"], expected, "{id}: {result}");
            }
            "diagnostic" => {
                let diagnostic = result["diagnostic"]
                    .as_str()
                    .or_else(|| result["stderr"].as_str())
                    .unwrap_or_default();
                assert!(diagnostic.contains(expected), "{id}: {result}");
            }
            kind => panic!("unknown validation kind {kind}"),
        }
        if let Some(evidence) = result["evidence"].as_array() {
            assert!(!evidence.is_empty(), "{id} has exact ordered evidence");
            assert!(
                evidence.len() as u64 <= maximum_evidence,
                "{id} evidence stays inside its browser-plan bound"
            );
            assert!(
                evidence
                    .iter()
                    .all(|event| event["tick"].as_u64().unwrap() <= lesson_tick_budget),
                "{id} evidence stays inside its lesson tick budget"
            );
            assert!(
                evidence
                    .iter()
                    .any(|event| event["event_kind"] == "terminal"),
                "{id} has one visible terminal event"
            );
            assert_eq!(result["patchbay"]["evidence"], result["evidence"]);
        }
        if let Some(scheduler_events) = result["scheduler_event_count"].as_u64() {
            assert!(
                scheduler_events <= maximum_scheduler_events,
                "{id} scheduler observations stay inside the browser-plan bound"
            );
        }
    }
}

#[test]
fn value_envelope_platform_lesson_is_fixture_backed_and_executable() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/c5/value-envelope-clock-feedback.json"
    ))
    .expect("value envelope fixture is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "platform.value-envelope-clock-feedback")
        .expect("platform lesson is selectable");
    let platform = &lesson["platform"];
    assert_eq!(platform["schema"], "conduit.tour-platform-lesson");
    for field in ["what", "when", "wrong"] {
        assert!(
            platform[field]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "platform lesson explains {field}"
        );
    }

    let result: Value =
        serde_json::from_str(&run_panel(lesson["source"].as_str().unwrap().to_owned())).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["display"], lesson["expected_display"]);
    assert_eq!(result["terminal"], "succeeded");

    let cases = fixture["cases"].as_array().unwrap();
    for profile in platform["profiles"].as_array().unwrap() {
        let fixture_case = cases
            .iter()
            .find(|case| case["id"] == profile["fixture_case"])
            .expect("lesson profile names a conformance case");
        match profile["admission"].as_str().unwrap() {
            "accepted" => assert_eq!(fixture_case["expected"]["accepted"], true),
            "rejected" => {
                assert_eq!(fixture_case["expected"]["accepted"], false);
                assert_eq!(fixture_case["expected"]["code"], profile["code"]);
            }
            other => panic!("unknown admission state {other}"),
        }
        if let Some(bytes) = profile["maximum_retained_bytes"].as_u64() {
            assert_eq!(
                fixture_case["expected"]["retained_bytes"].as_u64(),
                Some(bytes)
            );
        }
    }

    let fields = lesson["presentation"]["patchbay_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        fields,
        [
            "clock_conversions",
            "feedback_boundaries",
            "value_envelopes"
        ]
        .into_iter()
        .collect()
    );

    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for link in [
        platform["fixture"].as_str().unwrap(),
        platform["panel"].as_str().unwrap(),
    ]
    .into_iter()
    .chain(
        platform["docs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|link| link.as_str().unwrap()),
    ) {
        let path = link
            .split('#')
            .next()
            .unwrap()
            .strip_prefix("../../")
            .unwrap();
        assert!(root.join(path).is_file(), "lesson reference {path} exists");
    }
}

#[test]
fn resource_lease_platform_lesson_is_fixture_backed_and_executable() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/c5/resource-lease-effect-commit.json"
    ))
    .expect("resource lease fixture is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "platform.resource-lease-effect-commit")
        .expect("resource lease lesson is selectable");
    let platform = &lesson["platform"];
    assert_eq!(platform["schema"], "conduit.tour-platform-lesson");

    let result: Value =
        serde_json::from_str(&run_panel(lesson["source"].as_str().unwrap().to_owned())).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["display"], lesson["expected_display"]);
    assert_eq!(result["terminal"], "succeeded");

    let cases = fixture["cases"].as_array().unwrap();
    for profile in platform["profiles"].as_array().unwrap() {
        let fixture_case = cases
            .iter()
            .find(|case| case["id"] == profile["fixture_case"])
            .expect("lesson profile names a conformance case");
        match profile["admission"].as_str().unwrap() {
            "accepted" => assert_eq!(fixture_case["expected"]["accepted"], true),
            "rejected" => {
                assert_eq!(fixture_case["expected"]["accepted"], false);
                assert_eq!(fixture_case["expected"]["code"], profile["code"]);
            }
            other => panic!("unknown admission state {other}"),
        }
    }

    let fields = lesson["presentation"]["patchbay_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        fields,
        ["effect_commit_profiles", "resource_leases"]
            .into_iter()
            .collect()
    );
}

#[test]
fn workload_platform_lesson_keeps_guarantees_distinct_from_observations() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/c5/workload-admission-deadline.json"
    ))
    .expect("workload fixture is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "platform.workload-admission-deadline")
        .expect("workload lesson is selectable");
    let platform = &lesson["platform"];
    assert_eq!(platform["schema"], "conduit.tour-platform-lesson");

    let result: Value =
        serde_json::from_str(&run_panel(lesson["source"].as_str().unwrap().to_owned())).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["display"], lesson["expected_display"]);
    assert_eq!(result["terminal"], "succeeded");

    let cases = fixture["cases"].as_array().unwrap();
    for profile in platform["profiles"].as_array().unwrap() {
        let fixture_case = cases
            .iter()
            .find(|case| case["id"] == profile["fixture_case"])
            .expect("lesson profile names a conformance case");
        match profile["admission"].as_str().unwrap() {
            "accepted" => assert_eq!(fixture_case["expected"]["accepted"], true),
            "rejected" => {
                assert_eq!(fixture_case["expected"]["accepted"], false);
                assert_eq!(fixture_case["expected"]["code"], profile["code"]);
            }
            other => panic!("unknown admission state {other}"),
        }
    }
    let fields = lesson["presentation"]["patchbay_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(fields, ["bindings", "workloads"].into_iter().collect());
}

fn cross_host_profile_registry(profile: &str) -> Registry {
    let mut registry = Registry::hosted_primitives();
    match profile {
        "deterministic-host" => {
            conduit_media::register_deterministic_media_providers(&mut registry)
                .expect("deterministic media providers are available");
            conduit_media::register_deterministic_codec_providers(&mut registry)
                .expect("deterministic codec providers are available");
        }
        "provider-fixture-alpha" | "explicit-adapter-fixture" => {
            conduit_media::register_deterministic_media_providers(&mut registry)
                .expect("deterministic media providers are available");
            conduit_media::register_media_codec_contracts(&mut registry);
            if profile == "provider-fixture-alpha" {
                register_provider_fixture_alpha_codec_providers(&mut registry)
                    .expect("provider fixture alpha is available");
            } else {
                register_explicit_adapter_codec_fixture_providers(&mut registry)
                    .expect("explicit adapter codec fixture providers are available");
            }
        }
        "provider-fixture-beta" => {
            conduit_media::register_deterministic_media_providers(&mut registry)
                .expect("deterministic media providers are available");
            conduit_media::register_media_codec_contracts(&mut registry);
            register_provider_fixture_beta_codec_providers(&mut registry)
                .expect("provider fixture beta is available");
        }
        other => panic!("unknown cross-host profile {other}"),
    }
    registry
}

fn register_codec_fixture_provider(
    registry: &mut Registry,
    providers: &[(
        &'static NodeContract<'static>,
        &'static str,
        &'static str,
        &'static str,
        &'static [u8],
    )],
) -> Result<(), RegistryError> {
    struct FixtureCodecProvider;

    impl Handler for FixtureCodecProvider {
        fn run(
            &mut self,
            _node: &conduit_panel::Node,
            _inputs: &[RuntimeValue],
            _io: &mut RunIo<'_>,
        ) -> Result<Vec<RuntimeValue>, RuntimeError> {
            Ok(Vec::new())
        }
    }
    static NO_AUTHORITIES: [conduit_core::SemanticHash; 0] = [];
    for (contract, implementation_id, artifact_id, entrypoint, source_bytes) in providers {
        registry.register_compiled_in_host_service(CompiledInHostService {
            contract,
            implementation_id,
            artifact_id,
            entrypoint,
            source_bytes,
            required_authorities: &NO_AUTHORITIES,
            factory: || Box::new(FixtureCodecProvider),
            validate_config: |_node: &conduit_panel::Node| Ok(()),
        })?;
    }
    Ok(())
}

fn register_provider_fixture_alpha_codec_providers(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_codec_fixture_provider(
        registry,
        &[
            (
                &conduit_media::WAVE_LITERAL_CONTRACT,
                "conduit.media/wave-literal-provider-fixture-alpha",
                "conduit.media/wave-literal-provider-fixture-alpha-artifact",
                "media-wave-literal-provider-fixture-alpha",
                b"conduit.media/wave-literal-provider-fixture-alpha.source",
            ),
            (
                &conduit_media::DEMUX_CONTRACT,
                "conduit.media/wave-demux-provider-fixture-alpha",
                "conduit.media/wave-demux-provider-fixture-alpha-artifact",
                "media-wave-demux-provider-fixture-alpha",
                b"conduit.media/wave-demux-provider-fixture-alpha.source",
            ),
            (
                &conduit_media::DECODE_CONTRACT,
                "conduit.media/pcm-decode-provider-fixture-alpha",
                "conduit.media/pcm-decode-provider-fixture-alpha-artifact",
                "media-pcm-decode-provider-fixture-alpha",
                b"conduit.media/pcm-decode-provider-fixture-alpha.source",
            ),
        ],
    )
}

fn register_provider_fixture_beta_codec_providers(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_codec_fixture_provider(
        registry,
        &[
            (
                &conduit_media::WAVE_LITERAL_CONTRACT,
                "conduit.media/wave-literal-provider-fixture-beta",
                "conduit.media/wave-literal-provider-fixture-beta-artifact",
                "media-wave-literal-provider-fixture-beta",
                b"conduit.media/wave-literal-provider-fixture-beta.source",
            ),
            (
                &conduit_media::DEMUX_CONTRACT,
                "conduit.media/wave-demux-provider-fixture-beta",
                "conduit.media/wave-demux-provider-fixture-beta-artifact",
                "media-wave-demux-provider-fixture-beta",
                b"conduit.media/wave-demux-provider-fixture-beta.source",
            ),
            (
                &conduit_media::DECODE_CONTRACT,
                "conduit.media/pcm-decode-provider-fixture-beta",
                "conduit.media/pcm-decode-provider-fixture-beta-artifact",
                "media-pcm-decode-provider-fixture-beta",
                b"conduit.media/pcm-decode-provider-fixture-beta.source",
            ),
        ],
    )
}

fn register_explicit_adapter_codec_fixture_providers(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_codec_fixture_provider(
        registry,
        &[
            (
                &conduit_media::WAVE_LITERAL_CONTRACT,
                "conduit.media/wave-literal-explicit-adapter-fixture",
                "conduit.media/wave-literal-explicit-adapter-fixture-artifact",
                "media-wave-literal-explicit-adapter-fixture",
                b"conduit.media/wave-literal-explicit-adapter-fixture.source",
            ),
            (
                &conduit_media::DEMUX_CONTRACT,
                "conduit.media/wave-demux-explicit-adapter-fixture",
                "conduit.media/wave-demux-explicit-adapter-fixture-artifact",
                "media-wave-demux-explicit-adapter-fixture",
                b"conduit.media/wave-demux-explicit-adapter-fixture.source",
            ),
            (
                &conduit_media::DECODE_CONTRACT,
                "conduit.media/pcm-decode-explicit-adapter-fixture",
                "conduit.media/pcm-decode-explicit-adapter-fixture-artifact",
                "media-pcm-decode-explicit-adapter-fixture",
                b"conduit.media/pcm-decode-explicit-adapter-fixture.source",
            ),
        ],
    )
}

fn cross_host_media_bindings(
    source: &str,
    profile: &str,
) -> BTreeMap<String, (String, String, String, String)> {
    let registry = cross_host_profile_registry(profile);
    let mut installed =
        InstalledProfile::observe_registry(source, &registry).unwrap_or_else(|error| {
            panic!("profile {profile} must produce a compatible installed profile: {error}")
        });
    let profile_observation = match profile {
        "deterministic-host" => ("conduit/conduct-host-observation", "conduit/conduct-host"),
        "provider-fixture-alpha" | "provider-fixture-beta" | "explicit-adapter-fixture" => (
            "conduit/cross-host/provider-fixture-observation",
            "conduit/cross-host/provider-fixture-host",
        ),
        _ => panic!("unknown cross-host profile {profile}"),
    };
    installed.input.candidates.iter_mut().for_each(|candidate| {
        candidate.host_report.id = profile_observation.0.to_owned();
        candidate.host_report.host = profile_observation.1.to_owned();
    });
    installed
        .input
        .seal()
        .expect("cross-host media candidate profile is sealed");
    let document = compile_source(source, &installed.input)
        .unwrap_or_else(|error| panic!("profile {profile} must compile the media graph: {error}"));
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap_or_else(|error| {
        panic!("profile {profile} must produce an execution plan: {error}")
    });
    let bindings = installed
        .bindings(&plan)
        .unwrap_or_else(|error| panic!("profile {profile} must compute exact bindings: {error}"));
    let _ = bindings;

    plan.nodes
        .iter()
        .filter_map(|node| {
            let contract = node.contract.id.as_str();
            if !contract.starts_with("conduit.media/") {
                return None;
            }
            Some((
                contract.to_owned(),
                (
                    node.implementation.id.to_string(),
                    node.contract.semantic_hash.to_string(),
                    node.artifact.to_string(),
                    node.host_observation.to_string(),
                ),
            ))
        })
        .collect()
}

#[test]
fn cross_host_provider_lesson_retains_the_complete_exact_chain() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/c5/cross-host-provider-conformance.json"
    ))
    .expect("cross-host fixture is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "platform.cross-host-provider-conformance")
        .expect("cross-host provider lesson is selectable");
    let result: Value =
        serde_json::from_str(&run_panel(lesson["source"].as_str().unwrap().to_owned())).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["display"], lesson["expected_display"]);
    assert_eq!(result["terminal"], "succeeded");

    let source = lesson["source"].as_str().unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    let mut accepted_profile_ids = BTreeSet::new();
    let mut accepted_profile_bindings = BTreeMap::new();
    let mut accepted_boundaries = BTreeSet::new();
    let mut accepted_type_relations = BTreeSet::new();
    let mut rejected_profile_codes = BTreeMap::new();
    for profile in lesson["platform"]["profiles"].as_array().unwrap() {
        let fixture_case = cases
            .iter()
            .find(|case| case["id"] == profile["fixture_case"])
            .expect("lesson profile names a conformance case");
        if profile["admission"] == "accepted" {
            assert_eq!(fixture_case["expected"], "bound");
            let profile_id = profile["id"]
                .as_str()
                .expect("platform profile names its accepted profile id");
            accepted_profile_ids.insert(profile_id.to_owned());
            accepted_profile_bindings.insert(
                profile_id.to_owned(),
                cross_host_media_bindings(source, profile_id),
            );
            accepted_boundaries.insert(
                fixture_case["boundary"]
                    .as_str()
                    .expect("fixture case names a boundary"),
            );
            accepted_type_relations.insert(
                fixture_case["type_relation"]
                    .as_str()
                    .expect("fixture case names a type relation"),
            );
        } else {
            let profile_id = profile["id"].as_str().expect("platform profile has id");
            let profile_code = profile["code"]
                .as_str()
                .expect("platform profile has rejection code");
            assert_eq!(fixture_case["expected"], profile_code);
            rejected_profile_codes.insert(profile_id.to_owned(), profile_code.to_owned());
        }
    }
    assert_eq!(
        accepted_profile_ids,
        [
            "deterministic-host",
            "provider-fixture-alpha",
            "provider-fixture-beta",
            "explicit-adapter-fixture"
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    assert!(accepted_boundaries.contains("native"));
    assert_eq!(accepted_boundaries, ["native"].into_iter().collect());
    assert!(accepted_type_relations.contains("exact"));
    assert!(accepted_type_relations.contains("different-explicit-adapter"));
    assert_eq!(rejected_profile_codes.len(), 3);
    assert_eq!(
        rejected_profile_codes.get("firmware-unsupported"),
        Some(&"CND-HCF-005".to_string())
    );
    assert_eq!(
        rejected_profile_codes.get("describe-only"),
        Some(&"CND-HCF-003".to_string())
    );
    assert_eq!(
        rejected_profile_codes.get("provider-lost"),
        Some(&"CND-HCF-007".to_string())
    );
    let deterministic_bindings = accepted_profile_bindings
        .get("deterministic-host")
        .expect("deterministic-host profile is accepted");
    let fixture_alpha_bindings = accepted_profile_bindings
        .get("provider-fixture-alpha")
        .expect("provider fixture alpha is accepted");
    let fixture_beta_bindings = accepted_profile_bindings
        .get("provider-fixture-beta")
        .expect("provider fixture beta is accepted");
    let explicit_bindings = accepted_profile_bindings
        .get("explicit-adapter-fixture")
        .expect("explicit-adapter-fixture profile is accepted");
    let deterministic_decode = deterministic_bindings["conduit.media/audio/decode"]
        .0
        .clone();
    let fixture_alpha_decode = fixture_alpha_bindings["conduit.media/audio/decode"]
        .0
        .clone();
    let fixture_beta_decode = fixture_beta_bindings["conduit.media/audio/decode"]
        .0
        .clone();
    let explicit_decode = explicit_bindings["conduit.media/audio/decode"].0.clone();
    let deterministic_artifact = deterministic_bindings["conduit.media/audio/decode"]
        .2
        .clone();
    let fixture_alpha_artifact = fixture_alpha_bindings["conduit.media/audio/decode"]
        .2
        .clone();
    let fixture_beta_artifact = fixture_beta_bindings["conduit.media/audio/decode"]
        .2
        .clone();
    let explicit_artifact = explicit_bindings["conduit.media/audio/decode"].2.clone();
    let deterministic_observation = deterministic_bindings["conduit.media/audio/decode"]
        .3
        .clone();
    let fixture_alpha_observation = fixture_alpha_bindings["conduit.media/audio/decode"]
        .3
        .clone();
    let fixture_beta_observation = fixture_beta_bindings["conduit.media/audio/decode"]
        .3
        .clone();
    let explicit_observation = explicit_bindings["conduit.media/audio/decode"].3.clone();
    assert_ne!(deterministic_decode, fixture_alpha_decode);
    assert_ne!(deterministic_decode, fixture_beta_decode);
    assert_ne!(fixture_alpha_decode, fixture_beta_decode);
    assert_ne!(fixture_alpha_decode, explicit_decode);
    assert_ne!(deterministic_artifact, fixture_alpha_artifact);
    assert_ne!(deterministic_artifact, fixture_beta_artifact);
    assert_ne!(fixture_alpha_artifact, fixture_beta_artifact);
    assert_ne!(explicit_artifact, deterministic_artifact);
    assert_ne!(explicit_artifact, fixture_alpha_artifact);
    assert_ne!(explicit_artifact, fixture_beta_artifact);
    assert!(!deterministic_observation.is_empty());
    assert_ne!(fixture_alpha_observation, deterministic_observation);
    assert_ne!(fixture_beta_observation, deterministic_observation);
    assert_ne!(explicit_observation, deterministic_observation);
    assert_eq!(fixture_alpha_observation, fixture_beta_observation);
    assert_eq!(fixture_alpha_observation, explicit_observation);
    let deterministic_contract_hashes: BTreeSet<_> = deterministic_bindings
        .iter()
        .map(|(contract, (_, identity, _, _))| (contract.as_str(), identity.as_str()))
        .collect();
    assert_eq!(
        deterministic_contract_hashes,
        explicit_bindings
            .iter()
            .map(|(contract, (_, identity, _, _))| (contract.as_str(), identity.as_str()))
            .collect()
    );
    let fixture_alpha_contract_hashes: BTreeSet<_> = fixture_alpha_bindings
        .iter()
        .map(|(contract, (_, identity, _, _))| (contract.as_str(), identity.as_str()))
        .collect();
    let fixture_beta_contract_hashes: BTreeSet<_> = fixture_beta_bindings
        .iter()
        .map(|(contract, (_, identity, _, _))| (contract.as_str(), identity.as_str()))
        .collect();
    assert_eq!(
        deterministic_contract_hashes.len(),
        deterministic_bindings.len()
    );
    assert_eq!(fixture_alpha_contract_hashes, deterministic_contract_hashes);
    assert_eq!(fixture_beta_contract_hashes, deterministic_contract_hashes);
    let fields = lesson["presentation"]["patchbay_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        fields,
        [
            "exact_bindings",
            "extensions",
            "mandatory_facts",
            "optional_providers"
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn bounded_process_lesson_is_selectable_and_honest_about_browser_support() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-process-exec")
        .expect("bounded process lesson is selectable");
    assert_eq!(lesson["runnability"]["state"], "contract-only");
    assert_eq!(lesson["runnability"]["proof"], "resolver-rejection");
    assert_eq!(lesson["expected_diagnostic"], "CND-IMP-001");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");

    let contracts = lesson["library"]["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|contract| contract["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts,
        [
            "conduit.host/process/exec",
            "io/stderr-stream",
            "io/stdin-stream",
            "io/stdout-stream",
        ]
        .into_iter()
        .collect()
    );
    let fields = lesson["presentation"]["patchbay_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for required in [
        "executable_resource",
        "argv",
        "stream",
        "spawn",
        "exit",
        "signal",
        "cancellation",
        "cleanup",
        "terminal",
    ] {
        assert!(fields.contains(required), "Patchbay exposes {required}");
    }
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);
    assert!(
        lesson["accessibility"]["non_audio"]
            .as_str()
            .unwrap()
            .contains("ordered text table")
    );

    let scenarios = lesson["library"]["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 5);
    let scenario_ids = scenarios
        .iter()
        .map(|scenario| scenario["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for required in [
        "standalone",
        "independent-streams",
        "cancel-and-cleanup",
        "typed-adapter",
        "browser-unsupported",
    ] {
        assert!(scenario_ids.contains(required));
    }
    assert_eq!(
        scenarios
            .iter()
            .find(|scenario| scenario["id"] == "browser-unsupported")
            .unwrap()["diagnostic"],
        "CND-IMP-001"
    );
}

#[test]
fn bounded_socket_lesson_keeps_transports_distinct_and_browser_support_honest() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-sockets")
        .expect("bounded socket lesson is selectable");
    assert_eq!(lesson["runnability"]["state"], "contract-only");
    assert_eq!(lesson["runnability"]["proof"], "resolver-rejection");
    assert_eq!(lesson["expected_diagnostic"], "CND-IMP-001");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");

    let contracts = lesson["library"]["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|contract| contract["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts,
        [
            "conduit.host/net/tcp/connect",
            "conduit.host/net/tcp/listen",
            "conduit.host/net/udp/connected",
            "conduit.host/net/udp/datagram",
        ]
        .into_iter()
        .collect()
    );
    let fields = lesson["presentation"]["patchbay_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for required in [
        "network_resource",
        "grant",
        "session",
        "accept",
        "stream_chunk",
        "datagram",
        "mtu",
        "half_close",
        "eof",
        "loss",
        "duplicate",
        "reorder",
        "cancellation",
        "cleanup",
        "terminal",
    ] {
        assert!(fields.contains(required), "Patchbay exposes {required}");
    }
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);
    assert!(
        lesson["accessibility"]["non_audio"]
            .as_str()
            .unwrap()
            .contains("ordered text table")
    );
    let scenarios = lesson["library"]["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 5);
    let scenario_ids = scenarios
        .iter()
        .map(|scenario| scenario["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for required in [
        "tcp-connect-composition",
        "tcp-listen-composition",
        "udp-connected-standalone",
        "udp-unconnected-standalone",
        "browser-unsupported",
    ] {
        assert!(scenario_ids.contains(required));
    }
}

#[test]
fn bounded_http_client_lesson_exposes_authority_limits_and_terminal_evidence() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-http-client")
        .expect("bounded HTTP client lesson is selectable");
    assert_eq!(lesson["runnability"]["state"], "contract-only");
    assert_eq!(lesson["runnability"]["proof"], "resolver-rejection");
    assert_eq!(lesson["expected_diagnostic"], "CND-IMP-001");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);

    let contracts = lesson["library"]["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|contract| contract["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts,
        [
            "net/http/fetch",
            "net/http/listen",
            "net/http/request/literal"
        ]
        .into_iter()
        .collect()
    );
    let fields = lesson["presentation"]["patchbay_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for required in [
        "numeric_address",
        "host_observation",
        "resource_binding",
        "grant_status",
        "effect_constraints",
        "transport",
        "redirect",
        "request_commit",
        "body_chunk",
        "run_state",
        "host_wake",
        "admitted_requests",
        "quiescence",
        "cancellation",
        "cleanup",
        "terminal",
    ] {
        assert!(fields.contains(required), "Patchbay exposes {required}");
    }
    let scenarios = lesson["library"]["scenarios"].as_array().unwrap();
    for required in [
        "request-literal-standalone",
        "request-client-composition",
        "redirect-and-downgrade",
        "cancel-partial-provider-loss",
        "browser-unsupported",
        "listener-standalone",
        "listener-session-composition",
    ] {
        assert!(
            scenarios.iter().any(|scenario| scenario["id"] == required),
            "lesson includes {required}"
        );
    }
}

#[test]
fn bounded_filesystem_lesson_runs_exact_browser_providers_and_failure_paths() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-filesystem")
        .expect("bounded filesystem lesson is selectable");
    assert_eq!(lesson["runnability"]["state"], "runnable");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    assert_eq!(
        lesson["library"]["contracts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|contract| contract["id"].as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        ["fs/chunk/literal", "fs/read", "fs/watch", "fs/write"]
            .into_iter()
            .collect()
    );
    let fields = lesson["presentation"]["patchbay_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for required in [
        "provider_state",
        "resource_handle",
        "grant",
        "operation_bounds",
        "queue",
        "pressure",
        "rename_identity",
        "cancellation",
        "error",
        "terminal",
    ] {
        assert!(fields.contains(required), "Patchbay exposes {required}");
    }
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);
    assert!(
        lesson["accessibility"]["non_audio"]
            .as_str()
            .unwrap()
            .contains("ordered text table")
    );

    let scenarios = lesson["library"]["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 6);
    assert_eq!(lesson["source"], scenarios[0]["source"]);
    for scenario in scenarios {
        let id = scenario["id"].as_str().unwrap();
        let source = scenario["source"].as_str().unwrap();
        assert_current_panel_source(id, source);
        let raw = if scenario["execution"] == "cancel-before-first-step" {
            cancel_panel(source.to_owned())
        } else {
            run_panel(source.to_owned())
        };
        assert!(!raw.contains("/home/"), "{id} redacts host paths: {raw}");
        assert!(
            !raw.contains("read-source.txt"),
            "{id} redacts provider mapping: {raw}"
        );
        let result: Value =
            serde_json::from_str(&raw).unwrap_or_else(|error| panic!("{id}: {error}: {raw}"));
        let validation = &scenario["validation"];
        let expected = validation["value"].as_str().unwrap();
        match validation["kind"].as_str().unwrap() {
            "display" => {
                assert_eq!(result["ok"], true, "{id}: {result}");
                assert_eq!(result["display"], expected, "{id}: {result}");
                assert_eq!(result["terminal"], "succeeded", "{id}: {result}");
            }
            "terminal" => {
                assert_eq!(result["ok"], true, "{id}: {result}");
                assert_eq!(result["terminal"], expected, "{id}: {result}");
            }
            "diagnostic" => {
                assert_eq!(result["ok"], false, "{id}: {result}");
                let diagnostic = result["diagnostic"]
                    .as_str()
                    .or_else(|| result["stderr"].as_str())
                    .unwrap_or_default();
                assert!(diagnostic.contains(expected), "{id}: {result}");
            }
            kind => panic!("unexpected filesystem validation kind {kind}"),
        }
    }
}

#[test]
fn evictable_storage_cache_lesson_runs_exact_browser_provider_and_failure_paths() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.evictable-storage-cache")
        .expect("evictable cache lesson is selectable");
    assert_eq!(lesson["runnability"]["state"], "runnable");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    assert_eq!(
        lesson["library"]["contracts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|contract| contract["id"].as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        [
            "storage/blob/literal",
            "storage/cache/get",
            "storage/cache/put",
            "storage/cache/remove",
        ]
        .into_iter()
        .collect()
    );
    let fields = lesson["presentation"]["patchbay_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for required in [
        "provider_state",
        "availability_evidence",
        "descriptor",
        "resource",
        "grant",
        "handle_scope",
        "content_identity",
        "retention",
        "eviction",
        "queue",
        "pressure",
        "cancellation",
        "error",
        "terminal",
    ] {
        assert!(fields.contains(required), "Patchbay exposes {required}");
    }
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);
    assert!(
        lesson["accessibility"]["non_audio"]
            .as_str()
            .unwrap()
            .contains("ordered text table")
    );
    assert!(
        lesson["library"]["wrong"]
            .as_str()
            .unwrap()
            .contains("no hidden fallback")
    );

    let scenarios = lesson["library"]["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 5);
    for scenario in scenarios {
        let id = scenario["id"].as_str().unwrap();
        let source = scenario["source"].as_str().unwrap();
        assert_current_panel_source(id, source);
        let raw = if scenario["execution"] == "cancel-before-first-step" {
            cancel_panel(source.to_owned())
        } else {
            run_panel(source.to_owned())
        };
        let result: Value =
            serde_json::from_str(&raw).unwrap_or_else(|error| panic!("{id}: {error}: {raw}"));
        let validation = &scenario["validation"];
        let expected = validation["value"].as_str().unwrap();
        match validation["kind"].as_str().unwrap() {
            "display" => {
                assert_eq!(result["ok"], true, "{id}: {result}");
                assert_eq!(result["display"], expected, "{id}: {result}");
                assert_eq!(result["terminal"], "succeeded", "{id}: {result}");
            }
            "terminal" => {
                assert_eq!(result["ok"], true, "{id}: {result}");
                assert_eq!(result["terminal"], expected, "{id}: {result}");
            }
            "diagnostic" => {
                assert_eq!(result["ok"], false, "{id}: {result}");
                let diagnostic = result["diagnostic"]
                    .as_str()
                    .or_else(|| result["stderr"].as_str())
                    .unwrap_or_default();
                assert!(diagnostic.contains(expected), "{id}: {result}");
            }
            kind => panic!("unexpected cache validation kind {kind}"),
        }
    }
}

#[test]
fn tour_reference_panels_are_canonical_runnable_or_fail_closed() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("tour/reference-panels/current.json"))
            .expect("reference-panel manifest exists"),
    )
    .expect("reference-panel manifest is valid");
    assert_eq!(manifest["schema"], "conduit.tour-reference-panels");

    for reference in manifest["panels"]
        .as_array()
        .expect("reference panels are listed")
    {
        let id = reference["id"].as_str().expect("reference id");
        let relative = reference["source_path"]
            .as_str()
            .expect("canonical source path")
            .strip_prefix("../../")
            .expect("source path points to repository fixture");
        let source = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{id} canonical source is readable: {error}"));
        assert_current_panel_source(id, &source);
        if reference["runnability"]["state"] == "runnable" {
            assert_eq!(
                reference["runnability"]["proof"],
                "browser-worker-exact-plan"
            );
            let result: Value =
                serde_json::from_str(&run_panel(source)).expect("runnable reference emits JSON");
            assert_eq!(result["ok"], true, "{id} runs in its browser profile");
            assert_eq!(result["terminal"], "succeeded", "{id} terminates");
            continue;
        }
        assert_eq!(
            reference["runnability"]["state"], "contract-only",
            "{id} has a verified runnability state"
        );
        assert!(
            reference["runnability"]["reason"]
                .as_str()
                .is_some_and(|reason| !reason.is_empty()),
            "{id} explains why Run is unavailable"
        );
        let panel = conduit_panel::parse(&source).expect("current reference source already parsed");
        let failure = Registry::hosted_primitives()
            .resolve(&panel)
            .expect_err("reference panel must fail closed without its provider");
        assert_eq!(
            failure.code,
            reference["runnability"]["code"]
                .as_str()
                .expect("structured rejection code"),
            "{id} declaration matches authoritative resolver"
        );
    }
}

#[test]
fn tour_linked_panel_examples_use_the_current_grammar() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");

    for lesson in manifest["lessons"].as_array().expect("lessons are listed") {
        let mut linked_panels = lesson["library"]["docs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        if let Some(panel) = lesson["platform"]["panel"].as_str() {
            linked_panels.push(panel);
        }
        for link in linked_panels {
            let path = link.split('#').next().unwrap();
            if !path.ends_with(".panel") {
                continue;
            }
            let relative = path
                .strip_prefix("../../")
                .expect("Tour panel link is repository-relative");
            let source = std::fs::read_to_string(root.join(relative))
                .unwrap_or_else(|error| panic!("{relative} is readable: {error}"));
            assert_current_panel_source(relative, &source);
        }
    }
}
