//! Hosted residence for the committed checkpoint Fore of an episode Plot.
use crate::state_value::{DurableStateBinding, FileDurableStateResidence, RecoveryDisposition};
use conduit_body::BodyConversationContext;
use conduit_chat::{
    decode_episode_checkpoint, episode_memory_state_type, initial_episode_memory,
    EpisodeMemoryOwner, MAXIMUM_EPISODE_CHECKPOINT_BYTES,
};
use std::path::Path;

/// An application-selected Body and Wake journal. The Plot and scheduler
/// remain responsible for producing each committed checkpoint.
pub struct EpisodeCheckpointResidence {
    journal: FileDurableStateResidence,
    wake: String,
    wake_sequence: u64,
}

impl EpisodeCheckpointResidence {
    pub fn open(root: &Path, context: &BodyConversationContext) -> Result<Self, String> {
        let wake = context.wake_id.as_str().to_owned();
        let journal = FileDurableStateResidence::open(
            root,
            DurableStateBinding {
                body: context.body_id.as_str().into(),
                state: format!("body/episode-checkpoint/{wake}").into(),
                value_kind: episode_memory_state_type()
                    .profile()
                    .map_err(|error| format!("episode memory profile: {error:?}"))?
                    .value_kind()
                    .clone(),
                maximum_value_bytes: MAXIMUM_EPISODE_CHECKPOINT_BYTES as u32,
            },
        )
        .map_err(|error| format!("open episode residence: {error:?}"))?;
        Ok(Self {
            journal,
            wake,
            wake_sequence: context.wake_sequence,
        })
    }

    pub fn recover_seed(&self) -> Result<Vec<u8>, String> {
        match self
            .journal
            .recover_value()
            .map_err(|error| format!("recover episode checkpoint: {error:?}"))?
        {
            (RecoveryDisposition::Absent, None) => Ok(initial_episode_memory()),
            (RecoveryDisposition::Recovered { generation }, Some(bytes)) => {
                if self.generation(&bytes)? != generation {
                    return Err("checkpoint generation differs from its episode count".into());
                }
                Ok(bytes)
            }
            _ => Err("inconsistent episode checkpoint recovery".into()),
        }
    }

    pub fn commit(&self, checkpoint: &[u8]) -> Result<(), String> {
        let generation = self.generation(checkpoint)?;
        if generation == 0 {
            if self.recover_seed()? != checkpoint {
                return Err("empty episode checkpoint cannot replace committed history".into());
            }
        } else {
            self.journal
                .commit(generation, checkpoint)
                .map_err(|error| format!("commit episode checkpoint: {error:?}"))?;
        }
        Ok(())
    }

    fn generation(&self, bytes: &[u8]) -> Result<u64, String> {
        let state = decode_episode_checkpoint(bytes)
            .map_err(|error| format!("invalid episode checkpoint: {error:?}"))?;
        if let EpisodeMemoryOwner::Bound(owner) = state.owner() {
            if owner.body_id() != &self.journal.binding().body
                || owner.wake_id() != &self.wake
                || *owner.wake_sequence() != self.wake_sequence
            {
                return Err("episode checkpoint belongs to a different Body or Wake".into());
            }
        }
        Ok(*state.completed())
    }
}
