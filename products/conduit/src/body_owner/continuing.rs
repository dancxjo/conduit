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
#[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
#[path = "continuing/native.rs"]
pub(super) mod native;
#[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
pub(super) use native::SourceSelection;
#[cfg(all(
    feature = "installed-environmental-sources",
    feature = "installed-body-storage",
    target_os = "linux"
))]
#[path = "continuing/catalog_run.rs"]
mod catalog_run;
#[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
#[path = "continuing/fore_refusal.rs"]
mod fore_refusal;
#[cfg(all(
    feature = "installed-environmental-sources",
    feature = "installed-body-storage",
    target_os = "linux"
))]
#[path = "continuing/recording_run.rs"]
mod recording_run;
#[cfg(all(
    feature = "installed-environmental-sources",
    feature = "installed-body-storage",
    target_os = "linux"
))]
#[path = "continuing/recovered_read_run.rs"]
pub(super) mod recovered_read_run;
#[cfg(all(
    feature = "installed-environmental-sources",
    feature = "installed-body-storage",
    target_os = "linux"
))]
#[path = "continuing/recovery_run.rs"]
pub(super) mod recovery_run;
#[cfg(all(
    feature = "installed-environmental-sources",
    feature = "installed-body-storage",
    target_os = "linux"
))]
#[path = "continuing/source_recording_stage.rs"]
pub(super) mod source_recording_stage;
#[path = "continuing/start.rs"]
mod start;
struct WorkerOutcome {
    #[cfg(all(
        feature = "installed-environmental-sources",
        feature = "installed-body-storage",
        target_os = "linux"
    ))]
    recording: Option<recording_run::RecordingRun>,
    host: StdHost,
    result: Result<BodyRunReport, String>,
    output: Vec<u8>,
    #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
    native: Option<native::NativeRun>,
}

pub(crate) struct RunWorker {
    control: RunControl,
    started: Receiver<(BodyPlayIdentity, Wake)>,
    acknowledge: Option<SyncSender<Result<(), String>>>,
    thread: Option<JoinHandle<WorkerOutcome>>,
    #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
    live_fore: Option<Receiver<native::LiveFore>>,
    #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
    fore_refusal: Option<fore_refusal::ForeRefusal>,
    #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
    returned_equipment:
        Option<crate::durable_host::selected_environmental_sources::AttachedEquipment>,
}

impl Owner {
    #[cfg(all(
        feature = "installed-environmental-sources",
        feature = "installed-body-storage",
        target_os = "linux"
    ))]
    pub(crate) fn current_body_id(&self) -> &conduit_body::BodyId {
        &self.session.evidence().body_id
    }

    pub(crate) fn current_play_id(&self) -> Option<&conduit_core::ActivePlayId> {
        self.session
            .realization()
            .and_then(|realization| realization.play.as_ref())
            .map(|play| &play.active_play_id)
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
        #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
        accepted_fore: Option<native::AcceptedSourceFore>,
        #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
        fore_refusal: Option<&fore_refusal::ForeRefusal>,
        #[cfg(all(
            feature = "installed-environmental-sources",
            feature = "installed-body-storage",
            target_os = "linux"
        ))]
        publication: Option<serde_json::Value>,
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
                    "native_failure":report.native_failure,
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
        #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
        let receipt = {
            let mut receipt = receipt;
            if let Some(refusal) = fore_refusal {
                receipt["source_fore_refusal"] = refusal.diagnostics();
            }
            #[cfg(feature = "installed-body-storage")]
            if let Some(stage) = self.source_recording_stage.as_mut() {
                // Worker cleanup has returned its Host before encoding begins.
                // A codec refusal retains all original accepted bytes.
                match stage
                    .prepare_checkpoint_input()
                    .and_then(|()| stage.prepared_input_metadata())
                {
                    Ok(metadata) => {
                        receipt["source_recording_prepared"] =
                            metadata.unwrap_or(serde_json::Value::Null)
                    }
                    Err(error) => receipt["source_recording_preparation_refusal"] = error.into(),
                }
                receipt["source_recording_pending"] = serde_json::json!({
                    "custody": "volatile-accepted-fore-not-durable-recording",
                    "count": stage.len(),
                    "capacity": source_recording_stage::CAPACITY,
                    "source_selection": stage.scope().map(|scope| &*scope.selection),
                    "native_play": stage.scope().map(|scope| &scope.native_play),
                    "producer_retirement_authority": false,
                });
            }
            if let Some(accepted) = accepted_fore {
                let value = accepted.value;
                receipt["accepted_source_fore"] = serde_json::json!({
                    "custody": "accepted-typed-fore-not-durable-recording",
                    "source_selection": &*accepted.selection, "native_play": accepted.native_play,
                    "front_port_id": value.front_port_id, "track": value.track,
                    "value_kind": value.value_kind, "sequence": value.sequence,
                    "canonical_bytes": value.bytes.as_slice(),
                });
            }
            receipt
        };
        #[cfg(all(
            feature = "installed-environmental-sources",
            feature = "installed-body-storage",
            target_os = "linux"
        ))]
        let receipt = {
            let mut receipt = receipt;
            if let Some(publication) = publication {
                let field = if publication.get("read_completed").is_some() {
                    "source_recovered_read"
                } else if publication.get("catalog_reference_recovered").is_some() {
                    "source_catalog_recovery"
                } else if publication.get("catalog_checkpoint_observed").is_some() {
                    "source_catalog"
                } else if publication.get("snapshot_readback_completed").is_some() {
                    "source_readback"
                } else {
                    "source_publication"
                };
                receipt[field] = publication;
            }
            receipt
        };
        self.retain_execution(root, next, receipt)?;
        Ok(())
    }
}

