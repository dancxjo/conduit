//! Explicit authenticated recorder requests; authority is caller supplied.
use super::DurableHostRuntime;
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecordingRequest {
    pub(crate) operation: RecordingOperation,
    pub(crate) maximum_millis: u64,
    pub(crate) preparation_limit_bytes: u64,
    pub(crate) expected_body_id: String,
    pub(crate) expected_boot_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum RecordingOperation {
    Publish {
        authority: conduit_core::AuthorityGrant,
    },
    Readback {
        authority: conduit_core::AuthorityGrant,
    },
    Catalog {
        authority: conduit_core::AuthorityGrant,
    },
    RecoverCatalog {},
    ReadRecoveredCatalog {
        authority: conduit_core::AuthorityGrant,
    },
    ReadRecoveredChunk {
        authority: conduit_core::AuthorityGrant,
        index: usize,
    },
}
impl DurableHostRuntime {
    #[cfg(all(
        feature = "installed-environmental-sources",
        feature = "installed-body-storage",
        target_os = "linux"
    ))]
    pub(super) fn start_owned_recording(
        &mut self,
        request: RecordingRequest,
    ) -> Result<(), String> {
        if self.environmental_worker_active() {
            return Err("native-source-active".into());
        }
        #[cfg(unix)]
        if super::terminal_attach::is_attached(self) {
            return Err("terminal-attachment-must-detach-before-start".into());
        }
        let super::body::HostSource::Body {
            owner,
            root,
            running,
        } = &mut self.host
        else {
            return Err("installed Host does not own a live Body session".into());
        };
        if running.is_some() {
            return Err("Body Play is already running".into());
        }
        let ad = owner.host.advertisement();
        if request.expected_body_id != owner.current_body_id().as_str()
            || request.expected_boot_id != ad.boot_id.as_str()
        {
            return Err("recording request differs from the exact current Body/Boot".into());
        }
        if !(1..=900000).contains(&request.maximum_millis) {
            return Err("recording duration must be 1..900000 milliseconds".into());
        }
        let selected = owner
            .host
            .current()
            .selected_body_storage()
            .ok_or("recording requires selected Body storage custody")?;
        if request.preparation_limit_bytes == 0
            || request.preparation_limit_bytes > selected.bounds().preparation_bytes
        {
            return Err("recording preparation limit exceeds selected finite capacity".into());
        }
        if matches!(&request.operation,RecordingOperation::ReadRecoveredChunk{index,..} if *index>=16)
        {
            return Err("recording chunk index exceeds the finite catalog profile".into());
        }
        let previous_generation = ad.offer_generation;
        // Never take selected_environmental_equipment: the single ordinary
        // General worker owns only this operation's original Host custody.
        let result = match request.operation {
            RecordingOperation::Publish { authority } => owner.start_service_source_publication(
                root,
                request.maximum_millis,
                &authority,
                request.preparation_limit_bytes,
            ),
            RecordingOperation::Readback { authority } => owner.start_service_source_readback(
                root,
                request.maximum_millis,
                &authority,
                request.preparation_limit_bytes,
            ),
            RecordingOperation::Catalog { authority } => owner.start_service_source_catalog(
                root,
                request.maximum_millis,
                &authority,
                request.preparation_limit_bytes,
            ),
            RecordingOperation::RecoverCatalog {} => owner.start_service_source_catalog_recovery(
                root,
                request.maximum_millis,
                request.preparation_limit_bytes,
            ),
            RecordingOperation::ReadRecoveredCatalog { authority } => owner
                .start_service_recovered_source_read(
                    root,
                    request.maximum_millis,
                    &authority,
                    None,
                    request.preparation_limit_bytes,
                ),
            RecordingOperation::ReadRecoveredChunk { authority, index } => owner
                .start_service_recovered_source_read(
                    root,
                    request.maximum_millis,
                    &authority,
                    Some(index),
                    request.preparation_limit_bytes,
                ),
        };
        // Install a successfully created worker before any fallible marker IO;
        // failure after preparation is not rollback and cannot orphan a worker.
        let preparation_error = match result {
            Ok(worker) => {
                *running = Some(super::body_run::OwnedRunWorker::General(worker));
                None
            }
            Err(error) => Some(error),
        };
        let current = owner.host.advertisement();
        if current.offer_generation != previous_generation {
            if let Err(error) = crate::durable_host::refresh_offer_generation(
                root,
                &current.host_id,
                &current.boot_id,
                current.offer_generation,
            ) {
                return Err(match preparation_error {
                    Some(prior) => {
                        format!("{prior}; observable generation refresh failed: {error}")
                    }
                    None => format!(
                        "recording worker retained; observable generation refresh failed: {error}"
                    ),
                });
            }
        }
        preparation_error.map_or(Ok(()), Err)
    }
    #[cfg(not(all(
        feature = "installed-environmental-sources",
        feature = "installed-body-storage",
        target_os = "linux"
    )))]
    pub(super) fn start_owned_recording(
        &mut self,
        _request: RecordingRequest,
    ) -> Result<(), String> {
        Err("installed recording requires installed-environmental-sources and installed-body-storage on Linux".into())
    }
}
#[cfg(unix)]
pub(crate) fn recording(state_dir: &Path, request: RecordingRequest) -> Result<(), String> {
    let expected_millis = request.maximum_millis;
    match super::body::call(
        state_dir,
        super::Request::BodyRecording {
            protocol: super::PROTOCOL,
            token: super::body::token(state_dir)?,
            request: Box::new(request),
        },
    )? {
        super::Response::BodyRunRequested {
            protocol: super::PROTOCOL,
            maximum_millis,
        } if maximum_millis == expected_millis => {
            println!(
                "{}",
                serde_json::json!({"schema":"conduit.body/service-run-requested@1","maximum_millis":maximum_millis,"play_state":"preparing"})
            );
            Ok(())
        }
        super::Response::Refused { code, .. } => {
            Err(format!("Body owner refused recording: {code}"))
        }
        _ => Err("Body owner returned the wrong recording response".into()),
    }
}
#[cfg(not(unix))]
pub(crate) fn recording(_state_dir: &Path, _request: RecordingRequest) -> Result<(), String> {
    Err("installed recording control requires Unix local transport".into())
}
#[cfg(test)]
#[path = "body_recording/tests.rs"]
mod tests;
