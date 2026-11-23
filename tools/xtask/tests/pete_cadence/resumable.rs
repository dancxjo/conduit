//! One seeded cadenced resumable Play; the model and sensor inputs are fenced.
use super::*;

const SOURCE: &str = include_str!("../../../../plots/pete-cognition-cadenced/resumable.conduit");

fn harness_with_seed(
    model: Box<dyn conduit_std_host::hosted_local_model::HostedLocalModelAdapter>,
    seed: &[u8],
) -> common::Harness {
    let back = HostedFrameBack::new(
        conduit_pete::pete_cognition_inputs_offer(),
        conduit_pete::MAXIMUM_PETE_COGNITION_FRAME_BYTES as u32,
        conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
        conduit_pete::prepare_pete_experience_input_bytes,
    )
    .unwrap();
    let mut host = conduit_std_host::StdHost::new_with_local_model_frame_and_episode_checkpoint(
        conduit_std_host::StdHostConfig {
            host_id: "host/pete-cadence-resumable".into(),
            boot_id: "boot/pete-cadence-resumable".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        model,
        back,
        seed,
    )
    .unwrap();
    let (value, trigger) = contracts();
    host.install_finite_current_sampling(&value, &trigger)
        .unwrap();
    common::plan_host(
        host,
        SOURCE,
        "pete-cognition",
        &[
            ("context", PortDirection::Input, 32768),
            (
                "frames",
                PortDirection::Input,
                conduit_pete::MAXIMUM_PETE_COGNITION_FRAME_BYTES as u32,
            ),
            ("reflections", PortDirection::Input, 8),
            (
                "reactions",
                PortDirection::Output,
                conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
            ),
            (
                "attention",
                PortDirection::Output,
                conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
            ),
            ("episodes", PortDirection::Output, 32768),
            (
                "checkpoints",
                PortDirection::Output,
                conduit_chat::MAXIMUM_EPISODE_CHECKPOINT_BYTES as u32,
            ),
        ],
        false,
        |startup, profile| {
            conduit_pete::install_pete_cognition_catalog(startup, profile)?;
            conduit_chat::install_episode_resumable_memory_catalog(seed, startup, profile)?;
            conduit_semantic_catalog::install_flow_coalesce_finite_kind(&value, startup, profile)?;
            conduit_semantic_catalog::install_current_sample_finite_kind(
                &value, &trigger, startup, profile,
            )
        },
    )
}

#[test]
fn seeded_cadenced_resumable_play_emits_checkpoint_and_bounded_instruction_requests() {
    let (model, requests) = common::fenced_model();
    let seed = conduit_chat::initial_episode_memory();
    let mut harness = harness_with_seed(model, &seed);
    let mut incoming = common::inputs(&[]);
    incoming.extend([
        frame(1, "The hallway label is COPPER KITE."),
        trigger(1),
        frame(2, "What was the hallway label?"),
        trigger(2),
    ]);
    let mut collector = common::Collector::default();
    let report = common::run(
        &mut harness,
        &incoming,
        &mut collector,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(common::terminal(&report), &TerminalDisposition::Completed);
    let episodes: Vec<_> = collector
        .0
        .iter()
        .filter(|item| item.front_port_id.as_str() == "episodes")
        .map(|item| conduit_chat::EpisodeRecord::decode(&item.bytes).unwrap())
        .collect();
    assert_eq!(episodes.len(), 2);
    assert_eq!(episodes[0].body_id(), episodes[1].body_id());
    assert_eq!(episodes[0].wake_id(), episodes[1].wake_id());
    assert_eq!(*episodes[1].recalled().get()[0].episode(), 1);
    assert!(episodes[1]
        .response()
        .unwrap()
        .content()
        .contains("COPPER KITE"));

    let checkpoints: Vec<_> = collector
        .0
        .iter()
        .filter(|item| item.front_port_id.as_str() == "checkpoints")
        .collect();
    assert!(checkpoints
        .iter()
        .all(|item| item.bytes.len() <= conduit_chat::MAXIMUM_EPISODE_CHECKPOINT_BYTES));
    let restored =
        conduit_chat::EpisodeMemoryState::decode(&checkpoints.last().unwrap().bytes).unwrap();
    assert_eq!(*restored.completed(), 2);
    assert_eq!(restored.memories().get().len(), 1);
    assert_eq!(*restored.memories().get()[0].episode(), 1);

    let expected = SOURCE
        .split_once("instruction = \"")
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap();
    assert_eq!(expected.len(), 404);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    for request in requests.iter() {
        assert!(request.len() <= 4096);
        let context = conduit_ai::decode_generation_context(request)
            .unwrap()
            .unwrap();
        assert_eq!(context.instruction(), expected);
    }
    println!(
        "CONDUIT_PLOT_EVIDENCE={}",
        serde_json::json!({
            "proof_class": "fenced_seeded_cadenced_resumable_play",
            "plan_id": harness.plan_id,
            "play_id": report.kernel.as_ref().unwrap().active_play_id.as_str(),
            "checkpoint_completed": restored.completed(),
            "model_request_bytes": requests.iter().map(String::len).collect::<Vec<_>>(),
        })
    );
}
