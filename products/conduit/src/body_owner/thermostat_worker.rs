//! The installed Owner's sole Host plays one finite, live Thermostat scan.
use super::{debug, state, thermostat_face::ThermostatLive, BoundedOutput, DeadlineTimer, Owner};
use conduit_body::{BodyPlayIdentity, Wake};
use conduit_presentation::{FaceInteraction, MaskShow};
use conduit_std_host::{
    body_execution::{BodyForeOutputAdapter, BodyRunReport, BodyRunRequest},
    BodyLiveForeAdmission, BodyLiveForeQueue, ExternalForeDelivery, RunControl,
    RunControlRequestId, StdHost,
};
use conduit_thermostat_plot::ThermostatState;
use std::{
    path::Path,
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

type Outcome = (StdHost, Result<BodyRunReport, String>);
struct Capture(SyncSender<ThermostatState>);
impl BodyForeOutputAdapter for Capture {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        if output.front_port_id != conduit_core::port_id("states")
            || output.track != conduit_core::ConnectionTrack::Payload
        {
            return Err("Thermostat scan emitted an unexpected Fore".into());
        }
        let state = ThermostatState::decode_info(&output.bytes).map_err(debug)?;
        self.0
            .try_send(state)
            .map_err(|_| "Thermostat output encounter is under pressure".into())
    }
}

pub(crate) struct ThermostatWorker {
    control: RunControl,
    queue: BodyLiveForeQueue,
    initial: ThermostatState,
    started: Receiver<(BodyPlayIdentity, Wake)>,
    acknowledge: Option<SyncSender<Result<(), String>>>,
    outputs: Receiver<ThermostatState>,
    pending_output: bool,
    observed_outputs: u16,
    maximum_items: u16,
    thread: Option<JoinHandle<Outcome>>,
}

impl Owner {
    pub(crate) fn thermostat_terminal_receipt(&self) -> Option<&serde_json::Value> {
        self.last_execution
            .as_ref()
            .filter(|receipt| receipt["schema"] == "conduit.thermostat/owner-scan-receipt@1")
    }

    pub(crate) fn is_thermostat(&self) -> bool {
        self.resident_name.as_deref() == Some("thermostat/main")
    }

    pub(crate) fn start_thermostat_run(
        &mut self,
        root: &Path,
        maximum_millis: u64,
    ) -> Result<ThermostatWorker, String> {
        if !(1..=super::continuing::MAXIMUM_SERVICE_RUN_MILLIS).contains(&maximum_millis)
            || self.host.is_playing()
            || self.session.realization().is_some()
            || !self.is_thermostat()
        {
            return Err(
                "Thermostat scan requires an idle exact resident and bounded duration".into(),
            );
        }
        let bytes =
            super::super::super::bounded_read(&root.join("body/source.conduit"), 256 * 1024)?;
        let source = crate::plot_source::parse(std::str::from_utf8(&bytes).map_err(debug)?)?;
        let checked = source.expand_entry_for_authoring()?;
        let (initial, maximum_items) = super::super::scoped_thermostat_initial(&checked)?
            .ok_or("Thermostat scan is missing")?;
        self.set_resident_plot_name(&checked)?;
        self.plan_with_source(&source, &checked)?;
        self.persist(root)?;
        let proposed = self
            .session
            .realization()
            .ok_or("Thermostat Plan vanished")?
            .clone();
        let control = RunControl::default();
        let queue = BodyLiveForeQueue::for_thermostat_plan(
            &proposed.plan.plots[0].plan,
            control.clone(),
            1,
        )?;
        let worker_queue = queue.clone();
        let worker_control = control.clone();
        let mut host = self.host.take_for_play()?;
        let (started_tx, started) = mpsc::sync_channel(1);
        let (acknowledge, ack_rx) = mpsc::sync_channel(1);
        let (output_tx, outputs) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("conduit-thermostat-scan".into())
            .stack_size(4 * 1024 * 1024)
            .spawn(move || {
                let mut output = BoundedOutput(Vec::with_capacity(32 * 1024));
                let mut timer = DeadlineTimer {
                    until: Instant::now() + Duration::from_millis(maximum_millis),
                    control: worker_control.clone(),
                };
                let result = thread::scope(|scope| {
                    let (finished, receiver) = mpsc::sync_channel::<()>(1);
                    let deadline = worker_control.clone();
                    scope.spawn(move || {
                        if receiver
                            .recv_timeout(Duration::from_millis(maximum_millis))
                            .is_err()
                        {
                            let _ = deadline.request_stop(
                                RunControlRequestId::new("owner/thermostat-deadline")
                                    .expect("bounded request"),
                            );
                        }
                    });
                    let result = host.run_body_plan_with_live_fore_to_with_start(
                        BodyRunRequest {
                            wake: &proposed.wake,
                            plan: &proposed.plan,
                            control: &worker_control,
                            keyboard: None,
                        },
                        &worker_queue,
                        &mut Capture(output_tx),
                        &mut output,
                        &mut timer,
                        |play, wake| {
                            started_tx
                                .send((play.clone(), wake.clone()))
                                .map_err(debug)?;
                            ack_rx.recv().map_err(debug)?
                        },
                    );
                    let _ = finished.send(());
                    result
                });
                (host, result)
            })
            .expect("failure to launch the only Body Play worker stops the owner service");
        Ok(ThermostatWorker {
            control,
            queue,
            initial,
            started,
            acknowledge: Some(acknowledge),
            outputs,
            pending_output: false,
            observed_outputs: 0,
            maximum_items,
            thread: Some(thread),
        })
    }
}

