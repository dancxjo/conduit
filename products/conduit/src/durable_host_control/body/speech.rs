//! One acknowledged browser Show read through the installed owner's sole Host.
use super::{DurableHostRuntime, HostSource};
use crate::durable_host::selected_speech::AttachedEquipment;
use conduit_core::LinkBindingId;
use conduit_presentation::{MaskShow, OwnerFaceSnapshotRequest, Presentation};
use conduit_std_host::{
    spoken_face_mask::{ReaderCommand, SpokenFaceSession},
    spoken_face_stream_execution::{
        execute_spoken_batch_on_attached_host, SpokenPlaybackOutcome, SpokenStreamExecutionRefusal,
    },
    RunControl, RunControlRequestId, StdHost,
};
use serde_json::{json, Value};
use std::{sync::mpsc, thread::JoinHandle};

const MAXIMUM_BATCHES: usize = 64;

#[cfg(all(test, unix))]
#[path = "speech/tests.rs"]
mod tests;

enum SpeechFailure {
    Refused(String),
    Cancelled(String),
    PlayRefused(String),
    Failed(String),
}

impl SpeechFailure {
    fn outcome(&self) -> &'static str {
        match self {
            Self::Refused(_) => "refused",
            Self::Cancelled(_) => "cancelled",
            Self::PlayRefused(_) => "play-refused-unclassified",
            Self::Failed(_) => "failed",
        }
    }

    fn detail(self) -> String {
        match self {
            Self::Refused(detail)
            | Self::Cancelled(detail)
            | Self::PlayRefused(detail)
            | Self::Failed(detail) => detail,
        }
    }
}

pub(in crate::durable_host_control) struct SpeechWorker {
    operation_id: String,
    control: RunControl,
    source_window_id: String,
    source_binding: LinkBindingId,
    source_request: OwnerFaceSnapshotRequest,
    source_show: MaskShow,
    thread: JoinHandle<Option<(StdHost, Result<Value, SpeechFailure>)>>,
}

impl DurableHostRuntime {
    pub(in crate::durable_host_control) fn start_browser_speech(
        &mut self,
        window_id: String,
        binding: LinkBindingId,
        request: OwnerFaceSnapshotRequest,
        show: MaskShow,
    ) -> Result<String, String> {
        self.progress_browser_speech()?;
        if self.speech_worker.is_some() || self.owner_spoken_worker.is_some() {
            return Err("selected speech is already running".into());
        }
        #[cfg(unix)]
        if super::super::terminal_attach::is_attached(self) {
            return Err("terminal attachment must detach before selected speech".into());
        }
        let equipment = self
            .selected_speech_equipment
            .clone()
            .ok_or("installed Host has no selected speech equipment")?;
        let HostSource::Body {
            owner,
            running: None,
            ..
        } = &mut self.host
        else {
            return Err("selected speech requires an idle installed Body owner".into());
        };
        if !owner.selected_speech_host_is_idle() {
            return Err("Body Play or another Host effect already owns the Host".into());
        }
        owner.validate_browser_mask_show(&window_id, &binding, &request, &show)?;
        let face = owner.local_face_snapshot()?;
        if !equipment.matches(owner.host.current()) {
            return Err("selected speech equipment differs from current Host Boot".into());
        }
        let operation_id = crate::durable_host::fresh_identity("selected-speech", &window_id);
        // Start the worker before moving the Host. Failed spawn leaves it with
        // the owner; failed handoff returns the Host for restoration.
        let (sender, receiver) = mpsc::sync_channel::<Option<StdHost>>(1);
        let control = RunControl::default();
        let worker_control = control.clone();
        let worker_show = show.clone();
        let thread = std::thread::Builder::new()
            .name("conduit-owner-selected-speech".into())
            .spawn(move || {
                let mut host = receiver.recv().ok().flatten()?;
                #[cfg(test)]
                if let Some(gate) = &equipment.before_play {
                    gate.wait();
                    gate.wait();
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    play_selected(&mut host, &face, &worker_show, &equipment, &worker_control)
                }))
                .unwrap_or_else(|_| {
                    Err(SpeechFailure::Failed(
                        "selected speech worker panicked".into(),
                    ))
                });
                Some((host, result))
            })
            .map_err(|error| format!("start selected speech worker: {error}"))?;
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
            return Err("selected speech worker stopped before Host handoff".into());
        }
        self.speech_worker = Some(SpeechWorker {
            operation_id: operation_id.clone(),
            control,
            source_window_id: window_id,
            source_binding: binding,
            source_request: request,
            source_show: show,
            thread,
        });
        self.speech_terminal = None;
        Ok(operation_id)
    }

    pub(in crate::durable_host_control) fn progress_browser_speech(
        &mut self,
    ) -> Result<(), String> {
        if !self
            .speech_worker
            .as_ref()
            .is_some_and(|worker| worker.thread.is_finished())
        {
            return Ok(());
        }
        let worker = self.speech_worker.take().expect("finished speech worker");
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("selected speech lost its installed Body owner".into());
        };
        let (host, result) = worker
            .thread
            .join()
            .map_err(|_| "selected speech worker failed before returning Host".to_string())?
            .ok_or("selected speech worker exited without Host handoff")?;
        owner.host.restore_after_play(host)?;
        let source_current = owner
            .validate_browser_mask_show(
                &worker.source_window_id,
                &worker.source_binding,
                &worker.source_request,
                &worker.source_show,
            )
            .is_ok();
        let terminal = match result {
            Ok(mut receipt) => {
                receipt["operation_id"] = json!(worker.operation_id);
                receipt["source_show_still_current"] = json!(source_current);
                receipt
            }
            Err(failure) => {
                let outcome = failure.outcome();
                json!({"schema":"conduit.body/selected-speech-terminal@1",
                    "operation_id":worker.operation_id, "outcome":outcome,
                    "source_show_id":worker.source_show.show_id.as_str(),
                    "host_id":owner.host.advertisement().host_id.as_str(),
                    "boot_id":owner.host.advertisement().boot_id.as_str(),
                    "offer_generation":owner.host.advertisement().offer_generation.0,
                    "source_show_still_current":source_current, "detail":failure.detail()})
            }
        };
        self.speech_terminal = Some(terminal);
        Ok(())
    }

    pub(in crate::durable_host_control) fn browser_speech_status(
        &mut self,
        operation_id: &str,
    ) -> Result<Value, String> {
        self.progress_browser_speech()?;
        if self
            .speech_worker
            .as_ref()
            .is_some_and(|worker| worker.operation_id == operation_id)
        {
            return Ok(json!({"schema":"conduit.body/selected-speech-status@1",
                "operation_id":operation_id, "state":"running"}));
        }
        self.speech_terminal
            .as_ref()
            .filter(|terminal| terminal["operation_id"] == operation_id)
            .cloned()
            .ok_or_else(|| "unknown selected speech operation".into())
    }

    pub(in crate::durable_host_control) fn stop_browser_speech(
        &mut self,
        operation_id: &str,
    ) -> Result<(), String> {
        self.progress_browser_speech()?;
        let worker = self
            .speech_worker
            .as_ref()
            .filter(|worker| worker.operation_id == operation_id)
            .ok_or("selected speech is not running")?;
        worker
            .control
            .request_stop(RunControlRequestId::new(format!("stop/{operation_id}"))?)
            .map_err(|_| "selected speech stop already requested".into())
    }

    pub(super) fn stop_speech_from_window(&mut self, window_id: &str) {
        if let Some(worker) = self
            .speech_worker
            .as_ref()
            .filter(|worker| worker.source_window_id == window_id)
        {
            if let Ok(request) = RunControlRequestId::new(format!("leave/{}", worker.operation_id))
            {
                let _ = worker.control.request_stop(request);
            }
        }
    }
}

