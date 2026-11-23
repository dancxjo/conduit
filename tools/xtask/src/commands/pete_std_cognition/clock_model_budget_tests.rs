//! Real operator-shaped frames and planning, with injected time/UART and a fenced model.
//! This measures request compatibility, not model quality or physical observations.
use super::{identity, receipt, wire_frame, CaptureContext, Provider};
use crate::commands::pete_std_cognition::{
    clock::ClockSource, frame_with_namespace_using, planning,
};
use conduit_core::{ConnectionTrack, TerminalDisposition};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::{
    ExternalForeInput, HostedFrameBack, RunControl, StdHost, StdHostComposition, StdHostConfig,
};
use serde_json::Value;

#[path = "../../../../../targets/std/tests/common_episode_loop/mod.rs"]
mod common;

const BASIS: &str = "clock/linux-monotonic@1/12345678-1234-4234-8234-123456789abc/0/0";
const CADENCED_NAMESPACE: &str = "0123456789abcdef0123";
const UTTERANCES: [&str; 2] = [
    "The hallway label is COPPER KITE.",
    "What was the hallway label?",
];

struct OperatorClock(u64);
impl ClockSource for OperatorClock {
    fn basis(&self) -> &str {
        BASIS
    }
    fn millis(&mut self) -> Result<u64, String> {
        let ticks = self.0;
        self.0 += 1;
        Ok(ticks)
    }
}

#[test]
fn ordinary_operator_clock_frames_fit_installed_model_and_checkpoint_bounds() {
    operator_budget(false);
}

#[test]
fn cadenced_operator_clock_frames_fit_installed_model_and_checkpoint_bounds() {
    operator_budget(true);
}

