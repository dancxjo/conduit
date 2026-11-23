//! Finite checkpoint/trace turnover through the ordinary resumable Pete Plot.
//! The existing full frame fixture and fenced model are unchanged. A refusal
//! before episode 32 is a product prerequisite, not permission to shrink facts,
//! drop retained frames, increase bounds, or fabricate a replay gap.
use super::*;
use conduit_chat::{EpisodeMemoryState, EpisodeRecall, EpisodeRecord};
use conduit_plot::rust_binding::BoundedSequence;
use std::collections::BTreeSet;

const SOURCE: &str = include_str!("../../../../plots/pete-cognition-resumable/main.conduit");
const FRAME_COUNT: u64 = 32;
const TRACE_RECORD_BOUND: usize = 128;

#[test]
fn thirty_two_retained_frames_preserve_checkpoints_and_expose_trace_horizon() {
    let seed = conduit_chat::initial_episode_memory();
    let (model, requests) = common::fenced_model();
    let mut harness = pete_harness_for(model, SOURCE, "pete-cognition-resumable", Some(&seed));
    let context = common::context();
    let mut incoming = inputs(&[]);
    for revision in 1..=FRAME_COUNT {
        let content = format!("Marker {revision:02} observed.");
        incoming.push(frame_input(frame(revision, &content, true)));
    }
    let mut collector = Collector::default();
    let result = run(
        &mut harness,
        &incoming,
        &mut collector,
        &RunControl::default().with_debug_body(context.body_id.clone()),
    );
    let requests = requests.lock().unwrap();
    let request_bytes: Vec<_> = requests.iter().map(String::len).collect();
    let episode_outputs: Vec<_> = collector
        .0
        .iter()
        .filter(|item| item.front_port_id.as_str() == "episodes")
        .collect();
    let checkpoints: Vec<_> = collector
        .0
        .iter()
        .filter(|item| item.front_port_id.as_str() == "checkpoints")
        .collect();
    // Retain measured progress before any terminal/count assertion. In
    // particular, pre-fact-view request overflow must remain a visible failure.
    println!(
        "CONDUIT_PETE_TRACE_HORIZON_PROGRESS={}",
        serde_json::json!({
            "proof_class": "fenced_finite_pete_checkpoint_trace_turnover",
            "declared_retained_frames": FRAME_COUNT,
            "observed_episodes": episode_outputs.len(),
            "model_calls": requests.len(), "model_request_bytes": request_bytes,
            "checkpoint_bytes": checkpoints.iter().map(|item| item.bytes.len()).collect::<Vec<_>>(),
            "last_checkpoint_completed": checkpoints.last()
                .and_then(|item| conduit_chat::decode_episode_checkpoint(&item.bytes).ok())
                .map(|state| *state.completed()),
            "run_error": result.as_ref().err(),
            "last_observation": result.as_ref().ok()
                .and_then(|report| report.observations.last())
                .map(|observation| format!("{:?}", observation.kind)),
        })
    );
    let report = result.expect("all 32 retained frames must complete under existing bounds");
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    assert_eq!(episode_outputs.len(), FRAME_COUNT as usize);
    assert_eq!(requests.len(), FRAME_COUNT as usize);
    assert!(request_bytes.iter().all(|bytes| *bytes <= 4096));

    let episodes: Vec<_> = episode_outputs
        .iter()
        .map(|output| conduit_chat::EpisodeRecord::decode(&output.bytes).unwrap())
        .collect();
    for (index, episode) in episodes.iter().enumerate() {
        assert_eq!(*episode.episode(), index as u64 + 1);
        assert!(*episode.retained());
        assert_eq!(episode.body_id().as_str(), context.body_id.as_str());
        assert_eq!(episode.wake_id().as_str(), context.wake_id.as_str());
        assert_eq!(*episode.wake_sequence(), context.wake_sequence);
    }
    let owner = conduit_chat::EpisodeMemoryOwner::bound(
        episodes[0].body_id().clone(),
        episodes[0].wake_id().clone(),
        *episodes[0].wake_sequence(),
    )
    .unwrap();
    assert_eq!(checkpoints.len(), FRAME_COUNT as usize + 1);
    assert_eq!(checkpoints[0].bytes, seed);
    let states: Vec<_> = checkpoints
        .iter()
        .enumerate()
        .map(|(completed, checkpoint)| {
            assert!(checkpoint.bytes.len() <= conduit_chat::MAXIMUM_EPISODE_CHECKPOINT_BYTES);
            let state = conduit_chat::decode_episode_checkpoint(&checkpoint.bytes).unwrap();
            assert_eq!(*state.completed(), completed as u64);
            assert_eq!(state.clone().encode().unwrap(), checkpoint.bytes);
            if completed != 0 {
                assert_eq!(state.owner(), &owner);
            }
            state
        })
        .collect();
    let mut rejected_suffixes = Vec::new();
    for (transition, episode) in states.windows(2).zip(&episodes) {
        if let Some((candidate, bytes)) =
            assert_maximal_checkpoint_suffix(&transition[0], &transition[1], episode)
        {
            rejected_suffixes.push((*episode.episode(), candidate, bytes));
        }
    }
    let final_checkpoint = checkpoints.last().unwrap();
    let state = states.last().unwrap();
    assert_eq!(*state.completed(), FRAME_COUNT);
    let retained: Vec<_> = state
        .memories()
        .get()
        .iter()
        .map(|memory| *memory.episode())
        .collect();
    assert_eq!(retained, vec![30, 31, 32]);
    let (completed, candidate, rejected_bytes) = rejected_suffixes.last().unwrap();
    assert_eq!(*completed, FRAME_COUNT);
    assert_eq!(candidate, &[29, 30, 31, 32]);
    assert!(*rejected_bytes > conduit_chat::MAXIMUM_EPISODE_CHECKPOINT_BYTES);
    for memory in state.memories().get() {
        let number = *memory.episode();
        assert_eq!(
            memory.observation().identity(),
            &format!("utterance/{number}")
        );
        assert_eq!(
            memory.observation().content(),
            &format!("Marker {number:02} observed.")
        );
        assert!(memory.evidence().is_some());
    }

    let kernel = report.kernel.as_ref().unwrap();
    let history = kernel.debug_history.as_ref().unwrap();
    assert!(!history.records.is_empty());
    assert!(history.records.len() <= TRACE_RECORD_BOUND);
    let gap = history
        .gap
        .expect("32 episodes must cross the retained trace horizon");
    assert!(gap.dropped_records > 0);
    assert_eq!(gap.first_retained_sequence, history.records[0].sequence);
    let snapshot =
        pete_replay::snapshot(&harness.fragment, &harness.authoring, &context, &report).unwrap();
    let timeline = snapshot.timeline.as_ref().unwrap();
    let projected_gap = timeline.gap.as_ref().unwrap();
    assert_eq!(projected_gap.dropped_records, gap.dropped_records);
    assert_eq!(
        projected_gap.first_retained_sequence,
        gap.first_retained_sequence
    );
    let projection = snapshot.timeline_projection.as_ref().unwrap();
    assert!(!projection.exact_reconstruction);
    let trace = timeline.trace.as_ref().unwrap();
    assert!(!trace.missing_parent_sequences.is_empty());

    // Verify unavailable ancestry against the actual kernel parent edges.
    // Timeline projection may lose additional records to its own byte bound;
    // every reported missing parent must still come from a real traced record.
    let retained_sequences: BTreeSet<_> =
        timeline.events.iter().map(|event| event.sequence).collect();
    let mut missing_from_kernel = BTreeSet::new();
    for step in &trace.steps {
        let raw = history
            .records
            .iter()
            .find(|record| record.sequence == step.sequence)
            .unwrap();
        for parent in raw
            .causal_parent_sequence
            .into_iter()
            .chain(raw.additional_parent_sequences.into_iter().flatten())
        {
            if !retained_sequences.contains(&parent) {
                missing_from_kernel.insert(parent);
            }
        }
    }
    let reported_missing: BTreeSet<_> = trace.missing_parent_sequences.iter().copied().collect();
    assert_eq!(reported_missing, missing_from_kernel);

    use sha2::{Digest, Sha256};
    println!(
        "CONDUIT_PETE_TRACE_HORIZON_EVIDENCE={}",
        serde_json::json!({
            "proof_class": "fenced_finite_pete_checkpoint_trace_turnover",
            "plot_source_sha256": format!("{:x}", Sha256::digest(SOURCE.as_bytes())),
            "host_id": harness.fragment.host_id.as_str(),
            "plan_id": harness.plan_id, "play_id": kernel.active_play_id.as_str(),
            "body_id": episodes[0].body_id(), "wake_id": episodes[0].wake_id(),
            "wake_sequence": episodes[0].wake_sequence(),
            "episodes": episodes.len(), "model_calls": requests.len(),
            "model_request_bytes": request_bytes,
            "checkpoint_bytes": final_checkpoint.bytes.len(),
            "checkpoint_completed": state.completed(), "retained_episodes": retained,
            "byte_eviction_transitions": rejected_suffixes.len(),
            "nearest_rejected_episodes": candidate,
            "nearest_rejected_checkpoint_bytes": rejected_bytes,
            "retained_debug_records": history.records.len(),
            "dropped_debug_records": gap.dropped_records,
            "first_retained_sequence": gap.first_retained_sequence,
            "timeline_evicted_events": timeline.evicted_events,
            "exact_reconstruction": projection.exact_reconstruction,
            "unavailable_parent_sequences": trace.missing_parent_sequences,
        })
    );
}

