//! Portable contracts for immutable retrieval custody and grounded context.
use alloc::{
    string::{String, ToString},
    vec,
};
use conduit_core::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature};
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
    let mut kind = contract(
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
    );
    kind.kind_contract_revision = "conduit.body/episode-retrieval-window@2".into();
    kind.outputs.push(port(
        "intent_current",
        conduit_ai::TemporalRetrievalIntent::semantic_type().unwrap(),
        Output,
        true,
    ));
    let defaults = [
        (
            "intent",
            conduit_ai::TemporalRetrievalIntent::EventOrdering
                .encode()
                .unwrap(),
        ),
        (
            "policy",
            conduit_ai::ContextSelectionPolicy::new(
                "configuration/required".into(),
                crate::EPISODE_CONTEXT_BYTE_CHARGE_PROFILE.into(),
                conduit_ai::ContextRedundancyPolicy::KeepAll,
                conduit_ai::ContextOrderingPolicy::Reranked,
                1,
                1,
                1,
                1,
            )
            .unwrap()
            .encode()
            .unwrap(),
        ),
        (
            "clock",
            crate::EpisodeRetrievalClockClass::MonotonicMilliseconds
                .encode()
                .unwrap(),
        ),
    ];
    for (name, bytes) in defaults {
        let value = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
        let profile = value.value_type().profile().unwrap().value_kind().clone();
        kind.startup_parameters.push(FrontStartupParameter {
            name: name.into(),
            value_type: profile.clone(),
            has_default: false,
        });
        kind.configuration.push(KindConfigurationField {
            key: name.into(),
            default_value: ConfigurationValue::Structured(
                StructuredConfigurationValue::new(profile.clone(), bytes).unwrap(),
            ),
            rule: KindConfigurationRule::Structured { profile },
        });
    }
    kind
}

pub fn episode_retrieval_context_contract() -> Kind {
    use PortDirection::*;
    let mut kind = contract(
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
            crate::episode_model_context_contract().outputs[0].clone(),
            port("pending", crate::episode_pending_type(), Output, false),
        ],
    );
    kind.kind_contract_revision = "conduit.body/episode-retrieval-context@3".into();
    let profile = conduit_ai::ContextSelectionPolicy::semantic_type()
        .unwrap()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    let placeholder = conduit_ai::ContextSelectionPolicy::new(
        "configuration/required".into(),
        crate::EPISODE_CONTEXT_BYTE_CHARGE_PROFILE.into(),
        conduit_ai::ContextRedundancyPolicy::KeepAll,
        conduit_ai::ContextOrderingPolicy::Reranked,
        1,
        1,
        1,
        1,
    )
    .unwrap()
    .encode()
    .unwrap();
    kind.startup_parameters = vec![
        FrontStartupParameter {
            name: "relevance_profile".into(),
            value_type: kind_id("value/text"),
            has_default: false,
        },
        FrontStartupParameter {
            name: "policy".into(),
            value_type: profile.clone(),
            has_default: false,
        },
        FrontStartupParameter {
            name: "instruction".into(),
            value_type: kind_id("value/text"),
            has_default: false,
        },
    ];
    kind.configuration = vec![
        KindConfigurationField {
            key: "relevance_profile".into(),
            default_value: ConfigurationValue::Text(String::new()),
            rule: KindConfigurationRule::TextBytes { maximum: 128 },
        },
        KindConfigurationField {
            key: "policy".into(),
            default_value: ConfigurationValue::Structured(
                StructuredConfigurationValue::new(profile.clone(), placeholder).unwrap(),
            ),
            rule: KindConfigurationRule::Structured { profile },
        },
        KindConfigurationField {
            key: "instruction".into(),
            default_value: ConfigurationValue::Text(String::new()),
            rule: KindConfigurationRule::TextBytes { maximum: 1024 },
        },
    ];
    kind
}
pub fn install_episode_retrieval_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    conduit_ai::install_temporal_evidence_flow_catalog(startup, profile)?;
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
            startup_parameters: if kind.kind_id.as_str() == EPISODE_RETRIEVAL_CONTEXT_KIND {
                vec![
                    StartupParameterSignature {
                        name: "relevance_profile".into(),
                        value_type: "Text".into(),
                        default: None,
                    },
                    StartupParameterSignature {
                        name: "policy".into(),
                        value_type: "ContextSelectionPolicy".into(),
                        default: None,
                    },
                    StartupParameterSignature {
                        name: "instruction".into(),
                        value_type: "Text".into(),
                        default: None,
                    },
                ]
            } else {
                [
                    ("intent", "TemporalRetrievalIntent"),
                    ("policy", "ContextSelectionPolicy"),
                    ("clock", "EpisodeRetrievalClockClass"),
                ]
                .into_iter()
                .map(|(name, value_type)| StartupParameterSignature {
                    name: name.into(),
                    value_type: value_type.into(),
                    default: None,
                })
                .collect()
            },
        })?;
        profile.insert_kind(kind).map_err(|e| e.to_string())?;
    }
    Ok(())
}
