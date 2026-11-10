//! Opt-in hosted episode-loop proof with an already installed Ollama model.

#[path = "common_episode_loop/mod.rs"]
mod common;

use common::*;
use conduit_ai::LocalModelKindProfile;
use conduit_core::TerminalDisposition;
use conduit_kernel::KernelEventKind;
use conduit_std_host::{
    hosted_local_model::{HostedLocalModelAdapter, OllamaDiscovery},
    RunControl,
};

#[test]
#[ignore = "requires a running loopback Ollama with gemma3:1b already installed"]
fn installed_ollama_episode_loop_recalls_the_hallway_switch_label() {
    let follow_up = (
        "obs/2",
        "What is the exact label on the hallway light switch?",
        false,
    );
    let adapter = OllamaDiscovery::discover_at("http://127.0.0.1:11434", "gemma3:1b")
        .unwrap()
        .initialize(2048, vec![LocalModelKindProfile::GenerateFlow])
        .unwrap();
    let implementation = format!(
        "{}/{}",
        conduit_ai::LOCAL_MODEL_IMPLEMENTATION,
        adapter.offer().identity.model_content_identity,
    );
    let mut harness = harness_with_model(Box::new(adapter));
    let mut collector = Collector::default();
    let report = run(
        &mut harness,
        &inputs(&[
            (
                "obs/1",
                "The hallway light switch is labeled COPPER KITE.",
                true,
            ),
            follow_up,
        ]),
        &mut collector,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    let episodes = records(&collector.0);
    let [first, second] = episodes.as_slice() else {
        panic!("expected exactly two completed episodes");
    };
    assert_eq!(first.body_id(), second.body_id());
    assert_eq!(first.wake_id(), second.wake_id());
    assert!(first.recalled().get().is_empty());
    let [memory] = second.recalled().get().as_slice() else {
        panic!("expected one selected memory in the second episode");
    };
    assert_eq!(memory.observation(), first.observation());
    assert_eq!(memory.response(), first.response());
    assert_ne!(first.response().run(), second.response().run());
    let kernel = report.kernel.as_ref().expect("installed kernel execution");
    for kind in [
        KernelEventKind::HostCallRequested,
        KernelEventKind::HostCallCompleted,
    ] {
        assert_eq!(
            kernel
                .kernel_sign
                .iter()
                .filter(|event| event.kind == kind)
                .count(),
            2
        );
    }
    println!(
        "Plan {} / Play {}",
        harness.plan_id,
        kernel.active_play_id.as_str()
    );
    for episode in &episodes {
        assert_eq!(episode.response().implementation(), &implementation);
        println!(
            "Episode {} ({}): {}",
            episode.episode(),
            episode.response().run(),
            episode.response().content()
        );
    }
    assert!(
        second
            .response()
            .content()
            .to_ascii_uppercase()
            .contains("COPPER KITE"),
        "the follow-up response must use the label from the earlier observation"
    );

    let mut control = Collector::default();
    let control_report = run(
        &mut harness,
        &inputs(&[follow_up]),
        &mut control,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(terminal(&control_report), &TerminalDisposition::Completed);
    let control_play = &control_report.kernel.as_ref().unwrap().active_play_id;
    assert_ne!(control_play, &kernel.active_play_id);
    let control_episodes = records(&control.0);
    let [unprimed] = control_episodes.as_slice() else {
        panic!("expected exactly one control episode");
    };
    assert!(unprimed.recalled().get().is_empty());
    println!(
        "Same question in a fresh Play: {}",
        unprimed.response().content()
    );
    assert!(
        !unprimed
            .response()
            .content()
            .to_ascii_uppercase()
            .contains("COPPER KITE"),
        "the control must lack the label from the earlier episode"
    );
}
