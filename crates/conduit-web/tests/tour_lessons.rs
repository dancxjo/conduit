use std::collections::{BTreeMap, BTreeSet};

use conduit_compile::{InstalledHostObservationInput, InstalledProfile, compile_source};
use conduit_runtime::Registry;
use conduit_web::{
    cancel_panel, patchbay_open_session as patchbay_open_session_with_front,
    patchbay_pump_exact_run, patchbay_start_exact_run, run_panel,
};
use serde_json::Value;

fn patchbay_open_session(document_id: String, source: String) -> String {
    patchbay_open_session_with_front(document_id, source, String::new(), String::new())
}

fn assert_current_panel_source(id: &str, source: &str) {
    let panel = conduit_panel::parse(source)
        .unwrap_or_else(|error| panic!("{id} must parse through conduit-panel: {error}"));
    assert_eq!(
        panel.version,
        conduit_panel::SOURCE_AST_SCHEMA_VERSION,
        "{id} must teach the current Panel grammar"
    );
}

fn browser_response_bytes(response: &str) -> usize {
    let value: Value = serde_json::from_str(response).expect("browser response is valid JSON");
    serde_json::to_vec(&serde_json::json!({
        "id": 1,
        "ok": true,
        "value": value,
    }))
    .expect("browser response envelope serializes")
    .len()
}

