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
    initial_episode_memory, EpisodeExperience, EpisodeMemoryState, EpisodeObservation,
    EpisodePending, EpisodeRecord, EPISODE_COMPOSE_KIND, EPISODE_COMPOSE_REVISION,
    EPISODE_MEMORY_KIND, EPISODE_MEMORY_REVISION, EPISODE_RETAIN_KIND, EPISODE_RETAIN_REVISION,
    MAXIMUM_BODY_CHAT_CONTEXT_BYTES, MAXIMUM_EPISODE_MEMORY_BYTES,
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
pub const EPISODE_EXPERIENCE_COMPOSE_KIND: &str = "body/episode-compose-experience";
pub const MAXIMUM_EPISODE_EXPERIENCE_BYTES: usize = 131_072;

pub fn episode_experience_type() -> StructuredInfoType {
    EpisodeExperience::semantic_type().expect("native episode experience Type")
}

pub fn episode_experience_compose_semantic_contract() -> Kind {
    let mut contract = episode_compose_semantic_contract();
    contract.kind_id = kind_id(EPISODE_EXPERIENCE_COMPOSE_KIND);
    contract.kind_contract_revision = "conduit.body/episode-compose-experience@2".into();
    contract.inputs[0].port_id = port_id("experience");
    contract.inputs[0].value_kind = type_kind(episode_experience_type());
    contract.limits.max_queue_bytes = MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32;
    contract
}

/// Retrieve selected episodes separately from rendering model context.
pub const EPISODE_RECALL_EXPERIENCE_KIND: &str = "body/episode-recall-experience";
pub fn episode_recollection_type() -> StructuredInfoType {
    crate::EpisodeRecollection::semantic_type().expect("native recollection Type")
}
pub fn episode_recall_experience_contract() -> Kind {
    let mut contract = episode_experience_compose_semantic_contract();
    contract.kind_id = kind_id(EPISODE_RECALL_EXPERIENCE_KIND);
    contract.kind_contract_revision = "conduit.body/episode-recall-experience@2".into();
    contract.outputs[0].port_id = port_id("recollection");
    contract.outputs[0].value_kind = type_kind(episode_recollection_type());
    contract
}
pub fn episode_model_context_contract() -> Kind {
    use PortDirection::{Input, Output};
    kind(
        "body/episode-model-context",
        "conduit.body/episode-model-context@2",
        vec![flow(
            "recollection",
            type_kind(episode_recollection_type()),
            Input,
            true,
        )],
        vec![flow(
            "request",
            kind_id(conduit_ai::GENERATION_REQUEST_VALUE_KIND),
            Output,
            true,
        )],
        crate::MAXIMUM_EPISODE_RECOLLECTION_BYTES,
    )
}

/// Integrate sourced inputs through CurrentExperience before retrieval.
pub fn episode_experience_input_type() -> StructuredInfoType {
    crate::EpisodeExperienceInput::semantic_type().expect("native experience input Type")
}
pub fn episode_experience_integration_contract() -> Kind {
    use PortDirection::{Input, Output};
    kind(
        "experience/integrate-episode",
        "conduit.experience/integrate-episode@1",
        vec![flow(
            "inputs",
            type_kind(episode_experience_input_type()),
            Input,
            true,
        )],
        vec![flow(
            "experience",
            type_kind(episode_experience_type()),
            Output,
            true,
        )],
        MAXIMUM_EPISODE_EXPERIENCE_BYTES,
    )
}

/// One pending episode pairs with one model-derived response.
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

pub const EPISODE_RESUMABLE_MEMORY_KIND: &str = "body/episode-memory-resume";

pub fn episode_resumable_memory_contract(initial: &[u8]) -> Result<Kind, crate::EpisodeRefusal> {
    crate::EpisodeMemoryCell::from_checkpoint(initial)?;
    let mut contract = episode_memory_semantic_contract();
    contract.kind_id = kind_id(EPISODE_RESUMABLE_MEMORY_KIND);
    contract.kind_contract_revision = "conduit.body/episode-memory-resume@2".into();
    for law in &mut contract.semantic_laws {
        if let KindSemanticLaw::InitializedState(law) = law {
            law.initial = initial.to_vec();
            law.current.maximum_bytes = crate::MAXIMUM_EPISODE_CHECKPOINT_BYTES as u32;
        }
    }
    Ok(contract)
}

pub fn install_episode_resumable_memory_catalog(
    initial: &[u8],
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    let contract = episode_resumable_memory_contract(initial)
        .map_err(|error| alloc::format!("episode checkpoint: {error:?}"))?;
    startup.insert(KindSignature {
        kind: contract.kind_id.as_str().into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert_kind(contract)
        .map_err(|error| error.to_string())
}

/// Install after [`crate::install_body_chat_catalog`], which owns the
/// `BodyConversationContext` alias.
pub fn install_episode_loop_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in [
        ("EpisodeObservation", episode_observation_type()),
        ("EpisodeExperience", episode_experience_type()),
        ("EpisodeExperienceInput", episode_experience_input_type()),
        ("EpisodeMemoryState", episode_memory_state_type()),
        ("EpisodePending", episode_pending_type()),
        ("EpisodeRecollection", episode_recollection_type()),
        ("EpisodeRecord", episode_record_type()),
    ] {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    for contract in [
        episode_compose_semantic_contract(),
        episode_experience_compose_semantic_contract(),
        episode_recall_experience_contract(),
        episode_model_context_contract(),
        episode_experience_integration_contract(),
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
