//! Delivered episodes and committed checkpoints share one acknowledgement.
use super::*;
pub(super) struct EpisodeOutput {
    delivered: Option<mpsc::Sender<()>>,
    archive: Option<std::fs::File>,
    journal: Option<EpisodeCheckpointResidence>,
    checkpoint_episode: u64,
    delivered_episode: u64,
    acknowledged_episode: u64,
}

impl ExternalForeOutputAdapter for EpisodeOutput {
    fn deliver(&mut self, delivery: ExternalForeDelivery) -> Result<(), String> {
        if delivery.front_port_id.as_str() == "checkpoints" {
            let journal = self.journal.as_ref().ok_or("unexpected Pete checkpoint")?;
            let memory = conduit_chat::decode_episode_checkpoint(&delivery.bytes)
                .map_err(|error| format!("decode Pete checkpoint: {error:?}"))?;
            journal.commit(&delivery.bytes)?;
            self.checkpoint_episode = *memory.completed();
            self.acknowledge();
            return Ok(());
        }
        let episode = conduit_chat::EpisodeRecord::decode(&delivery.bytes)
            .map_err(|error| format!("decode Pete episode: {error:?}"))?;
        if let Some(archive) = &mut self.archive {
            archive_episode(archive, &episode, &delivery.bytes)?;
        }
        match episode.response() {
            Some(response) => println!("{}", response.content()),
            None => eprintln!(
                "Episode {} ended without a model response.",
                episode.episode()
            ),
        }
        self.delivered_episode = *episode.episode();
        self.acknowledge();
        Ok(())
    }
}

impl EpisodeOutput {
    pub(super) fn new(
        delivered: Option<mpsc::Sender<()>>,
        archive: Option<std::fs::File>,
        journal: Option<EpisodeCheckpointResidence>,
        completed: u64,
    ) -> Self {
        Self {
            delivered,
            archive,
            journal,
            checkpoint_episode: completed,
            delivered_episode: completed,
            acknowledged_episode: completed,
        }
    }
    pub(super) fn acknowledged_episode(&self) -> u64 {
        self.acknowledged_episode
    }

    fn acknowledge(&mut self) {
        let complete = if self.journal.is_some() {
            self.checkpoint_episode.min(self.delivered_episode)
        } else {
            self.delivered_episode
        };
        if complete > self.acknowledged_episode {
            self.acknowledged_episode = complete;
            if let Some(delivered) = &self.delivered {
                let _ = delivered.send(());
            }
        }
    }
}

fn archive_episode(
    archive: &mut std::fs::File,
    episode: &conduit_chat::EpisodeRecord,
    canonical: &[u8],
) -> Result<(), String> {
    use conduit_chat::EpisodeOutcome;
    use std::io::Write;

    let evidence = episode
        .evidence()
        .as_deref()
        .map(serde_json::from_str::<serde_json::Value>)
        .transpose()
        .map_err(|error| format!("decode archived episode evidence: {error}"))?;
    let outcome = match episode.outcome() {
        EpisodeOutcome::Produced(outcome) => {
            let response = outcome.response();
            serde_json::json!({
                "kind": "produced",
                "origin": "model_derived",
                "content": response.content(),
                "implementation": response.implementation(),
                "run": response.run(),
                "truncated": response.truncated(),
            })
        }
        EpisodeOutcome::ProviderLost(outcome) => serde_json::json!({
            "kind": "provider_lost",
            "attempt": outcome.attempt(),
            "implementation": outcome.implementation(),
        }),
    };
    let recalled: Vec<_> = episode
        .recalled()
        .get()
        .iter()
        .map(|item| item.episode())
        .collect();
    let mut line = serde_json::to_vec(&serde_json::json!({
        "schema": "conduit.pete/episode-archive-row@1",
        "body_id": episode.body_id(),
        "wake_id": episode.wake_id(),
        "wake_sequence": episode.wake_sequence(),
        "episode": episode.episode(),
        "observation": {
            "identity": episode.observation().identity(),
            "content": episode.observation().content(),
        },
        "retained": episode.retained(),
        "recalled_episodes": recalled,
        "evidence": evidence,
        "outcome": outcome,
        "canonical_episode_bytes": canonical,
    }))
    .map_err(|error| format!("encode archived episode: {error}"))?;
    line.push(b'\n');
    archive
        .write_all(&line)
        .and_then(|()| archive.sync_data())
        .map_err(|error| format!("commit Pete episode archive row: {error}"))
}