#[test]
fn cumulative_instrument_views_fit_the_explicit_browser_message_bound() {
    let browser_plan: Value =
        serde_json::from_str(include_str!("../../../tour/browser-plan-contract.json"))
            .expect("Tour browser plan contract is valid JSON");
    let maximum_message_bytes = browser_plan["bounds"]["maximum_message_bytes"]
        .as_u64()
        .expect("browser plan bounds message bytes") as usize;
    let session_id = "test/cumulative-instrument-message-bound";
    let source = include_str!("../../../examples/living-instrument.panel");

    let opened = patchbay_open_session(session_id.to_owned(), source.to_owned());
    assert!(
        browser_response_bytes(&opened) <= maximum_message_bytes,
        "the exact semantic and candidate-plan view fits the worker response bound"
    );

    let started = patchbay_start_exact_run(session_id.to_owned());
    assert!(
        browser_response_bytes(&started) <= maximum_message_bytes,
        "the exact active-plan view fits the worker response bound"
    );
    let started_value: Value = serde_json::from_str(&started).expect("start response JSON");
    assert_eq!(started_value["ok"], true);

    for _ in 0..8 {
        let pumped = patchbay_pump_exact_run(
            session_id.to_owned(),
            started_value["run_id"]
                .as_str()
                .expect("run identity")
                .to_owned(),
            started_value["source_revision"]
                .as_u64()
                .expect("source revision"),
            started_value["plan_identity"]
                .as_str()
                .expect("plan identity")
                .to_owned(),
            32,
        );
        assert!(
            browser_response_bytes(&pumped) <= maximum_message_bytes,
            "every bounded exact-run pump view fits the worker response bound"
        );
        let pumped_value: Value = serde_json::from_str(&pumped).expect("pump response JSON");
        assert_eq!(pumped_value["ok"], true);
        if pumped_value["state"] != "active" {
            break;
        }
    }
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
    assert_eq!(
        actual_ids.len(),
        lessons.len(),
        "Tour lesson ids are unique; the manifest is the published inventory"
    );
    let browser_plan: Value =
        serde_json::from_str(include_str!("../../../tour/browser-plan-contract.json"))
            .expect("Tour browser plan contract is valid JSON");
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
        let registry = if matches!(
            id,
            "platform.cross-host-provider-conformance" | "platform.audited-robotics-profile"
        ) {
            let mut registry = Registry::hosted_primitives();
            conduit_media::register_deterministic_media_providers(&mut registry)
                .expect("browser media value providers register");
            conduit_audio::transform_implementations::install_audio_gain_implementation(
                &mut registry,
                conduit_audio::transform_implementations::ObservedMediaArtifact::browser_wasm_linked(
                    include_bytes!("../src/lib.rs"),
                    0,
                    u64::MAX,
                )
                .expect("browser linked media provider is source-attested"),
            )
            .expect("browser gain implementation registers");
            registry
        } else if id == "library.bounded-quick-local-chat" {
            let mut registry = Registry::hosted_primitives();
            conduit_ai::register_deterministic_chat_provider(&mut registry)
                .expect("browser chat reference provider registers");
            registry
        } else if matches!(
            id,
            "library.bounded-audio-processing" | "library.audio-device-boundaries"
        ) {
            let mut registry = Registry::hosted_primitives();
            conduit_media::register_deterministic_signal_providers(&mut registry)
                .expect("browser signal providers register");
            conduit_media::register_deterministic_audio_processing_providers(&mut registry)
                .expect("browser audio providers register");
            registry
        } else if id == "library.bounded-brainstem-network" {
            let mut registry = Registry::hosted_primitives();
            conduit_net::register_deterministic_network_providers(&mut registry)
                .expect("browser network reference providers register");
            registry
        } else {
            Registry::compatibility_demo()
        };

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
fn quick_local_chat_lesson_runs_standalone_and_typed_result_composition() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-quick-local-chat")
        .expect("quick-local chat lesson is selectable");
    let contracts = lesson["library"]["contracts"].as_array().unwrap();
    assert!(contracts.iter().any(|contract| contract["id"] == "ai/chat"));
    assert!(
        contracts
            .iter()
            .any(|contract| contract["id"] == "ai/chat/result/inspect")
    );
    let scenarios = lesson["library"]["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 2);
    for scenario in scenarios {
        let source = scenario["source"].as_str().unwrap();
        assert_current_panel_source(scenario["id"].as_str().unwrap(), source);
        let result: Value = serde_json::from_str(&run_panel(source.to_owned())).unwrap();
        assert_eq!(result["ok"], true, "{result}");
        assert_eq!(result["display"], scenario["validation"]["value"]);
        assert_eq!(result["terminal"], "succeeded");
    }
    let prose = lesson["prose"].as_str().unwrap();
    for boundary in [
        "same contract",
        "host capability",
        "resource",
        "grant",
        "exact plan",
    ] {
        assert!(prose.contains(boundary), "lesson explains {boundary}");
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
fn bounded_audio_lesson_covers_every_processor_and_the_standing_patch() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-audio-processing")
        .expect("bounded audio lesson is selectable");
    assert_eq!(lesson["execution"], "continuous-watch");
    assert_eq!(lesson["validation"]["kind"], "watch");
    assert_eq!(
        lesson["source"],
        include_str!("../../../examples/audio-standing-patch.panel")
    );

    let contracts = lesson["library"]["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|contract| contract["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts,
        [
            "conduit.media/audio/channel-map",
            "conduit.media/audio/from-control",
            "conduit.media/audio/gain",
            "conduit.media/audio/meter",
            "conduit.media/audio/mix",
            "conduit.media/audio/resample",
            "conduit.media/audio/tee",
            "conduit.media/audio/trim",
        ]
        .into_iter()
        .collect()
    );
    let scenarios = lesson["library"]["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 10);
    for contract in ["mix", "gain", "channel-map", "resample", "trim", "meter"] {
        assert!(scenarios.iter().any(|scenario| {
            scenario["id"]
                .as_str()
                .is_some_and(|id| id.contains(contract) && id.contains("standalone"))
        }));
    }
    for field in [
        "numeric_profile",
        "named_channel_layout",
        "matrix_q15",
        "ramp_points",
        "group_delay",
        "retained_history",
        "flush",
        "meter_window",
        "pressure",
        "cancellation",
        "run_state",
        "terminal",
    ] {
        assert!(
            lesson["presentation"]["patchbay_fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == field),
            "audio lesson exposes {field}"
        );
    }
    let accessibility = &lesson["accessibility"];
    assert_eq!(accessibility["reduced_motion"], true);
    assert!(
        accessibility["keyboard"]
            .as_str()
            .unwrap()
            .contains("ArrowRight")
    );
    assert!(
        accessibility["non_audio"]
            .as_str()
            .unwrap()
            .contains("ordered event table")
    );
}

#[test]
fn audio_device_lesson_covers_both_boundaries_isolated_and_composed() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.audio-device-boundaries")
        .expect("audio device lesson is selectable");
    assert_eq!(lesson["execution"], "continuous-watch");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence-marble");
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);
    assert!(lesson["accessibility"]["non_audio"].as_str().is_some());
    assert!(lesson["accessibility"]["screen_reader"].as_str().is_some());

    let contracts = lesson["library"]["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|contract| contract["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts,
        [
            "conduit.media/audio/capture",
            "conduit.media/audio/playback"
        ]
        .into_iter()
        .collect()
    );
    let scenario_ids = lesson["library"]["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .map(|scenario| scenario["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        scenario_ids,
        [
            "audio-capture-standalone",
            "audio-device-composition",
            "audio-playback-standalone"
        ]
        .into_iter()
        .collect()
    );
    let panel = conduit_panel::parse(lesson["source"].as_str().unwrap()).unwrap();
    assert!(
        panel
            .nodes
            .iter()
            .any(|node| node.kind == "conduit.media/audio/capture")
    );
    assert!(
        panel
            .nodes
            .iter()
            .any(|node| node.kind == "conduit.media/audio/playback")
    );
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
fn bounded_learned_lifecycle_separates_evaluation_from_promotion_authority() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-learned-lifecycle")
        .expect("bounded learned lifecycle lesson is selectable");

    assert_eq!(lesson["runnability"]["state"], "runnable");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);
    for field in [
        "dataset_snapshot",
        "dataset_revision",
        "training_job",
        "checkpoint",
        "evaluation_report",
        "promotion_approval",
        "promotion_grant",
        "resource_lease",
        "commit_policy",
        "promotion_receipt",
        "deadline",
        "work",
        "storage_bytes",
        "cancellation",
        "provider_loss",
        "unknown_commit",
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
            "learned/dataset/inspect",
            "learned/dataset/literal",
            "learned/evaluate",
            "learned/evaluation/inspect",
            "learned/promote",
            "learned/promotion/inspect",
            "learned/train",
        ]
        .into_iter()
        .collect()
    );

    let scenarios = lesson["library"]["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 3);
    assert_eq!(lesson["source"], scenarios[0]["source"]);
    for scenario in scenarios {
        let id = scenario["id"].as_str().unwrap();
        let source = scenario["source"].as_str().unwrap();
        assert_current_panel_source(id, source);
        let result: Value = serde_json::from_str(&run_panel(source.to_owned())).unwrap();
        if scenario["validation"]["kind"] == "diagnostic" {
            assert_eq!(result["ok"], false, "{id}: {result}");
            assert_eq!(result["code"], scenario["validation"]["value"], "{id}");
        } else {
            assert_eq!(result["ok"], true, "{id}: {result}");
            assert_eq!(result["display"], scenario["validation"]["value"], "{id}");
            assert_eq!(result["terminal"], "succeeded", "{id}: {result}");
            assert_eq!(result["stdout"], "", "{id}: {result}");
            assert_eq!(result["stderr"], "", "{id}: {result}");
        }
    }
}

#[test]
fn bounded_knowledge_lesson_keeps_source_citation_and_run_evidence_distinct() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-knowledge-retrieval")
        .expect("bounded knowledge lesson is selectable");

    assert_eq!(lesson["runnability"]["state"], "runnable");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    for field in [
        "source_identity",
        "revision_identity",
        "span",
        "index_identity",
        "coverage",
        "embedding_identity",
        "embedding_dimensions",
        "provider_artifact",
        "retained_bytes",
        "work",
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
            "knowledge/citation/assemble",
            "knowledge/citation/inspect",
            "knowledge/document/literal",
            "knowledge/index/fixture",
            "knowledge/query/literal",
            "knowledge/rerank",
            "knowledge/retrieve",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);

    let raw = run_panel(lesson["source"].as_str().unwrap().to_owned());
    let result: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["display"], "knowledge:citation:31..42:exact plans");
    assert_eq!(result["stdout"], "");
    assert_eq!(result["stderr"], "");

    let cancelled: Value =
        serde_json::from_str(&cancel_panel(lesson["source"].as_str().unwrap().to_owned())).unwrap();
    assert_eq!(cancelled["ok"], true, "{cancelled}");
    assert_eq!(cancelled["terminal"], "cancelled");
}

