#[path = "common_episode_loop/mod.rs"]
mod common;

use common::{experience_episode_harness, inputs, records, run, terminal, Collector};
use conduit_chat::{EpisodeExperience, EpisodeObservation};
use conduit_core::{ConnectionTrack, TemporalInstant, TemporalScale, TerminalDisposition};
use conduit_human::ExperienceTemporalPolicy;
use conduit_pete::{
    integrate_pete_experience, project_pete_experience_evidence, select_pete_situation, Observed,
    PeteBodyState, SituationInput,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::{ExternalForeInput, RunControl};

const PROGRAM: &str = include_str!("../../../plots/experience-episode-loop/main.conduit");

fn at(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        clock_basis: "clock/pete-episodes".into(),
        scale: TemporalScale::Milliseconds,
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn episode(revision: u64, message: &str, retain: bool) -> ExternalForeInput {
    let id = format!("utterance/{revision}");
    let now = at(100 * revision);
    let observed = |value| Observed {
        value,
        sign_id: format!("body/{revision}").into(),
        observed_at: now.clone(),
    };
    let situation = select_pete_situation(
        now.clone(),
        10,
        SituationInput::Observed(Observed {
            value: conduit_robotics::BatteryObservation::new(670, 13_000).unwrap(),
            sign_id: format!("battery/{revision}").into(),
            observed_at: now.clone(),
        }),
        SituationInput::Observed(Observed {
            value: message.into(),
            sign_id: id.clone().into(),
            observed_at: now.clone(),
        }),
        SituationInput::Observed(observed(PeteBodyState {
            motion_available: false,
            safety_inhibited: true,
        })),
    )
    .unwrap();
    let experience = integrate_pete_experience(
        &situation,
        ExperienceTemporalPolicy::new(10, 20).unwrap(),
        &[],
    )
    .unwrap();
    let evidence = project_pete_experience_evidence(&experience, revision, "pete/wake/2").unwrap();
    let observation = EpisodeObservation::new(message.into(), id, retain).unwrap();
    ExternalForeInput {
        front_port_id: "experiences".into(),
        track: ConnectionTrack::Payload,
        bytes: EpisodeExperience::new(evidence, observation)
            .unwrap()
            .encode()
            .unwrap(),
    }
}

#[test]
fn pete_situation_evidence_and_episode_recall_reach_one_model_turn() {
    let mut harness = experience_episode_harness(PROGRAM);
    let mut incoming = inputs(&[]);
    incoming.extend([
        episode(1, "The switch label is COPPER KITE.", true),
        episode(2, "What was the switch label?", false),
    ]);
    let mut collector = Collector::default();
    let report = run(
        &mut harness,
        &incoming,
        &mut collector,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    let episodes = records(&collector.0);
    let [first, second] = episodes.as_slice() else {
        panic!("expected two episodes");
    };
    assert_eq!(first.body_id(), second.body_id());
    assert_eq!(first.wake_id(), second.wake_id());
    assert_eq!(
        second.recalled().get()[0].observation(),
        first.observation()
    );
    assert!(second.response().content().contains("COPPER KITE"));

    let requests = harness.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let second: serde_json::Value = serde_json::from_str(&requests[1]).unwrap();
    let selected = &second["selected_experience"];
    assert_eq!(selected["revision"], 2);
    assert_eq!(
        selected["evidence"][0]["identity"],
        "pete/battery/battery/2"
    );
    assert_eq!(
        selected["evidence"][1]["identity"],
        "pete/utterance/utterance/2"
    );
    assert_eq!(
        selected["evidence"][1]["classifications"]["origin"],
        "human_reported"
    );
    assert_eq!(selected["evidence"][2]["identity"], "pete/body/body/2");
    assert_eq!(selected["evidence"][2]["observed_at"]["ticks"], 200);
    assert_eq!(second["recalled"][0]["response"]["origin"], "model_derived");
    let kernel = report.kernel.unwrap();
    println!(
        "CONDUIT_PLOT_EVIDENCE={{\"plan_id\":\"{}\",\"play_id\":\"{}\",\"workload_revision\":null}}",
        harness.plan_id,
        kernel.active_play_id.as_str()
    );
}

#[test]
#[ignore = "requires a running loopback Ollama with gemma3:latest already installed"]
fn live_pete_experience_episode_uses_earlier_memory() {
    use conduit_ai::LocalModelKindProfile;
    use conduit_std_host::hosted_local_model::OllamaDiscovery;

    let adapter = OllamaDiscovery::discover_at("http://127.0.0.1:11434", "gemma3:latest")
        .unwrap()
        .initialize(6144, vec![LocalModelKindProfile::GenerateFlow])
        .unwrap();
    let mut harness = common::experience_episode_harness_with_model(PROGRAM, Box::new(adapter));
    let question = "What exact label did I report for the hallway switch?";
    let mut incoming = inputs(&[]);
    incoming.extend([
        episode(1, "The hallway switch label is COPPER KITE.", true),
        episode(2, question, false),
    ]);
    let mut collector = Collector::default();
    let report = run(
        &mut harness,
        &incoming,
        &mut collector,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    let episodes = records(&collector.0);
    let [first, second] = episodes.as_slice() else {
        panic!("expected two completed episodes");
    };
    assert_eq!(first.body_id(), second.body_id());
    assert_eq!(first.wake_id(), second.wake_id());
    assert_eq!(
        second.recalled().get()[0].observation(),
        first.observation()
    );
    println!("With recall: {}", second.response().content());
    assert!(second
        .response()
        .content()
        .to_ascii_uppercase()
        .contains("COPPER KITE"));

    let mut incoming = inputs(&[]);
    incoming.push(episode(2, question, false));
    let mut control = Collector::default();
    let control_report = run(
        &mut harness,
        &incoming,
        &mut control,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(terminal(&control_report), &TerminalDisposition::Completed);
    let control = records(&control.0);
    assert!(control[0].recalled().get().is_empty());
    println!("Fresh Play: {}", control[0].response().content());
    assert!(!control[0]
        .response()
        .content()
        .to_ascii_uppercase()
        .contains("COPPER KITE"));
}
