use crate::{
    body_execution::{BodyForeOutputAdapter, BodyRunRequest},
    BodyLiveForeAdmission, BodyLiveForeQueue, ExternalForeDelivery, RunControl,
    RunControlRequestId, StdHost, StdHostConfig, TimerAdapter,
};
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core::{
    BootId, CancellationReason, HostId, OfferGeneration, SignId, TerminalDisposition,
};
use conduit_todo_plot::{TodoCommand, TodoState};
use std::{sync::mpsc, time::Duration};

struct NoTimer;

impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {}
}

struct Capture {
    values: Vec<ExternalForeDelivery>,
    first: Option<mpsc::Sender<()>>,
}

impl BodyForeOutputAdapter for Capture {
    fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
        self.values.push(value.clone());
        if let Some(sender) = self.first.take() {
            sender.send(()).map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

fn prepared() -> (
    StdHost,
    BodyPlan,
    conduit_body::Wake,
    BodyLiveForeQueue,
    RunControl,
) {
    let initial = TodoState::new("Groceries".into()).unwrap();
    let host = StdHost::new_for_todo_scan(
        StdHostConfig {
            host_id: HostId::from("std-todo-offer-proof"),
            boot_id: BootId::from("std-todo-offer-proof/boot"),
            offer_generation: OfferGeneration(1),
        },
        &initial,
        2,
    )
    .unwrap();
    let plan = crate::flow_activation::authored_todo_plan_on_host(host.advertisement(), 2);
    let control = RunControl::default();
    let queue = BodyLiveForeQueue::for_todo_plan(&plan, control.clone(), 1).unwrap();
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        SignId::from("sign/live-todo-born"),
    )
    .unwrap();
    let wake = body.wake(1, SignId::from("sign/live-todo-wake")).unwrap().1;
    let body_plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan,
        }],
    )
    .unwrap();
    (host, body_plan, wake, queue, control)
}

#[test]
fn later_typed_actions_share_one_live_body_play_until_explicit_close() {
    let (mut host, plan, wake, queue, control) = prepared();
    let first = TodoCommand::Add {
        text: "Milk".into(),
    }
    .encode_info()
    .unwrap();
    let second = TodoCommand::SetComplete {
        id: "task-1".into(),
        complete: true,
    }
    .encode_info()
    .unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (first_tx, first_rx) = mpsc::channel();
    let producer = queue.clone();
    let sender = std::thread::spawn(move || {
        started_rx.recv().unwrap();
        assert_eq!(
            producer.submit(&first).unwrap(),
            BodyLiveForeAdmission::Accepted { sequence: 0 }
        );
        first_rx.recv().unwrap();
        assert_eq!(
            producer.submit(&second).unwrap(),
            BodyLiveForeAdmission::Accepted { sequence: 1 }
        );
        producer.close().unwrap();
    });
    let mut capture = Capture {
        values: Vec::new(),
        first: Some(first_tx),
    };
    let report = host
        .run_body_plan_with_live_fore_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &queue,
            &mut capture,
            &mut Vec::new(),
            &mut NoTimer,
            |play, _| {
                started_tx.send(play.active_play_id.clone()).unwrap();
                Ok(())
            },
        )
        .unwrap();
    sender.join().unwrap();
    assert_eq!(report.terminal, TerminalDisposition::Completed);
    assert_eq!(capture.values.len(), 2);
    assert_eq!(report.play.play_sequence, 0);
    let status = report.live_fore_status.unwrap();
    assert_eq!(
        (status.queue_accepted, status.kernel_admitted, status.queued),
        (2, 2, 0)
    );
    assert!(status.close_requested && status.kernel_closed && status.play_terminal);
    let receipts = report.scan_child_signs.unwrap();
    assert_eq!(receipts.len(), 2);
    assert!(receipts
        .iter()
        .all(|receipt| receipt.parent_active_play_id == report.play.active_play_id));
}

#[test]
fn stop_wakes_idle_live_body_without_synthetic_close() {
    let (mut host, plan, wake, queue, control) = prepared();
    let (started_tx, started_rx) = mpsc::channel();
    let stopper = control.clone();
    let sender = std::thread::spawn(move || {
        started_rx.recv().unwrap();
        stopper
            .request_stop(RunControlRequestId::new("stop-idle-live-body").unwrap())
            .unwrap();
    });
    let mut capture = Capture {
        values: Vec::new(),
        first: None,
    };
    let report = host
        .run_body_plan_with_live_fore_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &queue,
            &mut capture,
            &mut Vec::new(),
            &mut NoTimer,
            |_, _| {
                started_tx.send(()).unwrap();
                Ok(())
            },
        )
        .unwrap();
    sender.join().unwrap();
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Cancelled {
            reason: CancellationReason::OperatorRequested
        }
    ));
    assert!(capture.values.is_empty());
    assert!(!report.scan_cancellation_failed);
}

#[test]
fn cancellation_reports_staged_command_not_admitted_to_kernel() {
    let (mut host, plan, wake, queue, control) = prepared();
    let command = TodoCommand::Add {
        text: "Milk".into(),
    }
    .encode_info()
    .unwrap();
    assert_eq!(
        queue.submit(&command).unwrap(),
        BodyLiveForeAdmission::Accepted { sequence: 0 }
    );
    let mut capture = Capture {
        values: Vec::new(),
        first: None,
    };
    let report = host
        .run_body_plan_with_live_fore_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &queue,
            &mut capture,
            &mut Vec::new(),
            &mut NoTimer,
            |_, _| {
                control
                    .request_stop(RunControlRequestId::new("stop-before-ingress").unwrap())
                    .unwrap();
                Ok(())
            },
        )
        .unwrap();
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Cancelled { .. }
    ));
    let status = report.live_fore_status.unwrap();
    assert_eq!(status.queue_accepted, 1);
    assert_eq!(status.kernel_admitted, 0);
    assert_eq!(status.queued, 1);
    assert!(status.play_terminal);
    assert!(queue.submit(&command).is_err());
    assert!(capture.values.is_empty());
}
