//! Hosted registry, resolver, explainer, and executor.
//!
//! This first runtime intentionally executes finite, one-shot acyclic panels.
//! The portable core now includes the normative allocator-free bounded queue;
//! a later hosted streaming scheduler can drive it without changing node,
//! port, cord, or flow-policy identity.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{Read, Write};
use std::rc::Rc;
use std::sync::OnceLock;

use conduit_core::{
    ArtifactDigest, ArtifactManifest, ArtifactProvenance, BlockingFairness, CanonicalDescriptor,
    CanonicalValue, CompatibilityOutcome, ConfigContract, ConfigFieldContract, ConfigIdentity,
    ConfigMutability, ConfigRequirement, ConnectionCardinality, Delivery, DescriptorRef, Direction,
    Endpoint as CoreEndpoint, ExecutionPlan, ExecutorKind, FieldDisposition, FlowCapacity,
    FlowPolicy, FlowQueueState, FlowTypeFacts, FlowWatermarks, Id, ImplementationMachine,
    LossAcceptance, ManifestArtifactRef, ManifestEntrypoint, MapField, MemoryAccounting,
    NodeContract, PinnedDescriptor, PlanArtifact, PlanCord, PlanGraph, PlanNode, PortContract,
    PortFlowConstraints, Presence, Pressure, ReplacementSupport, ResolvedPlanNode, SampleSchedule,
    SchedulerPolicy, SemanticHash, Sensitivity, TemporalContract, TerminalClass, TerminalContract,
    TraitProof, TypeContractRef, ValueCardinality, validate_artifact_manifest,
    validate_implementation_manifest, validate_plan_graph,
};
use conduit_panel::{
    CompositeDefinition, ConfigEntry, Cord, Endpoint, ExportDirection, Node, Panel, SourcePressure,
    SourceValue,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

mod artifact_verification;
mod config_resolution;
mod distributed;
mod evidence_ndjson;
mod exact_evidence;
mod host_conformance;
mod host_resolution;
mod implementation_binding;
mod pool;
mod resource_effect;
mod runtime_evidence;
mod scheduler;
mod source_lowering;
mod supervision;
mod transition;
mod transport;
mod type_registry;
mod workload;

pub use artifact_verification::{
    ArtifactRejectionEvidence, EvidencedArtifactVerificationError, HostedArtifactVerificationError,
    VerifiedArtifactBytes, verify_artifact_bytes, verify_artifact_owned,
    verify_artifact_owned_evidenced,
};
pub use conduit_core::ImplementationManifest;
pub use config_resolution::{
    ConfigAssignment, ConfigResolutionError, ConfigValue, ResolvedConfig, ResolvedConfigEntry,
    SecretValue, resolve_config, validate_config_update,
};
pub use distributed::{
    DistributedBackendReadiness, DistributedCordBackend, DistributedFrameKind,
    HostedDistributedEvidence, InMemoryDistributedCordBackend, InMemoryTransportFault,
    OutboundDistributedFrame, ReceivedDistributedFrame, received_evidence_kind,
};
pub use evidence_ndjson::{
    EvidenceDecodeLimits, NdjsonError, NdjsonLimit, OwnedEventCorrelation, OwnedEventPayload,
    OwnedEventRelations, OwnedEventTerminality, OwnedEventTime, OwnedExecutionEvent,
    OwnedPayloadShape, OwnedTypeRef, decode_event_ndjson, decode_event_ndjson_with_limits,
    encode_event_ndjson, encode_owned_event_ndjson,
};
pub use exact_evidence::ExactEvidenceRecord;
pub use host_conformance::{
    BoundedProviderRun, ProviderRunError, ProviderRunEvidence, ProviderRunEvidenceKind,
    ProviderRunPhase,
};
pub use host_resolution::{
    CandidateAuthority, CandidateRejection, CandidateRejectionReason, CapabilityPredicate,
    HostResolverPolicy, PlacementCandidate, PlacementRequest, PlanSealingReason, ResolutionFailure,
    ResolvedPlacement, ResolvedPlacementBinding, ResolvedReplacementSupport, ResolverTiePolicy,
    ResourcePredicate, TopologyPredicate, resolve_host_placement, seal_resolved_execution_plan,
};
pub use implementation_binding::{
    ForeignStepReply, ForeignStepRequest, MessageStepBinding, MessageStepEndpoint,
    NativeStepBinding, NativeStepImplementation, OwnedStepOutcome, OwnedStepReply,
    OwnedWakeInterest,
};
pub use pool::{
    HostedPoolError, HostedPoolRuntime, HostedPoolStepError, HostedPoolStepObservation,
    instantiate_plan_pool, instantiate_pool, observe_pool_step,
};
#[cfg(target_os = "linux")]
pub use resource_effect::linux::{commit_file, commit_process, commit_socket, force_kill_and_wait};
pub use resource_effect::{
    DeterministicEffectBackend, DeterministicEffectFault, HostedEffectDisposition,
    HostedEffectError, HostedLeaseUse,
};
pub use runtime_evidence::{
    RuntimeEvidenceContext, RuntimeEvidenceError, record_scheduler_evidence,
};
pub use scheduler::{
    DeterministicExecutor, RuntimeTimestamp, RuntimeValue, RuntimeValueEnvelope, ScheduledNode,
    SchedulerAllocation, SchedulerError, SchedulerEvent, SchedulerEventKind, SchedulerHighWater,
    SchedulerNode, SchedulerReservation, SchedulerStatus, SchedulerStep, SchedulerSubject,
    SendStatus, StepIo, validate_runtime_value_for_cord,
};
pub use source_lowering::{
    ConfigProvenance, LOWERED_SOURCE_SCHEMA_V1, LOWERED_SOURCE_SCHEMA_V2, LOWERED_SOURCE_SCHEMA_V3,
    LOWERED_SOURCE_SCHEMA_V4, LiteralValidationError, LoweredBindingV2, LoweredCompositeChildV2,
    LoweredCompositeV2, LoweredConfigEntry, LoweredConfigValue, LoweredCordV2, LoweredExportV2,
    LoweredGroupPort, LoweredInterfaceMemberProofV4, LoweredInterfaceProofV4, LoweredNode,
    LoweredNodeV2, LoweredPool, LoweredRootSelectionV2, LoweredSource, LoweredSourceV2,
    LoweredSourceV3, LoweredSourceV4, LoweredSupervisionV3, LoweringDiagnostic,
    OwnedConfigFieldSchema, OwnedConfigRequirement, OwnedInterfaceContract, OwnedInterfaceMember,
    OwnedNodeContract, OwnedNodeSchema, OwnedPortContract, OwnedPortReference, OwnedSemanticValue,
    OwnedTypeReference, SOURCE_AST_SCHEMA_V2, SOURCE_AST_SCHEMA_V3, SOURCE_AST_SCHEMA_V4,
    SOURCE_AST_SCHEMA_V5, SourceContractCatalog, SourceMapEntry, SourceOrigin,
    VersionedLoweredSource, lower_source, lower_source_v2, lower_source_v3, lower_source_v4,
    lower_source_version, migrate_lowered_source_v1,
};
pub use supervision::BoundedSupervisionRuntime;
pub use transition::{
    HostedDrainObservation, HostedGenerationBinding, HostedTransitionAdmission,
    HostedTransitionAdmissionError, HostedTransitionError, HostedTransitionGeneration,
    HostedTransitionReservation, HostedTransitionTransaction, RetainedReplayItem,
    RetainedReplayProvider, StableBoundaryRouter, admit_hosted_transition,
};
pub use transport::{
    CarrierSecurityCapabilities, CarrierSecurityMode, DISTRIBUTED_ENVELOPE_FIXED_BYTES,
    DISTRIBUTED_ENVELOPE_V2_FIXED_BYTES, DISTRIBUTED_ENVELOPE_VERSION,
    DISTRIBUTED_ENVELOPE_VERSION_V1, DISTRIBUTED_ENVELOPE_VERSION_V2, DecodedDistributedEnvelope,
    ResolvedTransportSelection, TransportCapabilities, TransportReason, TransportTransition,
    decode_distributed_envelope, decode_distributed_envelope_v2, encode_distributed_envelope,
    encode_distributed_envelope_v2, validate_transport_selection, validate_transport_transition,
};
pub use type_registry::{
    ProviderTypeDecision, TypeComparisonStrategy, TypeContractDescription, TypeContractProvider,
    TypeRegistry, TypeRegistryError, TypeSatisfactionReport,
};
pub use workload::{
    LinuxWorkloadObservation, WorkloadRunEvidence, observe_linux_workload,
    run_deterministic_workload,
};

/// Allocator-aware convenience around the core-compatible exact-plan validator.
pub fn validate_hosted_execution_plan(
    plan: &conduit_core::ExecutionPlan<'_>,
    context: conduit_core::PlanValidationContext<'_>,
) -> Result<(), conduit_core::PlanValidationError> {
    let fact_count =
        plan.validation_scratch_count()
            .map_err(|_| conduit_core::PlanValidationError {
                code: conduit_core::PlanDiagnosticCode::InvalidDescriptor,
                collection: conduit_core::PlanCollection::Header,
                subject_index: None,
            })?;
    let mut scratch = vec![SemanticHash::from_bytes([0; 32]); fact_count];
    conduit_core::validate_execution_plan(plan, context, &mut scratch)?;
    let nodes = plan
        .nodes
        .iter()
        .map(|node| node.instance)
        .collect::<Vec<_>>();
    let mut removed = vec![false; nodes.len()];
    conduit_core::validate_feedback_graph(
        &nodes,
        plan.cords,
        plan.feedback_boundaries,
        &mut removed,
    )
    .map_err(|reason| conduit_core::PlanValidationError {
        code: conduit_core::PlanDiagnosticCode::ValueEnvelope(reason),
        collection: conduit_core::PlanCollection::FeedbackBoundaries,
        subject_index: None,
    })
}

const TEXT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/text"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([
        0x79, 0xdd, 0x1d, 0x77, 0xe2, 0xcf, 0x64, 0x59, 0xbc, 0x3a, 0x8f, 0x96, 0xc6, 0x5a, 0x91,
        0x5a, 0xdc, 0x10, 0xdb, 0x51, 0x6d, 0xca, 0xc0, 0x39, 0xf7, 0x81, 0xbe, 0xe5, 0xc1, 0xca,
        0xb5, 0xab,
    ]),
};
const BYTES_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/bytes"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([
        0x7b, 0xe7, 0xdf, 0x9a, 0x17, 0xc7, 0x5a, 0x28, 0xc8, 0xb5, 0xdf, 0x5f, 0xa6, 0xea, 0x6a,
        0x85, 0x9d, 0x88, 0x86, 0x69, 0x91, 0x3d, 0x83, 0x6d, 0xe2, 0xc6, 0x14, 0x1c, 0x8d, 0x19,
        0xd4, 0x53,
    ]),
};
const FORMAT_VALUES_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/format-values"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([
        0xba, 0x23, 0xe2, 0x76, 0xb7, 0x0b, 0x1b, 0x0c, 0x74, 0x7d, 0x2b, 0x4a, 0xda, 0x10, 0x0d,
        0x72, 0xfa, 0x5b, 0x38, 0x74, 0xe4, 0xfa, 0x2b, 0xaa, 0x25, 0x0c, 0xf0, 0x71, 0x49, 0x79,
        0x5c, 0xc0,
    ]),
};
const TERMINAL_OBSERVATION_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/terminal"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([
        0xfb, 0xab, 0x7e, 0x8b, 0xbc, 0x24, 0xca, 0x50, 0xa4, 0xb3, 0x73, 0x91, 0x1e, 0x8f, 0xf2,
        0xee, 0xa9, 0xac, 0xb9, 0x23, 0x27, 0xd9, 0xa4, 0x57, 0xa2, 0x4b, 0x05, 0x20, 0x5d, 0x13,
        0x4d, 0x1c,
    ]),
};
const SUPERVISION_DECISION_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("supervision/decision"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([
        0x81, 0xca, 0x8b, 0xc6, 0xdd, 0x48, 0xc4, 0x68, 0x84, 0x32, 0x8a, 0x30, 0x22, 0xae, 0x41,
        0xc0, 0xd1, 0x98, 0x2a, 0x1a, 0x04, 0x51, 0x15, 0xf2, 0xed, 0x5d, 0x89, 0xba, 0xc6, 0x7c,
        0xb2, 0x46,
    ]),
};
const EMPTY_CONFIG: ConfigContract<'static> = ConfigContract { fields: &[] };
const fn optional_text_field(key: &'static str) -> ConfigFieldContract<'static> {
    ConfigFieldContract {
        key: Id(key),
        value_type: TEXT_TYPE,
        requirement: ConfigRequirement::Optional,
        sensitivity: Sensitivity::Public,
        mutability: ConfigMutability::PreStart,
        identity: ConfigIdentity::Semantic,
    }
}
const TEE_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[optional_text_field("mode")],
};
const MERGE_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[optional_text_field("ordering")],
};
const ZIP_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[optional_text_field("unpaired")],
};
const GATE_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        optional_text_field("initial"),
        optional_text_field("retained"),
    ],
};
const SELECT_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        optional_text_field("initial"),
        optional_text_field("inactive"),
    ],
};
const LITERAL_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[ConfigFieldContract {
        key: Id("value"),
        value_type: TEXT_TYPE,
        requirement: ConfigRequirement::Required,
        sensitivity: Sensitivity::Public,
        mutability: ConfigMutability::PreStart,
        identity: ConfigIdentity::Semantic,
    }],
};
const FORMAT_VALUES_LITERAL_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[ConfigFieldContract {
        key: Id("values"),
        value_type: FORMAT_VALUES_TYPE,
        requirement: ConfigRequirement::Required,
        sensitivity: Sensitivity::Public,
        mutability: ConfigMutability::PreStart,
        identity: ConfigIdentity::Semantic,
    }],
};
const HTTP_SERVE_ONCE_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        ConfigFieldContract {
            key: Id("listen"),
            value_type: TEXT_TYPE,
            requirement: ConfigRequirement::Required,
            sensitivity: Sensitivity::Public,
            mutability: ConfigMutability::PreStart,
            identity: ConfigIdentity::Plan,
        },
        ConfigFieldContract {
            key: Id("method"),
            value_type: TEXT_TYPE,
            requirement: ConfigRequirement::Required,
            sensitivity: Sensitivity::Public,
            mutability: ConfigMutability::PreStart,
            identity: ConfigIdentity::Semantic,
        },
        ConfigFieldContract {
            key: Id("path"),
            value_type: TEXT_TYPE,
            requirement: ConfigRequirement::Required,
            sensitivity: Sensitivity::Public,
            mutability: ConfigMutability::PreStart,
            identity: ConfigIdentity::Semantic,
        },
        ConfigFieldContract {
            key: Id("response"),
            value_type: TEXT_TYPE,
            requirement: ConfigRequirement::Required,
            sensitivity: Sensitivity::Public,
            mutability: ConfigMutability::PreStart,
            identity: ConfigIdentity::Semantic,
        },
        ConfigFieldContract {
            key: Id("deadline_ms"),
            value_type: TEXT_TYPE,
            requirement: ConfigRequirement::Required,
            sensitivity: Sensitivity::Public,
            mutability: ConfigMutability::PreStart,
            identity: ConfigIdentity::Plan,
        },
    ],
};
const TEXT_INPUT: PortContract<'static> = PortContract {
    id: Id("text"),
    direction: Direction::Input,
    value_type: TEXT_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::ExactlyOne,
    values: ValueCardinality::ExactlyOne,
    delivery: Delivery::FiniteBatch,
    temporal: TemporalContract::Atemporal,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Public,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
};
const TEXT_OUTPUT: PortContract<'static> = PortContract {
    id: Id("text"),
    direction: Direction::Output,
    value_type: TEXT_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::OneOrMore,
    values: ValueCardinality::ExactlyOne,
    delivery: Delivery::FiniteBatch,
    temporal: TemporalContract::Atemporal,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Public,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
};
const VALUE_TEXT_INPUT: PortContract<'static> = PortContract {
    id: Id("value"),
    ..TEXT_INPUT
};
const VALUE_TEXT_OUTPUT: PortContract<'static> = PortContract {
    id: Id("value"),
    ..TEXT_OUTPUT
};
const BYTES_INPUT: PortContract<'static> = PortContract {
    id: Id("bytes"),
    value_type: BYTES_TYPE,
    ..TEXT_INPUT
};
const BYTES_OUTPUT: PortContract<'static> = PortContract {
    id: Id("bytes"),
    value_type: BYTES_TYPE,
    ..TEXT_OUTPUT
};
const FORMAT_TEMPLATE_INPUT: PortContract<'static> = PortContract {
    id: Id("template"),
    direction: Direction::Input,
    value_type: TEXT_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::ExactlyOne,
    values: ValueCardinality::ExactlyOne,
    delivery: Delivery::FiniteBatch,
    temporal: TemporalContract::Atemporal,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Public,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
};
const FORMAT_VALUES_INPUT: PortContract<'static> = PortContract {
    id: Id("values"),
    direction: Direction::Input,
    value_type: FORMAT_VALUES_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::ExactlyOne,
    values: ValueCardinality::ExactlyOne,
    delivery: Delivery::FiniteBatch,
    temporal: TemporalContract::Atemporal,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Public,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
};
const FORMAT_VALUES_OUTPUT: PortContract<'static> = PortContract {
    id: Id("values"),
    direction: Direction::Output,
    value_type: FORMAT_VALUES_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::OneOrMore,
    values: ValueCardinality::ExactlyOne,
    delivery: Delivery::FiniteBatch,
    temporal: TemporalContract::Atemporal,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Public,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
};
const TERMINAL_OBSERVATION_INPUT: PortContract<'static> = PortContract {
    id: Id("terminal"),
    direction: Direction::Input,
    value_type: TERMINAL_OBSERVATION_TYPE,
    // This typed value is delivered by the exact supervision binding on the
    // control plane, not by a source-authored data cord.
    presence: Presence::Optional,
    connections: ConnectionCardinality::ZeroOrOne,
    values: ValueCardinality::ZeroOrMore,
    delivery: Delivery::Stream,
    temporal: TemporalContract::Progressive,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Restricted,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
};
const SUPERVISION_DECISION_OUTPUT: PortContract<'static> = PortContract {
    id: Id("decision"),
    direction: Direction::Output,
    value_type: SUPERVISION_DECISION_TYPE,
    // The exact supervision binding consumes this control value directly.
    presence: Presence::Optional,
    connections: ConnectionCardinality::ZeroOrOne,
    values: ValueCardinality::ZeroOrMore,
    delivery: Delivery::Stream,
    temporal: TemporalContract::Progressive,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Restricted,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
};

