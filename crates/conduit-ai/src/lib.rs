//! Bounded domain-owned chat contracts and provider implementations.
//!
//! Chat is an ordinary semantic node. Implementations, model and adapter
//! artifacts, host observations, capabilities, resources, grants, exact-plan
//! bindings, execution evidence, and presentation projections remain distinct.
//! Registering these contracts performs no discovery, download, network
//! access, model load, or authority acquisition.

use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};

use conduit_core::{
    ArtifactDigest, CanonicalDescriptor, CanonicalValue, ConfigContract, ConfigFieldContract,
    ConfigIdentity, ConfigMutability, ConfigRequirement, ConnectionCardinality, Delivery,
    Direction, ExecutorKind, FieldDisposition, Id, LossAcceptance, MapField, NodeContract,
    PinnedDescriptor, PlanResourceBudget, PortContract, PortFlowConstraints, Presence,
    SemanticHash, Sensitivity, TemporalContract, TerminalContract, TypeContractRef,
    ValueCardinality,
};
use conduit_panel::{Node, SourceValue};
#[cfg(not(target_arch = "wasm32"))]
use conduit_runtime::ExactHostedServiceBinding;
use conduit_runtime::{
    CompiledInHostService, Handler, InstalledArtifactRegistration, InstalledCapabilityRequirement,
    InstalledImplementationRegistration, Registry, RegistryError, ResolutionError, RunIo,
    RuntimeError, Value,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

#[cfg(not(target_arch = "wasm32"))]
mod ollama;
#[cfg(not(target_arch = "wasm32"))]
pub use ollama::{
    OllamaInstallation, OllamaObservation, OllamaObservationError, OllamaObserver,
    install_observed_ollama_implementation,
};

pub const CHAT_SCHEMA_VERSION: u32 = 0;
pub const QUICK_LOCAL_MODE: &str = "conduit.ai/chat/quick-local";
pub const CHAT_PROFILE_ID: &str = "conduit.ai/chat/provider-profile";
pub const LOCAL_CHAT_AUTHORITY: SemanticHash = SemanticHash::from_bytes([0xa1; 32]);
pub const LOCAL_CHAT_ACTION: &str = "conduit.action/ai-chat-local";
pub const LOCAL_CHAT_RESOURCE_KIND: &str = "conduit.resource/local-model-service";
pub const ENDPOINT_CONSTRAINT: &str = "conduit.constraint/ai-chat-endpoint";
pub const MODEL_CONSTRAINT: &str = "conduit.constraint/ai-chat-model-artifact";
pub const PROFILE_CONSTRAINT: &str = "conduit.constraint/ai-chat-provider-profile";

pub const MAXIMUM_MESSAGE_BYTES: usize = 16 * 1024;
pub const MAXIMUM_CONTEXT_BYTES: usize = 32 * 1024;
pub const MAXIMUM_OUTPUT_BYTES: usize = 64 * 1024;
pub const MAXIMUM_CHUNK_BYTES: usize = 16 * 1024;
pub const MAXIMUM_CHUNKS: u16 = 64;
pub const MAXIMUM_CONCURRENCY: u16 = 32;
pub const MAXIMUM_TIMEOUT_MILLIS: u64 = 120_000;
pub const MAXIMUM_EVIDENCE_EVENTS: usize = 128;

pub const CHAT_CONTEXT_DESCRIPTOR: &str =
    "conduit.ai/chat/context|0|caller-supplied|bounded|no-retained-state";
pub const CHAT_RESULT_DESCRIPTOR: &str =
    "conduit.ai/chat/result|0|terminal|reason|chunks|bytes|provider|artifact";

pub const CHAT_CONTEXT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("ai/chat/context"),
    schema_version: CHAT_SCHEMA_VERSION,
    semantic_hash: SemanticHash::from_bytes([
        0xbb, 0x6e, 0xd3, 0x52, 0x24, 0x63, 0x27, 0x63, 0x38, 0x35, 0xce, 0x20, 0x15, 0x81, 0x50,
        0x06, 0x8c, 0x1d, 0xdb, 0x4b, 0xcf, 0xf4, 0x15, 0x76, 0xdb, 0x04, 0x16, 0x17, 0x9c, 0xfc,
        0x52, 0x8a,
    ]),
};
pub const CHAT_RESULT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("ai/chat/result"),
    schema_version: CHAT_SCHEMA_VERSION,
    semantic_hash: SemanticHash::from_bytes([
        0x54, 0xe9, 0x2b, 0x27, 0x10, 0xd8, 0x52, 0xcd, 0xd3, 0x4b, 0xb3, 0x05, 0x56, 0x05, 0x52,
        0x02, 0xc8, 0x15, 0x49, 0xb4, 0x69, 0xd3, 0x9b, 0xea, 0x98, 0x4e, 0x1d, 0x83, 0xfc, 0x87,
        0x85, 0xd9,
    ]),
};
const TEXT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/text"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x94, 0xdf, 0xe2, 0x55, 0x09, 0xfe, 0x62, 0x4d, 0x89, 0x74, 0xb1, 0xdd, 0x44, 0x2e, 0xb7,
        0xf9, 0x6f, 0x7e, 0x62, 0x1e, 0x6e, 0x71, 0xf0, 0x35, 0xac, 0x6f, 0x08, 0x04, 0x63, 0x61,
        0x80, 0x72,
    ]),
};
const U64_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/u64"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xf9, 0xba, 0xd3, 0xea, 0x53, 0xd3, 0xca, 0x01, 0xa0, 0xa4, 0xd6, 0x9f, 0x86, 0xc8, 0x25,
        0x65, 0x17, 0x07, 0x16, 0x45, 0xea, 0x7d, 0x68, 0xef, 0x63, 0x6b, 0x6d, 0x94, 0x87, 0x70,
        0xf0, 0xec,
    ]),
};

const fn config_field(
    key: &'static str,
    value_type: TypeContractRef<'static>,
) -> ConfigFieldContract<'static> {
    ConfigFieldContract {
        key: Id(key),
        value_type,
        requirement: ConfigRequirement::Required,
        sensitivity: Sensitivity::Public,
        mutability: ConfigMutability::PreStart,
        identity: ConfigIdentity::Semantic,
    }
}

const CHAT_CONFIG: [ConfigFieldContract<'static>; 13] = [
    config_field("mode", TEXT_TYPE),
    config_field("maximum_message_bytes", U64_TYPE),
    config_field("maximum_context_bytes", U64_TYPE),
    config_field("maximum_output_bytes", U64_TYPE),
    config_field("maximum_chunk_bytes", U64_TYPE),
    config_field("maximum_chunks", U64_TYPE),
    config_field("maximum_concurrency", U64_TYPE),
    config_field("timeout_millis", U64_TYPE),
    config_field("sensitivity", TEXT_TYPE),
    config_field("conversation_state", TEXT_TYPE),
    config_field("retention", TEXT_TYPE),
    config_field("tools", TEXT_TYPE),
    config_field("structured_output", TEXT_TYPE),
];

