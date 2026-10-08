//! One first-action Todo encounter on the installed Owner's existing Body.
//! The waiting Play is retained before any Mask command may enter its Fore.
//! This module does not restore a later generation or roll over the Host.

use super::{debug, state, BoundedOutput, DeadlineTimer, Owner};
use conduit_body::{BodyPlayIdentity, Wake};
use conduit_core::{port_id, AuthorityGrant, ConnectionTrack, TerminalDisposition};
use conduit_presentation::{FaceInteraction, MaskShow};
use conduit_std_host::body_execution::{
    BodyForeOutputAdapter, BodyRunReport, BodyRunRequest, TodoCheckpointSelection,
};
use conduit_std_host::todo_durable_resource::{CheckpointIdentity, MissingV2Disposition};
use conduit_std_host::{
    BodyLiveForeAdmission, BodyLiveForeQueue, ExternalForeDelivery, RunControl,
    RunControlRequestId, StdHost,
};
use conduit_todo_plot::{TodoState, STATE_MAX_BYTES};
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const BODY_PLAY_STACK_BYTES: usize = 4 * 1024 * 1024;
type Outcome = (StdHost, Result<BodyRunReport, String>, u8);

struct OneCommittedFore(u8);

impl BodyForeOutputAdapter for OneCommittedFore {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        if self.0 != 0
            || output.front_port_id != port_id("committed")
            || output.track != ConnectionTrack::Payload
            || output.bytes.len() > STATE_MAX_BYTES
        {
            return Err("Todo waiting Play emitted an unexpected Fore".into());
        }
        self.0 = 1;
        Ok(())
    }
}

pub(crate) struct TodoWaitingWorker {
    control: RunControl,
    queue: BodyLiveForeQueue,
    initial: TodoState,
    started: Receiver<(BodyPlayIdentity, Wake)>,
    acknowledge: Option<SyncSender<Result<(), String>>>,
    thread: Option<JoinHandle<Outcome>>,
}

impl Owner {
    /// Begin a single explicitly new list. The current Form is selected before
    /// planning; the caller separately selected Host residence and authority.
    pub(crate) fn start_waiting_todo(
        &mut self,
        state_root: &Path,
        source: &crate::plot_source::CanonicalSource,
        plot: &conduit_plot::ExpandedAuthoringPlot,
        grant: &AuthorityGrant,
        checkpoint_root: PathBuf,
        identity: CheckpointIdentity,
        current: TodoState,
        maximum_millis: u64,
    ) -> Result<TodoWaitingWorker, String> {
        if !(1..=300_000).contains(&maximum_millis)
            || current.revision != 0
            || !current.items.is_empty()
            || self.last_execution.is_some()
            || self.todo_live.is_some()
            || identity.missing_v2 != MissingV2Disposition::StartNewList
            || identity.body != self.session.evidence().body_id.as_str()
            || identity.plot != plot.expanded.checked_plot_id.as_str()
        {
            return Err(
                "Todo waiting encounter requires one explicit first-new-list action".into(),
            );
        }
        current.validate().map_err(debug)?;
        self.plan_checkpoint_once(source, plot, grant)?;
        let proposed = self
            .session
            .realization()
            .ok_or("Todo Plan vanished")?
            .clone();
        let current_bytes = current.encode_info().map_err(debug)?;
        let control = RunControl::default();
        let queue = BodyLiveForeQueue::for_todo_checkpoint_plan(
            &proposed.plan.plots[0].plan,
            control.clone(),
            current_bytes,
            Duration::from_millis(maximum_millis),
        )?;
        self.persist(state_root)?;
        let mut host = self.host.take_for_play()?;
        let worker_control = control.clone();
        let worker_queue = queue.clone();
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (ack_tx, ack_rx) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("conduit-todo-waiting-play".into())
            .stack_size(BODY_PLAY_STACK_BYTES)
            .spawn(move || {
                let mut fore = OneCommittedFore(0);
                let mut output = BoundedOutput(Vec::with_capacity(32 * 1024));
                let mut timer = DeadlineTimer {
                    until: Instant::now() + Duration::from_millis(maximum_millis),
                    control: worker_control.clone(),
                };
                let result = host.run_body_plan_with_waiting_todo_checkpoint_to_with_start(
                    BodyRunRequest {
                        wake: &proposed.wake,
                        plan: &proposed.plan,
                        control: &worker_control,
                        keyboard: None,
                    },
                    &worker_queue,
                    &mut fore,
                    TodoCheckpointSelection {
                        root: &checkpoint_root,
                        identity,
                    },
                    &mut output,
                    &mut timer,
                    |play, wake| {
                        started_tx
                            .send((play.clone(), wake.clone()))
                            .map_err(|_| "Todo owner start observer disconnected".to_string())?;
                        ack_rx
                            .recv()
                            .map_err(|_| "Todo start acknowledgement lost".to_string())?
                    },
                );
                (host, result, fore.0)
            })
            .map_err(|error| format!("start Todo waiting worker: {error}"))?;
        Ok(TodoWaitingWorker {
            control,
            queue,
            initial: current,
            started: started_rx,
            acknowledge: Some(ack_tx),
            thread: Some(thread),
        })
    }
}

