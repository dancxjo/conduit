//! One service-owned Body Play on the installed Host's existing kernel.
//! The service retains the biography; only the Host and finite kernel run move
//! to the worker. Start is acknowledged after durable lifecycle publication.
use super::{debug, runtime_face, state, BoundedOutput, DeadlineTimer, Owner};
use conduit_body::{BodyPlayIdentity, Wake};
use conduit_std_host::body_execution::{
    BodyRunReport, BodyRunRequest, CommittedBodyStartup, ImageBodyRunReport,
};
#[path = "continuing/patchbay_receiver.rs"]
pub(super) mod patchbay_receiver;
#[path = "continuing/startup.rs"]
pub(super) mod startup;
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
const SERVICE_DEADLINE_REQUEST: &str = "owner/service-deadline";
const BODY_PLAY_STACK_BYTES: usize = 4 * 1024 * 1024;
enum RunReport {
    Reference(BodyRunReport),
    Image(ImageBodyRunReport),
}
impl RunReport {
    fn report(&self) -> &BodyRunReport {
        match self {
            Self::Reference(report) => report,
            Self::Image(report) => report.report(),
        }
    }
}
type WorkerOutcome = (StdHost, Result<RunReport, String>, Vec<u8>);

pub(crate) struct RunWorker {
    mode: WorkerMode,
}
enum WorkerMode {
    Local(Box<LocalWorker>),
    Member(super::startup_member::MemberWorker),
}
struct LocalWorker {
    control: RunControl,
    started: Receiver<(BodyPlayIdentity, Wake)>,
    faces: runtime_face::RuntimeFaceReceiver,
    patchbay: Option<patchbay_receiver::PreparedPatchbay>,
    acknowledge: Option<SyncSender<Result<Option<CommittedBodyStartup>, String>>>,
    thread: Option<JoinHandle<WorkerOutcome>>,
    retained_deadline: bool,
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
        authorize_startup_chime: bool,
        renewals: u16,
    ) -> Result<RunWorker, String> {
        if !(1..=MAXIMUM_SERVICE_RUN_MILLIS).contains(&maximum_millis) {
            return Err("service run duration must be 1..900000 milliseconds".into());
        }
        if self.host.is_playing() || self.session.realization().is_some() {
            return Err("Body already has a current proposal or Play".into());
        }
        self.ensure_execution_retained(root)?;
        // Each admitted Play can produce a terminal receipt, including the final renewal.
        state::require_execution_capacity_for(root, usize::from(renewals) + 1)?;
        let sources = super::super::retained_source::workset(root)?
            .ok_or("retained Source workset is unavailable for Body Play")?;
        if sources
            .iter()
            .any(|source| !source.plot.expanded.activations.is_empty())
        {
            return Err("service workset requires an installed activation-aware adapter".into());
        }
        let borrowed: Vec<_> = sources
            .iter()
            .map(|source| (&source.source, &source.plot))
            .collect();
        let chime = if authorize_startup_chime {
            if !sources.iter().any(|source| {
                source.plot.expanded.gears.iter().any(|gear| {
                    gear.kind_id.as_str() == conduit_semantic_catalog::STARTUP_CHIME_KIND
                })
            }) {
                return Err("Body has no retained Startup Chime".into());
            }
            let mut nonce = [0_u8; 16];
            getrandom::fill(&mut nonce).map_err(|_| "startup chime grant entropy unavailable")?;
            let grant_id = format!("wake/startup-chime/{:032x}", u128::from_be_bytes(nonce));
            Some(self.host.startup_chime_authority_grant(&grant_id)?)
        } else {
            None
        };
        self.plan_workset_for_wake(&borrowed, chime.as_ref())?;
        // Source/catalog ownership is preparation-only; the one finite worker
        // receives the sealed complete Body Plan, never parser contexts.
        drop(borrowed);
        self.persist(root)?;
        let proposed = self
            .session
            .realization()
            .expect("Plan just committed")
            .clone();
        let needs_startup = startup::needs_startup(&proposed.plan);
        let host = self.host.take_for_play()?;
        let control = RunControl::default();
        let patchbay = match patchbay_receiver::prepare(
            &host,
            &proposed.wake,
            &proposed.plan,
            &sources,
            control.clone(),
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.host.restore_after_play(host)?;
                self.retain_finished(root, Err(error.clone()), Vec::new(), None)?;
                return Err(error);
            }
        };
        drop(sources);
        let worker_patchbay = patchbay.clone();
        let worker_control = control.clone();
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (ack_tx, ack_rx) = mpsc::sync_channel(1);
        let (mut face_sender, faces) = runtime_face::channel();
        let thread = thread::Builder::new()
            .name("conduit-body-play".into())
            .stack_size(BODY_PLAY_STACK_BYTES)
            .spawn(move || {
                let mut host = host;
                let mut output = BoundedOutput(Vec::with_capacity(32 * 1024));
                let mut timer = DeadlineTimer {
                    until: Instant::now() + Duration::from_millis(maximum_millis),
                    stop_request: SERVICE_DEADLINE_REQUEST,
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
                                RunControlRequestId::new(SERVICE_DEADLINE_REQUEST)
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
                    let started = |play: &BodyPlayIdentity,
                                   wake: &Wake|
                     -> Result<
                        Option<conduit_std_host::body_execution::CommittedBodyStartup>,
                        String,
                    > {
                        started_tx
                            .send((play.clone(), wake.clone()))
                            .map_err(|_| "Body owner start observer disconnected".to_string())?;
                        ack_rx
                            .recv()
                            .map_err(|_| "Body owner start acknowledgement lost".to_string())?
                    };
                    let result = if let Some(mut patchbay) = worker_patchbay {
                        use conduit_std_host::body_execution::{
                            BodyStartedAcknowledgment, WaitingPatchbayFore,
                        };
                        host.run_selected_image_body_plan_with_patchbay_to(
                            request,
                            WaitingPatchbayFore {
                                queue: &patchbay.queue,
                                output: &mut patchbay.receiver,
                            },
                            &mut output,
                            &mut timer,
                            &mut face_sender,
                            |play, wake| {
                                Ok(started(play, wake)?.map_or(
                                    BodyStartedAcknowledgment::LifecycleOnly,
                                    BodyStartedAcknowledgment::CommittedStartup,
                                ))
                            },
                        )
                        .map(RunReport::Image)
                    } else if needs_startup && host.image_identity().is_some() {
                        host.run_selected_image_body_plan_with_startup_and_face_to(
                            request,
                            &mut output,
                            &mut timer,
                            &mut face_sender,
                            |play, wake| {
                                started(play, wake)?.ok_or_else(|| {
                                    "durable startup acknowledgment omitted evidence".to_string()
                                })
                            },
                        )
                        .map(RunReport::Image)
                    } else if needs_startup {
                        host.run_body_plan_with_committed_startup_to(
                            request,
                            &mut output,
                            &mut timer,
                            None,
                            |play, wake| {
                                started(play, wake)?.ok_or_else(|| {
                                    "durable startup acknowledgment omitted evidence".to_string()
                                })
                            },
                        )
                        .map(RunReport::Reference)
                    } else if host.image_identity().is_some() {
                        host.run_selected_image_body_plan_with_face_to(
                            request,
                            &mut output,
                            &mut timer,
                            &mut face_sender,
                            |play, wake| started(play, wake).map(|_| ()),
                        )
                        .map(RunReport::Image)
                    } else {
                        host.run_body_plan_to_with_start(
                            request,
                            &mut output,
                            &mut timer,
                            |play, wake| started(play, wake).map(|_| ()),
                        )
                        .map(RunReport::Reference)
                    };
                    let _ = finished.send(());
                    result
                });
                (host, result, output.0)
            })
            .expect("failure to launch the only Body Play worker stops the owner service");
        Ok(RunWorker {
            mode: WorkerMode::Local(Box::new(LocalWorker {
                control,
                started: started_rx,
                faces,
                patchbay,
                acknowledge: Some(ack_tx),
                thread: Some(thread),
                retained_deadline: false,
            })),
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
        result: Result<RunReport, String>,
        output: Vec<u8>,
        accepted_stop: Option<&RunControlRequestId>,
    ) -> Result<(), String> {
        let authority = self.host.advertisement();
        let proposed = self
            .session
            .realization()
            .ok_or("Body proposal vanished while its Host was executing")?
            .clone();
        let mut next = self.session.clone();
        // Borrow the report so its original IMAGE lease survives serialization
        // AND the existing durable owner transaction below.
        let receipt = match &result {
            Ok(owned_report) => {
                let report = owned_report.report();
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
                    "accepted_stop_request_id":accepted_stop.map(RunControlRequestId::as_str),
                    "startup_chimes":report.startup_chimes,
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

impl LocalWorker {
    pub(crate) fn progress(&mut self, owner: &mut Owner, root: &Path) -> Result<bool, String> {
        match self.started.try_recv() {
            Ok((play, wake)) => {
                let result = startup::retain_acknowledgment(owner, root, play, wake);
                if let Some(acknowledge) = self.acknowledge.take() {
                    let status = result.as_ref().map(|_| ()).map_err(Clone::clone);
                    let _ = acknowledge.send(result);
                    status?;
                    owner.runtime_patchbay = self.patchbay.clone();
                } else {
                    return Err("Body owner received duplicate Started acknowledgment".into());
                }
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
        }
        if !self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            self.faces.progress(owner)?;
            return Ok(false);
        }
        owner.runtime_patchbay = None;
        owner.patchbay_inspection = false;
        owner.runtime_face_inspection = false;
        if owner.runtime_face.take().is_some() {
            owner.advance_runtime_face_revision()?;
        }
        let (host, result, output) = self
            .thread
            .take()
            .expect("finished worker has a join handle")
            .join()
            .map_err(|_| "Body Play worker panicked; owner service must restart".to_string())?;
        owner.host.restore_after_play(host)?;
        let accepted_stop = self.control.accepted_stop_request();
        let clean_deadline = accepted_stop.as_ref().map(RunControlRequestId::as_str)
            == Some(SERVICE_DEADLINE_REQUEST)
            && result.as_ref().is_ok_and(|report| {
                let report = report.report();
                matches!(
                    report.terminal,
                    conduit_core::TerminalDisposition::Cancelled { .. }
                ) && report.failure.is_none()
                    && report.kernel_failure.is_none()
                    && report.cleanup_failure.is_none()
            });
        owner.retain_finished(root, result, output, accepted_stop.as_ref())?;
        self.retained_deadline = clean_deadline;
        Ok(true)
    }

    pub(crate) fn request_lull(&self) -> Result<(), String> {
        self.control
            .request_stop(RunControlRequestId::new("owner/service-lull")?)
            .map_err(|_| "Body Play already has a stop request".into())
    }
}

impl RunWorker {
    pub(crate) fn retained_clean_deadline(&self) -> bool {
        matches!(&self.mode, WorkerMode::Local(worker) if worker.retained_deadline)
    }

    pub(crate) fn request_renewing_lull(&self) -> Result<(), String> {
        let WorkerMode::Local(worker) = &self.mode else {
            return Err("renewing Lull requires a local Body Play".into());
        };
        match worker.request_lull() {
            Err(_) if worker.control.stop_requested() => Ok(()),
            result => result,
        }
    }

    pub(super) fn from_member(worker: super::startup_member::MemberWorker) -> Self {
        Self {
            mode: WorkerMode::Member(worker),
        }
    }
    pub(crate) fn progress(&mut self, owner: &mut Owner, root: &Path) -> Result<bool, String> {
        match &mut self.mode {
            WorkerMode::Local(worker) => worker.progress(owner, root),
            WorkerMode::Member(worker) => worker.progress(owner, root),
        }
    }
    pub(crate) fn request_lull(&self) -> Result<(), String> {
        match &self.mode {
            WorkerMode::Local(worker) => worker.request_lull(),
            WorkerMode::Member(worker) => worker.request_lull(),
        }
    }
}

#[cfg(test)]
#[path = "continuing/receipt_tests.rs"]
mod receipt_tests;