const fn finite_port(
    id: &'static str,
    direction: Direction,
    value_type: TypeContractRef<'static>,
    presence: Presence,
    connections: ConnectionCardinality,
    values: ValueCardinality,
    sensitivity: Sensitivity,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type,
        presence,
        connections,
        values,
        delivery: Delivery::FiniteBatch,
        temporal: TemporalContract::Atemporal,
        terminal: TerminalContract::Finite,
        sensitivity,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

const fn stream_port(
    id: &'static str,
    direction: Direction,
    value_type: TypeContractRef<'static>,
    presence: Presence,
    connections: ConnectionCardinality,
    values: ValueCardinality,
    sensitivity: Sensitivity,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type,
        presence,
        connections,
        values,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: TerminalContract::Finite,
        sensitivity,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

const CHAT_INPUTS: [PortContract<'static>; 2] = [
    finite_port(
        "message",
        Direction::Input,
        TEXT_TYPE,
        Presence::Required,
        ConnectionCardinality::ExactlyOne,
        ValueCardinality::ExactlyOne,
        Sensitivity::Public,
    ),
    finite_port(
        "context",
        Direction::Input,
        CHAT_CONTEXT_TYPE,
        Presence::Optional,
        ConnectionCardinality::ZeroOrOne,
        ValueCardinality::ZeroOrOne,
        Sensitivity::Restricted,
    ),
];
const CHAT_OUTPUTS: [PortContract<'static>; 2] = [
    stream_port(
        "chunks",
        Direction::Output,
        TEXT_TYPE,
        Presence::Optional,
        ConnectionCardinality::ZeroOrMore,
        ValueCardinality::ZeroOrMore,
        Sensitivity::Restricted,
    ),
    finite_port(
        "result",
        Direction::Output,
        CHAT_RESULT_TYPE,
        Presence::Optional,
        ConnectionCardinality::ZeroOrMore,
        ValueCardinality::ExactlyOne,
        Sensitivity::Public,
    ),
];
const RESULT_INPUT: [PortContract<'static>; 1] = [finite_port(
    "result",
    Direction::Input,
    CHAT_RESULT_TYPE,
    Presence::Required,
    ConnectionCardinality::ExactlyOne,
    ValueCardinality::ExactlyOne,
    Sensitivity::Public,
)];
const SUMMARY_OUTPUT: [PortContract<'static>; 1] = [finite_port(
    "summary",
    Direction::Output,
    TEXT_TYPE,
    Presence::Required,
    ConnectionCardinality::OneOrMore,
    ValueCardinality::ExactlyOne,
    Sensitivity::Public,
)];

