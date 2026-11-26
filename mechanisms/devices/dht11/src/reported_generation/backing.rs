//! Requested Rust backing, excluding allocator bookkeeping, stacks and caller
//! ObservationContract/catalog construction. No allocation-free Step is claimed.
use super::*;
use core::mem::size_of;

impl Metadata {
    fn retained_backing_bytes(&self) -> Option<usize> {
        self.producer
            .iter()
            .chain(self.channels.iter().flatten())
            .try_fold(0usize, |bytes, text| bytes.checked_add(text.capacity()))
    }
}
impl PreparedReportedRecordingGeneration {
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.history
            .retained_backing_bytes()?
            .checked_add(self.metadata.retained_backing_bytes()?)?
            .checked_add(self.original.capacity())
    }
    /// Complete wrapper preparation plus its finite encoding workspace. The
    /// supplied already checked ObservationContract remains caller-owned.
    pub fn preparation_backing_bound(contract: &ObservationContract) -> Option<usize> {
        PreparedRecordingHistory::preparation_backing_bound(contract)?
            .checked_add(size_of::<Self>())?
            .checked_add(MAXIMUM_CHUNK_BYTES)?
            .checked_add(schema::TEXT_FIELDS.len().checked_mul(schema::TEXT_BYTES)?)?
            .checked_add(encoding_workspace_bound()?)
    }
}
impl ReportedRecordingGeneration {
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.reader
            .retained_backing_bytes()?
            .checked_add(self.metadata.retained_backing_bytes()?)?
            .checked_add(self.original.capacity())
    }
}
impl PreparedReportedRecordingGenerationInput {
    /// Decoder preparation, existing reader/Form preparation, returned owned
    /// record/metadata and four temporary typed context IDs coexist. Canonical
    /// input custody and any later Face/escaped contribution are separate.
    pub fn decoding_backing_bound(contract: &ObservationContract) -> Option<usize> {
        let ty = value_type().ok()?;
        let schema = ty.retained_backing_bytes()?;
        let prefix = ty.canonical_bytes().ok()?.len();
        schema
            // Own Type, validator alias and simultaneous Type-construction table.
            .checked_mul(3)?
            .checked_add(prefix.checked_mul(2)?)?
            .checked_add(size_of::<Self>())?
            .checked_add(size_of::<ReportedRecordingGeneration>())?
            .checked_add(PreparedRecordingReader::preparation_backing_bound(
                contract,
            )?)?
            .checked_add(MAXIMUM_CHUNK_BYTES)?
            .checked_add((schema::TEXT_FIELDS.len() + 4).checked_mul(schema::TEXT_BYTES)?)
    }
}
/// Encoding owns the closed Type, exact-capacity field/value vectors, bounded
/// leaf byte vectors and canonical output. Core Type aliases are conservatively
/// charged per field because retained_backing_bytes deliberately ignores shared
/// allocation identity. Three output envelopes cover simultaneous old/new canonical Vec storage during growth.
pub fn encoding_workspace_bound() -> Option<usize> {
    use conduit_core::{StructuredFieldValue, StructuredInfoType, StructuredInfoValue};
    let ty = value_type().ok()?;
    let count = schema::TEXT_FIELDS
        .len()
        .checked_add(schema::SCALARS.len())?
        .checked_add(1)?;
    let maximum = maximum_canonical_bytes().ok()? as usize;
    let schema = ty.retained_backing_bytes()?;
    // Root Type + one alias in every leaf value + root value alias, including
    // the Type constructor's field-vector material while it is being returned.
    schema
        .checked_mul(count.checked_add(3)?)?
        .checked_add(count.checked_mul(
            size_of::<StructuredFieldValue>()
                + size_of::<StructuredInfoValue>()
                + size_of::<StructuredInfoType>(),
        )?)?
        // The canonical ceiling also bounds field names and all leaf payloads.
        .checked_add(maximum)?
        .checked_add(maximum.checked_mul(3)?)
}