const INPUT_PRIMARY: PortContract<'static> = PortContract {
    id: Id("primary"),
    direction: Direction::Input,
    value_type: TEXT_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::ExactlyOne,
    values: ValueCardinality::ExactlyOne,
    delivery: Delivery::FiniteBatch,
    temporal: TemporalContract::Atemporal,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Public,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
};
const INPUT_FALLBACK: PortContract<'static> = PortContract {
    id: Id("fallback"),
    direction: Direction::Input,
    value_type: TEXT_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::ExactlyOne,
    values: ValueCardinality::ExactlyOne,
    delivery: Delivery::FiniteBatch,
    temporal: TemporalContract::Atemporal,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Public,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
};
const STREAM_INPUT_TEXT: PortContract<'static> = PortContract {
    values: ValueCardinality::ZeroOrMore,
    delivery: Delivery::Stream,
    temporal: TemporalContract::Committed,
    terminal: TerminalContract::Either,
    sensitivity: Sensitivity::Restricted,
    id: Id("value"),
    ..TEXT_INPUT
};
const STREAM_INPUT_TEXT_1: PortContract<'static> = PortContract {
    id: Id("left"),
    ..STREAM_INPUT_TEXT
};
const STREAM_INPUT_TEXT_2: PortContract<'static> = PortContract {
    id: Id("right"),
    ..STREAM_INPUT_TEXT
};
const STREAM_CONTROL_TEXT: PortContract<'static> = PortContract {
    id: Id("command"),
    ..STREAM_INPUT_TEXT
};
const STREAM_OUTPUT_TEXT: PortContract<'static> = PortContract {
    values: ValueCardinality::ZeroOrMore,
    delivery: Delivery::Stream,
    temporal: TemporalContract::Committed,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Restricted,
    id: Id("value"),
    ..TEXT_OUTPUT
};
const STREAM_OUTPUT_TEXT_1: PortContract<'static> = PortContract {
    id: Id("left"),
    ..STREAM_OUTPUT_TEXT
};
const STREAM_OUTPUT_TEXT_2: PortContract<'static> = PortContract {
    id: Id("right"),
    ..STREAM_OUTPUT_TEXT
};
const STREAM_OUTPUT_TEXT_LEFT: PortContract<'static> = PortContract {
    id: Id("left"),
    ..STREAM_OUTPUT_TEXT
};
const STREAM_OUTPUT_TEXT_RIGHT: PortContract<'static> = PortContract {
    id: Id("right"),
    ..STREAM_OUTPUT_TEXT
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryError {
    pub code: &'static str,
    pub message: String,
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for RegistryError {}

pub const LITERAL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("std/literal"),
    config: LITERAL_CONFIG,
    inputs: &[],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const STDIN_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("io/stdin"),
    config: EMPTY_CONFIG,
    inputs: &[],
    outputs: &[BYTES_OUTPUT],
};
pub const UPPERCASE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("text/uppercase"),
    config: EMPTY_CONFIG,
    inputs: &[TEXT_INPUT],
    outputs: &[TEXT_OUTPUT],
};
pub const ENCODE_UTF8_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("text/encode-utf8"),
    config: EMPTY_CONFIG,
    inputs: &[TEXT_INPUT],
    outputs: &[BYTES_OUTPUT],
};
pub const FORMAT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("std/text/format"),
    config: EMPTY_CONFIG,
    inputs: &[FORMAT_TEMPLATE_INPUT, FORMAT_VALUES_INPUT],
    outputs: &[TEXT_OUTPUT],
};
pub const FORMAT_VALUES_LITERAL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("std/format-values/literal"),
    config: FORMAT_VALUES_LITERAL_CONFIG,
    inputs: &[],
    outputs: &[FORMAT_VALUES_OUTPUT],
};
pub const STDOUT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("io/stdout"),
    config: EMPTY_CONFIG,
    inputs: &[BYTES_INPUT],
    outputs: &[],
};
pub const STDERR_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("io/stderr"),
    config: EMPTY_CONFIG,
    inputs: &[BYTES_INPUT],
    outputs: &[],
};
pub const DISPLAY_TEXT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("display/text"),
    config: EMPTY_CONFIG,
    inputs: &[TEXT_INPUT],
    outputs: &[],
};
pub const SUPERVISOR_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("supervision/supervisor"),
    config: EMPTY_CONFIG,
    inputs: &[TERMINAL_OBSERVATION_INPUT],
    outputs: &[SUPERVISION_DECISION_OUTPUT],
};
pub const PASS_THROUGH_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("flow/identity"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const TEE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/tee"),
    config: TEE_CONFIG,
    inputs: &[STREAM_INPUT_TEXT],
    outputs: &[STREAM_OUTPUT_TEXT_1, STREAM_OUTPUT_TEXT_2],
};
pub const MERGE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/merge"),
    config: MERGE_CONFIG,
    inputs: &[STREAM_INPUT_TEXT_1, STREAM_INPUT_TEXT_2],
    outputs: &[STREAM_OUTPUT_TEXT],
};
pub const ZIP_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/zip"),
    config: ZIP_CONFIG,
    inputs: &[STREAM_INPUT_TEXT_1, STREAM_INPUT_TEXT_2],
    outputs: &[STREAM_OUTPUT_TEXT_LEFT, STREAM_OUTPUT_TEXT_RIGHT],
};
pub const GATE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/gate"),
    config: GATE_CONFIG,
    inputs: &[STREAM_INPUT_TEXT, STREAM_CONTROL_TEXT],
    outputs: &[STREAM_OUTPUT_TEXT],
};
pub const SELECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/select"),
    config: SELECT_CONFIG,
    inputs: &[
        STREAM_INPUT_TEXT_1,
        STREAM_INPUT_TEXT_2,
        STREAM_CONTROL_TEXT,
    ],
    outputs: &[STREAM_OUTPUT_TEXT],
};
pub const DELAY_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("time/delay"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const DEBOUNCE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("time/debounce"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const THROTTLE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("time/throttle"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const TAKE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("flow/take"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const SKIP_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("flow/skip"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const FILTER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("flow/filter"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const FALLBACK_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("flow/fallback"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_PRIMARY, INPUT_FALLBACK],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const PROBE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("test/probe"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const LOG_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("observe/log"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const ASSERT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("test/assertion"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const RECORD_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("test/record"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const REPLAY_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("test/replay"),
    config: EMPTY_CONFIG,
    inputs: &[],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const FAULT_SOURCE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("test/fault-source"),
    config: EMPTY_CONFIG,
    inputs: &[],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const FILE_READ_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("fs/read"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const FILE_WRITE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("fs/write"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[],
};
pub const BLOB_STORE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("storage/blob/store"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const KV_STORE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("storage/key-value"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const PROCESS_SPAWN_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("process/run"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const GPIO_PIN_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("device/gpio/pin"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const SERIAL_PORT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("device/serial/port"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const CELL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("state/cell"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const COUNTER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("state/counter"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const DEDUPLICATE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("state/deduplicate"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const CACHE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("state/cache"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const CIRCUIT_BREAKER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("supervision/circuit-breaker"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const HEALTH_GATE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("supervision/health-gate"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const BACKOFF_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("supervision/backoff"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const WIFI_STATION_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/wifi/join"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const WIFI_AP_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/wifi/access-point"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const NETWORK_INTERFACE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/interface"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const TCP_SOCKET_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/tcp/socket"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const UDP_SOCKET_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/udp/socket"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
pub const DNS_RESOLVER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/dns/resolve"),
    config: EMPTY_CONFIG,
    inputs: &[VALUE_TEXT_INPUT],
    outputs: &[VALUE_TEXT_OUTPUT],
};
/// Minimal bounded hosted HTTP service boundary.
///
/// Rich HTTP request/response/route contracts remain in `conduit-http`; this
/// source-facing node binds exactly one finite route and terminates after one
/// exchange.
pub const HTTP_SERVE_ONCE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/http/serve-once"),
    config: HTTP_SERVE_ONCE_CONFIG,
    inputs: &[],
    outputs: &[],
};

/// Typed runtime value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Value {
    /// Exact semantic type identity.
    pub value_type: TypeContractRef<'static>,
    /// Canonical or implementation-agreed payload bytes.
    pub bytes: Vec<u8>,
}

impl Value {
    #[must_use]
    pub fn text(value: impl Into<Vec<u8>>) -> Self {
        Self {
            value_type: TEXT_TYPE,
            bytes: value.into(),
        }
    }

    #[must_use]
    pub fn bytes(value: impl Into<Vec<u8>>) -> Self {
        Self {
            value_type: BYTES_TYPE,
            bytes: value.into(),
        }
    }
}

/// Process boundary supplied by the host.
pub struct RunIo<'a> {
    /// Process standard input.
    pub input: &'a mut dyn Read,
    /// Process standard output.
    pub output: &'a mut dyn Write,
    /// Process standard error.
    pub error: &'a mut dyn Write,
}

/// Behavior-specific hosted implementation selected by an exact binding.
///
/// This is deliberately not inferred from a semantic contract. A caller must
/// explicitly associate one of these implementations with the exact
/// implementation and artifact identities named by the plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedPrimitiveImplementation {
    Literal,
    FormatValuesLiteral,
    Format,
    Lines,
    Join,
    Stdin,
    Uppercase,
    EncodeUtf8,
    Stdout,
    Stderr,
    PassThrough,
    Tee,
    Merge,
    Zip,
    Gate,
    Select,
    Fallback,
    /// An exact, registered host-service provider with no value cords.
    ///
    /// The provider callback remains coupled to its registered manifest and
    /// artifact. This marker never derives behavior from a semantic ID.
    HostedService,
}

/// One installed hosted implementation binding available to an exact run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactHostedBinding {
    pub implementation_id: String,
    pub implementation_identity: SemanticHash,
    pub artifact_id: String,
    pub artifact_digest: conduit_core::ArtifactDigest,
    pub implementation: HostedPrimitiveImplementation,
}

/// Finite caller-supplied implementation set for one exact hosted run.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExactHostedBindings {
    bindings: Vec<ExactHostedBinding>,
}

impl ExactHostedBindings {
    pub fn new(bindings: Vec<ExactHostedBinding>) -> Result<Self, RuntimeError> {
        let mut identities = BTreeSet::new();
        for binding in &bindings {
            Id::new(&binding.implementation_id).map_err(|_| {
                RuntimeError::new(
                    "CND-RUN-007",
                    "hosted binding has an invalid implementation identity",
                )
            })?;
            Id::new(&binding.artifact_id).map_err(|_| {
                RuntimeError::new(
                    "CND-RUN-007",
                    "hosted binding has an invalid artifact identity",
                )
            })?;
            let key = format!(
                "{}@{}",
                binding.implementation_id, binding.implementation_identity
            );
            if !identities.insert(key) {
                return Err(RuntimeError::new(
                    "CND-RUN-007",
                    "hosted binding set contains a duplicate implementation",
                ));
            }
        }
        Ok(Self { bindings })
    }

    fn resolve(
        &self,
        node: &ResolvedPlanNode<'_>,
        artifacts: &[PlanArtifact<'_>],
    ) -> Result<HostedPrimitiveImplementation, RuntimeError> {
        let binding = self
            .bindings
            .iter()
            .find(|binding| {
                binding.implementation_id == node.implementation.id.as_str()
                    && binding.implementation_identity == node.implementation.semantic_hash
            })
            .ok_or_else(|| {
                RuntimeError::new(
                    "CND-RUN-007",
                    format!(
                        "no installed implementation matches exact binding `{}`",
                        node.implementation.id
                    ),
                )
            })?;
        if binding.artifact_id != node.artifact.as_str()
            || !artifacts.iter().any(|artifact| {
                artifact.id.as_str() == binding.artifact_id
                    && artifact.digest == binding.artifact_digest
            })
        {
            return Err(RuntimeError::new(
                "CND-RUN-008",
                format!(
                    "installed implementation `{}` lacks the plan's exact artifact",
                    node.implementation.id
                ),
            ));
        }
        Ok(binding.implementation)
    }
}

/// Exact non-provider inputs that bound one hosted executor run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactRunContext<'a> {
    pub semantic_source_hash: SemanticHash,
    pub plan_epoch: u64,
    pub run_id: Id<'a>,
    pub validation: conduit_core::PlanValidationContext<'a>,
    pub scheduler_policy: SchedulerPolicy,
    pub reservation: SchedulerReservation,
}

/// Bounded observations returned by one terminal exact-plan execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactExecutionReport {
    pub summary: ExecutionSummary,
    pub terminal: TerminalClass,
    pub allocation: SchedulerAllocation,
    pub high_water: SchedulerHighWater,
    pub scheduler_events: Vec<SchedulerEvent>,
    pub evidence: Vec<ExactEvidenceRecord>,
    pub evidence_bytes: u64,
}

pub trait Handler {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError>;
}

pub type HandlerFactory = fn() -> Box<dyn Handler>;
pub type ConfigValidator = fn(&Node) -> Result<(), ResolutionError>;

/// Static facts and callbacks for one provider linked into the current host
/// executable.
pub struct CompiledInHostService {
    pub contract: &'static NodeContract<'static>,
    pub implementation_id: &'static str,
    pub artifact_id: &'static str,
    pub entrypoint: &'static str,
    pub source_bytes: &'static [u8],
    pub required_authorities: &'static [SemanticHash],
    pub factory: HandlerFactory,
    pub validate_config: ConfigValidator,
}

#[derive(Debug)]
pub struct RegisteredExecutable {
    pub manifest: &'static ImplementationManifest<'static>,
    pub artifacts: &'static [&'static ArtifactManifest<'static>],
    pub factory: HandlerFactory,
    pub validate_config: ConfigValidator,
}

#[derive(Debug)]
struct CompatibilityExecutable {
    factory: HandlerFactory,
    validate_config: ConfigValidator,
}

#[derive(Debug)]
struct RegisteredNode {
    contract: &'static NodeContract<'static>,
    executable: Option<RegisteredExecutable>,
    compatibility_executable: Option<CompatibilityExecutable>,
}

/// Exact compiled-in provider facts independently trusted by the hosted runtime.
#[derive(Clone, Copy, Debug)]
pub struct InstalledHostedProvider {
    pub contract: &'static NodeContract<'static>,
    pub manifest: &'static ImplementationManifest<'static>,
    pub artifact: &'static ArtifactManifest<'static>,
    pub implementation: HostedPrimitiveImplementation,
}

#[derive(Clone, Copy)]
struct HostedProviderDefinition {
    installed: InstalledHostedProvider,
    artifacts: &'static [&'static ArtifactManifest<'static>],
    factory: HandlerFactory,
    validate_config: ConfigValidator,
}

impl RegisteredNode {
    fn factory(&self) -> HandlerFactory {
        self.executable
            .as_ref()
            .map(|executable| executable.factory)
            .or_else(|| {
                self.compatibility_executable
                    .as_ref()
                    .map(|executable| executable.factory)
            })
            .expect("resolved node has executable implementation")
    }
}

/// Operational availability state of a node or provider contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AvailabilityState {
    /// Semantic contract present, but no executable provider implementation is registered.
    ContractOnly,
    /// Executable provider implementation is registered in the host registry.
    ProviderAvailable,
    /// Provider implementation is registered and satisfied for host execution.
    ResolvableOnThisHost,
    /// Node is bound to a specific node instance in an execution plan.
    BoundInThisPlan,
    /// Node instance is running.
    Running,
    /// Node is unsupported due to missing provider, artifact, capability, or grant.
    Unsupported,
}

impl AvailabilityState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ContractOnly => "contract-only",
            Self::ProviderAvailable => "provider-available",
            Self::ResolvableOnThisHost => "resolvable-on-this-host",
            Self::BoundInThisPlan => "bound-in-this-plan",
            Self::Running => "running",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Structured availability facts with stable reason codes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodeAvailability {
    pub contract_id: String,
    pub state: AvailabilityState,
    pub reason_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implementation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_identity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub rejection_reasons: Vec<String>,
}

impl NodeAvailability {
    pub fn from_resolved_binding(
        contract: PinnedDescriptor<'_>,
        binding: &ResolvedPlacementBinding,
    ) -> Result<Self, RegistryError> {
        if binding.semantic_contract != contract.semantic_hash {
            return Err(RegistryError {
                code: "CND-REG-006",
                message: format!(
                    "resolved binding for `{}` names a different semantic contract",
                    binding.instance
                ),
            });
        }
        Ok(Self {
            contract_id: contract.id.as_str().to_owned(),
            state: AvailabilityState::ResolvableOnThisHost,
            reason_code: "CND-AVL-003".to_owned(),
            implementation_id: Some(binding.implementation_id.clone()),
            host_id: Some(binding.host.clone()),
            plan_identity: None,
            run_id: None,
            rejection_reasons: Vec::new(),
        })
    }
}

/// Built-in hosted implementation registry.
///
/// Registry identity and discovery are deliberately above `conduit-core`.
pub struct Registry {
    nodes: BTreeMap<&'static str, RegisteredNode>,
    interfaces: BTreeMap<String, OwnedInterfaceContract>,
    types: TypeRegistry,
    allow_installed_resolution: bool,
}

impl Registry {
    /// Register one linked, source-attested host-service implementation.
    ///
    /// The returned executable identity is derived from the exact semantic
    /// contract and linked source bytes; callers cannot substitute a planner
    /// candidate name for installed code.
    pub fn register_compiled_in_host_service(
        &mut self,
        service: CompiledInHostService,
    ) -> Result<(), RegistryError> {
        let source_digest = ArtifactDigest::from_bytes(Sha256::digest(service.source_bytes).into());
        let mut artifact = ArtifactManifest {
            schema_version: 1,
            identity: SemanticHash::from_bytes([0; 32]),
            id: Id(service.artifact_id),
            digest: source_digest,
            media_type: "application/vnd.conduit.compiled-in-provider",
            byte_size: u64::try_from(service.source_bytes.len()).map_err(|_| RegistryError {
                code: "CND-REG-008",
                message: "linked host-service artifact is too large".to_owned(),
            })?,
            target: Some(Id(std::env::consts::ARCH)),
            abi: Some(Id("conduit/rust-in-process-v1")),
            provenance: ArtifactProvenance {
                builder: Id("conduit/rustc-workspace-build"),
                source_digest,
                build_recipe_digest: ArtifactDigest::from_bytes(
                    Sha256::digest(b"cargo build --workspace").into(),
                ),
                reproducible: true,
            },
            signatures: &[],
            license_expressions: &["MIT", "Apache-2.0"],
            notices: &[],
            sbom: None,
            source: None,
            related_artifacts: &[],
            locations: &[],
        };
        let mut artifact_scratch =
            vec![SemanticHash::from_bytes([0; 32]); artifact.identity_fact_count()];
        artifact.identity = artifact
            .computed_semantic_hash(&mut artifact_scratch)
            .map_err(|_| RegistryError {
                code: "CND-REG-008",
                message: "linked host-service artifact identity is invalid".to_owned(),
            })?;
        let artifact = &*Box::leak(Box::new(artifact));
        let artifacts: &'static [&'static ArtifactManifest<'static>] =
            Box::leak(Box::new([artifact]));
        let references = Box::leak(Box::new([ManifestArtifactRef {
            id: artifact.id,
            digest: artifact.digest,
            role: Id("executable"),
            required: true,
        }]));
        let mut manifest = ImplementationManifest {
            schema_version: 1,
            identity: SemanticHash::from_bytes([0; 32]),
            id: Id(service.implementation_id),
            implementation_version: "1",
            semantic_contract: PinnedDescriptor {
                id: service.contract.id,
                schema_version: 1,
                semantic_hash: OwnedNodeSchema::from_contract(service.contract).semantic_hash(),
            },
            executor: ExecutorKind::NativeInProcess,
            entrypoint: ManifestEntrypoint {
                name: Id(service.entrypoint),
                adapter: Id("conduit/host-service-step"),
                abi: Id("conduit/host-service-v1"),
                protocol_version: 1,
            },
            execution_profile: PinnedDescriptor {
                id: Id("conduit/hosted-primitive-profile-v1"),
                schema_version: 1,
                semantic_hash: SemanticHash::from_bytes(
                    Sha256::digest(b"conduit/hosted-primitive-profile/v1").into(),
                ),
            },
            artifacts: references,
            required_interfaces: &[],
            provided_interfaces: &[],
            required_authorities: service.required_authorities,
            required_effects: &[],
            minimum_plan_version: 1,
            maximum_plan_version: u32::MAX,
            minimum_runtime_protocol: 1,
            maximum_runtime_protocol: 1,
            replacement: ReplacementSupport::Cold,
            coexistence_memory_bytes: 0,
            reproducibility: None,
        };
        let mut manifest_scratch =
            vec![SemanticHash::from_bytes([0; 32]); manifest.identity_fact_count()];
        manifest.identity = manifest
            .computed_semantic_hash(&mut manifest_scratch)
            .map_err(|_| RegistryError {
                code: "CND-REG-007",
                message: "linked host-service manifest identity is invalid".to_owned(),
            })?;
        let manifest = &*Box::leak(Box::new(manifest));
        self.register_executable_provider(
            service.contract,
            manifest,
            artifacts,
            service.factory,
            service.validate_config,
        )
    }

    /// Returns the finite executable provider inventory registered in this
    /// exact host registry.
    #[must_use]
    pub fn installed_providers(&self) -> Vec<InstalledHostedProvider> {
        self.nodes
            .values()
            .filter_map(|node| {
                let executable = node.executable.as_ref()?;
                let artifact_ref = executable.manifest.artifacts.first()?;
                let artifact = executable.artifacts.iter().copied().find(|artifact| {
                    artifact.id == artifact_ref.id && artifact.digest == artifact_ref.digest
                })?;
                let implementation = Self::installed_hosted_providers()
                    .iter()
                    .find(|installed| installed.manifest.id == executable.manifest.id)
                    .map_or(HostedPrimitiveImplementation::HostedService, |installed| {
                        installed.implementation
                    });
                Some(InstalledHostedProvider {
                    contract: node.contract,
                    manifest: executable.manifest,
                    artifact,
                    implementation,
                })
            })
            .collect()
    }

    pub fn register_interface(&mut self, interface: OwnedInterfaceContract) {
        self.interfaces.insert(interface.id.clone(), interface);
    }

    /// Resolves a contract ID to its canonical semantic ID.
    pub fn resolve_canonical_id<'a>(&'a self, contract_id: &str) -> Result<&'a str, RegistryError> {
        if let Some((canonical, _)) = self.nodes.get_key_value(contract_id) {
            return Ok(canonical);
        }
        Err(RegistryError {
            code: "CND-REG-003",
            message: format!("unknown contract id `{}`", contract_id),
        })
    }

    fn get_registered_node(&self, contract_id: &str) -> Option<&RegisteredNode> {
        self.nodes.get(contract_id)
    }

    /// Registers a concrete executable provider implementation with manifest and host resolution evidence.
    pub fn register_executable_provider(
        &mut self,
        contract: &'static NodeContract<'static>,
        manifest: &'static ImplementationManifest<'static>,
        artifacts: &'static [&'static ArtifactManifest<'static>],
        factory: HandlerFactory,
        validate_config: ConfigValidator,
    ) -> Result<(), RegistryError> {
        let canonical_target_id = self.resolve_canonical_id(contract.id.as_str())?;
        let manifest_target_canonical =
            self.resolve_canonical_id(manifest.semantic_contract.id.as_str())?;

        if manifest_target_canonical != canonical_target_id {
            return Err(RegistryError {
                code: "CND-REG-004",
                message: format!(
                    "cross-contract semantic impersonation rejected: handler advertised contract `{}` cannot implement `{}`",
                    manifest.semantic_contract.id,
                    contract.id.as_str()
                ),
            });
        }

        let expected_hash = OwnedNodeSchema::from_contract(contract).semantic_hash();
        if manifest.semantic_contract.semantic_hash != expected_hash {
            return Err(RegistryError {
                code: "CND-REG-005",
                message: format!(
                    "contract hash mismatch for `{}`: manifest specifies `{}`, expected `{}`",
                    contract.id.as_str(),
                    manifest.semantic_contract.semantic_hash,
                    expected_hash
                ),
            });
        }
        let mut manifest_scratch =
            vec![SemanticHash::from_bytes([0; 32]); manifest.identity_fact_count()];
        validate_implementation_manifest(manifest, &mut manifest_scratch).map_err(|reason| {
            RegistryError {
                code: "CND-REG-007",
                message: format!(
                    "invalid implementation manifest `{}`: {}",
                    manifest.id,
                    reason.code()
                ),
            }
        })?;
        for artifact_ref in manifest.artifacts {
            let artifact = artifacts
                .iter()
                .copied()
                .find(|artifact| {
                    artifact.id == artifact_ref.id && artifact.digest == artifact_ref.digest
                })
                .ok_or_else(|| RegistryError {
                    code: "CND-REG-008",
                    message: format!(
                        "implementation manifest `{}` is missing exact artifact `{}`",
                        manifest.id, artifact_ref.id
                    ),
                })?;
            let mut artifact_scratch =
                vec![SemanticHash::from_bytes([0; 32]); artifact.identity_fact_count()];
            validate_artifact_manifest(artifact, &mut artifact_scratch).map_err(|reason| {
                RegistryError {
                    code: "CND-REG-008",
                    message: format!(
                        "invalid artifact manifest `{}` for implementation `{}`: {}",
                        artifact.id,
                        manifest.id,
                        reason.code()
                    ),
                }
            })?;
        }

        self.nodes.insert(
            contract.id.as_str(),
            RegisteredNode {
                contract,
                executable: Some(RegisteredExecutable {
                    manifest,
                    artifacts,
                    factory,
                    validate_config,
                }),
                compatibility_executable: None,
            },
        );
        Ok(())
    }

    /// Registers a semantic contract as contract-only.
    pub fn register_contract_only(&mut self, contract: &'static NodeContract<'static>) {
        self.nodes.insert(
            contract.id.as_str(),
            RegisteredNode {
                contract,
                executable: None,
                compatibility_executable: None,
            },
        );
    }

    /// Returns the availability state for a contract id at tick 100.
    pub fn node_availability(&self, contract_id: &str) -> NodeAvailability {
        self.node_availability_at_tick(contract_id, 100)
    }

    /// Returns the availability state for a contract id at a specific tick.
    pub fn node_availability_at_tick(
        &self,
        contract_id: &str,
        _current_tick: u64,
    ) -> NodeAvailability {
        if let Some(registered) = self.get_registered_node(contract_id) {
            let canonical_id = registered.contract.id.as_str();
            if let Some(ref exec) = registered.executable {
                let manifest_canonical = self
                    .resolve_canonical_id(exec.manifest.semantic_contract.id.as_str())
                    .unwrap_or(exec.manifest.semantic_contract.id.as_str());

                let expected_hash =
                    OwnedNodeSchema::from_contract(registered.contract).semantic_hash();

                if manifest_canonical != canonical_id {
                    // Cross-contract mismatch
                    NodeAvailability {
                        contract_id: canonical_id.to_owned(),
                        state: AvailabilityState::ContractOnly,
                        reason_code: "CND-AVL-001".to_owned(),
                        implementation_id: None,
                        host_id: None,
                        plan_identity: None,
                        run_id: None,
                        rejection_reasons: vec!["CND-RES-008".to_owned()],
                    }
                } else if exec.manifest.semantic_contract.semantic_hash != expected_hash {
                    // Contract hash mismatch
                    NodeAvailability {
                        contract_id: canonical_id.to_owned(),
                        state: AvailabilityState::ContractOnly,
                        reason_code: "CND-AVL-001".to_owned(),
                        implementation_id: None,
                        host_id: None,
                        plan_identity: None,
                        run_id: None,
                        rejection_reasons: vec!["CND-RES-002".to_owned()],
                    }
                } else {
                    NodeAvailability {
                        contract_id: canonical_id.to_owned(),
                        state: AvailabilityState::ProviderAvailable,
                        reason_code: "CND-AVL-002".to_owned(),
                        implementation_id: Some(exec.manifest.id.to_string()),
                        host_id: None,
                        plan_identity: None,
                        run_id: None,
                        rejection_reasons: vec!["CND-RES-025".to_owned()],
                    }
                }
            } else {
                NodeAvailability {
                    contract_id: canonical_id.to_owned(),
                    state: AvailabilityState::ContractOnly,
                    reason_code: "CND-AVL-001".to_owned(),
                    implementation_id: None,
                    host_id: None,
                    plan_identity: None,
                    run_id: None,
                    rejection_reasons: vec!["CND-RES-008".to_owned()],
                }
            }
        } else {
            NodeAvailability {
                contract_id: contract_id.to_owned(),
                state: AvailabilityState::Unsupported,
                reason_code: "CND-AVL-006".to_owned(),
                implementation_id: None,
                host_id: None,
                plan_identity: None,
                run_id: None,
                rejection_reasons: vec!["CND-RES-001".to_owned()],
            }
        }
    }

    /// Explicit finite batch/demo registry.
    ///
    /// These linked callbacks are not implementation manifests, host reports,
    /// exact bindings, or production availability claims.
    #[must_use]
    pub fn compatibility_demo() -> Self {
        let mut registry = Self::default();
        let mut install = |contract: &'static NodeContract<'static>,
                           factory: HandlerFactory,
                           validate_config: ConfigValidator| {
            let registered = registry
                .nodes
                .get_mut(contract.id.as_str())
                .expect("default registry contains every compatibility contract");
            registered.compatibility_executable = Some(CompatibilityExecutable {
                factory,
                validate_config,
            });
        };
        install(&LITERAL_CONTRACT, || Box::new(Literal), validate_literal);
        install(
            &FORMAT_VALUES_LITERAL_CONTRACT,
            || Box::new(FormatValuesLiteral),
            validate_format_values_literal,
        );
        install(&FORMAT_CONTRACT, || Box::new(Format), validate_format);
        install(&STDIN_CONTRACT, || Box::new(Stdin), validate_empty_config);
        install(
            &UPPERCASE_CONTRACT,
            || Box::new(Uppercase),
            validate_empty_config,
        );
        install(
            &ENCODE_UTF8_CONTRACT,
            || Box::new(EncodeUtf8),
            validate_empty_config,
        );
        install(&STDOUT_CONTRACT, || Box::new(Stdout), validate_empty_config);
        install(&STDERR_CONTRACT, || Box::new(Stderr), validate_empty_config);
        install(
            &SUPERVISOR_CONTRACT,
            || Box::new(Supervisor),
            validate_empty_config,
        );
        install(
            &PASS_THROUGH_CONTRACT,
            || Box::new(PassThroughHandler),
            validate_empty_config,
        );
        install(&TEE_CONTRACT, || Box::new(TeeHandler), validate_tee);
        install(&MERGE_CONTRACT, || Box::new(MergeHandler), validate_merge);
        install(&ZIP_CONTRACT, || Box::new(ZipHandler), validate_zip);
        install(&GATE_CONTRACT, || Box::new(GateHandler), validate_gate);
        install(
            &SELECT_CONTRACT,
            || Box::new(SelectHandler),
            validate_select,
        );
        install(
            &FALLBACK_CONTRACT,
            || Box::new(FallbackHandler),
            validate_empty_config,
        );
        registry
    }

    /// Registry of compiled-in hosted primitive providers.
    ///
    /// Each callback is coupled to a validated implementation manifest and
    /// source-attested artifact before it can participate in resolution.
    #[must_use]
    pub fn hosted_primitives() -> Self {
        let mut registry = Self {
            allow_installed_resolution: true,
            ..Self::default()
        };
        for definition in hosted_provider_definitions() {
            registry
                .register_executable_provider(
                    definition.installed.contract,
                    definition.installed.manifest,
                    definition.artifacts,
                    definition.factory,
                    definition.validate_config,
                )
                .expect("compiled-in hosted primitive manifest is valid");
        }
        registry
    }

    /// Finite exact inventory used to compare planned bindings with installed
    /// executable providers. These facts are not derived from compile input.
    #[must_use]
    pub fn installed_hosted_providers() -> &'static [InstalledHostedProvider] {
        static INSTALLED: OnceLock<Vec<InstalledHostedProvider>> = OnceLock::new();
        INSTALLED
            .get_or_init(|| {
                hosted_provider_definitions()
                    .iter()
                    .map(|definition| definition.installed)
                    .collect()
            })
            .as_slice()
    }
}

fn hosted_provider_definitions() -> &'static [HostedProviderDefinition] {
    static DEFINITIONS: OnceLock<Vec<HostedProviderDefinition>> = OnceLock::new();
    DEFINITIONS.get_or_init(|| {
        let source_bytes = include_bytes!("lib.rs");
        let source_hash: [u8; 32] = Sha256::digest(source_bytes).into();
        let artifact_digest = ArtifactDigest::from_bytes(source_hash);
        let mut artifact = ArtifactManifest {
            schema_version: 1,
            identity: SemanticHash::from_bytes([0; 32]),
            id: Id("conduit/hosted-primitives-artifact"),
            digest: artifact_digest,
            media_type: "application/vnd.conduit.compiled-in-provider",
            byte_size: u64::try_from(source_bytes.len()).expect("source length fits u64"),
            target: Some(Id(std::env::consts::ARCH)),
            abi: Some(Id("conduit/rust-in-process-v1")),
            provenance: ArtifactProvenance {
                builder: Id("conduit/rustc-workspace-build"),
                source_digest: artifact_digest,
                build_recipe_digest: ArtifactDigest::from_bytes(
                    Sha256::digest(b"cargo build -p conduit-runtime").into(),
                ),
                reproducible: true,
            },
            signatures: &[],
            license_expressions: &["MIT", "Apache-2.0"],
            notices: &[],
            sbom: None,
            source: None,
            related_artifacts: &[],
            locations: &[],
        };
        let mut artifact_scratch =
            vec![SemanticHash::from_bytes([0; 32]); artifact.identity_fact_count()];
        artifact.identity = artifact
            .computed_semantic_hash(&mut artifact_scratch)
            .expect("compiled-in artifact identity");
        let artifact = &*Box::leak(Box::new(artifact));
        let artifacts: &'static [&'static ArtifactManifest<'static>] =
            Box::leak(Box::new([artifact]));
        let profile_hash =
            SemanticHash::from_bytes(Sha256::digest(b"conduit/hosted-primitive-profile/v1").into());

        let lines_contract = conduit_std::standard_node_contract("std/text/lines")
            .expect("lines is in the standard catalog");
        let join_contract = conduit_std::standard_node_contract("std/text/join")
            .expect("join is in the standard catalog");
        let specifications: &[(
            &'static NodeContract<'static>,
            &'static str,
            HostedPrimitiveImplementation,
            HandlerFactory,
            ConfigValidator,
        )] = &[
            (
                &LITERAL_CONTRACT,
                "literal",
                HostedPrimitiveImplementation::Literal,
                || Box::new(Literal),
                validate_literal,
            ),
            (
                &FORMAT_VALUES_LITERAL_CONTRACT,
                "format-values-literal",
                HostedPrimitiveImplementation::FormatValuesLiteral,
                || Box::new(FormatValuesLiteral),
                validate_format_values_literal,
            ),
            (
                &FORMAT_CONTRACT,
                "format",
                HostedPrimitiveImplementation::Format,
                || Box::new(Format),
                validate_format,
            ),
            (
                lines_contract,
                "text-lines",
                HostedPrimitiveImplementation::Lines,
                || Box::new(Lines),
                validate_lines,
            ),
            (
                join_contract,
                "text-join",
                HostedPrimitiveImplementation::Join,
                || Box::new(Join),
                validate_join,
            ),
            (
                &STDIN_CONTRACT,
                "stdin",
                HostedPrimitiveImplementation::Stdin,
                || Box::new(Stdin),
                validate_empty_config,
            ),
            (
                &UPPERCASE_CONTRACT,
                "uppercase",
                HostedPrimitiveImplementation::Uppercase,
                || Box::new(Uppercase),
                validate_empty_config,
            ),
            (
                &ENCODE_UTF8_CONTRACT,
                "encode-utf8",
                HostedPrimitiveImplementation::EncodeUtf8,
                || Box::new(EncodeUtf8),
                validate_empty_config,
            ),
            (
                &STDOUT_CONTRACT,
                "stdout",
                HostedPrimitiveImplementation::Stdout,
                || Box::new(Stdout),
                validate_empty_config,
            ),
            (
                &STDERR_CONTRACT,
                "stderr",
                HostedPrimitiveImplementation::Stderr,
                || Box::new(Stderr),
                validate_empty_config,
            ),
            (
                &PASS_THROUGH_CONTRACT,
                "pass-through",
                HostedPrimitiveImplementation::PassThrough,
                || Box::new(PassThroughHandler),
                validate_empty_config,
            ),
            (
                &TEE_CONTRACT,
                "tee",
                HostedPrimitiveImplementation::Tee,
                || Box::new(TeeHandler),
                validate_tee,
            ),
            (
                &MERGE_CONTRACT,
                "merge",
                HostedPrimitiveImplementation::Merge,
                || Box::new(MergeHandler),
                validate_merge,
            ),
            (
                &ZIP_CONTRACT,
                "zip",
                HostedPrimitiveImplementation::Zip,
                || Box::new(ZipHandler),
                validate_zip,
            ),
            (
                &GATE_CONTRACT,
                "gate",
                HostedPrimitiveImplementation::Gate,
                || Box::new(GateHandler),
                validate_gate,
            ),
            (
                &SELECT_CONTRACT,
                "select",
                HostedPrimitiveImplementation::Select,
                || Box::new(SelectHandler),
                validate_select,
            ),
            (
                &FALLBACK_CONTRACT,
                "fallback",
                HostedPrimitiveImplementation::Fallback,
                || Box::new(FallbackHandler),
                validate_empty_config,
            ),
        ];

        specifications
            .iter()
            .map(
                |(contract, entrypoint, implementation, factory, validate_config)| {
                    let implementation_id: &'static str =
                        Box::leak(format!("conduit/hosted-{entrypoint}-v1").into_boxed_str());
                    let artifact_references = Box::leak(Box::new([ManifestArtifactRef {
                        id: artifact.id,
                        digest: artifact.digest,
                        role: Id("executable"),
                        required: true,
                    }]));
                    let mut manifest = ImplementationManifest {
                        schema_version: 1,
                        identity: SemanticHash::from_bytes([0; 32]),
                        id: Id(implementation_id),
                        implementation_version: "1",
                        semantic_contract: PinnedDescriptor {
                            id: contract.id,
                            schema_version: 1,
                            semantic_hash: OwnedNodeSchema::from_contract(contract).semantic_hash(),
                        },
                        executor: ExecutorKind::NativeInProcess,
                        entrypoint: ManifestEntrypoint {
                            name: Id(entrypoint),
                            adapter: Id("conduit/hosted-primitive-step"),
                            abi: Id("conduit/hosted-primitive-v1"),
                            protocol_version: 1,
                        },
                        execution_profile: PinnedDescriptor {
                            id: Id("conduit/hosted-primitive-profile-v1"),
                            schema_version: 1,
                            semantic_hash: profile_hash,
                        },
                        artifacts: artifact_references,
                        required_interfaces: &[],
                        provided_interfaces: &[],
                        required_authorities: &[],
                        required_effects: &[],
                        minimum_plan_version: 1,
                        maximum_plan_version: u32::MAX,
                        minimum_runtime_protocol: 1,
                        maximum_runtime_protocol: 1,
                        replacement: ReplacementSupport::Cold,
                        coexistence_memory_bytes: 0,
                        reproducibility: None,
                    };
                    let mut manifest_scratch =
                        vec![SemanticHash::from_bytes([0; 32]); manifest.identity_fact_count()];
                    manifest.identity = manifest
                        .computed_semantic_hash(&mut manifest_scratch)
                        .expect("compiled-in implementation identity");
                    let manifest = &*Box::leak(Box::new(manifest));
                    HostedProviderDefinition {
                        installed: InstalledHostedProvider {
                            contract,
                            manifest,
                            artifact,
                            implementation: *implementation,
                        },
                        artifacts,
                        factory: *factory,
                        validate_config: *validate_config,
                    }
                },
            )
            .collect()
    })
}

impl Default for Registry {
    fn default() -> Self {
        let mut nodes = BTreeMap::new();
        // Default discovery publishes semantic contracts only. Runnable
        // compatibility callbacks are installed only by `compatibility_demo`.
        let honest_primitive = |contract: &'static NodeContract<'static>,
                                _factory: HandlerFactory,
                                _validate_config: ConfigValidator|
         -> RegisteredNode {
            RegisteredNode {
                contract,
                executable: None,
                compatibility_executable: None,
            }
        };

        // Honest runnable primitives
        nodes.insert(
            LITERAL_CONTRACT.id.as_str(),
            honest_primitive(&LITERAL_CONTRACT, || Box::new(Literal), validate_literal),
        );
        nodes.insert(
            FORMAT_VALUES_LITERAL_CONTRACT.id.as_str(),
            honest_primitive(
                &FORMAT_VALUES_LITERAL_CONTRACT,
                || Box::new(FormatValuesLiteral),
                validate_format_values_literal,
            ),
        );
        nodes.insert(
            FORMAT_CONTRACT.id.as_str(),
            honest_primitive(&FORMAT_CONTRACT, || Box::new(Format), validate_format),
        );
        nodes.insert(
            STDIN_CONTRACT.id.as_str(),
            honest_primitive(&STDIN_CONTRACT, || Box::new(Stdin), validate_empty_config),
        );
        nodes.insert(
            UPPERCASE_CONTRACT.id.as_str(),
            honest_primitive(
                &UPPERCASE_CONTRACT,
                || Box::new(Uppercase),
                validate_empty_config,
            ),
        );
        nodes.insert(
            ENCODE_UTF8_CONTRACT.id.as_str(),
            honest_primitive(
                &ENCODE_UTF8_CONTRACT,
                || Box::new(EncodeUtf8),
                validate_empty_config,
            ),
        );
        nodes.insert(
            STDOUT_CONTRACT.id.as_str(),
            honest_primitive(&STDOUT_CONTRACT, || Box::new(Stdout), validate_empty_config),
        );
        nodes.insert(
            STDERR_CONTRACT.id.as_str(),
            honest_primitive(&STDERR_CONTRACT, || Box::new(Stderr), validate_empty_config),
        );
        nodes.insert(
            SUPERVISOR_CONTRACT.id.as_str(),
            honest_primitive(
                &SUPERVISOR_CONTRACT,
                || Box::new(Supervisor),
                validate_empty_config,
            ),
        );
        nodes.insert(
            PASS_THROUGH_CONTRACT.id.as_str(),
            honest_primitive(
                &PASS_THROUGH_CONTRACT,
                || Box::new(PassThroughHandler),
                validate_empty_config,
            ),
        );
        nodes.insert(
            TEE_CONTRACT.id.as_str(),
            honest_primitive(&TEE_CONTRACT, || Box::new(TeeHandler), validate_tee),
        );
        nodes.insert(
            MERGE_CONTRACT.id.as_str(),
            honest_primitive(&MERGE_CONTRACT, || Box::new(MergeHandler), validate_merge),
        );
        nodes.insert(
            ZIP_CONTRACT.id.as_str(),
            honest_primitive(&ZIP_CONTRACT, || Box::new(ZipHandler), validate_zip),
        );
        nodes.insert(
            GATE_CONTRACT.id.as_str(),
            honest_primitive(&GATE_CONTRACT, || Box::new(GateHandler), validate_gate),
        );
        nodes.insert(
            SELECT_CONTRACT.id.as_str(),
            honest_primitive(
                &SELECT_CONTRACT,
                || Box::new(SelectHandler),
                validate_select,
            ),
        );
        nodes.insert(
            FALLBACK_CONTRACT.id.as_str(),
            honest_primitive(
                &FALLBACK_CONTRACT,
                || Box::new(FallbackHandler),
                validate_empty_config,
            ),
        );

        // Discoverable contract-only semantic nodes (no default executable provider)
        let contract_only_list: &[&'static NodeContract<'static>] = &[
            &DELAY_CONTRACT,
            &DEBOUNCE_CONTRACT,
            &THROTTLE_CONTRACT,
            &TAKE_CONTRACT,
            &SKIP_CONTRACT,
            &FILTER_CONTRACT,
            &PROBE_CONTRACT,
            &LOG_CONTRACT,
            &ASSERT_CONTRACT,
            &RECORD_CONTRACT,
            &REPLAY_CONTRACT,
            &FAULT_SOURCE_CONTRACT,
            &FILE_READ_CONTRACT,
            &FILE_WRITE_CONTRACT,
            &BLOB_STORE_CONTRACT,
            &KV_STORE_CONTRACT,
            &PROCESS_SPAWN_CONTRACT,
            &GPIO_PIN_CONTRACT,
            &SERIAL_PORT_CONTRACT,
            &CELL_CONTRACT,
            &COUNTER_CONTRACT,
            &DEDUPLICATE_CONTRACT,
            &CACHE_CONTRACT,
            &CIRCUIT_BREAKER_CONTRACT,
            &HEALTH_GATE_CONTRACT,
            &BACKOFF_CONTRACT,
            &WIFI_STATION_CONTRACT,
            &WIFI_AP_CONTRACT,
            &NETWORK_INTERFACE_CONTRACT,
            &TCP_SOCKET_CONTRACT,
            &UDP_SOCKET_CONTRACT,
            &DNS_RESOLVER_CONTRACT,
            &HTTP_SERVE_ONCE_CONTRACT,
            &DISPLAY_TEXT_CONTRACT,
        ];

        for &contract in contract_only_list {
            nodes.insert(
                contract.id.as_str(),
                RegisteredNode {
                    contract,
                    executable: None,
                    compatibility_executable: None,
                },
            );
        }
        for entry in conduit_std::STANDARD_CATALOG {
            nodes
                .entry(entry.contract.id.as_str())
                .or_insert(RegisteredNode {
                    contract: &entry.contract,
                    executable: None,
                    compatibility_executable: None,
                });
        }

        let mut types = TypeRegistry::default();
        for namespace in ["std", "supervision", "net", "fs", "process", "crypto"] {
            types
                .register(BuiltinTypeProvider(namespace))
                .expect("built-in type namespace is unique and valid");
        }

        let mut interfaces = BTreeMap::new();
        let stream_sink_member = OwnedInterfaceMember {
            requirement: conduit_core::InterfaceMemberRequirement::Required,
            id: "text".to_owned(),
            direction: Direction::Input,
            value_type: TEXT_TYPE.into(),
            presence: Presence::Required,
            connections: ConnectionCardinality::ExactlyOne,
            values: ValueCardinality::ExactlyOne,
            delivery: Delivery::FiniteBatch,
            temporal: TemporalContract::Atemporal,
            terminal: TerminalContract::Finite,
            sensitivity: Sensitivity::Public,
            loss: LossAcceptance::LosslessOnly,
        };
        let mut stream_sink = OwnedInterfaceContract {
            id: "conduit/stream-sink".to_owned(),
            schema_version: 1,
            members: vec![stream_sink_member],
            semantic_hash: SemanticHash::from_bytes([0; 32]),
        };
        stream_sink.semantic_hash = stream_sink
            .compute_semantic_hash()
            .expect("valid stream-sink interface");
        interfaces.insert(stream_sink.id.clone(), stream_sink);

        let text_processor_in = OwnedInterfaceMember {
            requirement: conduit_core::InterfaceMemberRequirement::Required,
            id: "text".to_owned(),
            direction: Direction::Input,
            value_type: TEXT_TYPE.into(),
            presence: Presence::Required,
            connections: ConnectionCardinality::ExactlyOne,
            values: ValueCardinality::ExactlyOne,
            delivery: Delivery::FiniteBatch,
            temporal: TemporalContract::Atemporal,
            terminal: TerminalContract::Finite,
            sensitivity: Sensitivity::Public,
            loss: LossAcceptance::LosslessOnly,
        };
        let text_processor_out = OwnedInterfaceMember {
            requirement: conduit_core::InterfaceMemberRequirement::Required,
            id: "text".to_owned(),
            direction: Direction::Output,
            value_type: TEXT_TYPE.into(),
            presence: Presence::Required,
            connections: ConnectionCardinality::OneOrMore,
            values: ValueCardinality::ExactlyOne,
            delivery: Delivery::FiniteBatch,
            temporal: TemporalContract::Atemporal,
            terminal: TerminalContract::Finite,
            sensitivity: Sensitivity::Public,
            loss: LossAcceptance::LosslessOnly,
        };
        let mut text_processor = OwnedInterfaceContract {
            id: "conduit/text-processor".to_owned(),
            schema_version: 1,
            members: vec![text_processor_in, text_processor_out],
            semantic_hash: SemanticHash::from_bytes([0; 32]),
        };
        text_processor.semantic_hash = text_processor
            .compute_semantic_hash()
            .expect("valid text-processor interface");
        interfaces.insert(text_processor.id.clone(), text_processor);

        Self {
            nodes,
            interfaces,
            types,
            allow_installed_resolution: false,
        }
    }
}

impl Registry {
    /// Resolves semantic source references to concrete hosted implementations.
    pub fn resolve<'a>(&'a self, panel: &'a Panel) -> Result<ResolvedPanel<'a>, ResolutionError> {
        self.resolve_inner(panel, true)
    }

    /// Resolves only the semantic topology, without claiming that any
    /// contract-only node is executable. Compilers use this after lowering;
    /// execution must always call [`Self::resolve`].
    pub fn resolve_contracts<'a>(
        &'a self,
        panel: &'a Panel,
    ) -> Result<ResolvedPanel<'a>, ResolutionError> {
        self.resolve_inner(panel, false)
    }

    fn resolve_inner<'a>(
        &'a self,
        panel: &'a Panel,
        require_executable: bool,
    ) -> Result<ResolvedPanel<'a>, ResolutionError> {
        let has_unlowered_source = !panel.imports.is_empty()
            || !panel.roots.is_empty()
            || !panel.port_groups.is_empty()
            || !panel.pools.is_empty()
            || panel.nodes.iter().any(|node| node.constraint.is_some())
            || panel.definitions.iter().any(|definition| {
                !definition.parameters.is_empty()
                    || !definition.port_groups.is_empty()
                    || !definition.pools.is_empty()
                    || definition
                        .nodes
                        .iter()
                        .any(|node| node.constraint.is_some())
            });
        if has_unlowered_source {
            return Err(ResolutionError::new(
                "CND-PLN-005",
                "imports, roots, constraints, port groups, and pools must be explicitly lowered before runtime resolution",
            ));
        }
        let expanded = expand_panel(panel, self)?;
        if expanded.nodes.len() > usize::from(u16::MAX) {
            return Err(ResolutionError::new(
                "CND-PLN-003",
                "panel has more nodes than the portable plan can address",
            ));
        }

        let mut nodes = Vec::with_capacity(expanded.nodes.len());
        for source in expanded.nodes {
            Id::new(&source.id).map_err(|error| {
                ResolutionError::new(
                    "CND-ID-001",
                    format!("invalid expanded node id `{}`: {error}", source.id),
                )
            })?;
            let definition = self.get_registered_node(&source.kind).ok_or_else(|| {
                ResolutionError::new(
                    "CND-IMP-001",
                    format!("no ready implementation for `{}`", source.kind),
                )
            })?;
            let validate_config = definition
                .executable
                .as_ref()
                .filter(|_| self.allow_installed_resolution)
                .map(|executable| executable.validate_config)
                .or_else(|| {
                    definition
                        .compatibility_executable
                        .as_ref()
                        .map(|executable| executable.validate_config)
                })
                .or_else(|| (!require_executable).then_some(validate_contract_config));
            let validate_config = validate_config.ok_or_else(|| {
                ResolutionError::new(
                    "CND-IMP-001",
                    format!("no ready implementation for `{}`", source.kind),
                )
            })?;
            validate_config(&source)?;
            nodes.push(ResolvedNode { source, definition });
        }

        let mut cords = Vec::with_capacity(expanded.cords.len());
        for source in expanded.cords {
            let from_node = node_index(&nodes, &source.from.node)?;
            let to_node = node_index(&nodes, &source.to.node)?;
            let from_port = port_index(
                nodes[from_node].definition.contract.outputs,
                &source.from.port,
                &source.from.node,
            )?;
            let to_port = port_index(
                nodes[to_node].definition.contract.inputs,
                &source.to.port,
                &source.to.node,
            )?;
            cords.push(ResolvedCord {
                source,
                from_node,
                from_port,
                to_node,
                to_port,
            });
        }

        let core_nodes = nodes
            .iter()
            .map(|node| PlanNode {
                id: Id(node.source.id.as_str()),
                contract: node.definition.contract,
            })
            .collect::<Vec<_>>();
        let core_cords = cords
            .iter()
            .map(|cord| {
                let flow = resolve_flow(&cord.source)?;
                let value_type =
                    nodes[cord.from_node].definition.contract.outputs[cord.from_port].value_type;
                let flow_decision = self.types.assess_flow_policy(value_type, flow);
                if flow_decision.outcome != CompatibilityOutcome::Compatible {
                    return Err(ResolutionError::new(
                        "CND-FLW-004",
                        flow_decision.reason.as_str(),
                    ));
                }
                Ok(PlanCord {
                    id: Id(cord.source.id.as_str()),
                    from: CoreEndpoint {
                        node: u16::try_from(cord.from_node).expect("node count checked"),
                        port: u16::try_from(cord.from_port).map_err(|_| {
                            ResolutionError::new("CND-PLN-003", "too many output ports")
                        })?,
                    },
                    to: CoreEndpoint {
                        node: u16::try_from(cord.to_node).expect("node count checked"),
                        port: u16::try_from(cord.to_port).map_err(|_| {
                            ResolutionError::new("CND-PLN-003", "too many input ports")
                        })?,
                    },
                    flow,
                })
            })
            .collect::<Result<Vec<_>, ResolutionError>>()?;
        validate_plan_graph(&PlanGraph {
            nodes: &core_nodes,
            cords: &core_cords,
        })
        .map_err(|error| ResolutionError::new(error.code.as_str(), error.to_string()))?;

        reject_cycles(&nodes, &cords)?;

        Ok(ResolvedPanel {
            source: panel,
            nodes,
            cords,
            logical_composites: expanded.logical_composites,
            supervisions: expanded.supervisions,
        })
    }

    /// Returns the semantic contracts available from this registry.
    pub fn contracts(&self) -> impl Iterator<Item = &'static NodeContract<'static>> + '_ {
        self.nodes.values().map(|node| node.contract)
    }

    /// Returns the domain type registry used during flow resolution.
    #[must_use]
    pub const fn type_registry(&self) -> &TypeRegistry {
        &self.types
    }
}

impl SourceContractCatalog for Registry {
    fn node_schema(&self, id: &str) -> Option<OwnedNodeSchema> {
        self.get_registered_node(id)
            .map(|registered| OwnedNodeSchema::from_contract(registered.contract))
    }

    fn node_contract(&self, id: &str) -> Option<OwnedNodeContract> {
        self.get_registered_node(id)
            .map(|registered| OwnedNodeContract::from_contract(registered.contract))
    }

    fn interface_contract(&self, id: &str) -> Option<OwnedInterfaceContract> {
        self.interfaces.get(id).cloned()
    }

    fn type_reference(&self, id: &str) -> Option<OwnedTypeReference> {
        conduit_std::standard_type_reference(id).map(Into::into)
    }

    fn port_contract(&self, id: &str) -> Option<OwnedPortReference> {
        let contract = match id {
            "conduit/input-text" => Some(&VALUE_TEXT_INPUT),
            "conduit/output-text" => Some(&VALUE_TEXT_OUTPUT),
            _ => self
                .nodes
                .values()
                .flat_map(|registered| {
                    registered
                        .contract
                        .inputs
                        .iter()
                        .chain(registered.contract.outputs.iter())
                })
                .find(|port| port.id.as_str() == id),
        }?;
        OwnedPortReference::from_contract(contract).ok()
    }

    fn validate_literal(
        &self,
        expected: &OwnedTypeReference,
        source: &conduit_panel::SourceValue,
    ) -> Result<OwnedSemanticValue, LiteralValidationError> {
        let reference = conduit_std::standard_type_reference(&expected.id)
            .ok_or(LiteralValidationError::ProviderUnavailable)?;
        if expected != &OwnedTypeReference::from(reference) {
            return Err(LiteralValidationError::ProviderUnavailable);
        }
        validate_standard_literal(&expected.id, source)
    }

    fn validate_default(
        &self,
        expected: &OwnedTypeReference,
        value: &OwnedSemanticValue,
    ) -> Result<(), LiteralValidationError> {
        match (expected.id.as_str(), value) {
            ("std/text", OwnedSemanticValue::Text(_))
            | ("std/bytes", OwnedSemanticValue::Bytes(_))
            | ("std/bool", OwnedSemanticValue::Boolean(_))
            | ("std/format-values", OwnedSemanticValue::List(_))
            | (
                "std/integer" | "std/natural" | "std/i8" | "std/i16" | "std/i32" | "std/i64"
                | "std/i128" | "std/u8" | "std/u16" | "std/u32" | "std/u64" | "std/u128",
                OwnedSemanticValue::Integer(_),
            ) => Ok(()),
            _ => Err(LiteralValidationError::WrongKind),
        }
    }
}

fn validate_standard_literal(
    id: &str,
    source: &conduit_panel::SourceValue,
) -> Result<OwnedSemanticValue, LiteralValidationError> {
    use conduit_panel::SourceValue;

    match (id, source) {
        ("std/bool", SourceValue::Boolean(value)) => Ok(OwnedSemanticValue::Boolean(*value)),
        ("std/integer" | "std/i128", SourceValue::Integer(value)) => {
            Ok(OwnedSemanticValue::Integer(*value))
        }
        ("std/natural" | "std/u128", SourceValue::Integer(value)) if *value >= 0 => {
            Ok(OwnedSemanticValue::Integer(*value))
        }
        ("std/i8", SourceValue::Integer(value)) if i8::try_from(*value).is_ok() => {
            Ok(OwnedSemanticValue::Integer(*value))
        }
        ("std/i16", SourceValue::Integer(value)) if i16::try_from(*value).is_ok() => {
            Ok(OwnedSemanticValue::Integer(*value))
        }
        ("std/i32", SourceValue::Integer(value)) if i32::try_from(*value).is_ok() => {
            Ok(OwnedSemanticValue::Integer(*value))
        }
        ("std/i64", SourceValue::Integer(value)) if i64::try_from(*value).is_ok() => {
            Ok(OwnedSemanticValue::Integer(*value))
        }
        ("std/u8", SourceValue::Integer(value)) if u8::try_from(*value).is_ok() => {
            Ok(OwnedSemanticValue::Integer(*value))
        }
        ("std/u16", SourceValue::Integer(value)) if u16::try_from(*value).is_ok() => {
            Ok(OwnedSemanticValue::Integer(*value))
        }
        ("std/u32", SourceValue::Integer(value)) if u32::try_from(*value).is_ok() => {
            Ok(OwnedSemanticValue::Integer(*value))
        }
        ("std/u64", SourceValue::Integer(value)) if u64::try_from(*value).is_ok() => {
            Ok(OwnedSemanticValue::Integer(*value))
        }
        ("std/text", SourceValue::Text(value)) => Ok(OwnedSemanticValue::Text(value.clone())),
        ("std/bytes", SourceValue::Bytes(value)) => Ok(OwnedSemanticValue::Bytes(value.clone())),
        ("std/format-values", SourceValue::List(values)) => {
            let formatted = values
                .iter()
                .map(source_format_value)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| LiteralValidationError::InvalidValue)?;
            conduit_std::validate_format_values(&formatted)
                .map_err(|_| LiteralValidationError::InvalidValue)?;
            source_value(source)
        }
        ("std/decimal" | "std/float", SourceValue::ExactDecimal(value)) => {
            Ok(OwnedSemanticValue::Text(value.clone()))
        }
        ("std/list/text", SourceValue::List(values)) => values
            .iter()
            .map(|value| match value {
                SourceValue::Text(value) => Ok(OwnedSemanticValue::Text(value.clone())),
                _ => Err(LiteralValidationError::WrongKind),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(OwnedSemanticValue::List),
        ("std/record", SourceValue::Record(fields)) => fields
            .iter()
            .map(|(key, value)| Ok((key.clone(), source_value(value)?)))
            .collect::<Result<Vec<_>, LiteralValidationError>>()
            .map(OwnedSemanticValue::Map),
        ("std/id" | "std/reference/any", SourceValue::Reference(value))
        | ("std/id" | "std/reference/any", SourceValue::ContractReference(value)) => {
            Id::new(value).map_err(|_| LiteralValidationError::InvalidValue)?;
            Ok(OwnedSemanticValue::Identifier(value.clone()))
        }
        (
            "std/duration"
            | "std/instant"
            | "std/timestamp"
            | "std/error"
            | "std/terminal"
            | "std/health"
            | "std/progress"
            | "supervision/decision"
            | "net/ip/address"
            | "net/socket/address"
            | "net/http/method"
            | "net/http/request"
            | "net/http/response"
            | "net/http/status"
            | "net/http/headers"
            | "fs/path"
            | "process/exit-status"
            | "crypto/digest",
            SourceValue::Text(value),
        ) => Ok(OwnedSemanticValue::Text(value.clone())),
        _ => Err(LiteralValidationError::WrongKind),
    }
}

fn source_value(
    source: &conduit_panel::SourceValue,
) -> Result<OwnedSemanticValue, LiteralValidationError> {
    use conduit_panel::SourceValue;

    match source {
        SourceValue::Boolean(value) => Ok(OwnedSemanticValue::Boolean(*value)),
        SourceValue::Integer(value) => Ok(OwnedSemanticValue::Integer(*value)),
        SourceValue::Text(value) | SourceValue::ExactDecimal(value) => {
            Ok(OwnedSemanticValue::Text(value.clone()))
        }
        SourceValue::Bytes(value) => Ok(OwnedSemanticValue::Bytes(value.clone())),
        SourceValue::Reference(value) | SourceValue::ContractReference(value) => {
            Id::new(value).map_err(|_| LiteralValidationError::InvalidValue)?;
            Ok(OwnedSemanticValue::Identifier(value.clone()))
        }
        SourceValue::List(values) => values
            .iter()
            .map(source_value)
            .collect::<Result<Vec<_>, _>>()
            .map(OwnedSemanticValue::List),
        SourceValue::Record(fields) => fields
            .iter()
            .map(|(key, value)| Ok((key.clone(), source_value(value)?)))
            .collect::<Result<Vec<_>, LiteralValidationError>>()
            .map(OwnedSemanticValue::Map),
        SourceValue::SecretReference(_) => Err(LiteralValidationError::InvalidValue),
    }
}

struct BuiltinTypeProvider(&'static str);

impl TypeContractProvider for BuiltinTypeProvider {
    fn provider_descriptor(&self) -> DescriptorRef<'static> {
        DescriptorRef {
            kind: Id("conduit/builtin-type-provider"),
            schema_version: 1,
            semantic_hash: SemanticHash::from_bytes([0x24; 32]),
        }
    }

    fn namespace(&self) -> &str {
        self.0
    }

    fn describe<'a>(
        &'a self,
        reference: TypeContractRef<'a>,
    ) -> Option<TypeContractDescription<'a>> {
        let exact = conduit_std::standard_type_reference(reference.contract_id.as_str())?;
        if reference != exact {
            return None;
        }
        let definition = conduit_std::standard_type(reference.contract_id.as_str())?;
        Some(TypeContractDescription {
            human_name: definition.human_name,
            descriptor: conduit_std::standard_type_descriptor(definition),
            strategy: TypeComparisonStrategy::Nominal,
            flow_type_facts: FlowTypeFacts {
                disposable: TraitProof::Disproven,
                coalescers: Some(&[]),
            },
        })
    }

    fn consumer_accepts_producer<'a>(
        &'a self,
        _: TypeContractRef<'a>,
        _: TypeContractRef<'a>,
    ) -> ProviderTypeDecision<'a> {
        ProviderTypeDecision {
            outcome: CompatibilityOutcome::Incompatible,
            rule: Id("conduit/no-type-rule"),
        }
    }
}

#[derive(Debug)]
struct ExpandedSource {
    nodes: Vec<Node>,
    cords: Vec<Cord>,
    logical_composites: Vec<LogicalComposite>,
    supervisions: Vec<ExpandedSupervision>,
}

#[derive(Debug)]
struct LogicalComposite {
    path: String,
    definition: String,
    children: Vec<(String, String)>,
    cords: Vec<(String, String)>,
    exports: Vec<(ExportDirection, String, Endpoint)>,
    bindings: Vec<(String, String)>,
}

#[derive(Debug)]
struct ExpandedSupervision {
    instance: String,
    source_binding_hash: String,
    subject: String,
    handler: String,
}

type BoundaryMap = BTreeMap<(u8, String), Endpoint>;

fn expand_panel(panel: &Panel, registry: &Registry) -> Result<ExpandedSource, ResolutionError> {
    validate_definition_names(panel, registry)?;
    validate_definition_shapes(panel, registry)?;
    validate_definition_cycles(panel)?;
    let mut expanded = ExpandedSource {
        nodes: Vec::new(),
        cords: Vec::new(),
        logical_composites: Vec::new(),
        supervisions: Vec::new(),
    };
    let mut roots = BTreeMap::<String, BoundaryMap>::new();
    for node in &panel.nodes {
        validate_instance_id(&node.id)?;
        if roots.contains_key(&node.id) {
            return Err(ResolutionError::new(
                "CND-ID-002",
                format!("duplicate node id `{}`", node.id),
            ));
        }
        let boundary = expand_instance(
            panel,
            registry,
            node,
            &node.id,
            &mut Vec::new(),
            &mut expanded,
        )?;
        roots.insert(node.id.clone(), boundary);
    }
    for cord in &panel.cords {
        let from = resolve_boundary_endpoint(&roots, &cord.from, ExportDirection::Output)?;
        let to = resolve_boundary_endpoint(&roots, &cord.to, ExportDirection::Input)?;
        push_expanded_cord(&mut expanded, cord, from, to);
    }
    expand_supervision_bindings(
        panel,
        registry,
        &panel.nodes,
        &panel.supervisions,
        "",
        &mut expanded,
    )?;
    Ok(expanded)
}

fn validate_definition_shapes(panel: &Panel, registry: &Registry) -> Result<(), ResolutionError> {
    for definition in &panel.definitions {
        for (index, child) in definition.nodes.iter().enumerate() {
            validate_instance_id(&child.id)?;
            if definition.nodes[..index]
                .iter()
                .any(|prior| prior.id == child.id)
            {
                return Err(ResolutionError::new(
                    "CND-ID-002",
                    format!("duplicate child `{}` in `{}`", child.id, definition.id),
                ));
            }
        }
        for cord in &definition.cords {
            for (endpoint, direction) in [
                (&cord.from, ExportDirection::Output),
                (&cord.to, ExportDirection::Input),
            ] {
                let child = definition
                    .nodes
                    .iter()
                    .find(|child| child.id == endpoint.node)
                    .ok_or_else(|| {
                        ResolutionError::new(
                            "CND-CMP-003",
                            format!(
                                "cord in `{}` targets missing child `{}`",
                                definition.id, endpoint.node
                            ),
                        )
                    })?;
                if !kind_has_port(panel, registry, &child.kind, direction, &endpoint.port) {
                    return Err(ResolutionError::new(
                        "CND-CMP-003",
                        format!(
                            "cord in `{}` targets missing or wrong-direction port `{}.{}`",
                            definition.id, endpoint.node, endpoint.port
                        ),
                    ));
                }
            }
        }
        for (index, export) in definition.exports.iter().enumerate() {
            if definition.exports[..index].iter().any(|prior| {
                prior.direction == export.direction
                    && (prior.id == export.id || prior.target == export.target)
            }) {
                return Err(ResolutionError::new(
                    "CND-CMP-002",
                    format!("duplicate export `{}` in `{}`", export.id, definition.id),
                ));
            }
            let child = definition
                .nodes
                .iter()
                .find(|child| child.id == export.target.node)
                .ok_or_else(|| {
                    ResolutionError::new(
                        "CND-CMP-003",
                        format!(
                            "export `{}` targets missing child `{}`",
                            export.id, export.target.node
                        ),
                    )
                })?;
            if !kind_has_port(
                panel,
                registry,
                &child.kind,
                export.direction,
                &export.target.port,
            ) {
                return Err(ResolutionError::new(
                    "CND-CMP-003",
                    format!(
                        "export `{}` targets missing or wrong-direction port `{}.{}`",
                        export.id, export.target.node, export.target.port
                    ),
                ));
            }
        }
        for (index, binding) in definition.bindings.iter().enumerate() {
            if definition.bindings[..index]
                .iter()
                .any(|prior| prior.parameter == binding.parameter && prior.target == binding.target)
            {
                return Err(ResolutionError::new(
                    "CND-CMP-002",
                    format!(
                        "duplicate binding `{}` to `{}.{}`",
                        binding.parameter, binding.target.node, binding.target.port
                    ),
                ));
            }
            let child = definition
                .nodes
                .iter()
                .find(|child| child.id == binding.target.node)
                .ok_or_else(|| {
                    ResolutionError::new(
                        "CND-CMP-003",
                        format!(
                            "binding `{}` targets missing child `{}`",
                            binding.parameter, binding.target.node
                        ),
                    )
                })?;
            if !kind_has_parameter(panel, registry, &child.kind, &binding.target.port) {
                return Err(ResolutionError::new(
                    "CND-CMP-003",
                    format!(
                        "binding `{}` targets missing field `{}.{}`",
                        binding.parameter, binding.target.node, binding.target.port
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn kind_has_port(
    panel: &Panel,
    registry: &Registry,
    kind: &str,
    direction: ExportDirection,
    port: &str,
) -> bool {
    if let Some(primitive) = registry.get_registered_node(kind) {
        let ports = match direction {
            ExportDirection::Input => primitive.contract.inputs,
            ExportDirection::Output => primitive.contract.outputs,
        };
        return ports.iter().any(|candidate| candidate.id.as_str() == port);
    }
    panel
        .definitions
        .iter()
        .find(|definition| definition.id == kind)
        .is_some_and(|definition| {
            definition
                .exports
                .iter()
                .any(|export| export.direction == direction && export.id == port)
        })
}

fn kind_has_parameter(panel: &Panel, registry: &Registry, kind: &str, parameter: &str) -> bool {
    if let Some(primitive) = registry.get_registered_node(kind) {
        return primitive
            .contract
            .config
            .fields
            .iter()
            .any(|field| field.key.as_str() == parameter);
    }
    panel
        .definitions
        .iter()
        .find(|definition| definition.id == kind)
        .is_some_and(|definition| {
            definition
                .bindings
                .iter()
                .any(|binding| binding.parameter == parameter)
        })
}

fn validate_definition_names(panel: &Panel, registry: &Registry) -> Result<(), ResolutionError> {
    for (index, definition) in panel.definitions.iter().enumerate() {
        Id::new(&definition.id).map_err(|error| {
            ResolutionError::new(
                "CND-CMP-001",
                format!("invalid composite id `{}`: {error}", definition.id),
            )
        })?;
        if registry
            .get_registered_node(definition.id.as_str())
            .is_some()
            || panel.definitions[..index]
                .iter()
                .any(|prior| prior.id == definition.id)
        {
            return Err(ResolutionError::new(
                "CND-CMP-001",
                format!("duplicate node definition `{}`", definition.id),
            ));
        }
    }
    for definition in &panel.definitions {
        for child in &definition.nodes {
            if registry.get_registered_node(child.kind.as_str()).is_none()
                && !panel
                    .definitions
                    .iter()
                    .any(|candidate| candidate.id == child.kind)
            {
                return Err(ResolutionError::new(
                    "CND-CMP-005",
                    format!(
                        "composite `{}` references unknown definition `{}`",
                        definition.id, child.kind
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn validate_definition_cycles(panel: &Panel) -> Result<(), ResolutionError> {
    fn visit<'a>(
        panel: &'a Panel,
        definition: &'a CompositeDefinition,
        visiting: &mut Vec<&'a str>,
        visited: &mut Vec<&'a str>,
    ) -> Result<(), ResolutionError> {
        if visiting.contains(&definition.id.as_str()) {
            let mut cycle = visiting.join(" -> ");
            cycle.push_str(" -> ");
            cycle.push_str(&definition.id);
            return Err(ResolutionError::new(
                "CND-CMP-005",
                format!("recursive composite definition: {cycle}"),
            ));
        }
        if visited.contains(&definition.id.as_str()) {
            return Ok(());
        }
        visiting.push(&definition.id);
        for child in &definition.nodes {
            if let Some(nested) = panel
                .definitions
                .iter()
                .find(|candidate| candidate.id == child.kind)
            {
                visit(panel, nested, visiting, visited)?;
            }
        }
        visiting.pop();
        visited.push(&definition.id);
        Ok(())
    }

    let mut visiting = Vec::new();
    let mut visited = Vec::new();
    for definition in &panel.definitions {
        visit(panel, definition, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn expand_instance(
    panel: &Panel,
    registry: &Registry,
    source: &Node,
    path: &str,
    stack: &mut Vec<String>,
    expanded: &mut ExpandedSource,
) -> Result<BoundaryMap, ResolutionError> {
    if let Some(primitive) = registry.get_registered_node(source.kind.as_str()) {
        let id = expanded_id(path);
        let mut boundary = BoundaryMap::new();
        for port in primitive.contract.inputs {
            boundary.insert(
                (
                    direction_key(ExportDirection::Input),
                    port.id.as_str().to_owned(),
                ),
                Endpoint {
                    node: id.clone(),
                    port: port.id.as_str().to_owned(),
                },
            );
        }
        for port in primitive.contract.outputs {
            boundary.insert(
                (
                    direction_key(ExportDirection::Output),
                    port.id.as_str().to_owned(),
                ),
                Endpoint {
                    node: id.clone(),
                    port: port.id.as_str().to_owned(),
                },
            );
        }
        let mut node = source.clone();
        node.id = id;
        expanded.nodes.push(node);
        return Ok(boundary);
    }

    let definition = panel
        .definitions
        .iter()
        .find(|definition| definition.id == source.kind)
        .ok_or_else(|| {
            ResolutionError::new(
                "CND-IMP-001",
                format!("no ready implementation or composite for `{}`", source.kind),
            )
        })?;
    if stack.contains(&definition.id) {
        return Err(ResolutionError::new(
            "CND-CMP-005",
            format!("recursive composite `{}`", definition.id),
        ));
    }
    stack.push(definition.id.clone());

    validate_instance_config(source, definition)?;
    let mut children = BTreeMap::<String, BoundaryMap>::new();
    for child in &definition.nodes {
        if children.contains_key(&child.id) {
            return Err(ResolutionError::new(
                "CND-ID-002",
                format!("duplicate child `{}` in `{}`", child.id, definition.id),
            ));
        }
        let mut bound = child.clone();
        apply_bindings(source, definition, &mut bound)?;
        let child_path = format!("{path}/{}", child.id);
        let boundary = expand_instance(panel, registry, &bound, &child_path, stack, expanded)?;
        children.insert(child.id.clone(), boundary);
    }
    for cord in &definition.cords {
        let from = resolve_boundary_endpoint(&children, &cord.from, ExportDirection::Output)?;
        let to = resolve_boundary_endpoint(&children, &cord.to, ExportDirection::Input)?;
        push_expanded_cord(expanded, cord, from, to);
    }
    expand_supervision_bindings(
        panel,
        registry,
        &definition.nodes,
        &definition.supervisions,
        path,
        expanded,
    )?;

    let mut boundary = BoundaryMap::new();
    let mut logical_exports = Vec::new();
    for export in &definition.exports {
        let key = (direction_key(export.direction), export.id.clone());
        if boundary.contains_key(&key) {
            return Err(ResolutionError::new(
                "CND-CMP-002",
                format!("duplicate export `{}` in `{}`", export.id, definition.id),
            ));
        }
        let target = resolve_boundary_endpoint(&children, &export.target, export.direction)?;
        boundary.insert(key, target.clone());
        logical_exports.push((export.direction, export.id.clone(), target));
    }
    expanded.logical_composites.push(LogicalComposite {
        path: path.to_owned(),
        definition: definition.id.clone(),
        children: definition
            .nodes
            .iter()
            .map(|child| (format!("{path}/{}", child.id), child.kind.clone()))
            .collect(),
        cords: definition
            .cords
            .iter()
            .map(|cord| {
                (
                    format!("{}.{}", cord.from.node, cord.from.port),
                    format!("{}.{}", cord.to.node, cord.to.port),
                )
            })
            .collect(),
        exports: logical_exports,
        bindings: definition
            .bindings
            .iter()
            .map(|binding| {
                (
                    binding.parameter.clone(),
                    format!("{path}/{}.{}", binding.target.node, binding.target.port),
                )
            })
            .collect(),
    });
    stack.pop();
    Ok(boundary)
}

fn validate_instance_config(
    source: &Node,
    definition: &CompositeDefinition,
) -> Result<(), ResolutionError> {
    for entry in &source.config {
        let count = definition
            .bindings
            .iter()
            .filter(|binding| binding.parameter == entry.key)
            .count();
        if count == 0 {
            return Err(ResolutionError::new(
                "CND-CMP-007",
                format!(
                    "composite `{}` has no parameter `{}`",
                    definition.id, entry.key
                ),
            ));
        }
        if source
            .config
            .iter()
            .filter(|candidate| candidate.key == entry.key)
            .count()
            != 1
        {
            return Err(ResolutionError::new(
                "CND-CFG-002",
                format!("duplicate composite parameter `{}`", entry.key),
            ));
        }
    }
    Ok(())
}

fn apply_bindings(
    source: &Node,
    definition: &CompositeDefinition,
    child: &mut Node,
) -> Result<(), ResolutionError> {
    for binding in definition
        .bindings
        .iter()
        .filter(|binding| binding.target.node == child.id)
    {
        let source_entry = source
            .config
            .iter()
            .find(|entry| entry.key == binding.parameter)
            .ok_or_else(|| {
                ResolutionError::new(
                    "CND-CMP-007",
                    format!(
                        "composite `{}` requires parameter `{}`",
                        definition.id, binding.parameter
                    ),
                )
            })?;
        if child
            .config
            .iter()
            .any(|entry| entry.key == binding.target.port)
        {
            return Err(ResolutionError::new(
                "CND-CMP-007",
                format!(
                    "binding for `{}.{}` conflicts with child configuration",
                    child.id, binding.target.port
                ),
            ));
        }
        child.config.push(ConfigEntry {
            key: binding.target.port.clone(),
            value: source_entry.value.clone(),
            source_span: source_entry.source_span,
        });
    }
    for binding in &definition.bindings {
        if !definition
            .nodes
            .iter()
            .any(|candidate| candidate.id == binding.target.node)
        {
            return Err(ResolutionError::new(
                "CND-CMP-003",
                format!(
                    "binding `{}` targets missing child `{}`",
                    binding.parameter, binding.target.node
                ),
            ));
        }
    }
    Ok(())
}

fn resolve_boundary_endpoint(
    instances: &BTreeMap<String, BoundaryMap>,
    endpoint: &Endpoint,
    direction: ExportDirection,
) -> Result<Endpoint, ResolutionError> {
    let boundary = instances.get(&endpoint.node).ok_or_else(|| {
        ResolutionError::new(
            "CND-CMP-006",
            format!(
                "endpoint `{}` bypasses an instance boundary or names no child",
                endpoint.node
            ),
        )
    })?;
    boundary
        .get(&(direction_key(direction), endpoint.port.clone()))
        .cloned()
        .ok_or_else(|| {
            ResolutionError::new(
                "CND-CMP-003",
                format!(
                    "dangling or wrong-direction port mapping `{}.{}`",
                    endpoint.node, endpoint.port
                ),
            )
        })
}

fn push_expanded_cord(expanded: &mut ExpandedSource, source: &Cord, from: Endpoint, to: Endpoint) {
    let mut cord = source.clone();
    cord.id = format!("cord-{}", expanded.cords.len());
    cord.from = from;
    cord.to = to;
    expanded.cords.push(cord);
}

fn expand_supervision_bindings(
    panel: &Panel,
    registry: &Registry,
    nodes: &[Node],
    bindings: &[conduit_panel::SupervisionBinding],
    parent_path: &str,
    expanded: &mut ExpandedSource,
) -> Result<(), ResolutionError> {
    for binding in bindings {
        let subject = nodes
            .iter()
            .find(|node| node.id == binding.subject)
            .ok_or_else(|| {
                ResolutionError::new(
                    "CND-SRC-012",
                    format!("supervision subject `{}` is unavailable", binding.subject),
                )
            })?;
        let handler = nodes
            .iter()
            .find(|node| node.id == binding.handler)
            .ok_or_else(|| {
                ResolutionError::new(
                    "CND-SRC-012",
                    format!("supervision handler `{}` is unavailable", binding.handler),
                )
            })?;
        let source_binding_hash = binding.resolved_identity.clone().ok_or_else(|| {
            ResolutionError::new(
                "CND-LWR-012",
                "supervision binding lacks its exact lowered identity",
            )
        })?;
        let logical_subject = join_instance_path(parent_path, &subject.id);
        let logical_handler = join_instance_path(parent_path, &handler.id);
        expanded.supervisions.push(ExpandedSupervision {
            instance: format!("root/supervision/{logical_subject}"),
            source_binding_hash,
            subject: exact_supervision_subject(panel, registry, subject, &logical_subject)?,
            handler: exact_supervision_subject(panel, registry, handler, &logical_handler)?,
        });
    }
    Ok(())
}

fn join_instance_path(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_owned()
    } else {
        format!("{parent}/{child}")
    }
}

fn exact_supervision_subject(
    panel: &Panel,
    registry: &Registry,
    node: &Node,
    logical_path: &str,
) -> Result<String, ResolutionError> {
    if registry.get_registered_node(node.kind.as_str()).is_some() {
        Ok(format!("root/{}", expanded_id(logical_path)))
    } else if panel
        .definitions
        .iter()
        .any(|definition| definition.id == node.kind)
    {
        Ok(format!("root/{logical_path}"))
    } else {
        Err(ResolutionError::new(
            "CND-IMP-001",
            format!("supervision subject `{logical_path}` has no resolved implementation"),
        ))
    }
}

const fn direction_key(direction: ExportDirection) -> u8 {
    match direction {
        ExportDirection::Input => 0,
        ExportDirection::Output => 1,
    }
}

fn expanded_id(path: &str) -> String {
    path.replace('/', ".")
}

fn validate_instance_id(id: &str) -> Result<(), ResolutionError> {
    if id.contains('/') || id.contains('.') || Id::new(id).is_err() {
        return Err(ResolutionError::new(
            "CND-CMP-001",
            format!("`{id}` is not a valid local instance id"),
        ));
    }
    Ok(())
}

/// A source node paired with its selected implementation.
#[derive(Debug)]
struct ResolvedNode<'a> {
    source: Node,
    definition: &'a RegisteredNode,
}

/// A source cord with resolved numeric endpoints.
#[derive(Debug)]
struct ResolvedCord {
    source: conduit_panel::Cord,
    from_node: usize,
    from_port: usize,
    to_node: usize,
    to_port: usize,
}

/// A validated, implementation-resolved hosted panel.
#[derive(Debug)]
pub struct ResolvedPanel<'a> {
    source: &'a Panel,
    nodes: Vec<ResolvedNode<'a>>,
    cords: Vec<ResolvedCord>,
    logical_composites: Vec<LogicalComposite>,
    supervisions: Vec<ExpandedSupervision>,
}

/// Presentation-neutral structured view of one validated hosted resolution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedPanelView {
    pub panel_version: u16,
    pub root_nodes: usize,
    pub root_cords: usize,
    pub composites: Vec<ResolvedCompositeView>,
    pub nodes: Vec<ResolvedNodeView>,
    pub cords: Vec<ResolvedCordView>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedCompositeView {
    pub path: String,
    pub definition: String,
    pub children: Vec<ResolvedChildView>,
    pub cords: Vec<ResolvedLogicalCordView>,
    pub exports: Vec<ResolvedExportView>,
    pub bindings: Vec<ResolvedBindingView>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedChildView {
    pub path: String,
    pub contract_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedLogicalCordView {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedExportView {
    pub direction: &'static str,
    pub id: String,
    pub target_node: String,
    pub target_port: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedBindingView {
    pub parameter: String,
    pub target: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedNodeView {
    pub index: usize,
    pub id: String,
    pub contract_id: String,
    pub inputs: Vec<ResolvedPortView>,
    pub outputs: Vec<ResolvedPortView>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedPortView {
    pub id: String,
    pub type_id: String,
    pub delivery: &'static str,
    pub connections: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedCordView {
    pub index: usize,
    pub id: String,
    pub from_node: String,
    pub from_port: String,
    pub to_node: String,
    pub to_port: String,
    pub capacity_items: u16,
    pub max_value_bytes: u32,
    pub max_queued_bytes: u64,
    pub low_watermark_items: u16,
    pub high_watermark_items: u16,
    pub pressure: String,
}

/// Exact source-derived topology facts consumed by hosted plan compilation.
///
/// This is not another plan type: it contains no implementation, artifact,
/// host, authority, resource, or resolver selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactTopologyView {
    pub source_semantic_hash: SemanticHash,
    pub nodes: Vec<ExactTopologyNode>,
    pub cords: Vec<ExactTopologyCord>,
    pub composites: Vec<ExactTopologyComposite>,
    pub supervisions: Vec<ExactTopologySupervision>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactTopologyNode {
    pub instance: String,
    pub contract_id: String,
    pub contract_hash: SemanticHash,
    pub inputs: Vec<ExactTopologyPort>,
    pub outputs: Vec<ExactTopologyPort>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactTopologyPort {
    pub id: String,
    pub direction: Direction,
    pub contract_hash: SemanticHash,
    pub value_type: TypeContractRef<'static>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactTopologyCord {
    pub id: String,
    pub from_node: String,
    pub from_port: ExactTopologyPort,
    pub to_node: String,
    pub to_port: ExactTopologyPort,
    pub capacity_items: u16,
    pub max_value_bytes: u32,
    pub max_queued_bytes: u64,
    pub low_watermark_items: u16,
    pub high_watermark_items: u16,
    pub pressure: SourcePressure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactTopologyComposite {
    pub instance: String,
    pub definition_hash: SemanticHash,
    pub members: Vec<String>,
    pub exports: Vec<ExactTopologyExport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactTopologyExport {
    pub boundary_port: String,
    pub member: String,
    pub member_port: String,
    pub direction: Direction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactTopologySupervision {
    pub instance: String,
    pub source_binding_hash: SemanticHash,
    pub subject: String,
    pub handler: String,
}

impl ResolvedPanel<'_> {
    /// Returns only semantic/source topology needed before exact host binding.
    pub fn exact_topology(&self) -> Result<ExactTopologyView, ResolutionError> {
        let source_hash = if self.source.version >= 2 {
            conduit_panel::semantic_source_hash_v3(self.source)
        } else {
            conduit_panel::semantic_source_hash_v2(self.source)
        };
        let source_semantic_hash = semantic_hash_text(&source_hash).ok_or_else(|| {
            ResolutionError::new("CND-CMP-002", "semantic source hash is malformed")
        })?;
        let nodes = self
            .nodes
            .iter()
            .map(|node| {
                let contract_hash =
                    OwnedNodeSchema::from_contract(node.definition.contract).semantic_hash();
                let inputs = node
                    .definition
                    .contract
                    .inputs
                    .iter()
                    .map(exact_topology_port)
                    .collect::<Result<Vec<_>, _>>()?;
                let outputs = node
                    .definition
                    .contract
                    .outputs
                    .iter()
                    .map(exact_topology_port)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ExactTopologyNode {
                    instance: format!("root/{}", node.source.id),
                    contract_id: node.definition.contract.id.as_str().to_owned(),
                    contract_hash,
                    inputs,
                    outputs,
                })
            })
            .collect::<Result<Vec<_>, ResolutionError>>()?;
        let cords = self
            .cords
            .iter()
            .map(|cord| {
                let from = &self.nodes[cord.from_node];
                let to = &self.nodes[cord.to_node];
                Ok(ExactTopologyCord {
                    id: cord.source.id.clone(),
                    from_node: format!("root/{}", from.source.id),
                    from_port: exact_topology_port(
                        &from.definition.contract.outputs[cord.from_port],
                    )?,
                    to_node: format!("root/{}", to.source.id),
                    to_port: exact_topology_port(&to.definition.contract.inputs[cord.to_port])?,
                    capacity_items: cord.source.capacity_items,
                    max_value_bytes: cord.source.max_value_bytes,
                    max_queued_bytes: cord.source.max_queued_bytes,
                    low_watermark_items: cord.source.low_watermark_items,
                    high_watermark_items: cord.source.high_watermark_items,
                    pressure: cord.source.pressure.clone(),
                })
            })
            .collect::<Result<Vec<_>, ResolutionError>>()?;
        let composites = self
            .logical_composites
            .iter()
            .map(|composite| {
                let expanded_prefix = expanded_id(&composite.path);
                let member_prefix = format!("{expanded_prefix}.");
                let mut members = self
                    .nodes
                    .iter()
                    .filter(|node| {
                        node.source.id == expanded_prefix
                            || node.source.id.starts_with(&member_prefix)
                    })
                    .map(|node| format!("root/{}", node.source.id))
                    .collect::<Vec<_>>();
                members.sort();
                members.dedup();
                let exports = composite
                    .exports
                    .iter()
                    .map(|(direction, boundary_port, target)| ExactTopologyExport {
                        boundary_port: boundary_port.clone(),
                        member: format!("root/{}", target.node),
                        member_port: target.port.clone(),
                        direction: match direction {
                            ExportDirection::Input => Direction::Input,
                            ExportDirection::Output => Direction::Output,
                        },
                    })
                    .collect();
                Ok(ExactTopologyComposite {
                    instance: format!("root/{}", composite.path),
                    definition_hash: composite_definition_hash(
                        source_semantic_hash,
                        &composite.definition,
                    )?,
                    members,
                    exports,
                })
            })
            .collect::<Result<Vec<_>, ResolutionError>>()?;
        let mut supervisions = self
            .supervisions
            .iter()
            .map(|supervision| {
                Ok(ExactTopologySupervision {
                    instance: supervision.instance.clone(),
                    source_binding_hash: semantic_hash_text(&supervision.source_binding_hash)
                        .ok_or_else(|| {
                            ResolutionError::new(
                                "CND-LWR-012",
                                "supervision source-binding hash is malformed",
                            )
                        })?,
                    subject: supervision.subject.clone(),
                    handler: supervision.handler.clone(),
                })
            })
            .collect::<Result<Vec<_>, ResolutionError>>()?;
        supervisions.sort_by(|left, right| left.instance.cmp(&right.instance));
        Ok(ExactTopologyView {
            source_semantic_hash,
            nodes,
            cords,
            composites,
            supervisions,
        })
    }

    /// Returns structured resolution facts without choosing a CLI encoding.
    #[must_use]
    pub fn view(&self) -> ResolvedPanelView {
        let mut composites = self
            .logical_composites
            .iter()
            .map(|composite| ResolvedCompositeView {
                path: composite.path.clone(),
                definition: composite.definition.clone(),
                children: composite
                    .children
                    .iter()
                    .map(|(path, contract_id)| ResolvedChildView {
                        path: path.clone(),
                        contract_id: contract_id.clone(),
                    })
                    .collect(),
                cords: composite
                    .cords
                    .iter()
                    .map(|(from, to)| ResolvedLogicalCordView {
                        from: from.clone(),
                        to: to.clone(),
                    })
                    .collect(),
                exports: composite
                    .exports
                    .iter()
                    .map(|(direction, id, target)| ResolvedExportView {
                        direction: match direction {
                            ExportDirection::Input => "input",
                            ExportDirection::Output => "output",
                        },
                        id: id.clone(),
                        target_node: target.node.clone(),
                        target_port: target.port.clone(),
                    })
                    .collect(),
                bindings: composite
                    .bindings
                    .iter()
                    .map(|(parameter, target)| ResolvedBindingView {
                        parameter: parameter.clone(),
                        target: target.clone(),
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        composites.sort_by(|left, right| left.path.cmp(&right.path));
        let nodes = self
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| ResolvedNodeView {
                index,
                id: node.source.id.clone(),
                contract_id: node.definition.contract.id.as_str().to_owned(),
                inputs: node
                    .definition
                    .contract
                    .inputs
                    .iter()
                    .map(resolved_port_view)
                    .collect(),
                outputs: node
                    .definition
                    .contract
                    .outputs
                    .iter()
                    .map(resolved_port_view)
                    .collect(),
            })
            .collect();
        let cords = self
            .cords
            .iter()
            .enumerate()
            .map(|(index, cord)| ResolvedCordView {
                index,
                id: cord.source.id.clone(),
                from_node: self.nodes[cord.from_node].source.id.clone(),
                from_port: self.nodes[cord.from_node].definition.contract.outputs[cord.from_port]
                    .id
                    .as_str()
                    .to_owned(),
                to_node: self.nodes[cord.to_node].source.id.clone(),
                to_port: self.nodes[cord.to_node].definition.contract.inputs[cord.to_port]
                    .id
                    .as_str()
                    .to_owned(),
                capacity_items: cord.source.capacity_items,
                max_value_bytes: cord.source.max_value_bytes,
                max_queued_bytes: cord.source.max_queued_bytes,
                low_watermark_items: cord.source.low_watermark_items,
                high_watermark_items: cord.source.high_watermark_items,
                pressure: cord.source.pressure.to_string(),
            })
            .collect();
        ResolvedPanelView {
            panel_version: self.source.version,
            root_nodes: self.source.nodes.len(),
            root_cords: self.source.cords.len(),
            composites,
            nodes,
            cords,
        }
    }

    /// Produces deterministic logical and expanded resolution output.
    #[must_use]
    pub fn explain(&self) -> String {
        format!("{}\n{}", self.explain_logical(), self.explain_expanded())
    }

    /// Shows authored instances and composite boundary provenance.
    #[must_use]
    pub fn explain_logical(&self) -> String {
        use std::fmt::Write as _;

        let mut explanation = String::new();
        writeln!(
            explanation,
            "logical panel v{}: {} root nodes, {} root cords",
            self.source.version,
            self.source.nodes.len(),
            self.source.cords.len()
        )
        .expect("writing to String cannot fail");
        for node in &self.source.nodes {
            writeln!(explanation, "  instance {} : {}", node.id, node.kind)
                .expect("writing to String cannot fail");
        }
        let mut composites = self.logical_composites.iter().collect::<Vec<_>>();
        composites.sort_by(|left, right| left.path.cmp(&right.path));
        for composite in composites {
            writeln!(
                explanation,
                "  composite {} : {}",
                composite.path, composite.definition
            )
            .expect("writing to String cannot fail");
            for (child_path, definition) in &composite.children {
                writeln!(explanation, "    child {child_path} : {definition}")
                    .expect("writing to String cannot fail");
            }
            for (from, to) in &composite.cords {
                writeln!(explanation, "    cord {from} -> {to}")
                    .expect("writing to String cannot fail");
            }
            for (direction, id, target) in &composite.exports {
                let direction = match direction {
                    ExportDirection::Input => "input",
                    ExportDirection::Output => "output",
                };
                writeln!(
                    explanation,
                    "    export {direction} {id} -> {}.{}",
                    target.node, target.port
                )
                .expect("writing to String cannot fail");
            }
            for (parameter, target) in &composite.bindings {
                writeln!(explanation, "    bind {parameter} -> {target}")
                    .expect("writing to String cannot fail");
            }
        }
        explanation
    }

    /// Shows the exact flattened primitive execution topology.
    #[must_use]
    pub fn explain_expanded(&self) -> String {
        use std::fmt::Write as _;

        let mut explanation = String::new();
        writeln!(
            explanation,
            "expanded plan: {} nodes, {} cords",
            self.nodes.len(),
            self.cords.len()
        )
        .expect("writing to String cannot fail");
        for (index, node) in self.nodes.iter().enumerate() {
            writeln!(
                explanation,
                "  node {index}: {} : {} -> hosted builtin",
                node.source.id, node.definition.contract.id
            )
            .expect("writing to String cannot fail");
            for port in node.definition.contract.inputs {
                writeln!(
                    explanation,
                    "    input  {} : {} {:?} {:?}",
                    port.id, port.value_type.contract_id, port.delivery, port.connections
                )
                .expect("writing to String cannot fail");
            }
            for port in node.definition.contract.outputs {
                writeln!(
                    explanation,
                    "    output {} : {} {:?} {:?}",
                    port.id, port.value_type.contract_id, port.delivery, port.connections
                )
                .expect("writing to String cannot fail");
            }
        }
        for (index, cord) in self.cords.iter().enumerate() {
            writeln!(
                explanation,
                "  cord {index}: {}.{} -> {}.{} capacity={} max_value_bytes={} max_queued_bytes={} watermarks={}..{} pressure={}",
                self.nodes[cord.from_node].source.id,
                self.nodes[cord.from_node].definition.contract.outputs[cord.from_port].id,
                self.nodes[cord.to_node].source.id,
                self.nodes[cord.to_node].definition.contract.inputs[cord.to_port].id,
                cord.source.capacity_items,
                cord.source.max_value_bytes,
                cord.source.max_queued_bytes,
                cord.source.low_watermark_items,
                cord.source.high_watermark_items,
                cord.source.pressure
            )
            .expect("writing to String cannot fail");
        }
        explanation
    }

    /// Executes using the production DeterministicExecutor.
    pub fn run_exact<'p, 'r, 'i>(
        &self,
        plan: &'p ExecutionPlan<'p>,
        bindings: &ExactHostedBindings,
        context: ExactRunContext<'p>,
        io: &'r mut RunIo<'i>,
    ) -> Result<ExecutionSummary, RuntimeError> {
        self.run_exact_report(plan, bindings, context, io)
            .map(|report| report.summary)
    }

    /// Executes exactly and returns the executor's bounded allocation,
    /// high-water, and event observations.
    pub fn run_exact_report<'p, 'r, 'i>(
        &self,
        plan: &'p ExecutionPlan<'p>,
        bindings: &ExactHostedBindings,
        context: ExactRunContext<'p>,
        io: &'r mut RunIo<'i>,
    ) -> Result<ExactExecutionReport, RuntimeError> {
        self.run_exact_report_controlled(plan, bindings, context, None, io)
    }

    /// Starts the exact executor and immediately applies one plan-visible
    /// cancellation policy, returning its deterministic terminal evidence.
    pub fn cancel_exact_report<'p, 'r, 'i>(
        &self,
        plan: &'p ExecutionPlan<'p>,
        bindings: &ExactHostedBindings,
        context: ExactRunContext<'p>,
        stop: conduit_core::StopPolicy,
        io: &'r mut RunIo<'i>,
    ) -> Result<ExactExecutionReport, RuntimeError> {
        self.run_exact_report_controlled(plan, bindings, context, Some(stop), io)
    }

    fn run_exact_report_controlled<'p, 'r, 'i>(
        &self,
        plan: &'p ExecutionPlan<'p>,
        bindings: &ExactHostedBindings,
        context: ExactRunContext<'p>,
        initial_stop: Option<conduit_core::StopPolicy>,
        io: &'r mut RunIo<'i>,
    ) -> Result<ExactExecutionReport, RuntimeError> {
        validate_hosted_execution_plan(plan, context.validation)
            .map_err(|error| RuntimeError::new(error.code.as_str(), error.to_string()))?;
        let topology = self
            .exact_topology()
            .map_err(|error| RuntimeError::new(error.code, error.message))?;
        if context.semantic_source_hash != plan.source_semantic_hash
            || topology.nodes.len() != plan.nodes.len()
            || topology.cords.len() != plan.cords.len()
        {
            return Err(RuntimeError::new(
                "CND-RUN-009",
                "source semantic topology does not match the exact plan",
            ));
        }
        for planned in plan.nodes {
            let source = topology
                .nodes
                .iter()
                .find(|source| source.instance == planned.instance.as_str())
                .ok_or_else(|| {
                    RuntimeError::new(
                        "CND-RUN-009",
                        format!(
                            "planned node `{}` is absent from source topology",
                            planned.instance.as_str()
                        ),
                    )
                })?;
            if source.instance != planned.instance.as_str()
                || source.contract_id != planned.contract.id.as_str()
                || source.contract_hash != planned.contract.semantic_hash
            {
                return Err(RuntimeError::new(
                    "CND-RUN-009",
                    format!(
                        "source node `{}` ({}, {}) does not match exact plan node `{}` ({}, {})",
                        source.instance,
                        source.contract_id,
                        source.contract_hash,
                        planned.instance.as_str(),
                        planned.contract.id,
                        planned.contract.semantic_hash
                    ),
                ));
            }
        }
        for planned in plan.cords {
            let source = topology
                .cords
                .iter()
                .find(|source| source.id == planned.id.as_str())
                .ok_or_else(|| {
                    RuntimeError::new(
                        "CND-RUN-009",
                        format!(
                            "planned cord `{}` is absent from source topology",
                            planned.id
                        ),
                    )
                })?;
            let expected_flow = self
                .cords
                .iter()
                .find(|cord| cord.source.id == source.id)
                .ok_or_else(|| {
                    RuntimeError::new("CND-RUN-009", "planned cord is absent from source topology")
                })
                .and_then(|cord| {
                    resolve_flow(&cord.source)
                        .map_err(|error| RuntimeError::new(error.code, error.message))
                })?;
            if source.id != planned.id.as_str()
                || source.from_node != planned.from.node.as_str()
                || source.from_port.id != planned.from.port.as_str()
                || source.to_node != planned.to.node.as_str()
                || source.to_port.id != planned.to.port.as_str()
                || source.from_port.contract_hash != planned.from.port_contract_hash
                || source.to_port.contract_hash != planned.to.port_contract_hash
                || source.from_port.value_type != planned.from.value_type
                || source.to_port.value_type != planned.to.value_type
                || expected_flow != planned.flow
                || source.max_queued_bytes != planned.queue_memory_bytes
            {
                return Err(RuntimeError::new(
                    "CND-RUN-009",
                    format!("source cord `{}` does not match the exact plan", source.id),
                ));
            }
        }

        let maximum_value_store_bytes = plan.cords.iter().try_fold(0_u64, |total, cord| {
            total.checked_add(cord.queue_memory_bytes)
        });
        let maximum_value_store_bytes = maximum_value_store_bytes.ok_or_else(|| {
            RuntimeError::new("CND-RUN-009", "planned value-store bound overflowed")
        })?;
        let store = Rc::new(RefCell::new(HostValueStore::with_limit(
            maximum_value_store_bytes,
        )));
        let io_cell = Rc::new(RefCell::new(io));
        let host_failure = Rc::new(RefCell::new(None));
        let mut scheduled_nodes = Vec::with_capacity(plan.nodes.len());
        for (node_index, planned) in plan.nodes.iter().enumerate() {
            let implementation = bindings.resolve(planned, plan.artifacts)?;
            let expected_contract = match implementation {
                HostedPrimitiveImplementation::Literal => "std/literal",
                HostedPrimitiveImplementation::FormatValuesLiteral => "std/format-values/literal",
                HostedPrimitiveImplementation::Format => "std/text/format",
                HostedPrimitiveImplementation::Lines => "std/text/lines",
                HostedPrimitiveImplementation::Join => "std/text/join",
                HostedPrimitiveImplementation::Stdin => "io/stdin",
                HostedPrimitiveImplementation::Uppercase => "text/uppercase",
                HostedPrimitiveImplementation::EncodeUtf8 => "text/encode-utf8",
                HostedPrimitiveImplementation::Stdout => "io/stdout",
                HostedPrimitiveImplementation::Stderr => "io/stderr",
                HostedPrimitiveImplementation::PassThrough => "flow/identity",
                HostedPrimitiveImplementation::Tee => "conduit.std/tee",
                HostedPrimitiveImplementation::Merge => "conduit.std/merge",
                HostedPrimitiveImplementation::Zip => "conduit.std/zip",
                HostedPrimitiveImplementation::Gate => "conduit.std/gate",
                HostedPrimitiveImplementation::Select => "conduit.std/select",
                HostedPrimitiveImplementation::Fallback => "flow/fallback",
                HostedPrimitiveImplementation::HostedService => planned.contract.id.as_str(),
            };
            if planned.contract.id.as_str() != expected_contract {
                return Err(RuntimeError::new(
                    "CND-RUN-007",
                    format!(
                        "implementation `{}` cannot satisfy semantic contract `{}`",
                        planned.implementation.id, planned.contract.id
                    ),
                ));
            }
            let resolved = self
                .nodes
                .iter()
                .find(|node| {
                    planned
                        .instance
                        .as_str()
                        .strip_prefix("root/")
                        .is_some_and(|instance| node.source.id == instance)
                })
                .ok_or_else(|| {
                    RuntimeError::new(
                        "CND-RUN-009",
                        format!(
                            "planned node `{}` is absent from source",
                            planned.instance.as_str()
                        ),
                    )
                })?;
            let in_cords = plan
                .cords
                .iter()
                .enumerate()
                .filter(|(_, cord)| cord.to.node == planned.instance)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let out_cords = plan
                .cords
                .iter()
                .enumerate()
                .filter(|(_, cord)| cord.from.node == planned.instance)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let kind = match implementation {
                HostedPrimitiveImplementation::Literal => {
                    let value = resolved
                        .source
                        .config("value")
                        .map_or_else(Vec::new, |value| value.as_bytes().to_vec());
                    HostedNodeKind::Literal {
                        value,
                        emitted: false,
                    }
                }
                HostedPrimitiveImplementation::FormatValuesLiteral => {
                    let values =
                        source_format_values(&resolved.source).map_err(format_runtime_error)?;
                    let bytes = encode_format_values(&values).map_err(format_runtime_error)?;
                    HostedNodeKind::Literal {
                        value: bytes,
                        emitted: false,
                    }
                }
                HostedPrimitiveImplementation::Format => {
                    let input_cord = |port: &str| {
                        plan.cords
                            .iter()
                            .position(|cord| {
                                cord.to.node == planned.instance && cord.to.port.as_str() == port
                            })
                            .ok_or_else(|| {
                                RuntimeError::new(
                                    "CND-RUN-009",
                                    format!("format input `{port}` is absent from the exact plan"),
                                )
                            })
                    };
                    HostedNodeKind::Format {
                        template_cord: input_cord("template")?,
                        values_cord: input_cord("values")?,
                        template: None,
                        values: None,
                        output: None,
                        emitted: false,
                    }
                }
                HostedPrimitiveImplementation::Lines => HostedNodeKind::Lines {
                    state: conduit_std::LinesState::new(),
                    input: None,
                    cursor: 0,
                    retained_bytes: 0,
                    output: Vec::with_capacity(conduit_std::LINES_MAX_LINE_BYTES),
                    output_cursor: 0,
                    pending_output: None,
                    terminal_seen: false,
                    maximum_line_bytes: source_usize(&resolved.source, "maximum_line_bytes")?,
                    maximum_retained_prefix_bytes: source_usize(
                        &resolved.source,
                        "maximum_retained_prefix_bytes",
                    )?,
                },
                HostedPrimitiveImplementation::Join => HostedNodeKind::Join {
                    inputs: Vec::with_capacity(source_usize(&resolved.source, "maximum_items")?),
                    separator: resolved
                        .source
                        .config("separator")
                        .unwrap_or_default()
                        .as_bytes()
                        .to_vec(),
                    output: Vec::with_capacity(conduit_std::JOIN_MAX_OUTPUT_BYTES),
                    copy_item: 0,
                    copy_byte: 0,
                    separator_cursor: 0,
                    utf8: conduit_std::Utf8State::new(),
                    pending_output: None,
                    terminal_seen: false,
                    emitted: false,
                    maximum_items: source_usize(&resolved.source, "maximum_items")?,
                    maximum_item_bytes: source_usize(&resolved.source, "maximum_item_bytes")?,
                    maximum_output_bytes: source_usize(&resolved.source, "maximum_output_bytes")?,
                },
                HostedPrimitiveImplementation::Stdin => HostedNodeKind::Stdin { emitted: false },
                HostedPrimitiveImplementation::Uppercase => HostedNodeKind::Uppercase,
                HostedPrimitiveImplementation::EncodeUtf8 => HostedNodeKind::PassThrough,
                HostedPrimitiveImplementation::Stdout => HostedNodeKind::Stdout,
                HostedPrimitiveImplementation::Stderr => HostedNodeKind::Stderr,
                HostedPrimitiveImplementation::PassThrough => HostedNodeKind::PassThrough,
                HostedPrimitiveImplementation::Tee => HostedNodeKind::Tee {
                    isolated: resolved.source.config("mode") == Some("isolated"),
                    retained: None,
                    delivered: [false; 2],
                },
                HostedPrimitiveImplementation::Merge => HostedNodeKind::Merge {
                    inputs: [
                        planned_input_cord(plan, planned.instance, "left")?,
                        planned_input_cord(plan, planned.instance, "right")?,
                    ],
                    cursor: 0,
                },
                HostedPrimitiveImplementation::Zip => HostedNodeKind::Zip {
                    inputs: [
                        planned_input_cord(plan, planned.instance, "left")?,
                        planned_input_cord(plan, planned.instance, "right")?,
                    ],
                    left: None,
                    right: None,
                    drop_unpaired: resolved.source.config("unpaired") == Some("drop"),
                },
                HostedPrimitiveImplementation::Gate => HostedNodeKind::Gate {
                    input: planned_input_cord(plan, planned.instance, "value")?,
                    control: planned_input_cord(plan, planned.instance, "command")?,
                    open: resolved.source.config("initial") == Some("open"),
                },
                HostedPrimitiveImplementation::Select => HostedNodeKind::Select {
                    inputs: [
                        planned_input_cord(plan, planned.instance, "left")?,
                        planned_input_cord(plan, planned.instance, "right")?,
                    ],
                    control: planned_input_cord(plan, planned.instance, "command")?,
                    selected: usize::from(resolved.source.config("initial") == Some("right")),
                },
                HostedPrimitiveImplementation::Fallback => {
                    HostedNodeKind::Fallback { emitted: false }
                }
                HostedPrimitiveImplementation::HostedService => {
                    if !in_cords.is_empty() || !out_cords.is_empty() {
                        return Err(RuntimeError::new(
                            "CND-RUN-007",
                            "host-service bindings cannot hide value cords",
                        ));
                    }
                    HostedNodeKind::HostedService {
                        handler: (resolved.definition.factory())(),
                        node: resolved.source.clone(),
                        completed: false,
                    }
                }
            };
            let maximum_input_bytes = out_cords
                .iter()
                .map(|index| plan.cords[*index].flow.capacity.max_value_bytes())
                .min()
                .unwrap_or(0);
            let profile = planned.execution_profile.ok_or_else(|| {
                RuntimeError::new(
                    "CND-RUN-009",
                    format!(
                        "planned node `{}` has no execution profile",
                        planned.instance.as_str()
                    ),
                )
            })?;
            let grants = plan
                .authorities
                .iter()
                .filter(|authority| authority.node == planned.instance)
                .map(|authority| authority.grant.id)
                .collect::<Vec<_>>();
            let caller_memory_bytes = profile
                .memory_claims
                .iter()
                .filter(|claim| claim.accounting == MemoryAccounting::ExecutorAllocated)
                .try_fold(0_u64, |total, claim| total.checked_add(claim.bytes))
                .ok_or_else(|| {
                    RuntimeError::new(
                        "CND-RUN-009",
                        "implementation caller-memory claim overflowed",
                    )
                })?;
            let machine = ImplementationMachine::instantiate(
                profile,
                conduit_core::InstantiationContext {
                    instance: planned.instance,
                    implementation: planned.implementation,
                    artifact: planned.artifact,
                    execution_profile_hash: profile.semantic_hash,
                    configuration_validated: true,
                    caller_memory_bytes,
                    required_resource_bindings: planned.required_resources,
                    provided_resource_bindings: planned.required_resources,
                    required_grants: &grants,
                    provided_grants: &grants,
                    cancellation_scope: Id("run"),
                },
            )
            .map_err(|error| {
                RuntimeError::new(
                    error.code(),
                    format!(
                        "failed to instantiate `{}`: {error}",
                        planned.instance.as_str()
                    ),
                )
            })?;
            scheduled_nodes.push(ScheduledNode {
                driver: HostedSchedulerDriver {
                    kind,
                    store: Rc::clone(&store),
                    io: Rc::clone(&io_cell),
                    in_cords,
                    out_cords,
                    maximum_input_bytes,
                    host_failure: Rc::clone(&host_failure),
                },
                machine,
            });
            debug_assert_eq!(scheduled_nodes.len(), node_index + 1);
        }
        let mut executor = DeterministicExecutor::start(
            plan,
            context.validation,
            context.scheduler_policy,
            context.reservation,
            scheduled_nodes,
        )
        .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
        if let Some(stop) = initial_stop {
            executor
                .cancel(stop)
                .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
        }
        let status = match executor.run_until_stalled() {
            Ok(status) => status,
            Err(error) => {
                return Err(host_failure
                    .borrow_mut()
                    .take()
                    .unwrap_or_else(|| RuntimeError::new(error.code(), error.to_string())));
            }
        };
        let allocation = executor.allocation();
        let high_water = executor.high_water();
        let scheduler_events: Vec<SchedulerEvent> = executor.events().copied().collect();
        let evidence = exact_evidence::project_exact_evidence(
            plan,
            context.plan_epoch,
            context.run_id.as_str(),
            &scheduler_events,
        );
        let evidence_bytes = evidence
            .iter()
            .map(|record| {
                serde_json::to_vec(record).map_or(u64::MAX, |bytes| {
                    u64::try_from(bytes.len()).unwrap_or(u64::MAX)
                })
            })
            .fold(0_u64, u64::saturating_add);
        if evidence_bytes > plan.budget.evidence_bytes {
            return Err(RuntimeError::new(
                "CND-RUN-011",
                "exact execution evidence exceeded the plan-visible byte budget",
            ));
        }
        match status {
            SchedulerStatus::Succeeded => Ok(ExactExecutionReport {
                summary: ExecutionSummary {
                    nodes_completed: plan.nodes.len(),
                    cords_conducted: plan.cords.len(),
                },
                terminal: TerminalClass::Succeeded,
                allocation,
                high_water,
                scheduler_events,
                evidence,
                evidence_bytes,
            }),
            SchedulerStatus::Failed(_) => Err(host_failure
                .borrow_mut()
                .take()
                .unwrap_or_else(|| RuntimeError::new("CND-RUN-005", "exact executor run failed"))),
            SchedulerStatus::Cancelled if initial_stop.is_some() => Ok(ExactExecutionReport {
                summary: ExecutionSummary {
                    nodes_completed: 0,
                    cords_conducted: 0,
                },
                terminal: TerminalClass::Cancelled,
                allocation,
                high_water,
                scheduler_events,
                evidence,
                evidence_bytes,
            }),
            SchedulerStatus::Cancelled => Err(RuntimeError::new(
                "CND-RUN-006",
                "exact executor run cancelled",
            )),
            SchedulerStatus::Running | SchedulerStatus::Stalled | SchedulerStatus::Disconnected => {
                Err(RuntimeError::new(
                    "CND-RUN-010",
                    "exact executor stopped without a terminal success record",
                ))
            }
        }
    }

    /// Executes the finite acyclic proof batch compatibility runtime.
    pub fn run_batch(&self, io: &mut RunIo<'_>) -> Result<ExecutionSummary, RuntimeError> {
        let mut outputs: Vec<Option<Vec<Value>>> = vec![None; self.nodes.len()];
        let mut remaining = self.nodes.len();

        while remaining > 0 {
            let mut progress = false;
            for node_index in 0..self.nodes.len() {
                if outputs[node_index].is_some() {
                    continue;
                }
                let incoming = self
                    .cords
                    .iter()
                    .filter(|cord| cord.to_node == node_index)
                    .collect::<Vec<_>>();
                if incoming
                    .iter()
                    .any(|cord| outputs[cord.from_node].is_none())
                {
                    continue;
                }

                let mut inputs = Vec::with_capacity(incoming.len());
                for input_port in 0..self.nodes[node_index].definition.contract.inputs.len() {
                    for cord in incoming.iter().filter(|cord| cord.to_port == input_port) {
                        let value = outputs[cord.from_node]
                            .as_ref()
                            .and_then(|values| values.get(cord.from_port))
                            .ok_or_else(|| {
                                RuntimeError::new(
                                    "CND-RUN-004",
                                    format!(
                                        "node `{}` did not emit required port {}",
                                        self.nodes[cord.from_node].source.id, cord.from_port
                                    ),
                                )
                            })?
                            .clone();
                        inputs.push(value);
                    }
                }

                let resolved = &self.nodes[node_index];
                let mut handler = (resolved.definition.factory())();
                let node_outputs = handler.run(&resolved.source, &inputs, io)?;
                if node_outputs.len() != resolved.definition.contract.outputs.len() {
                    return Err(RuntimeError::new(
                        "CND-RUN-004",
                        format!(
                            "node `{}` emitted {} ports; contract requires {}",
                            resolved.source.id,
                            node_outputs.len(),
                            resolved.definition.contract.outputs.len()
                        ),
                    ));
                }
                for (value, port) in node_outputs
                    .iter()
                    .zip(resolved.definition.contract.outputs)
                {
                    if value.value_type != port.value_type {
                        return Err(RuntimeError::new(
                            "CND-RUN-004",
                            format!(
                                "node `{}` emitted `{}` on `{}`; expected `{}`",
                                resolved.source.id,
                                value.value_type.contract_id,
                                port.id,
                                port.value_type.contract_id
                            ),
                        ));
                    }
                }
                outputs[node_index] = Some(node_outputs);
                remaining -= 1;
                progress = true;
            }
            if !progress {
                return Err(RuntimeError::new(
                    "CND-RUN-001",
                    "execution made no progress; the plan contains a dependency cycle",
                ));
            }
        }

        Ok(ExecutionSummary {
            nodes_completed: self.nodes.len(),
            cords_conducted: self.cords.len(),
        })
    }
}

struct HostValueStore {
    values: Vec<Vec<u8>>,
    retained_bytes: u64,
    maximum_bytes: u64,
}

impl HostValueStore {
    fn with_limit(maximum_bytes: u64) -> Self {
        Self {
            values: Vec::new(),
            retained_bytes: 0,
            maximum_bytes,
        }
    }

    fn store(&mut self, bytes: Vec<u8>) -> Option<u64> {
        let byte_count = u64::try_from(bytes.len()).ok()?;
        let retained_bytes = self.retained_bytes.checked_add(byte_count)?;
        if retained_bytes > self.maximum_bytes {
            return None;
        }
        let handle = self.values.len() as u64;
        self.values.push(bytes);
        self.retained_bytes = retained_bytes;
        Some(handle)
    }

    fn get(&self, handle: u64) -> Option<&[u8]> {
        self.values.get(handle as usize).map(|v| v.as_slice())
    }
}

// Runtime values carry the complete fixed envelope inline so executor
// allocation remains exact and no per-value metadata allocation is hidden.
#[allow(clippy::large_enum_variant)]
enum HostedNodeKind {
    Literal {
        value: Vec<u8>,
        emitted: bool,
    },
    Format {
        template_cord: usize,
        values_cord: usize,
        template: Option<RuntimeValue>,
        values: Option<RuntimeValue>,
        output: Option<RuntimeValue>,
        emitted: bool,
    },
    Lines {
        state: conduit_std::LinesState,
        input: Option<RuntimeValue>,
        cursor: usize,
        retained_bytes: usize,
        output: Vec<u8>,
        output_cursor: usize,
        pending_output: Option<RuntimeValue>,
        terminal_seen: bool,
        maximum_line_bytes: usize,
        maximum_retained_prefix_bytes: usize,
    },
    Join {
        inputs: Vec<RuntimeValue>,
        separator: Vec<u8>,
        output: Vec<u8>,
        copy_item: usize,
        copy_byte: usize,
        separator_cursor: usize,
        utf8: conduit_std::Utf8State,
        pending_output: Option<RuntimeValue>,
        terminal_seen: bool,
        emitted: bool,
        maximum_items: usize,
        maximum_item_bytes: usize,
        maximum_output_bytes: usize,
    },
    Stdin {
        emitted: bool,
    },
    Uppercase,
    Stdout,
    Stderr,
    PassThrough,
    Tee {
        isolated: bool,
        retained: Option<RuntimeValue>,
        delivered: [bool; 2],
    },
    Merge {
        inputs: [usize; 2],
        cursor: usize,
    },
    Zip {
        inputs: [usize; 2],
        left: Option<RuntimeValue>,
        right: Option<RuntimeValue>,
        drop_unpaired: bool,
    },
    Gate {
        input: usize,
        control: usize,
        open: bool,
    },
    Select {
        inputs: [usize; 2],
        control: usize,
        selected: usize,
    },
    Fallback {
        emitted: bool,
    },
    HostedService {
        handler: Box<dyn Handler>,
        node: Node,
        completed: bool,
    },
}

struct HostedSchedulerDriver<'r, 'i> {
    kind: HostedNodeKind,
    store: Rc<RefCell<HostValueStore>>,
    io: Rc<RefCell<&'r mut RunIo<'i>>>,
    in_cords: Vec<usize>,
    out_cords: Vec<usize>,
    maximum_input_bytes: u32,
    host_failure: Rc<RefCell<Option<RuntimeError>>>,
}

impl<'r, 'i> SchedulerNode for HostedSchedulerDriver<'r, 'i> {
    fn prepare(&mut self) -> Result<conduit_core::LifecycleUsage, Id<'static>> {
        Ok(conduit_core::LifecycleUsage::default())
    }

    fn start(&mut self) -> Result<conduit_core::LifecycleUsage, Id<'static>> {
        Ok(conduit_core::LifecycleUsage::default())
    }

    fn step(&mut self, io: &mut StepIo<'_, '_>) -> SchedulerStep {
        match &mut self.kind {
            HostedNodeKind::Literal { value, emitted } => {
                if *emitted {
                    return SchedulerStep::Completed;
                }
                if self.out_cords.is_empty() {
                    return SchedulerStep::Completed;
                }
                let Some(handle) = self.store.borrow_mut().store(value.clone()) else {
                    return SchedulerStep::Failed {
                        code: Id("conduit/value-store-bound-exceeded"),
                    };
                };
                let mut sent_any = false;
                for &out_cord in &self.out_cords {
                    let res = io.send(
                        out_cord,
                        RuntimeValue {
                            handle,
                            accounted_bytes: value.len() as u32,
                            envelope: RuntimeValueEnvelope::EMPTY,
                        },
                        None,
                    );
                    if matches!(res, Ok(SendStatus::Reserved)) {
                        sent_any = true;
                    }
                }
                if sent_any {
                    *emitted = true;
                    SchedulerStep::Progress
                } else {
                    SchedulerStep::Pending
                }
            }
            HostedNodeKind::Format {
                template_cord,
                values_cord,
                template,
                values,
                output,
                emitted,
            } => {
                if *emitted {
                    return SchedulerStep::Completed;
                }
                if template.is_none() {
                    if let Ok(Some(value)) = io.receive(*template_cord) {
                        *template = Some(value);
                    } else if matches!(
                        io.input_state(*template_cord),
                        Ok(FlowQueueState::Completed)
                    ) {
                        *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                            "format/missing-value",
                            "template input completed without a value",
                        ));
                        return SchedulerStep::Failed {
                            code: Id("format/missing-value"),
                        };
                    }
                }
                if values.is_none() {
                    if let Ok(Some(value)) = io.receive(*values_cord) {
                        *values = Some(value);
                    } else if matches!(io.input_state(*values_cord), Ok(FlowQueueState::Completed))
                    {
                        *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                            "format/missing-value",
                            "values input completed without a value",
                        ));
                        return SchedulerStep::Failed {
                            code: Id("format/missing-value"),
                        };
                    }
                }
                if template.is_none() || values.is_none() {
                    if template.is_none() {
                        let _ = io.wait_for_input(*template_cord);
                    }
                    if values.is_none() {
                        let _ = io.wait_for_input(*values_cord);
                    }
                    return SchedulerStep::Pending;
                }
                if output.is_none() {
                    let formatted = {
                        let store = self.store.borrow();
                        let template_bytes = store
                            .get(template.expect("checked template").handle)
                            .unwrap_or(&[]);
                        let values_bytes = store
                            .get(values.expect("checked values").handle)
                            .unwrap_or(&[]);
                        format_input_bytes(template_bytes, values_bytes)
                    };
                    let formatted = match formatted {
                        Ok(formatted) => formatted,
                        Err(error) => {
                            *self.host_failure.borrow_mut() = Some(format_runtime_error(error));
                            return SchedulerStep::Failed {
                                code: Id(error.code()),
                            };
                        }
                    };
                    let accounted_bytes = match u32::try_from(formatted.len()) {
                        Ok(bytes) => bytes,
                        Err(_) => {
                            return SchedulerStep::Failed {
                                code: Id("format/output-overflow"),
                            };
                        }
                    };
                    let Some(handle) = self.store.borrow_mut().store(formatted) else {
                        return SchedulerStep::Failed {
                            code: Id("conduit/value-store-bound-exceeded"),
                        };
                    };
                    *output = Some(RuntimeValue {
                        handle,
                        accounted_bytes,
                        envelope: RuntimeValueEnvelope::EMPTY,
                    });
                }
                let Some(&out_cord) = self.out_cords.first() else {
                    return SchedulerStep::Completed;
                };
                match io.send(out_cord, output.expect("format output exists"), None) {
                    Ok(SendStatus::Reserved) => {
                        *emitted = true;
                        SchedulerStep::Progress
                    }
                    Ok(_) | Err(_) => SchedulerStep::Pending,
                }
            }
            HostedNodeKind::Lines {
                state,
                input,
                cursor,
                retained_bytes,
                output,
                output_cursor,
                pending_output,
                terminal_seen,
                maximum_line_bytes,
                maximum_retained_prefix_bytes,
            } => {
                let Some(&out_cord) = self.out_cords.first() else {
                    return SchedulerStep::Completed;
                };
                if let Some(output) = *pending_output {
                    return match io.send(out_cord, output, None) {
                        Ok(SendStatus::Reserved) => {
                            *pending_output = None;
                            SchedulerStep::Progress
                        }
                        Ok(_) | Err(_) => {
                            let _ = io.wait_for_output(out_cord);
                            SchedulerStep::Pending
                        }
                    };
                }
                if let Some(length) = state.ready_len() {
                    if length > *maximum_line_bytes {
                        *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                            "CND-TXT-003",
                            "lines output exceeded the exact line bound",
                        ));
                        return SchedulerStep::Failed {
                            code: Id("CND-TXT-003"),
                        };
                    }
                    while *output_cursor < length && io.remaining_work() > 0 {
                        if io.consume_work(1).is_err() {
                            return SchedulerStep::Failed {
                                code: Id("conduit/step-work-bound-exceeded"),
                            };
                        }
                        let Some(byte) = state.ready_byte(*output_cursor) else {
                            return SchedulerStep::Failed {
                                code: Id("CND-TXT-003"),
                            };
                        };
                        output.push(byte);
                        *output_cursor += 1;
                    }
                    if *output_cursor < length || io.remaining_work() == 0 {
                        return if io.record_host_progress().is_ok() {
                            SchedulerStep::Progress
                        } else {
                            SchedulerStep::Failed {
                                code: Id("conduit/step-work-bound-exceeded"),
                            }
                        };
                    }
                    if io.consume_work(1).is_err() {
                        return SchedulerStep::Failed {
                            code: Id("conduit/step-work-bound-exceeded"),
                        };
                    }
                    if state.clear_ready().is_err() {
                        return SchedulerStep::Failed {
                            code: Id("CND-TXT-003"),
                        };
                    }
                    let bytes = std::mem::replace(output, Vec::with_capacity(*maximum_line_bytes));
                    *output_cursor = 0;
                    *retained_bytes = 0;
                    let accounted_bytes = bytes.len() as u32;
                    let Some(handle) = self.store.borrow_mut().store(bytes) else {
                        return SchedulerStep::Failed {
                            code: Id("conduit/value-store-bound-exceeded"),
                        };
                    };
                    let output_value = RuntimeValue {
                        handle,
                        accounted_bytes,
                        envelope: RuntimeValueEnvelope::EMPTY,
                    };
                    return match io.send(out_cord, output_value, None) {
                        Ok(SendStatus::Reserved) => SchedulerStep::Progress,
                        Ok(_) | Err(_) => {
                            *pending_output = Some(output_value);
                            if io.record_host_progress().is_err() {
                                return SchedulerStep::Failed {
                                    code: Id("conduit/step-work-bound-exceeded"),
                                };
                            }
                            SchedulerStep::Progress
                        }
                    };
                }
                let in_cord = match self.in_cords.first() {
                    Some(&cord) => cord,
                    None => return SchedulerStep::Completed,
                };
                if input.is_none() && !*terminal_seen {
                    match io.receive(in_cord) {
                        Ok(Some(value)) => {
                            *input = Some(value);
                            *cursor = 0;
                            return SchedulerStep::Progress;
                        }
                        _ if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) => {
                            *terminal_seen = true;
                        }
                        _ => {
                            let _ = io.wait_for_input(in_cord);
                            return SchedulerStep::Pending;
                        }
                    }
                }
                while let Some(value) = *input {
                    if io.remaining_work() == 0 {
                        return if io.record_host_progress().is_ok() {
                            SchedulerStep::Progress
                        } else {
                            SchedulerStep::Failed {
                                code: Id("conduit/step-work-bound-exceeded"),
                            }
                        };
                    }
                    let next = {
                        let store = self.store.borrow();
                        store
                            .get(value.handle)
                            .and_then(|bytes| bytes.get(*cursor))
                            .copied()
                    };
                    let Some(byte) = next else {
                        *input = None;
                        *cursor = 0;
                        if io.record_host_progress().is_err() {
                            return SchedulerStep::Failed {
                                code: Id("conduit/step-work-bound-exceeded"),
                            };
                        }
                        return SchedulerStep::Progress;
                    };
                    if io.consume_work(1).is_err() {
                        return SchedulerStep::Failed {
                            code: Id("conduit/step-work-bound-exceeded"),
                        };
                    }
                    *cursor += 1;
                    if byte != b'\n' {
                        *retained_bytes += 1;
                        if *retained_bytes > *maximum_retained_prefix_bytes {
                            *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                "CND-TXT-001",
                                "lines retained prefix exceeded the exact bound",
                            ));
                            return SchedulerStep::Failed {
                                code: Id("CND-TXT-001"),
                            };
                        }
                    }
                    match state.push_byte(byte) {
                        Ok(true) => {
                            if io.remaining_work() == 0 {
                                return if io.record_host_progress().is_ok() {
                                    SchedulerStep::Progress
                                } else {
                                    SchedulerStep::Failed {
                                        code: Id("conduit/step-work-bound-exceeded"),
                                    }
                                };
                            }
                            if io.record_host_progress().is_err() {
                                return SchedulerStep::Failed {
                                    code: Id("conduit/step-work-bound-exceeded"),
                                };
                            }
                            return SchedulerStep::Progress;
                        }
                        Ok(false) => {}
                        Err(error) => {
                            *self.host_failure.borrow_mut() = Some(text_line_runtime_error(error));
                            return SchedulerStep::Failed {
                                code: Id(error.code()),
                            };
                        }
                    }
                }
                if *terminal_seen {
                    match state.finish() {
                        Ok(true) => {
                            if io.record_host_progress().is_err() {
                                SchedulerStep::Failed {
                                    code: Id("conduit/step-work-bound-exceeded"),
                                }
                            } else {
                                SchedulerStep::Progress
                            }
                        }
                        Ok(false) => SchedulerStep::Completed,
                        Err(error) => {
                            *self.host_failure.borrow_mut() = Some(text_line_runtime_error(error));
                            SchedulerStep::Failed {
                                code: Id(error.code()),
                            }
                        }
                    }
                } else {
                    SchedulerStep::Progress
                }
            }
            HostedNodeKind::Join {
                inputs,
                separator,
                output,
                copy_item,
                copy_byte,
                separator_cursor,
                utf8,
                pending_output,
                terminal_seen,
                emitted,
                maximum_items,
                maximum_item_bytes,
                maximum_output_bytes,
            } => {
                if *emitted {
                    return SchedulerStep::Completed;
                }
                let Some(&out_cord) = self.out_cords.first() else {
                    return SchedulerStep::Completed;
                };
                if let Some(output) = *pending_output {
                    return match io.send(out_cord, output, None) {
                        Ok(SendStatus::Reserved) => {
                            *emitted = true;
                            SchedulerStep::Progress
                        }
                        Ok(_) | Err(_) => {
                            let _ = io.wait_for_output(out_cord);
                            SchedulerStep::Pending
                        }
                    };
                }
                let in_cord = match self.in_cords.first() {
                    Some(&cord) => cord,
                    None => return SchedulerStep::Completed,
                };
                if !*terminal_seen {
                    match io.receive(in_cord) {
                        Ok(Some(value)) => {
                            if inputs.len() >= *maximum_items {
                                *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                    "CND-TXT-004",
                                    "join input exceeded the exact retained-item bound",
                                ));
                                return SchedulerStep::Failed {
                                    code: Id("CND-TXT-004"),
                                };
                            }
                            if value.accounted_bytes as usize > *maximum_item_bytes {
                                *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                    "CND-TXT-005",
                                    "join item exceeded the exact item-byte bound",
                                ));
                                return SchedulerStep::Failed {
                                    code: Id("CND-TXT-005"),
                                };
                            }
                            inputs.push(value);
                            return SchedulerStep::Progress;
                        }
                        _ if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) => {
                            *terminal_seen = true;
                        }
                        _ => {
                            let _ = io.wait_for_input(in_cord);
                            return SchedulerStep::Pending;
                        }
                    }
                }
                while *copy_item < inputs.len() && io.remaining_work() > 0 {
                    if *copy_item > 0 && *separator_cursor < separator.len() {
                        if io.consume_work(1).is_err() {
                            return SchedulerStep::Failed {
                                code: Id("conduit/step-work-bound-exceeded"),
                            };
                        }
                        if output.len() >= *maximum_output_bytes {
                            *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                "CND-TXT-007",
                                "joined output exceeded the exact output bound",
                            ));
                            return SchedulerStep::Failed {
                                code: Id("CND-TXT-007"),
                            };
                        }
                        output.push(separator[*separator_cursor]);
                        *separator_cursor += 1;
                        continue;
                    }
                    let next = {
                        let store = self.store.borrow();
                        store
                            .get(inputs[*copy_item].handle)
                            .and_then(|bytes| bytes.get(*copy_byte))
                            .copied()
                    };
                    if let Some(byte) = next {
                        if io.consume_work(1).is_err() {
                            return SchedulerStep::Failed {
                                code: Id("conduit/step-work-bound-exceeded"),
                            };
                        }
                        if utf8.push_byte(byte).is_err() {
                            *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                "CND-TXT-002",
                                "join input was not valid UTF-8",
                            ));
                            return SchedulerStep::Failed {
                                code: Id("CND-TXT-002"),
                            };
                        }
                        if output.len() >= *maximum_output_bytes {
                            *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                "CND-TXT-007",
                                "joined output exceeded the exact output bound",
                            ));
                            return SchedulerStep::Failed {
                                code: Id("CND-TXT-007"),
                            };
                        }
                        output.push(byte);
                        *copy_byte += 1;
                        let item_complete = {
                            let store = self.store.borrow();
                            store
                                .get(inputs[*copy_item].handle)
                                .is_some_and(|bytes| *copy_byte == bytes.len())
                        };
                        if item_complete {
                            if utf8.finish().is_err() {
                                *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                    "CND-TXT-002",
                                    "join input was not valid UTF-8",
                                ));
                                return SchedulerStep::Failed {
                                    code: Id("CND-TXT-002"),
                                };
                            }
                            utf8.reset();
                            *copy_item += 1;
                            *copy_byte = 0;
                            *separator_cursor = 0;
                        }
                    } else {
                        if *copy_byte != 0 || io.consume_work(1).is_err() {
                            return SchedulerStep::Failed {
                                code: Id("conduit/value-store-missing"),
                            };
                        }
                        utf8.reset();
                        *copy_item += 1;
                        *separator_cursor = 0;
                    }
                }
                if *copy_item < inputs.len() {
                    return if io.record_host_progress().is_ok() {
                        SchedulerStep::Progress
                    } else {
                        SchedulerStep::Failed {
                            code: Id("conduit/step-work-bound-exceeded"),
                        }
                    };
                }
                let accounted_bytes = output.len() as u32;
                let joined = std::mem::take(output);
                let Some(handle) = self.store.borrow_mut().store(joined) else {
                    return SchedulerStep::Failed {
                        code: Id("conduit/value-store-bound-exceeded"),
                    };
                };
                let output = RuntimeValue {
                    handle,
                    accounted_bytes,
                    envelope: RuntimeValueEnvelope::EMPTY,
                };
                match io.send(out_cord, output, None) {
                    Ok(SendStatus::Reserved) => {
                        *emitted = true;
                        SchedulerStep::Progress
                    }
                    Ok(_) | Err(_) => {
                        *pending_output = Some(output);
                        if io.record_host_progress().is_err() {
                            return SchedulerStep::Failed {
                                code: Id("conduit/step-work-bound-exceeded"),
                            };
                        }
                        SchedulerStep::Progress
                    }
                }
            }
            HostedNodeKind::Stdin { emitted } => {
                if *emitted {
                    return SchedulerStep::Completed;
                }
                if self.out_cords.is_empty() {
                    return SchedulerStep::Completed;
                }
                let mut bytes = Vec::new();
                let mut chunk = [0_u8; 4096];
                loop {
                    let remaining = usize::try_from(self.maximum_input_bytes)
                        .unwrap_or(usize::MAX)
                        .saturating_sub(bytes.len());
                    if remaining == 0 {
                        let mut extra = [0_u8; 1];
                        match self.io.borrow_mut().input.read(&mut extra) {
                            Ok(0) => break,
                            Ok(_) => {
                                return SchedulerStep::Failed {
                                    code: Id("io/stdin-bound-exceeded"),
                                };
                            }
                            Err(_) => {
                                return SchedulerStep::Failed {
                                    code: Id("io/stdin-read-error"),
                                };
                            }
                        }
                    }
                    let read_limit = remaining.min(chunk.len());
                    match self.io.borrow_mut().input.read(&mut chunk[..read_limit]) {
                        Ok(0) => break,
                        Ok(read) => bytes.extend_from_slice(&chunk[..read]),
                        Err(_) => {
                            return SchedulerStep::Failed {
                                code: Id("io/stdin-read-error"),
                            };
                        }
                    }
                }
                let Some(handle) = self.store.borrow_mut().store(bytes.clone()) else {
                    return SchedulerStep::Failed {
                        code: Id("conduit/value-store-bound-exceeded"),
                    };
                };
                let mut sent_any = false;
                for &out_cord in &self.out_cords {
                    let res = io.send(
                        out_cord,
                        RuntimeValue {
                            handle,
                            accounted_bytes: bytes.len() as u32,
                            envelope: RuntimeValueEnvelope::EMPTY,
                        },
                        None,
                    );
                    if matches!(res, Ok(SendStatus::Reserved)) {
                        sent_any = true;
                    }
                }
                if sent_any {
                    *emitted = true;
                    SchedulerStep::Progress
                } else {
                    SchedulerStep::Pending
                }
            }
            HostedNodeKind::HostedService {
                handler,
                node,
                completed,
            } => {
                if *completed {
                    return SchedulerStep::Completed;
                }
                match handler.run(node, &[], &mut self.io.borrow_mut()) {
                    Ok(outputs) if outputs.is_empty() => {
                        *completed = true;
                        SchedulerStep::Completed
                    }
                    Ok(_) => SchedulerStep::Failed {
                        code: Id("conduit/host-service-hidden-output"),
                    },
                    Err(error) => {
                        *self.host_failure.borrow_mut() = Some(error);
                        SchedulerStep::Failed {
                            code: Id("conduit/host-service-failed"),
                        }
                    }
                }
            }
            HostedNodeKind::Uppercase => {
                let in_cord = match self.in_cords.first() {
                    Some(&c) => c,
                    None => return SchedulerStep::Completed,
                };
                if self.out_cords.is_empty() {
                    return SchedulerStep::Completed;
                }
                if let Ok(Some(val)) = io.receive(in_cord) {
                    let store = self.store.borrow();
                    let bytes = store.get(val.handle).unwrap_or(&[]);
                    let text = std::str::from_utf8(bytes).unwrap_or("");
                    let upper_bytes = text.to_uppercase().into_bytes();
                    drop(store);
                    let Some(handle) = self.store.borrow_mut().store(upper_bytes.clone()) else {
                        return SchedulerStep::Failed {
                            code: Id("conduit/value-store-bound-exceeded"),
                        };
                    };
                    let mut sent_any = false;
                    for &out_cord in &self.out_cords {
                        let res = io.send(
                            out_cord,
                            RuntimeValue {
                                handle,
                                accounted_bytes: upper_bytes.len() as u32,
                                envelope: RuntimeValueEnvelope::EMPTY,
                            },
                            None,
                        );
                        if matches!(res, Ok(SendStatus::Reserved)) {
                            sent_any = true;
                        }
                    }
                    if sent_any {
                        SchedulerStep::Progress
                    } else {
                        SchedulerStep::Pending
                    }
                } else if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) {
                    SchedulerStep::Completed
                } else {
                    let _ = io.wait_for_input(in_cord);
                    SchedulerStep::Pending
                }
            }
            HostedNodeKind::Stdout => {
                let in_cord = match self.in_cords.first() {
                    Some(&c) => c,
                    None => return SchedulerStep::Completed,
                };
                if let Ok(Some(val)) = io.receive(in_cord) {
                    let store = self.store.borrow();
                    let bytes = store.get(val.handle).unwrap_or(&[]);
                    if self.io.borrow_mut().output.write_all(bytes).is_err() {
                        return SchedulerStep::Failed {
                            code: Id("io/stdout-write-error"),
                        };
                    }
                    SchedulerStep::Progress
                } else if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) {
                    SchedulerStep::Completed
                } else {
                    let _ = io.wait_for_input(in_cord);
                    SchedulerStep::Pending
                }
            }
            HostedNodeKind::Stderr => {
                let in_cord = match self.in_cords.first() {
                    Some(&c) => c,
                    None => return SchedulerStep::Completed,
                };
                if let Ok(Some(val)) = io.receive(in_cord) {
                    let store = self.store.borrow();
                    let bytes = store.get(val.handle).unwrap_or(&[]);
                    if self.io.borrow_mut().error.write_all(bytes).is_err() {
                        return SchedulerStep::Failed {
                            code: Id("io/stderr-write-error"),
                        };
                    }
                    SchedulerStep::Progress
                } else if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) {
                    SchedulerStep::Completed
                } else {
                    let _ = io.wait_for_input(in_cord);
                    SchedulerStep::Pending
                }
            }
            HostedNodeKind::PassThrough => {
                let in_cord = match self.in_cords.first() {
                    Some(&c) => c,
                    None => return SchedulerStep::Completed,
                };
                if self.out_cords.is_empty() {
                    return SchedulerStep::Completed;
                }
                if let Ok(Some(val)) = io.receive(in_cord) {
                    let mut sent_any = false;
                    for &out_cord in &self.out_cords {
                        let res = io.send(out_cord, val, None);
                        if matches!(res, Ok(SendStatus::Reserved)) {
                            sent_any = true;
                        }
                    }
                    if sent_any {
                        SchedulerStep::Progress
                    } else {
                        SchedulerStep::Pending
                    }
                } else if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) {
                    SchedulerStep::Completed
                } else {
                    let _ = io.wait_for_input(in_cord);
                    SchedulerStep::Pending
                }
            }
            HostedNodeKind::Tee {
                isolated,
                retained,
                delivered,
            } => {
                let in_cord = match self.in_cords.first() {
                    Some(&c) => c,
                    None => return SchedulerStep::Completed,
                };
                if *isolated {
                    if retained.is_none() {
                        if let Ok(Some(value)) = io.receive(in_cord) {
                            *retained = Some(value);
                            *delivered = [false; 2];
                            return SchedulerStep::Progress;
                        }
                        if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) {
                            return SchedulerStep::Completed;
                        }
                        let _ = io.wait_for_input(in_cord);
                        return SchedulerStep::Pending;
                    }
                    if let Some((branch, &out_cord)) = self
                        .out_cords
                        .iter()
                        .enumerate()
                        .find(|(branch, _)| !delivered[*branch])
                    {
                        return match io.send(
                            out_cord,
                            retained.expect("isolated tee retained input"),
                            None,
                        ) {
                            Ok(SendStatus::Reserved) => {
                                delivered[branch] = true;
                                SchedulerStep::Progress
                            }
                            Ok(SendStatus::WouldBlock) => {
                                let _ = io.wait_for_output(out_cord);
                                SchedulerStep::Pending
                            }
                            Ok(_) | Err(_) => SchedulerStep::Failed {
                                code: Id("conduit.std/tee-branch-rejected"),
                            },
                        };
                    }
                    *retained = None;
                    *delivered = [false; 2];
                    return if io.record_host_progress().is_ok() {
                        SchedulerStep::Progress
                    } else {
                        SchedulerStep::Failed {
                            code: Id("conduit/step-work-bound-exceeded"),
                        }
                    };
                }
                let value = match io.receive(in_cord) {
                    Ok(Some(value)) => value,
                    _ if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) => {
                        return SchedulerStep::Completed;
                    }
                    _ => {
                        let _ = io.wait_for_input(in_cord);
                        return SchedulerStep::Pending;
                    }
                };
                for &out_cord in &self.out_cords {
                    match io.send(out_cord, value, None) {
                        Ok(SendStatus::Reserved) => {}
                        Ok(SendStatus::WouldBlock) => {
                            let _ = io.wait_for_output(out_cord);
                            return SchedulerStep::Pending;
                        }
                        Ok(_) | Err(_) => {
                            return SchedulerStep::Failed {
                                code: Id("conduit.std/tee-branch-rejected"),
                            };
                        }
                    }
                }
                SchedulerStep::Progress
            }
            HostedNodeKind::Merge { inputs, cursor } => {
                let Some(&out_cord) = self.out_cords.first() else {
                    return SchedulerStep::Completed;
                };
                for offset in 0..inputs.len() {
                    let ordinal = (*cursor + offset) % inputs.len();
                    let in_cord = inputs[ordinal];
                    if let Ok(Some(value)) = io.receive(in_cord) {
                        return match io.send(out_cord, value, None) {
                            Ok(SendStatus::Reserved) => {
                                *cursor = (ordinal + 1) % inputs.len();
                                SchedulerStep::Progress
                            }
                            Ok(SendStatus::WouldBlock) => {
                                let _ = io.wait_for_output(out_cord);
                                SchedulerStep::Pending
                            }
                            Ok(_) | Err(_) => SchedulerStep::Failed {
                                code: Id("conduit.std/merge-output-rejected"),
                            },
                        };
                    }
                }
                if inputs
                    .iter()
                    .all(|&c| matches!(io.input_state(c), Ok(FlowQueueState::Completed)))
                {
                    SchedulerStep::Completed
                } else {
                    for &in_cord in inputs.iter() {
                        let _ = io.wait_for_input(in_cord);
                    }
                    SchedulerStep::Pending
                }
            }
            HostedNodeKind::Zip {
                inputs,
                left,
                right,
                drop_unpaired,
            } => {
                if left.is_none() {
                    if let Ok(Some(value)) = io.receive(inputs[0]) {
                        *left = Some(value);
                        return SchedulerStep::Progress;
                    }
                }
                if right.is_none() {
                    if let Ok(Some(value)) = io.receive(inputs[1]) {
                        *right = Some(value);
                        return SchedulerStep::Progress;
                    }
                }
                if let (Some(left_value), Some(right_value)) = (*left, *right) {
                    let Some(&left_out) = self.out_cords.first() else {
                        return SchedulerStep::Completed;
                    };
                    let Some(&right_out) = self.out_cords.get(1) else {
                        return SchedulerStep::Failed {
                            code: Id("conduit.std/zip-output-missing"),
                        };
                    };
                    for (cord, value) in [(left_out, left_value), (right_out, right_value)] {
                        match io.send(cord, value, None) {
                            Ok(SendStatus::Reserved) => {}
                            Ok(SendStatus::WouldBlock) => {
                                let _ = io.wait_for_output(cord);
                                return SchedulerStep::Pending;
                            }
                            Ok(_) | Err(_) => {
                                return SchedulerStep::Failed {
                                    code: Id("conduit.std/zip-output-rejected"),
                                };
                            }
                        }
                    }
                    *left = None;
                    *right = None;
                    return SchedulerStep::Progress;
                }
                let left_done = matches!(io.input_state(inputs[0]), Ok(FlowQueueState::Completed));
                let right_done = matches!(io.input_state(inputs[1]), Ok(FlowQueueState::Completed));
                if left_done || right_done {
                    if left.is_some() || right.is_some() {
                        if *drop_unpaired {
                            *left = None;
                            *right = None;
                            return SchedulerStep::Completed;
                        }
                        *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                            "conduit.std/zip-unpaired",
                            "zip input terminated with an unpaired value",
                        ));
                        return SchedulerStep::Failed {
                            code: Id("conduit.std/zip-unpaired"),
                        };
                    }
                    return SchedulerStep::Completed;
                }
                if left.is_none() {
                    let _ = io.wait_for_input(inputs[0]);
                }
                if right.is_none() {
                    let _ = io.wait_for_input(inputs[1]);
                }
                SchedulerStep::Pending
            }
            HostedNodeKind::Gate {
                input,
                control,
                open,
            } => {
                if let Ok(Some(value)) = io.receive(*control) {
                    let next = {
                        let store = self.store.borrow();
                        match store.get(value.handle).unwrap_or(&[]) {
                            b"open" => Some(true),
                            b"closed" => Some(false),
                            _ => None,
                        }
                    };
                    let Some(next) = next else {
                        *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                            "conduit.std/gate-invalid-control",
                            "gate control must be `open` or `closed`",
                        ));
                        return SchedulerStep::Failed {
                            code: Id("conduit.std/gate-invalid-control"),
                        };
                    };
                    *open = next;
                    return SchedulerStep::Progress;
                }
                if *open {
                    if let Ok(Some(value)) = io.receive(*input) {
                        let Some(&out_cord) = self.out_cords.first() else {
                            return SchedulerStep::Completed;
                        };
                        return match io.send(out_cord, value, None) {
                            Ok(SendStatus::Reserved) => SchedulerStep::Progress,
                            Ok(SendStatus::WouldBlock) => {
                                let _ = io.wait_for_output(out_cord);
                                SchedulerStep::Pending
                            }
                            Ok(_) | Err(_) => SchedulerStep::Failed {
                                code: Id("conduit.std/gate-output-rejected"),
                            },
                        };
                    }
                }
                if matches!(io.input_state(*input), Ok(FlowQueueState::Completed)) {
                    return SchedulerStep::Completed;
                }
                if !*open && matches!(io.input_state(*control), Ok(FlowQueueState::Completed)) {
                    *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                        "conduit.std/gate-closed-terminal",
                        "gate control terminated while the gate was closed",
                    ));
                    return SchedulerStep::Failed {
                        code: Id("conduit.std/gate-closed-terminal"),
                    };
                }
                let _ = io.wait_for_input(*control);
                if *open {
                    let _ = io.wait_for_input(*input);
                }
                SchedulerStep::Pending
            }
            HostedNodeKind::Select {
                inputs,
                control,
                selected,
            } => {
                if let Ok(Some(value)) = io.receive(*control) {
                    let next = {
                        let store = self.store.borrow();
                        match store.get(value.handle).unwrap_or(&[]) {
                            b"left" => Some(0),
                            b"right" => Some(1),
                            _ => None,
                        }
                    };
                    let Some(next) = next else {
                        *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                            "conduit.std/select-invalid-control",
                            "select command must be `left` or `right`",
                        ));
                        return SchedulerStep::Failed {
                            code: Id("conduit.std/select-invalid-control"),
                        };
                    };
                    *selected = next;
                    return SchedulerStep::Progress;
                }
                let selected_cord = inputs[*selected];
                if let Ok(Some(value)) = io.receive(selected_cord) {
                    let Some(&out_cord) = self.out_cords.first() else {
                        return SchedulerStep::Completed;
                    };
                    return match io.send(out_cord, value, None) {
                        Ok(SendStatus::Reserved) => SchedulerStep::Progress,
                        Ok(SendStatus::WouldBlock) => {
                            let _ = io.wait_for_output(out_cord);
                            SchedulerStep::Pending
                        }
                        Ok(_) | Err(_) => SchedulerStep::Failed {
                            code: Id("conduit.std/select-output-rejected"),
                        },
                    };
                }
                if matches!(io.input_state(selected_cord), Ok(FlowQueueState::Completed))
                    && matches!(io.input_state(*control), Ok(FlowQueueState::Completed))
                {
                    if inputs
                        .iter()
                        .all(|&cord| matches!(io.input_state(cord), Ok(FlowQueueState::Completed)))
                    {
                        return SchedulerStep::Completed;
                    }
                    *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                        "conduit.std/select-inactive-terminal",
                        "select control terminated with an inactive input remainder",
                    ));
                    return SchedulerStep::Failed {
                        code: Id("conduit.std/select-inactive-terminal"),
                    };
                }
                if inputs
                    .iter()
                    .all(|&cord| matches!(io.input_state(cord), Ok(FlowQueueState::Completed)))
                {
                    return SchedulerStep::Completed;
                }
                let _ = io.wait_for_input(*control);
                let _ = io.wait_for_input(selected_cord);
                SchedulerStep::Pending
            }
            HostedNodeKind::Fallback { emitted } => {
                if *emitted {
                    return SchedulerStep::Completed;
                }
                let primary_cord = self.in_cords.first().copied();
                let fallback_cord = self.in_cords.get(1).copied();
                let out_cord = self.out_cords.first().copied();

                if let Some(p_cord) = primary_cord {
                    if let Ok(Some(val)) = io.receive(p_cord) {
                        if let Some(out) = out_cord {
                            let _ = io.send(out, val, None);
                        }
                        *emitted = true;
                        return SchedulerStep::Progress;
                    }
                }
                let primary_completed = primary_cord
                    .map(|p| matches!(io.input_state(p), Ok(FlowQueueState::Completed)))
                    .unwrap_or(true);
                if primary_completed {
                    if let Some(f_cord) = fallback_cord {
                        if let Ok(Some(val)) = io.receive(f_cord) {
                            if let Some(out) = out_cord {
                                let _ = io.send(out, val, None);
                            }
                            *emitted = true;
                            return SchedulerStep::Progress;
                        }
                    }
                }
                if self
                    .in_cords
                    .iter()
                    .all(|&c| matches!(io.input_state(c), Ok(FlowQueueState::Completed)))
                {
                    SchedulerStep::Completed
                } else {
                    for &in_cord in &self.in_cords {
                        let _ = io.wait_for_input(in_cord);
                    }
                    SchedulerStep::Pending
                }
            }
        }
    }
}