impl ThermostatWorker {
    pub(crate) fn output_pending(&self) -> bool {
        self.pending_output
    }

    /// Resolve the acknowledged current Show and queue a canonical command.
    /// Admission is distinct from the state output observed by progress.
    pub(crate) fn submit_interaction(
        &mut self,
        owner: &Owner,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<BodyLiveForeAdmission, String> {
        if self.pending_output {
            return Err("Thermostat command is awaiting its state Fore".into());
        }
        let command = owner.resolve_thermostat_interaction(show, interaction)?;
        let admission = self.queue.submit(&command.encode())?;
        if matches!(admission, BodyLiveForeAdmission::Accepted { .. }) {
            self.pending_output = true;
            if self.queue.status().queue_accepted == self.maximum_items {
                // The selected producer admits no further values. Close its
                // Fore after this queued value drains so the scan completes.
                self.queue.close()?;
            }
        }
        Ok(admission)
    }

    fn observe_output(&mut self, owner: &mut Owner) -> Result<(), String> {
        match self.outputs.try_recv() {
            Ok(state) => {
                if !self.pending_output {
                    return Err("Thermostat emitted an unsolicited state Fore".into());
                }
                let live = owner
                    .thermostat_live
                    .as_mut()
                    .ok_or("Thermostat output lacks a retained Play")?;
                live.state = state;
                live.revision = live
                    .revision
                    .checked_add(1)
                    .ok_or("Thermostat Face revision exhausted")?;
                let status = self.queue.status();
                live.actions_admitted = !status.play_terminal
                    && !status.close_requested
                    && !self.control.stop_requested()
                    && status.queue_accepted < self.maximum_items;
                self.pending_output = false;
                self.observed_outputs = self
                    .observed_outputs
                    .checked_add(1)
                    .ok_or("Thermostat observed output bound exhausted")?;
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
        }
        Ok(())
    }

    pub(crate) fn progress(&mut self, owner: &mut Owner, root: &Path) -> Result<bool, String> {
        match self.started.try_recv() {
            Ok((play, wake)) => {
                let authority = owner.host.advertisement();
                let mut next = owner.session.clone();
                let result = next
                    .started(&authority.host_id, &authority.boot_id, play.clone(), wake)
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
                if result.is_ok() {
                    owner.session = next;
                    owner.thermostat_live = Some(ThermostatLive {
                        play: play.active_play_id,
                        state: self.initial,
                        revision: owner.session.evidence().last_sequence(),
                        actions_admitted: true,
                    });
                }
                if let Some(acknowledge) = self.acknowledge.take() {
                    let _ = acknowledge.send(result.clone());
                }
                result?;
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
        }
        self.observe_output(owner)?;
        // Stop and selected ingress exhaustion revoke controls even when no
        // further state Fore arrives. Refresh keeps availability truthful.
        if let Some(live) = &mut owner.thermostat_live {
            let status = self.queue.status();
            let admitted = !status.play_terminal
                && !status.close_requested
                && !self.control.stop_requested()
                && status.queue_accepted < self.maximum_items;
            if live.actions_admitted != admitted {
                live.actions_admitted = admitted;
                live.revision = live
                    .revision
                    .checked_add(1)
                    .ok_or("Thermostat Face revision exhausted")?;
            }
        }
        self.finish_if_ready(owner, root)
    }

    fn finish_if_ready(&mut self, owner: &mut Owner, root: &Path) -> Result<bool, String> {
        if !self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            return Ok(false);
        }
        let (host, result) = self
            .thread
            .take()
            .ok_or("Thermostat worker vanished")?
            .join()
            .map_err(|_| "Thermostat worker panicked; owner must restart".to_string())?;
        owner.host.restore_after_play(host)?;
        // The last Fore may arrive between the first output poll and the
        // terminal check. Join fences the producer before this final drain.
        self.observe_output(owner)?;
        let report = result?;
        let proposed = owner
            .session
            .realization()
            .ok_or("Thermostat Play vanished")?
            .clone();
        if proposed.play.as_ref() != Some(&report.play) {
            return Err("Thermostat worker returned a different Play".into());
        }
        let authority = owner.host.advertisement();
        let mut next = owner.session.clone();
        next.lull(&authority.host_id, &authority.boot_id, Some(&report.play))
            .map_err(debug)?;
        let child_signs = report.scan_child_signs.as_ref().map(|children| {
            children.iter().map(|child| serde_json::json!({
                "parent_active_play_id":child.parent_active_play_id,
                "activation_id":child.activation_id, "selected_plan_id":child.selected_plan_id,
                "invocation":child.invocation, "child_host_id":child.child_host_id,
                "child_active_play_id":child.child_active_play_id,
                "events":child.events.iter().map(|event| serde_json::json!({
                    "sequence":event.sequence, "node":event.node.0,
                    "port":event.port.map(|port| port.0), "request":event.request.map(|request| request.0),
                    "kind":format!("{:?}", event.kind),
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>()
        });
        let receipt = serde_json::json!({
            "schema":"conduit.thermostat/owner-scan-receipt@1",
            "observed_outputs":self.observed_outputs,
            "pending_output":self.pending_output,
            "state":owner.thermostat_live.as_ref().filter(|_| self.observed_outputs > 0).map(|live| serde_json::json!({
                "revision":live.state.revision,"target_decicelsius":live.state.target,
                "mode":format!("{:?}",live.state.mode),"fan":format!("{:?}",live.state.fan),
                "preset":format!("{:?}",live.state.preset),"measured_decicelsius":live.state.measured,
            })),
            "body_id":proposed.wake.body_id, "plan_id":proposed.plan.plan_id,
            "play":report.play, "terminal":report.terminal, "terminal_sign":report.terminal_sign,
            "failure":report.failure, "cleanup_failure":report.cleanup_failure,
            "live_fore_status":report.live_fore_status.map(|status| serde_json::json!({
                "queue_accepted":status.queue_accepted,"kernel_admitted":status.kernel_admitted,
                "queued":status.queued,"close_requested":status.close_requested,
                "kernel_closed":status.kernel_closed,"play_terminal":status.play_terminal,
            })), "scan_child_signs":child_signs.as_ref().ok(),
            "scan_child_sign_refusal":child_signs.as_ref().err().map(|error| format!("{error:?}")),
            "kernel_failure":report.kernel_failure.map(|failure| serde_json::json!({"code":failure.code.as_str(), "detail":failure.detail})),
            "scan_cancellation_failed":report.scan_cancellation_failed,
            "scan_output_completion_failed":report.scan_output_completion_failed,
        });
        state::retain_session(
            root,
            &mut next,
            Some(&receipt),
            owner.admissions.as_ref(),
            None,
            None,
        )?;
        owner.session = next;
        owner.last_execution = Some(receipt);
        owner.thermostat_live = None;
        Ok(true)
    }

    pub(crate) fn request_lull(&self) -> Result<(), String> {
        self.control
            .request_stop(RunControlRequestId::new("owner/thermostat-lull")?)
            .map_err(|_| "Thermostat already has a Stop request".into())
    }
}

impl Drop for ThermostatWorker {
    fn drop(&mut self) {
        if let Some(worker) = self.thread.take() {
            // A worker waiting for durable start must be released before join.
            // Drop owns only shutdown, never a replacement Owner Host.
            self.acknowledge.take();
            let _ = self.control.request_stop(
                RunControlRequestId::new("owner/thermostat-shutdown").expect("bounded request"),
            );
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
#[path = "thermostat_worker_tests.rs"]
mod tests;
