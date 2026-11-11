//! Bounded pieces of one Body's observe -> recall -> respond -> retain loop.
//!
//! The loop is ordinary Plot composition: a composer turns one observation plus
//! the memory cell's Current into a model request, the model answers, a
//! retainer pairs the actual outcome with the pending episode, and the explicit memory
//! cell retains the selected episode for later recall. Observations, recalled
//! memories, and model-derived responses remain distinct typed facts.
//! Retention is selected only by the observation's typed `retain` field.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use sha2::{Digest, Sha256};

#[path = "episode_checkpoint_v1.rs"]
mod episode_checkpoint_v1;
#[path = "episode_prompt.rs"]
mod episode_prompt;
#[path = "episode_recall.rs"]
mod episode_recall;
use crate::episode_outcome::decode_outcome;
pub use episode_checkpoint_v1::{episode_checkpoint_v1_type, migrate_episode_checkpoint_v1};
use episode_prompt::prompt;
pub(crate) use episode_recall::shared_terms;
#[path = "episode_recollection.rs"]
mod episode_recollection;
#[path = "episode_retention.rs"]
mod episode_retention;
pub use episode_retention::EpisodeRetainer;
#[path = "episode_checkpoint_v2.rs"]
mod episode_checkpoint_v2;
pub use episode_checkpoint_v2::{episode_checkpoint_v2_type, migrate_episode_checkpoint_v2};

pub use episode_recollection::{episode_recollection_request, MAXIMUM_EPISODE_RECOLLECTION_BYTES};

use crate::{
    EpisodeMemory, EpisodeMemoryOwner, EpisodeMemoryState, EpisodeModelResponse,
    EpisodeObservation, EpisodeOutcome, EpisodePending, EpisodeRecall, EpisodeRecord,
    EpisodeRefusal,
};

pub const EPISODE_COMPOSE_KIND: &str = "body/episode-compose";
pub const EPISODE_COMPOSE_REVISION: &str = "conduit.body/episode-compose@2";
pub const EPISODE_RETAIN_KIND: &str = "body/episode-retain";
pub const EPISODE_RETAIN_REVISION: &str = "conduit.body/episode-retain@2";
pub const EPISODE_MEMORY_KIND: &str = "body/episode-memory";
pub const EPISODE_MEMORY_REVISION: &str = "conduit.body/episode-memory@2";
pub const EPISODE_PROMPT_SCHEMA: &str = "conduit.body/episode-prompt@1";
pub const MAXIMUM_EPISODE_RECALL: usize = 4;
pub const MAXIMUM_EPISODE_PROMPT_BYTES: usize = 4_096;
pub const MAXIMUM_EPISODE_OBSERVATION_BYTES: usize = 1_024;
pub const MAXIMUM_EPISODE_MEMORY_BYTES: usize = 8_192;
// Persistence must preserve the resident cell's admitted retention policy.
pub const MAXIMUM_EPISODE_CHECKPOINT_BYTES: usize = MAXIMUM_EPISODE_MEMORY_BYTES;
pub const MAXIMUM_EPISODE_RECORD_BYTES: usize = 8_192;
pub const MAXIMUM_EPISODE_RESULT_BYTES: usize =
    conduit_ai::MAXIMUM_MODEL_RESULT_ENVELOPE_BYTES as usize;

#[derive(Debug, Clone, PartialEq, Eq)]
struct EpisodeBody {
    body_id: String,
    wake_id: String,
    wake_sequence: u64,
    display_name: String,
}

/// Begins at most one episode per memory Current. Because the memory cell
/// publishes its next Current only after the previous episode is retained,
/// a second observation waits upstream instead of racing its predecessor.
#[derive(Debug)]
pub struct EpisodeComposer {
    body: Option<EpisodeBody>,
    begun: u64,
    staged: Option<u64>,
    request: Vec<u8>,
    pending: Vec<u8>,
    recall_scores: Vec<u64>,
}

impl Default for EpisodeComposer {
    fn default() -> Self {
        Self::new()
    }
}

impl EpisodeComposer {
    pub fn new() -> Self {
        Self {
            body: None,
            begun: 0,
            staged: None,
            request: Vec::with_capacity(MAXIMUM_EPISODE_PROMPT_BYTES),
            pending: Vec::with_capacity(MAXIMUM_EPISODE_RECORD_BYTES),
            recall_scores: Vec::with_capacity(MAXIMUM_EPISODE_RECALL),
        }
    }