#[test]
fn bounded_knowledge_graph_lesson_keeps_claim_support_edge_local() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-knowledge-graph")
        .expect("bounded knowledge graph lesson is selectable");

    assert_eq!(lesson["runnability"]["state"], "runnable");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    for field in [
        "entity_schema_identity",
        "entity_schema_version",
        "relation_schema_identity",
        "relation_schema_version",
        "claim_identity",
        "claim_disposition",
        "confidence_descriptor_identity",
        "validity",
        "sensitivity",
        "source_identity",
        "revision_identity",
        "span",
        "graph_schema_identity",
        "snapshot_identity",
        "coverage",
        "provider_identity",
        "depth",
        "breadth",
        "paths",
        "results",
        "retained_bytes",
        "work",
        "evidence_events",
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
            "knowledge/claim/from-citation",
            "knowledge/graph/fixture",
            "knowledge/graph/query/literal",
            "knowledge/graph/results/inspect",
            "knowledge/graph/traverse",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(lesson["library"]["scenarios"].as_array().unwrap().len(), 2);
    for scenario in lesson["library"]["scenarios"].as_array().unwrap() {
        assert_eq!(scenario["execution"], "run");
        assert!(
            scenario["source"]
                .as_str()
                .is_some_and(|source| !source.is_empty())
        );
    }
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);

    let raw = run_panel(lesson["source"].as_str().unwrap().to_owned());
    let result: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(
        result["display"],
        "knowledge:graph:Conduit--keeps-distinct-->exact-plans[source:31..42]"
    );
    assert_eq!(result["stdout"], "");
    assert_eq!(result["stderr"], "");

    let composition = &lesson["library"]["scenarios"][1];
    let composed: Value = serde_json::from_str(&run_panel(
        composition["source"].as_str().unwrap().to_owned(),
    ))
    .unwrap();
    assert_eq!(composed["ok"], true, "{composed}");
    assert_eq!(
        composed["display"],
        "KNOWLEDGE:GRAPH:CONDUIT--KEEPS-DISTINCT-->EXACT-PLANS[SOURCE:31..42]"
    );

    let cancelled: Value =
        serde_json::from_str(&cancel_panel(lesson["source"].as_str().unwrap().to_owned())).unwrap();
    assert_eq!(cancelled["ok"], true, "{cancelled}");
    assert_eq!(cancelled["terminal"], "cancelled");
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
fn bounded_spatial_data_lesson_runs_standalone_and_composed_exact_plans() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-spatial-data")
        .expect("bounded spatial-data lesson is selectable");

    assert_eq!(lesson["runnability"]["state"], "runnable");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);
    for field in [
        "scan_schema_identity",
        "grid_schema_identity",
        "trajectory_schema_identity",
        "snapshot_identity",
        "representation_identity",
        "provider_identity",
        "frame",
        "unit",
        "clock",
        "calibration_identity",
        "chunk_index",
        "chunk_count",
        "point_count",
        "grid_dimensions",
        "coverage",
        "trajectory_history",
        "retained_bytes",
        "numeric_work",
        "queue_bytes",
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
            "spatial/grid/from-scan",
            "spatial/grid/inspect",
            "spatial/scan/fixture",
            "spatial/scan/transform",
            "spatial/trajectory/fixture",
            "spatial/trajectory/inspect",
        ]
        .into_iter()
        .collect()
    );

    let scenarios = lesson["library"]["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 3);
    assert!(scenarios.iter().all(|scenario| {
        scenario["execution"] == "run"
            && scenario["source"]
                .as_str()
                .is_some_and(|source| source.starts_with("panel 0"))
    }));

    let raw = run_panel(lesson["source"].as_str().unwrap().to_owned());
    let result: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(
        result["display"],
        "spatial:grid:map:2x2:occupied=2:coverage=complete"
    );
    assert_eq!(result["stdout"], "");
    assert_eq!(result["stderr"], "");

    let standalone = scenarios
        .iter()
        .find(|scenario| scenario["id"] == "spatial-grid-standalone")
        .unwrap();
    let standalone_result: Value = serde_json::from_str(&run_panel(
        standalone["source"].as_str().unwrap().to_owned(),
    ))
    .unwrap();
    assert_eq!(standalone_result["ok"], true, "{standalone_result}");
    assert_eq!(
        standalone_result["display"],
        "spatial:grid:sensor:2x2:occupied=2:coverage=complete"
    );

    let trajectory = scenarios
        .iter()
        .find(|scenario| scenario["id"] == "spatial-trajectory-text-composition")
        .unwrap();
    let trajectory_result: Value = serde_json::from_str(&run_panel(
        trajectory["source"].as_str().unwrap().to_owned(),
    ))
    .unwrap();
    assert_eq!(trajectory_result["ok"], true, "{trajectory_result}");
    assert_eq!(
        trajectory_result["display"],
        "SPATIAL:TRAJECTORY:MAP:2:CLOCK/FIXTURE:LINEAR-Q30-SHORTEST"
    );

    let cancelled: Value =
        serde_json::from_str(&cancel_panel(lesson["source"].as_str().unwrap().to_owned())).unwrap();
    assert_eq!(cancelled["ok"], true, "{cancelled}");
    assert_eq!(cancelled["terminal"], "cancelled");
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
    assert_eq!(lesson["execution"], "continuous-watch");
    assert_eq!(lesson["validation"]["kind"], "watch");
    assert_eq!(
        lesson["source"],
        include_str!("../../../examples/pico-network-providers.panel")
    );
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
            "net/dns/local-authority",
            "net/dns-sd",
            "net/reachability",
            "net/wifi/access-point",
            "net/address/tee",
        ]
        .into_iter()
        .collect()
    );
    let scenarios = lesson["library"]["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .map(|scenario| scenario["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for required in [
        "two-isolated-endpoints",
        "attachment-lease-composition",
        "route-and-observation",
        "listener-and-sessions",
        "application-exchanges",
        "failure-and-recovery",
        "assembled-topology",
    ] {
        assert!(
            scenarios.contains(required),
            "missing Tour stage {required}"
        );
    }
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);
    for forbidden in ["CreateUart", "motor_grant", "possession_grant"] {
        assert!(!lesson["source"].as_str().unwrap().contains(forbidden));
    }

    let result: Value =
        serde_json::from_str(&cancel_panel(lesson["source"].as_str().unwrap().to_owned())).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["terminal"], "cancelled");
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
fn bounded_control_lesson_exposes_typed_admission_pressure_and_causal_timelines() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-control-composites")
        .expect("bounded control lesson is selectable");

    assert_eq!(lesson["runnability"]["state"], "contract-only");
    assert_eq!(lesson["runnability"]["proof"], "resolver-rejection");
    assert_eq!(lesson["expected_diagnostic"], "CND-IMP-001");
    assert_eq!(
        lesson["presentation"]["timeline"],
        "accessible-textual-evidence-projection"
    );
    let projected_fields = lesson["presentation"]["patchbay_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    let required_fields = conduit_patchbay::REQUEST_REPLY_PATCHBAY_FIELDS
        .iter()
        .chain(conduit_patchbay::CANCELLABLE_ACTION_PATCHBAY_FIELDS)
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(projected_fields, required_fields);
    let contracts = lesson["library"]["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|contract| contract["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contracts,
        [
            "conduit.std/control/cancellable-action",
            "conduit.std/control/request-reply",
        ]
        .into_iter()
        .collect()
    );
    assert!(lesson["library"]["wrong"].as_str().is_some_and(|wrong| {
        wrong.contains("universal Action<T>")
            && wrong.contains("success booleans")
            && wrong.contains("silent provider handoff")
    }));
    let scenarios = lesson["library"]["scenarios"].as_array().unwrap();
    for required in [
        "request-reply",
        "action",
        "composition",
        "unavailable-host",
        "robotics-denial",
    ] {
        assert!(
            scenarios.iter().any(|scenario| scenario["id"] == required),
            "lesson includes {required} story"
        );
    }
    assert!(
        lesson["accessibility"]["non_audio"]
            .as_str()
            .is_some_and(|text| text.contains("ordered textual timeline"))
    );

    let panel = conduit_panel::parse(lesson["source"].as_str().unwrap()).unwrap();
    let error = Registry::compatibility_demo()
        .resolve(&panel)
        .expect_err("an imported control shape is not an observed provider");
    assert_eq!(error.code, "CND-IMP-001");
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
        lesson["prose"]
            .as_str()
            .is_some_and(|prose| prose.contains("monotonic cursors")
                && prose.contains("explicit gap")
                && prose.contains("semantic tee")
                && prose.contains("lossless recorder")
                && prose.contains("Freeze Display")
                && prose.contains("structurally visible but redacted")
                && prose.contains("Strict audit")
                && prose.contains("does not execute the graph or own the evidence store"))
    );
    assert!(
        lesson["accessibility"]["keyboard"]
            .as_str()
            .is_some_and(|text| text.contains("Shift+Enter")
                && text.contains("W to attach or remove")
                && text.contains("F to freeze or resume")
                && text.contains("ArrowRight"))
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
        "watch_attach_detach",
        "watch_gap",
        "watch_redaction",
        "watch_tee_recorder_comparison",
        "freeze_display",
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
        serde_json::from_str(include_str!("../../../tour/browser-plan-contract.json"))
            .expect("Tour browser plan contract is valid JSON");
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

#[test]
fn cross_host_provider_lesson_retains_the_complete_exact_chain() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/c5/cross-host-media-implementations.json"
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
    assert_eq!(
        source,
        include_str!("../../../examples/media-gain-provider.panel")
    );
    let mut overlap_registry = Registry::hosted_primitives();
    conduit_media::register_deterministic_media_providers(&mut overlap_registry).unwrap();
    conduit_audio::transform_implementations::install_audio_gain_implementation(
        &mut overlap_registry,
        conduit_audio::transform_implementations::ObservedMediaArtifact::browser_wasm_linked(
            include_bytes!("../src/lib.rs"),
            10,
            20,
        )
        .unwrap(),
    )
    .unwrap();
    conduit_media::register_deterministic_audio_processing_providers(&mut overlap_registry)
        .unwrap();
    let mut browser_host = InstalledHostObservationInput::conduct_host();
    browser_host.id = "conduit/browser-worker-host-observation".to_owned();
    browser_host.host = "conduit/browser-worker".to_owned();
    browser_host.time_basis = "clock/browser-worker".to_owned();
    let installed =
        InstalledProfile::observe_registry_on_host(source, &overlap_registry, &browser_host, &[])
            .unwrap()
            .with_implementation_preference(vec![
                conduit_audio::transform_implementations::MediaImplementation::BrowserWasmLinked
                    .id()
                    .to_owned(),
            ])
            .unwrap();
    let preferred = compile_source(source, &installed.input).unwrap();
    assert!(preferred.nodes.iter().any(|node| {
        node.contract.id == "conduit.media/audio/gain"
            && node.implementation.id
                == conduit_audio::transform_implementations::MediaImplementation::BrowserWasmLinked
                    .id()
    }));
    let cases = fixture["cases"].as_array().unwrap();
    let mut profile_ids = BTreeSet::new();
    for profile in lesson["platform"]["profiles"].as_array().unwrap() {
        let fixture_case = cases
            .iter()
            .find(|case| case["id"] == profile["fixture_case"])
            .expect("lesson profile names a conformance case");
        let profile_id = profile["id"].as_str().expect("platform profile has id");
        profile_ids.insert(profile_id);
        if profile["admission"] == "rejected" {
            assert_eq!(fixture_case["expected"], profile["code"]);
        }
    }
    assert_eq!(
        profile_ids,
        [
            "reference-media",
            "linux-ffmpeg-process",
            "linux-sox-process",
            "browser-worker",
            "known-contract-no-provider",
        ]
        .into_iter()
        .collect()
    );

    let opened: Value = serde_json::from_str(&patchbay_open_session(
        "cross-host-media-implementation".to_owned(),
        source.to_owned(),
    ))
    .expect("Patchbay session JSON");
    assert_eq!(opened["ok"], true, "{opened}");
    let started: Value = serde_json::from_str(&patchbay_start_exact_run(
        "cross-host-media-implementation".to_owned(),
    ))
    .expect("Patchbay exact-run JSON");
    assert_eq!(started["ok"], true, "{started}");
    let logical_gain = started["view"]["topology"]["logical_nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["contract_id"] == "conduit.media/audio/gain")
        .collect::<Vec<_>>();
    assert_eq!(logical_gain.len(), 1, "one semantic gain node");
    assert!(logical_gain[0].get("implementation_id").is_none());
    let planned_gain = started["view"]["topology"]["planned_realization"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["binding"]["contract_id"] == "conduit.media/audio/gain")
        .collect::<Vec<_>>();
    assert_eq!(planned_gain.len(), 1, "one realized gain node");
    assert_eq!(
        planned_gain[0]["binding"]["implementation_id"],
        "conduit.media/audio-gain-browser-wasm-linked"
    );
    assert_eq!(
        planned_gain[0]["binding"]["host_id"],
        "conduit/browser-worker"
    );
    assert!(
        planned_gain[0]["binding"]["artifact_digest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:"))
    );
    let fields = lesson["presentation"]["patchbay_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for required in [
        "contract_id",
        "implementation_id",
        "artifact_digest",
        "host_observation",
        "limits",
        "resource",
        "grant",
        "cancellation",
        "cleanup",
        "terminal",
    ] {
        assert!(fields.contains(required), "Patchbay exposes {required}");
    }
}

