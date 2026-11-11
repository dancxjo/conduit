//! One service-owned Body Play on the installed Host's existing kernel.
//! The service retains the biography; only the Host and finite kernel run move
//! to the worker. Start is acknowledged after durable lifecycle publication.
use super::{debug, state, BoundedOutput, DeadlineTimer, Owner};
use conduit_body::{BodyPlayIdentity, Wake};
use conduit_std_host::body_execution::BodyRunRequest;
#[path = "continuing/report.rs"]
mod report;
use conduit_std_host::{RunControl, RunControlRequestId, StdHost};
pub(super) use report::RunReport;
#[path = "continuing/fore.rs"]
mod fore;
#[path = "continuing/spawn.rs"]
mod spawn;
pub(crate) use fore::ImageIngress;
use fore::ReceiptFore;
pub(super) use fore::{boundaries_for_plots, boundaries_for_plots_with_local};
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
    Result<RunReport, String>,
    Vec<u8>,
    ReceiptFore,
    Option<conduit::creche_encounter::RetainedCrecheFactory>,
    Option<ImageIngress>,
);

pub(crate) struct RunWorker {
    control: RunControl,
    started: Receiver<(BodyPlayIdentity, Wake)>,
    acknowledge: Option<SyncSender<Result<(), String>>>,
    thread: Option<JoinHandle<WorkerOutcome>>,
    runtime_faces: super::runtime_face::RuntimeFaceReceiver,
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

    /// Plan the complete retained workset for the owner's existing finite
    /// receipt adapter. Output budgets confer no input or Host Call authority.
    pub(crate) fn plan_retained(&mut self, root: &Path) -> Result<(), String> {
        self.plan_retained_with_ingress(root, None)
    }

    pub(crate) fn plan_retained_with_ingress(
        &mut self,
        root: &Path,
        ingress: Option<&ImageIngress>,
    ) -> Result<(), String> {
        self.ensure_execution_retained(root)?;
        if let Some(ingress) = ingress {
            self.host.verify_selected_image_fore_input(&ingress.input)?;
        }
        #[cfg(target_os = "linux")]
        if let Some(retained) = self.selected_file_workset.take() {
            let result = (|| {
                retained.verify(&self.host, &self.session.evidence().body.workset)?;
                let boundaries = fore::boundaries_for_plots_with_local(
                    retained.checked.plots.iter(),
                    ingress,
                    self.local_fore_adapters.as_ref(),
                )?;
                let borrowed = retained
                    .checked
                    .plots
                    .iter()
                    .map(|plot| (&retained.checked.source, plot))
                    .collect::<Vec<_>>();
                self.plan_workset_with_source_boundaries(&borrowed, &boundaries)
            })();
            self.selected_file_workset = Some(retained);
            return result;
        }
        let sources = super::super::retained_source::workset(root)?
            .ok_or("retained Source workset is unavailable for Body Plan")?;
        let boundaries = fore::boundaries_for_plots_with_local(
            sources.iter().map(|source| &source.plot),
            ingress,
            self.local_fore_adapters.as_ref(),
        )?;
        let borrowed: Vec<_> = sources
            .iter()
            .map(|source| (&source.source, &source.plot))
            .collect();
        self.plan_workset_with_source_boundaries(&borrowed, &boundaries)
    }

    /// Foreground Run consumes its already proposed exact Plan, using the same
    /// admitted execution and durable Start acknowledgment as the service.
    pub(crate) fn execute_prepared(
        &mut self,
        root: &Path,
        maximum_millis: u64,
    ) -> Result<(), String> {
        self.execute_prepared_with_ingress(root, maximum_millis, None)
    }

    pub(crate) fn execute_prepared_with_ingress(
        &mut self,
        root: &Path,
        maximum_millis: u64,
        ingress: Option<ImageIngress>,
    ) -> Result<(), String> {
        if let Some(ingress) = &ingress {
            self.host.verify_selected_image_fore_input(&ingress.input)?;
        }
        if !(1..=60_000).contains(&maximum_millis) {
            return Err("run duration must be 1..60000 milliseconds".into());
        }
        self.ensure_execution_retained(root)?;
        let proposed = self
            .session
            .realization()
            .ok_or("plan required before run")?;
        if self.host.is_playing() || proposed.play.is_some() {
            return Err("Body already has a current Play".into());
        }
        if proposed
            .plan
            .plots
            .iter()
            .any(|plot| !plot.plan.activations.is_empty())
        {
            return Err("installed Body Play has no activation ingress or egress".into());
        }
        if proposed
            .plan
            .plots
            .iter()
            .flat_map(|plot| &plot.plan.fragments)
            .any(|fragment| fragment.host_id != self.host.advertisement().host_id)
        {
            return Err("remote-fragment-preparation-unavailable".into());
        }
        let sources = super::super::retained_source::workset(root)?
            .ok_or("retained Source workset is unavailable for Body Play")?;
        let supplied = conduit_body::BodyWorkset::from_plots(sources.iter().map(|source| {
            conduit_body::ResidentPlot::new(
                source.plot.expanded.source_document_id.clone(),
                source.plot.expanded.checked_plot_id.clone(),
            )
        }))
        .map_err(debug)?;
        let planned = conduit_body::BodyWorkset::from_plots(
            proposed.plan.plots.iter().map(|plot| plot.plot.clone()),
        )
        .map_err(debug)?;
        if supplied != planned || supplied != self.session.evidence().body.workset {
            return Err("prepared Plan differs from retained Source workset".into());
        }
        let has_fore = !fore::boundaries_for_plots_with_local(
            sources.iter().map(|source| &source.plot),
            ingress.as_ref(),
            self.local_fore_adapters.as_ref(),
        )?
        .is_empty();
        drop(sources);
        let mut worker =
            self.spawn_prepared_worker(root, maximum_millis, has_fore, None, ingress)?;
        while !worker.progress(self, root)? {
            thread::sleep(Duration::from_millis(2));
        }
        if let Some(error) = self
            .last_execution
            .as_ref()
            .and_then(|receipt| receipt.get("pre_play_refusal"))
            .and_then(|value| value.as_str())
        {
            return Err(error.to_owned());
        }
        Ok(())
    }