    pub fn has_context(&self) -> bool {
        self.body.is_some()
    }

    pub fn begun(&self) -> u64 {
        self.begun
    }

    /// Admit the typed Body context. A later context may refresh it only for
    /// the same Body and Wake; it cannot move memory to another identity.
    pub fn admit_context(&mut self, encoded: &[u8]) -> Result<(), EpisodeRefusal> {
        let context = crate::decode_body_conversation_context(encoded)
            .map_err(|_| EpisodeRefusal::ContextMalformed)?;
        let body = EpisodeBody {
            body_id: context.body_id.as_str().into(),
            wake_id: context.wake_id.as_str().into(),
            wake_sequence: context.wake_sequence,
            display_name: context.display_name,
        };
        if let Some(current) = &self.body {
            if current.body_id != body.body_id
                || current.wake_id != body.wake_id
                || current.wake_sequence != body.wake_sequence
            {
                return Err(EpisodeRefusal::ContextChangedIdentity);
            }
        }
        self.body = Some(body);
        Ok(())
    }

    /// Check that a memory Current reflects every begun episode.
    pub fn check_memory(
        &self,
        encoded_memory: &[u8],
    ) -> Result<EpisodeMemoryState, EpisodeRefusal> {
        let memory = decode_memory(encoded_memory)?;
        if self.begun != 0 && *memory.completed() != self.begun {
            return Err(EpisodeRefusal::MemoryOutOfOrder);
        }
        if let EpisodeMemoryOwner::Bound(owner) = memory.owner() {
            let body = self.body.as_ref().ok_or(EpisodeRefusal::ContextMissing)?;
            if owner.body_id() != &body.body_id
                || owner.wake_id() != &body.wake_id
                || *owner.wake_sequence() != body.wake_sequence
            {
                return Err(EpisodeRefusal::ContextChangedIdentity);
            }
        }
        Ok(memory)
    }

    /// Stage the bounded model request and pending episode. They become
    /// current only after [`Self::commit`].
    pub fn compose(
        &mut self,
        encoded_observation: &[u8],
        encoded_memory: &[u8],
    ) -> Result<(), EpisodeRefusal> {
        self.staged = None;
        if encoded_observation.len() > MAXIMUM_EPISODE_OBSERVATION_BYTES {
            return Err(EpisodeRefusal::ObservationMalformed);
        }
        let observation = EpisodeObservation::decode(encoded_observation)
            .map_err(|_| EpisodeRefusal::ObservationMalformed)?;
        self.compose_episode(observation, encoded_memory, None)
    }

