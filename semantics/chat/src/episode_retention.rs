//! Retain selected observations and exact actual outcomes.
use super::*;

/// Pairs one pending episode with its exact model-derived result.
#[derive(Debug)]
pub struct EpisodeRetainer {
    record: Vec<u8>,
}

impl Default for EpisodeRetainer {
    fn default() -> Self {
        Self::new()
    }
}

impl EpisodeRetainer {
    pub fn new() -> Self {
        Self {
            record: Vec::with_capacity(MAXIMUM_EPISODE_RECORD_BYTES),
        }
    }

    pub fn retain(
        &mut self,
        encoded_pending: &[u8],
        encoded_result: &[u8],
    ) -> Result<&[u8], EpisodeRefusal> {
        if encoded_pending.len() > MAXIMUM_EPISODE_RECORD_BYTES {
            return Err(EpisodeRefusal::PendingMalformed);
        }
        let pending = EpisodePending::decode(encoded_pending)
            .map_err(|_| EpisodeRefusal::PendingMalformed)?;
        let outcome = decode_outcome(encoded_result)?;
        let retained = *pending.observation().retain();
        let encoded = EpisodeRecord::new(
            pending.body_id().clone(),
            *pending.episode(),
            pending.evidence().clone(),
            pending.observation().clone(),
            outcome,
            pending.recalled().clone(),
            retained,
            pending.wake_id().clone(),
            *pending.wake_sequence(),
        )
        .and_then(NativeRustBinding::encode)
        .map_err(|_| EpisodeRefusal::Encoding)?;
        if encoded.len() > MAXIMUM_EPISODE_RECORD_BYTES {
            return Err(EpisodeRefusal::Encoding);
        }
        self.record.clear();
        self.record.extend_from_slice(&encoded);
        Ok(&self.record)
    }

    pub fn record(&self) -> &[u8] {
        &self.record
    }
}
