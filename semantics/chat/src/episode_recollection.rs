//! Typed retrieval result consumed by an ordinary model-context Gear.
use super::{
    episode_prompt::prompt, episode_recall::RecallCandidate, EpisodeBody, EpisodeComposer,
    MAXIMUM_EPISODE_PROMPT_BYTES,
};
use crate::{EpisodePending, EpisodeRecallScores, EpisodeRecollection, EpisodeRefusal};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use sha2::{Digest, Sha256};

pub const MAXIMUM_EPISODE_RECOLLECTION_BYTES: usize = 16_384;

impl EpisodeComposer {
    /// Capture exactly the selected records that fit this episode's context bound.
    pub fn recollection(&self) -> Result<Vec<u8>, EpisodeRefusal> {
        let body = self.body.as_ref().ok_or(EpisodeRefusal::ContextMissing)?;
        let pending =
            EpisodePending::decode(&self.pending).map_err(|_| EpisodeRefusal::PendingMalformed)?;
        let scores = EpisodeRecallScores::new(
            BoundedSequence::try_from_iter(self.recall_scores.iter().copied())
                .map_err(|_| EpisodeRefusal::Encoding)?,
        )
        .map_err(|_| EpisodeRefusal::Encoding)?;
        let bytes = EpisodeRecollection::new(body.display_name.clone(), pending, scores)
            .and_then(NativeRustBinding::encode)
            .map_err(|_| EpisodeRefusal::Encoding)?;
        if bytes.len() > MAXIMUM_EPISODE_RECOLLECTION_BYTES {
            return Err(EpisodeRefusal::PromptBoundExceeded);
        }
        Ok(bytes)
    }
}

/// Render the selected records without searching memory or changing selection.
pub fn episode_recollection_request(bytes: &[u8]) -> Result<Vec<u8>, EpisodeRefusal> {
    if bytes.len() > MAXIMUM_EPISODE_RECOLLECTION_BYTES {
        return Err(EpisodeRefusal::PendingMalformed);
    }
    let selected =
        EpisodeRecollection::decode(bytes).map_err(|_| EpisodeRefusal::PendingMalformed)?;
    let pending = selected.pending();
    if selected.scores().get().iter().any(|score| *score > 64)
        || selected.scores().get().len() != pending.recalled().get().len()
        || pending
            .recalled()
            .get()
            .iter()
            .any(|memory| memory.episode() >= pending.episode())
    {
        return Err(EpisodeRefusal::PendingMalformed);
    }
    let body = EpisodeBody {
        body_id: pending.body_id().clone(),
        wake_id: pending.wake_id().clone(),
        wake_sequence: *pending.wake_sequence(),
        display_name: selected.display_name().clone(),
    };
    let parse = |text: &str| {
        serde_json::from_str::<serde_json::Value>(text)
            .map_err(|_| EpisodeRefusal::PendingMalformed)
    };
    let evidence = pending.evidence().as_deref().map(parse).transpose()?;
    let digest = pending
        .evidence()
        .as_deref()
        .map(|text| Sha256::digest(text.as_bytes()));
    let mut recalled = Vec::with_capacity(pending.recalled().get().len());
    for (memory, score) in pending
        .recalled()
        .get()
        .iter()
        .zip(selected.scores().get().iter())
    {
        recalled.push(RecallCandidate {
            memory,
            shared_terms: evidence.as_ref().map(|_| *score as usize),
            trigger_terms: 0,
            historical_evidence: memory.evidence().as_deref().map(parse).transpose()?,
        });
    }
    let request = serde_json::to_vec(&prompt(
        &body,
        *pending.episode(),
        pending.observation(),
        &recalled,
        evidence
            .as_ref()
            .zip(digest.as_ref().map(|digest| &digest[..])),
    ))
    .map_err(|_| EpisodeRefusal::Encoding)?;
    if request.len() > MAXIMUM_EPISODE_PROMPT_BYTES {
        return Err(EpisodeRefusal::PromptBoundExceeded);
    }
    Ok(request)
}