    /// Compose with evidence JSON already validated and rendered by the caller.
    pub fn compose_episode(
        &mut self,
        observation: EpisodeObservation,
        encoded_memory: &[u8],
        selected_experience: Option<&[u8]>,
    ) -> Result<(), EpisodeRefusal> {
        self.staged = None;
        let memory = self.check_memory(encoded_memory)?;
        let body = self.body.as_ref().ok_or(EpisodeRefusal::ContextMissing)?;
        let episode = memory
            .completed()
            .checked_add(1)
            .ok_or(EpisodeRefusal::Encoding)?;
        if selected_experience.is_some_and(|evidence| evidence.len() > MAXIMUM_EPISODE_PROMPT_BYTES)
        {
            return Err(EpisodeRefusal::PromptBoundExceeded);
        }
        let evidence = selected_experience
            .map(core::str::from_utf8)
            .transpose()
            .map_err(|_| EpisodeRefusal::Encoding)?
            .map(String::from);
        let experience_digest = selected_experience.map(Sha256::digest);
        let selected_experience: Option<serde_json::Value> = selected_experience
            .map(serde_json::from_slice)
            .transpose()
            .map_err(|_| EpisodeRefusal::Encoding)?;
        let selected_experience = selected_experience
            .as_ref()
            .zip(experience_digest.as_ref().map(|digest| &digest[..]));
        let mut memories = episode_recall::select(
            observation.content(),
            memory.memories().get().as_slice(),
            selected_experience.is_some(),
        );
        // The pending record names exactly the memories that fit.
        let mut first = 0;
        let request = loop {
            let prompt = prompt(
                body,
                episode,
                &observation,
                &memories[first..],
                selected_experience,
            );
            let encoded = serde_json::to_vec(&prompt).map_err(|_| EpisodeRefusal::Encoding)?;
            let context_len = episode_prompt::model_context(prompt)?.len();
            if encoded.len().max(context_len) <= MAXIMUM_EPISODE_PROMPT_BYTES {
                break encoded;
            }
            if first == memories.len() {
                return Err(EpisodeRefusal::PromptBoundExceeded);
            }
            if let Some(candidate) = memories[first..].iter_mut().find(|candidate| {
                candidate.trigger_terms > 0 && candidate.historical_evidence.is_some()
            }) {
                candidate.historical_evidence = None;
                candidate.shared_terms = Some(candidate.trigger_terms);
            } else {
                first += 1;
            }
        };
        self.recall_scores.clear();
        self.recall_scores.extend(
            memories[first..]
                .iter()
                .map(|candidate| candidate.shared_terms.unwrap_or(0) as u64),
        );
        let mut recalled = BoundedSequence::new();
        for candidate in &memories[first..] {
            recalled
                .push(
                    EpisodeMemory::new(
                        *candidate.memory.episode(),
                        candidate
                            .historical_evidence
                            .as_ref()
                            .map(serde_json::Value::to_string),
                        candidate.memory.observation().clone(),
                        candidate.memory.outcome().clone(),
                    )
                    .map_err(|_| EpisodeRefusal::Encoding)?,
                )
                .map_err(|_| EpisodeRefusal::Encoding)?;
        }
        let pending = EpisodePending::new(
            body.body_id.clone(),
            episode,
            evidence,
            observation,
            EpisodeRecall::new(recalled).map_err(|_| EpisodeRefusal::Encoding)?,
            body.wake_id.clone(),
            body.wake_sequence,
        )
        .and_then(NativeRustBinding::encode)
        .map_err(|_| EpisodeRefusal::Encoding)?;
        if pending.len() > MAXIMUM_EPISODE_RECORD_BYTES {
            return Err(EpisodeRefusal::Encoding);
        }
        self.request.clear();
        self.request.extend_from_slice(&request);
        self.pending.clear();
        self.pending.extend_from_slice(&pending);
        self.staged = Some(episode);
        Ok(())
    }

    pub fn request(&self) -> &[u8] {
        &self.request
    }

    pub fn pending(&self) -> &[u8] {
        &self.pending
    }

    /// Apply the staged episode after its outputs have been committed.
    pub fn commit(&mut self) {
        if let Some(episode) = self.staged.take() {
            self.begun = episode;
        }
    }

    pub fn cancel(&mut self) {
        self.staged = None;
    }
}

/// The explicit, Play-scoped memory cell. Its Current advances once for
/// every completed episode and keeps at most four selected memories.
#[derive(Debug)]
pub struct EpisodeMemoryCell {
    state: EpisodeMemoryState,
    staged: Option<EpisodeMemoryState>,
    current: Vec<u8>,
    maximum_bytes: usize,
}

impl EpisodeMemoryCell {
    pub fn new() -> Result<Self, EpisodeRefusal> {
        let state = empty_memory()?;
        let mut current = Vec::with_capacity(MAXIMUM_EPISODE_MEMORY_BYTES);
        current.extend_from_slice(
            &state
                .clone()
                .encode()
                .map_err(|_| EpisodeRefusal::Encoding)?,
        );
        Ok(Self {
            state,
            staged: None,
            current,
            maximum_bytes: MAXIMUM_EPISODE_MEMORY_BYTES,
        })
    }

    pub fn current(&self) -> &[u8] {
        &self.current
    }

    pub fn from_checkpoint(encoded: &[u8]) -> Result<Self, EpisodeRefusal> {
        let state = decode_episode_checkpoint(encoded)?;
        let mut current = Vec::with_capacity(MAXIMUM_EPISODE_CHECKPOINT_BYTES);
        current.extend_from_slice(encoded);
        Ok(Self {
            state,
            staged: None,
            current,
            maximum_bytes: MAXIMUM_EPISODE_CHECKPOINT_BYTES,
        })
    }

