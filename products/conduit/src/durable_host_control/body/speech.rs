//! One acknowledged browser or direct Show read through the owner's sole Host.
use super::{DurableHostRuntime, HostSource};
use crate::durable_host::selected_speech::AttachedEquipment;
use conduit_core::LinkBindingId;
use conduit_presentation::{MaskShow, OwnerFaceSnapshotRequest, Presentation};
use conduit_std_host::{RunControl, RunControlRequestId, StdHost};
use serde_json::{json, Value};
use std::{sync::mpsc, thread::JoinHandle};

// The retained WAV root admits 64 exact per-Play artifacts. The browser path
// keeps its one-segment batches; the direct owner Mask plays its Show's brief
// opening through the separate accepted-wording path.
const MAXIMUM_BATCHES: usize = 64;

#[path = "speech/execution.rs"]
mod execution;
use execution::{play_selected, ReadingScope};

#[cfg(all(test, unix))]
#[path = "speech/tests.rs"]
mod tests;

pub(super) enum SpeechFailure {
    Refused(String),
    Cancelled(String),
    PlayRefused(String),
    Failed(String),
    Partial {
        cause: Box<SpeechFailure>,
        completed_batches: Vec<Value>,
    },
}

impl SpeechFailure {
    pub(super) fn outcome(&self) -> &'static str {
        match self {
            Self::Refused(_) => "refused",
            Self::Cancelled(_) => "cancelled",
            Self::PlayRefused(_) => "play-refused-unclassified",
            Self::Failed(_) => "failed",
            Self::Partial { cause, .. } => cause.outcome(),
        }
    }

    pub(super) fn completed_batches(&self) -> &[Value] {
        match self {
            Self::Partial {
                completed_batches, ..
            } => completed_batches,
            _ => &[],
        }
    }

    pub(super) fn detail(self) -> String {
        match self {
            Self::Refused(detail)
            | Self::Cancelled(detail)
            | Self::PlayRefused(detail)
            | Self::Failed(detail) => detail,
            Self::Partial { cause, .. } => cause.detail(),
        }
    }
}

struct BrowserSpeechStart {
    window_id: String,
    binding: LinkBindingId,
    request: OwnerFaceSnapshotRequest,
    show: MaskShow,
}

enum SpeechSource {
    Browser {
        window_id: String,
        binding: LinkBindingId,
        request: Box<OwnerFaceSnapshotRequest>,
    },
    Direct {
        seal: Box<conduit_presentation::LocalOwnerMaskRouteSeal>,
    },
}
impl SpeechSource {
    fn scope(&self) -> ReadingScope {
        match self {
            Self::Browser { .. } => ReadingScope::WholeFace,
            Self::Direct { .. } => ReadingScope::RemainingItems,
        }
    }
    fn identity(&self) -> &str {
        match self {
            Self::Browser { window_id, .. } => window_id,
            Self::Direct { seal } => seal.route_plan_id.as_str(),
        }
    }
    fn is_direct(&self) -> bool {
        matches!(self, Self::Direct { .. })
    }
    fn validate(
        &self,
        owner: &mut crate::durable_host::owner::Owner,
        show: &MaskShow,
    ) -> Result<(), String> {
        match self {
            Self::Browser {
                window_id,
                binding,
                request,
            } => owner.validate_browser_mask_show(window_id, binding, request, show),
            Self::Direct { seal } => owner.validate_selected_direct_spoken_read(seal, show),
        }
    }
}