fn exact_topology_port(port: &PortContract<'static>) -> Result<ExactTopologyPort, ResolutionError> {
    Ok(ExactTopologyPort {
        id: port.id.as_str().to_owned(),
        direction: port.direction,
        contract_hash: port
            .semantic_hash()
            .map_err(|_| ResolutionError::new("CND-CMP-002", "port contract is malformed"))?,
        value_type: port.value_type,
    })
}

fn semantic_hash_text(value: &str) -> Option<SemanticHash> {
    let value = value.strip_prefix("sha256:")?;
    if value.len() != 64 {
        return None;
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        bytes[index] = (high << 4) | low;
    }
    Some(SemanticHash::from_bytes(bytes))
}

fn composite_definition_hash(
    source_semantic_hash: SemanticHash,
    definition: &str,
) -> Result<SemanticHash, ResolutionError> {
    let definition = Id::new(definition)
        .map_err(|_| ResolutionError::new("CND-CMP-002", "composite id is malformed"))?;
    let fields = [
        MapField {
            name: Id("source_semantic_hash"),
            value: CanonicalValue::Bytes(source_semantic_hash.as_bytes()),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("definition"),
            value: CanonicalValue::Identifier(definition),
            disposition: FieldDisposition::Semantic,
        },
    ];
    CanonicalDescriptor {
        kind: Id("conduit/composite-definition-ref"),
        schema_version: 1,
        body: CanonicalValue::Map(&fields),
    }
    .semantic_hash()
    .map_err(|_| ResolutionError::new("CND-CMP-002", "composite identity is malformed"))
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn resolved_port_view(port: &PortContract<'_>) -> ResolvedPortView {
    ResolvedPortView {
        id: port.id.as_str().to_owned(),
        type_id: port.value_type.contract_id.as_str().to_owned(),
        delivery: port.delivery.as_str(),
        connections: port.connections.as_str(),
    }
}

fn resolve_flow(source: &conduit_panel::Cord) -> Result<FlowPolicy<'_>, ResolutionError> {
    let capacity = FlowCapacity::new(
        source.capacity_items,
        source.max_value_bytes,
        source.max_queued_bytes,
    )
    .map_err(|error| ResolutionError::new(error.code(), error.to_string()))?;
    let watermarks = FlowWatermarks::new(
        source.low_watermark_items,
        source.high_watermark_items,
        capacity,
    )
    .map_err(|error| ResolutionError::new(error.code(), error.to_string()))?;
    let pressure = match &source.pressure {
        SourcePressure::Block => Pressure::Block(BlockingFairness::Fifo),
        SourcePressure::Reject => Pressure::Reject,
        SourcePressure::Coalesce { relation } => Pressure::Coalesce {
            relation: Id(relation),
        },
        SourcePressure::Sample { every, offset } => Pressure::Sample(
            SampleSchedule::new(*every, *offset)
                .map_err(|error| ResolutionError::new(error.code(), error.to_string()))?,
        ),
        SourcePressure::DropDisposable => Pressure::DropDisposable,
        SourcePressure::Disconnect => Pressure::Disconnect,
        SourcePressure::Fail => Pressure::Fail,
    };
    FlowPolicy::new(capacity, pressure, watermarks)
        .map_err(|error| ResolutionError::new(error.code(), error.to_string()))
}

