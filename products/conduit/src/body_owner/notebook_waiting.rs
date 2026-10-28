//! One finite edit encounter on the continuing owner's existing Host.
use super::*;
use conduit_body::{BodyPlayIdentity, Wake};
use conduit_core::CancellationReason;
use conduit_face::{FaceInteraction, MaskShow};
use conduit_std_host::body_execution::BodyRunReport;
use conduit_std_host::{BodyLiveForeAdmission, BodyLiveForeQueue, RunControlRequestId, StdHost};
use std::{
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError},
    thread::{self, JoinHandle},
};

type Outcome = (StdHost, Result<BodyRunReport, String>, CurrentOutput);

pub(crate) struct NotebookWaitingWorker {
    control: RunControl,
    queue: BodyLiveForeQueue,
    initial: Vec<u8>,
    accepted: bool,
    accepted_mask: Option<(MaskShow, FaceInteraction)>,
    started: Receiver<(BodyPlayIdentity, Wake)>,
    acknowledge: Option<SyncSender<Result<(), String>>>,
    thread: Option<JoinHandle<Outcome>>,
}

impl Owner {
    pub(crate) fn has_open_notebook(&self) -> bool {
        self.notebook.is_some()
    }

    pub(crate) fn start_notebook_waiting(
        &mut self,
        root: &Path,
        maximum_millis: u64,
    ) -> Result<NotebookWaitingWorker, String> {
        self.ensure_execution_retained(root)?;
        if !(1..=300_000).contains(&maximum_millis) {
            return Err("notebook encounter duration must be 1..300000 milliseconds".into());
        }
        let current = self
            .notebook
            .as_ref()
            .ok_or("notebook is not open")?
            .current
            .clone();
        self.plan_notebook_edit()?;
        let proposed = self
            .session
            .realization()
            .ok_or("notebook Plan required")?
            .clone();
        let [partition] = proposed.plan.plots.as_slice() else {
            return Err("notebook requires one resident Plot".into());
        };
        let control = RunControl::default();
        let queue = BodyLiveForeQueue::for_document_step_plan(
            &partition.plan,
            control.clone(),
            current.clone(),
            Duration::from_millis(maximum_millis),
        )?;
        self.persist(root)?;
        state::require_execution_capacity(root)?;
        let mut host = self.host.take_for_play()?;
        let worker_control = control.clone();
        let worker_queue = queue.clone();
        let (started_tx, started) = mpsc::sync_channel(1);
        let (acknowledge, ack_rx) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("conduit-notebook-waiting".into())
            .stack_size(4 * 1024 * 1024)
            .spawn(move || {
                let mut candidate =
                    CurrentOutput(Vec::with_capacity(MAXIMUM_TEXT_DOCUMENT_BYTES as usize));
                let mut output = BoundedOutput(Vec::with_capacity(32 * 1024));
                let mut timer = DeadlineTimer {
                    until: Instant::now() + Duration::from_millis(maximum_millis),
                    control: worker_control.clone(),
                };
                let report = host.run_body_plan_with_live_fore_to_with_start(
                    BodyRunRequest {
                        wake: &proposed.wake,
                        plan: &proposed.plan,
                        control: &worker_control,
                        keyboard: None,
                    },
                    &worker_queue,
                    &mut candidate,
                    &mut output,
                    &mut timer,
                    |play, wake| {
                        started_tx
                            .send((play.clone(), wake.clone()))
                            .map_err(|_| "notebook start observer disconnected".to_string())?;
                        ack_rx
                            .recv()
                            .map_err(|_| "notebook start acknowledgement lost".to_string())?
                    },
                );
                (host, report, candidate)
            })
            .map_err(|error| format!("start notebook waiting worker: {error}"))?;
        Ok(NotebookWaitingWorker {
            control,
            queue,
            initial: current,
            accepted: false,
            accepted_mask: None,
            started,
            acknowledge: Some(acknowledge),
            thread: Some(thread),
        })
    }
}