#[test]
fn audited_robotics_profile_reuses_generic_host_presentation_without_runtime_invention() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/c5/netherwick-describe-only-profile.json"
    ))
    .expect("robotics profile fixture is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "platform.audited-robotics-profile")
        .expect("audited robotics profile lesson is selectable");

    assert_eq!(lesson["presentation"]["layout"], "one-node-provider-matrix");
    assert_eq!(
        lesson["source"],
        include_str!("../../../examples/media-gain-provider.panel")
    );
    let result: Value =
        serde_json::from_str(&run_panel(lesson["source"].as_str().unwrap().to_owned())).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["display"], lesson["expected_display"]);
    assert_eq!(result["terminal"], "succeeded");

    let profiles = lesson["platform"]["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 3);
    for id in [
        "netherwick-linux-describe-only",
        "netherwick-pico-w-describe-only",
    ] {
        let profile = profiles
            .iter()
            .find(|profile| profile["id"] == id)
            .expect("lesson includes both describe-only hosts");
        assert_eq!(profile["admission"], "rejected-before-work");
        assert_eq!(profile["code"], "CND-HCF-003");
        assert_eq!(profile["effects"], 0);
    }
    let hosts = fixture["hosts"].as_array().unwrap();
    assert_eq!(hosts.len(), 2);
    assert!(
        hosts
            .iter()
            .all(|host| { host["class"] == "describe-only" && host["observation"].is_null() })
    );
    assert_eq!(fixture["contract"], "conduit.robotics/profile");
    assert_eq!(fixture["command_flow"]["ordinary_ingress_capacity"], 1);
    assert_eq!(fixture["command_flow"]["motion_ingress_capacity"], 1);
    assert_eq!(fixture["command_flow"]["execution_queue_capacity"], 16);
    assert_eq!(
        fixture["command_flow"]["maximum_interrupted_command_ids"],
        2
    );
    assert_ne!(hosts[0]["implementation"], hosts[1]["implementation"]);
    assert_ne!(hosts[0]["artifact"], hosts[1]["artifact"]);

    let fields = lesson["presentation"]["patchbay_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for required in [
        "compiled_provider_state",
        "initialized_observation",
        "logical_relationship",
        "path_observation",
        "possession_identity",
        "authority_identity",
        "command_flow_policy",
        "ordinary_ingress",
        "motion_ingress",
        "execution_queue",
        "command_interruption",
        "effect_audit",
        "terminal",
    ] {
        assert!(
            fields.contains(required),
            "typed projection includes {required}"
        );
    }
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
fn persistent_http_service_lesson_exposes_authority_limits_and_terminal_evidence() {
    let manifest: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let lesson = manifest["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lesson| lesson["id"] == "library.bounded-http-service")
        .expect("persistent HTTP service lesson is selectable");
    assert_eq!(lesson["execution"], "persistent-hosted-service");
    assert_eq!(lesson["runnability"]["state"], "contract-only");
    assert_eq!(lesson["runnability"]["proof"], "resolver-rejection");
    assert_eq!(lesson["runnability"]["code"], "CND-IMP-001");
    assert_eq!(lesson["expected_diagnostic"], "CND-IMP-001");
    assert_eq!(lesson["presentation"]["timeline"], "exact-evidence");
    assert_eq!(lesson["accessibility"]["reduced_motion"], true);

    let panel = conduit_panel::parse(lesson["source"].as_str().unwrap())
        .expect("the Tour HTTP service uses current Panel source");
    assert_eq!(panel.nodes.len(), 1);
    assert_eq!(panel.nodes[0].kind, "net/http/listen");

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
        [
            "fs/chunk/literal",
            "fs/read",
            "fs/watch",
            "fs/write",
            "fs/write-result/sink",
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
    let task_source = lesson["source"].as_str().expect("Copy task source");
    assert_current_panel_source("copy-task", task_source);
    assert!(task_source.contains("conduit.resource-binding/copy-source"));
    assert!(task_source.contains("conduit.resource-binding/copy-destination"));
    assert!(!task_source.contains("conduit.resource/filesystem-example-read"));
    assert_eq!(lesson["task_front"]["name"], "Copy a file");
    assert_eq!(lesson["task_front"]["controls"][0]["label"], "From");
    assert_eq!(lesson["task_front"]["controls"][1]["label"], "To");
    assert_eq!(lesson["task_front"]["controls"][2]["label"], "Mode");
    assert_eq!(
        lesson["task_front"]["controls"][3]["label"],
        "Maximum bytes"
    );
    assert_eq!(
        lesson["task_front"]["result"]["target"],
        "root/copy/port/outgoing/result"
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
fn reader_manifest_resolves_every_exact_lab_and_separates_book_from_directories() {
    let lessons: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let book: Value = serde_json::from_str(include_str!("../../../tour/book/current.json"))
        .expect("Tour book manifest is valid JSON");
    let ledger: Value = serde_json::from_str(include_str!("../../../tour/book/migration.json"))
        .expect("Tour migration ledger is valid JSON");

    assert_eq!(book["schema"], "conduit.tour-book");
    assert_eq!(book["schema_version"], 0);
    assert_eq!(book["migration_ledger"], "./migration.json");
    assert_eq!(book["fresh_reader_study"], "./fresh-reader-study.json");
    assert!(
        book["cover"]["start_section"]
            .as_str()
            .is_some_and(|value| !value.is_empty()),
        "the reader cover names its first section"
    );

    let lesson_by_id = lessons["lessons"]
        .as_array()
        .expect("lessons are listed")
        .iter()
        .map(|lesson| {
            assert!(
                lesson.get("origin").is_none(),
                "reader narrative is not stored in machine lesson {}",
                lesson["id"]
            );
            (lesson["id"].as_str().expect("lesson id"), lesson)
        })
        .collect::<BTreeMap<_, _>>();
    let disposition_by_id = ledger["entries"]
        .as_array()
        .expect("migration entries are listed")
        .iter()
        .map(|entry| {
            (
                entry["lesson_id"].as_str().expect("ledger lesson id"),
                entry,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let allowed_kinds = BTreeSet::from([
        "invitation",
        "need",
        "idea",
        "action",
        "lab",
        "witness",
        "explanation",
        "reflection",
        "next-hook",
    ]);
    let required_narrative_kinds = BTreeSet::from([
        "invitation",
        "need",
        "idea",
        "action",
        "witness",
        "explanation",
        "reflection",
        "next-hook",
    ]);
    let mut section_ids = BTreeSet::new();
    let mut sequential_lesson_ids = BTreeSet::new();
    let mut first_section = None;
    let mut build_projects = 0;

    for project in book["projects"].as_array().expect("projects are listed") {
        let project_id = project["id"].as_str().expect("project id");
        let kind = project["kind"].as_str().expect("project kind");
        assert!(
            ["prologue", "project"].contains(&kind),
            "{project_id} has a reader-facing kind"
        );
        if kind == "project" {
            build_projects += 1;
        }
        assert!(
            project["title"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
                && project["description"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty()),
            "every cumulative project is named and described"
        );
        let artifact = &project["artifact"];
        for field in [
            "id",
            "initial_revision",
            "final_revision",
            "state_key",
            "non_audio_result",
        ] {
            assert!(
                artifact[field]
                    .as_str()
                    .is_some_and(|value| !value.is_empty()),
                "{project_id} artifact records {field}"
            );
        }
        if let Some(source_path) = artifact["source_path"].as_str() {
            let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
            let source_path = root.join("tour/public").join(source_path);
            let source = std::fs::read_to_string(&source_path).unwrap_or_else(|error| {
                panic!(
                    "{project_id} exact artifact {} is readable: {error}",
                    source_path.display()
                )
            });
            assert_current_panel_source(project_id, &source);
        }
        let mut expected_revision = artifact["initial_revision"]
            .as_str()
            .expect("initial revision");
        for chapter in project["chapters"].as_array().expect("chapters are listed") {
            assert!(chapter["number"].is_u64(), "chapter number is explicit");
            for field in ["title", "description", "opening"] {
                assert!(
                    chapter[field]
                        .as_str()
                        .is_some_and(|value| !value.is_empty()),
                    "chapter {field} is reader-facing prose"
                );
            }
            for section in chapter["sections"].as_array().expect("sections are listed") {
                let section_id = section["id"].as_str().expect("section id");
                first_section.get_or_insert(section_id);
                assert!(section_ids.insert(section_id), "section ids are unique");
                assert!(
                    section["title"]
                        .as_str()
                        .is_some_and(|value| !value.is_empty())
                        && section["summary"]
                            .as_str()
                            .is_some_and(|value| !value.is_empty()),
                    "section headings are meaningful without a lab"
                );
                for field in ["opening_result", "starting_artifact"] {
                    assert!(
                        section[field]
                            .as_str()
                            .is_some_and(|value| !value.is_empty()),
                        "{section_id} begins from a meaningful project result"
                    );
                }
                assert_eq!(
                    section["starting_artifact"], section["state"]["inherits"],
                    "{section_id} starts from its inherited artifact"
                );
                assert_eq!(
                    section["state"]["inherits"], expected_revision,
                    "{section_id} carries the previous section artifact forward"
                );
                expected_revision = section["state"]["produces"]
                    .as_str()
                    .expect("produced artifact revision");
                assert!(
                    section["state"]["carry_forward"]
                        .as_str()
                        .is_some_and(|value| !value.is_empty()),
                    "{section_id} names what continues"
                );
                for field in ["non_audio", "reduced_motion", "screen_reader"] {
                    assert!(
                        section["accessibility"][field]
                            .as_str()
                            .is_some_and(|value| !value.is_empty()),
                        "{section_id} has an adjacent {field} equivalent"
                    );
                }
                let blocks = section["blocks"]
                    .as_array()
                    .expect("ordered section blocks");
                let labs = blocks
                    .iter()
                    .enumerate()
                    .filter(|(_, block)| block["kind"] == "lab")
                    .collect::<Vec<_>>();
                assert_eq!(labs.len(), 1, "{section_id} embeds one exact lab");
                let (lab_index, lab) = labs[0];
                assert!(
                    lab_index > 0 && lab_index + 1 < blocks.len(),
                    "{section_id} has prose before and after its lab"
                );
                let lesson_id = lab["lesson_id"].as_str().expect("exact lesson reference");
                assert!(
                    lesson_by_id.contains_key(lesson_id),
                    "{section_id} resolves lesson {lesson_id}"
                );
                assert!(
                    !lesson_id.starts_with("welcome."),
                    "the main book never returns to the retired Hello Panel curriculum"
                );
                sequential_lesson_ids.insert(lesson_id);
                for technical_ref in section["technical_refs"]
                    .as_array()
                    .expect("technical references are listed")
                {
                    let technical_ref = technical_ref.as_str().expect("technical lesson id");
                    assert!(
                        lesson_by_id.contains_key(technical_ref),
                        "{section_id} resolves supporting proof {technical_ref}"
                    );
                    sequential_lesson_ids.insert(technical_ref);
                }
                let mut section_narrative_kinds = BTreeSet::new();
                for block in blocks {
                    let kind = block["kind"].as_str().expect("block kind");
                    assert!(allowed_kinds.contains(kind), "known reader block {kind}");
                    if kind != "lab" {
                        section_narrative_kinds.insert(kind);
                        assert!(
                            block["body"]
                                .as_str()
                                .is_some_and(|value| !value.is_empty()),
                            "narrative blocks remain coherent without the lab"
                        );
                    }
                }
                assert_eq!(
                    section_narrative_kinds, required_narrative_kinds,
                    "{section_id} follows the complete project chapter anatomy"
                );
            }
        }
        assert_eq!(
            expected_revision,
            artifact["final_revision"].as_str().expect("final revision"),
            "{project_id} reaches its declared final artifact"
        );
    }
    assert_eq!(
        build_projects, 3,
        "the first book release has three cumulative builds"
    );
    let instrument = book["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|project| project["id"] == "living-instrument")
        .expect("living instrument project");
    assert_eq!(
        instrument["artifact"]["source_path"],
        "../../examples/living-instrument.panel"
    );
    assert_eq!(instrument["artifact"]["execution"], "continuous-watch");
    assert_eq!(instrument["artifact"]["watch_target"]["from_node"], "scope");
    assert_eq!(instrument["artifact"]["watch_target"]["from_port"], "text");
    assert_eq!(instrument["artifact"]["validation"]["kind"], "watch-prefix");
    assert_eq!(
        first_section,
        book["cover"]["start_section"].as_str(),
        "the cover begins the sequential path"
    );

    let mut cookbook_lesson_ids = BTreeSet::new();
    let mut recipe_ids = BTreeSet::new();
    for recipe in book["cookbook"]["recipes"]
        .as_array()
        .expect("Cookbook recipes are listed")
    {
        let recipe_id = recipe["id"].as_str().expect("recipe id");
        let lesson_id = recipe["lesson_id"].as_str().expect("recipe lesson id");
        assert!(recipe_ids.insert(recipe_id), "recipe ids are unique");
        assert!(
            lesson_by_id.contains_key(lesson_id),
            "recipe resolves {lesson_id}"
        );
        assert!(
            cookbook_lesson_ids.insert(lesson_id),
            "Cookbook lesson references are unique"
        );
    }
    let reference_lesson_ids = book["reference"]["lessons"]
        .as_array()
        .expect("Reference lessons are listed")
        .iter()
        .map(|lesson| lesson.as_str().expect("reference lesson id"))
        .collect::<BTreeSet<_>>();
    let retired_lesson_ids = book["retired"]["lessons"]
        .as_array()
        .expect("retired lessons are listed")
        .iter()
        .map(|lesson| {
            assert!(
                section_ids.contains(
                    lesson["replacement_section"]
                        .as_str()
                        .expect("replacement section")
                ),
                "every retired fixture has a current reading destination"
            );
            assert!(
                lesson["reason"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty()),
                "retirement is explained honestly"
            );
            lesson["lesson_id"].as_str().expect("retired lesson id")
        })
        .collect::<BTreeSet<_>>();

    for (lesson_id, disposition) in &disposition_by_id {
        let destination_id = disposition["destination"]["id"]
            .as_str()
            .expect("destination id");
        match disposition["classification"]
            .as_str()
            .expect("classification")
        {
            "Book" | "Interlude" => {
                assert!(
                    section_ids.contains(destination_id),
                    "{lesson_id} resolves section"
                );
                assert!(
                    sequential_lesson_ids.contains(lesson_id),
                    "{lesson_id} is backed by a lab or technical proof in its project section"
                );
            }
            "Cookbook" => assert!(
                cookbook_lesson_ids.contains(lesson_id),
                "{lesson_id} resolves a Cookbook recipe"
            ),
            "Reference" => assert!(
                reference_lesson_ids.contains(lesson_id),
                "{lesson_id} resolves a Reference entry"
            ),
            "Retire/Replace" => assert!(
                retired_lesson_ids.contains(lesson_id),
                "{lesson_id} resolves an honest retired page"
            ),
            _ => unreachable!(),
        }
    }
    assert_eq!(
        disposition_by_id.keys().copied().collect::<BTreeSet<_>>(),
        lesson_by_id.keys().copied().collect::<BTreeSet<_>>(),
        "every machine lesson is reachable through its deliberate editorial disposition"
    );
    assert_eq!(
        book["reference"]["panel_manifest"], "../reference-panels/current.json",
        "Reference navigation owns the canonical panel directory"
    );
}

#[test]
fn tour_migration_ledger_classifies_the_complete_machine_inventory() {
    let lessons: Value = serde_json::from_str(include_str!("../../../tour/lessons/current.json"))
        .expect("Tour lesson manifest is valid JSON");
    let ledger: Value = serde_json::from_str(include_str!("../../../tour/book/migration.json"))
        .expect("Tour migration ledger is valid JSON");

    assert_eq!(ledger["schema"], "conduit.tour-migration-ledger");
    assert_eq!(ledger["schema_version"], 0);
    assert_eq!(ledger["source_manifest"], "../lessons/current.json");

    let machine_lessons = lessons["lessons"]
        .as_array()
        .expect("lessons are listed")
        .iter()
        .map(|lesson| (lesson["id"].as_str().expect("lesson id"), lesson))
        .collect::<BTreeMap<_, _>>();
    let entries = ledger["entries"]
        .as_array()
        .expect("migration entries are listed");
    let mut ledger_ids = BTreeSet::new();
    let mut classifications = BTreeSet::new();

    for entry in entries {
        let id = entry["lesson_id"].as_str().expect("ledger lesson id");
        assert!(ledger_ids.insert(id), "ledger lesson ids are unique");
        let lesson = machine_lessons
            .get(id)
            .unwrap_or_else(|| panic!("ledger resolves machine lesson {id}"));
        assert_eq!(entry["title"], lesson["title"], "{id} title is current");
        assert_eq!(
            entry["chapter"], lesson["chapter"],
            "{id} source chapter is recorded"
        );
        assert_eq!(
            entry["runnability"], lesson["runnability"]["state"],
            "{id} runnability is current"
        );
        assert_eq!(
            entry["prerequisites"], lesson["prerequisites"],
            "{id} prerequisites are current"
        );
        for field in ["concept_proof", "reader_payoff", "narrative_rewrite"] {
            assert!(
                entry[field].as_str().is_some_and(|value| !value.is_empty()),
                "{id} records {field}"
            );
        }
        assert!(entry["advances_project"].is_boolean(), "{id} is classified");
        assert_eq!(entry["preserve"]["source"], true, "{id} keeps exact source");

        let classification = entry["classification"].as_str().expect("classification");
        assert!(
            [
                "Book",
                "Interlude",
                "Cookbook",
                "Reference",
                "Retire/Replace"
            ]
            .contains(&classification),
            "{id} uses one editorial classification"
        );
        classifications.insert(classification);
        let destination_kind = entry["destination"]["kind"]
            .as_str()
            .expect("destination kind");
        let expected_kind = match classification {
            "Book" | "Interlude" => "section",
            "Cookbook" => "recipe",
            "Reference" => "reference",
            "Retire/Replace" => "retired",
            _ => unreachable!(),
        };
        assert_eq!(destination_kind, expected_kind, "{id} has an honest route");
        assert_eq!(
            entry["old_route"]["kind"], entry["destination"]["kind"],
            "{id} old deep links resolve through the current destination kind"
        );
        if entry["advances_project"] == true {
            assert_eq!(
                classification, "Book",
                "only project work advances a project"
            );
        }

        let expected_scenarios = lesson["library"]["scenarios"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|scenario| scenario["id"].as_str())
            .chain(
                lesson["platform"]["profiles"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|profile| profile["id"].as_str()),
            )
            .collect::<BTreeSet<_>>();
        let preserved_scenarios = entry["preserve"]["scenarios"]
            .as_array()
            .expect("preserved scenarios are listed")
            .iter()
            .map(|scenario| scenario.as_str().expect("scenario id"))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            preserved_scenarios, expected_scenarios,
            "{id} preserves every scenario/profile proof"
        );
    }

    assert_eq!(
        ledger_ids,
        machine_lessons.keys().copied().collect::<BTreeSet<_>>(),
        "every current machine lesson has exactly one editorial disposition"
    );
    assert_eq!(
        classifications,
        BTreeSet::from([
            "Book",
            "Cookbook",
            "Interlude",
            "Reference",
            "Retire/Replace"
        ]),
        "the migration uses every deliberate editorial destination"
    );
}

#[test]
fn fresh_reader_study_covers_the_book_promise_without_claiming_observations() {
    let study: Value =
        serde_json::from_str(include_str!("../../../tour/book/fresh-reader-study.json"))
            .expect("Tour fresh-reader study is valid JSON");

    assert_eq!(study["schema"], "conduit.tour-fresh-reader-study");
    assert_eq!(study["schema_version"], 0);
    assert_eq!(
        study["status"], "protocol-ready",
        "a checked protocol does not impersonate a completed independent study"
    );
    assert!(
        study["participant_requirement"]
            .as_str()
            .is_some_and(|requirement| requirement.contains("not read the issue bodies")),
        "the participant is independent of implementation context"
    );
    assert_eq!(study["conditions"]["maximum_minutes_to_first_result"], 1);
    assert_eq!(study["conditions"]["audio_optional"], true);
    assert_eq!(study["conditions"]["keyboard_only_pass_required"], true);
    assert_eq!(study["conditions"]["reduced_motion_pass_required"], true);
    assert_eq!(
        study["conditions"]["technical_drawers_initially_closed"],
        true
    );
    let questions = study["questions"]
        .as_array()
        .expect("study questions are listed");
    assert_eq!(
        questions
            .iter()
            .map(|question| question["id"].as_str().expect("question id"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["why-conduit", "made", "needed-ideas", "next"]),
        "the study asks why, what was made, why each idea was needed, and what comes next"
    );
    for question in questions {
        assert!(
            question["prompt"]
                .as_str()
                .is_some_and(|prompt| !prompt.is_empty()),
            "every question is askable"
        );
        assert!(
            question["passing_evidence"]
                .as_array()
                .is_some_and(|evidence| !evidence.is_empty()),
            "every question has an explicit evaluation boundary"
        );
    }
    assert_eq!(
        study["observations"].as_array().map(Vec::len),
        Some(0),
        "no independent participant observation is fabricated"
    );
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