/// Successful execution counts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionSummary {
    /// Nodes that reached completion.
    pub nodes_completed: usize,
    /// Resolved cords in the conducted plan.
    pub cords_conducted: usize,
}

/// Resolution failure with a stable diagnostic code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionError {
    /// Stable code.
    pub code: &'static str,
    /// Human-readable detail.
    pub message: String,
}

impl ResolutionError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ResolutionError {}

/// Runtime failure with a stable diagnostic code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeError {
    /// Stable code.
    pub code: &'static str,
    /// Human-readable detail.
    pub message: String,
}

impl RuntimeError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for RuntimeError {}

fn node_index(nodes: &[ResolvedNode<'_>], id: &str) -> Result<usize, ResolutionError> {
    let matches = nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.source.id == id)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [index] => Ok(*index),
        [] => Err(ResolutionError::new(
            "CND-ID-003",
            format!("unknown node `{id}`"),
        )),
        _ => Err(ResolutionError::new(
            "CND-ID-002",
            format!("duplicate node id `{id}`"),
        )),
    }
}

fn port_index(ports: &[PortContract<'_>], id: &str, node: &str) -> Result<usize, ResolutionError> {
    ports
        .iter()
        .position(|port| port.id.as_str() == id)
        .ok_or_else(|| ResolutionError::new("CND-ID-003", format!("unknown port `{node}.{id}`")))
}

fn reject_cycles(
    nodes: &[ResolvedNode<'_>],
    cords: &[ResolvedCord],
) -> Result<(), ResolutionError> {
    let mut completed = vec![false; nodes.len()];
    let mut remaining = nodes.len();
    while remaining > 0 {
        let mut progress = false;
        for node in 0..nodes.len() {
            if completed[node] {
                continue;
            }
            if cords
                .iter()
                .filter(|cord| cord.to_node == node)
                .all(|cord| completed[cord.from_node])
            {
                completed[node] = true;
                remaining -= 1;
                progress = true;
            }
        }
        if !progress {
            return Err(ResolutionError::new(
                "CND-CMP-001",
                "panel contains a dependency cycle",
            ));
        }
    }
    Ok(())
}

fn validate_empty_config(node: &Node) -> Result<(), ResolutionError> {
    if let Some(entry) = node.config.first() {
        return Err(ResolutionError::new(
            "CND-SRC-002",
            format!(
                "node `{}` does not accept configuration field `{}`",
                node.id, entry.key
            ),
        ));
    }
    Ok(())
}

fn validate_enum_config(node: &Node, fields: &[(&str, &[&str])]) -> Result<(), ResolutionError> {
    for entry in &node.config {
        let Some((_, accepted)) = fields.iter().find(|(key, _)| *key == entry.key) else {
            return Err(ResolutionError::new(
                "CND-SRC-002",
                format!(
                    "node `{}` has unknown configuration field `{}`",
                    node.id, entry.key
                ),
            ));
        };
        let Some(value) = node.config(entry.key.as_str()) else {
            return Err(ResolutionError::new(
                "CND-SRC-002",
                format!("node `{}` requires text field `{}`", node.id, entry.key),
            ));
        };
        if !accepted.contains(&value) {
            return Err(ResolutionError::new(
                "CND-IMP-001",
                format!(
                    "node `{}` requests unsupported {} profile `{value}`",
                    node.id, entry.key
                ),
            ));
        }
    }
    Ok(())
}

fn validate_tee(node: &Node) -> Result<(), ResolutionError> {
    validate_enum_config(node, &[("mode", &["coupled", "isolated"])])
}

fn validate_merge(node: &Node) -> Result<(), ResolutionError> {
    validate_enum_config(node, &[("ordering", &["round-robin"])])
}

fn validate_zip(node: &Node) -> Result<(), ResolutionError> {
    validate_enum_config(node, &[("unpaired", &["fail", "drop"])])
}

fn validate_gate(node: &Node) -> Result<(), ResolutionError> {
    validate_enum_config(
        node,
        &[("initial", &["open", "closed"]), ("retained", &["block"])],
    )
}

fn validate_select(node: &Node) -> Result<(), ResolutionError> {
    validate_enum_config(
        node,
        &[("initial", &["left", "right"]), ("inactive", &["block"])],
    )
}

fn validate_contract_config(_node: &Node) -> Result<(), ResolutionError> {
    Ok(())
}

fn validate_literal(node: &Node) -> Result<(), ResolutionError> {
    if node.config("value").is_none() {
        return Err(ResolutionError::new(
            "CND-SRC-002",
            format!("literal node `{}` requires `value`", node.id),
        ));
    }
    if let Some(entry) = node.config.iter().find(|entry| entry.key != "value") {
        return Err(ResolutionError::new(
            "CND-SRC-002",
            format!(
                "literal node `{}` has unknown field `{}`",
                node.id, entry.key
            ),
        ));
    }
    Ok(())
}

fn validate_format(node: &Node) -> Result<(), ResolutionError> {
    validate_empty_config(node)
}

fn source_usize(node: &Node, key: &str) -> Result<usize, RuntimeError> {
    let value = match node.config_value(key) {
        Some(SourceValue::Integer(value)) => *value,
        _ => {
            return Err(RuntimeError::new(
                "CND-RUN-004",
                format!("node `{}` has no exact integer `{key}`", node.id),
            ));
        }
    };
    usize::try_from(value).map_err(|_| {
        RuntimeError::new(
            "CND-RUN-004",
            format!("node `{}` has out-of-range `{key}`", node.id),
        )
    })
}

fn planned_input_cord(
    plan: &ExecutionPlan<'_>,
    instance: conduit_core::InstancePath<'_>,
    port: &str,
) -> Result<usize, RuntimeError> {
    plan.cords
        .iter()
        .position(|cord| cord.to.node == instance && cord.to.port.as_str() == port)
        .ok_or_else(|| {
            RuntimeError::new(
                "CND-RUN-009",
                format!("planned input `{}.{port}` is absent", instance.as_str()),
            )
        })
}

fn validate_bounded_integer(
    node: &Node,
    key: &str,
    minimum: usize,
    maximum: usize,
) -> Result<(), ResolutionError> {
    let value = source_usize(node, key)
        .map_err(|error| ResolutionError::new("CND-SRC-002", error.message))?;
    if !(minimum..=maximum).contains(&value) {
        return Err(ResolutionError::new(
            "CND-SRC-002",
            format!("node `{}` has out-of-range `{key}`", node.id),
        ));
    }
    Ok(())
}

fn validate_lines(node: &Node) -> Result<(), ResolutionError> {
    for entry in &node.config {
        if !["maximum_line_bytes", "maximum_retained_prefix_bytes"].contains(&entry.key.as_str()) {
            return Err(ResolutionError::new(
                "CND-SRC-002",
                format!("lines node `{}` has unknown field `{}`", node.id, entry.key),
            ));
        }
    }
    validate_bounded_integer(
        node,
        "maximum_line_bytes",
        1,
        conduit_std::LINES_MAX_LINE_BYTES,
    )?;
    validate_bounded_integer(
        node,
        "maximum_retained_prefix_bytes",
        1,
        conduit_std::LINES_MAX_RETAINED_PREFIX_BYTES,
    )
}

fn validate_join(node: &Node) -> Result<(), ResolutionError> {
    for entry in &node.config {
        if ![
            "separator",
            "maximum_items",
            "maximum_item_bytes",
            "maximum_separator_bytes",
            "maximum_output_bytes",
        ]
        .contains(&entry.key.as_str())
        {
            return Err(ResolutionError::new(
                "CND-SRC-002",
                format!("join node `{}` has unknown field `{}`", node.id, entry.key),
            ));
        }
    }
    let separator = node.config("separator").ok_or_else(|| {
        ResolutionError::new(
            "CND-SRC-002",
            format!("join node `{}` requires text `separator`", node.id),
        )
    })?;
    let maximum_separator_bytes = source_usize(node, "maximum_separator_bytes")
        .map_err(|error| ResolutionError::new("CND-SRC-002", error.message))?;
    if separator.len() > maximum_separator_bytes
        || maximum_separator_bytes > conduit_std::JOIN_MAX_SEPARATOR_BYTES
    {
        return Err(ResolutionError::new(
            "CND-SRC-002",
            format!("join node `{}` has an oversized separator", node.id),
        ));
    }
    validate_bounded_integer(node, "maximum_items", 1, conduit_std::JOIN_MAX_ITEMS)?;
    validate_bounded_integer(
        node,
        "maximum_item_bytes",
        1,
        conduit_std::JOIN_MAX_ITEM_BYTES,
    )?;
    validate_bounded_integer(
        node,
        "maximum_output_bytes",
        1,
        conduit_std::JOIN_MAX_OUTPUT_BYTES,
    )
}

fn validate_format_values_literal(node: &Node) -> Result<(), ResolutionError> {
    if let Some(entry) = node.config.iter().find(|entry| entry.key != "values") {
        return Err(ResolutionError::new(
            "CND-SRC-002",
            format!(
                "format-values literal `{}` has unknown field `{}`",
                node.id, entry.key
            ),
        ));
    }
    let values = source_format_values(node).map_err(|error| {
        ResolutionError::new(
            "CND-SRC-002",
            format!(
                "format-values literal `{}` failed {}",
                node.id,
                error.code()
            ),
        )
    })?;
    encode_format_values(&values).map(|_| ()).map_err(|error| {
        ResolutionError::new(
            "CND-SRC-002",
            format!(
                "format-values literal `{}` failed {}",
                node.id,
                error.code()
            ),
        )
    })
}

fn source_format_values(
    node: &Node,
) -> Result<Vec<conduit_std::FormatValueRef<'_>>, conduit_std::FormatError> {
    let values = node
        .config
        .iter()
        .find(|entry| entry.key == "values")
        .and_then(|entry| match &entry.value {
            conduit_panel::SourceValue::List(values) => Some(values.as_slice()),
            _ => None,
        })
        .ok_or(conduit_std::FormatError::InvalidValuesEncoding)?;
    let values = values
        .iter()
        .map(source_format_value)
        .collect::<Result<Vec<_>, _>>()?;
    conduit_std::validate_format_values(&values)?;
    Ok(values)
}

fn source_format_value(
    value: &conduit_panel::SourceValue,
) -> Result<conduit_std::FormatValueRef<'_>, conduit_std::FormatError> {
    use conduit_panel::SourceValue;

    if let SourceValue::Record(fields) = value {
        if fields.len() != 2 {
            return Err(conduit_std::FormatError::UnsupportedValueKind);
        }
        let name = fields
            .iter()
            .find(|(key, _)| key == "name")
            .and_then(|(_, value)| match value {
                SourceValue::Text(value) => Some(value.as_str()),
                _ => None,
            })
            .ok_or(conduit_std::FormatError::InvalidName)?;
        let scalar = fields
            .iter()
            .find(|(key, _)| key == "value")
            .map(|(_, value)| source_format_scalar(value))
            .ok_or(conduit_std::FormatError::UnsupportedValueKind)??;
        return Ok(conduit_std::FormatValueRef {
            name: Some(name),
            value: scalar,
        });
    }
    Ok(conduit_std::FormatValueRef {
        name: None,
        value: source_format_scalar(value)?,
    })
}

