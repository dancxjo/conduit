use conduit_core::{Back, BackOfferBuilder, CapabilityOffer, Kind};

pub const EPISODE_COMPOSE_IMPLEMENTATION: &str = "std/body-episode-compose@1";
pub const EPISODE_EXPERIENCE_COMPOSE_IMPLEMENTATION: &str = "std/body-episode-compose-experience@1";
pub const EPISODE_RETAIN_IMPLEMENTATION: &str = "std/body-episode-retain@1";
pub const EPISODE_MEMORY_IMPLEMENTATION: &str = "std/body-episode-memory@1";

/// Pure in-kernel Back: no Host Call, resource, or authority requirement.
fn pure_offer(contract: Kind, name: &str, implementation: &str) -> CapabilityOffer {
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: name.into(),
            execution_profile_id: format!("conduit.std/{name}-kernel@1").into(),
            implementation_id: implementation.into(),
            artifact_id: format!("conduit-std-host/{name}@1").into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

pub fn episode_compose_offer() -> CapabilityOffer {
    pure_offer(
        conduit_chat::episode_compose_semantic_contract(),
        "body-episode-compose",
        EPISODE_COMPOSE_IMPLEMENTATION,
    )
}

pub fn episode_experience_compose_offer() -> CapabilityOffer {
    pure_offer(
        conduit_chat::episode_experience_compose_semantic_contract(),
        "body-episode-compose-experience",
        EPISODE_EXPERIENCE_COMPOSE_IMPLEMENTATION,
    )
}

pub fn episode_retain_offer() -> CapabilityOffer {
    pure_offer(
        conduit_chat::episode_retain_semantic_contract(),
        "body-episode-retain",
        EPISODE_RETAIN_IMPLEMENTATION,
    )
}

pub fn episode_memory_offer() -> CapabilityOffer {
    pure_offer(
        conduit_chat::episode_memory_semantic_contract(),
        "body-episode-memory",
        EPISODE_MEMORY_IMPLEMENTATION,
    )
}

pub const EPISODE_RESUMABLE_MEMORY_IMPLEMENTATION: &str = "std/body-episode-memory-resume@1";

pub fn episode_resumable_memory_offer(
    initial: &[u8],
) -> Result<CapabilityOffer, conduit_chat::EpisodeRefusal> {
    Ok(pure_offer(
        conduit_chat::episode_resumable_memory_contract(initial)?,
        "body-episode-memory-resume",
        EPISODE_RESUMABLE_MEMORY_IMPLEMENTATION,
    ))
}

pub const EPISODE_RECALL_EXPERIENCE_IMPLEMENTATION: &str = "std/body-episode-recall-experience@1";
pub const EPISODE_MODEL_CONTEXT_IMPLEMENTATION: &str = "std/body-episode-model-context@1";

pub fn episode_recall_experience_offer() -> CapabilityOffer {
    pure_offer(
        conduit_chat::episode_recall_experience_contract(),
        "body-episode-recall-experience",
        EPISODE_RECALL_EXPERIENCE_IMPLEMENTATION,
    )
}
pub fn episode_model_context_offer() -> CapabilityOffer {
    pure_offer(
        conduit_chat::episode_model_context_contract(),
        "body-episode-model-context",
        EPISODE_MODEL_CONTEXT_IMPLEMENTATION,
    )
}

pub const EPISODE_EXPERIENCE_INTEGRATION_IMPLEMENTATION: &str =
    "std/experience-integrate-episode@1";
pub fn episode_experience_integration_offer() -> CapabilityOffer {
    pure_offer(
        conduit_chat::episode_experience_integration_contract(),
        "experience-integrate-episode",
        EPISODE_EXPERIENCE_INTEGRATION_IMPLEMENTATION,
    )
}

pub const EPISODE_RETRIEVAL_PRODUCER_IMPLEMENTATION: &str = "std/body-episode-retrieval-window@2";
pub const EPISODE_RETRIEVAL_CONTEXT_IMPLEMENTATION: &str = "std/body-episode-retrieval-context@2";
pub fn episode_retrieval_producer_offer() -> CapabilityOffer {
    pure_offer(
        conduit_chat::episode_retrieval_producer_contract(),
        "body-episode-retrieval-window",
        EPISODE_RETRIEVAL_PRODUCER_IMPLEMENTATION,
    )
}
pub fn episode_retrieval_context_offer() -> CapabilityOffer {
    pure_offer(
        conduit_chat::episode_retrieval_context_contract(),
        "body-episode-retrieval-context",
        EPISODE_RETRIEVAL_CONTEXT_IMPLEMENTATION,
    )
}
