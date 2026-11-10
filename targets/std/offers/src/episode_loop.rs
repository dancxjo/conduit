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
