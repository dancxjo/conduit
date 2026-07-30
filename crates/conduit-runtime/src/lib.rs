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
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

mod artifact_verification;
mod config_resolution;
mod distributed;
mod evidence_ndjson;
mod exact_evidence;
mod host_resolution;
mod implementation_binding;
mod pool;
mod runtime_evidence;
mod scheduler;
mod source_lowering;
mod supervision;
mod transition;
mod transport;
mod type_registry;

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
pub use runtime_evidence::{
    RuntimeEvidenceContext, RuntimeEvidenceError, record_scheduler_evidence,
};
pub use scheduler::{
    DeterministicExecutor, RuntimeValue, ScheduledNode, SchedulerAllocation, SchedulerError,
    SchedulerEvent, SchedulerEventKind, SchedulerHighWater, SchedulerNode, SchedulerReservation,
    SchedulerStatus, SchedulerStep, SchedulerSubject, SendStatus, StepIo,
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
    SourceContractCatalog, SourceMapEntry, SourceOrigin, VersionedLoweredSource, lower_source,
    lower_source_v2, lower_source_v3, lower_source_v4, lower_source_version,
    migrate_lowered_source_v1,
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
    DISTRIBUTED_ENVELOPE_VERSION, DecodedDistributedEnvelope, ResolvedTransportSelection,
    TransportCapabilities, TransportReason, TransportTransition, decode_distributed_envelope,
    encode_distributed_envelope, validate_transport_selection, validate_transport_transition,
};
pub use type_registry::{
    ProviderTypeDecision, TypeComparisonStrategy, TypeContractDescription, TypeContractProvider,
    TypeRegistry, TypeRegistryError, TypeSatisfactionReport,
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
    conduit_core::validate_execution_plan(plan, context, &mut scratch)
}

const TEXT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit/text.utf8"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([
        0x23, 0xf6, 0xb8, 0xc6, 0xd7, 0x84, 0x79, 0x9a, 0x10, 0x09, 0xbd, 0x45, 0x32, 0x26, 0x67,
        0x0d, 0xdd, 0x91, 0x80, 0xe0, 0x06, 0xd4, 0xc2, 0x32, 0x70, 0x55, 0xcb, 0xf3, 0x50, 0x77,
        0x6e, 0x9b,
    ]),
};
const TEXT_LIST_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit/text-list"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([0x6f; 32]),
};
const TERMINAL_OBSERVATION_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit/terminal-observation"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([
        0xd3, 0x21, 0xc6, 0xa0, 0x12, 0xe8, 0x1f, 0x84, 0xc4, 0x6a, 0x6f, 0xd6, 0x23, 0x11, 0xdc,
        0x81, 0x37, 0x46, 0x0f, 0x92, 0x85, 0x68, 0x6b, 0x68, 0x45, 0x9d, 0xc1, 0xb6, 0x45, 0x54,
        0x5b, 0x58,
    ]),
};
const SUPERVISION_DECISION_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit/supervision-decision"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([
        0x30, 0xc7, 0x67, 0x8a, 0x03, 0x31, 0xd9, 0xbb, 0x2d, 0x03, 0x38, 0x9d, 0xda, 0xe0, 0xb5,
        0x0d, 0x62, 0xf6, 0x6e, 0x2a, 0xe3, 0x45, 0xe2, 0x32, 0x57, 0x9e, 0x2e, 0xad, 0xfd, 0xff,
        0x7e, 0xee,
    ]),
};
const EMPTY_CONFIG: ConfigContract<'static> = ConfigContract { fields: &[] };
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
const FORMAT_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        ConfigFieldContract {
            key: Id("template"),
            value_type: TEXT_TYPE,
            requirement: ConfigRequirement::Required,
            sensitivity: Sensitivity::Public,
            mutability: ConfigMutability::PreStart,
            identity: ConfigIdentity::Semantic,
        },
        ConfigFieldContract {
            key: Id("parameters"),
            value_type: TEXT_LIST_TYPE,
            requirement: ConfigRequirement::Required,
            sensitivity: Sensitivity::Public,
            mutability: ConfigMutability::PreStart,
            identity: ConfigIdentity::Semantic,
        },
    ],
};
const INPUT_TEXT: PortContract<'static> = PortContract {
    id: Id("in"),
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
const OUTPUT_TEXT: PortContract<'static> = PortContract {
    id: Id("out"),
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