impl NotebookWaitingWorker {
    pub(crate) fn submit_interaction(
        &mut self,
        owner: &Owner,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<BodyLiveForeAdmission, String> {
        let NotebookAction::Replace(text) =
            owner.resolve_notebook_interaction(show, interaction)?
        else {
            return Err("Save is a publication request, not an edit queue item".into());
        };
        let (_, current) = owner.notebook_document().ok_or("notebook is not open")?;
        let end = std::str::from_utf8(current.text)
            .map_err(|error| error.to_string())?
            .chars()
            .count() as u32;
        let admission = self.submit_edit(
            owner,
            TextEdit {
                base_revision: current.revision,
                start: 0,
                end,
                insert_text: &text,
            },
        )?;
        if matches!(admission, BodyLiveForeAdmission::Accepted { .. }) {
            self.accepted_mask = Some((show.clone(), interaction.clone()));
        }
        Ok(admission)
    }

    pub(crate) fn authorize_save(
        &mut self,
        owner: &Owner,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<(), String> {
        if self.accepted
            || self.accepted_mask.is_some()
            || !matches!(
                owner.resolve_notebook_interaction(show, interaction)?,
                NotebookAction::Save
            )
        {
            return Err("notebook Save differs from its current idle edit encounter".into());
        }
        self.request_lull()?;
        self.accepted_mask = Some((show.clone(), interaction.clone()));
        Ok(())
    }

    /// The service must first resolve the caller's exact Show and interaction.
    /// Queue staging is not an acknowledgement of a document change.
    pub(super) fn submit_edit(
        &mut self,
        owner: &Owner,
        edit: TextEdit<'_>,
    ) -> Result<BodyLiveForeAdmission, String> {
        if self.accepted
            || owner.current_play_id().is_none()
            || owner
                .notebook
                .as_ref()
                .is_none_or(|document| document.current != self.initial)
        {
            return Err("notebook proposal differs from the current waiting document".into());
        }
        let mut bytes = [0; MAXIMUM_TEXT_EDIT_BYTES as usize];
        let length = edit.encode_into(&mut bytes).map_err(debug)?;
        self.queue.wait_until_play_started()?;
        let admission = self.queue.submit(&bytes[..length])?;
        if matches!(admission, BodyLiveForeAdmission::Accepted { .. }) {
            self.accepted = true;
        }
        Ok(admission)
    }

    /// None is pending; Some reports the terminal encounter's actual adoption.
    pub(crate) fn progress(
        &mut self,
        owner: &mut Owner,
        root: &Path,
    ) -> Result<Option<bool>, String> {
        match self.started.try_recv() {
            Ok((play, wake)) => {
                let ad = owner.host.advertisement();
                let mut next = owner.session.clone();
                let retained = next
                    .started(&ad.host_id, &ad.boot_id, play, wake)
                    .map_err(debug)
                    .and_then(|_| {
                        state::retain_session(
                            root,
                            &mut next,
                            owner.last_execution.as_ref(),
                            owner.admissions.as_ref(),
                            None,
                            None,
                        )
                    });
                if retained.is_ok() {
                    owner.session = next;
                }
                if let Some(ack) = self.acknowledge.take() {
                    let _ = ack.send(retained.clone());
                }
                retained?;
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
        }
        if !self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            return Ok(None);
        }
        let (host, report, candidate) = self
            .thread
            .take()
            .ok_or("notebook worker lost its thread")?
            .join()
            .map_err(|_| "notebook worker panicked; owner must restart".to_string())?;
        owner.host.restore_after_play(host)?;
        let report = report?;
        let proposed = owner
            .session
            .realization()
            .ok_or("notebook Play vanished")?
            .clone();
        if proposed.play.as_ref() != Some(&report.play) {
            return Err("notebook worker returned a different Play".into());
        }
        let exact_terminal =
            report.terminal_sign.active_play_id == Some(report.play.active_play_id.clone());
        let adopted = self.accepted
            && report.terminal == TerminalDisposition::Completed
            && report.failure.is_none()
            && report.cleanup_failure.is_none()
            && exact_terminal
            && report
                .live_fore_status
                .is_some_and(|status| status.kernel_admitted == 1)
            && report.fore_deliveries.len() == 1
            && !candidate.0.is_empty()
            && report.fore_deliveries[0].bytes.as_slice() == candidate.0.as_slice();
        let idle_cancelled = !self.accepted
            && report.terminal
                == (TerminalDisposition::Cancelled {
                    reason: CancellationReason::OperatorRequested,
                })
            && report.failure.is_none()
            && report.cleanup_failure.is_none()
            && exact_terminal
            && candidate.0.is_empty()
            && report.fore_deliveries.is_empty()
            && report.fore_pending_output.is_none()
            && report
                .live_fore_status
                .is_some_and(|status| status.queue_accepted == 0 && status.kernel_admitted == 0);
        let ad = owner.host.advertisement();
        let mut next = owner.session.clone();
        next.lull(&ad.host_id, &ad.boot_id, Some(&report.play))
            .map_err(debug)?;
        let receipt = serde_json::json!({
            "schema":"conduit.notebook/waiting-edit-receipt@1", "body_id":proposed.wake.body_id,
            "host_id":ad.host_id, "boot_id":ad.boot_id, "plan_id":proposed.plan.plan_id,
            "wake_id":proposed.wake.wake_id, "play":report.play, "terminal":report.terminal,
            "failure":report.failure, "cleanup_failure":report.cleanup_failure,
            "terminal_sign":report.terminal_sign, "document_adopted":adopted,
            "initiating_show":self.accepted_mask.as_ref().map(|(show, _)| show),
            "initiating_action":self.accepted_mask.as_ref().map(|(_, action)| action),
        });
        if adopted {
            owner
                .notebook
                .as_mut()
                .ok_or("notebook disappeared")?
                .current = candidate.0;
        }
        owner.retain_execution(root, next, receipt)?;
        if adopted || idle_cancelled {
            Ok(Some(adopted))
        } else {
            Err("notebook waiting edit was refused; current document preserved".into())
        }
    }

    pub(crate) fn request_lull(&self) -> Result<(), String> {
        self.control
            .request_stop(RunControlRequestId::new("owner/notebook-lull")?)
            .map_err(|_| "notebook Play already has a stop request".into())
    }
}
