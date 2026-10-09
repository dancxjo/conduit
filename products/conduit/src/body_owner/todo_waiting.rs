//! One selected Todo command encounter on the installed Owner's existing Body.
//! The waiting Play is retained before any Mask command may enter its Fore.
//! A later encounter must supply its independently verified read and fresh
//! selected write generation before this waiting Play is admitted.

use super::{debug, state, BoundedOutput, DeadlineTimer, Owner};
use conduit_body::{BodyPlayIdentity, Wake};
use conduit_core::{
    port_id, AuthorityGrant, ConnectionTrack, ResourceAccessMode, ResourceContentRequirement,
    TerminalDisposition,
};
use conduit_kernel::KernelEventKind;
use conduit_presentation::{FaceInteraction, FaceInteractionId, MaskShow};
use conduit_std_host::body_execution::{
    BodyForeOutputAdapter, BodyRunReport, BodyRunRequest, TodoCheckpointSelection, WaitingTodoFore,
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

#[path = "todo_waiting/admission.rs"]
mod admission;

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
    checkpoint_identity: CheckpointIdentity,
    selected_content: ResourceContentRequirement,
    accepted_interaction: Option<FaceInteractionId>,
    accepted_mask: Option<(MaskShow, FaceInteraction)>,
    started: Receiver<(BodyPlayIdentity, Wake)>,
    acknowledge: Option<SyncSender<Result<(), String>>>,
    thread: Option<JoinHandle<Outcome>>,
}

pub(crate) struct NewTodoCheckpoint {
    pub(crate) root: PathBuf,
    pub(crate) identity: CheckpointIdentity,
    pub(crate) current: TodoState,
}

impl Owner {
    pub(crate) fn start_selected_new_todo_list(
        &mut self,
        state_root: &Path,
        checkpoint_root: PathBuf,
        selected_content: &ResourceContentRequirement,
        list_key: String,
        maximum_millis: u64,
    ) -> Result<TodoWaitingWorker, String> {
        if list_key.is_empty() || list_key.len() > 128 || list_key.chars().any(char::is_control) {
            return Err("new Todo list key is invalid".into());
        }
        let source_path = state_root.join("body/source.conduit");
        let bytes = super::super::super::bounded_read(&source_path, 256 * 1024)?;
        let text =
            std::str::from_utf8(&bytes).map_err(|error| format!("Todo Source UTF-8: {error}"))?;
        let source = crate::plot_source::parse(text)?;
        let plot = source.expand_entry_for_authoring()?;
        if plot.expanded.name != "todo/checkpoint-once" {
            return Err("installed new Todo list requires checkpoint-once Source".into());
        }
        let advertisement = self.host.advertisement();
        let offer = advertisement
            .capabilities
            .iter()
            .find(|offer| {
                offer.implementation.implementation_id.as_str()
                    == conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION
            })
            .ok_or("installed Host has no selected Todo checkpoint offer")?;
        let resource = advertisement
            .resources
            .iter()
            .find(|resource| {
                resource
                    .content
                    .as_ref()
                    .is_some_and(|content| &content.contract == selected_content)
            })
            .ok_or("selected Todo checkpoint content differs from Host offer")?;
        if resource.class_id.as_str() != "resource/todo-checkpoint@1"
            || offer.authority_requirements.len() != 1
        {
            return Err("installed Todo checkpoint offer is not exact".into());
        }
        let requirement = &offer.authority_requirements[0];
        let body = self.session.evidence().body_id.as_str().to_owned();
        let grant = AuthorityGrant {
            grant_id: format!("grant/todo/{body}/first").into(),
            contract_id: requirement.contract_id.clone(),
            host_call_contract_id: requirement.host_call_contract_id.clone(),
            subject_kind: requirement.subject_kind.clone(),
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            capability_id: offer.capability_id.clone(),
        };
        self.start_waiting_todo(
            state_root,
            &source,
            &plot,
            &grant,
            NewTodoCheckpoint {
                root: checkpoint_root,
                identity: CheckpointIdentity {
                    body,
                    plot: plot.expanded.checked_plot_id.as_str().to_owned(),
                    workload: list_key.clone(),
                    missing_v2: MissingV2Disposition::StartNewList,
                },
                current: TodoState::new(list_key).map_err(debug)?,
            },
            maximum_millis,
        )
    }

    pub(crate) fn todo_commit_receipt(&self) -> Option<&serde_json::Value> {
        self.last_execution.as_ref().filter(|receipt| {
            receipt["schema"] == "conduit.todo/first-checkpoint-receipt@1"
                || receipt["schema"] == "conduit.todo/next-checkpoint-receipt@1"
        })
    }

