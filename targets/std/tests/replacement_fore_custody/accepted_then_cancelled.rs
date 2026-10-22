//! Acknowledged in-flight output remains old-Play truth after cancellation.
use super::*;
use conduit_std_host::RunControlRequestId;

struct AcceptThenCancel {
    control: RunControl,
    received: Vec<ExternalForeDelivery>,
}
impl BodyForeOutputAdapter for AcceptThenCancel {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        self.received.push(output.clone());
        self.control
            .request_stop(RunControlRequestId::new("cancel-after-output-accepted").unwrap())
            .unwrap();
        Ok(())
    }
}

#[test]
fn accepted_output_is_drained_before_cancellation_and_not_replayed_by_replacement() {
    std::thread::Builder::new()
        .name("accepted-output-cancellation".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(prove_cancelled_custody)
        .unwrap()
        .join()
        .unwrap();
}

fn prove_cancelled_custody() {
    let (mut host, plan, root) = planned_checkpoint();
    let (wake, body) = body_plan(plan);
    let control = RunControl::default();
    let mut output = AcceptThenCancel {
        control: control.clone(),
        received: Vec::with_capacity(1),
    };
    let old = host
        .run_body_plan_with_todo_checkpoint_to(
            BodyRunRequest {
                wake: &wake,
                plan: &body,
                control: &control,
                keyboard: None,
            },
            BodyForeExchange {
                inputs: &inputs(),
                output: &mut output,
            },
            TodoCheckpointSelection {
                root: &root,
                identity: checkpoint_identity(),
            },
            &mut Vec::new(),
            &mut ThreadTimer,
        )
        .unwrap();
    assert!(matches!(
        old.terminal,
        TerminalDisposition::Cancelled { .. }
    ));
    assert!(old.failure.is_none(), "{:?}", old.failure);
    assert!(old.cleanup_failure.is_none());
    assert!(old.fore_pending_output.is_none());
    assert_eq!(output.received.len(), 1);
    assert_eq!(old.fore_deliveries, output.received);
    let offered = old
        .kernel_events
        .iter()
        .find(|event| event.kind == conduit_kernel::KernelEventKind::RemoteValueOffered)
        .unwrap()
        .sequence;
    let delivered = old
        .kernel_events
        .iter()
        .find(|event| event.kind == conduit_kernel::KernelEventKind::RemoteValueDelivered)
        .unwrap()
        .sequence;
    let cancelled = old
        .kernel_events
        .iter()
        .find(|event| event.kind == conduit_kernel::KernelEventKind::RunCancelled)
        .unwrap()
        .sequence;
    assert!(offered < delivered && delivered < cancelled);
    assert_eq!(
        old.terminal_sign.active_play_id.as_ref(),
        Some(&old.play.active_play_id)
    );
    let evidence = format!("{old:?}");
    let original_files = files(&root);

    let (mut replacement_host, replacement_plan, replacement_root) =
        planned_checkpoint_with_version(
            StdHostConfig {
                host_id: "host-a".into(),
                boot_id: "after-cancel-boot".into(),
                offer_generation: OfferGeneration(2),
            },
            [3; 32],
        );
    let (replacement_wake, replacement_body) = body_plan(replacement_plan);
    copy_checkpoint(&original_files, &replacement_root);
    let mut fresh = inputs();
    fresh[0].bytes = old.fore_deliveries[0].bytes.as_slice().to_vec();
    fresh[1].bytes = TodoCommand::Add {
        text: "After cancellation".into(),
    }
    .encode_info()
    .unwrap();
    let mut captured = CapturedFore::default();
    let replacement = replacement_host
        .run_body_plan_with_todo_checkpoint_to(
            BodyRunRequest {
                wake: &replacement_wake,
                plan: &replacement_body,
                control: &RunControl::default(),
                keyboard: None,
            },
            BodyForeExchange {
                inputs: &fresh,
                output: &mut captured,
            },
            TodoCheckpointSelection {
                root: &replacement_root,
                identity: checkpoint_identity(),
            },
            &mut Vec::new(),
            &mut ThreadTimer,
        )
        .unwrap();
    assert_eq!(
        replacement.terminal,
        TerminalDisposition::Completed,
        "{:?}",
        replacement.failure
    );
    assert!(replacement.cleanup_failure.is_none());
    assert_ne!(old.play.plan_id, replacement.play.plan_id);
    assert_ne!(old.play.active_play_id, replacement.play.active_play_id);
    assert_ne!(old.terminal_sign.sign_id, replacement.terminal_sign.sign_id);
    assert_eq!(captured.0.len(), 1);
    let state = TodoState::decode_info(&captured.0[0].bytes).unwrap();
    assert_eq!(state.items.len(), 2);
    assert_eq!(state.items[0].text, "Buy milk");
    assert_eq!(state.items[1].text, "After cancellation");
    assert_eq!(format!("{old:?}"), evidence);
    assert_eq!(files(&root), original_files);
    assert_eq!(output.received.len(), 1);
    println!(
        "{}",
        serde_json::json!({
            "case":"accepted-output-drained-before-cancelled-replacement",
        "old_body_plan":body, "replacement_body_plan":replacement_body,
        "old_report":format!("{old:?}"), "replacement_report":format!("{replacement:?}"),
        "replacement_items":state.items.iter().map(|item|item.text.as_str()).collect::<Vec<_>>(),
            "old_plan":old.play.plan_id.as_str(), "old_play":old.play.active_play_id.as_str(),
            "old_terminal_sign":old.terminal_sign.sign_id.as_str(),
            "offered_event":offered, "delivered_event":delivered, "cancelled_event":cancelled,
            "replacement_plan":replacement.play.plan_id.as_str(), "replacement_play":replacement.play.active_play_id.as_str(),
            "replacement_terminal_sign":replacement.terminal_sign.sign_id.as_str(),
            "old_output_count":old.fore_deliveries.len(), "replacement_output_count":captured.0.len(),
        })
    );
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(replacement_root).unwrap();
}