fn source_format_scalar(
    value: &conduit_panel::SourceValue,
) -> Result<conduit_std::FormatScalarRef<'_>, conduit_std::FormatError> {
    match value {
        conduit_panel::SourceValue::Text(value) => Ok(conduit_std::FormatScalarRef::Text(value)),
        conduit_panel::SourceValue::Boolean(value) => {
            Ok(conduit_std::FormatScalarRef::Boolean(*value))
        }
        conduit_panel::SourceValue::Integer(value) => {
            Ok(conduit_std::FormatScalarRef::Integer(*value))
        }
        _ => Err(conduit_std::FormatError::UnsupportedValueKind),
    }
}

const FORMAT_VALUES_MAGIC: &[u8; 4] = b"CFV\x01";

fn encode_format_values(
    values: &[conduit_std::FormatValueRef<'_>],
) -> Result<Vec<u8>, conduit_std::FormatError> {
    conduit_std::validate_format_values(values)?;
    let mut encoded = Vec::with_capacity(conduit_std::FORMAT_VALUES_MAX_ENCODED_BYTES);
    encoded.extend_from_slice(FORMAT_VALUES_MAGIC);
    encoded.push(u8::try_from(values.len()).map_err(|_| conduit_std::FormatError::TooManyValues)?);
    for value in values {
        let name = value.name.unwrap_or("").as_bytes();
        encoded.push(u8::try_from(name.len()).map_err(|_| conduit_std::FormatError::NameTooLarge)?);
        encoded.extend_from_slice(name);
        match value.value {
            conduit_std::FormatScalarRef::Text(text) => {
                encoded.push(1);
                encoded.extend_from_slice(
                    &u16::try_from(text.len())
                        .map_err(|_| conduit_std::FormatError::ScalarTooLarge)?
                        .to_be_bytes(),
                );
                encoded.extend_from_slice(text.as_bytes());
            }
            conduit_std::FormatScalarRef::Boolean(value) => {
                encoded.extend_from_slice(&[2, u8::from(value)]);
            }
            conduit_std::FormatScalarRef::Integer(value) => {
                encoded.push(3);
                encoded.extend_from_slice(&value.to_be_bytes());
            }
            conduit_std::FormatScalarRef::Unsupported(_) => {
                return Err(conduit_std::FormatError::UnsupportedValueKind);
            }
        }
        if encoded.len() > conduit_std::FORMAT_VALUES_MAX_ENCODED_BYTES {
            return Err(conduit_std::FormatError::InvalidValuesEncoding);
        }
    }
    Ok(encoded)
}

fn decode_format_values(
    encoded: &[u8],
) -> Result<Vec<conduit_std::FormatValueRef<'_>>, conduit_std::FormatError> {
    if encoded.len() > conduit_std::FORMAT_VALUES_MAX_ENCODED_BYTES
        || encoded.get(..4) != Some(FORMAT_VALUES_MAGIC.as_slice())
    {
        return Err(conduit_std::FormatError::InvalidValuesEncoding);
    }
    let mut cursor = 4;
    let count = usize::from(
        *encoded
            .get(cursor)
            .ok_or(conduit_std::FormatError::InvalidValuesEncoding)?,
    );
    cursor += 1;
    if count > conduit_std::FORMAT_MAX_VALUES {
        return Err(conduit_std::FormatError::TooManyValues);
    }
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let name_length = usize::from(
            *encoded
                .get(cursor)
                .ok_or(conduit_std::FormatError::InvalidValuesEncoding)?,
        );
        cursor += 1;
        let name_bytes = take_encoded(encoded, &mut cursor, name_length)?;
        let name = if name_bytes.is_empty() {
            None
        } else {
            Some(
                std::str::from_utf8(name_bytes)
                    .map_err(|_| conduit_std::FormatError::InvalidValuesEncoding)?,
            )
        };
        let kind = *encoded
            .get(cursor)
            .ok_or(conduit_std::FormatError::InvalidValuesEncoding)?;
        cursor += 1;
        let value = match kind {
            1 => {
                let length = u16::from_be_bytes(
                    take_encoded(encoded, &mut cursor, 2)?
                        .try_into()
                        .expect("exact two-byte length"),
                );
                let text =
                    std::str::from_utf8(take_encoded(encoded, &mut cursor, usize::from(length))?)
                        .map_err(|_| conduit_std::FormatError::InvalidTextEncoding)?;
                conduit_std::FormatScalarRef::Text(text)
            }
            2 => match *encoded
                .get(cursor)
                .ok_or(conduit_std::FormatError::InvalidValuesEncoding)?
            {
                0 => {
                    cursor += 1;
                    conduit_std::FormatScalarRef::Boolean(false)
                }
                1 => {
                    cursor += 1;
                    conduit_std::FormatScalarRef::Boolean(true)
                }
                _ => return Err(conduit_std::FormatError::InvalidValuesEncoding),
            },
            3 => {
                let value = i128::from_be_bytes(
                    take_encoded(encoded, &mut cursor, 16)?
                        .try_into()
                        .expect("exact sixteen-byte integer"),
                );
                conduit_std::FormatScalarRef::Integer(value)
            }
            other => conduit_std::FormatScalarRef::Unsupported(other),
        };
        values.push(conduit_std::FormatValueRef { name, value });
    }
    if cursor != encoded.len() {
        return Err(conduit_std::FormatError::InvalidValuesEncoding);
    }
    conduit_std::validate_format_values(&values)?;
    Ok(values)
}

