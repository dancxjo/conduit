//! Durable recall of successful and unsuccessful selected episodes.
use super::*;

#[test]
fn pete_recalls_a_committed_checkpoint_in_a_replacement_play() {
    assert_checkpoint_recall(false, false);
}
#[test]
fn pete_recalls_observation_after_provider_loss_and_checkpoint_recovery() {
    assert_checkpoint_recall(true, false);
}

#[test]
#[ignore = "requires a running loopback Ollama with gemma3:latest already installed"]
fn live_pete_recalls_observation_retained_during_provider_loss() {
    assert_checkpoint_recall(true, true);
}

fn assert_checkpoint_recall(provider_lost: bool, live_recovery: bool) {
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
    let directory = std::env::temp_dir().join(format!(
        "conduit-pete-checkpoint-{}-{provider_lost}-{live_recovery}",
        std::process::id()
    ));
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
    let (model, _) = common::fenced_model_with_terminal(provider_lost.then_some((
        1,
        conduit_std_host::hosted_local_model::LocalModelAdapterTerminal::ProviderLost,
    )));
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
    let completed = records(&collector.episodes);
    assert!(*completed[0].retained());
    assert_eq!(completed[0].response().is_none(), provider_lost);
    drop(collector);
    drop(journal);

    let journal = EpisodeCheckpointResidence::open(&directory.0, &context).unwrap();
    let restored = journal.recover_seed().unwrap();
    assert_eq!(
        *EpisodeMemoryState::decode(&restored).unwrap().completed(),
        1
    );
    let saved = EpisodeMemoryState::decode(&restored).unwrap();
    assert_eq!(
        saved.memories().get()[0].response().is_none(),
        provider_lost
    );
    let first_plan = harness.plan_id.clone();
    let model = if live_recovery {
        ollama_episode_model()
    } else {
        common::fenced_model().0
    };
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
    if live_recovery {
        incoming.push(frame_input(frame(
            3,
            "What was the earlier model provider outcome?",
            false,
        )));
    }
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
        if live_recovery { 3 } else { 2 }
    );
    assert_eq!(episodes.len(), if live_recovery { 2 } else { 1 });
    let episode = &episodes[0];
    assert_eq!(*episode.episode(), 2);
    assert_eq!(*episode.recalled().get()[0].episode(), 1);
    assert_eq!(
        episode.recalled().get()[0].response().is_none(),
        provider_lost
    );
    assert_eq!(
        episode.recalled().get()[0].outcome(),
        completed[0].outcome()
    );
    let historical: serde_json::Value =
        serde_json::from_str(episode.recalled().get()[0].evidence().as_ref().unwrap()).unwrap();
    assert_eq!(historical["revision"], 1);
    assert_eq!(
        historical["evidence"][0]["sources"][0]["sign"],
        "utterance/1"
    );
    let recalled_answer = episode.response().unwrap().content();
    assert!(recalled_answer.contains("COPPER KITE"), "{recalled_answer}");
    if live_recovery {
        let answer = episodes[1].response().unwrap().content();
        println!("Recalled provider outcome: {answer}");
        assert!(answer.contains("provider_lost"), "{answer}");
    }
    println!(
        "CONDUIT_PLOT_EVIDENCE={{\"plan_id\":\"{}\",\"play_id\":\"{}\",\"workload_revision\":null}}",
        harness.plan_id,
        second_play.as_str()
    );
}

#[test]
fn pete_checkpoint_keeps_an_answering_experience_after_retaining_its_followup_question() {
    const SOURCE: &str = include_str!("../../../plots/pete-cognition-resumable/main.conduit");
    let initial = conduit_chat::initial_episode_memory();
    let mut harness = pete_harness_for(
        common::fenced_model().0,
        SOURCE,
        "pete-cognition-resumable",
        Some(&initial),
    );
    let mut incoming = inputs(&[]);
    incoming.extend([
        frame_input(frame(1, "The hallway switch label is COPPER KITE.", true)),
        frame_input(frame(2, "What was the hallway switch label?", true)),
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
    let checkpoint = collector
        .0
        .iter()
        .rev()
        .find(|item| item.front_port_id.as_str() == "checkpoints")
        .unwrap()
        .bytes
        .to_vec();
    let state = conduit_chat::decode_episode_checkpoint(&checkpoint).unwrap();
    assert_eq!(
        state
            .memories()
            .get()
            .iter()
            .map(|memory| *memory.episode())
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    let first_plan = harness.plan_id;
    let mut resumed = pete_harness_for(
        common::fenced_model().0,
        SOURCE,
        "pete-cognition-resumable",
        Some(&checkpoint),
    );
    assert_ne!(first_plan, resumed.plan_id);
    let mut incoming = inputs(&[]);
    incoming.push(frame_input(frame(
        3,
        "What was the hallway switch label?",
        false,
    )));
    let mut collector = Collector::default();
    let report = run(
        &mut resumed,
        &incoming,
        &mut collector,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    let episode = collector
        .0
        .iter()
        .find(|item| item.front_port_id.as_str() == "episodes")
        .unwrap();
    let episode = conduit_chat::EpisodeRecord::decode(&episode.bytes).unwrap();
    assert_eq!(*episode.episode(), 3);
    assert!(episode
        .recalled()
        .get()
        .iter()
        .any(|memory| *memory.episode() == 1));
}

#[test]
fn pete_checkpoint_upgrades_the_previous_current_schema_residence_without_losing_provenance() {
    use conduit_std_host::{
        episode_checkpoint::EpisodeCheckpointResidence,
        state_value::{DurableStateBinding, FileDurableStateResidence},
    };
    const SOURCE: &str = include_str!("../../../plots/pete-cognition-resumable/main.conduit");
    let initial = conduit_chat::initial_episode_memory();
    let mut harness = pete_harness_for(
        common::fenced_model().0,
        SOURCE,
        "pete-cognition-resumable",
        Some(&initial),
    );
    let mut incoming = inputs(&[]);
    incoming.push(frame_input(frame(
        1,
        "The hallway label is COPPER KITE.",
        true,
    )));
    let mut collector = Collector::default();
    run(
        &mut harness,
        &incoming,
        &mut collector,
        &RunControl::default(),
    )
    .unwrap();
    let checkpoint = collector
        .0
        .iter()
        .rev()
        .find(|item| item.front_port_id.as_str() == "checkpoints")
        .unwrap()
        .bytes
        .to_vec();
    assert!(checkpoint.len() <= 4096);
    let context = common::context();
    let directory = std::env::temp_dir().join(format!(
        "conduit-episode-current-profile-upgrade-{}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let previous = FileDurableStateResidence::open(
        &directory,
        DurableStateBinding {
            body: context.body_id.as_str().into(),
            state: format!("body/episode-checkpoint/{}", context.wake_id.as_str()).into(),
            value_kind: conduit_chat::episode_memory_state_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
            maximum_value_bytes: 4096,
        },
    )
    .unwrap();
    previous.commit(1, &checkpoint).unwrap();
    let current = EpisodeCheckpointResidence::open(&directory, &context).unwrap();
    assert_eq!(current.recover_seed().unwrap(), checkpoint);
    drop(current);
    let reopened = EpisodeCheckpointResidence::open(&directory, &context).unwrap();
    assert_eq!(reopened.recover_seed().unwrap(), checkpoint);
    drop(reopened);
    std::fs::remove_dir_all(directory).unwrap();
}