fn play_selected(
    host: &mut StdHost,
    face: &Presentation,
    show: &MaskShow,
    equipment: &AttachedEquipment,
    control: &RunControl,
) -> Result<Value, SpeechFailure> {
    if !equipment.matches(host) {
        return Err(SpeechFailure::Refused(
            "selected speech equipment changed before Play".into(),
        ));
    }
    let offered = host.advertisement().clone();
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).map_err(|error| {
        SpeechFailure::Refused(format!("selected spoken Face refused: {error:?}"))
    })?;
    reader
        .command(face, show, ReaderCommand::ReadAll, 1)
        .map_err(|error| {
            SpeechFailure::Refused(format!("selected spoken read-all refused: {error:?}"))
        })?;
    let mut receipts = Vec::with_capacity(MAXIMUM_BATCHES);
    let mut completed = false;
    for _ in 0..MAXIMUM_BATCHES {
        if control.stop_requested() {
            return Err(SpeechFailure::Cancelled(
                "selected speech stop requested".into(),
            ));
        }
        let Some(batch) = reader.next_batch_with_limits(1, 64).map_err(|error| {
            SpeechFailure::Refused(format!("selected speech batch refused: {error:?}"))
        })?
        else {
            completed = true;
            break;
        };
        let result = execute_spoken_batch_on_attached_host(
            face,
            show,
            &batch,
            &equipment.playback,
            &equipment.authorization,
            control,
            host,
        )
        .map_err(|error| match error {
            SpokenStreamExecutionRefusal::PlaybackPlay { detail, .. } => {
                SpeechFailure::PlayRefused(detail)
            }
            other => SpeechFailure::Refused(format!("selected speaker Play refused: {other:?}")),
        })?;
        let outcome = match &result.outcome {
            SpokenPlaybackOutcome::Completed => "completed",
            SpokenPlaybackOutcome::Cancelled => "cancelled",
            SpokenPlaybackOutcome::Failed => "failed",
        };
        receipts.push(json!({"stream_identity":result.stream_identity,
            "source_segments_sha256":result.source_segments_sha256,
            "plan_id":result.playback_plan_id, "play_id":result.playback_play_id,
            "provider_sha256":result.provider_sha256,
            "speaker_blocks_committed":result.playback.metrics.blocks_committed,
            "outcome":outcome}));
        let terminal = reader
            .acknowledge_batch(result.delivery())
            .map_err(|error| {
                SpeechFailure::Failed(format!("selected speaker receipt refused: {error:?}"))
            })?;
        if outcome != "completed" {
            return Err(match &result.outcome {
                SpokenPlaybackOutcome::Cancelled => {
                    SpeechFailure::Cancelled("selected speaker Play cancelled".into())
                }
                _ => SpeechFailure::Failed("selected speaker Play failed".into()),
            });
        }
        if terminal.is_some() {
            completed = true;
            break;
        }
    }
    if !completed || receipts.is_empty() {
        return Err(SpeechFailure::Failed(
            "selected speech ended without bounded complete readout".into(),
        ));
    }
    Ok(json!({"schema":"conduit.body/selected-speech-terminal@1",
        "outcome":"completed", "face_id":face.identity.as_str(),
        "face_revision":face.revision, "source_show_id":show.show_id.as_str(),
        "host_id":offered.host_id.as_str(), "boot_id":offered.boot_id.as_str(),
        "offer_generation":offered.offer_generation.0,
        "provider_sha256":equipment.provider_sha256,
        "selected_resource_pool_id":equipment.playback.pool_id().as_str(),
        "authority_grant_id":equipment.authorization.grant_id(), "batches":receipts}))
}
