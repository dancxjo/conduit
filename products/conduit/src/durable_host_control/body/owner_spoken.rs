//! The selected owner's one finite Face-to-artifact speech Play.
use super::{DurableHostRuntime, HostSource};
use crate::durable_host::owner::{DirectSpokenStart, LlmSpokenStart};
use conduit_core::{port_id, ConnectionTrack};
use conduit_presentation::{
    ArtifactAcknowledgedSpokenShow, DirectArtifactAcknowledgedSpokenShow, MaskShow,
    SpokenMaskArtifactReceipt,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeOutputAdapter, RunControl, RunControlRequestId, StdHost,
    TimerAdapter,
};
use serde_json::{json, Value};
use std::{sync::mpsc, thread::JoinHandle, time::Duration};

struct ResultShow {
    show: MaskShow,
    artifact: SpokenMaskArtifactReceipt,
    generated_manifestation_identity: Option<String>,
    accepted_wording: Option<String>,
    active_play_id: conduit_core::ActivePlayId,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Mode {
    Direct,
    Llm,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Llm => "llm-assisted",
        }
    }
}

enum Start {
    Direct(Box<DirectSpokenStart>),
    Llm(Box<LlmSpokenStart>),
}

impl Start {
    fn seal(&self) -> &conduit_presentation::LocalOwnerMaskRouteSeal {
        match self {
            Self::Direct(start) => &start.seal,
            Self::Llm(start) => &start.seal,
        }
    }
}

pub(in crate::durable_host_control) struct OwnerSpokenWorker {
    operation_id: String,
    control: RunControl,
    mode: Mode,
    seal: conduit_presentation::LocalOwnerMaskRouteSeal,
    thread: JoinHandle<Option<(StdHost, Result<ResultShow, String>)>>,
}

#[derive(Default)]
struct OneShow(Option<ExternalForeDelivery>);

impl ExternalForeOutputAdapter for OneShow {
    fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String> {
        if self.0.is_some()
            || output.front_port_id != port_id("show")
            || output.track != ConnectionTrack::Payload
            || output.sequence != 0
            || output.bytes.len() > 262_144
        {
            return Err("owner spoken Mask emitted an unexpected Show".into());
        }
        self.0 = Some(output);
        Ok(())
    }
}

struct NoWait;
impl TimerAdapter for NoWait {
    fn wait(&mut self, _: Duration) {}
}

fn run_one(host: &mut StdHost, start: Start, control: &RunControl) -> Result<ResultShow, String> {
    match start {
        Start::Direct(start) => run_direct(host, *start, control),
        Start::Llm(start) => run_llm(host, *start, control),
    }
}

fn run_direct(
    host: &mut StdHost,
    start: DirectSpokenStart,
    control: &RunControl,
) -> Result<ResultShow, String> {
    if !host.spoken_mask_artifact_route_is_current() {
        return Err("direct spoken provider or artifact was lost before Play".into());
    }
    let mut collector = OneShow::default();
    let report = host.run_direct_spoken_mask_controlled_to(
        start.fragment,
        start.preparation,
        &[start.input],
        &mut collector,
        &mut std::io::sink(),
        &mut NoWait,
        control,
    )?;
    if control.stop_requested() && collector.0.is_none() {
        return Err("direct spoken Mask Play was cancelled".into());
    }
    let kernel = report.kernel.ok_or("direct spoken Mask returned no Play")?;
    let delivery = collector.0.ok_or("direct spoken Mask emitted no Show")?;
    let shown: DirectArtifactAcknowledgedSpokenShow = serde_json::from_slice(&delivery.bytes)
        .map_err(|error| format!("decode direct spoken Show: {error}"))?;
    shown
        .validate(&start.face)
        .map_err(|error| format!("validate direct spoken Show: {error:?}"))?;
    if shown.show.show.active_play_id != kernel.active_play_id
        || shown.show.planned_mask.plan.plan_id != start.seal.planned_mask.plan.plan_id
    {
        return Err("direct spoken Show differs from its completed Play or route".into());
    }
    Ok(ResultShow {
        show: shown.show,
        artifact: shown.artifact,
        generated_manifestation_identity: None,
        accepted_wording: None,
        active_play_id: kernel.active_play_id,
    })
}

