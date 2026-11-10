//! Exact kind contracts for the bounded Body episode loop.
//!
//! None of these kinds names a host, transport, device, or binding. The memory
//! cell is an initialized-state kind: its `update` input is the one declared
//! delay that lets ordinary Plot feedback close the loop.

use alloc::{string::String, string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, InitializedStateLaw, Kind, KindId,
    KindSemanticLaw, PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
    TemporalStateBehavior,
};
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog};

use crate::{
    initial_episode_memory, EpisodeMemoryState, EpisodeObservation, EpisodePending, EpisodeRecord,
    EPISODE_COMPOSE_KIND, EPISODE_COMPOSE_REVISION, EPISODE_MEMORY_KIND, EPISODE_MEMORY_REVISION,
    EPISODE_RETAIN_KIND, EPISODE_RETAIN_REVISION, MAXIMUM_BODY_CHAT_CONTEXT_BYTES,
    MAXIMUM_EPISODE_MEMORY_BYTES,
};

/// Admission queue bound shared by the episode kinds; it admits the largest
/// Body context, record, and memory value with headroom for planned queues.
const EPISODE_QUEUE_BYTES: usize = MAXIMUM_BODY_CHAT_CONTEXT_BYTES;

fn type_kind(value_type: StructuredInfoType) -> KindId {
    value_type
        .profile()
        .expect("bounded episode Type")
        .value_kind()
        .clone()
}

pub fn episode_observation_type() -> StructuredInfoType {
    EpisodeObservation::semantic_type().expect("native episode observation Type")
}

pub fn episode_memory_state_type() -> StructuredInfoType {
    EpisodeMemoryState::semantic_type().expect("native episode memory Type")
}

pub fn episode_pending_type() -> StructuredInfoType {
    EpisodePending::semantic_type().expect("native pending episode Type")
}

pub fn episode_record_type() -> StructuredInfoType {
    EpisodeRecord::semantic_type().expect("native episode record Type")
}

fn flow(name: &str, value_kind: KindId, direction: PortDirection, closes: bool) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind,
        direction,
        temporal: PortTemporal::Flow { closes },
        abnormal_kind: None,
    }
}

fn kind(
    id: &str,
    revision: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    queue_bytes: usize,
) -> Kind {
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(id),
        kind_contract_revision: revision.into(),
        inputs,
        outputs,
        configuration: Default::default(),
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: queue_bytes as u32,
        },
    }
}

/// One observation plus the memory Current that reflects every earlier episode
/// becomes one bounded model request and one typed pending episode.
pub fn episode_compose_semantic_contract() -> Kind {
    use PortDirection::{Input, Output};
    kind(
        EPISODE_COMPOSE_KIND,
        EPISODE_COMPOSE_REVISION,
        vec![
            flow(
                "observation",
                type_kind(episode_observation_type()),
                Input,
                true,
            ),
            flow(
                "memory",
                type_kind(episode_memory_state_type()),
                Input,
                false,
            ),
            flow(
                "context",
                kind_id(conduit_body::BODY_CONVERSATION_CONTEXT_VALUE_KIND),
                Input,
                true,
            ),
        ],
        vec![
            flow(
                "request",
                kind_id(conduit_ai::GENERATION_REQUEST_VALUE_KIND),
                Output,
                true,
            ),
            flow("pending", type_kind(episode_pending_type()), Output, true),
        ],
        EPISODE_QUEUE_BYTES,
    )
}

/// One pending episode plus its exact model-derived result becomes one record.
pub fn episode_retain_semantic_contract() -> Kind {
    use PortDirection::{Input, Output};
    kind(
        EPISODE_RETAIN_KIND,
        EPISODE_RETAIN_REVISION,
        vec![
            flow("pending", type_kind(episode_pending_type()), Input, true),
            flow(
                "response",
                kind_id(conduit_ai::GENERATED_RESULT_VALUE_KIND),
                Input,
                true,
            ),
        ],
        vec![flow(
            "episode",
            type_kind(episode_record_type()),
            Output,
            false,
        )],
        EPISODE_QUEUE_BYTES.max(crate::MAXIMUM_EPISODE_RESULT_BYTES),
    )
}

/// Initialized memory cell: publishes the empty memory, then one Current for
/// each completed episode record, retaining only selected episodes.
pub fn episode_memory_semantic_contract() -> Kind {
    let memory = type_kind(episode_memory_state_type());
    let mut contract = kind(
        EPISODE_MEMORY_KIND,
        EPISODE_MEMORY_REVISION,
        vec![flow(
            "update",
            type_kind(episode_record_type()),
            PortDirection::Input,
            false,
        )],
        vec![flow(
            "current",
            memory.clone(),
            PortDirection::Output,
            false,
        )],
        EPISODE_QUEUE_BYTES,
    );
    contract.semantic_laws = vec![
        KindSemanticLaw::TemporalState(TemporalStateBehavior::Retained),
        KindSemanticLaw::InitializedState(InitializedStateLaw {
            update_port_id: port_id("update"),
            current_port_id: port_id("current"),
            current: CheckedValueContract::new(memory, MAXIMUM_EPISODE_MEMORY_BYTES as u32, vec![])
                .expect("finite episode memory contract"),
            initial: initial_episode_memory(),
        }),
    ];
    contract
}

/// Install after [`crate::install_body_chat_catalog`], which owns the
/// `BodyConversationContext` alias.
pub fn install_episode_loop_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in [
        ("EpisodeObservation", episode_observation_type()),
        ("EpisodeMemoryState", episode_memory_state_type()),
        ("EpisodePending", episode_pending_type()),
        ("EpisodeRecord", episode_record_type()),
    ] {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    for contract in [
        episode_compose_semantic_contract(),
        episode_retain_semantic_contract(),
        episode_memory_semantic_contract(),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().into(),
            startup_parameters: Vec::new(),
        })?;
        profile
            .insert_kind(contract)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
