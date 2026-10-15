//! Real planner/kernel execution with an explicitly named in-memory output sink.
//! The producer fixture supplies only the Face basis, never a successful Mask Show.
use super::*;
use crate::terminal_face_mask::TerminalFaceMask;
#[cfg(unix)]
use crate::{hosted_terminal_mask_host::receive_terminal_frame_and_ack, StdHost};
use conduit_face::*;
#[allow(
    dead_code,
    clippy::duplicate_mod,
    reason = "reuse private renderer test fixture without expanding production visibility"
)]
#[path = "../terminal_face_mask/fixture.rs"]
mod producer_fixture;

fn host() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: "host/terminal-fixture".into(),
        boot_id: "boot/terminal-fixture".into(),
        offer_generation: conduit_core::OfferGeneration(7),
        profile: "std/terminal-fixture".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
fn face(revision: u64) -> Face {
    let plan = producer_fixture::producer_plan();
    Face::new_with_semantics(
        revision,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some(plan.source_document_id),
            checked_plot_id: Some(plan.checked_plot_id),
            expanded_plot_id: Some(plan.expanded_plot_id),
            plan_id: Some(plan.plan_id),
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![FaceSubject {
            identity: "clock".into(),
            role: FaceRole::Document,
            name: "A useful clock".into(),
        }],
        vec![],
        vec![],
        vec![FaceText {
            subject: "clock".into(),
            text: "Ready to start.".into(),
        }],
        vec![FaceAction {
            identity: "start".into(),
            intent: "clock/start".into(),
            target: "clock".into(),
            name: "Start".into(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        }],
        vec![],
    )
    .unwrap()
}
fn show(execution: &mut HostedTerminalMaskExecution, face: &Face) -> MaskShow {
    let mut renderer = TerminalFaceMask::prepare(face.clone(), 80, 24).unwrap();
    let mut fixture_terminal = Vec::new();
    renderer.present(execution, &mut fixture_terminal).unwrap();
    assert!(!fixture_terminal.is_empty());
    renderer.show().unwrap().clone()
}
#[cfg(unix)]
#[test]
fn attached_owner_host_waits_for_actual_foreground_flush_before_available_show() {
    use std::os::unix::net::UnixStream;
    let mut host = StdHost::new();
    let (service, mut foreground) = UnixStream::pair().unwrap();
    host.attach_terminal_mask(service).unwrap();
    let current = host.advertisement().clone();
    assert!(current
        .resources
        .iter()
        .any(|resource| resource.pool_id.as_str() == "terminal/input"));
    assert!(current.capabilities.iter().any(|offer| {
        offer.implementation.implementation_id.as_str() == "face/terminal-input@1"
    }));
    let face = face(7);
    let (finished, release) = std::sync::mpsc::sync_channel::<()>(1);
    let client = std::thread::spawn(move || {
        let mut output = Vec::new();
        let receipt = receive_terminal_frame_and_ack(&mut foreground, &mut output).unwrap();
        release.recv().unwrap();
        (receipt, output)
    });
    let available = host.present_attached_terminal_face(&face).unwrap();
    finished.send(()).unwrap();
    let (receipt, output) = client.join().unwrap();
    assert!(!output.is_empty());
    assert_eq!(receipt.bytes_written as usize, output.len());
    assert_eq!(available.show.lifecycle, ManifestationLifecycle::Available);
    assert_eq!(available.show.host_id, current.host_id);
    assert_eq!(available.show.boot_id, current.boot_id);
    assert_eq!(
        available.planned_mask.plan.fragments[0].offer_generation,
        current.offer_generation
    );
    assert_eq!(available.face_id, face.identity);
    assert_eq!(available.face_revision, face.revision);
    assert_eq!(
        lower_plan_fragment(&available.planned_mask.plan.fragments[0])
            .unwrap()
            .host_calls
            .len(),
        2,
        "attached terminal route plans both output and typed return Host Calls"
    );
}

#[cfg(unix)]
#[test]
fn disconnected_foreground_cannot_produce_an_available_terminal_show() {
    use std::os::unix::net::UnixStream;
    let mut host = StdHost::new();
    let (service, foreground) = UnixStream::pair().unwrap();
    host.attach_terminal_mask(service).unwrap();
    let attached_generation = host.advertisement().offer_generation;
    drop(foreground);
    assert!(host.present_attached_terminal_face(&face(8)).is_err());
    assert!(host.terminal_attachment_mut().is_none());
    assert_ne!(host.advertisement().offer_generation, attached_generation);
}

#[cfg(unix)]
#[test]
fn failed_foreground_flush_never_acknowledges_a_show() {
    use std::{io::Write, os::unix::net::UnixStream};
    struct FailFlush;
    impl Write for FailFlush {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
    }
    let mut host = StdHost::new();
    let (service, mut foreground) = UnixStream::pair().unwrap();
    host.attach_terminal_mask(service).unwrap();
    let client =
        std::thread::spawn(move || receive_terminal_frame_and_ack(&mut foreground, &mut FailFlush));
    assert!(host.present_attached_terminal_face(&face(9)).is_err());
    assert!(client.join().unwrap().is_err());
    assert!(host.terminal_attachment_mut().is_none());
}
#[test]
fn ordinary_mask_runs_to_real_renderer_call_then_show_and_interaction_fore() {
    let face = face(1);
    let mut execution = HostedTerminalMaskExecution::new(&host()).unwrap();
    assert!(conduit_core::verify_plan(&execution.planned_mask().plan));
    assert_eq!(
        execution.planned_mask().plan.fragments[0].offer_generation,
        host().offer_generation
    );
    let available = show(&mut execution, &face);
    assert_eq!(available.show.lifecycle, ManifestationLifecycle::Available);
    assert_ne!(
        available.face_plan_id,
        Some(available.planned_mask.plan.plan_id.clone())
    );
    let interaction = FaceInteraction::new(&face, &available, "start", "clock", vec![], 0).unwrap();
    let correlation = execution.interact(interaction.clone()).unwrap();
    assert_eq!(correlation.interaction, interaction);
    assert_eq!(correlation.show_id, available.show_id);
    assert!(!execution.has_pending_play());
    assert!(execution.interact(interaction).is_err());
}
#[test]
fn renderer_is_pending_until_exact_flush_receipt_and_receipt_cannot_replay() {
    let face = face(1);
    let mut execution = HostedTerminalMaskExecution::new(&host()).unwrap();
    let prepared = execution.begin_render(&face).unwrap();
    assert_eq!(prepared.show.lifecycle, ManifestationLifecycle::Prepared);
    assert!(execution.begin_render(&face).is_err());
    let mut renderer = TerminalFaceMask::prepare(face.clone(), 80, 24).unwrap();
    let receipt = renderer.render(&mut Vec::new(), &prepared).unwrap();
    let available = execution.complete_render(&receipt).unwrap();
    assert_eq!(available.show.lifecycle, ManifestationLifecycle::Available);
    execution.close_without_input().unwrap();
    execution.begin_render(&face).unwrap();
    assert!(matches!(
        execution.complete_render(&receipt),
        Err(TerminalError::StaleShow)
    ));
    assert!(!execution.has_pending_play());
}
#[test]
fn stale_interaction_and_changed_host_retire_pending_execution() {
    let face = face(1);
    let mut execution = HostedTerminalMaskExecution::new(&host()).unwrap();
    let available = show(&mut execution, &face);
    let mut interaction =
        FaceInteraction::new(&face, &available, "start", "clock", vec![], 0).unwrap();
    interaction.face_revision += 1;
    assert!(matches!(
        execution.interact(interaction),
        Err(TerminalError::Interaction(
            FaceInteractionRefusal::StaleFace
        ))
    ));
    assert!(!execution.has_pending_play());
    execution.begin_render(&face).unwrap();
    let mut changed = host();
    changed.offer_generation.0 += 1;
    assert!(execution.validate_current_host(&changed).is_err());
    assert!(!execution.has_pending_play());
    assert!(execution.begin_render(&face).is_err());
}
#[test]
fn cancellation_and_no_input_drain_are_distinct_and_next_play_is_fresh() {
    let face = face(1);
    let mut execution = HostedTerminalMaskExecution::new(&host()).unwrap();
    let first = execution.begin_render(&face).unwrap();
    execution.cancel().unwrap();
    let available = show(&mut execution, &face);
    assert_ne!(first.show.active_play_id, available.show.active_play_id);
    execution.close_without_input().unwrap();
    assert!(!execution.has_pending_play());
    execution.cancel().unwrap();
}
#[test]
fn failed_terminal_write_cancels_kernel_without_available_show() {
    struct Lost;
    impl std::io::Write for Lost {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut execution = HostedTerminalMaskExecution::new(&host()).unwrap();
    let mut renderer = TerminalFaceMask::prepare(face(1), 80, 24).unwrap();
    assert!(matches!(
        renderer.present(&mut execution, &mut Lost),
        Err(TerminalError::Io(std::io::ErrorKind::BrokenPipe))
    ));
    assert!(renderer.show().is_none());
    assert!(!execution.has_pending_play());
}
#[test]
fn oversized_face_refuses_before_admitting_a_play() {
    let face = face(1);
    let text = vec![
        FaceText {
            subject: "clock".into(),
            text: "x".repeat(1024)
        };
        140
    ];
    let large = Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    let mut execution = HostedTerminalMaskExecution::new(&host()).unwrap();
    assert!(matches!(
        execution.begin_render(&large),
        Err(TerminalError::DocumentPressure)
    ));
    assert!(!execution.has_pending_play());
}

#[test]
fn compact_and_spacious_presentations_return_one_unchanged_face_action() {
    use crate::terminal_face_mask::{TerminalInput, TerminalInputOutcome};
    let base = face(9);
    let face = Face::new_with_semantics(
        base.revision, base.basis, base.subjects, base.relationships, base.properties,
        vec![FaceText { subject: "clock".into(), text: "Ready to start this useful clock with the same current action in either presentation.".into() }],
        base.actions, base.disclosures,
    ).unwrap();
    let mut execution = HostedTerminalMaskExecution::new(&host()).unwrap();
    let original_plan = execution.planned_mask().plan.plan_id.clone();
    let mut outputs = Vec::new();
    let mut references = Vec::new();
    let mut shows = Vec::new();
    for width in [40, 80] {
        let mut renderer = TerminalFaceMask::prepare(face.clone(), width, 24).unwrap();
        renderer.focus_prepared_action("start").unwrap();
        let mut sink = Vec::new();
        renderer.present(&mut execution, &mut sink).unwrap();
        let available = renderer.show().unwrap().clone();
        available.validate(&face).unwrap();
        assert_eq!(available.show.lifecycle, ManifestationLifecycle::Available);
        let TerminalInputOutcome::Interaction(interaction) =
            renderer.input(TerminalInput::Apply, &available, 0).unwrap()
        else {
            panic!("current Start action")
        };
        let correlated = execution.interact(interaction).unwrap();
        let value = correlated.interaction;
        assert!(value.arguments.is_empty());
        references.push((
            value.face_id,
            value.face_revision,
            value.action_id,
            value.target,
        ));
        shows.push(available.show_id);
        outputs.push(sink);
        assert_eq!(execution.planned_mask().plan.plan_id, original_plan);
        assert!(!execution.has_pending_play());
    }
    assert_ne!(outputs[0], outputs[1], "actual flushed layouts must differ");
    assert_ne!(shows[0], shows[1]);
    assert_eq!(references[0], references[1]);
    assert_eq!(references[0].0, face.identity.as_str());
    assert_eq!(references[0].2, "start");
    assert_eq!(references[0].3, "clock");
    eprintln!(
        "RENDERER_INTERLOCK_EVIDENCE={}",
        serde_json::json!({
            "proof_class":"native-planner-kernel-with-memory-output-sink",
            "face_id":face.identity.as_str(), "face_revision":face.revision,
            "face_source_id":face.basis.source_document_id.as_ref().unwrap().as_str(),
            "mask_source_id":execution.planned_mask().mask.plot_identity.source_document_id.as_str(),
            "mask_plan_id":original_plan.as_str(),
            "shows":shows.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            "action":"start", "target":"clock", "return_kind":conduit_face::FACE_INTERACTION_VALUE_KIND,
            "compact_output":String::from_utf8_lossy(&outputs[0]),
            "spacious_output":String::from_utf8_lossy(&outputs[1])
        })
    );
}

#[cfg(unix)]
#[path = "remote_tests.rs"]
mod remote;
