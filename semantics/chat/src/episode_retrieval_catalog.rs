//! Portable contracts for immutable retrieval custody and grounded context.
use alloc::{
    string::{String, ToString},
    vec,
};
use conduit_core::*;
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog};
pub const EPISODE_RETRIEVAL_PRODUCER_KIND: &str = "body/episode-retrieval-window";
pub const EPISODE_RETRIEVAL_CONTEXT_KIND: &str = "body/episode-retrieval-context";
fn port(
    name: &str,
    ty: StructuredInfoType,
    direction: PortDirection,
    current: bool,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: ty.profile().unwrap().value_kind().clone(),
        direction,
        temporal: if current {
            PortTemporal::Current
        } else {
            PortTemporal::Flow { closes: true }
        },
        abnormal_kind: None,
    }
}
fn contract(
    id: &str,
    inputs: alloc::vec::Vec<PortDescriptor>,
    outputs: alloc::vec::Vec<PortDescriptor>,
) -> Kind {
    Kind {
        kind_id: kind_id(id),
        kind_contract_revision: alloc::format!("conduit.{id}@1").into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs,
        outputs,
        configuration: vec![],
        semantic_laws: vec![KindSemanticLaw::ExternalEffects(
            ExternalEffectBehavior::None,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: crate::MAXIMUM_EPISODE_RETRIEVAL_CAPTURE_BYTES as u32,
        },
    }
}
pub fn episode_retrieval_producer_contract() -> Kind {
    use PortDirection::*;
    let mut context = crate::episode_recall_experience_contract().inputs[2].clone();
    context.port_id = port_id("context");
    let mut memory = port("memory", crate::episode_memory_state_type(), Input, false);
    memory.temporal = PortTemporal::Flow { closes: false };
    contract(
        EPISODE_RETRIEVAL_PRODUCER_KIND,
        vec![
            port("experience", crate::episode_experience_type(), Input, false),
            memory,
            context,
            port(
                "intent",
                conduit_ai::TemporalRetrievalIntent::semantic_type().unwrap(),
                Input,
                true,
            ),
            port(
                "policy",
                conduit_ai::ContextSelectionPolicy::semantic_type().unwrap(),
                Input,
                true,
            ),
            port(
                "clock",
                crate::episode_retrieval_clock_class_type(),
                Input,
                true,
            ),
        ],
        vec![
            port(
                "window",
                conduit_ai::temporal_evidence_window_type(),
                Output,
                false,
            ),
            port(
                "capture",
                crate::episode_retrieval_capture_type(),
                Output,
                false,
            ),
        ],
    )
}
pub fn episode_retrieval_context_contract() -> Kind {
    use PortDirection::*;
    contract(
        EPISODE_RETRIEVAL_CONTEXT_KIND,
        vec![
            port(
                "capture",
                crate::episode_retrieval_capture_type(),
                Input,
                false,
            ),
            port(
                "outcome",
                conduit_ai::temporal_evidence_outcome_type(),
                Input,
                false,
            ),
        ],
        vec![
            port(
                "recollection",
                crate::episode_recollection_type(),
                Output,
                false,
            ),
            port("pending", crate::episode_pending_type(), Output, false),
        ],
    )
}
pub fn install_episode_retrieval_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    for (name, ty) in [
        (
            "EpisodeRetrievalCapture",
            crate::episode_retrieval_capture_type(),
        ),
        (
            "EpisodeRetrievalClockClass",
            crate::episode_retrieval_clock_class_type(),
        ),
        (
            "ContextSelectionPolicy",
            conduit_ai::ContextSelectionPolicy::semantic_type().unwrap(),
        ),
    ] {
        startup
            .ensure_structured_type(name, ty)
            .map_err(|e| e.to_string())?;
    }
    for kind in [
        episode_retrieval_producer_contract(),
        episode_retrieval_context_contract(),
    ] {
        startup.insert(KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })?;
        profile.insert_kind(kind).map_err(|e| e.to_string())?;
    }
    Ok(())
}
