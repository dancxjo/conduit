//! One scoped Crèche Play on the retained Body's already proposed ordinary Plan.
//! UI progress must keep servicing native retirement until the worker is joined.
use super::{state, BoundedOutput, DeadlineTimer, Owner};
use conduit::creche_encounter::CrecheEncounterAdapter;
use conduit_body::{BodyPlayIdentity, Wake};
use conduit_std_host::{
    body_execution::{BodyForeOutputAdapter, BodyRunReport, BodyRunRequest},
    hosted_creche::{CrecheExecutionContext, HostedCrecheAdapter},
    ExternalForeDelivery, RunControl, RunControlRequestId, StdHost,
};
use std::{
    path::Path,
    sync::{
        mpsc::{self, Receiver, SyncSender, TryRecvError},
        Arc, OnceLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
#[path = "creche_worker/preparation.rs"]
mod preparation;
use preparation::{ObservedAdapter, Prepared};

const STACK_BYTES: usize = 4 * 1024 * 1024;
type Started = (Arc<Prepared>, BodyPlayIdentity, Wake);
type Outcome = (
    StdHost,
    Result<BodyRunReport, String>,
    Vec<u8>,
    CrecheCompletion,
);

/// Retained adapter custody and its one adapter-accepted typed outcome.
/// Kernel acknowledgment and cleanup remain separate facts in the run report.
/// Absence of a delivery is preserved on refusal/cancellation, never synthesized.
pub(crate) struct CrecheCompletion {
    pub(crate) adapter: CrecheEncounterAdapter,
    pub(crate) delivery: Option<ExternalForeDelivery>,
    pub(crate) other_fore: Box<dyn BodyForeOutputAdapter + Send>,
}
impl CrecheCompletion {
    /// Transfer retained custody once to the native application's completion
    /// owner. Adapter acceptance does not replace the retained terminal report.
    #[allow(dead_code)] // Consumed by the pending native application composition.
    pub(crate) fn into_parts(
        self,
    ) -> (
        CrecheEncounterAdapter,
        Option<ExternalForeDelivery>,
        Box<dyn BodyForeOutputAdapter + Send>,
    ) {
        (self.adapter, self.delivery, self.other_fore)
    }
}
/// Refused preparation returns effect-owner custody to the native application;
/// that application can service renderer retirement before draining the adapter.
pub(crate) struct CrecheStartRefusal {
    pub(crate) reason: String,
    pub(crate) adapter: CrecheEncounterAdapter,
    pub(crate) other_fore: Box<dyn BodyForeOutputAdapter + Send>,
}
impl CrecheStartRefusal {
    /// Recover both effect owners so the application can service native
    /// retirement before explicitly draining adapter custody after refusal.
    #[allow(dead_code)] // Consumed by the pending native application composition.
    pub(crate) fn into_parts(
        self,
    ) -> (
        String,
        CrecheEncounterAdapter,
        Box<dyn BodyForeOutputAdapter + Send>,
    ) {
        (self.reason, self.adapter, self.other_fore)
    }
}
impl std::fmt::Debug for CrecheStartRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CrecheStartRefusal")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}
#[must_use = "retain the worker and pump native retirement until actual completion"]
pub(crate) struct CrecheWorker {
    control: RunControl,
    started: Receiver<Started>,
    acknowledge: Option<SyncSender<Result<(), String>>>,
    thread: Option<JoinHandle<Outcome>>,
    completion: Option<CrecheCompletion>,
    retained: bool,
    terminal_error: Option<String>,
}
impl Owner {
    /// The caller first admits the workset and proposes its complete ordinary
    /// Body Plan. This entrance never replaces the Body, workset or local Host.
    #[allow(dead_code)] // Native application composition is the next consumer.
    pub(crate) fn start_creche_run(
        &mut self,
        root: &Path,
        maximum_millis: u64,
        adapter: CrecheEncounterAdapter,
        other_fore: Box<dyn BodyForeOutputAdapter + Send>,
    ) -> Result<CrecheWorker, Box<CrecheStartRefusal>> {
        let admitted = (|| -> Result<_, String> {
            if !(1..=super::continuing::MAXIMUM_SERVICE_RUN_MILLIS).contains(&maximum_millis) {
                return Err("Crèche run duration must be 1..900000 milliseconds".into());
            }
            if self.host.is_playing() {
                return Err("Body Play already owns the Host".into());
            }
            let proposed = self
                .session
                .realization()
                .ok_or("Crèche requires a current Body proposal")?
                .clone();
            if proposed.play.is_some() {
                return Err("Crèche cannot restart an issued Body Play".into());
            }
            self.ensure_execution_retained(root)?;
            self.persist(root)?;
            state::require_execution_capacity(root)?;
            let host = self.host.take_for_play()?;
            Ok((host, proposed))
        })();
        let (host, proposed) = match admitted {
            Ok(admitted) => admitted,
            Err(reason) => {
                return Err(Box::new(CrecheStartRefusal {
                    reason,
                    adapter,
                    other_fore,
                }))
            }
        };
        let control = RunControl::default();
        let worker_control = control.clone();
        let (started_tx, started) = mpsc::sync_channel(1);
        let (acknowledge, ack_rx) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("conduit-creche-play".into())
            .stack_size(STACK_BYTES)
            .spawn(move || {
                let mut host = host;
                let prepared = Arc::new(OnceLock::new());
                let mut adapter = ObservedAdapter::new(adapter, prepared.clone());
                let mut fore = OneOutcome {
                    prepared: prepared.clone(),
                    delivery: None,
                    other_fore,
                };
                let mut output = BoundedOutput(Vec::with_capacity(32 * 1024));
                let mut timer = DeadlineTimer {
                    until: Instant::now() + Duration::from_millis(maximum_millis),
                    control: worker_control.clone(),
                };
                let result = thread::scope(|scope| {
                    let (finished, receiver) = mpsc::sync_channel::<()>(1);
                    let deadline_control = worker_control.clone();
                    scope.spawn(move || {
                        if receiver
                            .recv_timeout(Duration::from_millis(maximum_millis))
                            .is_err()
                        {
                            let _ = deadline_control.request_stop(
                                RunControlRequestId::new("owner/creche-deadline")
                                    .expect("bounded request"),
                            );
                        }
                    });
                    let result = host.run_body_plan_with_creche_to_with_start(
                        BodyRunRequest {
                            wake: &proposed.wake,
                            plan: &proposed.plan,
                            control: &worker_control,
                            keyboard: None,
                        },
                        &mut adapter,
                        &mut fore,
                        &mut output,
                        &mut timer,
                        |play, wake| {
                            let context = prepared
                                .get()
                                .ok_or("Crèche start lacks prepared context")?
                                .clone();
                            if context.context.play() != play
                                || !play.validate_for(context.context.plan())
                            {
                                return Err("Crèche start differs from prepared execution".into());
                            }
                            started_tx
                                .send((context, play.clone(), wake.clone()))
                                .map_err(|_| "Crèche owner start observer disconnected")?;
                            // Stay owned until actual owner ACK or disconnection. The
                            // watchdog requests Stop; it cannot expire this message and
                            // let a delayed UI publish a start after callback refusal.
                            ack_rx
                                .recv()
                                .map_err(|_| "Crèche owner start acknowledgement unavailable")?
                        },
                    );
                    let _ = finished.send(());
                    result
                });
                (
                    host,
                    result,
                    output.0,
                    CrecheCompletion {
                        adapter: adapter.into_inner(),
                        delivery: fore.delivery,
                        other_fore: fore.other_fore,
                    },
                )
            })
            .expect("failure to launch the only Body Play worker stops the owner service");
        Ok(CrecheWorker {
            control,
            started,
            acknowledge: Some(acknowledge),
            thread: Some(thread),
            completion: None,
            retained: false,
            terminal_error: None,
        })
    }
}
impl Drop for CrecheWorker {
    fn drop(&mut self) {
        // Request Stop only; dropping a handle is never reported as a drain or
        // Host return. OwnerHost remains absent until explicit joined progress.
        if self.thread.is_some() {
            let _ = self.request_lull();
            self.acknowledge.take();
        }
    }
}
impl CrecheWorker {
    /// Only a real started event can invoke `bind_native`. Retain lifecycle
    /// evidence before that callback and ACK; preparation alone never calls it.
    /// On error keep this worker and service retirement; no join is attempted
    /// until its actual handle reports finished.
    #[allow(dead_code)]
    pub(crate) fn progress(
        &mut self,
        owner: &mut Owner,
        root: &Path,
        bind_native: impl FnOnce(&CrecheExecutionContext, &BodyPlayIdentity) -> Result<(), String>,
    ) -> Result<bool, String> {
        match self.started.try_recv() {
            Ok((prepared, play, wake)) => {
                let retained = owner.retain_started(root, play.clone(), wake);
                let binding = match &retained {
                    Ok(()) => bind_native(&prepared.context, &play),
                    Err(error) => Err(error.clone()),
                };
                if binding.is_err() {
                    let _ = self.request_lull();
                }
                // Once the real start is durably retained, a renderer failure
                // must still let the kernel return its actual cancelled report.
                // Bare start refusal cannot retire an already recorded Play.
                if let Some(ack) = self.acknowledge.take() {
                    let _ = ack.send(retained);
                }
                binding?;
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
        }
        if self.retained {
            return Ok(true);
        }
        if let Some(error) = &self.terminal_error {
            return Err(error.clone());
        }
        if self.thread.is_none() && self.completion.is_some() {
            owner.ensure_execution_retained(root)?;
            self.retained = true;
            return Ok(true);
        }
        if !self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            return Ok(false);
        }
        let joined = self
            .thread
            .take()
            .expect("finished worker has handle")
            .join();
        let (host, report, output, completion) = match joined {
            Ok(outcome) => outcome,
            Err(_) => {
                let error = "Crèche Body worker panicked; owner service must restart".to_string();
                self.terminal_error = Some(error.clone());
                return Err(error);
            }
        };
        self.completion = Some(completion);
        if let Err(error) = owner.host.restore_after_play(host) {
            self.terminal_error = Some(error.clone());
            return Err(error);
        }
        if let Err(error) = owner.retain_finished(root, report, output) {
            if !owner.execution_persistence_pending {
                self.terminal_error = Some(error.clone());
            }
            return Err(error);
        }
        self.retained = true;
        Ok(true)
    }
    #[allow(dead_code)]
    pub(crate) fn request_lull(&self) -> Result<(), String> {
        self.control
            .request_stop(RunControlRequestId::new("owner/creche-lull")?)
            .map_err(|_| "Crèche Body Play already has a stop request".into())
    }
    #[allow(dead_code)]
    pub(crate) fn take_completion(&mut self) -> Option<CrecheCompletion> {
        if self.retained {
            self.completion.take()
        } else {
            None
        }
    }
}
struct OneOutcome {
    prepared: Arc<OnceLock<Arc<Prepared>>>,
    delivery: Option<ExternalForeDelivery>,
    other_fore: Box<dyn BodyForeOutputAdapter + Send>,
}
impl BodyForeOutputAdapter for OneOutcome {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        use conduit_body_make::deployment_runtime::{CrecheOutcome, CRECHE_OUTCOME_KIND};
        let prepared = self
            .prepared
            .get()
            .ok_or("Crèche outcome precedes preparation")?;
        if output.front_port_id != prepared.output {
            return self.other_fore.deliver(output);
        }
        let outcome = CrecheOutcome::decode(&output.bytes).ok_or("Crèche outcome is malformed")?;
        if self.delivery.is_some()
            || output.front_port_id != prepared.output
            || output.track != conduit_core::ConnectionTrack::Payload
            || output.value_kind != conduit_core::kind_id(CRECHE_OUTCOME_KIND)
            || outcome.request != prepared.request
        {
            return Err("Crèche emitted an unexpected or duplicate typed outcome".into());
        }
        self.delivery = Some(output.clone());
        Ok(())
    }
}

#[cfg(test)]
#[path = "creche_worker/tests.rs"]
mod tests;
