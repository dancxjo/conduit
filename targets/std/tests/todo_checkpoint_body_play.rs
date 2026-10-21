use conduit_core::*;
use conduit_std_host::body_execution::{
    BodyForeExchange, BodyRunRequest, TodoCheckpointSelection, WaitingTodoFore,
};
use conduit_std_host::todo_durable_resource::{SelectedTodoResidence, READ_OPERATION};
use conduit_std_host::{RunControl, ThreadTimer};
use conduit_todo_plot::{TodoCommand, TodoState, STATE_MAX_BYTES};

#[path = "common/checkpoint_fixture.rs"]
mod checkpoint_fixture;
use checkpoint_fixture::*;

#[test]
fn authored_plan_selects_exact_installed_backs() {
    let (_host, plan, root) = planned_checkpoint();
    let placements = &plan.fragments[0].placements;
    assert_eq!(placements.len(), 2);
    let combine = placements
        .iter()
        .find(|p| p.kind_id == conduit_todo_plot::todo_combine_kind().kind_id)
        .unwrap();
    assert_eq!(combine.capability_id.as_str(), "std-todo-combine-v1");
    assert_eq!(
        combine.execution_profile_id.as_str(),
        conduit_std_offers::TODO_COMBINE_PROFILE
    );
    let checkpoint = placements
        .iter()
        .find(|p| p.kind_id == conduit_todo_plot::todo_checkpoint_kind().kind_id)
        .unwrap();
    assert_eq!(
        checkpoint.implementation_id.as_str(),
        conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION
    );
    assert_eq!(checkpoint.resources.len(), 1);
    assert_eq!(checkpoint.authority.len(), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn public_body_play_publishes_before_committed_fore_delivery() {
    std::thread::Builder::new()
        .name("todo-checkpoint-body-proof".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(public_body_play_proof)
        .unwrap()
        .join()
        .unwrap();
}

fn public_body_play_proof() {
    let (mut host, plan, root) = planned_checkpoint();
    let write = plan.fragments[0]
        .placements
        .iter()
        .find(|p| p.kind_id == conduit_todo_plot::todo_checkpoint_kind().kind_id)
        .unwrap()
        .clone();
    let (wake, body_plan) = body_plan(plan);
    let control = RunControl::default();
    let mut fore = CapturedFore::default();
    let mut output = Vec::new();
    let mut timer = ThreadTimer;
    let mut started = false;
    let report = host
        .run_body_plan_with_todo_checkpoint_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &body_plan,
                control: &control,
                keyboard: None,
            },
            BodyForeExchange {
                inputs: &inputs(),
                output: &mut fore,
            },
            TodoCheckpointSelection {
                root: &root,
                identity: checkpoint_identity(),
            },
            &mut output,
            &mut timer,
            |_, _| {
                started = true;
                Ok(())
            },
        )
        .unwrap();
    assert!(started);
    assert_eq!(report.terminal, TerminalDisposition::Completed);
    assert!(report.failure.is_none(), "{:?}", report.failure);
    assert_eq!(fore.0.len(), 1);
    let mut read = write;
    read.host_id = "host-b".into();
    read.boot_id = "boot-b".into();
    read.host_calls[0].contract_id = READ_OPERATION.into();
    read.host_calls[0].maximum_input_bytes = 0;
    read.host_calls[0].maximum_output_bytes = STATE_MAX_BYTES as u32;
    read.resources[0].content.as_mut().unwrap().contract.access = ResourceAccessMode::ReadPublished;
    read.resources[0]
        .content
        .as_mut()
        .unwrap()
        .contract
        .publication_slots = 0;
    read.resources[0].content.as_mut().unwrap().owner_host = read.host_id.clone();
    read.resources[0].content.as_mut().unwrap().owner_boot = read.boot_id.clone();
    read.authority[0].host_call_contract_id = READ_OPERATION.into();
    read.authority[0].host_id = read.host_id.clone();
    read.authority[0].boot_id = read.boot_id.clone();
    read.authority[0].grant_id = "grant/host-b/read".into();
    let reader = SelectedTodoResidence::prepare(&root, &read, checkpoint_identity()).unwrap();
    let recovered = reader.recover(&read.authority[0]).unwrap();
    assert_eq!(TodoState::decode_info(&fore.0[0].bytes).unwrap(), recovered);
    assert_eq!(recovered.items[0].text, "Buy milk");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn refused_play_start_leaves_residence_and_fore_untouched() {
    std::thread::Builder::new()
        .name("todo-checkpoint-start-refusal".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(refused_start_proof)
        .unwrap()
        .join()
        .unwrap();
}

fn refused_start_proof() {
    let (mut host, plan, root) = planned_checkpoint();
    let (wake, body_plan) = body_plan(plan);
    let control = RunControl::default();
    let mut fore = CapturedFore::default();
    let result = host.run_body_plan_with_todo_checkpoint_to_with_start(
        BodyRunRequest {
            wake: &wake,
            plan: &body_plan,
            control: &control,
            keyboard: None,
        },
        BodyForeExchange {
            inputs: &inputs(),
            output: &mut fore,
        },
        TodoCheckpointSelection {
            root: &root,
            identity: checkpoint_identity(),
        },
        &mut Vec::new(),
        &mut ThreadTimer,
        |_, _| Err("start receipt refused".into()),
    );
    assert!(result.is_err());
    assert!(fore.0.is_empty());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn rebound_residence_is_refused_before_play() {
    let (mut host, plan, root) = planned_checkpoint();
    let displaced = root.with_extension("displaced");
    std::fs::rename(&root, &displaced).unwrap();
    std::fs::create_dir(&root).unwrap();
    let (wake, body_plan) = body_plan(plan);
    let control = RunControl::default();
    let mut fore = CapturedFore::default();
    let result = host.run_body_plan_with_todo_checkpoint_to(
        BodyRunRequest {
            wake: &wake,
            plan: &body_plan,
            control: &control,
            keyboard: None,
        },
        BodyForeExchange {
            inputs: &inputs(),
            output: &mut fore,
        },
        TodoCheckpointSelection {
            root: &root,
            identity: checkpoint_identity(),
        },
        &mut Vec::new(),
        &mut ThreadTimer,
    );
    assert!(result.unwrap_err().contains("rebound"));
    assert!(fore.0.is_empty());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(displaced).unwrap();
}
// The authored Plan, selected Host Call, and public Body entrance are the proof
// boundary. This is deliberately not a direct provider invocation.

#[test]
fn command_arrives_after_started_play_and_commits_once() {
    std::thread::Builder::new()
        .name("todo-waiting-checkpoint".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(|| {
            let (mut host, plan, root) = planned_checkpoint();
            let control = RunControl::default();
            let current = TodoState::new("Groceries".into())
                .unwrap()
                .encode_info()
                .unwrap();
            let queue = conduit_std_host::BodyLiveForeQueue::for_todo_checkpoint_plan(
                &plan,
                control.clone(),
                current,
                std::time::Duration::from_secs(2),
            )
            .unwrap();
            let (wake, body_plan) = body_plan(plan);
            let (started_tx, started_rx) = std::sync::mpsc::channel();
            let command = TodoCommand::Add {
                text: "Buy milk".into(),
            }
            .encode_info()
            .unwrap();
            assert!(queue.submit(&command).is_err());
            let producer = queue.clone();
            let sender = std::thread::spawn(move || {
                started_rx.recv().unwrap();
                producer.wait_until_play_started().unwrap();
                assert_eq!(
                    producer.submit(&command).unwrap(),
                    conduit_std_host::BodyLiveForeAdmission::Accepted { sequence: 0 }
                );
                assert!(producer.submit(&command).is_err());
            });
            let mut captured = CapturedFore::default();
            let report = host
                .run_body_plan_with_waiting_todo_checkpoint_to_with_start(
                    BodyRunRequest {
                        wake: &wake,
                        plan: &body_plan,
                        control: &control,
                        keyboard: None,
                    },
                    WaitingTodoFore {
                        queue: &queue,
                        output: &mut captured,
                    },
                    TodoCheckpointSelection {
                        root: &root,
                        identity: checkpoint_identity(),
                    },
                    &mut Vec::new(),
                    &mut ThreadTimer,
                    |play, _| {
                        started_tx.send(play.active_play_id.clone()).unwrap();
                        Ok(())
                    },
                )
                .unwrap();
            sender.join().unwrap();
            assert_eq!(report.terminal, TerminalDisposition::Completed);
            assert!(report.failure.is_none(), "{:?}", report.failure);
            assert_eq!(
                report.terminal_sign.active_play_id.as_ref(),
                Some(&report.play.active_play_id)
            );
            assert_eq!(report.terminal_sign.sequence, 2);
            assert_eq!(captured.0.len(), 1);
            assert_eq!(
                TodoState::decode_info(&captured.0[0].bytes).unwrap().items[0].text,
                "Buy milk"
            );
            let status = report.live_fore_status.unwrap();
            assert_eq!(
                (status.queue_accepted, status.kernel_admitted, status.queued),
                (1, 1, 0)
            );
            assert!(status.kernel_closed && status.play_terminal);
            assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
            std::fs::remove_dir_all(root).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn refused_start_cannot_publish_waiting_checkpoint() {
    std::thread::Builder::new()
        .name("todo-waiting-refusal".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(|| {
            let (mut host, plan, root) = planned_checkpoint();
            let control = RunControl::default();
            let current = TodoState::new("Groceries".into())
                .unwrap()
                .encode_info()
                .unwrap();
            let queue = conduit_std_host::BodyLiveForeQueue::for_todo_checkpoint_plan(
                &plan,
                control.clone(),
                current,
                std::time::Duration::from_secs(2),
            )
            .unwrap();
            let (wake, body_plan) = body_plan(plan);
            let mut captured = CapturedFore::default();
            assert!(host
                .run_body_plan_with_waiting_todo_checkpoint_to_with_start(
                    BodyRunRequest {
                        wake: &wake,
                        plan: &body_plan,
                        control: &control,
                        keyboard: None
                    },
                    WaitingTodoFore {
                        queue: &queue,
                        output: &mut captured,
                    },
                    TodoCheckpointSelection {
                        root: &root,
                        identity: checkpoint_identity()
                    },
                    &mut Vec::new(),
                    &mut ThreadTimer,
                    |_, _| Err("start refused".into()),
                )
                .is_err());
            assert!(captured.0.is_empty());
            assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
            assert!(queue.status().play_terminal);
            assert!(queue.wait_until_play_started().is_err());
            std::fs::remove_dir_all(root).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn cancelled_wait_has_no_command_effect_or_committed_delivery() {
    std::thread::Builder::new()
        .name("todo-waiting-cancel".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(|| {
            let (mut host, plan, root) = planned_checkpoint();
            let control = RunControl::default();
            let current = TodoState::new("Groceries".into())
                .unwrap()
                .encode_info()
                .unwrap();
            let queue = conduit_std_host::BodyLiveForeQueue::for_todo_checkpoint_plan(
                &plan,
                control.clone(),
                current,
                std::time::Duration::from_secs(2),
            )
            .unwrap();
            let (wake, body_plan) = body_plan(plan);
            let mut captured = CapturedFore::default();
            let report = host
                .run_body_plan_with_waiting_todo_checkpoint_to_with_start(
                    BodyRunRequest {
                        wake: &wake,
                        plan: &body_plan,
                        control: &control,
                        keyboard: None,
                    },
                    WaitingTodoFore {
                        queue: &queue,
                        output: &mut captured,
                    },
                    TodoCheckpointSelection {
                        root: &root,
                        identity: checkpoint_identity(),
                    },
                    &mut Vec::new(),
                    &mut ThreadTimer,
                    |_, _| {
                        control
                            .request_stop(
                                conduit_std_host::RunControlRequestId::new("cancel-wait").unwrap(),
                            )
                            .unwrap();
                        Ok(())
                    },
                )
                .unwrap();
            assert!(matches!(
                report.terminal,
                TerminalDisposition::Cancelled { .. }
            ));
            assert_eq!(
                report.terminal_sign.active_play_id.as_ref(),
                Some(&report.play.active_play_id)
            );
            assert_eq!(report.terminal_sign.sequence, 2);
            assert!(captured.0.is_empty());
            assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
            assert!(queue.wait_until_play_started().is_ok());
            assert!(queue
                .submit(
                    &TodoCommand::Add {
                        text: "Milk".into()
                    }
                    .encode_info()
                    .unwrap()
                )
                .is_err());
            std::fs::remove_dir_all(root).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn elapsed_wait_refuses_without_publishing() {
    std::thread::Builder::new()
        .name("todo-waiting-timeout".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(|| {
            let (mut host, plan, root) = planned_checkpoint();
            let control = RunControl::default();
            let current = TodoState::new("Groceries".into())
                .unwrap()
                .encode_info()
                .unwrap();
            let queue = conduit_std_host::BodyLiveForeQueue::for_todo_checkpoint_plan(
                &plan,
                control.clone(),
                current,
                std::time::Duration::from_millis(15),
            )
            .unwrap();
            let (wake, body_plan) = body_plan(plan);
            let mut captured = CapturedFore::default();
            let report = host
                .run_body_plan_with_waiting_todo_checkpoint_to_with_start(
                    BodyRunRequest {
                        wake: &wake,
                        plan: &body_plan,
                        control: &control,
                        keyboard: None,
                    },
                    WaitingTodoFore {
                        queue: &queue,
                        output: &mut captured,
                    },
                    TodoCheckpointSelection {
                        root: &root,
                        identity: checkpoint_identity(),
                    },
                    &mut Vec::new(),
                    &mut ThreadTimer,
                    |_, _| Ok(()),
                )
                .unwrap();
            assert_ne!(report.terminal, TerminalDisposition::Completed);
            assert!(report
                .failure
                .as_deref()
                .is_some_and(|failure| failure.contains("timed out")));
            assert_eq!(
                report.terminal_sign.active_play_id.as_ref(),
                Some(&report.play.active_play_id)
            );
            assert_eq!(report.terminal_sign.sequence, 2);
            assert!(captured.0.is_empty());
            assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
            assert!(queue.status().play_terminal);
            std::fs::remove_dir_all(root).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}
