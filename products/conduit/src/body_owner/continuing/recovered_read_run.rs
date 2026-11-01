//! Independently admitted reads of the State-selected catalog and its chunks.
//! Decode only after quiescence; discovery and content never issue authority.
use super::*;
use conduit_core as c;
use conduit_environment_runtime::{
    recording::PreparedRecordingCatalogReader, source_recording::SourceRecordingSnapshot,
};
use conduit_std_host::body_execution::{BodyBorrowedForeOutputAdapter, BodyForeDeliveryRef};
use sha2::{Digest, Sha256};
use source_recording_stage::recovered_read::RecoveredSourceReadInput;
#[path = "recovered_read_run/evidence.rs"]
mod evidence;
pub(in super::super) struct RecoveredCatalogContents {
    catalog_reference: c::BoundedResourceRef,
    references: Vec<c::BoundedResourceRef>,
    verified: [bool; 16],
}
impl RecoveredCatalogContents {
    pub(in super::super) fn catalog_reference(&self) -> &c::BoundedResourceRef {
        &self.catalog_reference
    }
    pub(in super::super) fn references(&self) -> &[c::BoundedResourceRef] {
        &self.references
    }
    pub(super) fn accept_chunk(&mut self, chunk: &RecoveredSourceChunk) -> Result<(), String> {
        if self.references.get(chunk.index) != Some(&chunk.reference) {
            return Err("recovered chunk differs from current catalog".into());
        }
        self.verified[chunk.index] = true;
        Ok(())
    }
    pub(in super::super) fn all_chunks_verified(&self) -> bool {
        !self.references.is_empty() && self.verified[..self.references.len()].iter().all(|v| *v)
    }
}
pub(in super::super) struct RecoveredSourceChunk {
    index: usize,
    reference: c::BoundedResourceRef,
    snapshot: SourceRecordingSnapshot,
}
impl RecoveredSourceChunk {
    pub(in super::super) fn index(&self) -> usize {
        self.index
    }
    pub(in super::super) fn reference(&self) -> &c::BoundedResourceRef {
        &self.reference
    }
    pub(in super::super) fn snapshot(&self) -> &SourceRecordingSnapshot {
        &self.snapshot
    }
}
pub(super) enum RecoveredReadResult {
    Catalog(RecoveredCatalogContents),
    Chunk(RecoveredSourceChunk),
}
pub(super) struct RecoveredReadRun {
    input: RecoveredSourceReadInput,
    bytes: [u8; 4096],
    length: usize,
}
impl RecoveredReadRun {
    pub(super) fn new(input: RecoveredSourceReadInput) -> Self {
        Self {
            input,
            bytes: [0; 4096],
            length: 0,
        }
    }
    pub(super) fn execute<W: std::io::Write, T: conduit_std_host::TimerAdapter, F>(
        &mut self,
        mut host: StdHost,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> (StdHost, Result<BodyRunReport, String>)
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        let mut capture = ReadCapture {
            bytes: &mut self.bytes,
            length: 0,
            expected_length: self.input.reference.extent.bytes,
        };
        let result = host.run_body_plan_with_selected_storage_borrowed_to_with_start(
            request,
            std::slice::from_ref(&self.input.input),
            &mut capture,
            &[],
            Some(&self.input.authority),
            output,
            timer,
            started,
        );
        self.length = capture.length;
        (host, result)
    }
    fn clean(&self, report: Option<&BodyRunReport>) -> bool {
        self.length > 0
            && report.is_some_and(|r| {
                matches!(r.terminal, c::TerminalDisposition::Completed)
                    && r.failure.is_none()
                    && r.kernel_failure.is_none()
                    && r.native_failure.is_none()
                    && r.cleanup_failure.is_none()
                    && r.fore_pending_output.is_none()
            })
    }
    pub(super) fn recovered(
        &self,
        report: Option<&BodyRunReport>,
    ) -> Result<Option<RecoveredReadResult>, String> {
        if !self.clean(report) {
            return Ok(None);
        }
        let bytes = &self.bytes[..self.length];
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        // This environmental catalog profile deliberately uses canonical-byte
        // SHA256 for both identities. Core ResourceRefs need not do so generally.
        if self.input.reference.identity.digest() != digest
            || self.input.reference.lifetime.version.digest() != digest
        {
            return Err("recovered snapshot content differs from its catalogued identity".into());
        }
        if let Some(index) = self.input.chunk_index {
            let snapshot = SourceRecordingSnapshot::decode_candidate(bytes)
                .map_err(|e| format!("recovered Source snapshot: {e:?}"))?;
            if snapshot.context().body_id != report.expect("clean report").play.body_id.as_str() {
                return Err("recovered Source content belongs to another Body".into());
            }
            Ok(Some(RecoveredReadResult::Chunk(RecoveredSourceChunk {
                index,
                reference: self.input.reference.clone(),
                snapshot,
            })))
        } else {
            let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
            let mut reader = PreparedRecordingCatalogReader::default();
            reader
                .read(text)
                .map_err(|e| format!("recovered catalog: {e:?}"))?;
            let mut references = Vec::with_capacity(reader.len());
            for i in 0..reader.len() {
                let reference = c::BoundedResourceRef::decode(
                    reader.encoded_reference(i).ok_or("catalog entry absent")?,
                )
                .map_err(|e| format!("catalog chunk reference: {e:?}"))?;
                if reference.extent.bytes == 0
                    || reference.extent.bytes > 4096
                    || reference.extent.items != Some(1)
                    || reference.lifetime.expires_at.is_some()
                {
                    return Err("catalog chunk exceeds the selected Source profile".into());
                }
                references.push(reference);
            }
            if references.is_empty() || references.len() > 16 {
                return Err("recovered catalog has no finite Source entries".into());
            }
            Ok(Some(RecoveredReadResult::Catalog(
                RecoveredCatalogContents {
                    catalog_reference: self.input.reference.clone(),
                    references,
                    verified: [false; 16],
                },
            )))
        }
    }
    pub(super) fn metadata(&self, report: Option<&BodyRunReport>) -> serde_json::Value {
        // Structural evidence only. The private result is accepted after exact
        // issued-Play reconciliation, never by inspecting this public JSON.
        serde_json::json!({"custody":"admitted-snapshot-read-not-producer-retirement",
            "read_completed":self.clean(report),"chunk_index":self.input.chunk_index,
            "bytes":self.length,"producer_retirement_authorized":false})
    }
}
struct ReadCapture<'a> {
    bytes: &'a mut [u8; 4096],
    length: usize,
    expected_length: u64,
}
impl BodyBorrowedForeOutputAdapter for ReadCapture<'_> {
    fn deliver(&mut self, value: BodyForeDeliveryRef<'_>) -> Result<(), String> {
        if self.length != 0
            || value.front_port_id.as_str() != "content"
            || value.track != c::ConnectionTrack::Payload
            || value.value_kind.as_str() != "text/json-utf8@1"
            || value.sequence != 0
            || value.bytes.is_empty()
            || value.bytes.len() > 4096
            || value.bytes.len() as u64 != self.expected_length
        {
            return Err("recovered snapshot returned unexpected or excess content Fore".into());
        }
        self.bytes[..value.bytes.len()].copy_from_slice(value.bytes);
        self.length = value.bytes.len();
        Ok(())
    }
}