pub const CHAT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("ai/chat"),
    config: ConfigContract {
        fields: &CHAT_CONFIG,
    },
    inputs: &CHAT_INPUTS,
    outputs: &CHAT_OUTPUTS,
};
pub const CHAT_RESULT_INSPECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("ai/chat/result/inspect"),
    config: ConfigContract { fields: &[] },
    inputs: &RESULT_INPUT,
    outputs: &SUMMARY_OUTPUT,
};
pub const CHAT_CONTRACTS: [&NodeContract<'static>; 2] =
    [&CHAT_CONTRACT, &CHAT_RESULT_INSPECT_CONTRACT];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChatNetworkRequirement {
    None,
    LoopbackOnly,
}

impl ChatNetworkRequirement {
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::LoopbackOnly => "loopback-only",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChatProviderProfile {
    pub schema_version: u32,
    pub identity: String,
    pub semantic_mode: String,
    pub model_artifact_id: String,
    pub model_artifact_digest: String,
    pub model_format: String,
    pub model_family: String,
    pub model_parameter_profile: String,
    pub model_quantization: String,
    pub maximum_message_bytes: usize,
    pub maximum_context_bytes: usize,
    pub maximum_output_bytes: usize,
    pub maximum_chunk_bytes: usize,
    pub maximum_chunks: u16,
    pub maximum_concurrency: u16,
    pub streaming: bool,
    pub compute_profile: String,
    pub latency_objective_millis: u64,
    pub latency_evidence_window_requests: u32,
    pub locality: String,
    pub network_requirement: ChatNetworkRequirement,
    pub retention: String,
    pub accepted_sensitivity: String,
    pub tool_use: bool,
    pub structured_output: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatProviderProfileInput {
    pub model_artifact_id: String,
    pub model_artifact_digest: ArtifactDigest,
    pub model_format: String,
    pub model_family: String,
    pub model_parameter_profile: String,
    pub model_quantization: String,
    pub bounds: ChatBounds,
    pub maximum_concurrency: u16,
    pub network_requirement: ChatNetworkRequirement,
    pub latency_objective_millis: u64,
    pub latency_evidence_window_requests: u32,
}

impl ChatProviderProfile {
    pub fn new(input: ChatProviderProfileInput) -> Result<Self, ChatError> {
        let mut value = Self {
            schema_version: CHAT_SCHEMA_VERSION,
            identity: String::new(),
            semantic_mode: QUICK_LOCAL_MODE.to_owned(),
            model_artifact_id: input.model_artifact_id,
            model_artifact_digest: input.model_artifact_digest.to_string(),
            model_format: input.model_format,
            model_family: input.model_family,
            model_parameter_profile: input.model_parameter_profile,
            model_quantization: input.model_quantization,
            maximum_message_bytes: input.bounds.maximum_message_bytes,
            maximum_context_bytes: input.bounds.maximum_context_bytes,
            maximum_output_bytes: input.bounds.maximum_output_bytes,
            maximum_chunk_bytes: input.bounds.maximum_chunk_bytes,
            maximum_chunks: input.bounds.maximum_chunks,
            maximum_concurrency: input.maximum_concurrency,
            streaming: true,
            compute_profile: "quick".to_owned(),
            latency_objective_millis: input.latency_objective_millis,
            latency_evidence_window_requests: input.latency_evidence_window_requests,
            locality: "local".to_owned(),
            network_requirement: input.network_requirement,
            retention: "none".to_owned(),
            accepted_sensitivity: Sensitivity::Public.as_str().to_owned(),
            tool_use: false,
            structured_output: false,
        };
        value.identity = value.semantic_hash()?.to_string();
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), ChatError> {
        let bounds = self.bounds();
        if self.schema_version != CHAT_SCHEMA_VERSION
            || self.semantic_mode != QUICK_LOCAL_MODE
            || Id::new(&self.model_artifact_id).is_err()
            || !self.model_artifact_digest.starts_with("sha256:")
            || self.model_format.is_empty()
            || self.model_family.is_empty()
            || self.model_parameter_profile.is_empty()
            || self.model_quantization.is_empty()
            || self.maximum_concurrency == 0
            || self.maximum_concurrency > MAXIMUM_CONCURRENCY
            || !self.streaming
            || self.compute_profile != "quick"
            || self.latency_objective_millis == 0
            || self.latency_objective_millis > MAXIMUM_TIMEOUT_MILLIS
            || self.latency_evidence_window_requests == 0
            || self.locality != "local"
            || self.retention != "none"
            || self.accepted_sensitivity != Sensitivity::Public.as_str()
            || self.tool_use
            || self.structured_output
        {
            return Err(ChatError::new(
                ChatReason::UnsupportedProfile,
                "chat provider profile is malformed or overclaims support",
            ));
        }
        bounds.validate()?;
        if self.identity != self.semantic_hash()?.to_string() {
            return Err(ChatError::new(
                ChatReason::StaleProvider,
                "chat provider profile identity does not match its facts",
            ));
        }
        Ok(())
    }

    pub fn semantic_hash(&self) -> Result<SemanticHash, ChatError> {
        let model = Id::new(&self.model_artifact_id).map_err(|_| {
            ChatError::new(ChatReason::UnsupportedProfile, "invalid model artifact id")
        })?;
        let fields = [
            semantic(
                "semantic_mode",
                CanonicalValue::Identifier(Id(QUICK_LOCAL_MODE)),
            ),
            semantic("model_artifact", CanonicalValue::Identifier(model)),
            semantic(
                "model_digest",
                CanonicalValue::Text(&self.model_artifact_digest),
            ),
            semantic("model_format", CanonicalValue::Text(&self.model_format)),
            semantic("model_family", CanonicalValue::Text(&self.model_family)),
            semantic(
                "model_parameter_profile",
                CanonicalValue::Text(&self.model_parameter_profile),
            ),
            semantic(
                "model_quantization",
                CanonicalValue::Text(&self.model_quantization),
            ),
            semantic(
                "maximum_message_bytes",
                CanonicalValue::Integer(i128::try_from(self.maximum_message_bytes).map_err(
                    |_| {
                        ChatError::new(ChatReason::BoundsInvalid, "message bound does not fit i128")
                    },
                )?),
            ),
            semantic(
                "maximum_context_bytes",
                CanonicalValue::Integer(i128::try_from(self.maximum_context_bytes).map_err(
                    |_| {
                        ChatError::new(ChatReason::BoundsInvalid, "context bound does not fit i128")
                    },
                )?),
            ),
            semantic(
                "maximum_output_bytes",
                CanonicalValue::Integer(i128::try_from(self.maximum_output_bytes).map_err(
                    |_| ChatError::new(ChatReason::BoundsInvalid, "output bound does not fit i128"),
                )?),
            ),
            semantic(
                "maximum_chunk_bytes",
                CanonicalValue::Integer(i128::try_from(self.maximum_chunk_bytes).map_err(
                    |_| ChatError::new(ChatReason::BoundsInvalid, "chunk bound does not fit i128"),
                )?),
            ),
            semantic(
                "maximum_chunks",
                CanonicalValue::Integer(i128::from(self.maximum_chunks)),
            ),
            semantic(
                "maximum_concurrency",
                CanonicalValue::Integer(i128::from(self.maximum_concurrency)),
            ),
            semantic("streaming", CanonicalValue::Boolean(self.streaming)),
            semantic(
                "compute_profile",
                CanonicalValue::Text(&self.compute_profile),
            ),
            semantic(
                "latency_objective_millis",
                CanonicalValue::Integer(i128::from(self.latency_objective_millis)),
            ),
            semantic(
                "latency_evidence_window_requests",
                CanonicalValue::Integer(i128::from(self.latency_evidence_window_requests)),
            ),
            semantic("locality", CanonicalValue::Text(&self.locality)),
            semantic(
                "network_requirement",
                CanonicalValue::Identifier(Id(self.network_requirement.as_str())),
            ),
            semantic("retention", CanonicalValue::Text(&self.retention)),
            semantic(
                "accepted_sensitivity",
                CanonicalValue::Identifier(Id(&self.accepted_sensitivity)),
            ),
            semantic("tool_use", CanonicalValue::Boolean(self.tool_use)),
            semantic(
                "structured_output",
                CanonicalValue::Boolean(self.structured_output),
            ),
        ];
        CanonicalDescriptor {
            kind: Id(CHAT_PROFILE_ID),
            schema_version: self.schema_version,
            body: CanonicalValue::Map(&fields),
        }
        .semantic_hash()
        .map_err(|_| {
            ChatError::new(
                ChatReason::UnsupportedProfile,
                "chat provider profile cannot be canonically encoded",
            )
        })
    }

    #[must_use]
    pub const fn bounds(&self) -> ChatBounds {
        ChatBounds {
            maximum_message_bytes: self.maximum_message_bytes,
            maximum_context_bytes: self.maximum_context_bytes,
            maximum_output_bytes: self.maximum_output_bytes,
            maximum_chunk_bytes: self.maximum_chunk_bytes,
            maximum_chunks: self.maximum_chunks,
        }
    }

    #[must_use]
    pub fn pin(&self) -> PinnedDescriptor<'static> {
        PinnedDescriptor {
            id: Id(CHAT_PROFILE_ID),
            schema_version: self.schema_version,
            semantic_hash: self
                .semantic_hash()
                .expect("validated chat profile has a semantic hash"),
        }
    }

    #[must_use]
    pub fn capability_requirement(
        &self,
        implementation_id: impl Into<String>,
    ) -> InstalledCapabilityRequirement {
        InstalledCapabilityRequirement {
            interface: PinnedDescriptor {
                id: CHAT_CONTRACT.id,
                schema_version: CHAT_SCHEMA_VERSION,
                semantic_hash: conduit_runtime::OwnedNodeSchema::from_contract(&CHAT_CONTRACT)
                    .semantic_hash(),
            },
            mode: QUICK_LOCAL_MODE.to_owned(),
            subject: Some(implementation_id.into()),
            details: Some(
                self.semantic_hash()
                    .expect("validated chat profile has a semantic hash"),
            ),
            minimum_capacity: PlanResourceBudget {
                memory_bytes: 128 * 1024,
                storage_bytes: 0,
                cpu_units: 1,
                timers: 1,
                transports: u16::from(
                    self.network_requirement == ChatNetworkRequirement::LoopbackOnly,
                ),
                checkpoints: 0,
                evidence_bytes: 8 * 1024,
            },
        }
    }
}

const fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChatBounds {
    pub maximum_message_bytes: usize,
    pub maximum_context_bytes: usize,
    pub maximum_output_bytes: usize,
    pub maximum_chunk_bytes: usize,
    pub maximum_chunks: u16,
}

impl ChatBounds {
    pub const REFERENCE: Self = Self {
        maximum_message_bytes: 4096,
        maximum_context_bytes: 8192,
        maximum_output_bytes: 4096,
        maximum_chunk_bytes: 1024,
        maximum_chunks: 8,
    };

    pub fn validate(self) -> Result<(), ChatError> {
        if self.maximum_message_bytes == 0
            || self.maximum_message_bytes > MAXIMUM_MESSAGE_BYTES
            || self.maximum_context_bytes > MAXIMUM_CONTEXT_BYTES
            || self.maximum_output_bytes == 0
            || self.maximum_output_bytes > MAXIMUM_OUTPUT_BYTES
            || self.maximum_chunk_bytes == 0
            || self.maximum_chunk_bytes > MAXIMUM_CHUNK_BYTES
            || self.maximum_chunk_bytes > self.maximum_output_bytes
            || self.maximum_chunks == 0
            || self.maximum_chunks > MAXIMUM_CHUNKS
        {
            return Err(ChatError::new(
                ChatReason::BoundsInvalid,
                "chat request bounds are invalid or exceed the portable ceiling",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChatReason {
    Completed,
    EmptyPrompt,
    InputOverflow,
    ContextOverflow,
    OutputOverflow,
    ChunkOverflow,
    ProviderUnavailable,
    StaleProvider,
    ConcurrencyExhausted,
    TimedOut,
    Cancelled,
    ProviderLost,
    MalformedProviderOutput,
    SensitivityRefused,
    GrantDenied,
    UnexpectedNetwork,
    ToolsUnsupported,
    StructuredOutputUnsupported,
    ModelMismatch,
    UnsupportedProfile,
    BoundsInvalid,
}

impl ChatReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Completed => "CND-CHAT-000",
            Self::EmptyPrompt => "CND-CHAT-001",
            Self::InputOverflow => "CND-CHAT-002",
            Self::ContextOverflow => "CND-CHAT-003",
            Self::OutputOverflow => "CND-CHAT-004",
            Self::ChunkOverflow => "CND-CHAT-005",
            Self::ProviderUnavailable => "CND-CHAT-006",
            Self::StaleProvider => "CND-CHAT-007",
            Self::ConcurrencyExhausted => "CND-CHAT-008",
            Self::TimedOut => "CND-CHAT-009",
            Self::Cancelled => "CND-CHAT-010",
            Self::ProviderLost => "CND-CHAT-011",
            Self::MalformedProviderOutput => "CND-CHAT-012",
            Self::SensitivityRefused => "CND-CHAT-013",
            Self::GrantDenied => "CND-CHAT-014",
            Self::UnexpectedNetwork => "CND-CHAT-015",
            Self::ToolsUnsupported => "CND-CHAT-016",
            Self::StructuredOutputUnsupported => "CND-CHAT-017",
            Self::ModelMismatch => "CND-CHAT-018",
            Self::UnsupportedProfile => "CND-CHAT-019",
            Self::BoundsInvalid => "CND-CHAT-020",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatError {
    pub reason: ChatReason,
    pub message: String,
}

impl ChatError {
    fn new(reason: ChatReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ChatError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.reason.code(), self.message)
    }
}

impl std::error::Error for ChatError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatRequest {
    pub message: Vec<u8>,
    pub context: Option<Vec<u8>>,
    pub bounds: ChatBounds,
    pub timeout_millis: u64,
    pub sensitivity: Sensitivity,
    pub tools_requested: bool,
    pub structured_output_requested: bool,
}

impl ChatRequest {
    pub fn validate(&self) -> Result<(), ChatError> {
        self.bounds.validate()?;
        if self.message.is_empty() {
            return Err(ChatError::new(
                ChatReason::EmptyPrompt,
                "chat message is empty",
            ));
        }
        if self.message.len() > self.bounds.maximum_message_bytes {
            return Err(ChatError::new(
                ChatReason::InputOverflow,
                "chat message exceeds its exact byte bound",
            ));
        }
        if self
            .context
            .as_ref()
            .is_some_and(|context| context.len() > self.bounds.maximum_context_bytes)
        {
            return Err(ChatError::new(
                ChatReason::ContextOverflow,
                "caller-supplied chat context exceeds its exact byte bound",
            ));
        }
        if self.timeout_millis == 0 || self.timeout_millis > MAXIMUM_TIMEOUT_MILLIS {
            return Err(ChatError::new(
                ChatReason::TimedOut,
                "chat timeout is zero or exceeds the portable ceiling",
            ));
        }
        if self.sensitivity != Sensitivity::Public {
            return Err(ChatError::new(
                ChatReason::SensitivityRefused,
                "provider does not accept the requested sensitivity class",
            ));
        }
        if self.tools_requested {
            return Err(ChatError::new(
                ChatReason::ToolsUnsupported,
                "quick-local chat does not support tools",
            ));
        }
        if self.structured_output_requested {
            return Err(ChatError::new(
                ChatReason::StructuredOutputUnsupported,
                "quick-local chat does not guarantee structured output",
            ));
        }
        std::str::from_utf8(&self.message).map_err(|_| {
            ChatError::new(ChatReason::MalformedProviderOutput, "message is not UTF-8")
        })?;
        if let Some(context) = &self.context {
            std::str::from_utf8(context).map_err(|_| {
                ChatError::new(
                    ChatReason::MalformedProviderOutput,
                    "caller-supplied context is not UTF-8",
                )
            })?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChatEvidenceKind {
    Admitted,
    RequestCommitted,
    Chunk,
    Cancelled,
    Terminal,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChatEvidenceEvent {
    pub kind: ChatEvidenceKind,
    pub chunk_index: Option<u16>,
    pub bytes: usize,
    pub total_output_bytes: usize,
    pub reason_code: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChatResult {
    pub schema_version: u32,
    pub terminal: String,
    pub reason_code: String,
    pub chunk_count: u16,
    pub output_bytes: usize,
    pub semantic_mode: String,
    pub implementation_id: String,
    pub model_artifact_id: String,
    pub model_artifact_digest: String,
    pub locality: String,
    pub network_requirement: ChatNetworkRequirement,
    pub conversation_state: String,
    pub retention: String,
    pub tools_used: bool,
    pub structured_output_guaranteed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatExecution {
    pub chunks: Vec<String>,
    pub result: ChatResult,
    pub evidence: Vec<ChatEvidenceEvent>,
}

impl ChatExecution {
    fn completed(
        chunks: Vec<String>,
        profile: &ChatProviderProfile,
        implementation_id: &str,
    ) -> Result<Self, ChatError> {
        let mut output_bytes = 0_usize;
        let mut evidence = vec![
            ChatEvidenceEvent {
                kind: ChatEvidenceKind::Admitted,
                chunk_index: None,
                bytes: 0,
                total_output_bytes: 0,
                reason_code: None,
            },
            ChatEvidenceEvent {
                kind: ChatEvidenceKind::RequestCommitted,
                chunk_index: None,
                bytes: 0,
                total_output_bytes: 0,
                reason_code: None,
            },
        ];
        if chunks.is_empty() || chunks.len() > usize::from(profile.maximum_chunks) {
            return Err(ChatError::new(
                ChatReason::ChunkOverflow,
                "provider returned zero chunks or exceeded the chunk-count bound",
            ));
        }
        for (index, chunk) in chunks.iter().enumerate() {
            if chunk.len() > profile.maximum_chunk_bytes {
                return Err(ChatError::new(
                    ChatReason::ChunkOverflow,
                    "provider returned a chunk above its exact byte bound",
                ));
            }
            output_bytes = output_bytes
                .checked_add(chunk.len())
                .ok_or_else(|| ChatError::new(ChatReason::OutputOverflow, "output overflowed"))?;
            if output_bytes > profile.maximum_output_bytes {
                return Err(ChatError::new(
                    ChatReason::OutputOverflow,
                    "provider output exceeded its exact byte bound",
                ));
            }
            evidence.push(ChatEvidenceEvent {
                kind: ChatEvidenceKind::Chunk,
                chunk_index: Some(u16::try_from(index).map_err(|_| {
                    ChatError::new(ChatReason::ChunkOverflow, "chunk index does not fit u16")
                })?),
                bytes: chunk.len(),
                total_output_bytes: output_bytes,
                reason_code: None,
            });
        }
        evidence.push(ChatEvidenceEvent {
            kind: ChatEvidenceKind::Terminal,
            chunk_index: None,
            bytes: 0,
            total_output_bytes: output_bytes,
            reason_code: Some(ChatReason::Completed.code().to_owned()),
        });
        if evidence.len() > MAXIMUM_EVIDENCE_EVENTS {
            return Err(ChatError::new(
                ChatReason::ChunkOverflow,
                "chat evidence exceeded its retained-event bound",
            ));
        }
        Ok(Self {
            result: ChatResult {
                schema_version: CHAT_SCHEMA_VERSION,
                terminal: "completed".to_owned(),
                reason_code: ChatReason::Completed.code().to_owned(),
                chunk_count: u16::try_from(chunks.len()).map_err(|_| {
                    ChatError::new(ChatReason::ChunkOverflow, "chunk count does not fit u16")
                })?,
                output_bytes,
                semantic_mode: profile.semantic_mode.clone(),
                implementation_id: implementation_id.to_owned(),
                model_artifact_id: profile.model_artifact_id.clone(),
                model_artifact_digest: profile.model_artifact_digest.clone(),
                locality: profile.locality.clone(),
                network_requirement: profile.network_requirement,
                conversation_state: "caller-supplied-only".to_owned(),
                retention: profile.retention.clone(),
                tools_used: false,
                structured_output_guaranteed: false,
            },
            chunks,
            evidence,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceChatFault {
    None,
    Timeout,
    Cancelled,
    ProviderLoss,
    MalformedOutput,
    OutputOverflow,
}

pub const REFERENCE_REPLY: &str =
    "Conduit keeps contracts, implementations, host facts, plans, and evidence distinct.";
pub const REFERENCE_MODEL_BYTES: &[u8] =
    b"conduit.ai/reference-response-program|0|quick-local|bounded";

pub fn run_reference_chat(
    request: &ChatRequest,
    profile: &ChatProviderProfile,
    fault: ReferenceChatFault,
) -> Result<ChatExecution, ChatError> {
    request.validate()?;
    if request.bounds.maximum_message_bytes > profile.maximum_message_bytes
        || request.bounds.maximum_context_bytes > profile.maximum_context_bytes
        || request.bounds.maximum_output_bytes > profile.maximum_output_bytes
        || request.bounds.maximum_chunk_bytes > profile.maximum_chunk_bytes
        || request.bounds.maximum_chunks > profile.maximum_chunks
    {
        return Err(ChatError::new(
            ChatReason::UnsupportedProfile,
            "request asks beyond the selected provider profile",
        ));
    }
    match fault {
        ReferenceChatFault::Timeout => {
            return Err(ChatError::new(
                ChatReason::TimedOut,
                "provider deadline elapsed",
            ));
        }
        ReferenceChatFault::Cancelled => {
            return Err(ChatError::new(
                ChatReason::Cancelled,
                "chat request was cancelled",
            ));
        }
        ReferenceChatFault::ProviderLoss => {
            return Err(ChatError::new(
                ChatReason::ProviderLost,
                "chat provider was lost",
            ));
        }
        ReferenceChatFault::MalformedOutput => {
            return Err(ChatError::new(
                ChatReason::MalformedProviderOutput,
                "chat provider returned malformed framing",
            ));
        }
        ReferenceChatFault::OutputOverflow => {
            return ChatExecution::completed(
                vec![
                    "x".repeat(profile.maximum_chunk_bytes);
                    profile
                        .maximum_output_bytes
                        .checked_div(profile.maximum_chunk_bytes)
                        .unwrap_or(0)
                        .saturating_add(1)
                ],
                profile,
                "conduit.ai/chat-reference-rust",
            );
        }
        ReferenceChatFault::None => {}
    }
    ChatExecution::completed(
        vec![REFERENCE_REPLY.to_owned()],
        profile,
        "conduit.ai/chat-reference-rust",
    )
}

#[derive(Debug)]
pub struct ChatAdmissionPool {
    maximum: u16,
    active: AtomicU16,
}

impl ChatAdmissionPool {
    pub fn new(maximum: u16) -> Result<Self, ChatError> {
        if maximum == 0 || maximum > MAXIMUM_CONCURRENCY {
            return Err(ChatError::new(
                ChatReason::BoundsInvalid,
                "chat concurrency bound is invalid",
            ));
        }
        Ok(Self {
            maximum,
            active: AtomicU16::new(0),
        })
    }

    pub fn try_acquire(self: &Arc<Self>) -> Result<ChatAdmission, ChatError> {
        let mut current = self.active.load(Ordering::Acquire);
        loop {
            if current >= self.maximum {
                return Err(ChatError::new(
                    ChatReason::ConcurrencyExhausted,
                    "chat provider concurrency is exhausted",
                ));
            }
            match self.active.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Ok(ChatAdmission {
                        pool: Arc::clone(self),
                    });
                }
                Err(observed) => current = observed,
            }
        }
    }

    #[must_use]
    pub fn active(&self) -> u16 {
        self.active.load(Ordering::Acquire)
    }
}

#[derive(Debug)]
pub struct ChatAdmission {
    pool: Arc<ChatAdmissionPool>,
}

impl Drop for ChatAdmission {
    fn drop(&mut self) {
        self.pool.active.fetch_sub(1, Ordering::AcqRel);
    }
}

struct ReferenceChatHandler {
    profile: ChatProviderProfile,
    pool: Arc<ChatAdmissionPool>,
}

impl Handler for ReferenceChatHandler {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let _admission = self.pool.try_acquire().map_err(runtime_error)?;
        let request = request_from_node(node, inputs).map_err(runtime_error)?;
        let execution = run_reference_chat(&request, &self.profile, ReferenceChatFault::None)
            .map_err(runtime_error)?;
        execution_values(execution).map_err(runtime_error)
    }
}

struct ChatResultInspectHandler;

impl Handler for ChatResultInspectHandler {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [value] = inputs else {
            return Err(RuntimeError::new(
                ChatReason::MalformedProviderOutput.code(),
                "chat result inspector requires exactly one result",
            ));
        };
        let inspection = inspect_chat_result(value).map_err(runtime_error)?;
        Ok(vec![Value::text(inspection.summary())])
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChatInspection {
    pub friendly_label: String,
    pub semantic_mode: String,
    pub implementation_id: String,
    pub model_artifact_id: String,
    pub model_artifact_digest: String,
    pub terminal: String,
    pub reason_code: String,
    pub chunk_count: u16,
    pub output_bytes: usize,
    pub locality: String,
    pub network_requirement: ChatNetworkRequirement,
    pub conversation_state: String,
    pub retention: String,
    pub tools_used: bool,
    pub structured_output_guaranteed: bool,
}

impl ChatInspection {
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "quick local model: {}; {} chunk(s); {} bytes; conversation {}; retention {}",
            self.terminal,
            self.chunk_count,
            self.output_bytes,
            self.conversation_state,
            self.retention,
        )
    }
}

pub fn inspect_chat_result(value: &Value) -> Result<ChatInspection, ChatError> {
    if value.value_type != CHAT_RESULT_TYPE || value.bytes.len() > 16 * 1024 {
        return Err(ChatError::new(
            ChatReason::MalformedProviderOutput,
            "chat result has the wrong type or exceeds its inspection bound",
        ));
    }
    let result: ChatResult = serde_json::from_slice(&value.bytes).map_err(|_| {
        ChatError::new(
            ChatReason::MalformedProviderOutput,
            "chat result is not the current bounded schema",
        )
    })?;
    if result.schema_version != CHAT_SCHEMA_VERSION
        || result.semantic_mode != QUICK_LOCAL_MODE
        || result.reason_code.is_empty()
        || result.chunk_count == 0
        || result.output_bytes > MAXIMUM_OUTPUT_BYTES
        || result.conversation_state != "caller-supplied-only"
        || result.retention != "none"
        || result.tools_used
        || result.structured_output_guaranteed
    {
        return Err(ChatError::new(
            ChatReason::MalformedProviderOutput,
            "chat result contains invalid or overclaimed facts",
        ));
    }
    Ok(ChatInspection {
        friendly_label: "quick local model".to_owned(),
        semantic_mode: result.semantic_mode,
        implementation_id: result.implementation_id,
        model_artifact_id: result.model_artifact_id,
        model_artifact_digest: result.model_artifact_digest,
        terminal: result.terminal,
        reason_code: result.reason_code,
        chunk_count: result.chunk_count,
        output_bytes: result.output_bytes,
        locality: result.locality,
        network_requirement: result.network_requirement,
        conversation_state: result.conversation_state,
        retention: result.retention,
        tools_used: result.tools_used,
        structured_output_guaranteed: result.structured_output_guaranteed,
    })
}

fn execution_values(execution: ChatExecution) -> Result<Vec<Value>, ChatError> {
    let text = execution.chunks.concat();
    let result = serde_json::to_vec(&execution.result).map_err(|_| {
        ChatError::new(
            ChatReason::MalformedProviderOutput,
            "chat result could not be encoded",
        )
    })?;
    Ok(vec![
        Value::text(text),
        Value {
            value_type: CHAT_RESULT_TYPE,
            bytes: result,
        },
    ])
}

fn integer(node: &Node, key: &str) -> Result<u64, ChatError> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => u64::try_from(*value).map_err(|_| {
            ChatError::new(
                ChatReason::BoundsInvalid,
                format!("`{key}` must be nonnegative"),
            )
        }),
        _ => Err(ChatError::new(
            ChatReason::BoundsInvalid,
            format!("`{key}` must be an integer"),
        )),
    }
}

fn request_bounds(node: &Node) -> Result<ChatBounds, ChatError> {
    let bounds =
        ChatBounds {
            maximum_message_bytes: usize::try_from(integer(node, "maximum_message_bytes")?)
                .map_err(|_| {
                    ChatError::new(
                        ChatReason::BoundsInvalid,
                        "maximum_message_bytes does not fit usize",
                    )
                })?,
            maximum_context_bytes: usize::try_from(integer(node, "maximum_context_bytes")?)
                .map_err(|_| {
                    ChatError::new(
                        ChatReason::BoundsInvalid,
                        "maximum_context_bytes does not fit usize",
                    )
                })?,
            maximum_output_bytes: usize::try_from(integer(node, "maximum_output_bytes")?).map_err(
                |_| {
                    ChatError::new(
                        ChatReason::BoundsInvalid,
                        "maximum_output_bytes does not fit usize",
                    )
                },
            )?,
            maximum_chunk_bytes: usize::try_from(integer(node, "maximum_chunk_bytes")?).map_err(
                |_| {
                    ChatError::new(
                        ChatReason::BoundsInvalid,
                        "maximum_chunk_bytes does not fit usize",
                    )
                },
            )?,
            maximum_chunks: u16::try_from(integer(node, "maximum_chunks")?).map_err(|_| {
                ChatError::new(ChatReason::BoundsInvalid, "maximum_chunks does not fit u16")
            })?,
        };
    bounds.validate()?;
    Ok(bounds)
}

fn request_from_node(node: &Node, inputs: &[Value]) -> Result<ChatRequest, ChatError> {
    let Some(message) = inputs.first() else {
        return Err(ChatError::new(
            ChatReason::EmptyPrompt,
            "chat message is absent",
        ));
    };
    if message.value_type != TEXT_TYPE
        || inputs
            .get(1)
            .is_some_and(|context| context.value_type != CHAT_CONTEXT_TYPE)
        || inputs.len() > 2
    {
        return Err(ChatError::new(
            ChatReason::MalformedProviderOutput,
            "chat inputs do not match the exact message/context contract",
        ));
    }
    Ok(ChatRequest {
        message: message.bytes.clone(),
        context: inputs.get(1).map(|value| value.bytes.clone()),
        bounds: request_bounds(node)?,
        timeout_millis: integer(node, "timeout_millis")?,
        sensitivity: match node.config("sensitivity") {
            Some("public") => Sensitivity::Public,
            Some("restricted") => Sensitivity::Restricted,
            Some("secret") => Sensitivity::Secret,
            _ => {
                return Err(ChatError::new(
                    ChatReason::SensitivityRefused,
                    "unknown chat sensitivity class",
                ));
            }
        },
        tools_requested: node.config("tools") != Some("forbidden"),
        structured_output_requested: node.config("structured_output") != Some("unsupported"),
    })
}

fn validate_chat_config(node: &Node, profile: &ChatProviderProfile) -> Result<(), ResolutionError> {
    let expected = [
        "mode",
        "maximum_message_bytes",
        "maximum_context_bytes",
        "maximum_output_bytes",
        "maximum_chunk_bytes",
        "maximum_chunks",
        "maximum_concurrency",
        "timeout_millis",
        "sensitivity",
        "conversation_state",
        "retention",
        "tools",
        "structured_output",
    ];
    if node.config.len() != expected.len()
        || expected
            .iter()
            .any(|key| !node.config.iter().any(|entry| entry.key == *key))
    {
        return Err(resolution_error(ChatError::new(
            ChatReason::BoundsInvalid,
            "chat node does not contain the one current exact config",
        )));
    }
    if node.config("mode") != Some(QUICK_LOCAL_MODE) {
        return Err(resolution_error(ChatError::new(
            ChatReason::UnsupportedProfile,
            "chat node requests an unsupported semantic mode",
        )));
    }
    if node.config("conversation_state") != Some("caller-supplied-only")
        || node.config("retention") != Some("none")
    {
        return Err(resolution_error(ChatError::new(
            ChatReason::UnsupportedProfile,
            "provider-retained ambient conversation state is forbidden",
        )));
    }
    if node.config("tools") != Some("forbidden") {
        return Err(resolution_error(ChatError::new(
            ChatReason::ToolsUnsupported,
            "quick-local chat does not support tools",
        )));
    }
    if node.config("structured_output") != Some("unsupported") {
        return Err(resolution_error(ChatError::new(
            ChatReason::StructuredOutputUnsupported,
            "quick-local chat does not guarantee structured output",
        )));
    }
    if node.config("sensitivity") != Some("public") {
        return Err(resolution_error(ChatError::new(
            ChatReason::SensitivityRefused,
            "selected provider accepts only public input",
        )));
    }
    let bounds = request_bounds(node).map_err(resolution_error)?;
    let maximum_concurrency = integer(node, "maximum_concurrency").map_err(resolution_error)?;
    let timeout = integer(node, "timeout_millis").map_err(resolution_error)?;
    if bounds.maximum_message_bytes > profile.maximum_message_bytes
        || bounds.maximum_context_bytes > profile.maximum_context_bytes
        || bounds.maximum_output_bytes > profile.maximum_output_bytes
        || bounds.maximum_chunk_bytes > profile.maximum_chunk_bytes
        || bounds.maximum_chunks > profile.maximum_chunks
        || maximum_concurrency == 0
        || maximum_concurrency > u64::from(profile.maximum_concurrency)
        || timeout == 0
        || timeout > MAXIMUM_TIMEOUT_MILLIS
    {
        return Err(resolution_error(ChatError::new(
            ChatReason::UnsupportedProfile,
            "chat node asks beyond the selected provider's exact profile",
        )));
    }
    Ok(())
}

fn runtime_error(error: ChatError) -> RuntimeError {
    RuntimeError::new(error.reason.code(), error.message)
}

fn resolution_error(error: ChatError) -> ResolutionError {
    ResolutionError::new(error.reason.code(), error.message)
}

pub fn register_chat_contracts(registry: &mut Registry) {
    for contract in CHAT_CONTRACTS {
        registry.register_contract_only(contract);
    }
}

pub fn register_chat_result_inspector(registry: &mut Registry) -> Result<(), RegistryError> {
    registry.register_compiled_in_host_service(CompiledInHostService {
        contract: &CHAT_RESULT_INSPECT_CONTRACT,
        implementation_id: "conduit.ai/chat-result-inspect-rust",
        artifact_id: "conduit.ai/chat-result-inspect-rust-artifact",
        entrypoint: "ai-chat-result-inspect",
        source_bytes: include_bytes!("lib.rs"),
        required_authorities: &[],
        factory: || Box::new(ChatResultInspectHandler),
        validate_config: |node| {
            node.config.is_empty().then_some(()).ok_or_else(|| {
                ResolutionError::new(
                    ChatReason::BoundsInvalid.code(),
                    "chat result inspector has no configuration",
                )
            })
        },
    })
}

pub fn reference_chat_profile() -> ChatProviderProfile {
    ChatProviderProfile::new(ChatProviderProfileInput {
        model_artifact_id: "conduit.ai/model/reference-response-program".to_owned(),
        model_artifact_digest: ArtifactDigest::from_bytes([
            0x95, 0x3d, 0x7b, 0x51, 0x13, 0xff, 0xe1, 0x7f, 0xa0, 0x9f, 0xe3, 0xde, 0xec, 0xa4,
            0x17, 0xed, 0x9c, 0x89, 0xde, 0x96, 0x14, 0x74, 0x47, 0x54, 0x6a, 0x85, 0xe7, 0x5b,
            0xa0, 0xd5, 0xd8, 0x0d,
        ]),
        model_format: "conduit-reference-response-program".to_owned(),
        model_family: "deterministic-reference".to_owned(),
        model_parameter_profile: "fixed-bounded-reply".to_owned(),
        model_quantization: "exact".to_owned(),
        bounds: ChatBounds::REFERENCE,
        maximum_concurrency: 4,
        network_requirement: ChatNetworkRequirement::None,
        latency_objective_millis: 50,
        latency_evidence_window_requests: 64,
    })
    .expect("built-in reference chat profile is valid")
}

pub fn register_deterministic_chat_provider(registry: &mut Registry) -> Result<(), RegistryError> {
    register_chat_contracts(registry);
    let profile = reference_chat_profile();
    let pool = Arc::new(
        ChatAdmissionPool::new(profile.maximum_concurrency)
            .expect("built-in reference concurrency is valid"),
    );
    let adapter_bytes = include_bytes!("lib.rs");
    let adapter_digest = ArtifactDigest::from_bytes(Sha256::digest(adapter_bytes).into());
    let model_digest = ArtifactDigest::from_bytes(Sha256::digest(REFERENCE_MODEL_BYTES).into());
    let factory_profile = profile.clone();
    let validator_profile = profile.clone();
    registry.register_installed_implementation(InstalledImplementationRegistration {
        contract: &CHAT_CONTRACT,
        implementation_id: "conduit.ai/chat-reference-rust".to_owned(),
        implementation_version: "reference-chat-0".to_owned(),
        executor: ExecutorKind::NativeInProcess,
        entrypoint_name: "ai-chat-reference".to_owned(),
        entrypoint_adapter: "conduit/host-service-step".to_owned(),
        entrypoint_abi: "conduit/rust-in-process".to_owned(),
        entrypoint_protocol_version: CHAT_SCHEMA_VERSION,
        execution_profile: profile.pin(),
        artifacts: vec![
            InstalledArtifactRegistration {
                id: "conduit.ai/chat-reference-rust-artifact".to_owned(),
                digest: adapter_digest,
                media_type: "application/vnd.conduit.compiled-in-provider".to_owned(),
                byte_size: u64::try_from(adapter_bytes.len())
                    .expect("compiled adapter length fits u64"),
                target: Some(std::env::consts::ARCH.to_owned()),
                abi: Some("conduit/rust-in-process".to_owned()),
                builder: "conduit/rustc-workspace-build".to_owned(),
                source_digest: adapter_digest,
                build_recipe_digest: ArtifactDigest::from_bytes(
                    Sha256::digest(b"cargo build -p conduit-ai").into(),
                ),
                reproducible: true,
                license_expressions: vec!["MIT".to_owned(), "Apache-2.0".to_owned()],
                role: "adapter".to_owned(),
                required: true,
            },
            InstalledArtifactRegistration {
                id: profile.model_artifact_id.clone(),
                digest: model_digest,
                media_type: "application/vnd.conduit.deterministic-chat-model".to_owned(),
                byte_size: u64::try_from(REFERENCE_MODEL_BYTES.len())
                    .expect("reference model length fits u64"),
                target: None,
                abi: None,
                builder: "conduit/reference-model-builder".to_owned(),
                source_digest: model_digest,
                build_recipe_digest: model_digest,
                reproducible: true,
                license_expressions: vec!["MIT".to_owned(), "Apache-2.0".to_owned()],
                role: "model".to_owned(),
                required: true,
            },
        ],
        required_capabilities: Vec::new(),
        required_authorities: Vec::new(),
        required_effects: Vec::new(),
        minimum_plan_version: 0,
        maximum_plan_version: u32::MAX,
        minimum_runtime_protocol: 1,
        maximum_runtime_protocol: 1,
        coexistence_memory_bytes: 0,
        managed_lifecycle: None,
        factory: move || {
            Box::new(ReferenceChatHandler {
                profile: factory_profile.clone(),
                pool: Arc::clone(&pool),
            }) as Box<dyn Handler>
        },
        validate_config: move |node: &Node| validate_chat_config(node, &validator_profile),
    })?;
    register_chat_result_inspector(registry)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn validate_exact_local_binding(
    binding: &ExactHostedServiceBinding,
    implementation_id: &str,
    profile: &ChatProviderProfile,
    endpoint: &str,
) -> Result<(), ChatError> {
    if binding.implementation_id != implementation_id
        || binding
            .artifacts
            .iter()
            .all(|artifact| artifact.id != profile.model_artifact_id)
    {
        return Err(ChatError::new(
            ChatReason::ModelMismatch,
            "exact binding does not name the installed chat implementation and model",
        ));
    }
    let endpoint_hash =
        conduit_runtime::hosted_effect_constraint_hash(ENDPOINT_CONSTRAINT, endpoint.as_bytes());
    let model_hash = conduit_runtime::hosted_effect_constraint_hash(
        MODEL_CONSTRAINT,
        profile.model_artifact_digest.as_bytes(),
    );
    let profile_hash = conduit_runtime::hosted_effect_constraint_hash(
        PROFILE_CONSTRAINT,
        profile.identity.as_bytes(),
    );
    let authority = binding.authorities.iter().find(|authority| {
        authority.action == LOCAL_CHAT_ACTION
            && authority.resource_kind == LOCAL_CHAT_RESOURCE_KIND
            && authority
                .constraints
                .iter()
                .any(|(id, hash)| id == ENDPOINT_CONSTRAINT && *hash == endpoint_hash)
            && authority
                .constraints
                .iter()
                .any(|(id, hash)| id == MODEL_CONSTRAINT && *hash == model_hash)
            && authority
                .constraints
                .iter()
                .any(|(id, hash)| id == PROFILE_CONSTRAINT && *hash == profile_hash)
    });
    if authority.is_none() {
        return Err(ChatError::new(
            ChatReason::GrantDenied,
            "exact local-chat resource, grant, or constraint binding is absent",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFORMANCE: &str = include_str!("../../../conformance/c4/quick-local-chat.json");

    fn request() -> ChatRequest {
        ChatRequest {
            message: b"Summarize the boundary.".to_vec(),
            context: None,
            bounds: ChatBounds::REFERENCE,
            timeout_millis: 1_000,
            sensitivity: Sensitivity::Public,
            tools_requested: false,
            structured_output_requested: false,
        }
    }

    #[test]
    fn reference_profile_and_reply_are_exact_and_bounded() {
        let profile = reference_chat_profile();
        profile.validate().unwrap();
        assert_eq!(
            profile.model_artifact_digest,
            ArtifactDigest::from_bytes(Sha256::digest(REFERENCE_MODEL_BYTES).into()).to_string()
        );
        let execution = run_reference_chat(&request(), &profile, ReferenceChatFault::None).unwrap();
        assert_eq!(execution.chunks, [REFERENCE_REPLY]);
        assert_eq!(execution.result.reason_code, ChatReason::Completed.code());
        assert_eq!(execution.evidence.len(), 4);
        assert!(
            serde_json::to_string(&execution.evidence)
                .unwrap()
                .find("Summarize")
                .is_none()
        );
    }

    #[test]
    fn model_transition_requires_a_new_profile_identity() {
        let current = reference_chat_profile();
        let mut replacement = current.clone();
        replacement.model_artifact_digest = ArtifactDigest::from_bytes([0x42; 32]).to_string();

        assert_eq!(
            replacement.validate().unwrap_err().reason,
            ChatReason::StaleProvider
        );
        replacement.identity = replacement.semantic_hash().unwrap().to_string();
        replacement.validate().unwrap();

        assert_ne!(current.identity, replacement.identity);
        assert_ne!(current.pin(), replacement.pin());
    }

    #[test]
    fn request_failures_remain_distinct() {
        let profile = reference_chat_profile();
        let mut value = request();
        value.message.clear();
        assert_eq!(
            value.validate().unwrap_err().reason,
            ChatReason::EmptyPrompt
        );
        value = request();
        value.message = vec![b'x'; value.bounds.maximum_message_bytes + 1];
        assert_eq!(
            value.validate().unwrap_err().reason,
            ChatReason::InputOverflow
        );
        value = request();
        value.context = Some(vec![b'x'; value.bounds.maximum_context_bytes]);
        assert!(value.validate().is_ok());
        value = request();
        value.context = Some(vec![b'x'; value.bounds.maximum_context_bytes + 1]);
        assert_eq!(
            value.validate().unwrap_err().reason,
            ChatReason::ContextOverflow
        );
        value = request();
        value.sensitivity = Sensitivity::Secret;
        assert_eq!(
            value.validate().unwrap_err().reason,
            ChatReason::SensitivityRefused
        );
        value = request();
        value.tools_requested = true;
        assert_eq!(
            value.validate().unwrap_err().reason,
            ChatReason::ToolsUnsupported
        );
        value = request();
        value.structured_output_requested = true;
        assert_eq!(
            value.validate().unwrap_err().reason,
            ChatReason::StructuredOutputUnsupported
        );
        assert_eq!(
            run_reference_chat(&request(), &profile, ReferenceChatFault::Timeout)
                .unwrap_err()
                .reason,
            ChatReason::TimedOut
        );
        assert_eq!(
            run_reference_chat(&request(), &profile, ReferenceChatFault::Cancelled)
                .unwrap_err()
                .reason,
            ChatReason::Cancelled
        );
        assert_eq!(
            run_reference_chat(&request(), &profile, ReferenceChatFault::ProviderLoss)
                .unwrap_err()
                .reason,
            ChatReason::ProviderLost
        );
        assert_eq!(
            run_reference_chat(&request(), &profile, ReferenceChatFault::MalformedOutput)
                .unwrap_err()
                .reason,
            ChatReason::MalformedProviderOutput
        );
        assert_eq!(
            run_reference_chat(&request(), &profile, ReferenceChatFault::OutputOverflow)
                .unwrap_err()
                .reason,
            ChatReason::OutputOverflow
        );
        assert_eq!(
            ChatExecution::completed(
                vec![
                    "x".repeat(profile.maximum_chunk_bytes);
                    usize::from(profile.maximum_chunks) + 1
                ],
                &profile,
                "conduit.ai/chat-reference-rust",
            )
            .unwrap_err()
            .reason,
            ChatReason::ChunkOverflow
        );
    }

    #[test]
    fn admission_is_bounded_and_released() {
        let pool = Arc::new(ChatAdmissionPool::new(1).unwrap());
        let admission = pool.try_acquire().unwrap();
        assert_eq!(pool.active(), 1);
        assert_eq!(
            pool.try_acquire().unwrap_err().reason,
            ChatReason::ConcurrencyExhausted
        );
        drop(admission);
        assert!(pool.try_acquire().is_ok());
    }

    #[test]
    fn contract_only_registry_is_honestly_unsupported() {
        let mut registry = Registry::hosted_primitives();
        register_chat_contracts(&mut registry);
        let availability = registry.node_availability("ai/chat");
        assert_eq!(
            availability.state,
            conduit_runtime::AvailabilityState::ContractOnly
        );
        assert_eq!(availability.reason_code, "CND-AVL-001");
    }

    #[test]
    fn conformance_inventory_names_every_required_failure_and_transition() {
        let fixture: serde_json::Value = serde_json::from_str(CONFORMANCE).unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        for id in [
            "deterministic-fixed-reply",
            "bounded-streaming-reply",
            "same-contract-distinct-implementations",
            "same-source-distinct-host-plans",
            "empty-prompt",
            "maximum-context",
            "context-overflow",
            "maximum-output",
            "output-overflow",
            "provider-unavailable",
            "stale-provider-report",
            "concurrency-exhausted",
            "timeout",
            "cancellation",
            "provider-loss",
            "malformed-frame",
            "sensitivity-refusal",
            "secret-redaction",
            "grant-denied",
            "unexpected-network",
            "unsupported-tools",
            "structured-output-rejected",
            "model-mismatch",
            "provider-model-transition-new-plan",
        ] {
            assert!(
                cases.iter().any(|case| case["id"] == id),
                "missing conformance case {id}"
            );
        }
        for reason in 0..=20 {
            let expected = format!("CND-CHAT-{reason:03}");
            assert!(
                cases
                    .iter()
                    .any(|case| case["expected"]["reason"] == expected),
                "missing reason {expected}"
            );
        }
    }
}
