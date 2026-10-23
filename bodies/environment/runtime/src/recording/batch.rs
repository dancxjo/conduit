//! Complete finite recorder preparation. No generation minting, store effects,
//! routing, implicit flush, retry or durable acknowledgement is performed here.
use super::*;
use alloc::vec::Vec;
use conduit_core::BoundedResourceRef;

#[derive(Debug, PartialEq, Eq)]
pub enum RecordingBatchRefusal {
    Capacity,
    Generations,
    DuplicateGeneration,
    Snapshot(RecordingSnapshotRefusal),
}

/// Move the Back into the ordinary kernel's admitted driver set; route this
/// encoder's sealed content to it through the selected typed cord. Only the
/// Back's actual committed Host Call completion advances its observer.
pub struct PreparedRecordingGeneration {
    pub samples: usize,
    pub encoder: PreparedRecordingSnapshot,
    pub publication: RecordingSnapshotBack,
    pub observer: RecordingCommitObserver,
}

pub struct PreparedRecordingBatch {
    generations: Vec<PreparedRecordingGeneration>,
    content_bytes: u64,
}
impl PreparedRecordingBatch {
    /// Caller supplies independently admitted immutable references and finite
    /// resource ceilings. Full chunks precede the final possibly shorter chunk.
    /// All encoders, validators, output envelopes and publication Backs are
    /// initialized now; additional generations require a new admission.
    /// References and these ceilings confer no possession or authority. Root
    /// must separately admit heap/work, actual storage and current placement.
    pub fn prepare(
        context: RecordingContext<'_>,
        contract: &ObservationContract,
        samples: u64,
        references: &[BoundedResourceRef],
        maximum_generations: usize,
        maximum_content_bytes: u64,
    ) -> Result<Self, RecordingBatchRefusal> {
        use RecordingBatchRefusal as E;
        if samples == 0 || maximum_generations == 0 {
            return Err(E::Capacity);
        }
        let count = usize::try_from(samples.div_ceil(MAXIMUM_CHUNK_SAMPLES as u64))
            .map_err(|_| E::Capacity)?;
        if count > maximum_generations {
            return Err(E::Capacity);
        }
        if count != references.len() {
            return Err(E::Generations);
        }
        // Validate the complete closure before retaining any initialized Back.
        let mut content_bytes = 0_u64;
        for (index, reference) in references.iter().enumerate() {
            if references[..index].iter().any(|previous| {
                previous.identity == reference.identity
                    && previous.lifetime.version == reference.lifetime.version
            }) {
                return Err(E::DuplicateGeneration);
            }
            let size = chunk_samples(samples, index);
            let snapshot = PreparedRecordingSnapshot::prepare(context, contract, size, reference)
                .map_err(E::Snapshot)?;
            drop(snapshot);
            content_bytes = content_bytes
                .checked_add(reference.extent.bytes)
                .filter(|total| *total <= maximum_content_bytes)
                .ok_or(E::Capacity)?;
        }
        let mut generations = Vec::with_capacity(count);
        for (index, reference) in references.iter().enumerate() {
            let samples = chunk_samples(samples, index);
            let encoder = PreparedRecordingSnapshot::prepare(context, contract, samples, reference)
                .map_err(E::Snapshot)?;
            let (publication, observer) =
                RecordingSnapshotBack::prepare(context, contract, samples, reference)
                    .map_err(E::Snapshot)?;
            generations.push(PreparedRecordingGeneration {
                samples,
                encoder,
                publication,
                observer,
            });
        }
        Ok(Self {
            generations,
            content_bytes,
        })
    }
    pub fn content_bytes(&self) -> u64 {
        self.content_bytes
    }
    pub fn generations(&self) -> &[PreparedRecordingGeneration] {
        &self.generations
    }
    /// Ownership transfer only. This does not install or start the kernel.
    pub fn into_generations(self) -> Vec<PreparedRecordingGeneration> {
        self.generations
    }
}
fn chunk_samples(samples: u64, index: usize) -> usize {
    (samples - index as u64 * MAXIMUM_CHUNK_SAMPLES as u64).min(MAXIMUM_CHUNK_SAMPLES as u64)
        as usize
}