impl TodoWaitingWorker {
    /// Host staging is not a Todo acknowledgement. The caller must progress
    /// this worker to its committed output and terminal Sign first.
    pub(crate) fn submit_interaction(
        &self,
        owner: &Owner,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<BodyLiveForeAdmission, String> {
        let (play, state) = owner
            .todo_live
            .as_ref()
            .ok_or("Todo waiting Play has not been retained")?;
        if owner.current_play_id() != Some(play) || state != &self.initial {
            return Err("Todo action differs from current Play state".into());
        }
        let command = owner.resolve_todo_interaction(state, show, interaction)?;
        let canonical = command.encode_info().map_err(debug)?;
        self.queue.submit(&canonical)
    }

    /// Return `Some(committed state)` only after the exact terminal report has
    /// one acknowledged committed Fore and no failure. `None` is still active.
    pub(crate) fn progress(
        &mut self,
        owner: &mut Owner,
        state_root: &Path,
    ) -> Result<Option<TodoState>, String> {
        match self.started.try_recv() {
            Ok((play, wake)) => {
                let authority = owner.host.advertisement();
                let mut next = owner.session.clone();
                let result = next
                    .started(&authority.host_id, &authority.boot_id, play.clone(), wake)
                    .map_err(debug)
                    .and_then(|_| {
                        state::retain(
                            state_root,
                            next.evidence(),
                            owner.last_execution.as_ref(),
                            owner.admissions.as_ref(),
                        )
                    });
                if result.is_ok() {
                    owner.session = next;
                    owner.todo_live = Some((play.active_play_id, self.initial.clone()));
                }
                if let Some(ack) = self.acknowledge.take() {
                    let _ = ack.send(result.clone());
                }
                result?;
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
        }
        if !self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            return Ok(None);
        }
        let (host, result, count) = self
            .thread
            .take()
            .ok_or("Todo waiting worker lost its thread")?
            .join()
            .map_err(|_| "Todo waiting worker panicked; owner must restart".to_string())?;
        owner.host.restore_after_play(host)?;
        let report = result?;
        let proposed = owner
            .session
            .realization()
            .ok_or("Todo Play vanished")?
            .clone();
        if proposed.play.as_ref() != Some(&report.play) {
            return Err("Todo worker returned a different Play".into());
        }
        let committed = if report.terminal == TerminalDisposition::Completed
            && report.failure.is_none()
            && report.cleanup_failure.is_none()
            && report
                .live_fore_status
                .is_some_and(|status| status.kernel_admitted == 1)
            && count == 1
            && report.fore_deliveries.len() == 1
            && report.terminal_sign.active_play_id == Some(report.play.active_play_id.clone())
        {
            Some(TodoState::decode_info(&report.fore_deliveries[0].bytes).map_err(debug)?)
        } else {
            None
        };
        let authority = owner.host.advertisement();
        let mut next = owner.session.clone();
        next.lull(&authority.host_id, &authority.boot_id, Some(&report.play))
            .map_err(debug)?;
        let receipt = serde_json::json!({
            "schema":"conduit.todo/first-checkpoint-receipt@1",
            "body_id":proposed.wake.body_id, "plan_id":proposed.plan.plan_id,
            "play":report.play, "terminal":report.terminal,
            "failure":report.failure, "cleanup_failure":report.cleanup_failure,
            "terminal_sign":report.terminal_sign,
            "committed_fore_count":count,
            "committed_fore_sha256":report.fore_deliveries.first().map(|fore| super::super::super::digest(&fore.bytes)),
        });
        state::retain(
            state_root,
            next.evidence(),
            Some(&receipt),
            owner.admissions.as_ref(),
        )?;
        owner.session = next;
        owner.last_execution = Some(receipt);
        owner.todo_live = None;
        committed
            .ok_or("Todo waiting Play did not commit one checkpoint".into())
            .map(Some)
    }

    pub(crate) fn request_lull(&self) -> Result<(), String> {
        self.control
            .request_stop(RunControlRequestId::new("owner/todo-lull")?)
            .map_err(|_| "Todo Play already has a stop request".into())
    }
}
