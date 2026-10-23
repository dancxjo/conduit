//! Fixed-cardinality content for one independently admitted snapshot generation.
//! This does not issue a grant, execute storage or infer durable completion.
use super::*;
use conduit_core::{BoundedResourceRef, ResourceReferenceRefusal};

#[derive(Debug, PartialEq, Eq)]
pub enum RecordingSnapshotRefusal {
    Samples,
    Extent,
    Reference(ResourceReferenceRefusal),
    Incomplete,
    Recording(RecordingRefusal),
}
impl From<RecordingRefusal> for RecordingSnapshotRefusal {
    fn from(value: RecordingRefusal) -> Self {
        Self::Recording(value)
    }
}
/// A selected snapshot reference fixes the exact byte extent before Play. Each
/// sample has a fixed representation, including failures. A partial chunk cannot
/// silently change that extent or consume a different generation.
pub struct PreparedRecordingSnapshot {
    chunk: PreparedRecordingChunk,
    samples: usize,
    encoded_reference: alloc::vec::Vec<u8>,
}
impl PreparedRecordingSnapshot {
    /// Calculate construction material before the Root selects a generation.
    /// No samples, storage effects or authority are needed to know this extent.
    pub fn content_bytes(
        context: RecordingContext<'_>,
        contract: &ObservationContract,
        samples: usize,
    ) -> Result<usize, RecordingSnapshotRefusal> {
        if !(1..=MAXIMUM_CHUNK_SAMPLES).contains(&samples) {
            return Err(RecordingSnapshotRefusal::Samples);
        }
        let chunk = PreparedRecordingChunk::prepare(context, contract)?;
        Ok(chunk.prefix.len() + samples * SAMPLE_BYTES * 2 + 2)
    }
    /// The caller obtains this reference through ordinary placement/resource
    /// admission. Validating content here confers neither possession nor storage
    /// authority; the existing snapshot Host Call must still enforce both.
    pub fn prepare(
        context: RecordingContext<'_>,
        contract: &ObservationContract,
        samples: usize,
        reference: &BoundedResourceRef,
    ) -> Result<Self, RecordingSnapshotRefusal> {
        use RecordingSnapshotRefusal as E;
        if !(1..=MAXIMUM_CHUNK_SAMPLES).contains(&samples) {
            return Err(E::Samples);
        }
        reference.validate().map_err(E::Reference)?;
        let chunk = PreparedRecordingChunk::prepare(context, contract)?;
        let bytes = chunk.prefix.len() + samples * SAMPLE_BYTES * 2 + 2;
        if reference.access_class.as_str() != "resource/snapshot@1"
            || reference.content_profile.as_str() != "text/json-utf8@1"
            || reference.extent.items != Some(1)
            || reference.extent.bytes != bytes as u64
        {
            return Err(E::Extent);
        }
        Ok(Self {
            chunk,
            samples,
            encoded_reference: reference.encode().map_err(E::Reference)?,
        })
    }
    /// Access only the bounded append operation. The fixed publication shape
    /// cannot be mutated through an unguarded chunk reference.
    pub fn append(
        &mut self,
        sequence: u64,
        sampled_at_boot_ms: u64,
        disposition: RecordingDisposition,
        observation: Option<&[u8]>,
    ) -> Result<ChunkAppend, RecordingSnapshotRefusal> {
        self.check_room(sequence)?;
        Ok(self
            .chunk
            .append(sequence, sampled_at_boot_ms, disposition, observation)?)
    }
    fn check_room(&self, sequence: u64) -> Result<(), RecordingSnapshotRefusal> {
        if self.chunk.sealed {
            return Err(RecordingRefusal::Sealed.into());
        }
        // Existing duplicates remain idempotent even at the selected capacity.
        if self.chunk.count == self.samples
            && !self.chunk.samples[..self.chunk.count]
                .iter()
                .any(|sample| sample[..8] == sequence.to_le_bytes())
        {
            return Err(RecordingRefusal::Full.into());
        }
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.chunk.len()
    }
    pub fn is_empty(&self) -> bool {
        self.chunk.is_empty()
    }
    /// Exact JSON input to the existing resource snapshot publication Fore.
    /// A partial flush requires separately admitted content/generation, never a
    /// hidden alteration of this immutable reference. Refusal preserves the chunk.
    pub fn seal(&mut self) -> Result<&str, RecordingSnapshotRefusal> {
        if self.chunk.len() != self.samples {
            return Err(RecordingSnapshotRefusal::Incomplete);
        }
        Ok(self.chunk.seal()?)
    }
    /// Exact output expected from successful completion of the selected snapshot
    /// Host Call. Byte equality alone is not a durable acknowledgement: the Root
    /// must correlate it with that actual completed kernel request.
    pub fn expected_reference(&self) -> &[u8] {
        &self.encoded_reference
    }
}
