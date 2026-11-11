//! Shared native fixtures for independent snapshot/window contract tests.
#![allow(dead_code)]
use conduit_chat::episode_memory_state_type;
use conduit_chat::{EpisodeRetrievalOwner, EpisodeRetrievalSnapshotInput};
use conduit_core::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use sha2::{Digest, Sha256};
pub(crate) struct Fixture {
    pub(crate) memory: Vec<u8>,
    pub(crate) experience: Vec<u8>,
    pub(crate) intent: Vec<u8>,
    pub(crate) policy: Vec<u8>,
    pub(crate) source: BoundedResourceRef,
}
impl Fixture {
    pub(crate) fn new() -> Self {
        let memory = memory(&[1, 2], "body/one", "wake/one");
        let selection = conduit_human::ExperienceEvidenceSelection::new(
            conduit_human::ExperienceEvidenceIdentities::new(BoundedSequence::new()).unwrap(),
            conduit_human::ExperienceEvidenceIdentity::new("revision/one".into()).unwrap(),
            conduit_human::ExperienceEvidenceRelationships::new(BoundedSequence::new()).unwrap(),
            1,
        )
        .unwrap();
        let evidence = conduit_human::ExperienceEvidenceRequest::new(
            conduit_human::ExperienceEvidenceItems::new(BoundedSequence::new()).unwrap(),
            selection,
        )
        .unwrap();
        let experience = conduit_chat::EpisodeExperience::new(
            evidence,
            conduit_chat::EpisodeObservation::new(
                "What happened?".into(),
                "trigger/one".into(),
                true,
            )
            .unwrap(),
        )
        .unwrap();
        let intent = conduit_ai::TemporalRetrievalIntent::EventOrdering
            .encode()
            .unwrap();
        let policy = conduit_ai::ContextSelectionPolicy::new(
            "policy/one".into(),
            "tokens/bytes@1".into(),
            conduit_ai::ContextRedundancyPolicy::KeepAll,
            conduit_ai::ContextOrderingPolicy::Reranked,
            4,
            4096,
            4096,
            16,
        )
        .unwrap()
        .encode()
        .unwrap();
        let source = source(&memory);
        Self {
            memory,
            experience: experience.encode().unwrap(),
            intent,
            policy,
            source,
        }
    }
    pub(crate) fn input(&self) -> EpisodeRetrievalSnapshotInput<'_> {
        EpisodeRetrievalSnapshotInput {
            memory: &self.memory,
            experience: &self.experience,
            expected: EpisodeRetrievalOwner {
                body_id: "body/one",
                wake_id: "wake/one",
                wake_sequence: 3,
            },
            reference: time(100),
            retrieved: time(99),
            clock_basis: conduit_ai::BorrowedTemporalClockBasis::MonotonicMilliseconds("clock/one"),
            intent: &self.intent,
            context_policy: &self.policy,
            source: &self.source,
        }
    }
}
pub(crate) fn time(ticks: u64) -> BorrowedTemporalInstant<'static> {
    BorrowedTemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "clock/one",
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}
pub(crate) fn memory(episodes: &[u64], body: &str, wake: &str) -> Vec<u8> {
    let rows: Vec<_> = episodes.iter().map(|episode| (*episode, None)).collect();
    memory_with_evidence(&rows, body, wake)
}
pub(crate) fn memory_with_evidence(
    rows: &[(u64, Option<&str>)],
    body: &str,
    wake: &str,
) -> Vec<u8> {
    let memories = rows.iter().map(|(episode, evidence)| {
        let outcome = if *episode == 1 {
            conduit_chat::EpisodeOutcome::produced(
                conduit_chat::EpisodeModelResponse::new(
                    "Earlier model words".into(),
                    "provider/one".into(),
                    "run/one".into(),
                    false,
                )
                .unwrap(),
            )
            .unwrap()
        } else {
            conduit_chat::EpisodeOutcome::provider_lost(
                format!("attempt/{episode}"),
                "provider/one".into(),
            )
            .unwrap()
        };
        conduit_chat::EpisodeMemory::new(
            *episode,
            evidence.map(str::to_owned),
            conduit_chat::EpisodeObservation::new(
                "Actual earlier report".into(),
                format!("observation/{episode}"),
                true,
            )
            .unwrap(),
            outcome,
        )
        .unwrap()
    });
    conduit_chat::EpisodeMemoryState::new(
        rows.iter().map(|row| row.0).max().unwrap_or(2),
        conduit_chat::EpisodeRecall::new(BoundedSequence::try_from_iter(memories).unwrap())
            .unwrap(),
        conduit_chat::EpisodeMemoryOwner::bound(body.into(), wake.into(), 3).unwrap(),
    )
    .unwrap()
    .encode()
    .unwrap()
}
pub(crate) fn source(bytes: &[u8]) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: episode_memory_state_type()
            .profile()
            .unwrap()
            .value_kind()
            .clone(),
        access_class: ResourceClassId::from("resource/episode-memory-snapshot"),
        extent: ResourceExtent {
            bytes: bytes.len() as u64,
            items: None,
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest(Sha256::digest(bytes).into()),
            expires_at: None,
        },
    }
}