pub(in crate::durable_host_control) struct SpeechWorker {
    operation_id: String,
    control: RunControl,
    source: SpeechSource,
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
        self.start_spoken_reading(Some(BrowserSpeechStart {
            window_id,
            binding,
            request,
            show,
        }))
    }

    pub(in crate::durable_host_control) fn start_direct_remaining_speech(
        &mut self,
    ) -> Result<String, String> {
        self.start_spoken_reading(None)
    }

    fn start_spoken_reading(
        &mut self,
        browser: Option<BrowserSpeechStart>,
    ) -> Result<String, String> {
        self.progress_browser_speech()?;
        self.progress_owner_spoken()?;
        if self.speech_worker.is_some() || self.owner_spoken_worker.is_some() {
            return Err("selected speech is already running".into());
        }
        #[cfg(unix)]
        if super::super::terminal_attach::is_attached(self) {
            return Err("terminal attachment must detach before selected speech".into());
        }
        let equipment = self.selected_speech_equipment.clone();
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
        let (source, face, show) = if let Some(input) = browser {
            owner.validate_browser_mask_show(
                &input.window_id,
                &input.binding,
                &input.request,
                &input.show,
            )?;
            (
                SpeechSource::Browser {
                    window_id: input.window_id,
                    binding: input.binding,
                    request: Box::new(input.request),
                },
                owner.local_face_snapshot()?,
                input.show,
            )
        } else {
            let (seal, face, show) = owner.prepare_selected_direct_spoken_read()?;
            (
                SpeechSource::Direct {
                    seal: Box::new(seal),
                },
                face,
                show,
            )
        };
        let scope = source.scope();
        if equipment.as_ref().map_or_else(
            || !owner.host.current().spoken_artifact_only_route_is_current(),
            |equipment| !equipment.matches(owner.host.current()),
        ) {
            return Err(
                "installed Host has no selected speech equipment matching its current Boot".into(),
            );
        }
        let operation_id =
            crate::durable_host::fresh_identity("selected-speech", source.identity());
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
                if let Some(gate) = equipment
                    .as_ref()
                    .and_then(|equipment| equipment.before_play.as_ref())
                {
                    gate.wait();
                    gate.wait();
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    play_selected(
                        &mut host,
                        &face,
                        &worker_show,
                        equipment.as_ref(),
                        &worker_control,
                        1,
                        MAXIMUM_BATCHES,
                        scope,
                    )
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
            source,
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
        let source_current = worker.source.validate(owner, &worker.source_show).is_ok();
        let source_mask = if worker.source.is_direct() {
            "direct"
        } else {
            "browser"
        };
        let terminal = match result {
            Ok(mut receipt) => {
                receipt["operation_id"] = json!(worker.operation_id);
                receipt["source_show_still_current"] = json!(source_current);
                receipt["source_mask"] = json!(source_mask);
                receipt
            }
            Err(failure) => {
                let outcome = failure.outcome();
                let completed_batches = failure.completed_batches().to_vec();
                json!({"schema":"conduit.body/selected-speech-terminal@1",
                    "operation_id":worker.operation_id, "outcome":outcome,
                    "source_show_id":worker.source_show.show_id.as_str(),
                    "host_id":owner.host.advertisement().host_id.as_str(),
                    "boot_id":owner.host.advertisement().boot_id.as_str(),
                    "offer_generation":owner.host.advertisement().offer_generation.0,
                    "source_show_still_current":source_current,
                    "source_mask":source_mask, "reader_scope":worker.source.scope().name(),
                    "completed_batch_count":completed_batches.len(),
                    "batches":completed_batches,
                    "detail":failure.detail()})
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

    fn is_direct_reading_operation(&self, operation_id: &str) -> bool {
        self.speech_worker
            .as_ref()
            .is_some_and(|worker| worker.operation_id == operation_id && worker.source.is_direct())
            || self.speech_terminal.as_ref().is_some_and(|terminal| {
                terminal["operation_id"] == operation_id && terminal["source_mask"] == "direct"
            })
    }

    pub(in crate::durable_host_control) fn direct_spoken_or_reading_status(
        &mut self,
        operation_id: &str,
    ) -> Result<Value, String> {
        self.progress_browser_speech()?;
        if self.is_direct_reading_operation(operation_id) {
            self.browser_speech_status(operation_id)
        } else {
            self.direct_spoken_status(operation_id)
        }
    }

    pub(in crate::durable_host_control) fn stop_direct_spoken_or_reading(
        &mut self,
        operation_id: &str,
    ) -> Result<(), String> {
        self.progress_browser_speech()?;
        if self.is_direct_reading_operation(operation_id) {
            self.stop_browser_speech(operation_id)
        } else {
            self.stop_direct_spoken(operation_id)
        }
    }

    pub(super) fn stop_speech_from_window(&mut self, window_id: &str) {
        if let Some(worker) = self
            .speech_worker
            .as_ref()
            .filter(|worker| matches!(&worker.source, SpeechSource::Browser { window_id: source, .. } if source == window_id))
        {
            if let Ok(request) = RunControlRequestId::new(format!("leave/{}", worker.operation_id))
            {
                let _ = worker.control.request_stop(request);
            }
        }
    }
}