    pub(crate) fn start_service_run(
        &mut self,
        root: &Path,
        maximum_millis: u64,
    ) -> Result<RunWorker, String> {
        self.start_service_run_with_ingress(root, maximum_millis, None)
    }

    pub(crate) fn start_service_run_with_ingress(
        &mut self,
        root: &Path,
        maximum_millis: u64,
        ingress: Option<ImageIngress>,
    ) -> Result<RunWorker, String> {
        if let Some(ingress) = &ingress {
            self.host.verify_selected_image_fore_input(&ingress.input)?;
        }
        if !(1..=MAXIMUM_SERVICE_RUN_MILLIS).contains(&maximum_millis) {
            return Err("service run duration must be 1..900000 milliseconds".into());
        }
        if self.host.is_playing() || self.session.realization().is_some() {
            return Err("Body already has a current proposal or Play".into());
        }
        self.ensure_execution_retained(root)?;
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
        let boundaries = fore::boundaries_for_plots_with_local(
            sources.iter().map(|source| &source.plot),
            ingress.as_ref(),
            self.local_fore_adapters.as_ref(),
        )?;
        let has_fore = !boundaries.is_empty();
        let partitions = self.prepare_workset_partitions(&borrowed, &boundaries)?;
        drop(boundaries);
        let mut creche_binding = None;
        for source in &sources {
            if source.plot.expanded.name == "creche/main" {
                if has_fore {
                    return Err(
                        "service receipt Fore cannot share the Crèche's selected adapter".into(),
                    );
                }
                if creche_binding.is_some() {
                    return Err("workset has multiple Crèche adapters".into());
                }
                creche_binding = self.prepare_creche_service_run(root, &source.plot)?;
            }
        }
        // Source/catalog ownership ends after complete preparation, before Wake
        // or the one finite worker can retain parser contexts.
        drop(borrowed);
        drop(sources);
        let ad = self.host.advertisement();
        self.session
            .propose(partitions, &ad.host_id, &ad.boot_id)
            .map_err(debug)?;
        self.spawn_prepared_worker(root, maximum_millis, has_fore, creche_binding, ingress)
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
        self.runtime_face = None;
        Ok(())
    }

    fn retain_finished(
        &mut self,
        root: &Path,
        result: Result<RunReport, String>,
        output: Vec<u8>,
        fore: ReceiptFore,
    ) -> Result<(), String> {
        let authority = self.host.advertisement();
        let proposed = self
            .session
            .realization()
            .ok_or("Body proposal vanished while its Host was executing")?
            .clone();
        let mut next = self.session.clone();
        let mut receipt = match &result {
            Ok(report) => {
                let report = report.report();
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
                    "failure":report.failure, "kernel_failure":report.kernel_failure.map(|failure| serde_json::json!({
                        "code":failure.code.as_str(), "detail":failure.detail,
                    })),
                    "cleanup_failure":report.cleanup_failure,
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
        receipt["fore_outputs"] = fore.snapshot();
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
        self.runtime_faces.progress(owner)?;
        if owner.current_play_id().is_some() {
            if let Some(live) = &owner.creche_live {
                live.controller.start_production_if_reviewed()?;
            }
        }
        if !self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            return Ok(false);
        }
        let (host, result, output, fore, factory, _ingress) = self
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
            if let Some(products) = live.controller.take_retained_products()? {
                owner.creche_products = Some(products);
            }
        }
        owner.runtime_face = None;
        owner.retain_finished(root, result, output, fore)?;
        Ok(true)
    }

    pub(crate) fn request_lull(&self) -> Result<(), String> {
        self.control
            .request_stop(RunControlRequestId::new("owner/service-lull")?)
            .map_err(|_| "Body Play already has a stop request".into())
    }
}

#[cfg(test)]
#[path = "continuing/image_tests.rs"]
mod image_tests;