fn take_encoded<'a>(
    encoded: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<&'a [u8], conduit_std::FormatError> {
    let end = cursor
        .checked_add(length)
        .ok_or(conduit_std::FormatError::InvalidValuesEncoding)?;
    let value = encoded
        .get(*cursor..end)
        .ok_or(conduit_std::FormatError::InvalidValuesEncoding)?;
    *cursor = end;
    Ok(value)
}

fn format_input_bytes(
    template: &[u8],
    encoded_values: &[u8],
) -> Result<Vec<u8>, conduit_std::FormatError> {
    let template =
        std::str::from_utf8(template).map_err(|_| conduit_std::FormatError::InvalidTextEncoding)?;
    let values = decode_format_values(encoded_values)?;
    let mut output = vec![0; conduit_std::FORMAT_MAX_OUTPUT_BYTES];
    let length = conduit_std::format_text_into(template, &values, &mut output)?;
    output.truncate(length);
    Ok(output)
}

fn format_runtime_error(error: conduit_std::FormatError) -> RuntimeError {
    RuntimeError::new(error.code(), error.code())
}

struct Literal;

impl Handler for Literal {
    fn run(
        &mut self,
        node: &Node,
        _inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let value = node
            .config("value")
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "literal value disappeared"))?;
        Ok(vec![Value::text(value.as_bytes())])
    }
}