/// Inspect actual checkpoints, not a second memory cell or the production fit
/// function. Oversized counterfactuals exist only in this test after Play.
fn assert_maximal_checkpoint_suffix(
    previous: &EpisodeMemoryState,
    current: &EpisodeMemoryState,
    record: &EpisodeRecord,
) -> Option<(Vec<u64>, usize)> {
    let kept = current.memories().get().as_slice();
    assert!(!kept.is_empty());
    assert!(kept.len() <= conduit_chat::MAXIMUM_EPISODE_RECALL);
    assert_eq!(current.completed(), record.episode());
    let retained: Vec<_> = kept.iter().map(|memory| *memory.episode()).collect();
    let first = *current.completed() + 1 - kept.len() as u64;
    assert_eq!(retained, (first..=*current.completed()).collect::<Vec<_>>());
    let newest = kept.last().unwrap();
    assert_eq!(newest.observation(), record.observation());
    assert_eq!(newest.outcome(), record.outcome());
    assert!(newest.evidence().is_some());

    let available = previous.memories().get().as_slice();
    let survivors = &kept[..kept.len() - 1];
    assert!(survivors.len() <= available.len());
    // Equality covers every field, including complete evidence, observation
    // and actual outcome. Previously evicted records cannot reappear.
    assert_eq!(survivors, &available[available.len() - survivors.len()..]);
    if kept.len() == conduit_chat::MAXIMUM_EPISODE_RECALL || survivors.len() == available.len() {
        return None;
    }

    // One more older record remains within the four-entry ceiling. If this
    // nearest rejected suffix is too large, every still-longer suffix is too.
    let excluded = &available[available.len() - survivors.len() - 1];
    assert_eq!(*excluded.episode() + 1, first);
    let memories =
        BoundedSequence::try_from_iter(std::iter::once(excluded).chain(kept.iter()).cloned())
            .unwrap();
    let candidate = EpisodeMemoryState::new(
        *current.completed(),
        EpisodeRecall::new(memories).unwrap(),
        current.owner().clone(),
    )
    .unwrap();
    let identities = candidate
        .memories()
        .get()
        .iter()
        .map(|memory| *memory.episode())
        .collect();
    let bytes = candidate.encode().unwrap();
    assert!(
        bytes.len() > conduit_chat::MAXIMUM_EPISODE_CHECKPOINT_BYTES,
        "episode {} evicted a complete suffix that still fits: {} bytes",
        current.completed(),
        bytes.len()
    );
    Some((identities, bytes.len()))
}