fn run_llm(
    host: &mut StdHost,
    start: LlmSpokenStart,
    control: &RunControl,
) -> Result<ResultShow, String> {
    if !host.spoken_mask_artifact_route_is_current()
        || host
            .observe_planned_llm_provider(&start.seal.planned_mask)
            .is_err()
    {
        return Err("selected model, voice, or artifact was lost before LLM spoken Play".into());
    }
    let mut collector = OneShow::default();
    let report = host.run_spoken_mask_plot_controlled_to(
        start.fragment,
        start.preparation,
        &[start.input],
        &mut collector,
        &mut std::io::sink(),
        &mut NoWait,
        control,
    )?;
    if control.stop_requested() && collector.0.is_none() {
        return Err("LLM spoken Mask Play was cancelled".into());
    }
    let kernel = report.kernel.ok_or("LLM spoken Mask returned no Play")?;
    let delivery = collector.0.ok_or("LLM spoken Mask emitted no Show")?;
    let shown: ArtifactAcknowledgedSpokenShow = serde_json::from_slice(&delivery.bytes)
        .map_err(|error| format!("decode LLM spoken Show: {error}"))?;
    // The ordinary semantic Host session validated the model candidate and
    // its exact Face correlations before minting this Show. Recheck its outer
    // Face/artifact/Plan/Play envelope before the owner may acknowledge it.
    shown
        .validate_owner_artifact(&start.face)
        .map_err(|error| format!("validate LLM spoken artifact Show: {error:?}"))?;
    if shown.show.show.active_play_id != kernel.active_play_id
        || shown.show.planned_mask.plan.plan_id != start.seal.planned_mask.plan.plan_id
    {
        return Err("LLM spoken Show differs from its completed Play or route".into());
    }
    Ok(ResultShow {
        show: shown.show,
        artifact: shown.artifact,
        generated_manifestation_identity: Some(shown.generated_manifestation_identity),
        accepted_wording: Some(shown.accepted_wording),
        active_play_id: kernel.active_play_id,
    })
}

impl DurableHostRuntime {
    pub(in crate::durable_host_control) fn admit_direct_spoken(&mut self) -> Result<Value, String> {
        if self.owner_spoken_worker.is_some() || self.speech_worker.is_some() {
            return Err("selected speech already owns the Host".into());
        }
        let HostSource::Body {
            owner,
            running: None,
            ..
        } = &mut self.host
        else {
            return Err("direct spoken Mask needs an idle Body owner".into());
        };
        let seal = owner.admit_direct_spoken_route()?;
        Ok(json!({"schema":"conduit.body/direct-spoken-route@1",
            "route_plan_id":seal.route_plan_id, "mask_plot":seal.planned_mask.mask.plot_identity,
            "selection_unchanged":true, "show":null}))
    }

    pub(in crate::durable_host_control) fn select_direct_spoken(
        &mut self,
    ) -> Result<Value, String> {
        if self.owner_spoken_worker.is_some() || self.speech_worker.is_some() {
            return Err("selected speech already owns the Host".into());
        }
        let HostSource::Body {
            owner,
            running: None,
            ..
        } = &mut self.host
        else {
            return Err("direct spoken Mask needs an idle Body owner".into());
        };
        let seal = owner.select_direct_spoken_route()?;
        Ok(json!({"schema":"conduit.body/direct-spoken-route@1",
            "route_plan_id":seal.route_plan_id, "mask_plot":seal.planned_mask.mask.plot_identity,
            "selected":true, "show":null}))
    }

    pub(in crate::durable_host_control) fn admit_llm_spoken(&mut self) -> Result<Value, String> {
        if self.owner_spoken_worker.is_some() || self.speech_worker.is_some() {
            return Err("selected speech already owns the Host".into());
        }
        let HostSource::Body {
            owner,
            running: None,
            ..
        } = &mut self.host
        else {
            return Err("LLM spoken Mask needs an idle Body owner".into());
        };
        let seal = owner.admit_llm_spoken_route()?;
        Ok(json!({"schema":"conduit.body/llm-spoken-route@1",
            "route_plan_id":seal.route_plan_id, "mask_plot":seal.planned_mask.mask.plot_identity,
            "selection_unchanged":true, "show":null}))
    }

