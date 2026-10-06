//! One selected browser-source readout on the installed owner's actual std Host.
//! Preparation and Plays live on a worker; local control calls stay short.
use super::{DurableHostRuntime, HostSource};
use conduit_core::LinkBindingId;
use conduit_presentation::{MaskShow, OwnerFaceSnapshotRequest, Presentation};
use conduit_std_host::{
    hosted_audio::{
        discover_alsa_playback, ExplicitPlaybackAuthorization, HostedPlaybackSelection,
    },
    hosted_speech_synthesis::EspeakDiscovery,
    spoken_face_mask::{ReaderCommand, SpokenFaceSession},
    spoken_face_stream_execution::{execute_spoken_batch_on_attached_host, SpokenPlaybackOutcome},
    RunControl, RunControlRequestId, StdHost,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{path::PathBuf, thread::JoinHandle, time::Duration};

const MAXIMUM_BATCHES: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SpeechSelection {
    pub card_id: String,
    pub device: u32,
    pub executable: PathBuf,
    pub data_root: PathBuf,
    pub voice: String,
    pub engine_dependencies: Vec<PathBuf>,
    pub grant_id: String,
}

impl SpeechSelection {
    fn validate(&self) -> Result<(), String> {
        if self.card_id.is_empty()
            || self.card_id.len() > 128
            || self.voice.is_empty()
            || self.voice.len() > 64
            || self.grant_id.is_empty()
            || self.grant_id.len() > 128
            || self.engine_dependencies.is_empty()
            || self.engine_dependencies.len() > 16
            || self.executable.as_os_str().len() > 4096
            || self.data_root.as_os_str().len() > 4096
            || self
                .engine_dependencies
                .iter()
                .any(|path| path.as_os_str().len() > 4096)
        {
            return Err("selected speech inputs exceed their finite bounds".into());
        }
        Ok(())
    }
}

pub(super) struct SpeechWorker {
    pub operation_id: String,
    pub control: RunControl,
    pub source_window_id: String,
    pub source_binding: LinkBindingId,
    pub source_request: OwnerFaceSnapshotRequest,
    pub source_show: MaskShow,
    pub thread: JoinHandle<(StdHost, Result<Value, String>)>,
}

impl DurableHostRuntime {
    pub(super) fn start_browser_speech(
        &mut self,
        window_id: String,
        binding: LinkBindingId,
        request: OwnerFaceSnapshotRequest,
        show: MaskShow,
        selection: SpeechSelection,
    ) -> Result<String, String> {
        selection.validate()?;
        if self.speech_worker.is_some() {
            return Err("selected speech is already running".into());
        }
        #[cfg(unix)]
        if super::super::terminal_attach::is_attached(self) {
            return Err("terminal attachment must detach before selected speech".into());
        }
        let HostSource::Body {
            owner,
            running: None,
            ..
        } = &mut self.host
        else {
            return Err("selected speech requires an idle installed Body owner".into());
        };
        if owner.host.is_playing() || owner.session.realization().is_some() {
            return Err("Body Play or another Host effect already owns the Host".into());
        }
        owner.validate_browser_mask_show(&window_id, &binding, &request, &show)?;
        let face = owner.local_face_snapshot()?;
        let host = owner.host.take_for_play()?;
        let operation_id = crate::durable_host::fresh_identity("selected-speech", "browser-show");
        let control = RunControl::default();
        let worker_control = control.clone();
        let worker_show = show.clone();
        let thread = std::thread::Builder::new()
            .name("conduit-owner-selected-speech".into())
            .spawn(move || {
                let mut host = host;
                let result =
                    play_selected(&mut host, &face, &worker_show, selection, &worker_control);
                (host, result)
            })
            .expect("failure to launch sole selected speech worker stops owner service");
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

    pub(super) fn progress_browser_speech(&mut self) -> Result<(), String> {
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
        let (host, result) = worker.thread.join().map_err(|_| {
            "selected speech worker panicked; owner service must restart".to_string()
        })?;
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
            Err(error) => json!({"schema":"conduit.body/selected-speech-terminal@1",
                "operation_id":worker.operation_id, "outcome":"refused-or-failed",
                "source_show_still_current":source_current, "detail":error}),
        };
        self.speech_terminal = Some(terminal);
        Ok(())
    }

    pub(super) fn browser_speech_status(&mut self, operation_id: &str) -> Result<Value, String> {
        self.progress_browser_speech()?;
        if let Some(worker) = &self.speech_worker {
            if worker.operation_id == operation_id {
                return Ok(json!({"schema":"conduit.body/selected-speech-status@1",
                    "operation_id":operation_id, "state":"running"}));
            }
        }
        self.speech_terminal
            .as_ref()
            .filter(|terminal| terminal["operation_id"] == operation_id)
            .cloned()
            .ok_or_else(|| "unknown selected speech operation".into())
    }

    pub(super) fn stop_browser_speech(&mut self, operation_id: &str) -> Result<(), String> {
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
}

fn play_selected(
    host: &mut StdHost,
    face: &Presentation,
    show: &MaskShow,
    selection: SpeechSelection,
    control: &RunControl,
) -> Result<Value, String> {
    let offered = host.advertisement().clone();
    let observations = discover_alsa_playback()
        .map_err(|error| format!("selected speaker discovery failed: {error}"))?;
    let mut selected = observations
        .into_iter()
        .filter(|item| item.card_id == selection.card_id && item.device == selection.device);
    let observation = selected.next().ok_or("selected speaker is unavailable")?;
    if selected.next().is_some() {
        return Err("selected speaker is ambiguous".into());
    }
    let playback = HostedPlaybackSelection::from_observation(
        observation,
        offered.boot_id.clone(),
        offered.offer_generation,
    )
    .with_bounded_speech_queue();
    let authorization = ExplicitPlaybackAuthorization::new(&selection.grant_id)?;
    let discovery = EspeakDiscovery::inspect(
        &selection.executable,
        &selection.data_root,
        &selection.voice,
        &selection.engine_dependencies,
    )
    .map_err(|error| format!("selected speech provider refused: {error:?}"))?;
    if control.stop_requested() {
        return Err("selected speech cancelled during preparation".into());
    }
    let provider_sha256 = discovery.provider_sha256.clone();
    let adapter = discovery
        .initialize(
            offered.host_id.clone(),
            offered.boot_id.clone(),
            offered.offer_generation,
            "grant/owner-browser-selected-speech".into(),
            Duration::from_secs(30),
        )
        .map_err(|error| format!("initialize selected speech: {error:?}"))?;
    if host.playback.is_none() {
        host.attach_selected_playback(playback.clone())?;
    }
    if host.speech_synthesis.is_none() {
        host.attach_espeak_speech_for_selected_playback(adapter)?;
    }
    if host.playback.as_ref() != Some(&playback)
        || host
            .speech_synthesis
            .as_ref()
            .is_none_or(|provider| provider.provider_sha256() != provider_sha256)
    {
        return Err("retained Host has a different selected speaker or provider".into());
    }
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone())
        .map_err(|error| format!("selected spoken Face refused: {error:?}"))?;
    reader
        .command(face, show, ReaderCommand::ReadAll, 1)
        .map_err(|error| format!("selected spoken read-all refused: {error:?}"))?;
    let mut receipts = Vec::new();
    for _ in 0..MAXIMUM_BATCHES {
        if control.stop_requested() {
            return Err("selected speech stop requested".into());
        }
        let Some(batch) = reader
            .next_batch_with_limits(1, 64)
            .map_err(|error| format!("selected speech batch refused: {error:?}"))?
        else {
            break;
        };
        let result = execute_spoken_batch_on_attached_host(
            face,
            show,
            &batch,
            &playback,
            &authorization,
            control,
            host,
        )
        .map_err(|error| format!("selected speaker Play refused: {error:?}"))?;
        let outcome = match result.outcome {
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
            .map_err(|error| format!("selected speaker receipt refused: {error:?}"))?;
        if outcome != "completed" {
            return Err(format!("selected speaker Play {outcome}"));
        }
        if terminal.is_some() {
            break;
        }
    }
    if receipts.is_empty() || receipts.len() == MAXIMUM_BATCHES {
        return Err("selected speech ended without bounded complete readout".into());
    }
    Ok(json!({"schema":"conduit.body/selected-speech-terminal@1",
        "outcome":"completed", "face_id":face.identity.as_str(),
        "face_revision":face.revision, "source_show_id":show.show_id.as_str(),
        "host_id":offered.host_id.as_str(), "boot_id":offered.boot_id.as_str(),
        "offer_generation":offered.offer_generation.0,
        "provider_sha256":provider_sha256,
        "selected_resource_pool_id":playback.pool_id().as_str(),
        "authority_grant_id":authorization.grant_id(), "batches":receipts}))
}
