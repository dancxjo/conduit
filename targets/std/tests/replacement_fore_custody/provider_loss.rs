//! The lost selected filesystem provider must not retire another live Host Play.
use super::*;
use conduit_std_host::body_execution::WaitingTodoFore;
use conduit_std_host::BodyLiveForeQueue;
use std::time::Duration;

#[test]
fn provider_loss_and_replacement_leave_independent_resident_play_alive() {
    std::thread::Builder::new()
        .name("replacement-provider-loss".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(prove_partial_loss)
        .unwrap()
        .join()
        .unwrap();
}

fn prove_partial_loss() {
    let (mut steady_host, steady_plan, steady_root) =
        planned_checkpoint_with_config(StdHostConfig {
            host_id: "host-independent".into(),
            boot_id: "steady-boot".into(),
            offer_generation: OfferGeneration(1),
        });
    let steady_control = RunControl::default();
    let queue = BodyLiveForeQueue::for_todo_checkpoint_plan(
        &steady_plan,
        steady_control.clone(),
        TodoState::new("Independent".into())
            .unwrap()
            .encode_info()
            .unwrap(),
        Duration::from_secs(30),
    )
    .unwrap();
    let (steady_wake, steady_body_plan) = body_plan_at_birth(steady_plan, 2);
    let steady_root_worker = steady_root.clone();
    let queue_worker = queue.clone();
    let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
    let worker_control = steady_control.clone();
    let worker = std::thread::Builder::new()
        .name("independent-resident-Play".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(move || {
            let mut captured = CapturedFore::default();
            let report = steady_host
                .run_body_plan_with_waiting_todo_checkpoint_to_with_start(
                    BodyRunRequest {
                        wake: &steady_wake,
                        plan: &steady_body_plan,
                        control: &worker_control,
                        keyboard: None,
                    },
                    WaitingTodoFore {
                        queue: &queue_worker,
                        output: &mut captured,
                    },
                    TodoCheckpointSelection {
                        root: &steady_root_worker,
                        identity: checkpoint_identity(),
                    },
                    &mut Vec::new(),
                    &mut ThreadTimer,
                    |play, _| {
                        started_tx.send(play.clone()).unwrap();
                        Ok(())
                    },
                )
                .unwrap();
            (report, captured)
        })
        .unwrap();
    let steady_play = started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    queue.wait_until_play_started().unwrap();
    assert!(!worker.is_finished());
    assert!(files(&steady_root).is_empty());

    let (mut lost_host, lost_plan, lost_root) = planned_checkpoint();
    let old_plan = lost_plan.clone();
    let (lost_wake, lost_body_plan) = body_plan(lost_plan);
    assert_ne!(steady_play.body_id, lost_body_plan.body_id);
    let mut lost_output = CapturedFore::default();
    let lost = lost_host
        .run_body_plan_with_todo_checkpoint_to_with_start(
            BodyRunRequest {
                wake: &lost_wake,
                plan: &lost_body_plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            BodyForeExchange {
                inputs: &inputs(),
                output: &mut lost_output,
            },
            TodoCheckpointSelection {
                root: &lost_root,
                identity: checkpoint_identity(),
            },
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| {
                // Lose this exact selected provider after admission, before its
                // real publish Host Call. Another Host's selected root stays live.
                std::fs::remove_dir(&lost_root).unwrap();
                std::fs::write(&lost_root, b"selected provider lost").unwrap();
                Ok(())
            },
        )
        .unwrap();
    assert!(matches!(lost.terminal, TerminalDisposition::Failed { .. }));
    assert!(lost.failure.is_some());
    assert!(lost.cleanup_failure.is_none(), "{:?}", lost.cleanup_failure);
    assert!(lost.fore_deliveries.is_empty());
    assert!(
        lost.fore_pending_output.is_none(),
        "no external output was attempted"
    );
    assert!(lost_output.0.is_empty());
    assert!(lost
        .kernel_events
        .iter()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
    assert_eq!(
        lost.terminal_sign.active_play_id.as_ref(),
        Some(&lost.play.active_play_id)
    );
    let old_evidence = format!("{lost:?}");
    drop(lost_host); // All invalid old-Host runtime owners have been retired.
    assert!(!worker.is_finished());
    assert!(!queue.status().play_terminal);
    assert!(files(&steady_root).is_empty());

    let (mut replacement_host, replacement_plan, replacement_root) =
        planned_checkpoint_with_config(StdHostConfig {
            host_id: "host-a".into(),
            boot_id: "fresh-after-loss".into(),
            offer_generation: OfferGeneration(2),
        });
    assert_ne!(replacement_plan.plan_id, old_plan.plan_id);
    assert_eq!(replacement_plan.checked_plot_id, old_plan.checked_plot_id);
    let (replacement_wake, replacement_body_plan) = body_plan(replacement_plan);
    let mut fresh = inputs();
    fresh[1].bytes = TodoCommand::Add {
        text: "New authorized work".into(),
    }
    .encode_info()
    .unwrap();
    let mut replacement_output = CapturedFore::default();
    let replacement = replacement_host
        .run_body_plan_with_todo_checkpoint_to(
            BodyRunRequest {
                wake: &replacement_wake,
                plan: &replacement_body_plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            BodyForeExchange {
                inputs: &fresh,
                output: &mut replacement_output,
            },
            TodoCheckpointSelection {
                root: &replacement_root,
                identity: checkpoint_identity(),
            },
            &mut Vec::new(),
            &mut ThreadTimer,
        )
        .unwrap();
    assert_eq!(replacement.terminal, TerminalDisposition::Completed);
    assert!(replacement.cleanup_failure.is_none());
    assert_ne!(replacement.play.active_play_id, lost.play.active_play_id);
    assert_ne!(
        replacement.terminal_sign.sign_id,
        lost.terminal_sign.sign_id
    );
    assert_eq!(replacement_output.0.len(), 1);
    let state = TodoState::decode_info(&replacement_output.0[0].bytes).unwrap();
    assert_eq!(state.items.len(), 1);
    assert_eq!(state.items[0].text, "New authorized work");
    assert_eq!(format!("{lost:?}"), old_evidence);
    assert_eq!(
        std::fs::read(&lost_root).unwrap(),
        b"selected provider lost"
    );
    assert!(!worker.is_finished());
    assert!(!queue.status().play_terminal);
    queue
        .submit(
            &TodoCommand::Add {
                text: "Still alive".into(),
            }
            .encode_info()
            .unwrap(),
        )
        .unwrap();
    let (steady, steady_output) = worker.join().unwrap();
    assert_eq!(
        steady.play, steady_play,
        "the independent Play must not be replaced or restarted"
    );
    assert_eq!(steady.terminal, TerminalDisposition::Completed);
    assert!(steady.cleanup_failure.is_none());
    assert!(steady.fore_pending_output.is_none());
    assert_eq!(steady_output.0.len(), 1);
    assert_eq!(
        TodoState::decode_info(&steady_output.0[0].bytes)
            .unwrap()
            .items[0]
            .text,
        "Still alive"
    );
    println!(
        "{}",
        serde_json::json!({
            "case":"selected-provider-loss-independent-live-Play",
        "old_body_plan":lost_body_plan, "replacement_body_plan":replacement_body_plan,
        "old_plan":lost.play.plan_id.as_str(), "replacement_plan":replacement.play.plan_id.as_str(),
        "replacement_items":state.items.iter().map(|item|item.text.as_str()).collect::<Vec<_>>(),
        "old_report":format!("{lost:?}"), "replacement_report":format!("{replacement:?}"),
        "unaffected_report":format!("{steady:?}"),
            "old_play":lost.play.active_play_id.as_str(), "old_terminal_sign":lost.terminal_sign.sign_id.as_str(),
            "replacement_play":replacement.play.active_play_id.as_str(), "replacement_terminal_sign":replacement.terminal_sign.sign_id.as_str(),
            "unaffected_body_id":steady.play.body_id.as_str(),
            "unaffected_play":steady.play.active_play_id.as_str(), "unaffected_terminal_sign":steady.terminal_sign.sign_id.as_str(),
            "old_refusal":lost.failure, "unaffected_outputs":steady_output.0.len(), "replacement_outputs":replacement_output.0.len(),
        })
    );
    std::fs::remove_file(lost_root).unwrap();
    std::fs::remove_dir_all(replacement_root).unwrap();
    std::fs::remove_dir_all(steady_root).unwrap();
}
