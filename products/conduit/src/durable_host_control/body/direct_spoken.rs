//! The selected owner Mask's one finite Face-to-artifact Play.
use super::{DurableHostRuntime, HostSource};
use crate::durable_host::owner::DirectSpokenStart;
use conduit_core::{port_id, ConnectionTrack};
use conduit_presentation::DirectArtifactAcknowledgedSpokenShow;
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeOutputAdapter, RunControl, RunControlRequestId, StdHost,
    TimerAdapter,
};
use serde_json::{json, Value};
use std::{sync::mpsc, thread::JoinHandle, time::Duration};

struct ResultShow {
    shown: DirectArtifactAcknowledgedSpokenShow,
    active_play_id: conduit_core::ActivePlayId,
}

pub(in crate::durable_host_control) struct DirectSpokenWorker {
    operation_id: String,
    control: RunControl,
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
            return Err("direct spoken Mask emitted an unexpected Show".into());
        }
        self.0 = Some(output);
        Ok(())
    }
}

struct NoWait;
impl TimerAdapter for NoWait {
    fn wait(&mut self, _: Duration) {}
}

fn run_one(
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
    if control.stop_requested() {
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
        shown,
        active_play_id: kernel.active_play_id,
    })
}

impl DurableHostRuntime {
    pub(in crate::durable_host_control) fn admit_direct_spoken(&mut self) -> Result<Value, String> {
        if self.direct_spoken_worker.is_some() || self.speech_worker.is_some() {
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
        if self.direct_spoken_worker.is_some() || self.speech_worker.is_some() {
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

    pub(in crate::durable_host_control) fn start_direct_spoken(
        &mut self,
    ) -> Result<String, String> {
        self.progress_direct_spoken()?;
        if self.direct_spoken_worker.is_some() || self.speech_worker.is_some() {
            return Err("selected speech already owns the Host".into());
        }
        #[cfg(unix)]
        if super::super::terminal_attach::is_attached(self) {
            return Err("terminal attachment must detach before direct speech".into());
        }
        let HostSource::Body {
            owner,
            running: None,
            ..
        } = &mut self.host
        else {
            return Err("direct spoken Mask needs an idle Body owner".into());
        };
        let start = owner.prepare_selected_direct_spoken_start()?;
        let seal = start.seal.clone();
        let operation_id =
            crate::durable_host::fresh_identity("direct-spoken-mask", seal.route_plan_id.as_str());
        let (sender, receiver) = mpsc::sync_channel::<Option<StdHost>>(1);
        let control = RunControl::default();
        let worker_control = control.clone();
        let thread = std::thread::Builder::new()
            .name("conduit-owner-direct-spoken-mask".into())
            .spawn(move || {
                let mut host = receiver.recv().ok().flatten()?;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_one(&mut host, start, &worker_control)
                }))
                .unwrap_or_else(|_| Err("direct spoken Mask worker panicked".into()));
                Some((host, result))
            })
            .map_err(|error| format!("start direct spoken Mask worker: {error}"))?;
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
            return Err("direct spoken Mask worker stopped before Host handoff".into());
        }
        self.direct_spoken_worker = Some(DirectSpokenWorker {
            operation_id: operation_id.clone(),
            control,
            seal,
            thread,
        });
        self.direct_spoken_terminal = None;
        Ok(operation_id)
    }

    pub(in crate::durable_host_control) fn progress_direct_spoken(&mut self) -> Result<(), String> {
        if !self
            .direct_spoken_worker
            .as_ref()
            .is_some_and(|worker| worker.thread.is_finished())
        {
            return Ok(());
        }
        let worker = self
            .direct_spoken_worker
            .take()
            .expect("finished direct speech worker");
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("direct spoken Mask lost its Body owner".into());
        };
        let (host, result) = worker
            .thread
            .join()
            .map_err(|_| "direct spoken Mask worker failed before returning Host".to_string())?
            .ok_or("direct spoken Mask worker exited without Host handoff")?;
        owner.host.restore_after_play(host)?;
        let terminal = match result {
            Ok(result) => match owner
                .acknowledge_selected_direct_spoken_show(&worker.seal, &result.shown.show)
            {
                Ok(()) => json!({"schema":"conduit.body/direct-spoken-terminal@1",
                    "operation_id":worker.operation_id, "outcome":"available",
                    "route_plan_id":worker.seal.route_plan_id,
                    "active_play_id":result.active_play_id,
                    "show_id":result.shown.show.show_id,
                    "artifact":result.shown.artifact,
                    "speaker_played":false}),
                Err(detail) => json!({"schema":"conduit.body/direct-spoken-terminal@1",
                    "operation_id":worker.operation_id, "outcome":"show-refused",
                    "route_plan_id":worker.seal.route_plan_id, "detail":detail}),
            },
            Err(detail) => json!({"schema":"conduit.body/direct-spoken-terminal@1",
                "operation_id":worker.operation_id,
                "outcome":if worker.control.stop_requested() {"cancelled"} else {"failed"},
                "route_plan_id":worker.seal.route_plan_id, "detail":detail}),
        };
        self.direct_spoken_terminal = Some(terminal);
        Ok(())
    }

    pub(in crate::durable_host_control) fn direct_spoken_status(
        &mut self,
        operation_id: &str,
    ) -> Result<Value, String> {
        self.progress_direct_spoken()?;
        if self
            .direct_spoken_worker
            .as_ref()
            .is_some_and(|worker| worker.operation_id == operation_id)
        {
            return Ok(json!({"schema":"conduit.body/direct-spoken-status@1",
                "operation_id":operation_id,"state":"running"}));
        }
        self.direct_spoken_terminal
            .as_ref()
            .filter(|terminal| terminal["operation_id"] == operation_id)
            .cloned()
            .ok_or_else(|| "unknown direct spoken operation".into())
    }

    pub(in crate::durable_host_control) fn stop_direct_spoken(
        &mut self,
        operation_id: &str,
    ) -> Result<(), String> {
        self.progress_direct_spoken()?;
        let worker = self
            .direct_spoken_worker
            .as_ref()
            .filter(|worker| worker.operation_id == operation_id)
            .ok_or("direct spoken Mask is not running")?;
        worker
            .control
            .request_stop(RunControlRequestId::new(format!("stop/{operation_id}"))?)
            .map_err(|_| "direct spoken Mask stop already requested".into())
    }
}
