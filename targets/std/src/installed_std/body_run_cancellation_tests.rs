use super::{workload, Body, BodyFormPlan, BodyPlan, ResidentForm, SignId};
use crate::{
    body_execution::BodyRunRequest,
    hosted_keyboard::{HostedKeyboardAdapter, HostedKeyboardPoll},
    RunControl, RunControlRequestId, StdHost, TimerAdapter,
};
use conduit_core::TerminalDisposition;

struct Keys {
    stop: Option<RunControl>,
    events: std::collections::VecDeque<[u8; 3]>,
    polls: usize,
}
impl HostedKeyboardAdapter for Keys {
    fn poll_next(&mut self) -> HostedKeyboardPoll {
        self.polls += 1;
        if let Some(control) = self.stop.take() {
            control
                .request_stop(RunControlRequestId::new("stop-pending-keyboard").unwrap())
                .unwrap();
            return HostedKeyboardPoll::Pending;
        }
        self.events
            .pop_front()
            .map_or(HostedKeyboardPoll::Cancelled, |bytes| {
                HostedKeyboardPoll::Event(conduit_human::KeyEvent::decode(&bytes).unwrap())
            })
    }
}
struct Clock;
impl TimerAdapter for Clock {
    fn wait(&mut self, _: std::time::Duration) {}
}

#[test]
fn pending_keyboard_cancellation_releases_the_whole_workload_for_another_play() {
    let (advertisement, plans) = workload();
    let mut host = StdHost::from_advertisement(advertisement).unwrap();
    let first = &plans[0];
    let mut body = Body::born(
        first.source_document_id.clone(),
        first.checked_form_id.clone(),
        1,
        SignId::from("sign/cancel-body"),
    )
    .unwrap();
    for (index, part) in plans.iter().enumerate().skip(1) {
        body = body
            .admit_form(
                ResidentForm::new(
                    part.source_document_id.clone(),
                    part.checked_form_id.clone(),
                ),
                SignId::from(format!("sign/cancel-admit-{index}")),
            )
            .unwrap();
    }
    let wake = body.wake(1, SignId::from("sign/cancel-wake")).unwrap().1;
    let plan = BodyPlan::seal(
        &wake,
        plans
            .into_iter()
            .map(|plan| BodyFormPlan {
                form: ResidentForm::new(
                    plan.source_document_id.clone(),
                    plan.checked_form_id.clone(),
                ),
                plan,
            })
            .collect(),
    )
    .unwrap();
    let stop = RunControl::default();
    let mut keys = Keys {
        stop: Some(stop.clone()),
        events: [[0x2c, 0, 0], [0x2c, 1, 0]].into(),
        polls: 0,
    };
    let mut output = Vec::with_capacity(2048);
    let cancelled = host
        .run_body_plan_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &stop,
                keyboard: Some(&mut keys),
            },
            &mut output,
            &mut Clock,
        )
        .unwrap();
    assert!(
        matches!(cancelled.terminal, TerminalDisposition::Cancelled { .. }),
        "{:?}",
        cancelled.failure
    );
    assert!(cancelled.failure.is_none());
    assert_eq!(keys.polls, 1);
    assert_eq!(keys.events.len(), 2);
    assert!(!String::from_utf8(output).unwrap().contains("bool value="));
    let completed = host
        .run_body_plan_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: Some(&mut keys),
            },
            &mut Vec::new(),
            &mut Clock,
        )
        .unwrap();
    assert_eq!(
        completed.terminal,
        TerminalDisposition::Completed,
        "{:?}",
        completed.failure
    );
    assert_ne!(cancelled.play.active_play_id, completed.play.active_play_id);
    // Inject only the finalization error, not a claim that the real ledger
    // failed. Execution identity and all evidence must survive cleanup failure.
    let play = completed.play.clone();
    let terminal_sign = completed.terminal_sign.clone();
    let evidence = format!(
        "{:?}",
        (
            &completed.partitions,
            &completed.requests,
            &completed.kernel_events
        )
    );
    let mut completed = completed;
    completed.cleanup_failure = Some("earlier cleanup failure".to_string());
    let retained = crate::body_execution::finish_body_release(
        Ok(completed),
        vec!["first release".to_string(), "second release".to_string()],
    )
    .unwrap();
    assert_eq!(retained.play, play);
    assert_eq!(retained.terminal_sign, terminal_sign);
    assert_eq!(retained.terminal, TerminalDisposition::Completed);
    assert!(retained.failure.is_none());
    assert_eq!(
        format!(
            "{:?}",
            (
                &retained.partitions,
                &retained.requests,
                &retained.kernel_events
            )
        ),
        evidence
    );
    assert_eq!(
        retained.cleanup_failure.as_deref(),
        Some("earlier cleanup failure; Body reservation release: first release; second release")
    );
}

#[test]
fn release_failure_retains_the_original_pre_play_refusal() {
    let error = crate::body_execution::finish_body_release(
        Err("Body Sign sequence exhausted".to_string()),
        vec!["reservation unavailable".to_string()],
    )
    .unwrap_err();
    assert_eq!(
        error,
        "Body Sign sequence exhausted; Body reservation release: reservation unavailable"
    );
}
