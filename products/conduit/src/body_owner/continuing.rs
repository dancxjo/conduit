//! One service-owned Body Play on the installed Host's existing kernel.
//! The service retains the biography; only the Host and finite kernel run move
//! to the worker. Start is acknowledged after durable lifecycle publication.
use super::{debug, state, BoundedOutput, DeadlineTimer, Owner};
use conduit_body::{BodyPlayIdentity, ResidentPlot, Wake};
use conduit_std_host::body_execution::{BodyRunReport, BodyRunRequest};
use conduit_std_host::{RunControl, RunControlRequestId, StdHost};
use std::{
    path::Path,
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(crate) const MAXIMUM_SERVICE_RUN_MILLIS: u64 = 900_000;
// The installed kernel's finite inline scratch exceeds the platform default
// worker stack. This is one service worker per owned Body Play, never a pool.
const BODY_PLAY_STACK_BYTES: usize = 4 * 1024 * 1024;
type WorkerOutcome = (
    StdHost,
    Result<BodyRunReport, String>,
    Vec<u8>,
    Option<conduit::creche_encounter::CrecheEncounterFactory>,
);

pub(crate) struct RunWorker {
    control: RunControl,
    started: Receiver<(BodyPlayIdentity, Wake)>,
    acknowledge: Option<SyncSender<Result<(), String>>>,
    thread: Option<JoinHandle<WorkerOutcome>>,
    creche_controller: Receiver<conduit::creche_encounter::CrecheEncounterController>,
    creche_custody: Option<super::creche_route_custody::CrecheRouteCustody>,
}

impl Owner {
    pub(crate) fn current_play_id(&self) -> Option<&conduit_core::ActivePlayId> {
        self.session
            .realization()
            .and_then(|realization| realization.play.as_ref())
            .map(|play| &play.active_play_id)
    }

    pub(crate) fn start_service_run(
        &mut self,
        root: &Path,
        maximum_millis: u64,
    ) -> Result<RunWorker, String> {
        if !(1..=MAXIMUM_SERVICE_RUN_MILLIS).contains(&maximum_millis) {
            return Err("service run duration must be 1..900000 milliseconds".into());
        }
        if self.host.is_playing() || self.session.realization().is_some() {
            return Err("Body already has a current proposal or Play".into());
        }
        self.ensure_execution_retained(root)?;
        let checked = super::super::checked_retained_source(root)?
            .ok_or("retained source is unavailable for Body Play")?;
        let resident = ResidentPlot::new(
            checked.expanded.source_document_id.clone(),
            checked.expanded.checked_plot_id.clone(),
        );
        if self.resident.as_ref() != Some(&resident) {
            return Err("retained source differs from the Body's resident Plot".into());
        }
        let creche_binding = self.prepare_creche_service_run(root, &checked)?;
        self.plan(&checked)?;
        self.persist(root)?;
        state::require_execution_capacity(root)?;
        let proposed = self
            .session
            .realization()
            .expect("Plan just committed")
            .clone();
        let mut creche = self.take_creche_execution(creche_binding);
        let creche_custody = creche.as_ref().map(|run| run.custody.clone());
        let (creche_tx, creche_rx) = mpsc::sync_channel(1);
        let host = self.host.take_for_play()?;
        let control = RunControl::default();
        let worker_control = control.clone();
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (ack_tx, ack_rx) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("conduit-body-play".into())
            .stack_size(BODY_PLAY_STACK_BYTES)
            .spawn(move || {
                let mut host = host;
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
                                RunControlRequestId::new("owner/service-deadline")
                                    .expect("bounded request"),
                            );
                        }
                    });
                    let request = BodyRunRequest {
                        wake: &proposed.wake,
                        plan: &proposed.plan,
                        control: &worker_control,
                        keyboard: None,
                    };
                    let started = |play: &BodyPlayIdentity, wake: &Wake| {
                        started_tx
                            .send((play.clone(), wake.clone()))
                            .map_err(|_| "Body owner start observer disconnected".to_string())?;
                        ack_rx
                            .recv()
                            .map_err(|_| "Body owner start acknowledgement lost".to_string())?
                    };
                    let result = match creche.as_mut() {
                        Some(run) => run.run(
                            &mut host,
                            request,
                            &mut output,
                            &mut timer,
                            &creche_tx,
                            started,
                        ),
                        None => host.run_body_plan_to_with_start(
                            request,
                            &mut output,
                            &mut timer,
                            started,
                        ),
                    };
                    let _ = finished.send(());
                    result
                });
                (host, result, output.0, creche.map(|run| run.factory))
            })
            .expect("failure to launch the only Body Play worker stops the owner service");
        Ok(RunWorker {
            control,
            started: started_rx,
            acknowledge: Some(ack_tx),
            thread: Some(thread),
            creche_controller: creche_rx,
            creche_custody,
        })
    }

    fn retain_started(
        &mut self,
        root: &Path,
        play: BodyPlayIdentity,
        wake: Wake,
    ) -> Result<(), String> {
        let authority = self.host.advertisement();
        let mut next = self.session.clone();
        next.started(&authority.host_id, &authority.boot_id, play, wake)
            .map_err(debug)?;
        state::retain_session(
            root,
            &mut next,
            self.last_execution.as_ref(),
            self.admissions.as_ref(),
            None,
            None,
        )?;
        self.session = next;
        Ok(())
    }

    fn retain_finished(
        &mut self,
        root: &Path,
        result: Result<BodyRunReport, String>,
        output: Vec<u8>,
    ) -> Result<(), String> {
        let authority = self.host.advertisement();
        let proposed = self
            .session
            .realization()
            .ok_or("Body proposal vanished while its Host was executing")?
            .clone();
        let mut next = self.session.clone();
        let receipt = match result {
            Ok(report) => {
                if proposed.play.as_ref() != Some(&report.play) {
                    return Err("Body worker returned a different Play".into());
                }
                next.lull(&authority.host_id, &authority.boot_id, Some(&report.play))
                    .map_err(debug)?;
                serde_json::json!({
                    "host_id":authority.host_id, "boot_id":authority.boot_id,
                    "body_id":proposed.wake.body_id, "plan_id":proposed.plan.plan_id,
                    "wake_id":proposed.wake.wake_id, "play":report.play,
                    "wake_at_start":report.wake_at_start, "terminal":report.terminal,
                    "failure":report.failure, "cleanup_failure":report.cleanup_failure,
                    "fore_pending_output":report.fore_pending_output.as_ref().map(|pending|pending.metadata()),
                    "terminal_sign":report.terminal_sign,
                    "output_utf8":String::from_utf8_lossy(&output),
                })
            }
            Err(error) => {
                if proposed.play.is_some() {
                    return Err(format!(
                        "Body Play ended without terminal evidence: {error}"
                    ));
                }
                serde_json::json!({
                    "host_id":authority.host_id, "boot_id":authority.boot_id,
                    "body_id":proposed.wake.body_id, "plan_id":proposed.plan.plan_id,
                    "wake_id":proposed.wake.wake_id, "play":null,
                    "pre_play_refusal":error, "refusal_scope":"untyped-host-admission",
                })
            }
        };
        self.retain_execution(root, next, receipt)?;
        Ok(())
    }
}

