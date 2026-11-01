//! Explicit compare-and-select of the next chunk; original slots never retire.
use super::super::super::Owner;
use super::*;
use conduit_core as c;
use std::path::Path;
pub(super) struct VerifiedChunk {
    witness: super::super::recording_run::VerifiedSourceSnapshot,
    index: u8,
    first: usize,
    pub(super) end: usize,
}
impl SourceRecordingStage {
    fn advance_chunk(
        &mut self,
        expected_index: u8,
        expected_digest: [u8; 32],
    ) -> Result<serde_json::Value, String> {
        let current = self
            .prepared
            .as_ref()
            .ok_or("Source chunk is not prepared")?;
        if current.index != expected_index {
            let previous = self.archived.last().filter(|old| {
                old.index == expected_index
                    && current.index == expected_index.saturating_add(1)
                    && old.end == current.first
                    && old.witness.reference().identity.digest() == expected_digest
                    && old.witness.reference().lifetime.version.digest() == expected_digest
            });
            if previous.is_some() {
                return self
                    .prepared_input_metadata()?
                    .ok_or("Source current metadata absent".into());
            }
            return Err("Source next chunk compare-and-select differs from current prefix".into());
        }
        if current.reference.identity.digest() != expected_digest
            || current.reference.lifetime.version.digest() != expected_digest
        {
            return Err("Source next chunk digest differs from current prefix".into());
        }
        if self.verified_reference().is_none() {
            return Err("Source next chunk requires actual matching admitted readback".into());
        }
        if self.archived.len() != usize::from(current.index) || self.archived.len() >= CAPACITY - 1
        {
            return Err("Source chunk archive is not the finite contiguous prefix".into());
        }
        let frozen = self
            .frozen_length
            .ok_or("Source accepted prefix is not frozen")?;
        if frozen != self.length || current.end >= frozen {
            return Err("Source chunk has no remaining records".into());
        }
        let next = self.prepare_chunk(
            current.end,
            current
                .index
                .checked_add(1)
                .ok_or("Source chunk index exhausted")?,
        )?;
        let metadata = self.chunk_metadata(&next, self.archived.len() + 1, current.end)?;
        // All fallible construction precedes mutation. Only encoded temporary
        // content is replaced; every original accepted canonical slot remains.
        let old = self.prepared.replace(next).expect("prepared checked");
        self.archived.push(VerifiedChunk {
            witness: self.verified.take().expect("verified checked"),
            index: old.index,
            first: old.first,
            end: old.end,
        });
        Ok(metadata)
    }
    pub(super) fn verified_catalog_references(&self) -> Result<Vec<c::BoundedResourceRef>, String> {
        let current = self
            .prepared
            .as_ref()
            .ok_or("Source checkpoint input is not prepared")?;
        let verified = self
            .verified_reference()
            .ok_or("Source catalog requires actual matching admitted readback")?;
        let frozen = self
            .frozen_length
            .ok_or("Source accepted prefix is not frozen")?;
        let mut end = 0;
        for (index, old) in self.archived.iter().enumerate() {
            if usize::from(old.index) != index || old.first != end || old.end <= old.first {
                return Err("Source verified archive has a gap or overlap".into());
            }
            end = old.end;
        }
        if current.first != end
            || usize::from(current.index) != self.archived.len()
            || current.end != frozen
            || frozen != self.length
        {
            return Err(
                "Source catalog requires verified chunks covering the whole frozen prefix".into(),
            );
        }
        // Existing catalog codec consumes a slice of owned small references.
        // This finite temporary descriptor vector is not retained chunk content.
        Ok(self
            .archived
            .iter()
            .map(|c| c.witness.reference().clone())
            .chain(std::iter::once(verified.clone()))
            .collect())
    }
}
impl Owner {
    pub(crate) fn source_recording_stage_metadata(
        &self,
    ) -> Result<Option<serde_json::Value>, String> {
        self.source_recording_stage
            .as_ref()
            .map(|stage| stage.prepared_input_metadata())
            .transpose()
            .map(Option::flatten)
    }
    pub(crate) fn prepare_next_source_recording_chunk(
        &mut self,
        root: &Path,
        expected_index: u8,
        expected_canonical_sha256: [u8; 32],
        preparation_limit_bytes: u64,
    ) -> Result<serde_json::Value, String> {
        if self.host.is_playing()
            || self.session.realization().is_some()
            || self.pending_browser.is_some()
            || self.session.evidence().body.state != conduit_body::BodyState::Lulled
        {
            return Err("Source next chunk requires the original lulled idle owner".into());
        }
        self.ensure_execution_retained(root)?;
        let storage = self
            .host
            .selected_body_storage()
            .ok_or("Source next chunk has no selected storage")?;
        // Named finite codec/source volume, not allocator or whole-Root admission.
        let volume = 2 * conduit_environment_runtime::source_recording::MAXIMUM_BYTES as u64
            + CAPACITY as u64 * 512
            + std::mem::size_of::<checkpoint::PreparedSourceChunk>() as u64;
        if volume > preparation_limit_bytes
            || preparation_limit_bytes > storage.bounds().preparation_bytes
        {
            return Err(
                "Source next chunk preparation volume exceeds selected finite capacity".into(),
            );
        }
        let stage = self
            .source_recording_stage
            .as_mut()
            .ok_or("Source recording stage is absent")?;
        if stage.body_id != self.session.evidence().body_id {
            return Err("Source next chunk belongs to another Body".into());
        }
        if stage
            .prepared
            .as_ref()
            .is_some_and(|p| p.index == expected_index)
            && stage.archived.len() + 3 > storage.bounds().maximum_snapshot_records as usize
        {
            return Err("Source next chunk and catalog exceed selected record capacity".into());
        }
        stage.advance_chunk(expected_index, expected_canonical_sha256)
    }
}

#[cfg(test)]
#[path = "advance/tests.rs"]
mod tests;