    /// Begin one new-list or verified continuation command. The current Form
    /// is selected before planning; the Host residence and grant are explicit.
    pub(crate) fn start_waiting_todo(
        &mut self,
        state_root: &Path,
        source: &crate::plot_source::CanonicalSource,
        plot: &conduit_plot::ExpandedAuthoringPlot,
        grant: &AuthorityGrant,
        checkpoint: NewTodoCheckpoint,
        maximum_millis: u64,
    ) -> Result<TodoWaitingWorker, String> {
        let NewTodoCheckpoint {
            root: checkpoint_root,
            identity,
            current,
        } = checkpoint;
        let first = current.revision == 0 && current.items.is_empty();
        let continuing = self
            .todo_verified
            .as_ref()
            .is_some_and(|(_, verified)| verified == &current)
            && self.todo_verified_read_receipt().is_some();
        if !(1..=300_000).contains(&maximum_millis)
            || !(first && self.last_execution.is_none() || continuing)
            || self.todo_live.is_some()
            || identity.missing_v2
                != if first {
                    MissingV2Disposition::StartNewList
                } else {
                    MissingV2Disposition::Refuse
                }
            || identity.body != self.session.evidence().body_id.as_str()
            || identity.plot != plot.expanded.checked_plot_id.as_str()
        {
            return Err(
                "Todo waiting encounter requires an exact new or verified generation".into(),
            );
        }
        current.validate().map_err(debug)?;
        let advertised = self.host.advertisement();
        let selected_offer = advertised
            .capabilities
            .iter()
            .find(|offer| {
                offer.implementation.implementation_id.as_str()
                    == conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION
                    && offer.capability_id == grant.capability_id
            })
            .ok_or("Todo grant does not select the current checkpoint offer")?;
        if selected_offer.authority_requirements.len() != 1 {
            return Err("Todo checkpoint offer has unexpected authority".into());
        }
        let mut selected = advertised.resources.iter().filter(|resource| {
            resource.class_id.as_str() == "resource/todo-checkpoint@1"
                && resource.content.as_ref().is_some_and(|content| {
                    content.contract.access == ResourceAccessMode::WriteCandidatePublish
                })
        });
        let selected_content = selected
            .next()
            .and_then(|resource| resource.content.as_ref())
            .ok_or("Todo checkpoint has no selected write resource")?
            .contract
            .clone();
        if selected.next().is_some() {
            return Err("Todo checkpoint has ambiguous selected resources".into());
        }
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
        let checkpoint_identity = identity.clone();
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
                    WaitingTodoFore {
                        queue: &worker_queue,
                        output: &mut fore,
                    },
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
            checkpoint_identity,
            selected_content,
            accepted_interaction: None,
            accepted_mask: None,
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
        &mut self,
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
        let admission = self.queue.submit(&canonical)?;
        if matches!(admission, BodyLiveForeAdmission::Accepted { .. }) {
            self.accepted_interaction = Some(interaction.identity.clone());
            self.accepted_mask = Some((show.clone(), interaction.clone()));
        }
        Ok(admission)
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
                        state::retain_session(
                            state_root,
                            &mut next,
                            owner.last_execution.as_ref(),
                            owner.admissions.as_ref(),
                            None,
                            None,
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
            && report.requests.len() == 1
            && report.requests.first().is_some_and(|request| {
                [
                    KernelEventKind::HostCallRequested,
                    KernelEventKind::HostCallCompleted,
                ]
                .into_iter()
                .all(|kind| {
                    report
                        .kernel_events
                        .iter()
                        .filter(|event| {
                            event.kind == kind
                                && event.node == request.node
                                && event.request == Some(request.request)
                        })
                        .count()
                        == 1
                })
            })
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
        let previous_read = owner
            .last_execution
            .as_ref()
            .filter(|receipt| receipt["schema"] == "conduit.todo/verified-read-receipt@1");
        let receipt = serde_json::json!({
            "schema":if self.initial.revision == 0 {
                "conduit.todo/first-checkpoint-receipt@1"
            } else {
                "conduit.todo/next-checkpoint-receipt@1"
            },
            "body_id":proposed.wake.body_id, "plan_id":proposed.plan.plan_id,
            "play":report.play, "terminal":report.terminal,
            "failure":report.failure, "cleanup_failure":report.cleanup_failure,
            "terminal_sign":report.terminal_sign,
            "interaction_id":self.accepted_interaction.as_ref(),
            "initiating_show":self.accepted_mask.as_ref().map(|(show, _)| show),
            "initiating_action":self.accepted_mask.as_ref().map(|(_, action)| action),
            "committed_fore_count":count,
            "committed_fore_sha256":report.fore_deliveries.first().map(|fore| super::super::super::digest(&fore.bytes)),
            "checkpoint_namespace":{
                "body_id":self.checkpoint_identity.body,
                "write_plot_id":self.checkpoint_identity.plot,
                "list_key":self.checkpoint_identity.workload,
            },
            "selected_content":self.selected_content,
            // One prior verified witness survives a failed, unpublished command.
            // Successful writes retain only the compact summary below, so this
            // does not accumulate a receipt history across list revisions.
            "retained_verified_read":previous_read.filter(|_| committed.is_none()),
            "previous_read":previous_read.map(|receipt| serde_json::json!({
                "body_id":receipt["body_id"],
                "read_plan_id":receipt["read_plan_id"],
                "read_play_id":receipt["read_play"]["active_play_id"],
                "read_terminal_sign_id":receipt["read_terminal_sign"]["sign_id"],
                "selected_content":receipt["selected_content"],
                "restored_fore_sha256":receipt["restored_fore_sha256"],
            })),
            "host_call_request":report.requests.first().map(|request| serde_json::json!({
                "node":request.node.0,
                "request":request.request.0,
                "call":request.call.0,
            })),
        });
        state::retain_session(
            state_root,
            &mut next,
            Some(&receipt),
            owner.admissions.as_ref(),
            None,
            None,
        )?;
        owner.session = next;
        owner.last_execution = Some(receipt);
        owner.todo_live = None;
        if committed.is_none() {
            owner.todo_verified = None;
        }
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