impl RunWorker {
    pub(crate) fn progress(&mut self, owner: &mut Owner, root: &Path) -> Result<bool, String> {
        match self.creche_controller.try_recv() {
            Ok(controller) => {
                let custody = self
                    .creche_custody
                    .take()
                    .ok_or("Crèche returned a duplicate controller")?;
                owner.creche_live = Some(super::creche_worker::CrecheLive {
                    controller,
                    custody,
                });
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
        }
        match self.started.try_recv() {
            Ok((play, wake)) => {
                let result = owner.retain_started(root, play, wake);
                if let Some(acknowledge) = self.acknowledge.take() {
                    let _ = acknowledge.send(result.clone());
                }
                result?;
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
        }
        if owner.current_play_id().is_some() {
            if let Some(live) = &owner.creche_live {
                live.controller.start_production_if_reviewed()?;
            }
        }
        if !self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            return Ok(false);
        }
        let (host, result, output, factory) = self
            .thread
            .take()
            .expect("finished worker has a join handle")
            .join()
            .map_err(|_| "Body Play worker panicked; owner service must restart".to_string())?;
        owner.host.restore_after_play(host)?;
        if let Some(factory) = factory {
            owner.creche_factory = Some(factory);
        }
        if let Some(live) = &owner.creche_live {
            if let Some(products) = live.controller.take_products()? {
                owner.creche_products = Some(products);
            }
        }
        owner.retain_finished(root, result, output)?;
        Ok(true)
    }

    pub(crate) fn request_lull(&self) -> Result<(), String> {
        self.control
            .request_stop(RunControlRequestId::new("owner/service-lull")?)
            .map_err(|_| "Body Play already has a stop request".into())
    }
}