    /// Stage the next Current for the exact next completed episode.
    pub fn apply(&mut self, encoded_record: &[u8]) -> Result<&[u8], EpisodeRefusal> {
        self.staged = None;
        if encoded_record.len() > MAXIMUM_EPISODE_RECORD_BYTES {
            return Err(EpisodeRefusal::MemoryMalformed);
        }
        let record =
            EpisodeRecord::decode(encoded_record).map_err(|_| EpisodeRefusal::MemoryMalformed)?;
        if Some(*record.episode()) != self.state.completed().checked_add(1) {
            return Err(EpisodeRefusal::MemoryOutOfOrder);
        }
        let owner = EpisodeMemoryOwner::bound(
            record.body_id().clone(),
            record.wake_id().clone(),
            *record.wake_sequence(),
        )
        .map_err(|_| EpisodeRefusal::MemoryMalformed)?;
        if *self.state.completed() != 0 && self.state.owner() != &owner {
            return Err(EpisodeRefusal::ContextChangedIdentity);
        }
        let mut memories = BoundedSequence::<_, MAXIMUM_EPISODE_RECALL>::new();
        let kept = self.state.memories().get().as_slice();
        let skip = usize::from(*record.retained() && kept.len() == MAXIMUM_EPISODE_RECALL);
        for memory in &kept[skip..] {
            memories
                .push(memory.clone())
                .map_err(|_| EpisodeRefusal::Encoding)?;
        }
        if *record.retained() {
            let memory = EpisodeMemory::new(
                *record.episode(),
                record.evidence().as_deref().and_then(|evidence| {
                    episode_recall::compact_evidence(evidence, record.observation().content())
                }),
                record.observation().clone(),
                record.outcome().clone(),
            )
            .map_err(|_| EpisodeRefusal::Encoding)?;
            memories
                .push(memory)
                .map_err(|_| EpisodeRefusal::Encoding)?;
        }
        let (next, encoded) = fit_memory(
            *record.episode(),
            &owner,
            memories.as_slice(),
            self.maximum_bytes,
        )?;
        self.current.clear();
        self.current.extend_from_slice(&encoded);
        self.staged = Some(next);
        Ok(&self.current)
    }

    pub fn commit(&mut self) {
        if let Some(next) = self.staged.take() {
            self.state = next;
        }
    }

    pub fn cancel(&mut self) {
        self.staged = None;
    }
}

fn fit_memory(
    completed: u64,
    owner: &EpisodeMemoryOwner,
    memories: &[EpisodeMemory],
    maximum_bytes: usize,
) -> Result<(EpisodeMemoryState, Vec<u8>), EpisodeRefusal> {
    for skipped in 0..=memories.len() {
        let kept = BoundedSequence::try_from_iter(memories[skipped..].iter().cloned())
            .map_err(|_| EpisodeRefusal::Encoding)?;
        let next = EpisodeMemoryState::new(
            completed,
            EpisodeRecall::new(kept).map_err(|_| EpisodeRefusal::Encoding)?,
            owner.clone(),
        )
        .map_err(|_| EpisodeRefusal::Encoding)?;
        let encoded = next
            .clone()
            .encode()
            .map_err(|_| EpisodeRefusal::Encoding)?;
        if encoded.len() <= maximum_bytes {
            return Ok((next, encoded));
        }
    }
    Err(EpisodeRefusal::Encoding)
}

pub fn decode_episode_checkpoint(bytes: &[u8]) -> Result<EpisodeMemoryState, EpisodeRefusal> {
    if bytes.len() > MAXIMUM_EPISODE_CHECKPOINT_BYTES {
        return Err(EpisodeRefusal::MemoryMalformed);
    }
    decode_memory(bytes)
}

fn decode_memory(bytes: &[u8]) -> Result<EpisodeMemoryState, EpisodeRefusal> {
    if bytes.len() > MAXIMUM_EPISODE_MEMORY_BYTES {
        return Err(EpisodeRefusal::MemoryMalformed);
    }
    let memory = EpisodeMemoryState::decode(bytes).map_err(|_| EpisodeRefusal::MemoryMalformed)?;
    let mut previous = 0;
    for item in memory.memories().get() {
        let episode = *item.episode();
        if episode <= previous || episode > *memory.completed() {
            return Err(EpisodeRefusal::MemoryMalformed);
        }
        previous = episode;
    }
    Ok(memory)
}

fn empty_memory() -> Result<EpisodeMemoryState, EpisodeRefusal> {
    EpisodeMemoryState::new(
        0,
        EpisodeRecall::new(BoundedSequence::new()).map_err(|_| EpisodeRefusal::Encoding)?,
        EpisodeMemoryOwner::Unbound,
    )
    .map_err(|_| EpisodeRefusal::Encoding)
}

/// Canonical initial Current of every episode memory cell.
pub fn initial_episode_memory() -> Vec<u8> {
    empty_memory()
        .and_then(|state| state.encode().map_err(|_| EpisodeRefusal::Encoding))
        .expect("the empty episode memory is a valid bounded value")
}
