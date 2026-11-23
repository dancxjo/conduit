//! Simulated clock-domain conformance on two native Hosts, with a fenced model.
//! This proves omission/custody, not distinct-process clocks or clock correlation.
use super::*;
use conduit_chat::{decode_episode_checkpoint, EpisodeMemoryOwner};
use serde_json::{json, Value};

const SOURCE: &str = include_str!("../../../../plots/pete-cognition-resumable/main.conduit");
const SOURCE_CLOCK: &str = "clock/simulated/source";
const FOREIGN_CLOCK: &str = "clock/simulated/foreign";
const SOURCE_SIGN: &str = "clock-source/utterance/1";

#[test]
fn same_basis_checkpoint_recalls_original_source_time_and_sign() {
    checkpoint_clock_case(false);
}

#[test]
fn foreign_basis_equal_counter_omits_history_without_destroying_it() {
    checkpoint_clock_case(true);
}

fn instant(basis: &str, uncertainty: u64) -> NativeTemporalInstant {
    NativeTemporalInstant::new(
        basis.into(),
        1,
        NativeTemporalScale::Milliseconds,
        100,
        uncertainty,
    )
    .unwrap()
}

fn utterance(basis: &str, revision: u64, retain: bool) -> PeteCognitionFrame {
    PeteCognitionFrame::new(
        PeteCognitionBatteryInput::Missing,
        None,
        "clock-source/provenance/1".into(),
        instant(basis, 0),
        revision,
        ExperienceTemporalPolicy::new(10, 20).unwrap(),
        PeteCognitionUtterance::new(
            if retain {
                "The hallway label is COPPER KITE."
            } else {
                "What was the hallway label?"
            }
            .into(),
            instant(basis, if retain { 2 } else { 0 }),
            retain,
            PeteCognitionSign::new(
                if retain {
                    SOURCE_SIGN
                } else {
                    "clock-query/utterance/2"
                }
                .into(),
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

// Only the fixture's Host/Boot and checkpoint vary; all ordinary Plot contracts,
// installed backs, request limits and checkpoint limits stay unchanged.
fn harness(host_id: &str, boot_id: &str, seed: &[u8]) -> common::Harness {
    let (model, requests) = common::fenced_model();
    let back = HostedFrameBack::new(
        conduit_pete::pete_cognition_inputs_offer(),
        conduit_pete::MAXIMUM_PETE_COGNITION_FRAME_BYTES as u32,
        conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
        conduit_pete::prepare_pete_experience_input_bytes,
    )
    .unwrap();
    let host = conduit_std_host::StdHost::new_with_local_model_frame_and_episode_checkpoint(
        conduit_std_host::StdHostConfig {
            host_id: host_id.into(),
            boot_id: boot_id.into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        model,
        back,
        seed,
    )
    .unwrap();
    let mut harness = common::plan_host(
        host,
        SOURCE,
        "pete-cognition-resumable",
        &[
            ("context", PortDirection::Input, 32_768),
            (
                "frames",
                PortDirection::Input,
                conduit_pete::MAXIMUM_PETE_COGNITION_FRAME_BYTES as u32,
            ),
            ("episodes", PortDirection::Output, 32_768),
            (
                "checkpoints",
                PortDirection::Output,
                conduit_chat::MAXIMUM_EPISODE_CHECKPOINT_BYTES as u32,
            ),
        ],
        false,
        |startup, profile| {
            conduit_pete::install_pete_cognition_catalog(startup, profile)?;
            conduit_chat::install_episode_resumable_memory_catalog(seed, startup, profile)
        },
    );
    harness.requests = requests;
    harness
}

fn execute(
    harness: &mut common::Harness,
    frame: PeteCognitionFrame,
    foreign: bool,
    phase: &str,
) -> (
    conduit_std_host::StdRunReport,
    Vec<u8>,
    conduit_chat::EpisodeRecord,
    Value,
) {
    use sha2::{Digest, Sha256};
    let mut incoming = inputs(&[]);
    incoming.push(frame_input(frame));
    let mut collector = Collector::default();
    let result = run(harness, &incoming, &mut collector, &RunControl::default());
    let requests = harness.requests.lock().unwrap();
    // Emit before terminal/count/quality assertions: a first refusal remains
    // evidence, rather than being retried or hidden by a larger fixture budget.
    println!(
        "CONDUIT_CLOCK_BOUNDARY_PROGRESS={}",
        json!({
            "schema":"conduit.pete/clock-boundary-proof@1", "phase":phase,
            "case":if foreign {"foreign_basis"} else {"same_basis"},
            "proof_class":"simulated_inputs_fenced_model_native_plan_play",
            "source_sha256":format!("{:x}", Sha256::digest(SOURCE.as_bytes())),
            "host_id":harness.fragment.host_id.as_str(),
            "boot_id":harness.fragment.boot_id.as_str(), "plan_id":harness.plan_id,
            "play_id":result.as_ref().ok().and_then(|r| r.kernel.as_ref()).map(|k| k.active_play_id.as_str()),
            "run_error":result.as_ref().err(),
            "request_bytes":requests.iter().map(String::len).collect::<Vec<_>>(),
            "deliveries":collector.0.iter().map(|d| json!({"port":d.front_port_id.as_str(),"bytes":d.bytes.len()})).collect::<Vec<_>>(),
            "source_clock":{"basis":SOURCE_CLOCK,"ticks":100,"resolution_ticks":1,"uncertainty_ticks":2,"scale":"milliseconds"},
            "receiver_clock":{"basis":if foreign {FOREIGN_CLOCK} else {SOURCE_CLOCK},"ticks":100},
        })
    );
    let report = result.expect("ordinary resumable Pete Play");
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    assert_eq!(requests.len(), 1, "exactly one fenced model call per Play");
    assert!(requests[0].len() <= 4096);
    let request = common::request_data(&requests[0]);
    println!(
        "CONDUIT_CLOCK_BOUNDARY_SELECTION={}",
        json!({
            "phase":phase, "reference":request["reference"],
            "retrieval":request["retrieval"],
            "recalled_episodes":request["recalled"].as_array().map(|items| items.iter().map(|item| item["episode"].clone()).collect::<Vec<_>>()),
            "request_sha256":format!("{:x}", Sha256::digest(requests[0].as_bytes())),
        })
    );
    let episodes: Vec<_> = collector
        .0
        .iter()
        .filter(|d| d.front_port_id.as_str() == "episodes")
        .cloned()
        .collect();
    let mut episodes = records(&episodes);
    assert_eq!(episodes.len(), 1);
    let checkpoint = collector
        .0
        .iter()
        .rev()
        .find(|d| d.front_port_id.as_str() == "checkpoints")
        .unwrap()
        .bytes
        .to_vec();
    assert!(checkpoint.len() <= 8192);
    (report, checkpoint, episodes.remove(0), request)
}

fn source_item(evidence: &Value) -> &Value {
    evidence["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| {
            item["sources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|source| source["sign"] == SOURCE_SIGN)
        })
        .expect("original human-report source Sign")
}

fn assert_source_time(evidence: &Value) {
    let item = source_item(evidence);
    assert_eq!(item["origin"], "human_reported");
    assert_eq!(item["sources"], json!([{"sign":SOURCE_SIGN}]));
    assert_eq!(
        item["observed_at"],
        json!({
            "clock_basis":SOURCE_CLOCK, "scale":"milliseconds", "ticks":100,
            "resolution_ticks":1, "uncertainty_ticks":2,
        })
    );
    assert_eq!(
        item["utf8_content"],
        json!({"text":"The hallway label is COPPER KITE."}).to_string()
    );
}

fn checkpoint_clock_case(foreign: bool) {
    let source_time = instant(SOURCE_CLOCK, 2);
    let receiver_time = instant(if foreign { FOREIGN_CLOCK } else { SOURCE_CLOCK }, 0);
    let initial = conduit_chat::initial_episode_memory();
    let mut source = harness("host/clock-source", "boot/clock-source/1", &initial);
    let (first, seed, first_episode, _) = execute(
        &mut source,
        utterance(SOURCE_CLOCK, 1, true),
        foreign,
        "source",
    );
    let saved = decode_episode_checkpoint(&seed).unwrap();
    assert_eq!(*saved.completed(), 1);
    assert_eq!(saved.memories().get().len(), 1);
    assert_eq!(*first_episode.episode(), 1);
    assert!(*first_episode.retained());
    assert!(first_episode.recalled().get().is_empty());
    let context = common::context();
    let EpisodeMemoryOwner::Bound(owner) = saved.owner() else {
        panic!("bound checkpoint owner")
    };
    assert_eq!(owner.body_id(), context.body_id.as_str());
    assert_eq!(owner.wake_id(), context.wake_id.as_str());
    assert_eq!(*owner.wake_sequence(), context.wake_sequence);
    let memory = &saved.memories().get()[0];
    assert_eq!(*memory.episode(), 1);
    assert_eq!(memory.observation(), first_episode.observation());
    let archived: Value = serde_json::from_str(memory.evidence().as_deref().unwrap()).unwrap();
    let archived = common::model_projection::expanded_model_evidence(
        &archived,
        memory.observation().content(),
    );
    assert_eq!(archived["revision"], 1);
    assert_eq!(archived["provenance_revision"], "clock-source/provenance/1");
    assert_source_time(&archived);
    // These complete inputs retain the utterance plus a sourced-time
    // homeostatic reduction; missing battery/body produce no fabricated sample.
    let items = archived["evidence"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    for item in items {
        assert_eq!(item["observed_at"]["clock_basis"], SOURCE_CLOCK);
    }

    let mut receiver = harness("host/clock-receiver", "boot/clock-receiver/2", &seed);
    let (second, next, episode, request) = execute(
        &mut receiver,
        utterance(if foreign { FOREIGN_CLOCK } else { SOURCE_CLOCK }, 2, false),
        foreign,
        "receiver",
    );
    assert_ne!(source.fragment.host_id, receiver.fragment.host_id);
    assert_ne!(source.fragment.boot_id, receiver.fragment.boot_id);
    assert_ne!(source.plan_id, receiver.plan_id);
    assert_ne!(
        first.kernel.unwrap().active_play_id,
        second.kernel.unwrap().active_play_id
    );
    assert_eq!(*episode.episode(), 2);
    assert!(!*episode.retained());
    for record in [&first_episode, &episode] {
        assert_eq!(record.body_id(), context.body_id.as_str());
        assert_eq!(record.wake_id(), context.wake_id.as_str());
        assert_eq!(*record.wake_sequence(), context.wake_sequence);
    }
    assert_eq!(request["body_id"], context.body_id.as_str());
    assert_eq!(request["wake_id"], context.wake_id.as_str());
    assert_eq!(request["wake_sequence"], context.wake_sequence);
    assert_eq!(request["reference"]["ticks"], 100);
    assert_eq!(
        request["reference"]["clock_basis"],
        receiver_time.clock_basis().as_str()
    );
    let restored = decode_episode_checkpoint(&next).unwrap();
    assert_eq!(*restored.completed(), 2);
    assert_eq!(
        restored.owner().clone().encode().unwrap(),
        saved.owner().clone().encode().unwrap()
    );
    assert_eq!(
        restored.memories().clone().encode().unwrap(),
        saved.memories().clone().encode().unwrap(),
        "foreign evidence stays byte-identical in native checkpoint custody"
    );
    assert_eq!(
        restored
            .memories()
            .get()
            .iter()
            .map(|m| *m.episode())
            .collect::<Vec<_>>(),
        vec![1]
    );

    if foreign {
        assert_eq!(source_time.ticks(), receiver_time.ticks());
        assert_eq!(
            source_time.relation_to(&receiver_time),
            Err(conduit_core::TemporalRelationError::Incomparable)
        );
        assert!(episode.recalled().get().is_empty());
        assert_eq!(request["recalled"], json!([]));
        assert_eq!(request["retrieval"]["temporal"], "no_history");
        let expected: Vec<_> = items
            .iter()
            .enumerate()
            .map(|(item, _)| json!({"episode":1,"item":item,"reason":"clock_mismatch"}))
            .collect();
        assert_eq!(request["retrieval"]["temporal_omissions"], json!(expected));
        // no_history is scoped to this verified window: the checkpoint above
        // still owns episode 1; this is neither global absence nor a boundary gap.
        assert!(request["retrieval"]["context_omissions"]
            .as_array()
            .unwrap()
            .is_empty());
    } else {
        assert!(source_time.relation_to(&receiver_time).is_ok());
        assert_eq!(episode.recalled().get().len(), 1);
        assert_eq!(*episode.recalled().get()[0].episode(), 1);
        assert_eq!(request["retrieval"]["temporal"], "selected");
        assert_eq!(request["retrieval"]["temporal_omissions"], json!([]));
        assert_eq!(request["recalled"].as_array().unwrap().len(), 1);
        assert_eq!(request["recalled"][0]["episode"], 1);
        let history = &request["recalled"][0]["historical_evidence"];
        assert_eq!(history["original_provenance"]["revision"], 1);
        assert_eq!(
            history["original_provenance"]["provenance_revision"],
            "clock-source/provenance/1"
        );
        let history = common::model_projection::expanded_model_evidence(
            history,
            memory.observation().content(),
        );
        assert_source_time(&history);
    }
}
