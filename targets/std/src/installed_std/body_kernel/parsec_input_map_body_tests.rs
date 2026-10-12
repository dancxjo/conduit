//! Actual combined scan/input Body: software commands retain state while keyboard is pending.
//! A Space observation is not a binding grant or a game-command producer.
use super::*;
use crate::hosted_keyboard::{HostedKeyboardAdapter, HostedKeyboardPoll};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[test]
fn combined_body_retains_four_source_transitions_then_observes_space_without_a_command() {
    let mut host = BodyLiveForeQueue::parsec_input_test_host(StdHostConfig {
        host_id: "std/combined-map-input-proof".into(),
        boot_id: "std/combined-map-input-proof/boot".into(),
        offer_generation: OfferGeneration(1),
    });
    let selected = BodyLiveForeQueue::authored_parsec_input_plan_on_host(host.advertisement());
    let control = RunControl::default();
    let queue = BodyLiveForeQueue::for_parsec_plan(&selected, control.clone(), 1).unwrap();
    let body = Body::born(
        selected.source_document_id.clone(),
        selected.checked_plot_id.clone(),
        1,
        SignId::from("sign/combined-map-input-born"),
    )
    .unwrap();
    let wake = body
        .wake(1, SignId::from("sign/combined-map-input-wake"))
        .unwrap()
        .1;
    let plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                selected.source_document_id.clone(),
                selected.checked_plot_id.clone(),
            ),
            plan: selected,
        }],
    )
    .unwrap();
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
    let result = host.run_body_plan_with_live_fore_to_with_start(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: Some(&mut keyboard),
        },
        &queue,
        &mut capture,
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
    let keys: Vec<_> = capture
        .values
        .iter()
        .filter(|value| value.front_port_id.as_str() == "keys")
        .collect();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].bytes, [0x2c, 0, 0]);
    let fire: Vec<_> = capture
        .values
        .iter()
        .filter(|value| value.front_port_id.as_str() == "fire")
        .collect();
    assert_eq!(fire.len(), 1);
    assert_eq!(
        conduit_core::StructuredInfoValue::from_canonical_bytes(&fire[0].bytes)
            .unwrap()
            .value_type(),
        &conduit_semantic_catalog::input_button_transition_type()
    );
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Cancelled { .. }
    ));
    assert!(report.failure.is_none(), "{:?}", report.failure);
    assert!(
        report.kernel_failure.is_none(),
        "{:?}",
        report.kernel_failure
    );
    assert!(
        report.cleanup_failure.is_none(),
        "{:?}",
        report.cleanup_failure
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
