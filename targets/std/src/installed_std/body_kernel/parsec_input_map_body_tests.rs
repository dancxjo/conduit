//! Actual combined scan/input Body: software commands retain state while keyboard is pending.
//! A Space observation is not a binding grant or a game-command producer.
use super::*;
use crate::body_execution::{BodyParsecShowObserver, BodyParsecTerminalRoute};
use crate::hosted_keyboard::{HostedKeyboardAdapter, HostedKeyboardPoll};
use conduit_presentation::{ManifestationLifecycle, MaskShow, Presentation};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[test]
fn combined_body_retains_four_source_transitions_then_observes_space_without_a_command() {
    let (mut host, control, queue, wake, plan) = super::input_fixture::prepare();
    let states_seen = Arc::new(AtomicUsize::new(0));
    struct PendingUntilSourceStates {
        states: Arc<AtomicUsize>,
        polls: usize,
        emitted: bool,
    }
    impl HostedKeyboardAdapter for PendingUntilSourceStates {
        fn poll_next(&mut self) -> HostedKeyboardPoll {
            self.polls += 1;
            if !self.emitted && self.states.load(Ordering::SeqCst) == 4 {
                self.emitted = true;
                HostedKeyboardPoll::Event(conduit_human::KeyEvent::decode(&[0x2c, 0, 0]).unwrap())
            } else {
                HostedKeyboardPoll::Pending
            }
        }
    }
    let inputs = [
        command("game/start", false, 1),
        command("game/shoot", false, 2),
        command("game/remap", true, 2),
        command("game/start", true, 3),
    ];
    let (start_tx, start_rx) = mpsc::channel();
    let (ack_tx, ack_rx) = mpsc::channel();
    let producer = queue.clone();
    let sender = std::thread::spawn(move || -> Result<(), String> {
        start_rx.recv().map_err(|error| error.to_string())?;
        for (sequence, input) in inputs.iter().enumerate() {
            let admission = producer.submit(input)?;
            if admission
                != (BodyLiveForeAdmission::Accepted {
                    sequence: sequence as u64,
                })
            {
                return Err(format!("paced source command refused: {admission:?}"));
            }
            ack_rx
                .recv_timeout(Duration::from_secs(3))
                .map_err(|error| error.to_string())?;
        }
        // Both ingress sources remain standing until every output has been drained.
        Ok(())
    });
    struct Capture {
        values: Vec<ExternalForeDelivery>,
        states: Arc<AtomicUsize>,
        ack: Option<mpsc::Sender<()>>,
        control: RunControl,
    }
    impl BodyForeOutputAdapter for Capture {
        fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
            self.values.push(value.clone());
            if value.front_port_id.as_str() == "states" {
                self.states.fetch_add(1, Ordering::SeqCst);
                self.ack
                    .as_ref()
                    .unwrap()
                    .send(())
                    .map_err(|error| error.to_string())?;
            }
            if self.values.len() == 6 {
                self.control
                    .request_stop(
                        crate::RunControlRequestId::new("combined-source-and-input-drained")
                            .unwrap(),
                    )
                    .map_err(|error| format!("cancel standing sources: {error:?}"))?;
            }
            Ok(())
        }
    }
    let (watch_start_tx, watch_start_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let guard_control = control.clone();
    let guard = std::thread::spawn(move || {
        if watch_start_rx.recv().is_err() {
            return;
        }
        if done_rx.recv_timeout(Duration::from_secs(3)).is_err() {
            let _ = guard_control.request_stop(
                crate::RunControlRequestId::new("combined-source-input-watchdog").unwrap(),
            );
        }
    });
    let mut keyboard = PendingUntilSourceStates {
        states: states_seen.clone(),
        polls: 0,
        emitted: false,
    };
    let mut capture = Capture {
        values: Vec::new(),
        states: states_seen,
        ack: Some(ack_tx),
        control: control.clone(),
    };
    #[derive(Default)]
    struct Shows(Vec<(Presentation, MaskShow)>);
    impl BodyParsecShowObserver for Shows {
        fn showed(&mut self, face: &Presentation, show: &MaskShow) -> Result<(), String> {
            show.validate(face)
                .map_err(|error| format!("Show: {error:?}"))?;
            self.0.push((face.clone(), show.clone()));
            Ok(())
        }
    }
    let mut shows = Shows::default();
    let mut terminal = Vec::new();
    let result = host.run_body_plan_with_parsec_terminal_to_with_start(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: Some(&mut keyboard),
        },
        BodyParsecTerminalRoute {
            queue: &queue,
            fore_output: &mut capture,
            terminal_output: &mut terminal,
            observer: &mut shows,
            columns: 80,
            rows: 24,
        },
        &mut Vec::new(),
        &mut NoTimer,
        |_, _| {
            watch_start_tx.send(()).map_err(|error| error.to_string())?;
            start_tx.send(()).map_err(|error| error.to_string())
        },
    );
    // Runtime errors must release both waiters before joining, even before Play starts.
    drop(start_tx);
    drop(watch_start_tx);
    capture.ack.take();
    let _ = done_tx.send(());
    let sent = sender.join().unwrap();
    guard.join().unwrap();
    let report = result.unwrap();
    sent.unwrap();
    assert!(
        !terminal.is_empty(),
        "actual software terminal Write effect"
    );
    let output = std::str::from_utf8(&terminal).unwrap();
    assert!(output.contains("game/start") && output.contains("game/shoot"));
    assert_eq!(
        shows.0.len(),
        3,
        "unchanged maps must not mint another Show"
    );
    let mask_plan = &shows.0[0].1.planned_mask.plan.plan_id;
    let mut mask_plays = std::collections::BTreeSet::new();
    let mut show_ids = std::collections::BTreeSet::new();
    for (index, (face, show)) in shows.0.iter().enumerate() {
        let source = &plan.plots[0].plan;
        assert_eq!(face.revision, index as u64 + 1);
        let expected_basis = conduit_presentation::PresentationBasis {
            body_id: Some(plan.body_id.clone()),
            wake_id: Some(plan.wake_id.clone()),
            source_document_id: Some(source.source_document_id.clone()),
            checked_plot_id: Some(source.checked_plot_id.clone()),
            expanded_plot_id: Some(source.expanded_plot_id.clone()),
            plan_id: Some(plan.plan_id.clone()),
            active_play_id: Some(report.play.active_play_id.clone()),
            sign_ids: vec![],
        };
        assert_eq!(face.basis, expected_basis);
        assert_eq!(face.actions.len(), 1);
        assert_eq!(
            face.actions[0].identity,
            ["game/start", "game/shoot", "game/shoot"][index]
        );
        assert_eq!(face.actions[0].intent, face.actions[0].identity);
        assert_eq!(face.actions[0].target, "parsec/game");
        assert!(face.actions[0].availability.is_available());
        assert_eq!(show.show.lifecycle, ManifestationLifecycle::Available);
        assert_eq!(show.show.host_id, host.advertisement().host_id);
        assert_eq!(show.show.boot_id, host.advertisement().boot_id);
        assert_eq!(&show.planned_mask.plan.plan_id, mask_plan);
        assert_eq!(show.show.plan_id, *mask_plan);
        assert_eq!(show.show.play_sequence, index as u64);
        assert_ne!(show.show.active_play_id, report.play.active_play_id);
        assert!(mask_plays.insert(show.show.active_play_id.clone()));
        assert!(show_ids.insert(show.show_id.clone()));
        show.validate(face).unwrap();
        for previous in shows.0[..index].iter() {
            assert!(previous.1.validate(face).is_err());
        }
    }
    assert!(keyboard.emitted && keyboard.polls > 0);
    assert_eq!(capture.values.len(), 6);
    assert!(capture.values[..4]
        .iter()
        .all(|value| value.front_port_id.as_str() == "states"));
    let states: Vec<_> = capture
        .values
        .iter()
        .filter(|value| value.front_port_id.as_str() == "states")
        .map(|value| {
            ParsecMapTransitionResult::from_structured(
                conduit_core::StructuredInfoValue::from_canonical_bytes(&value.bytes).unwrap(),
            )
            .unwrap()
        })
        .collect();
    assert_eq!(
        states.len(),
        4,
        "Space must not synthesize a fifth game command"
    );
    for (index, accepted, keyboard, revision) in [
        (0, true, false, 2),
        (1, true, false, 2),
        (2, true, true, 3),
        (3, false, true, 3),
    ] {
        assert_eq!(*states[index].accepted(), accepted);
        assert!(*states[index].state().started());
        assert_eq!(*states[index].state().keyboard(), keyboard);
        assert_eq!(*states[index].state().mapping_revision(), revision);
    }
    assert_eq!(
        states[2].state(),
        states[3].state(),
        "stale Start cannot rewrite retained map"
    );
    let keys = capture
        .values
        .iter()
        .find(|value| value.front_port_id.as_str() == "keys")
        .unwrap();
    assert_eq!(keys.bytes, [0x2c, 0, 0]);
    let fire = capture
        .values
        .iter()
        .find(|value| value.front_port_id.as_str() == "fire")
        .unwrap();
    let fire = conduit_core::StructuredInfoValue::from_canonical_bytes(&fire.bytes).unwrap();
    assert_eq!(
        fire.value_type(),
        &conduit_semantic_catalog::input_button_transition_type()
    );
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Cancelled { .. }
    ));
    assert_eq!(
        (
            report.failure.as_ref(),
            report.kernel_failure.as_ref(),
            report.cleanup_failure.as_ref()
        ),
        (None, None, None)
    );
    assert!(!report.scan_cancellation_failed && !report.scan_output_completion_failed);
    let live = report.live_fore_status.unwrap();
    assert_eq!(
        (live.queue_accepted, live.kernel_admitted, live.queued),
        (4, 4, 0)
    );
    assert!(live.play_terminal && queue.status().play_terminal);
    assert!(queue.submit(&command("game/shoot", true, 3)).is_err());
    assert_eq!(
        report.terminal_sign.active_play_id.as_ref(),
        Some(&report.play.active_play_id)
    );
    let receipts = report.scan_child_signs.unwrap();
    assert_eq!(receipts.len(), 4);
    let mut child_ids = std::collections::BTreeSet::new();
    for (invocation, receipt) in receipts.into_iter().enumerate() {
        assert_eq!(usize::from(receipt.invocation), invocation);
        assert_eq!(receipt.parent_active_play_id, report.play.active_play_id);
        assert_ne!(receipt.child_active_play_id, report.play.active_play_id);
        assert!(child_ids.insert(receipt.child_active_play_id));
        assert!(!receipt.selected_plan_id.as_str().is_empty());
        assert!(!receipt.child_host_id.as_str().is_empty());
        assert!(receipt
            .events
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::BackCompleted));
    }
}