struct FormatValuesLiteral;

impl Handler for FormatValuesLiteral {
    fn run(
        &mut self,
        node: &Node,
        _inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let values = source_format_values(node).map_err(format_runtime_error)?;
        let bytes = encode_format_values(&values).map_err(format_runtime_error)?;
        Ok(vec![Value {
            value_type: FORMAT_VALUES_TYPE,
            bytes,
        }])
    }
}

struct Format;

impl Handler for Format {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !node.config.is_empty() {
            return Err(RuntimeError::new(
                "CND-RUN-004",
                "format configuration changed after resolution",
            ));
        }
        let template = inputs
            .first()
            .filter(|value| value.value_type == TEXT_TYPE)
            .ok_or_else(|| RuntimeError::new("format/missing-value", "template input is absent"))?;
        let values = inputs
            .get(1)
            .filter(|value| value.value_type == FORMAT_VALUES_TYPE)
            .ok_or_else(|| RuntimeError::new("format/missing-value", "values input is absent"))?;
        Ok(vec![Value::text(
            format_input_bytes(&template.bytes, &values.bytes).map_err(format_runtime_error)?,
        )])
    }
}

fn text_line_runtime_error(error: conduit_std::LineError) -> RuntimeError {
    RuntimeError::new(error.code(), error.code())
}

