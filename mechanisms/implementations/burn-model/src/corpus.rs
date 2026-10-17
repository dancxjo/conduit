//! Finite host-prepared corpus batches; no ambient dataset discovery or loader.
//! Exact semantic batch identities and tensor contents, not caller cursors, own
//! the next training input. This is a bounded manual-batch realization profile.
use crate::{BurnAdapter, BurnModelDefinition, Cancellation, Error, ModelBatch};
use conduit_ai::{BatchOrder, TrainStepOutcome, TrainStepRequest, TrainingBatch};
use conduit_plot::rust_binding::NativeRustBinding;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CORPUS_ORDER_PROFILE: &str = "burn/finite-batch-sha256-order@1";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusRecipe {
    /// None preserves the declared batch order; Some pins shuffled epoch order.
    pub shuffle_seed: Option<u64>,
    pub epochs: u64,
    /// Only the final declared batch may have fewer examples than the first.
    pub allow_short_final_batch: bool,
}
#[derive(Debug, Clone, Copy)]
pub struct CorpusLimits {
    pub maximum_batches: usize,
    pub maximum_tensor_bytes: u64,
    pub maximum_epochs: u64,
}
#[derive(Debug, Clone)]
pub struct CorpusBatch {
    pub identity: TrainingBatch,
    pub tensors: ModelBatch,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusCursor {
    pub profile: String,
    pub corpus_identity: [u8; 32],
    pub recipe: CorpusRecipe,
    pub batch_count: u64,
    pub epoch: u64,
    pub next_batch: u64,
}
#[derive(Debug, Clone, Copy)]
pub struct CorpusStorageEstimate {
    pub retained_bytes: u64,
    pub staging_bytes: u64,
}
pub struct PreparedTrainingCorpus {
    storage: CorpusStorageEstimate,
    context_identity: [u8; 32],
    batches: Vec<CorpusBatch>,
    order: Vec<usize>,
    keys: Vec<[u8; 32]>,
    cursor: CorpusCursor,
}
impl PreparedTrainingCorpus {
    pub fn prepare(
        context: &crate::TrainingContext,
        batches: Vec<CorpusBatch>,
        recipe: CorpusRecipe,
        limits: CorpusLimits,
    ) -> Result<Self, Error> {
        context
            .session
            .validate(&context.artifact, &context.dataset, &context.split)?;
        if limits.maximum_batches == 0
            || limits.maximum_batches > 256
            || batches.is_empty()
            || batches.len() > limits.maximum_batches
            || batches.capacity() > limits.maximum_batches
            || recipe.epochs == 0
            || recipe.epochs > limits.maximum_epochs
            || recipe
                .epochs
                .checked_mul(batches.len() as u64)
                .is_none_or(|n| n > context.session.resources.maximum_steps())
        {
            return Err(Error::ResourceBound);
        }
        let context_identity =
            context
                .session
                .semantic_digest(&context.artifact, &context.dataset, &context.split)?;
        let mut digest = Sha256::new();
        digest.update(CORPUS_ORDER_PROFILE.as_bytes());
        digest.update(
            context
                .dataset
                .semantic_digest()
                .map_err(|_| Error::InvalidDescriptor)?,
        );
        digest.update(
            context
                .split
                .semantic_digest()
                .map_err(|_| Error::InvalidDescriptor)?,
        );
        let mut total = 0u64;
        let mut examples = Vec::new();
        let first_count = batches[0].identity.example_identities_iter().count();
        for (index, batch) in batches.iter().enumerate() {
            let b = &batch.identity;
            let count = b.example_identities_iter().count();
            if b.dataset_identity != context.dataset.identity
                || b.split_identity != context.split.split_identity
                || b.order != BatchOrder::Stable
                || b.stochastic_seed.is_some()
                || count == 0
                || count > context.session.resources.maximum_batch_items() as usize
                || (count != first_count
                    && !(recipe.allow_short_final_batch
                        && index + 1 == batches.len()
                        && count < first_count))
                || batches[..index]
                    .iter()
                    .any(|prior| prior.identity.identity == b.identity)
            {
                return Err(Error::InvalidDescriptor);
            }
            for identity in b.example_identities_iter() {
                if *identity == [0; 32]
                    || examples.contains(identity)
                    || !context
                        .split
                        .identities()
                        .any(|member| member.get() == identity)
                {
                    return Err(Error::InvalidDescriptor);
                }
                examples.push(*identity);
                if examples.len() > conduit_ai::MAXIMUM_BATCH_EXAMPLES {
                    return Err(Error::ResourceBound);
                }
            }
            let encoded = b.clone().encode().map_err(|_| Error::InvalidDescriptor)?;
            digest.update((encoded.len() as u64).to_le_bytes());
            digest.update(encoded);
            let mut bytes = 0u64;
            for values in [&batch.tensors.inputs, &batch.tensors.targets] {
                if values.len() > conduit_ai::MODEL_WORK_MAXIMUM_TENSORS
                    || values.capacity() > conduit_ai::MODEL_WORK_MAXIMUM_TENSORS
                {
                    return Err(Error::ResourceBound);
                }
                digest.update((values.len() as u64).to_le_bytes());
                for tensor in values {
                    tensor.validate().map_err(|_| Error::InvalidTensor)?;
                    // Context tensors may omit a Batch axis; every declared
                    // Batch axis must describe the admitted example membership.
                    for (axis, dimension) in tensor.axes.iter().zip(tensor.dimensions.iter()) {
                        if axis.role == conduit_data::TensorAxisRole::Batch
                            && *dimension != count as u64
                        {
                            return Err(Error::InvalidTensor);
                        }
                    }
                    // A prepared corpus owns inline immutable payloads, never
                    // unresolved resource references whose contents may change.
                    if !matches!(tensor.backing, conduit_data::TensorBacking::Inline(_)) {
                        return Err(Error::InvalidTensor);
                    }
                    bytes = bytes
                        .checked_add(tensor.byte_count().map_err(|_| Error::InvalidTensor)?)
                        .ok_or(Error::ResourceBound)?;
                    digest.update(tensor.semantic_digest().map_err(|_| Error::InvalidTensor)?);
                }
            }
            if bytes != b.encoded_bytes || bytes > context.session.resources.maximum_batch_bytes() {
                return Err(Error::ResourceBound);
            }
            total = total.checked_add(bytes).ok_or(Error::ResourceBound)?;
            if total > limits.maximum_tensor_bytes {
                return Err(Error::ResourceBound);
            }
        }
        // This profile consumes every split occurrence exactly once per epoch.
        if examples.len() != context.split.identities().count() {
            return Err(Error::InvalidDescriptor);
        }
        let cursor = CorpusCursor {
            profile: CORPUS_ORDER_PROFILE.into(),
            corpus_identity: digest.finalize().into(),
            recipe,
            batch_count: batches.len() as u64,
            epoch: 0,
            next_batch: 0,
        };
        let mut retained = storage_add(
            core::mem::size_of::<Self>() as u64,
            storage_slots::<CorpusBatch>(batches.len())?,
        )?;
        retained = storage_add(retained, storage_slots::<usize>(batches.len())?)?;
        retained = storage_add(retained, storage_slots::<[u8; 32]>(batches.len())?)?;
        retained = storage_add(retained, CORPUS_ORDER_PROFILE.len() as u64 * 4)?;
        let mut staging = 0;
        for batch in &batches {
            let bytes = batch_storage(batch)?;
            retained = storage_add(retained, bytes)?;
            staging = staging.max(storage_add(
                bytes,
                core::mem::size_of::<CorpusCursor>() as u64 + CORPUS_ORDER_PROFILE.len() as u64 * 4,
            )?);
        }
        // Clone all validated carriers into fresh bounded storage. This removes
        // caller-owned spare capacities from nested Native sequences/Strings.
        let batches = batches.to_vec();
        let mut owner = Self {
            storage: CorpusStorageEstimate {
                retained_bytes: retained,
                staging_bytes: staging,
            },
            context_identity,
            order: (0..batches.len()).collect(),
            keys: vec![[0; 32]; batches.len()],
            batches,
            cursor,
        };
        owner.reorder();
        Ok(owner)
    }
    pub fn storage_estimate(&self) -> CorpusStorageEstimate {
        self.storage
    }
    pub(crate) fn working_bytes(&self) -> Result<u64, Error> {
        storage_add(self.storage.retained_bytes, self.storage.staging_bytes)
    }
    pub fn cursor(&self) -> &CorpusCursor {
        &self.cursor
    }
    pub fn next(&self) -> Option<&CorpusBatch> {
        if self.cursor.epoch >= self.cursor.recipe.epochs {
            return None;
        }
        self.batches
            .get(*self.order.get(self.cursor.next_batch as usize)?)
    }
    fn reorder(&mut self) {
        for (index, key) in self.keys.iter_mut().enumerate() {
            let mut h = Sha256::new();
            h.update(CORPUS_ORDER_PROFILE.as_bytes());
            h.update(self.cursor.recipe.shuffle_seed.unwrap_or(0).to_le_bytes());
            h.update(self.cursor.epoch.to_le_bytes());
            h.update((index as u64).to_le_bytes());
            *key = h.finalize().into();
        }
        for (index, value) in self.order.iter_mut().enumerate() {
            *value = index;
        }
        if self.cursor.recipe.shuffle_seed.is_some() {
            self.order
                .sort_unstable_by_key(|index| (self.keys[*index], *index));
        }
    }
    fn advanced(&self) -> CorpusCursor {
        let mut next = self.cursor.clone();
        next.next_batch += 1;
        if next.next_batch == next.batch_count {
            next.epoch += 1;
            next.next_batch = 0;
        }
        next
    }
    pub(crate) fn validate_resume(&self, saved: &CorpusCursor, steps: u64) -> Result<(), Error> {
        if saved.profile != CORPUS_ORDER_PROFILE
            || saved.corpus_identity != self.cursor.corpus_identity
            || saved.recipe != self.cursor.recipe
            || saved.batch_count != self.cursor.batch_count
            || saved.epoch > saved.recipe.epochs
            || saved.next_batch >= saved.batch_count
            || (saved.epoch == saved.recipe.epochs && saved.next_batch != 0)
            || saved
                .epoch
                .checked_mul(saved.batch_count)
                .and_then(|v| v.checked_add(saved.next_batch))
                != Some(steps)
        {
            return Err(Error::IncompatibleCheckpoint);
        }
        Ok(())
    }
    pub(crate) fn restore(&mut self, saved: CorpusCursor) {
        self.cursor = saved;
        self.reorder();
    }
}
impl<D: BurnModelDefinition> BurnAdapter<D> {
    pub fn attach_corpus(&mut self, corpus: PreparedTrainingCorpus) -> Result<(), Error> {
        self.ready()?;
        if self.inference_only || self.state.completed_steps != 0 || self.corpus.is_some() {
            return Err(Error::IncompatibleCheckpoint);
        }
        // A prepared owner belongs to this complete dataset/split context.
        if corpus.context_identity
            != self.context.session.semantic_digest(
                &self.context.artifact,
                &self.context.dataset,
                &self.context.split,
            )?
        {
            return Err(Error::IncompatibleCheckpoint);
        }
        for batch in &corpus.batches {
            self.validate_batch(&batch.tensors, &batch.identity)?;
            self.context
                .session
                .admit_batch(&batch.identity, &self.context.split)?;
        }
        let required = self
            .descriptor
            .resources
            .working_bytes()?
            .checked_add(corpus.working_bytes()?)
            .ok_or(Error::ResourceBound)?;
        if required > self.context.session.resources.working_memory_bytes()
            || required > self.offer.limits.maximum_working_memory_bytes
        {
            return Err(Error::ResourceBound);
        }
        self.corpus = Some(corpus);
        Ok(())
    }
    pub fn corpus_cursor(&self) -> Option<&CorpusCursor> {
        self.corpus.as_ref().map(PreparedTrainingCorpus::cursor)
    }
    pub fn next_corpus_batch(&self) -> Option<&CorpusBatch> {
        self.corpus.as_ref().and_then(PreparedTrainingCorpus::next)
    }
    pub fn train_next(&mut self, cancel: &Cancellation) -> Result<TrainStepOutcome, Error> {
        self.ready()?;
        let owner = self.corpus.as_ref().ok_or(Error::InvalidDescriptor)?;
        let next = owner.next().ok_or(Error::ResourceBound)?;
        let cursor = owner.advanced();
        let tensors = next.tensors.clone();
        let request = TrainStepRequest {
            step: self
                .state
                .completed_steps
                .checked_add(1)
                .ok_or(Error::ResourceBound)?,
            expected_generation: self.state.model.generation,
            admitted_work_units: self.descriptor.resources.maximum_work_per_step,
            batch: next.identity.clone(),
        };
        let outcome = self.train_step_inner(&request, &tensors, cancel)?;
        if matches!(outcome, TrainStepOutcome::Committed(_)) {
            self.corpus
                .as_mut()
                .expect("exclusive prepared owner")
                .restore(cursor);
        }
        Ok(outcome)
    }
}

// Conservative requested-storage estimates for normalized cloned carriers.
// The factor four covers geometric/minimum Vec and String capacities. These
// estimates are admitted profile values, not a whole-process allocator ceiling.
fn storage_add(a: u64, b: u64) -> Result<u64, Error> {
    a.checked_add(b).ok_or(Error::ResourceBound)
}
fn storage_slots<T>(count: usize) -> Result<u64, Error> {
    (count.max(4) as u64)
        .checked_mul(core::mem::size_of::<T>() as u64)
        .and_then(|n| n.checked_mul(4))
        .ok_or(Error::ResourceBound)
}
fn batch_storage(batch: &CorpusBatch) -> Result<u64, Error> {
    let mut bytes = core::mem::size_of::<CorpusBatch>() as u64;
    bytes = storage_add(bytes, batch.identity.split_identity.len() as u64 * 4)?;
    let pages = batch.identity.example_identities.get();
    bytes = storage_add(
        bytes,
        storage_slots::<conduit_ai::TrainingExampleIdentityPage>(pages.len())?,
    )?;
    for page in pages {
        bytes = storage_add(bytes, page.get().as_slice().len() as u64 * 4)?;
    }
    let modalities = batch.identity.present_modalities.get();
    bytes = storage_add(
        bytes,
        storage_slots::<conduit_ai::TrainingModality>(modalities.len())?,
    )?;
    for modality in modalities {
        bytes = storage_add(bytes, modality.get().len() as u64 * 4)?;
    }
    for values in [&batch.tensors.inputs, &batch.tensors.targets] {
        bytes = storage_add(
            bytes,
            storage_slots::<conduit_data::TensorValue>(values.len())?,
        )?;
        for tensor in values {
            bytes = storage_add(bytes, storage_slots::<u64>(tensor.dimensions.len())?)?;
            bytes = storage_add(
                bytes,
                storage_slots::<conduit_data::TensorAxis>(tensor.axes.len())?,
            )?;
            for axis in &tensor.axes {
                if let Some(identity) = &axis.identity {
                    bytes = storage_add(bytes, identity.len() as u64 * 4)?;
                }
                if let conduit_data::TensorAxisRole::Other(role) = &axis.role {
                    bytes = storage_add(bytes, role.identity().len() as u64 * 4)?;
                }
            }
            if let conduit_data::TensorBacking::Inline(payload) = &tensor.backing {
                bytes = storage_add(bytes, payload.as_slice().len() as u64 * 4)?;
            }
        }
    }
    Ok(bytes)
}
