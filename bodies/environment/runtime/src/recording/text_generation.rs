//! Fixed-cardinality recording content for an ordinary immutable Text generation.
//! This prepares bytes and a reference, never storage or a durable receipt.
use super::*;
use conduit_core::{kind_id, BoundedResourceRef};
use conduit_data::{DataReference, DataReferenceRefusal};

#[derive(Debug, PartialEq, Eq)]
pub enum RecordingTextRefusal {
    Samples,
    Extent,
    Reference(DataReferenceRefusal),
    Incomplete,
    Recording(RecordingRefusal),
}
impl From<RecordingRefusal> for RecordingTextRefusal {
    fn from(value: RecordingRefusal) -> Self {
        Self::Recording(value)
    }
}
/// The same canonical chunk used by snapshot recording, paired with the
/// independently selected ordinary `data/save` Text reference. The adapter does
/// not convert a snapshot grant into a data grant or choose another generation.
pub struct PreparedRecordingText {
    chunk: PreparedRecordingChunk,
    samples: usize,
    encoded_reference: alloc::vec::Vec<u8>,
}
impl PreparedRecordingText {
    /// Exact content size before samples arrive. This is construction material,
    /// not a reservation or a generation identity.
    pub fn content_bytes(
        context: RecordingContext<'_>,
        contract: &ObservationContract,
        samples: usize,
    ) -> Result<usize, RecordingTextRefusal> {
        if !(1..=MAXIMUM_CHUNK_SAMPLES).contains(&samples) {
            return Err(RecordingTextRefusal::Samples);
        }
        let chunk = PreparedRecordingChunk::prepare(context, contract)?;
        let bytes = chunk.prefix.len() + samples * SAMPLE_BYTES * 2 + 2;
        if bytes > conduit_data::MAXIMUM_DATA_TEXT_BYTES as usize {
            return Err(RecordingTextRefusal::Extent);
        }
        Ok(bytes)
    }
    /// Validate the shared data-reference contract and the exact selected extent.
    /// Root must separately admit and execute the actual file-backed operation.
    pub fn prepare(
        context: RecordingContext<'_>,
        contract: &ObservationContract,
        samples: usize,
        reference: &BoundedResourceRef,
    ) -> Result<Self, RecordingTextRefusal> {
        use RecordingTextRefusal as E;
        if !(1..=MAXIMUM_CHUNK_SAMPLES).contains(&samples) {
            return Err(E::Samples);
        }
        let data = DataReference::new(reference.clone()).map_err(E::Reference)?;
        data.validate_for(&kind_id("value/text"))
            .map_err(E::Reference)?;
        let chunk = PreparedRecordingChunk::prepare(context, contract)?;
        let bytes = chunk.prefix.len() + samples * SAMPLE_BYTES * 2 + 2;
        if bytes > conduit_data::MAXIMUM_DATA_TEXT_BYTES as usize
            || reference.extent.bytes != bytes as u64
        {
            return Err(E::Extent);
        }
        Ok(Self {
            chunk,
            samples,
            encoded_reference: data.encode().map_err(E::Reference)?,
        })
    }
    /// Preserve the producer's original sequence and boot-clock coordinates.
    /// Exact duplicates remain idempotent, including at the selected capacity.
    pub fn append(
        &mut self,
        sequence: u64,
        sampled_at_boot_ms: u64,
        disposition: RecordingDisposition,
        observation: Option<&[u8]>,
    ) -> Result<ChunkAppend, RecordingTextRefusal> {
        if self.chunk.sealed {
            return Err(RecordingRefusal::Sealed.into());
        }
        if self.chunk.count == self.samples
            && !self.chunk.samples[..self.chunk.count]
                .iter()
                .any(|sample| sample[..8] == sequence.to_le_bytes())
        {
            return Err(RecordingRefusal::Full.into());
        }
        Ok(self
            .chunk
            .append(sequence, sampled_at_boot_ms, disposition, observation)?)
    }
    /// Owned heap capacities. Inline sample backing, shared schemas and allocator
    /// overhead remain separate in the Root's accounting.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.chunk
            .retained_backing_bytes()?
            .checked_add(self.encoded_reference.capacity())
    }
    pub fn len(&self) -> usize {
        self.chunk.len()
    }
    pub fn is_empty(&self) -> bool {
        self.chunk.is_empty()
    }
    /// A partial flush cannot change the selected extent. Repeated sealing
    /// returns the same bytes; none of these outcomes proves publication.
    pub fn seal(&mut self) -> Result<&str, RecordingTextRefusal> {
        if self.chunk.len() != self.samples {
            return Err(RecordingTextRefusal::Incomplete);
        }
        Ok(self.chunk.seal()?)
    }
    /// Expected `saved` output of the ordinary Text publication Plot. Equality
    /// alone is not a receipt: Root must verify completed execution separately.
    pub fn expected_reference(&self) -> &[u8] {
        &self.encoded_reference
    }
}

#[cfg(test)]
#[path = "text_generation/tests.rs"]
mod tests;