    pub(in crate::durable_host_control) fn select_llm_spoken(&mut self) -> Result<Value, String> {
        if self.owner_spoken_worker.is_some() || self.speech_worker.is_some() {
            return Err("selected speech already owns the Host".into());
        }
        let HostSource::Body {
            owner,
            running: None,
            ..
        } = &mut self.host
        else {
            return Err("LLM spoken Mask needs an idle Body owner".into());
        };
        let seal = owner.select_llm_spoken_route()?;
        Ok(json!({"schema":"conduit.body/llm-spoken-route@1",
            "route_plan_id":seal.route_plan_id, "mask_plot":seal.planned_mask.mask.plot_identity,
            "selected":true, "show":null}))
    }

    pub(in crate::durable_host_control) fn start_direct_spoken(
        &mut self,
    ) -> Result<String, String> {
        self.start_owner_spoken(Mode::Direct)
    }

    pub(in crate::durable_host_control) fn start_llm_spoken(&mut self) -> Result<String, String> {
        self.start_owner_spoken(Mode::Llm)
    }

    fn start_owner_spoken(&mut self, mode: Mode) -> Result<String, String> {
        self.progress_owner_spoken()?;
        if self.owner_spoken_worker.is_some() || self.speech_worker.is_some() {
            return Err("selected speech already owns the Host".into());
        }
        #[cfg(unix)]
        if super::super::terminal_attach::is_attached(self) {
            return Err("terminal attachment must detach before owner speech".into());
        }
        let HostSource::Body {
            owner,
            running: None,
            ..
        } = &mut self.host
        else {
            return Err("owner spoken Mask needs an idle Body owner".into());
        };
        let start = match mode {
            Mode::Direct => Start::Direct(Box::new(owner.prepare_selected_direct_spoken_start()?)),
            Mode::Llm => Start::Llm(Box::new(owner.prepare_selected_llm_spoken_start()?)),
        };
        let seal = start.seal().clone();
        let operation_id =
            crate::durable_host::fresh_identity("owner-spoken-mask", seal.route_plan_id.as_str());
        let (sender, receiver) = mpsc::sync_channel::<Option<StdHost>>(1);
        let control = RunControl::default();
        let worker_control = control.clone();
        let thread = std::thread::Builder::new()
            .name("conduit-owner-spoken-mask".into())
            .spawn(move || {
                let mut host = receiver.recv().ok().flatten()?;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_one(&mut host, start, &worker_control)
                }))
                .unwrap_or_else(|_| Err("owner spoken Mask worker panicked".into()));
                Some((host, result))
            })
            .map_err(|error| format!("start owner spoken Mask worker: {error}"))?;
        let host = match owner.host.take_for_play() {
            Ok(host) => host,
            Err(error) => {
                let _ = sender.send(None);
                let _ = thread.join();
                return Err(error);
            }
        };
        if let Err(error) = sender.send(Some(host)) {
            if let Some(host) = error.0 {
                owner.host.restore_after_play(host)?;
            }
            let _ = thread.join();
            return Err("owner spoken Mask worker stopped before Host handoff".into());
        }
        self.owner_spoken_worker = Some(OwnerSpokenWorker {
            operation_id: operation_id.clone(),
            control,
            mode,
            seal,
            thread,
        });
        self.owner_spoken_terminal = None;
        Ok(operation_id)
    }

    pub(in crate::durable_host_control) fn progress_owner_spoken(&mut self) -> Result<(), String> {
        if !self
            .owner_spoken_worker
            .as_ref()
            .is_some_and(|worker| worker.thread.is_finished())
        {
            return Ok(());
        }
        let worker = self
            .owner_spoken_worker
            .take()
            .expect("finished owner speech worker");
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("owner spoken Mask lost its Body owner".into());
        };
        let (host, result) = worker
            .thread
            .join()
            .map_err(|_| "owner spoken Mask worker failed before returning Host".to_string())?
            .ok_or("owner spoken Mask worker exited without Host handoff")?;
        owner.host.restore_after_play(host)?;
        let terminal = match result {
            Ok(result) => {
                let acknowledged = match worker.mode {
                    Mode::Direct => {
                        owner.acknowledge_selected_direct_spoken_show(&worker.seal, &result.show)
                    }
                    Mode::Llm => {
                        owner.acknowledge_selected_llm_spoken_show(&worker.seal, &result.show)
                    }
                };
                match acknowledged {
                    Ok(()) => json!({"schema":"conduit.body/owner-spoken-terminal@1",
                        "operation_id":worker.operation_id, "mode":worker.mode.name(),
                        "outcome":"available", "route_plan_id":worker.seal.route_plan_id,
                        "source_face_id":result.show.show.presentation_id,
                        "active_play_id":result.active_play_id,
                        "show_id":result.show.show_id,
                        "stop_requested":worker.control.stop_requested(),
                        "generated_manifestation_identity":result.generated_manifestation_identity,
                        "accepted_wording":result.accepted_wording,
                        "artifact":result.artifact, "speaker_played":false}),
                    Err(detail) => json!({"schema":"conduit.body/owner-spoken-terminal@1",
                        "operation_id":worker.operation_id, "mode":worker.mode.name(),
                        "outcome":"show-refused", "route_plan_id":worker.seal.route_plan_id,
                        "detail":detail}),
                }
            }
            Err(detail) => json!({"schema":"conduit.body/owner-spoken-terminal@1",
                "operation_id":worker.operation_id, "mode":worker.mode.name(),
                "outcome":if detail.ends_with("Play was cancelled") {"cancelled"} else {"failed"},
                "stop_requested":worker.control.stop_requested(),
                "route_plan_id":worker.seal.route_plan_id, "detail":detail}),
        };
        self.owner_spoken_terminal = Some(terminal);
        Ok(())
    }

    pub(in crate::durable_host_control) fn direct_spoken_status(
        &mut self,
        operation_id: &str,
    ) -> Result<Value, String> {
        self.owner_spoken_status(operation_id, Mode::Direct)
    }

    pub(in crate::durable_host_control) fn llm_spoken_status(
        &mut self,
        operation_id: &str,
    ) -> Result<Value, String> {
        self.owner_spoken_status(operation_id, Mode::Llm)
    }

    fn owner_spoken_status(&mut self, operation_id: &str, mode: Mode) -> Result<Value, String> {
        self.progress_owner_spoken()?;
        if self
            .owner_spoken_worker
            .as_ref()
            .is_some_and(|worker| worker.operation_id == operation_id && worker.mode == mode)
        {
            return Ok(json!({"schema":"conduit.body/owner-spoken-status@1",
                "operation_id":operation_id, "mode":mode.name(), "state":"running"}));
        }
        self.owner_spoken_terminal
            .as_ref()
            .filter(|terminal| {
                terminal["operation_id"] == operation_id && terminal["mode"] == mode.name()
            })
            .cloned()
            .ok_or_else(|| "unknown owner spoken operation".into())
    }

    pub(in crate::durable_host_control) fn stop_direct_spoken(
        &mut self,
        operation_id: &str,
    ) -> Result<(), String> {
        self.stop_owner_spoken(operation_id, Mode::Direct)
    }

    pub(in crate::durable_host_control) fn stop_llm_spoken(
        &mut self,
        operation_id: &str,
    ) -> Result<(), String> {
        self.stop_owner_spoken(operation_id, Mode::Llm)
    }

    fn stop_owner_spoken(&mut self, operation_id: &str, mode: Mode) -> Result<(), String> {
        self.progress_owner_spoken()?;
        let worker = self
            .owner_spoken_worker
            .as_ref()
            .filter(|worker| worker.operation_id == operation_id && worker.mode == mode)
            .ok_or("owner spoken Mask is not running")?;
        worker
            .control
            .request_stop(RunControlRequestId::new(format!("stop/{operation_id}"))?)
            .map_err(|_| "owner spoken Mask stop already requested".into())
    }
}
