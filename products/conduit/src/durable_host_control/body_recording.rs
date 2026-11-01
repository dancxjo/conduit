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
        #[serde(deserialize_with = "deserialize_authority")]
        authority: conduit_core::AuthorityGrant,
    },
    Readback {
        #[serde(deserialize_with = "deserialize_authority")]
        authority: conduit_core::AuthorityGrant,
    },
    Catalog {
        #[serde(deserialize_with = "deserialize_authority")]
        authority: conduit_core::AuthorityGrant,
    },
    RecoverCatalog {},
    NextChunk {
        expected_index: u8,
        expected_canonical_sha256: [u8; 32],
    },
    ReadRecoveredCatalog {
        #[serde(deserialize_with = "deserialize_authority")]
        authority: conduit_core::AuthorityGrant,
    },
    ReadRecoveredChunk {
        #[serde(deserialize_with = "deserialize_authority")]
        authority: conduit_core::AuthorityGrant,
        index: usize,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectedAuthority {
    grant_id: conduit_core::AuthorityGrantId,
    contract_id: conduit_core::AuthorityContractId,
    host_call_contract_id: conduit_core::HostCallContractId,
    subject_kind: conduit_core::KindId,
    host_id: conduit_core::HostId,
    boot_id: conduit_core::BootId,
    capability_id: conduit_core::CapabilityId,
}
impl From<SelectedAuthority> for conduit_core::AuthorityGrant {
    fn from(value: SelectedAuthority) -> Self {
        Self {
            grant_id: value.grant_id,
            contract_id: value.contract_id,
            host_call_contract_id: value.host_call_contract_id,
            subject_kind: value.subject_kind,
            host_id: value.host_id,
            boot_id: value.boot_id,
            capability_id: value.capability_id,
        }
    }
}
fn deserialize_authority<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<conduit_core::AuthorityGrant, D::Error> {
    SelectedAuthority::deserialize(decoder).map(Into::into)
}
/// Decode only an explicitly selected finite grant, never issue authority.
pub(crate) fn decode_selected_authority(
    bytes: &[u8],
) -> Result<conduit_core::AuthorityGrant, String> {
    if bytes.is_empty() || bytes.len() > 16 * 1024 {
        return Err("selected recording authority exceeds its finite byte bound".into());
    }
    serde_json::from_slice::<SelectedAuthority>(bytes)
        .map(Into::into)
        .map_err(|e| format!("selected recording authority: {e}"))
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
    ) -> Result<super::Response, String> {
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
        if let RecordingOperation::NextChunk {
            expected_index,
            expected_canonical_sha256,
        } = &request.operation
        {
            if *expected_index >= 16 {
                return Err("recording chunk index exceeds the finite stage profile".into());
            }
            let metadata = owner.prepare_next_source_recording_chunk(
                root,
                *expected_index,
                *expected_canonical_sha256,
                request.preparation_limit_bytes,
            )?;
            return Ok(super::Response::BodyRecordingChunkPrepared {
                protocol: super::PROTOCOL,
                metadata,
            });
        }
        let previous_generation = ad.offer_generation;
        // Never take selected_environmental_equipment: the single ordinary
        // General worker owns only this operation's original Host custody.
        let result = match request.operation {
            RecordingOperation::NextChunk { .. } => {
                return Err("recording preparation was not handled".into())
            }
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
        preparation_error.map_or(
            Ok(super::Response::BodyRunRequested {
                protocol: super::PROTOCOL,
                maximum_millis: request.maximum_millis,
            }),
            Err,
        )
    }
    #[cfg(not(all(
        feature = "installed-environmental-sources",
        feature = "installed-body-storage",
        target_os = "linux"
    )))]
    pub(super) fn start_owned_recording(
        &mut self,
        _request: RecordingRequest,
    ) -> Result<super::Response, String> {
        Err("installed recording requires installed-environmental-sources and installed-body-storage on Linux".into())
    }
}
#[cfg(unix)]
pub(crate) fn recording(state_dir: &Path, request: RecordingRequest) -> Result<(), String> {
    let expected_millis = request.maximum_millis;
    let expected_chunk = match &request.operation {
        RecordingOperation::NextChunk { expected_index, .. } => {
            Some(u64::from(*expected_index) + 1)
        }
        _ => None,
    };
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
        } if expected_chunk.is_none() && maximum_millis == expected_millis => {
            println!(
                "{}",
                serde_json::json!({"schema":"conduit.body/service-run-requested@1","maximum_millis":maximum_millis,"play_state":"preparing"})
            );
            Ok(())
        }
        super::Response::BodyRecordingChunkPrepared {
            protocol: super::PROTOCOL,
            metadata,
        } if metadata
            .get("chunk_index")
            .and_then(serde_json::Value::as_u64)
            == expected_chunk
            && expected_chunk.is_some() =>
        {
            println!(
                "{}",
                serde_json::json!({"schema":"conduit.body/recording-chunk-prepared@1", "execution_started":false,"chunk":metadata})
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
