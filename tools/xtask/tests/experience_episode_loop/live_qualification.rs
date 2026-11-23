//! Synthetic Pete inputs qualify live model grounding, without physical access.
use super::*;

pub(super) fn bodily_frames() -> Vec<PeteCognitionFrame> {
    let unavailable = frame(4, "What is my battery charge percentage now?", false);
    vec![
        create_frame(1, "Remember my battery charge.", true),
        create_frame(2, "What was my battery charge percentage before?", false),
        create_frame(3, "What is my battery charge percentage now?", false),
        PeteCognitionFrame::new(
            PeteCognitionBatteryInput::unavailable("pete/current-battery".into()).unwrap(),
            unavailable.body().clone(),
            unavailable.provenance_revision().clone(),
            unavailable.reference().clone(),
            *unavailable.revision(),
            *unavailable.temporal_policy(),
            unavailable.utterance().clone(),
        )
        .unwrap(),
    ]
}

fn human_report_frame() -> PeteCognitionFrame {
    frame(
        1,
        "I report a battery charge of 72%. What charge did I report?",
        false,
    )
}

fn execute(
    model: Box<dyn conduit_std_host::hosted_local_model::HostedLocalModelAdapter>,
    frames: Vec<PeteCognitionFrame>,
) -> Vec<conduit_chat::EpisodeRecord> {
    let mut harness = pete_harness(model);
    let mut incoming = inputs(&[]);
    incoming.extend(frames.into_iter().map(frame_input));
    let mut collector = Collector::default();
    let report = run(
        &mut harness,
        &incoming,
        &mut collector,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    let kernel = report.kernel.as_ref().unwrap();
    println!(
        "CONDUIT_PLOT_EVIDENCE={}",
        serde_json::json!({
            "plan_id": harness.plan_id, "play_id": kernel.active_play_id.as_str(),
            "proof_class": "synthetic_inputs_through_installed_kernel",
        })
    );
    let episodes = records(&collector.0);
    for episode in &episodes {
        println!(
            "CONDUIT_MODEL_QUALITY_EPISODE={}",
            serde_json::json!({
                "episode": episode.episode(), "body_id": episode.body_id(),
                "wake_id": episode.wake_id(), "evidence": episode.evidence(),
                "recalled": episode.recalled().get().iter().map(|item| item.episode()).collect::<Vec<_>>(),
                "response": episode.response().map(|response| serde_json::json!({
                    "content": response.content(), "run": response.run(),
                    "implementation": response.implementation(),
                })),
            })
        );
    }
    for episode in &episodes {
        assert!(!*episode.response().unwrap().truncated());
    }
    for pair in episodes.windows(2) {
        assert_eq!(pair[0].body_id(), pair[1].body_id());
        assert_eq!(pair[0].wake_id(), pair[1].wake_id());
        assert_ne!(
            pair[0].response().unwrap().run(),
            pair[1].response().unwrap().run()
        );
    }
    episodes
}

pub(super) fn current(request: &serde_json::Value) -> serde_json::Value {
    common::model_projection::expanded_model_evidence(
        &request["current_experience"],
        request["observation"]["content"].as_str().unwrap(),
    )
}

pub(super) fn homeostasis(evidence: &serde_json::Value) -> serde_json::Value {
    let item = evidence["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["identity"] == "pete/homeostasis")
        .unwrap();
    serde_json::from_str(item["utf8_content"].as_str().unwrap()).unwrap()
}

#[test]
fn qualification_inputs_preserve_current_remembered_and_unavailable_bodily_facts() {
    let (model, requests) = common::fenced_model();
    let episodes = execute(model, bodily_frames());
    assert_eq!(episodes.len(), 4);
    assert_eq!(*episodes[1].recalled().get()[0].episode(), 1);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    let data: Vec<_> = requests
        .iter()
        .map(|request| common::request_data(request))
        .collect();
    assert_eq!(homeostasis(&current(&data[0]))["charge_percent"], "50.0%");
    assert_eq!(homeostasis(&current(&data[1]))["charge_percent"], "9.0%");
    let recalled = &data[1]["recalled"][0]["historical_evidence"]["evidence"][0];
    assert_eq!(recalled["origin"], "deterministic_derived");
    let previous: serde_json::Value =
        serde_json::from_str(recalled["utf8_content"].as_str().unwrap()).unwrap();
    assert_eq!(previous["charge_percent"], "50.0%");
    assert_eq!(homeostasis(&current(&data[2]))["charge_percent"], "9.0%");
    let unavailable = current(&data[3]);
    assert!(homeostasis(&unavailable)["charge_percent"].is_null());
    assert!(unavailable["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| { item["classifications"]["availability"] == "source_unavailable" }));
}

#[test]
fn qualification_human_report_remains_distinct_from_observed_battery() {
    let (model, requests) = common::fenced_model();
    execute(model, vec![human_report_frame()]);
    let requests = requests.lock().unwrap();
    let selected = current(&common::request_data(&requests[0]));
    let report = selected["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["classifications"]["origin"] == "human_reported")
        .unwrap();
    assert!(report["utf8_content"].as_str().unwrap().contains("72%"));
    assert_eq!(homeostasis(&selected)["charge_percent"], "9.0%");
}

fn assert_percentage(answer: &str, expected: &[&str]) {
    let compact: String = answer.chars().filter(|c| !c.is_whitespace()).collect();
    let percentages: Vec<_> = compact
        .split_inclusive('%')
        .filter_map(|part| part.strip_suffix('%'))
        .map(|part| {
            part.rsplit(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
                .next()
                .unwrap()
        })
        .collect();
    assert!(
        matches!(percentages.as_slice(), [value] if expected.contains(value)),
        "{answer}"
    );
}

#[test]
#[ignore = "requires the explicitly selected installed loopback Ollama model"]
fn live_qualification_separates_previous_current_and_unavailable_battery() {
    let model = ollama_episode_model();
    instruction_contract::receipt(model.as_ref(), PETE_PROGRAM);
    let episodes = execute(model, bodily_frames());
    assert_eq!(episodes.len(), 4);
    assert_eq!(*episodes[1].recalled().get()[0].episode(), 1);
    assert_percentage(episodes[1].response().unwrap().content(), &["50", "50.0"]);
    assert_percentage(episodes[2].response().unwrap().content(), &["9", "9.0"]);
    let unavailable = episodes[3].response().unwrap().content();
    let normalized = unavailable.to_ascii_lowercase();
    assert!(
        [
            "do not know",
            "don't know",
            "don’t know",
            "unknown",
            "unavailable",
            "not available",
            "cannot determine"
        ]
        .iter()
        .any(|phrase| normalized.contains(phrase)),
        "{unavailable}"
    );
    assert!(!unavailable.contains('%'), "{unavailable}");
    assert!(
        !normalized.contains("50") && !normalized.contains('9'),
        "{unavailable}"
    );
}

#[test]
#[ignore = "requires the explicitly selected installed loopback Ollama model"]
fn live_qualification_grounds_a_direct_human_report() {
    let model = ollama_episode_model();
    instruction_contract::receipt(model.as_ref(), PETE_PROGRAM);
    let episodes = execute(model, vec![human_report_frame()]);
    let [episode] = episodes.as_slice() else {
        panic!("expected one episode")
    };
    assert!(episode.recalled().get().is_empty());
    assert_percentage(episode.response().unwrap().content(), &["72", "72.0"]);
}
