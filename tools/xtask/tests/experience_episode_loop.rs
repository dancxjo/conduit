#[path = "../../../targets/std/tests/common_episode_loop/mod.rs"]
mod common;

use common::{experience_episode_harness, inputs, records, run, terminal, Collector};
use conduit_core::{ConnectionTrack, PortDirection, TerminalDisposition};
use conduit_human::ExperienceTemporalPolicy;
use conduit_pete::{
    PeteCognitionBattery, PeteCognitionFrame, PeteCognitionSafety, PeteCognitionSign,
    PeteCognitionUtterance,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::{ExternalForeInput, HostedFrameBack, RunControl, StdHostComposition};
use conduit_time::{NativeTemporalInstant, NativeTemporalScale};

const PROGRAM: &str = include_str!("../../../plots/experience-episode-loop/main.conduit");

const PETE_PROGRAM: &str = include_str!("../../../plots/pete-cognition/main.conduit");

fn pete_harness(
    model: Box<dyn conduit_std_host::hosted_local_model::HostedLocalModelAdapter>,
) -> common::Harness {
    pete_harness_for(model, PETE_PROGRAM, "pete-cognition", None)
}

fn pete_harness_for(
    model: Box<dyn conduit_std_host::hosted_local_model::HostedLocalModelAdapter>,
    program: &str,
    entry: &str,
    checkpoint: Option<&[u8]>,
) -> common::Harness {
    let config = conduit_std_host::StdHostConfig {
        host_id: "host/pete-cognition".into(),
        boot_id: if checkpoint.is_some_and(|bytes| bytes != conduit_chat::initial_episode_memory())
        {
            "boot/pete-cognition-replacement".into()
        } else {
            "boot/pete-cognition".into()
        },
        offer_generation: conduit_core::OfferGeneration(1),
    };
    let back = HostedFrameBack::new(
        conduit_pete::pete_cognition_situate_offer(),
        conduit_pete::MAXIMUM_PETE_COGNITION_FRAME_BYTES as u32,
        conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
        conduit_pete::situate_pete_frame_bytes,
    )
    .unwrap();
    let composition = StdHostComposition::minimal();
    let host = match checkpoint {
        Some(checkpoint) => {
            conduit_std_host::StdHost::new_with_local_model_frame_and_episode_checkpoint(
                config,
                composition,
                model,
                back,
                checkpoint,
            )
        }
        None => conduit_std_host::StdHost::new_with_local_model_and_frame_back(
            config,
            composition,
            model,
            back,
        ),
    }
    .unwrap();
    let mut ports = vec![
        ("context", PortDirection::Input, 32_768),
        (
            "frames",
            PortDirection::Input,
            conduit_pete::MAXIMUM_PETE_COGNITION_FRAME_BYTES as u32,
        ),
        ("episodes", PortDirection::Output, 32_768),
    ];
    if checkpoint.is_some() {
        ports.push((
            "checkpoints",
            PortDirection::Output,
            conduit_chat::MAXIMUM_EPISODE_CHECKPOINT_BYTES as u32,
        ));
    }
    common::plan_host(host, program, entry, &ports, false, |startup, profile| {
        conduit_pete::install_pete_cognition_catalog(startup, profile)?;
        if let Some(initial) = checkpoint {
            conduit_chat::install_episode_resumable_memory_catalog(initial, startup, profile)?;
        }
        Ok(())
    })
}

fn at(ticks: u64) -> NativeTemporalInstant {
    NativeTemporalInstant::new(
        "clock/pete-episodes".into(),
        1,
        NativeTemporalScale::Milliseconds,
        ticks,
        0,
    )
    .unwrap()
}

fn frame(revision: u64, message: &str, retain: bool) -> PeteCognitionFrame {
    let now = at(100 * revision);
    let sign = |kind| PeteCognitionSign::new(format!("{kind}/{revision}")).unwrap();
    PeteCognitionFrame::new(
        PeteCognitionBattery::new(90, false, 13_000, now.clone(), sign("battery")).unwrap(),
        PeteCognitionSafety::new(false, now.clone(), true, sign("body")).unwrap(),
        "pete/wake/2".into(),
        now.clone(),
        revision,
        ExperienceTemporalPolicy::new(10, 20).unwrap(),
        PeteCognitionUtterance::new(message.into(), now, retain, sign("utterance")).unwrap(),
    )
    .unwrap()
}

fn frame_input(frame: PeteCognitionFrame) -> ExternalForeInput {
    ExternalForeInput {
        front_port_id: "frames".into(),
        track: ConnectionTrack::Payload,
        bytes: frame.encode().unwrap(),
    }
}

fn episode(revision: u64, message: &str, retain: bool) -> ExternalForeInput {
    ExternalForeInput {
        front_port_id: "experiences".into(),
        track: ConnectionTrack::Payload,
        bytes: conduit_pete::situate_pete_frame(&frame(revision, message, retain))
            .unwrap()
            .encode()
            .unwrap(),
    }
}

#[test]
fn pete_situation_evidence_and_episode_recall_reach_one_model_turn() {
    assert_recall(
        experience_episode_harness(PROGRAM),
        [
            episode(1, "The switch label is COPPER KITE.", true),
            episode(2, "What was the switch label?", false),
        ],
    );
}

#[test]
fn pete_frames_reach_situation_and_recall_in_one_play() {
    let (model, requests) = common::fenced_model();
    let mut harness = pete_harness(model);
    harness.requests = requests;
    assert!(harness.fragment.placements.iter().any(|gear| {
        gear.kind_id.as_str() == conduit_pete::PETE_COGNITION_SITUATE_KIND
            && gear.implementation_id.as_str() == "std/pete-situate-experience@1"
    }));
    assert_recall(
        harness,
        [
            frame_input(frame(1, "The switch label is COPPER KITE.", true)),
            frame_input(frame(2, "What was the switch label?", false)),
        ],
    );
}

#[test]
fn future_frame_is_refused_before_model_admission() {
    let (model, requests) = common::fenced_model();
    let mut harness = pete_harness(model);
    let frame = frame(1, "What is the label?", true);
    let future = PeteCognitionFrame::new(
        frame.battery().clone(),
        frame.body().clone(),
        frame.provenance_revision().clone(),
        at(99),
        *frame.revision(),
        *frame.temporal_policy(),
        frame.utterance().clone(),
    )
    .unwrap();
    let mut incoming = inputs(&[]);
    incoming.push(frame_input(future));
    let mut collector = Collector::default();
    let failure = run(
        &mut harness,
        &incoming,
        &mut collector,
        &RunControl::default(),
    )
    .expect_err("future observations must refuse");
    assert!(
        failure.contains("pete-cognition/situate") && failure.contains("detail: 3"),
        "{failure}"
    );
    assert!(collector.0.is_empty());
    assert!(requests.lock().unwrap().is_empty());
}

fn assert_recall(mut harness: common::Harness, episodes: [ExternalForeInput; 2]) {
    let mut incoming = inputs(&[]);
    incoming.extend(episodes);
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
    assert!(second.response().unwrap().content().contains("COPPER KITE"));

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
    let derived = &selected["evidence"][3];
    assert_eq!(derived["identity"], "pete/homeostasis");
    assert_eq!(
        derived["classifications"]["origin"],
        "deterministic_derived"
    );
    assert_eq!(
        derived["sources"],
        serde_json::json!([
            {"sign": "battery/2"}, {"sign": "body/2"}
        ])
    );
    let homeostasis: serde_json::Value =
        serde_json::from_str(derived["utf8_content"].as_str().unwrap()).unwrap();
    assert_eq!(homeostasis["energy"], "critical");
    assert_eq!(homeostasis["motion"], "inhibited");
    assert_eq!(homeostasis["thermal"], "unknown");
    assert_eq!(homeostasis["charging"], false);
    assert_eq!(homeostasis["charge_percent"], "9.0%");
    assert!(homeostasis["policy_identity"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
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
    let mut harness = pete_harness(Box::new(adapter));
    let question = "What exact label did I report for the hallway switch?";
    let mut incoming = inputs(&[]);
    incoming.extend([
        frame_input(frame(1, "The hallway switch label is COPPER KITE.", true)),
        frame_input(frame(2, question, false)),
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
    println!("With recall: {}", second.response().unwrap().content());
    assert!(second
        .response()
        .unwrap()
        .content()
        .to_ascii_uppercase()
        .contains("COPPER KITE"));

    let mut incoming = inputs(&[]);
    incoming.push(frame_input(frame(2, question, false)));
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
    println!("Fresh Play: {}", control[0].response().unwrap().content());
    assert!(!control[0]
        .response()
        .unwrap()
        .content()
        .to_ascii_uppercase()
        .contains("COPPER KITE"));
}

#[test]
fn pete_retrieves_the_matching_episode_past_a_newer_unrelated_one() {
    let (model, requests) = common::fenced_model();
    let mut harness = pete_harness(model);
    let mut incoming = inputs(&[]);
    incoming.extend([
        frame_input(frame(1, "The hallway switch label is COPPER KITE.", true)),
        frame_input(frame(2, "The garden is cool and windy.", true)),
        frame_input(frame(
            3,
            "What exact label did I report for the hallway switch?",
            false,
        )),
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
    assert_eq!(episodes.len(), 3);
    assert!(episodes[1].recalled().get().is_empty());
    assert_eq!(episodes[2].recalled().get().len(), 1);
    assert_eq!(*episodes[2].recalled().get()[0].episode(), 1);
    assert!(episodes[2]
        .response()
        .unwrap()
        .content()
        .contains("COPPER KITE"));

    let requests = requests.lock().unwrap();
    let follow_up: serde_json::Value = serde_json::from_str(&requests[2]).unwrap();
    assert_eq!(follow_up["recalled"][0]["shared_observation_terms"], 3);
    assert!(!requests[2].contains("garden"));
}

#[test]
fn pete_records_provider_loss_and_resumes_in_the_same_play() {
    use conduit_chat::EpisodeOutcome;
    use conduit_std_host::hosted_local_model::LocalModelAdapterTerminal;

    let (model, requests) =
        common::fenced_model_with_terminal(Some((2, LocalModelAdapterTerminal::ProviderLost)));
    let mut harness = pete_harness(model);
    let mut incoming = inputs(&[]);
    incoming.extend([
        frame_input(frame(1, "The hallway switch label is COPPER KITE.", true)),
        frame_input(frame(2, "The garden gate is closed.", true)),
        frame_input(frame(3, "What was the hallway switch label?", false)),
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
    let [first, lost, resumed] = episodes.as_slice() else {
        panic!("expected one record for each completed attempt");
    };
    assert_eq!(first.body_id(), resumed.body_id());
    assert_eq!(first.wake_id(), resumed.wake_id());
    assert_eq!(*lost.episode(), 2);
    assert!(lost.response().is_none());
    assert!(!*lost.retained());
    let EpisodeOutcome::ProviderLost(outcome) = lost.outcome() else {
        panic!("expected typed provider loss");
    };
    assert!(outcome.attempt().starts_with("host-attempt/"));
    assert!(!outcome.implementation().is_empty());
    assert_eq!(*resumed.episode(), 3);
    assert_eq!(resumed.recalled().get().len(), 1);
    assert_eq!(*resumed.recalled().get()[0].episode(), 1);
    assert!(resumed
        .response()
        .unwrap()
        .content()
        .contains("COPPER KITE"));
    assert_eq!(requests.lock().unwrap().len(), 3);
}

#[test]
fn pete_recalls_a_committed_checkpoint_in_a_replacement_play() {
    use conduit_chat::{initial_episode_memory, EpisodeMemoryState};
    use conduit_std_host::episode_checkpoint::EpisodeCheckpointResidence;
    use conduit_std_host::{ExternalForeDelivery, ExternalForeOutputAdapter};

    const RESUMABLE: &str = include_str!("../../../plots/pete-cognition-resumable/main.conduit");
    struct DurableOutput<'a> {
        journal: &'a EpisodeCheckpointResidence,
        episodes: Vec<ExternalForeDelivery>,
    }
    impl ExternalForeOutputAdapter for DurableOutput<'_> {
        fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String> {
            if output.front_port_id.as_str() == "checkpoints" {
                self.journal.commit(&output.bytes)?;
            } else {
                self.episodes.push(output);
            }
            Ok(())
        }
    }
    let directory =
        std::env::temp_dir().join(format!("conduit-pete-checkpoint-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let directory = Cleanup(directory);
    let context = common::context();
    let journal = EpisodeCheckpointResidence::open(&directory.0, &context).unwrap();
    let (model, _) = common::fenced_model();
    let empty = journal.recover_seed().unwrap();
    assert_eq!(empty, initial_episode_memory());
    let mut harness = pete_harness_for(model, RESUMABLE, "pete-cognition-resumable", Some(&empty));
    let mut incoming = inputs(&[]);
    incoming.push(frame_input(frame(
        1,
        "The hallway switch label is COPPER KITE.",
        true,
    )));
    let mut collector = DurableOutput {
        journal: &journal,
        episodes: Vec::new(),
    };
    let first = run(
        &mut harness,
        &incoming,
        &mut collector,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(terminal(&first), &TerminalDisposition::Completed);
    drop(collector);
    drop(journal);

    let journal = EpisodeCheckpointResidence::open(&directory.0, &context).unwrap();
    let restored = journal.recover_seed().unwrap();
    assert_eq!(
        *EpisodeMemoryState::decode(&restored).unwrap().completed(),
        1
    );
    let first_plan = harness.plan_id.clone();
    let (model, _) = common::fenced_model();
    let mut harness = pete_harness_for(
        model,
        RESUMABLE,
        "pete-cognition-resumable",
        Some(&restored),
    );
    let mut incoming = inputs(&[]);
    incoming.push(frame_input(frame(
        2,
        "What was the hallway switch label?",
        false,
    )));
    assert_ne!(first_plan, harness.plan_id);
    let mut collector = DurableOutput {
        journal: &journal,
        episodes: Vec::new(),
    };
    let second = run(
        &mut harness,
        &incoming,
        &mut collector,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(terminal(&second), &TerminalDisposition::Completed);
    let first_play = first.kernel.unwrap().active_play_id;
    let second_play = second.kernel.unwrap().active_play_id;
    assert_ne!(first_play, second_play);
    let episodes = records(&collector.episodes);
    assert_eq!(
        *EpisodeMemoryState::decode(&journal.recover_seed().unwrap())
            .unwrap()
            .completed(),
        2
    );
    assert_eq!(episodes.len(), 1);
    let episode = &episodes[0];
    assert_eq!(*episode.episode(), 2);
    assert_eq!(*episode.recalled().get()[0].episode(), 1);
    assert!(episode
        .response()
        .unwrap()
        .content()
        .contains("COPPER KITE"));
    println!(
        "CONDUIT_PLOT_EVIDENCE={{\"plan_id\":\"{}\",\"play_id\":\"{}\",\"workload_revision\":null}}",
        harness.plan_id,
        second_play.as_str()
    );
}

#[test]
fn stale_battery_and_safety_do_not_become_current_homeostatic_facts() {
    let frame = frame(1, "What is the current body state?", false);
    let later = PeteCognitionFrame::new(
        frame.battery().clone(),
        frame.body().clone(),
        frame.provenance_revision().clone(),
        at(200),
        *frame.revision(),
        *frame.temporal_policy(),
        frame.utterance().clone(),
    )
    .unwrap();
    let episode = conduit_pete::situate_pete_frame(&later).unwrap();
    let evidence = episode.evidence().evidence().get();
    assert_eq!(
        *evidence[0].temporal_role(),
        conduit_human::ExperienceTemporalRole::Stale
    );
    let homeostasis = &evidence[3];
    assert!(homeostasis.sources().get().is_empty());
    let content: serde_json::Value =
        serde_json::from_slice(homeostasis.encoded_content().get().as_slice()).unwrap();
    assert_eq!(content["energy"], "unknown");
    assert_eq!(content["motion"], "unknown");
    assert!(content["charging"].is_null());
    assert!(content["charge_percent"].is_null());
}