const INPUT_TEXT_1: PortContract<'static> = PortContract {
    id: Id("in1"),
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
const INPUT_TEXT_2: PortContract<'static> = PortContract {
    id: Id("in2"),
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
const OUTPUT_TEXT_1: PortContract<'static> = PortContract {
    id: Id("out1"),
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
const OUTPUT_TEXT_2: PortContract<'static> = PortContract {
    id: Id("out2"),
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
    id: Id("conduit.std/literal"),
    config: LITERAL_CONFIG,
    inputs: &[],
    outputs: &[OUTPUT_TEXT],
};
pub const STDIN_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/stdin"),
    config: EMPTY_CONFIG,
    inputs: &[],
    outputs: &[OUTPUT_TEXT],
};
pub const UPPERCASE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/uppercase"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const FORMAT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/format"),
    config: FORMAT_CONFIG,
    inputs: &[],
    outputs: &[OUTPUT_TEXT],
};
pub const STDOUT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/stdout"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[],
};
pub const STDERR_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/stderr"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[],
};
pub const SUPERVISOR_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/supervisor"),
    config: EMPTY_CONFIG,
    inputs: &[TERMINAL_OBSERVATION_INPUT],
    outputs: &[SUPERVISION_DECISION_OUTPUT],
};
pub const PASS_THROUGH_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/pass-through"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const TEE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/tee"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT_1, OUTPUT_TEXT_2],
};
pub const MERGE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/merge"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT_1, INPUT_TEXT_2],
    outputs: &[OUTPUT_TEXT],
};
pub const DELAY_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/delay"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const DEBOUNCE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/debounce"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const THROTTLE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/throttle"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const TAKE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/take"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const SKIP_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/skip"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const FILTER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/filter"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const FALLBACK_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/fallback"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_PRIMARY, INPUT_FALLBACK],
    outputs: &[OUTPUT_TEXT],
};
pub const PROBE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/probe"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const LOG_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/log"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const ASSERT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/assert"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const RECORD_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/record"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const REPLAY_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/replay"),
    config: EMPTY_CONFIG,
    inputs: &[],
    outputs: &[OUTPUT_TEXT],
};
pub const FAULT_SOURCE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/fault-source"),
    config: EMPTY_CONFIG,
    inputs: &[],
    outputs: &[OUTPUT_TEXT],
};
pub const FILE_READ_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/file-read"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const FILE_WRITE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/file-write"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[],
};
pub const BLOB_STORE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/blob-store"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const KV_STORE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/kv-store"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const PROCESS_SPAWN_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/process-spawn"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const GPIO_PIN_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/gpio-pin"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const SERIAL_PORT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/serial-port"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const CELL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/cell"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const COUNTER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/counter"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const DEDUPLICATE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/deduplicate"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const CACHE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/cache"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const CIRCUIT_BREAKER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/circuit-breaker"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const HEALTH_GATE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/health-gate"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const BACKOFF_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/backoff"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const WIFI_STATION_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/wifi-station"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const WIFI_AP_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/wifi-ap"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const NETWORK_INTERFACE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/network-interface"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const TCP_SOCKET_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/tcp-socket"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const UDP_SOCKET_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/udp-socket"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
pub const DNS_RESOLVER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/dns-resolver"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
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
    Format,
    Stdin,
    Uppercase,
    Stdout,
    Stderr,
    PassThrough,
    Tee,
    Merge,
    Fallback,
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
    pub fn register_interface(&mut self, interface: OwnedInterfaceContract) {
        self.interfaces.insert(interface.id.clone(), interface);
    }

    /// Resolves a contract ID to its canonical semantic ID.
    pub fn resolve_canonical_id<'a>(&'a self, contract_id: &str) -> Result<&'a str, RegistryError> {
        if let Some((canonical, _)) = self.nodes.get_key_value(contract_id) {
            return Ok(canonical);
        }
        if let Some(suffix) = contract_id.strip_prefix("std/")
            && let Some((canonical, _)) = self
                .nodes
                .iter()
                .find(|(id, _)| id.strip_prefix("conduit.std/") == Some(suffix))
        {
            return Ok(canonical);
        }
        Err(RegistryError {
            code: "CND-REG-003",
            message: format!("unknown contract id `{}`", contract_id),
        })
    }

    fn get_registered_node(&self, contract_id: &str) -> Option<&RegisteredNode> {
        self.nodes.get(contract_id).or_else(|| {
            let suffix = contract_id.strip_prefix("std/")?;
            self.nodes
                .iter()
                .find(|(id, _)| id.strip_prefix("conduit.std/") == Some(suffix))
                .map(|(_, node)| node)
        })
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
        install(&FORMAT_CONTRACT, || Box::new(Format), validate_format);
        install(&STDIN_CONTRACT, || Box::new(Stdin), validate_empty_config);
        install(
            &UPPERCASE_CONTRACT,
            || Box::new(Uppercase),
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
        install(
            &TEE_CONTRACT,
            || Box::new(TeeHandler),
            validate_empty_config,
        );
        install(
            &MERGE_CONTRACT,
            || Box::new(MergeHandler),
            validate_empty_config,
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
                &FORMAT_CONTRACT,
                "format",
                HostedPrimitiveImplementation::Format,
                || Box::new(Format),
                validate_format,
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
                validate_empty_config,
            ),
            (
                &MERGE_CONTRACT,
                "merge",
                HostedPrimitiveImplementation::Merge,
                || Box::new(MergeHandler),
                validate_empty_config,
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
            honest_primitive(
                &TEE_CONTRACT,
                || Box::new(TeeHandler),
                validate_empty_config,
            ),
        );
        nodes.insert(
            MERGE_CONTRACT.id.as_str(),
            honest_primitive(
                &MERGE_CONTRACT,
                || Box::new(MergeHandler),
                validate_empty_config,
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

        let mut types = TypeRegistry::default();
        types
            .register(BuiltinTypeProvider)
            .expect("built-in type namespace is unique and valid");

        let mut interfaces = BTreeMap::new();
        let stream_sink_member = OwnedInterfaceMember {
            requirement: conduit_core::InterfaceMemberRequirement::Required,
            id: "in".to_owned(),
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
            id: "in".to_owned(),
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
            id: "out".to_owned(),
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
                .ok_or_else(|| {
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
        match id {
            value if value == TEXT_TYPE.contract_id.as_str() => Some(TEXT_TYPE.into()),
            value if value == TEXT_LIST_TYPE.contract_id.as_str() => Some(TEXT_LIST_TYPE.into()),
            value if value == TERMINAL_OBSERVATION_TYPE.contract_id.as_str() => {
                Some(TERMINAL_OBSERVATION_TYPE.into())
            }
            value if value == SUPERVISION_DECISION_TYPE.contract_id.as_str() => {
                Some(SUPERVISION_DECISION_TYPE.into())
            }
            _ => None,
        }
    }

    fn port_contract(&self, id: &str) -> Option<OwnedPortReference> {
        let contract = match id {
            "conduit/input-text" => Some(&INPUT_TEXT),
            "conduit/output-text" => Some(&OUTPUT_TEXT),
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
        if expected == &OwnedTypeReference::from(TEXT_TYPE) {
            return match source {
                conduit_panel::SourceValue::Text(value) => {
                    Ok(OwnedSemanticValue::Text(value.clone()))
                }
                _ => Err(LiteralValidationError::WrongKind),
            };
        }
        if expected == &OwnedTypeReference::from(TEXT_LIST_TYPE) {
            return match source {
                conduit_panel::SourceValue::List(values) => values
                    .iter()
                    .map(|value| match value {
                        conduit_panel::SourceValue::Text(value) => {
                            Ok(OwnedSemanticValue::Text(value.clone()))
                        }
                        _ => Err(LiteralValidationError::WrongKind),
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(OwnedSemanticValue::List),
                _ => Err(LiteralValidationError::WrongKind),
            };
        }
        Err(LiteralValidationError::ProviderUnavailable)
    }

    fn validate_default(
        &self,
        expected: &OwnedTypeReference,
        value: &OwnedSemanticValue,
    ) -> Result<(), LiteralValidationError> {
        if expected == &OwnedTypeReference::from(TEXT_TYPE)
            && matches!(value, OwnedSemanticValue::Text(_))
        {
            Ok(())
        } else {
            Err(LiteralValidationError::WrongKind)
        }
    }
}

struct BuiltinTypeProvider;

impl TypeContractProvider for BuiltinTypeProvider {
    fn provider_descriptor(&self) -> DescriptorRef<'static> {
        DescriptorRef {
            kind: Id("conduit/builtin-type-provider"),
            schema_version: 1,
            semantic_hash: SemanticHash::from_bytes([0x24; 32]),
        }
    }

    fn namespace(&self) -> &str {
        "conduit"
    }

    fn describe<'a>(
        &'a self,
        reference: TypeContractRef<'a>,
    ) -> Option<TypeContractDescription<'a>> {
        let (reference, human_name) = if reference == TEXT_TYPE {
            (TEXT_TYPE, "UTF-8 text")
        } else if reference == TEXT_LIST_TYPE {
            (TEXT_LIST_TYPE, "finite list of UTF-8 text")
        } else if reference == TERMINAL_OBSERVATION_TYPE {
            (TERMINAL_OBSERVATION_TYPE, "terminal observation")
        } else if reference == SUPERVISION_DECISION_TYPE {
            (SUPERVISION_DECISION_TYPE, "supervision decision")
        } else {
            return None;
        };
        Some(TypeContractDescription {
            human_name,
            descriptor: CanonicalDescriptor {
                kind: reference.contract_id,
                schema_version: reference.schema_version,
                body: CanonicalValue::Null,
            },
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
        let mut scheduled_nodes = Vec::with_capacity(plan.nodes.len());
        for (node_index, planned) in plan.nodes.iter().enumerate() {
            let implementation = bindings.resolve(planned, plan.artifacts)?;
            let expected_contract = match implementation {
                HostedPrimitiveImplementation::Literal => "conduit.std/literal",
                HostedPrimitiveImplementation::Format => "conduit.std/format",
                HostedPrimitiveImplementation::Stdin => "conduit.std/stdin",
                HostedPrimitiveImplementation::Uppercase => "conduit.std/uppercase",
                HostedPrimitiveImplementation::Stdout => "conduit.std/stdout",
                HostedPrimitiveImplementation::Stderr => "conduit.std/stderr",
                HostedPrimitiveImplementation::PassThrough => "conduit.std/pass-through",
                HostedPrimitiveImplementation::Tee => "conduit.std/tee",
                HostedPrimitiveImplementation::Merge => "conduit.std/merge",
                HostedPrimitiveImplementation::Fallback => "conduit.std/fallback",
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
                HostedPrimitiveImplementation::Format => {
                    let bytes = format_node(&resolved.source)?.into_bytes();
                    HostedNodeKind::Literal {
                        value: bytes,
                        emitted: false,
                    }
                }
                HostedPrimitiveImplementation::Stdin => HostedNodeKind::Stdin { emitted: false },
                HostedPrimitiveImplementation::Uppercase => HostedNodeKind::Uppercase,
                HostedPrimitiveImplementation::Stdout => HostedNodeKind::Stdout,
                HostedPrimitiveImplementation::Stderr => HostedNodeKind::Stderr,
                HostedPrimitiveImplementation::PassThrough => HostedNodeKind::PassThrough,
                HostedPrimitiveImplementation::Tee => HostedNodeKind::Tee,
                HostedPrimitiveImplementation::Merge => HostedNodeKind::Merge,
                HostedPrimitiveImplementation::Fallback => {
                    HostedNodeKind::Fallback { emitted: false }
                }
            };
            let in_cords = plan
                .cords
                .iter()
                .enumerate()
                .filter(|(_, cord)| cord.to.node == planned.instance)
                .map(|(index, _)| index)
                .collect();
            let out_cords = plan
                .cords
                .iter()
                .enumerate()
                .filter(|(_, cord)| cord.from.node == planned.instance)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
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
        let status = executor
            .run_until_stalled()
            .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
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
            SchedulerStatus::Failed(_) => Err(RuntimeError::new(
                "CND-RUN-005",
                "exact executor run failed",
            )),
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

enum HostedNodeKind {
    Literal { value: Vec<u8>, emitted: bool },
    Stdin { emitted: bool },
    Uppercase,
    Stdout,
    Stderr,
    PassThrough,
    Tee,
    Merge,
    Fallback { emitted: bool },
}

struct HostedSchedulerDriver<'r, 'i> {
    kind: HostedNodeKind,
    store: Rc<RefCell<HostValueStore>>,
    io: Rc<RefCell<&'r mut RunIo<'i>>>,
    in_cords: Vec<usize>,
    out_cords: Vec<usize>,
    maximum_input_bytes: u32,
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
                                    code: Id("conduit.std/stdin-bound-exceeded"),
                                };
                            }
                            Err(_) => {
                                return SchedulerStep::Failed {
                                    code: Id("conduit.std/stdin-read-error"),
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
                                code: Id("conduit.std/stdin-read-error"),
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
                            code: Id("conduit.std/stdout-write-error"),
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
                            code: Id("conduit.std/stderr-write-error"),
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
            HostedNodeKind::Tee => {
                let in_cord = match self.in_cords.first() {
                    Some(&c) => c,
                    None => return SchedulerStep::Completed,
                };
                if let Ok(Some(val)) = io.receive(in_cord) {
                    for &out_cord in &self.out_cords {
                        let _ = io.send(out_cord, val, None);
                    }
                    SchedulerStep::Progress
                } else if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) {
                    SchedulerStep::Completed
                } else {
                    let _ = io.wait_for_input(in_cord);
                    SchedulerStep::Pending
                }
            }
            HostedNodeKind::Merge => {
                let mut received = None;
                for &in_cord in &self.in_cords {
                    if let Ok(Some(val)) = io.receive(in_cord) {
                        received = Some(val);
                        break;
                    }
                }
                if let Some(val) = received {
                    if let Some(&out_cord) = self.out_cords.first() {
                        let _ = io.send(out_cord, val, None);
                    }
                    SchedulerStep::Progress
                } else if self
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
    let template = node.config("template").ok_or_else(|| {
        ResolutionError::new(
            "CND-SRC-002",
            format!("format node `{}` requires `template`", node.id),
        )
    })?;
    let parameters = node
        .config
        .iter()
        .find(|entry| entry.key == "parameters")
        .and_then(|entry| match &entry.value {
            conduit_panel::SourceValue::List(values) => Some(values),
            _ => None,
        })
        .ok_or_else(|| {
            ResolutionError::new(
                "CND-SRC-002",
                format!(
                    "format node `{}` requires a text array `parameters`",
                    node.id
                ),
            )
        })?;
    if parameters
        .iter()
        .any(|value| !matches!(value, conduit_panel::SourceValue::Text(_)))
    {
        return Err(ResolutionError::new(
            "CND-SRC-002",
            format!("format node `{}` parameters must all be text", node.id),
        ));
    }
    if let Some(entry) = node
        .config
        .iter()
        .find(|entry| entry.key != "template" && entry.key != "parameters")
    {
        return Err(ResolutionError::new(
            "CND-SRC-002",
            format!(
                "format node `{}` has unknown field `{}`",
                node.id, entry.key
            ),
        ));
    }
    render_template(template, parameters)
        .map(|_| ())
        .map_err(|message| {
            ResolutionError::new(
                "CND-SRC-002",
                format!("format node `{}`: {message}", node.id),
            )
        })
}

fn render_template(
    template: &str,
    parameters: &[conduit_panel::SourceValue],
) -> Result<String, &'static str> {
    let mut output = String::with_capacity(template.len());
    let mut parameter = 0;
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        match (ch, chars.peek().copied()) {
            ('{', Some('{')) => {
                chars.next();
                output.push('{');
            }
            ('}', Some('}')) => {
                chars.next();
                output.push('}');
            }
            ('{', Some('}')) => {
                chars.next();
                let Some(conduit_panel::SourceValue::Text(value)) = parameters.get(parameter)
                else {
                    return Err("template has more placeholders than parameters");
                };
                output.push_str(value);
                parameter += 1;
            }
            ('{' | '}', _) => return Err("unmatched brace in template"),
            _ => output.push(ch),
        }
    }
    if parameter != parameters.len() {
        return Err("template has fewer placeholders than parameters");
    }
    Ok(output)
}

fn format_node(node: &Node) -> Result<String, RuntimeError> {
    let template = node
        .config("template")
        .ok_or_else(|| RuntimeError::new("CND-RUN-004", "format template disappeared"))?;
    let parameters = node
        .config
        .iter()
        .find(|entry| entry.key == "parameters")
        .and_then(|entry| match &entry.value {
            conduit_panel::SourceValue::List(values) => Some(values.as_slice()),
            _ => None,
        })
        .ok_or_else(|| RuntimeError::new("CND-RUN-004", "format parameters disappeared"))?;
    render_template(template, parameters)
        .map_err(|message| RuntimeError::new("CND-RUN-004", message))
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

struct Format;

impl Handler for Format {
    fn run(
        &mut self,
        node: &Node,
        _inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        Ok(vec![Value::text(format_node(node)?.into_bytes())])
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
        std::str::from_utf8(&bytes)
            .map_err(|error| RuntimeError::new("CND-RUN-005", error.to_string()))?;
        Ok(vec![Value::text(bytes)])
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
    fn format_uses_positional_placeholders_and_escaped_braces() {
        let panel = parse(
            r#"
                panel 1
                node message : std/format {
                    template = "{} = {{status: {}}}"
                    parameters = list("worker", "ready")
                }
                node output : std/stdout
                cord message.out -> output.in
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
        assert_eq!(output, b"worker = {status: ready}");
    }

    #[test]
    fn format_rejects_placeholder_count_mismatch() {
        let panel = parse(
            r#"
                panel 1
                node message : std/format {
                    template = "{} {}"
                    parameters = list("only-one")
                }
            "#,
        )
        .unwrap();
        let error = Registry::compatibility_demo().resolve(&panel).unwrap_err();
        assert_eq!(error.code, "CND-SRC-002");
        assert!(error.message.contains("more placeholders than parameters"));
    }

    #[test]
    fn resolves_explains_and_runs_a_panel() {
        let panel = parse(
            r#"
                panel 1
                node greeting : conduit.std/literal {
                    value = "Hello from Conduit.\n"
                }
                node shout : conduit.std/uppercase
                node output : conduit.std/stdout
                cord greeting.out -> shout.in
                cord shout.out -> output.in
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
        assert_eq!(summary.nodes_completed, 3);
        assert_eq!(summary.cords_conducted, 2);
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
            "panel 1\nnode app { node child : conduit.std/literal }\nroot app",
            "panel 1\nnode source : conduit.std/literal using ready",
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
            "panel 1\nnode a : conduit.std/stdin\nnode b : conduit.std/stdout\n\
             cord a.out -> b.in {\n\
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
            "panel 1\nnode a : conduit.std/stdin\nnode b : conduit.std/stdout\n\
             cord a.out -> b.in {\n\
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
                node input : conduit.std/stdin
                node output : conduit.std/stdout
                cord input.out -> output.in
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
                    node source : conduit.std/literal
                    export output text = source.out
                    bind value = source.value
                }
                composite example/upper-line {
                    node source : example/literal-line
                    node upper : conduit.std/uppercase
                    cord source.text -> upper.in
                    export output text = upper.out
                    bind value = source.value
                }
                node line : example/upper-line { value = "mixed Case" }
                node stdout : conduit.std/stdout
                node stderr : conduit.std/stderr
                cord line.text -> stdout.in
                cord line.text -> stderr.in
            "#,
        )
        .expect("nested composite parses");
        let registry = Registry::compatibility_demo();
        let resolved = registry.resolve(&panel).expect("composite resolves");
        let logical = resolved.explain_logical();
        let expanded = resolved.explain_expanded();
        assert!(logical.contains("composite line : example/upper-line"));
        assert!(logical.contains("composite line/source : example/literal-line"));
        assert!(logical.contains("child line/upper : conduit.std/uppercase"));
        assert!(logical.contains("export output text -> line.upper.out"));
        assert!(logical.contains("bind value -> line/source.value"));
        assert!(expanded.contains("line.source.source : conduit.std/literal"));
        assert!(expanded.contains("line.upper : conduit.std/uppercase"));
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
        assert_eq!(summary.nodes_completed, 4);
        assert_eq!(output, b"MIXED CASE");
        assert_eq!(error, b"MIXED CASE");
    }

    #[test]
    fn composite_boundary_is_substitutable_for_primitive_inputs_and_outputs() {
        let panel = parse(
            r#"
                panel 1
                composite example/uppercase {
                    node worker : conduit.std/uppercase
                    export input in = worker.in
                    export output out = worker.out
                }
                node source : conduit.std/literal { value = "boundary" }
                node transform : example/uppercase
                node sink : conduit.std/stdout
                cord source.out -> transform.in
                cord transform.out -> sink.in
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
                   node source : conduit.std/stdin\n\
                   export output out = source.out\n\
                   export output out = source.out\n\
                 }\nnode root : example/a",
                Some("CND-SRC-002"),
                None,
            ),
            (
                "panel 1\ncomposite example/a {\n\
                   node source : conduit.std/stdin\n\
                   export output out = missing.out\n\
                 }\nnode root : example/a",
                Some("CND-SRC-009"),
                None,
            ),
            (
                "panel 1\ncomposite example/a {\n\
                   node source : conduit.std/stdin\n\
                   export input in = source.out\n\
                 }\nnode root : example/a",
                None,
                Some("CND-CMP-003"),
            ),
            (
                "panel 1\ncomposite example/a {\n\
                   node source : conduit.std/literal\n\
                   export output out = source.out\n\
                   bind value = source.missing\n\
                 }\nnode root : example/a { value = x }",
                None,
                Some("CND-CMP-003"),
            ),
            (
                "panel 1\ncomposite example/a {\n\
                   node source : conduit.std/stdin\n\
                   export output out = source.out\n\
                 }\nnode root : example/a\nnode sink : conduit.std/stdout\n\
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