fn operator_budget(cadenced: bool) {
    assert_eq!(BASIS.len(), 64);
    assert_eq!(CADENCED_NAMESPACE.len(), 20);
    let namespace = if cadenced { CADENCED_NAMESPACE } else { "" };
    let seed = conduit_chat::initial_episode_memory();
    let authoring = if cadenced {
        planning::authoring_cadenced(Some(&seed))
    } else {
        planning::authoring(Some(&seed))
    }
    .unwrap();
    let context = planning::context(&authoring).unwrap();
    let (model, requests) = common::fenced_model();
    let back = HostedFrameBack::new(
        conduit_pete::pete_cognition_inputs_offer(),
        conduit_pete::MAXIMUM_PETE_COGNITION_FRAME_BYTES as u32,
        conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
        conduit_pete::prepare_pete_experience_input_bytes,
    )
    .unwrap();
    let mode = if cadenced { "cadenced" } else { "ordinary" };
    let mut host = StdHost::new_with_local_model_frame_and_episode_checkpoint(
        StdHostConfig {
            host_id: format!("host/pete-clock-budget/{mode}").into(),
            boot_id: format!("boot/pete-clock-budget/{mode}").into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        model,
        back,
        &seed,
    )
    .unwrap();
    let fragment = if cadenced {
        planning::plan_cadenced(&mut host, &authoring, true)
    } else {
        planning::plan(&host, &authoring, true)
    }
    .unwrap();
    let mut inputs = vec![ExternalForeInput {
        front_port_id: "context".into(),
        track: ConnectionTrack::Payload,
        bytes: conduit_chat::encode_body_conversation_context(&context).unwrap(),
    }];
    let selected = identity();
    let mut clock = OperatorClock(1_234_567_890);
    let mut frame_bytes = Vec::new();
    for (index, utterance) in UTTERANCES.into_iter().enumerate() {
        let revision = index as u64 + 1;
        let mut provider = Provider {
            bytes: wire_frame(2_400).into(),
            ..Provider::default()
        };
        let mut capture_receipt = Vec::new();
        let frame = frame_with_namespace_using(
            Some(&mut provider),
            CaptureContext {
                identity: Some(&selected),
                revision,
                namespace,
            },
            utterance.into(),
            index == 0,
            &mut clock,
            &mut capture_receipt,
        )
        .unwrap();
        assert!(
            frame.body().is_none(),
            "the operator supplies no safety sample"
        );
        assert!(matches!(
            frame.battery(),
            conduit_pete::PeteCognitionBatteryInput::Observed(_)
        ));
        assert_eq!(receipt(&capture_receipt)["clock"]["basis"], BASIS);
        let bytes = frame.encode().unwrap();
        frame_bytes.push(bytes.len());
        inputs.push(ExternalForeInput {
            front_port_id: "frames".into(),
            track: ConnectionTrack::Payload,
            bytes,
        });
        if cadenced {
            inputs.push(ExternalForeInput {
                front_port_id: "reflections".into(),
                track: ConnectionTrack::Payload,
                bytes: revision.to_le_bytes().to_vec(),
            });
        }
    }
    let mut collector = common::Collector::default();
    let result = host.run_external_plot_sequence_controlled_to(
        fragment.clone(),
        &inputs,
        &mut collector,
        &mut Vec::new(),
        &mut common::NoopTimer,
        &RunControl::default(),
    );
    let requests = requests.lock().unwrap();
    let deliveries: Vec<_> = collector
        .0
        .iter()
        .filter(|item| item.front_port_id.as_str() == "episodes")
        .cloned()
        .collect();
    let checkpoints: Vec<_> = collector
        .0
        .iter()
        .filter(|item| item.front_port_id.as_str() == "checkpoints")
        .collect();
    println!(
        "CONDUIT_PETE_OPERATOR_CLOCK_BUDGET={}",
        serde_json::json!({
            "proof_class":"operator_full_frames_fenced_model_installed_kernel", "mode":mode,
            "clock_basis":BASIS, "clock_basis_bytes":BASIS.len(), "frame_namespace":namespace,
            "source_document_id":authoring.expanded.source_document_id.as_str(),
            "checked_plot_id":authoring.expanded.checked_plot_id.as_str(),
            "expanded_plot_id":authoring.expanded.expanded_plot_id.as_str(),
            "host_id":fragment.host_id.as_str(), "boot_id":fragment.boot_id.as_str(),
            "plan_id":fragment.plan_id.as_str(),
            "play_id":result.as_ref().ok().and_then(|r| r.kernel.as_ref()).map(|k| k.active_play_id.as_str()),
            "run_error":result.as_ref().err(), "frame_bytes":frame_bytes,
            "model_calls":requests.len(), "model_request_bytes":requests.iter().map(String::len).collect::<Vec<_>>(),
            "episode_deliveries":deliveries.len(),
            "checkpoint_bytes":checkpoints.iter().map(|item| item.bytes.len()).collect::<Vec<_>>(),
        })
    );
    let report = result.expect("operator-shaped frames fit the unchanged installed bounds");
    assert_eq!(common::terminal(&report), &TerminalDisposition::Completed);
    assert!(report.kernel.is_some());
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|request| request.len() <= 4096));
    let episodes = common::records(&deliveries);
    assert_eq!(episodes.len(), 2);
    assert!(episodes[0].recalled().get().is_empty());
    assert_eq!(episodes[1].recalled().get().len(), 1);
    assert_eq!(*episodes[1].recalled().get()[0].episode(), 1);
    assert_eq!(
        episodes[1].recalled().get()[0].observation(),
        episodes[0].observation()
    );
    for (index, (request, episode)) in requests.iter().zip(&episodes).enumerate() {
        assert_eq!(*episode.episode(), index as u64 + 1);
        assert_eq!(episode.body_id(), context.body_id.as_str());
        assert_eq!(episode.wake_id(), context.wake_id.as_str());
        assert_eq!(*episode.wake_sequence(), context.wake_sequence);
        assert_eq!(episode.observation().content(), UTTERANCES[index]);
        let request = common::request_data(request);
        assert_eq!(request["recalled"].as_array().unwrap().len(), index);
        if index == 1 {
            assert_eq!(request["recalled"][0]["episode"], 1);
        }
        let current = common::model_projection::expanded_model_evidence(
            &request["current_experience"],
            episode.observation().content(),
        );
        let archived: Value = serde_json::from_str(episode.evidence().as_deref().unwrap()).unwrap();
        let archived = common::model_projection::expanded_model_evidence(
            &archived,
            episode.observation().content(),
        );
        assert_eq!(current["evidence"].as_array().unwrap().len(), 3);
        assert_eq!(current["evidence"], archived["evidence"]);
        assert_eq!(current["revision"], archived["revision"]);
        assert_eq!(
            current["provenance_revision"],
            archived["provenance_revision"]
        );
        for item in current["evidence"].as_array().unwrap() {
            assert_eq!(item["observed_at"]["clock_basis"], BASIS);
        }
    }
    assert!(!checkpoints.is_empty());
    for checkpoint in &checkpoints {
        assert!(checkpoint.bytes.len() <= conduit_chat::MAXIMUM_EPISODE_CHECKPOINT_BYTES);
    }
    let memory =
        conduit_chat::decode_episode_checkpoint(&checkpoints.last().unwrap().bytes).unwrap();
    assert_eq!(*memory.completed(), 2);
    assert_eq!(memory.memories().get().len(), 1);
    assert_eq!(*memory.memories().get()[0].episode(), 1);
}