struct Lines;

impl Handler for Lines {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let mut state = conduit_std::LinesState::new();
        let mut values = Vec::new();
        for input in inputs {
            if input.value_type != TEXT_TYPE {
                return Err(RuntimeError::new(
                    "CND-TXT-002",
                    "lines input is not exact text",
                ));
            }
            for byte in &input.bytes {
                if state.push_byte(*byte).map_err(text_line_runtime_error)? {
                    let mut output = [0; conduit_std::LINES_MAX_LINE_BYTES];
                    let length = state
                        .take_ready(&mut output)
                        .map_err(text_line_runtime_error)?
                        .expect("ready line exists");
                    values.push(Value::text(output[..length].to_vec()));
                }
            }
        }
        if state.finish().map_err(text_line_runtime_error)? {
            let mut output = [0; conduit_std::LINES_MAX_LINE_BYTES];
            let length = state
                .take_ready(&mut output)
                .map_err(text_line_runtime_error)?
                .expect("final line exists");
            values.push(Value::text(output[..length].to_vec()));
        }
        Ok(values)
    }
}

struct Join;

impl Handler for Join {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let separator = node
            .config("separator")
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "join separator disappeared"))?;
        let mut text = Vec::with_capacity(inputs.len());
        for input in inputs {
            if input.value_type != TEXT_TYPE {
                return Err(RuntimeError::new(
                    "CND-TXT-002",
                    "join input is not exact text",
                ));
            }
            text.push(
                std::str::from_utf8(&input.bytes)
                    .map_err(|_| RuntimeError::new("CND-TXT-002", "join input is invalid UTF-8"))?,
            );
        }
        let mut output = vec![0; conduit_std::JOIN_MAX_OUTPUT_BYTES];
        let length = conduit_std::join_text_into(&text, separator, &mut output)
            .map_err(text_line_runtime_error)?;
        output.truncate(length);
        Ok(vec![Value::text(output)])
    }
}

struct Stdin;

impl Handler for Stdin {
    fn run(
        &mut self,
        _node: &Node,
        _inputs: &[Value],
        io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let mut bytes = Vec::new();
        io.input
            .read_to_end(&mut bytes)
            .map_err(|error| RuntimeError::new("CND-RUN-005", error.to_string()))?;
        Ok(vec![Value::bytes(bytes)])
    }
}

struct Uppercase;

impl Handler for Uppercase {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let input = inputs
            .first()
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "uppercase input missing"))?;
        let text = std::str::from_utf8(&input.bytes)
            .map_err(|error| RuntimeError::new("CND-RUN-005", error.to_string()))?;
        Ok(vec![Value::text(text.to_uppercase().into_bytes())])
    }
}

struct EncodeUtf8;

impl Handler for EncodeUtf8 {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let input = inputs
            .first()
            .filter(|value| value.value_type == TEXT_TYPE)
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "UTF-8 encoder text input missing"))?;
        std::str::from_utf8(&input.bytes)
            .map_err(|error| RuntimeError::new("CND-RUN-005", error.to_string()))?;
        Ok(vec![Value::bytes(input.bytes.clone())])
    }
}

struct Stdout;

impl Handler for Stdout {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let input = inputs
            .first()
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "stdout input missing"))?;
        io.output
            .write_all(&input.bytes)
            .map_err(|error| RuntimeError::new("CND-RUN-005", error.to_string()))?;
        Ok(Vec::new())
    }
}

struct Stderr;

impl Handler for Stderr {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let input = inputs
            .first()
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "stderr input missing"))?;
        io.error
            .write_all(&input.bytes)
            .map_err(|error| RuntimeError::new("CND-RUN-005", error.to_string()))?;
        Ok(Vec::new())
    }
}

struct Supervisor;

impl Handler for Supervisor {
    fn run(
        &mut self,
        _node: &Node,
        _inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        Err(RuntimeError::new(
            "CND-SUP-015",
            "typed supervisors run through the bounded supervision scheduler, not the legacy one-shot executor",
        ))
    }
}

struct PassThroughHandler;
impl Handler for PassThroughHandler {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let input = inputs.first().cloned().unwrap_or_else(|| Value::text(""));
        Ok(vec![input])
    }
}

struct TeeHandler;
impl Handler for TeeHandler {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let input = inputs.first().cloned().unwrap_or_else(|| Value::text(""));
        Ok(vec![input.clone(), input])
    }
}

struct MergeHandler;
impl Handler for MergeHandler {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let val = inputs
            .first()
            .or_else(|| inputs.get(1))
            .cloned()
            .unwrap_or_else(|| Value::text(""));
        Ok(vec![val])
    }
}

struct ZipHandler;
impl Handler for ZipHandler {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let left = inputs.first().cloned().ok_or_else(|| {
            RuntimeError::new("conduit.std/zip-unpaired", "zip left input is absent")
        })?;
        let right = inputs.get(1).cloned().ok_or_else(|| {
            RuntimeError::new("conduit.std/zip-unpaired", "zip right input is absent")
        })?;
        Ok(vec![left, right])
    }
}

struct GateHandler;
impl Handler for GateHandler {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let open = inputs
            .get(1)
            .map(|control| control.bytes.as_slice() == b"open")
            .unwrap_or_else(|| node.config("initial") == Some("open"));
        Ok(if open {
            inputs.first().cloned().into_iter().collect()
        } else {
            Vec::new()
        })
    }
}

struct SelectHandler;
impl Handler for SelectHandler {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let selected = inputs
            .get(2)
            .and_then(|control| match control.bytes.as_slice() {
                b"left" => Some(0),
                b"right" => Some(1),
                _ => None,
            })
            .unwrap_or_else(|| usize::from(node.config("initial") == Some("right")));
        Ok(inputs.get(selected).cloned().into_iter().collect())
    }
}

struct FallbackHandler;
impl Handler for FallbackHandler {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if let Some(val) = inputs.first() {
            if !val.bytes.is_empty() {
                return Ok(vec![val.clone()]);
            }
        }
        let fallback = inputs.get(1).cloned().unwrap_or_else(|| Value::text(""));
        Ok(vec![fallback])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_panel::parse;

    #[test]
    fn format_uses_typed_inputs_named_indexed_and_escaped_placeholders() {
        let panel = parse(
            r#"
                panel 1
                node template : std/literal {
                    value = "{worker} = {{status: {1}; count={2}}}"
                }
                node values : std/format-values/literal {
                    values = list(
                        record(name="worker", value="alpha"),
                        record(name="ready", value=true),
                        record(name="count", value=-7)
                    )
                }
                node message : std/text/format
                node encoded : text/encode-utf8
                node output : io/stdout
                cord template.value -> message.template
                cord values.values -> message.values
                cord message.text -> encoded.text
                cord encoded.bytes -> output.bytes
            "#,
        )
        .unwrap();
        let registry = Registry::compatibility_demo();
        let resolved = registry.resolve(&panel).unwrap();
        let mut input = &b""[..];
        let mut output = Vec::new();
        let mut error = Vec::new();
        resolved
            .run_batch(&mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
            })
            .unwrap();
        assert_eq!(output, b"alpha = {status: true; count=-7}");
    }

    #[test]
    fn format_rejects_missing_and_extra_values_at_execution() {
        let panel = parse(
            r#"
                panel 1
                node template : std/literal {
                    value = "{} {}"
                }
                node values : std/format-values/literal {
                    values = list("only-one")
                }
                node message : std/text/format
                node encoded : text/encode-utf8
                node output : io/stdout
                cord template.value -> message.template
                cord values.values -> message.values
                cord message.text -> encoded.text
                cord encoded.bytes -> output.bytes
            "#,
        )
        .unwrap();
        let registry = Registry::compatibility_demo();
        let resolved = registry.resolve(&panel).unwrap();
        let mut input = &b""[..];
        let mut output = Vec::new();
        let mut error = Vec::new();
        let failure = resolved
            .run_batch(&mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
            })
            .unwrap_err();
        assert_eq!(failure.code, "format/missing-value");
    }

    #[test]
    fn hosted_format_codec_matches_portable_success_and_failure_outcomes() {
        let values = [
            conduit_std::FormatValueRef {
                name: Some("text"),
                value: conduit_std::FormatScalarRef::Text("ready"),
            },
            conduit_std::FormatValueRef {
                name: Some("flag"),
                value: conduit_std::FormatScalarRef::Boolean(true),
            },
            conduit_std::FormatValueRef {
                name: Some("count"),
                value: conduit_std::FormatScalarRef::Integer(i128::MAX),
            },
        ];
        let encoded = encode_format_values(&values).unwrap();
        let template = b"{text}:{flag}:{count}";
        let hosted = format_input_bytes(template, &encoded).unwrap();
        let mut portable = [0; conduit_std::FORMAT_MAX_OUTPUT_BYTES];
        let length = conduit_std::format_text_into(
            std::str::from_utf8(template).unwrap(),
            &values,
            &mut portable,
        )
        .unwrap();
        assert_eq!(hosted, portable[..length]);

        let mut unsupported = encode_format_values(&[conduit_std::FormatValueRef {
            name: None,
            value: conduit_std::FormatScalarRef::Text("future"),
        }])
        .unwrap();
        unsupported[6] = 0xff;
        unsupported.truncate(7);
        assert_eq!(
            format_input_bytes(b"{0}", &unsupported),
            Err(conduit_std::FormatError::UnsupportedValueKind)
        );
        assert_eq!(
            conduit_std::format_text_into(
                "{0}",
                &[conduit_std::FormatValueRef {
                    name: None,
                    value: conduit_std::FormatScalarRef::Unsupported(0xff),
                }],
                &mut portable,
            ),
            Err(conduit_std::FormatError::UnsupportedValueKind)
        );
    }

    #[test]
    fn resolves_explains_and_runs_a_panel() {
        let panel = parse(
            r#"
                panel 1
                node greeting : std/literal {
                    value = "Hello from Conduit.\n"
                }
                node shout : text/uppercase
                node encoded : text/encode-utf8
                node output : io/stdout
                cord greeting.value -> shout.text
                cord shout.text -> encoded.text
                cord encoded.bytes -> output.bytes
            "#,
        )
        .expect("panel parses");
        let registry = Registry::compatibility_demo();
        let resolved = registry.resolve(&panel).expect("panel resolves");
        let explanation = resolved.explain();
        assert!(explanation.contains("capacity=8"));
        assert!(explanation.contains("max_value_bytes=65536"));
        assert!(explanation.contains("watermarks=7..8"));
        assert!(explanation.contains("pressure=block(fifo)"));

        let mut input = &b""[..];
        let mut output = Vec::new();
        let mut error = Vec::new();
        let summary = resolved
            .run_batch(&mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
            })
            .expect("panel runs");

        assert_eq!(output, b"HELLO FROM CONDUIT.\n");
        assert!(error.is_empty());
        assert_eq!(summary.nodes_completed, 4);
        assert_eq!(summary.cords_conducted, 3);
    }

    #[test]
    fn rejects_unknown_implementations() {
        let panel = parse("panel 1\nnode mystery : example/missing").expect("panel parses");
        let error = Registry::compatibility_demo()
            .resolve(&panel)
            .expect_err("missing implementation");
        assert_eq!(error.code, "CND-IMP-001");
    }

    #[test]
    fn source_only_module_group_and_pool_forms_require_explicit_lowering() {
        for source in [
            "panel 1\nimport \"./child.panel\" as child",
            "panel 1\nport-group routes input : fixture/request indexed max 8",
            "panel 1\npool sessions : fixture/handler { maximum = 8 admission = reject deadline_ms = 1000 idle_timeout_ms = 5000 supervision = isolate cleanup = abort }",
            "panel 1\nnode app { node child : std/literal }\nroot app",
            "panel 1\nnode source : std/literal using ready",
        ] {
            let panel = parse(source).expect("source form parses");
            let error = Registry::compatibility_demo()
                .resolve(&panel)
                .expect_err("source-only construct must not be ignored");
            assert_eq!(error.code, "CND-PLN-005");
        }
    }

    #[test]
    fn rejects_loss_and_missing_type_traits_before_execution() {
        let sample = parse(
            "panel 1\nnode a : io/stdin\nnode b : io/stdout\n\
             cord a.bytes -> b.bytes {\n\
               pressure = sample\n\
               sample_every = 2\n\
             }",
        )
        .unwrap();
        let error = Registry::compatibility_demo()
            .resolve(&sample)
            .expect_err("lossless ports reject sampling");
        assert_eq!(error.code, "CND-FLW-002");

        let coalesce = parse(
            "panel 1\nnode a : io/stdin\nnode b : io/stdout\n\
             cord a.bytes -> b.bytes {\n\
               pressure = coalesce\n\
               coalescer = conduit/replace-latest\n\
             }",
        )
        .unwrap();
        let error = Registry::compatibility_demo()
            .resolve(&coalesce)
            .expect_err("text type does not declare coalescing");
        assert_eq!(error.code, "CND-FLW-004");
        assert_eq!(error.message, "coalescing-relation-unavailable");
    }

    #[test]
    fn stdin_is_an_explicit_source_node() {
        let panel = parse(
            r#"
                panel 1
                node input : io/stdin
                node output : io/stdout
                cord input.bytes -> output.bytes
            "#,
        )
        .expect("panel parses");
        let registry = Registry::compatibility_demo();
        let resolved = registry.resolve(&panel).expect("panel resolves");
        let mut input = &b"pipe friendly"[..];
        let mut output = Vec::new();
        let mut error = Vec::new();

        resolved
            .run_batch(&mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
            })
            .expect("panel runs");

        assert_eq!(output, b"pipe friendly");
    }

    #[test]
    fn nested_composites_bind_parameters_export_ports_and_preserve_views() {
        let panel = parse(
            r#"
                panel 1
                composite example/literal-line {
                    node source : std/literal
                    export output text = source.value
                    bind value = source.value
                }
                composite example/upper-line {
                    node source : example/literal-line
                    node upper : text/uppercase
                    cord source.text -> upper.text
                    export output text = upper.text
                    bind value = source.value
                }
                node line : example/upper-line { value = "mixed Case" }
                node encoded : text/encode-utf8
                node stdout : io/stdout
                node stderr : io/stderr
                cord line.text -> encoded.text
                cord encoded.bytes -> stdout.bytes
                cord encoded.bytes -> stderr.bytes
            "#,
        )
        .expect("nested composite parses");
        let registry = Registry::compatibility_demo();
        let resolved = registry.resolve(&panel).expect("composite resolves");
        let logical = resolved.explain_logical();
        let expanded = resolved.explain_expanded();
        assert!(logical.contains("composite line : example/upper-line"));
        assert!(logical.contains("composite line/source : example/literal-line"));
        assert!(logical.contains("child line/upper : text/uppercase"));
        assert!(logical.contains("export output text -> line.upper.text"));
        assert!(logical.contains("bind value -> line/source.value"));
        assert!(expanded.contains("line.source.source : std/literal"));
        assert!(expanded.contains("line.upper : text/uppercase"));
        assert!(!expanded.contains("example/upper-line -> hosted builtin"));

        let mut input = &b""[..];
        let mut output = Vec::new();
        let mut error = Vec::new();
        let summary = resolved
            .run_batch(&mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
            })
            .expect("flattened composite runs");
        assert_eq!(summary.nodes_completed, 5);
        assert_eq!(output, b"MIXED CASE");
        assert_eq!(error, b"MIXED CASE");
    }

    #[test]
    fn composite_boundary_is_substitutable_for_primitive_inputs_and_outputs() {
        let panel = parse(
            r#"
                panel 1
                composite example/uppercase {
                    node worker : text/uppercase
                    export input text = worker.text
                    export output text = worker.text
                }
                node source : std/literal { value = "boundary" }
                node transform : example/uppercase
                node encoded : text/encode-utf8
                node sink : io/stdout
                cord source.value -> transform.text
                cord transform.text -> encoded.text
                cord encoded.bytes -> sink.bytes
            "#,
        )
        .expect("transparent composite parses");
        let registry = Registry::compatibility_demo();
        let resolved = registry
            .resolve(&panel)
            .expect("transparent boundary resolves");
        let mut input = &b""[..];
        let mut output = Vec::new();
        let mut error = Vec::new();
        resolved
            .run_batch(&mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
            })
            .expect("same primitive implementation runs");
        assert_eq!(output, b"BOUNDARY");
    }

    #[test]
    fn contract_only_http_service_is_not_executable() {
        let panel = parse(
            "panel 1\n\
             node server : net/http/serve-once {\n\
               listen = \"127.0.0.1:0\"\n\
               method = \"GET\"\n\
               path = \"/health\"\n\
               response = \"ok\"\n\
               deadline_ms = \"1000\"\n\
             }",
        )
        .expect("HTTP contract source parses");
        let registry = Registry::hosted_primitives();

        registry
            .resolve_contracts(&panel)
            .expect("compiler may inspect contract-only topology");
        let error = registry
            .resolve(&panel)
            .expect_err("execution must require an installed provider");
        assert_eq!(error.code, "CND-IMP-001");
    }

    #[test]
    fn rejects_recursive_duplicate_dangling_and_boundary_bypass() {
        let registry = Registry::compatibility_demo();
        for (source, source_code, runtime_code) in [
            (
                "panel 1\ncomposite example/a { node b : example/b }\n\
                 composite example/b { node a : example/a }\n\
                 node root : example/a",
                None,
                Some("CND-CMP-005"),
            ),
            (
                "panel 1\ncomposite example/a {\n\
                   node source : io/stdin\n\
                   export output out = source.out\n\
                   export output out = source.out\n\
                 }\nnode root : example/a",
                Some("CND-SRC-002"),
                None,
            ),
            (
                "panel 1\ncomposite example/a {\n\
                   node source : io/stdin\n\
                   export output out = missing.out\n\
                 }\nnode root : example/a",
                Some("CND-SRC-009"),
                None,
            ),
            (
                "panel 1\ncomposite example/a {\n\
                   node source : io/stdin\n\
                   export input in = source.out\n\
                 }\nnode root : example/a",
                None,
                Some("CND-CMP-003"),
            ),
            (
                "panel 1\ncomposite example/a {\n\
                   node source : std/literal\n\
                   export output out = source.out\n\
                   bind value = source.missing\n\
                 }\nnode root : example/a { value = x }",
                None,
                Some("CND-CMP-003"),
            ),
            (
                "panel 1\ncomposite example/a {\n\
                   node source : io/stdin\n\
                   export output out = source.out\n\
                 }\nnode root : example/a\nnode sink : io/stdout\n\
                 cord root.source.out -> sink.in",
                Some("CND-SRC-009"),
                None,
            ),
        ] {
            match parse(source) {
                Err(error) => {
                    assert_eq!(Some(error.code), source_code, "{}", error.message);
                }
                Ok(panel) => {
                    assert!(source_code.is_none(), "expected source rejection");
                    let error = registry.resolve(&panel).expect_err("must reject");
                    assert_eq!(Some(error.code), runtime_code, "{}", error.message);
                }
            }
        }
    }
}