impl RunWorker {
    pub(crate) fn progress(&mut self, owner: &mut Owner, root: &Path) -> Result<bool, String> {
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
        #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
        if self.fore_refusal.is_none() {
            if let Some(receiver) = &self.live_fore {
                while let Ok(value) = receiver.try_recv() {
                    if let Err(error) = owner.accept_live_source_fore(&value) {
                        self.fore_refusal = Some(fore_refusal::ForeRefusal::new(value, error));
                        // Retain the refused Fore and original worker until its
                        // cancellation and cleanup return the affine Host.
                        // An existing stop request already provides that path.
                        let _ = self.control.request_stop(RunControlRequestId::new(
                            "owner/source-consumer-refused",
                        )?);
                        break;
                    }
                }
            }
        }
        if !self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            return Ok(false);
        }
        let outcome = self
            .thread
            .take()
            .expect("finished worker has a join handle")
            .join()
            .map_err(|_| "Body Play worker panicked; owner service must restart".to_string())?;
        self.finish_joined(owner, root, outcome)
    }

    // Called only after join has established producer quiescence. The final
    // one-slot Fore may have arrived after the pre-join poll observed Empty.
    fn finish_joined(
        &mut self,
        owner: &mut Owner,
        root: &Path,
        outcome: WorkerOutcome,
    ) -> Result<bool, String> {
        let WorkerOutcome {
            #[cfg(all(
                feature = "installed-environmental-sources",
                feature = "installed-body-storage",
                target_os = "linux"
            ))]
            recording,
            host,
            result,
            output,
            #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
            mut native,
        } = outcome;
        #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
        {
            self.returned_equipment = native.as_mut().and_then(|run| run.equipment.take());
        }
        owner.host.restore_after_play(host)?;
        #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
        if self.fore_refusal.is_none() {
            if let Some(receiver) = self.live_fore.as_ref() {
                while let Ok(value) = receiver.try_recv() {
                    if let Err(error) = owner.accept_live_source_fore(&value) {
                        self.fore_refusal = Some(fore_refusal::ForeRefusal::new(value, error));
                        // The producer has joined: retain refusal without
                        // inventing another cancellation/stop request.
                        break;
                    }
                }
            }
        }
        #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
        if let Some(refusal) = self.fore_refusal.as_mut() {
            if let Some(receiver) = self.live_fore.as_ref() {
                refusal.capture_trailing(receiver);
            }
        }
        #[cfg(all(
            feature = "installed-environmental-sources",
            feature = "installed-body-storage",
            target_os = "linux"
        ))]
        let mut publication = recording.as_ref().map(|r| r.metadata(result.as_ref().ok()));
        #[cfg(all(
            feature = "installed-environmental-sources",
            feature = "installed-body-storage",
            target_os = "linux"
        ))]
        let recovered_content = recording
            .as_ref()
            .map(|r| r.recovered_content(result.as_ref().ok()))
            .transpose();
        #[cfg(all(
            feature = "installed-environmental-sources",
            feature = "installed-body-storage",
            target_os = "linux"
        ))]
        if let (Some(metadata), Err(error)) = (&mut publication, &recovered_content) {
            metadata["content_verification_refusal"] = error.clone().into();
        }
        #[cfg(all(
            feature = "installed-environmental-sources",
            feature = "installed-body-storage",
            target_os = "linux"
        ))]
        let recovered = recording
            .as_ref()
            .map(|r| r.recovered_catalog(result.as_ref().ok()))
            .transpose();
        #[cfg(all(
            feature = "installed-environmental-sources",
            feature = "installed-body-storage",
            target_os = "linux"
        ))]
        let verified = recording
            .as_ref()
            .and_then(|r| r.verified_snapshot(result.as_ref().ok()));
        owner.retain_finished(
            root,
            result,
            output,
            #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
            native
                .map(native::NativeRun::accepted)
                .transpose()?
                .flatten(),
            #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
            self.fore_refusal.as_ref(),
            #[cfg(all(
                feature = "installed-environmental-sources",
                feature = "installed-body-storage",
                target_os = "linux"
            ))]
            publication,
        )?;
        #[cfg(all(
            feature = "installed-environmental-sources",
            feature = "installed-body-storage",
            target_os = "linux"
        ))]
        if let Some(verified) = verified {
            // retain_finished first checks the exact issued Play and durably
            // records its terminal lifecycle. Only then can readback be used
            // as private input custody for a later catalog checkpoint.
            owner
                .source_recording_stage
                .as_mut()
                .ok_or("verified Source stage vanished")?
                .accept_verified_snapshot(verified)?;
        }
        #[cfg(all(
            feature = "installed-environmental-sources",
            feature = "installed-body-storage",
            target_os = "linux"
        ))]
        if let Some(Some(recovered)) = recovered? {
            if recovered.binding().body != owner.session.evidence().body_id.as_str() {
                return Err("recovered catalog belongs to another Body".into());
            }
            owner.recovered_source_catalog = Some(recovered);
        }
        #[cfg(all(
            feature = "installed-environmental-sources",
            feature = "installed-body-storage",
            target_os = "linux"
        ))]
        if let Some(Some(content)) = recovered_content? {
            match content {
                recovered_read_run::RecoveredReadResult::Catalog(contents) => {
                    if owner
                        .recovered_source_catalog
                        .as_ref()
                        .is_none_or(|r| r.reference() != contents.catalog_reference())
                    {
                        return Err("read catalog differs from recovered Body State".into());
                    }
                    owner.recovered_source_catalog_contents = Some(contents);
                    owner.recovered_source_chunk = None;
                }
                recovered_read_run::RecoveredReadResult::Chunk(chunk) => {
                    owner
                        .recovered_source_catalog_contents
                        .as_mut()
                        .ok_or("recovered catalog contents vanished")?
                        .accept_chunk(&chunk)?;
                    owner.recovered_source_chunk = Some(chunk);
                }
            }
        }
        Ok(true)
    }

    #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
    pub(crate) fn take_returned_equipment(
        &mut self,
    ) -> Option<crate::durable_host::selected_environmental_sources::AttachedEquipment> {
        self.returned_equipment.take()
    }

    pub(crate) fn request_lull(&self) -> Result<(), String> {
        self.control
            .request_stop(RunControlRequestId::new("owner/service-lull")?)
            .map_err(|_| "Body Play already has a stop request".into())
    }
}

#[cfg(all(test, feature = "installed-environmental-sources", target_os = "linux"))]
#[path = "continuing/fore_refusal_worker_tests.rs"]
mod fore_refusal_worker_tests;

#[cfg(all(test, feature = "installed-environmental-sources", target_os = "linux"))]
#[path = "continuing/final_fore_tests.rs"]
mod final_fore_tests;
