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
use std::sync::{Arc, OnceLock};

use conduit_core::{
    ArtifactDigest, ArtifactManifest, ArtifactProvenance, BlockingFairness, CanonicalDescriptor,
    CanonicalValue, CompatibilityOutcome, ConfigContract, ConfigFieldContract, ConfigIdentity,
    ConfigMutability, ConfigRequirement, ConnectionCardinality, Delivery, DescriptorRef, Direction,
    Endpoint as CoreEndpoint, ExecutionPlan, ExecutorKind, FieldDisposition, FlowCapacity,
    FlowPolicy, FlowQueueState, FlowTypeFacts, FlowWatermarks, GrantStatus, Id,
    ImplementationMachine, LossAcceptance, ManifestArtifactRef, ManifestEntrypoint,
    ManifestInterface, MapField, MemoryAccounting, NodeContract, PinnedDescriptor, PlanArtifact,
    PlanCord, PlanGraph, PlanNode, PortContract, PortFlowConstraints, Presence, Pressure,
    ReplacementSupport, ResolvedPlanNode, SampleSchedule, SchedulerPolicy, SemanticHash,
    Sensitivity, TemporalContract, TerminalClass, TerminalContract, TraitProof, TypeContractRef,
    ValueCardinality, assess_port_connection, assess_type_contract_exact,
    validate_artifact_manifest, validate_implementation_manifest, validate_plan_graph,
};
use conduit_panel::{
    CompositeDefinition, ConfigEntry, Cord, Endpoint, ExportDirection, Node, Panel, SourcePressure,
    SourceValue,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

mod artifact_verification;
mod config_resolution;
mod current_value;
mod distributed;
mod evidence_ndjson;
mod exact_evidence;
mod execution_arrangement;
mod host_conformance;
mod host_resolution;
mod hosted_lanes;
mod implementation_binding;
mod managed_component;
mod pool;
mod resource_effect;
mod runtime_evidence;
mod scheduler;
mod session;
mod source_lowering;
mod supervision;
mod temporal_conversion;
mod transition;
mod transport;
mod type_registry;
mod watch;
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
pub use current_value::{
    CurrentObservation, CurrentObservationError, CurrentUpdateRequest, CurrentValueCell,
    CurrentValueMutationAuthorizer, CurrentValueUpdateError,
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
pub use exact_evidence::{ExactEvidenceRecord, exact_evidence_batch_digest};
pub use execution_arrangement::{
    ExecutionArrangementPolicy, ExecutionArrangementReason, ResolvedExecutionArrangement,
    ResolvedExecutionBoundary, ResolvedExecutionCommitDomain, ResolvedExecutionRegion,
    resolve_execution_arrangement,
};
pub use host_conformance::{
    BoundedProviderRun, ProviderRunError, ProviderRunEvidence, ProviderRunEvidenceKind,
    ProviderRunPhase,
};
pub use host_resolution::{
    CandidateAuthority, CandidateRejection, CandidateRejectionReason, CapabilityPredicate,
    HostResolverPolicy, PlacementCandidate, PlacementRequest, PlanSealingReason, ResolutionFailure,
    ResolvedExecutionDescriptor, ResolvedExecutionLane, ResolvedExecutionPlacement,
    ResolvedHostExecutionObservation, ResolvedPlacement, ResolvedPlacementBinding,
    ResolvedReplacementSupport, ResolverTiePolicy, ResourcePredicate, TopologyPredicate,
    resolve_host_placement, seal_resolved_execution_plan,
};
pub use hosted_lanes::{
    FIXED_HOSTED_LANE_PROVIDER_ID, FixedHostedExecutionCoordinator, FixedHostedLaneProvider,
    HostedCommitBatch, HostedLaneAssignment, HostedLaneError, HostedLaneJob, HostedLaneObservation,
    HostedLaneReservation, HostedProposal, HostedProposalBatch,
};
pub use implementation_binding::{
    ForeignStepReply, ForeignStepRequest, MessageStepBinding, MessageStepEndpoint,
    NativeStepBinding, NativeStepImplementation, OwnedStepOutcome, OwnedStepReply,
    OwnedWakeInterest,
};
pub use managed_component::{
    MANAGED_COMPONENT_INTERFACE_ID, MANAGED_COMPONENT_SCHEMA_VERSION, ManagedAdapterBoundary,
    ManagedArtifactIdentity, ManagedCleanupState, ManagedComponentDescriptor,
    ManagedComponentIdentity, ManagedComponentMachine, ManagedComponentObservation,
    ManagedEvidenceKind, ManagedGrantState, ManagedLeaseState, ManagedLifecycleAction,
    ManagedLifecycleAuthority, ManagedLifecycleError, ManagedLifecycleEvidence,
    ManagedLifecycleFacets, ManagedLifecycleProgress, ManagedLifecycleReason,
    ManagedLifecycleRequest, ManagedLifecycleState, ManagedProviderAvailability,
    ManagedProviderEvent, ManagedRequestReceipt, ManagedResourceState, ManagedRuntimeReadiness,
    managed_component_interface_hash,
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
    SchedulerAllocation, SchedulerError, SchedulerEvent, SchedulerEventBatch, SchedulerEventKind,
    SchedulerHighWater, SchedulerNode, SchedulerReservation, SchedulerStatus, SchedulerStep,
    SchedulerSubject, SendStatus, StepIo, ValueStorageUsage, validate_runtime_value_for_cord,
};
pub use session::{
    ExactEvidenceBatch, ExactEvidenceCommitReceipt, ExactEvidenceCommitRequest,
    ExactEvidenceDrainError, ExactEvidenceProvider, ExactEvidenceProviderBinding,
    ExactEvidenceUseAuthority, ExactRunIdentity, ExactRunPump, ExactRunSession,
    ExactRunSessionRegistry, ExactRunState,
};

/// Resolves the immutable runtime evidence-provider identity solely from the
/// exact plan and its exact artifact collection.
pub fn exact_evidence_provider_binding(
    plan: &ExecutionPlan<'_>,
) -> Result<ExactEvidenceProviderBinding, RuntimeError> {
    let selected = plan.evidence_provider.ok_or_else(|| {
        RuntimeError::new(
            "CND-EVC-001",
            "exact plan does not select an evidence provider",
        )
    })?;
    let artifact = plan
        .artifacts
        .iter()
        .find(|artifact| artifact.id == selected.artifact)
        .ok_or_else(|| {
            RuntimeError::new(
                "CND-EVC-001",
                "exact plan evidence-provider artifact is absent",
            )
        })?;
    Ok(ExactEvidenceProviderBinding {
        implementation_id: selected.implementation.id.to_string(),
        implementation_identity: selected.implementation.semantic_hash,
        artifact_id: artifact.id.to_string(),
        artifact_digest: artifact.digest,
        host_observation_id: selected.host_observation.to_string(),
        store_resource_kind: selected.store.kind.to_string(),
        store_resource_id: selected.store.id.to_string(),
        store_generation: selected.store_generation,
        grant_hash: selected.grant_hash,
        time_basis: selected.time_basis.to_string(),
    })
}
pub use source_lowering::{
    ConfigProvenance, LOWERED_SOURCE_SCHEMA_VERSION, LiteralValidationError, LoweredAuthoredNode,
    LoweredBinding, LoweredComposite, LoweredCompositeChild, LoweredConfigEntry,
    LoweredConfigValue, LoweredCord, LoweredExport, LoweredGroupPort, LoweredInterfaceMemberProof,
    LoweredInterfaceProof, LoweredNode, LoweredPool, LoweredRootSelection, LoweredSource,
    LoweredSupervisedTopology, LoweredSupervision, LoweredTopology, LoweredTopologyBase,
    LoweringDiagnostic, OwnedConfigFieldSchema, OwnedConfigRequirement, OwnedInterfaceContract,
    OwnedInterfaceMember, OwnedNodeContract, OwnedNodeSchema, OwnedPortContract,
    OwnedPortReference, OwnedPrincipalPath, OwnedPrincipalProjectionError, OwnedSemanticValue,
    OwnedTypeReference, SOURCE_AST_SCHEMA_VERSION, SourceContractCatalog, SourceMapEntry,
    SourceOrigin, lower_source, lower_source_base, lower_supervision, lower_topology,
};
pub use supervision::BoundedSupervisionRuntime;
pub use temporal_conversion::{
    BoundedClosingCollector, ClosingFlowEvent, CollectError, CollectLimits, CollectRejection,
    CurrentChanges, EachClosingFlow, OpenFlowItem, hold_current, sample_current,
};
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
pub use watch::{
    ExactWatchBatch, ExactWatchMaterial, ExactWatchObservation, ExactWatchOperation,
    ExactWatchSubject, ExactWatchTimestamp, ExactWatchUsage, ExactWatchUseAuthority,
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
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x94, 0xdf, 0xe2, 0x55, 0x09, 0xfe, 0x62, 0x4d, 0x89, 0x74, 0xb1, 0xdd, 0x44, 0x2e, 0xb7,
        0xf9, 0x6f, 0x7e, 0x62, 0x1e, 0x6e, 0x71, 0xf0, 0x35, 0xac, 0x6f, 0x08, 0x04, 0x63, 0x61,
        0x80, 0x72,
    ]),
};
const BYTES_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/bytes"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xf8, 0x55, 0x1a, 0x62, 0x9e, 0x94, 0xf0, 0xd3, 0x66, 0x2f, 0x02, 0x78, 0x1d, 0x17, 0x63,
        0xdb, 0x29, 0xdf, 0x21, 0xce, 0x97, 0x7a, 0x90, 0xf5, 0xc7, 0x43, 0x76, 0x59, 0x9b, 0x21,
        0x90, 0x74,
    ]),
};
const RECORD_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/record"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xde, 0x0b, 0x25, 0xed, 0xf4, 0x15, 0xc7, 0x2c, 0x7d, 0xbb, 0xe1, 0x1d, 0xc7, 0x78, 0xbd,
        0x12, 0xe6, 0x8e, 0x5f, 0xc7, 0x3a, 0xb2, 0xe3, 0x8f, 0x61, 0x07, 0x2e, 0x1d, 0x29, 0x5f,
        0x22, 0xfa,
    ]),
};
const VALIDATION_DECISION_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/validation-decision"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xf9, 0x59, 0x03, 0x96, 0xa8, 0x2c, 0x69, 0xd3, 0x7e, 0xd0, 0xf2, 0x57, 0x46, 0x0e, 0x3c,
        0xee, 0x36, 0x5e, 0x79, 0x84, 0x6e, 0x1d, 0xd2, 0xa0, 0x0c, 0x83, 0xee, 0x8e, 0x86, 0x67,
        0x2a, 0xff,
    ]),
};
const FORMAT_VALUES_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/format-values"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xb6, 0x77, 0x82, 0xbd, 0x64, 0xf1, 0x19, 0x95, 0x15, 0xf7, 0x93, 0x1f, 0xd3, 0x9d, 0x9b,
        0xea, 0xca, 0xda, 0xb9, 0x1c, 0x78, 0xfe, 0x66, 0x75, 0x27, 0x12, 0x02, 0x4b, 0xa1, 0x5b,
        0xeb, 0x2e,
    ]),
};
const TERMINAL_OBSERVATION_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/terminal"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x56, 0xda, 0xdf, 0x89, 0x31, 0xf2, 0xe4, 0x26, 0xf8, 0xfa, 0x83, 0xbb, 0xb3, 0x4b, 0x30,
        0x1d, 0x13, 0xd0, 0xee, 0xc0, 0x8e, 0xd7, 0xcc, 0x98, 0x3d, 0x8a, 0x8a, 0xaa, 0xac, 0x18,
        0xfe, 0x4a,
    ]),
};
const SUPERVISION_DECISION_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("supervision/decision"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x81, 0x6e, 0x12, 0x35, 0xc6, 0x5e, 0xc7, 0xfc, 0xc4, 0xc3, 0xa3, 0x82, 0xa0, 0xa2, 0x95,
        0x08, 0x22, 0x54, 0xd7, 0x79, 0x69, 0x68, 0x7f, 0x76, 0x91, 0x65, 0x57, 0x43, 0xe6, 0x8b,
        0x34, 0xe4,
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
const HTTP_LISTENER_CONFIG: ConfigContract<'static> = ConfigContract {
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
            key: Id("deadline_ticks"),
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
const fn named_text_input(id: &'static str) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        ..VALUE_TEXT_INPUT
    }
}
const fn named_text_output(id: &'static str) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        ..VALUE_TEXT_OUTPUT
    }
}
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
const BYTES_STREAM_INPUT: PortContract<'static> = PortContract {
    id: Id("bytes"),
    direction: Direction::Input,
    value_type: BYTES_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::ExactlyOne,
    values: ValueCardinality::ZeroOrMore,
    delivery: Delivery::Stream,
    temporal: TemporalContract::Committed,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Restricted,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
};
const BYTES_STREAM_OUTPUT: PortContract<'static> = PortContract {
    id: Id("bytes"),
    direction: Direction::Output,
    value_type: BYTES_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::OneOrMore,
    values: ValueCardinality::ZeroOrMore,
    delivery: Delivery::Stream,
    temporal: TemporalContract::Committed,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Restricted,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
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
const DISCARD_TEXT_INPUT: PortContract<'static> = PortContract {
    id: Id("item"),
    sensitivity: Sensitivity::Public,
    ..STREAM_INPUT_TEXT
};
const STREAM_INPUT_TEXT_1: PortContract<'static> = PortContract {
    id: Id("left"),
    ..STREAM_INPUT_TEXT
};
const STREAM_INPUT_TEXT_2: PortContract<'static> = PortContract {
    id: Id("right"),
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
const fn named_stream_input(id: &'static str) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        ..STREAM_INPUT_TEXT
    }
}
const fn named_stream_output(id: &'static str) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        ..STREAM_OUTPUT_TEXT
    }
}

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
pub const STDIN_STREAM_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("io/stdin-stream"),
    config: EMPTY_CONFIG,
    inputs: &[],
    outputs: &[BYTES_STREAM_OUTPUT],
};
pub const UPPERCASE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("text/uppercase"),
    config: EMPTY_CONFIG,
    inputs: &[TEXT_INPUT],
    outputs: &[TEXT_OUTPUT],
};
const DATA_UTF8_CODEC_DESCRIPTOR: &str = "conduit.codec/utf-8";
const DATA_UTF8_CODEC_HASH: &[u8; 32] = &[
    0xf2, 0x19, 0x29, 0x7c, 0xb2, 0x76, 0xbc, 0x91, 0xec, 0xcd, 0xdb, 0x34, 0x6a, 0x8b, 0x21, 0xe7,
    0xed, 0xd4, 0x41, 0x4b, 0x88, 0x44, 0x01, 0x41, 0x08, 0x51, 0x37, 0x47, 0xae, 0x11, 0xbf, 0x53,
];
const DATA_LENGTH_U32BE_DESCRIPTOR: &str = "conduit.framing/length-u32be";
const DATA_LENGTH_U32BE_HASH: &[u8; 32] = &[
    0xa2, 0x4b, 0x8f, 0xf5, 0x68, 0x74, 0x21, 0x30, 0x3d, 0x44, 0xd8, 0x9a, 0xfc, 0xab, 0xe9, 0xf7,
    0x94, 0x14, 0x64, 0x41, 0x47, 0x85, 0x13, 0x4f, 0xb9, 0xe7, 0x4a, 0x32, 0x02, 0x14, 0xa6, 0xb8,
];
const DATA_CLOSED_RECORD_SCHEMA_DESCRIPTOR: &str = "conduit.schema/closed-record-name-count";
const DATA_CLOSED_RECORD_SCHEMA_HASH: &[u8; 32] = &[
    0xa3, 0xde, 0x3b, 0x0c, 0x27, 0x8e, 0x14, 0xc8, 0x76, 0x5d, 0x3e, 0x93, 0xa5, 0x25, 0x6e, 0x95,
    0x7f, 0x58, 0xee, 0x58, 0x20, 0x56, 0x4b, 0x5d, 0xce, 0x0d, 0x23, 0xd5, 0xde, 0xb0, 0x6b, 0xcf,
];
const TIME_CLOCK_DESCRIPTOR: &str = "conduit.clock/monotonic-ticks";
const TIME_CLOCK_HASH: &[u8; 32] = &[
    0x6b, 0x9c, 0x68, 0x72, 0x26, 0xd4, 0xa1, 0x96, 0x5e, 0x78, 0x0b, 0x63, 0xb4, 0xbd, 0xc0, 0x92,
    0x2d, 0xe2, 0xa6, 0x86, 0xc3, 0xc1, 0x36, 0x5f, 0x4f, 0x68, 0xf7, 0x21, 0x9f, 0x30, 0xcc, 0x48,
];
const STATE_TEXT_SCHEMA_DESCRIPTOR: &str = "conduit.state/text-register";
const STATE_TEXT_SCHEMA_HASH: &[u8; 32] = &[
    0x69, 0xd3, 0xf4, 0xd8, 0xd5, 0x37, 0x41, 0xfd, 0x07, 0x5b, 0xe4, 0xe6, 0x75, 0x5a, 0xf9, 0xbe,
    0x19, 0xdb, 0x95, 0x1a, 0x50, 0x65, 0xb8, 0x9b, 0xe5, 0xa2, 0xe8, 0x7c, 0x77, 0x55, 0x56, 0x5e,
];
const STATE_EQUALITY_DESCRIPTOR: &str = "conduit.equality/sha256-bytes";
const STATE_EQUALITY_HASH: &[u8; 32] = &[
    0xa2, 0x50, 0x96, 0xb1, 0xd4, 0x9a, 0xcb, 0x25, 0x54, 0x63, 0x9f, 0x36, 0xbd, 0x5c, 0xe0, 0x9f,
    0x18, 0x41, 0xdb, 0x66, 0x5e, 0x3d, 0x75, 0x5c, 0x22, 0x18, 0xa8, 0xe5, 0x87, 0x41, 0xf6, 0x57,
];
const STATE_CACHE_REQUEST_DESCRIPTOR: &str = "conduit.cache/text-request";
const STATE_CACHE_REQUEST_HASH: &[u8; 32] = &[
    0xe0, 0xa2, 0x09, 0xf8, 0xdd, 0xb8, 0xa8, 0xfb, 0x85, 0x50, 0x74, 0x36, 0x30, 0xc7, 0x86, 0x6c,
    0x03, 0xf2, 0xda, 0xb4, 0x87, 0x04, 0xfe, 0x66, 0x09, 0x2e, 0x23, 0x85, 0x8e, 0x41, 0xcb, 0x29,
];
const SUPERVISION_TERMINAL_DESCRIPTOR: &str = "conduit.supervision/text-terminal-observation";
const SUPERVISION_TERMINAL_HASH: &[u8; 32] = &[
    0x35, 0x4c, 0xeb, 0x69, 0x02, 0x07, 0x3d, 0x56, 0xe0, 0xcd, 0x8e, 0xa1, 0xee, 0x1a, 0xa1, 0x13,
    0x72, 0xfc, 0xec, 0xab, 0x51, 0xdb, 0x98, 0x86, 0x77, 0xc9, 0xc5, 0xb5, 0x69, 0xd1, 0x41, 0xc6,
];
const SUPERVISION_ENTROPY_DESCRIPTOR: &str = "conduit.entropy/injected-u64";
const SUPERVISION_ENTROPY_HASH: &[u8; 32] = &[
    0x0c, 0x58, 0x16, 0x8f, 0xc4, 0x5b, 0xea, 0x9f, 0x32, 0x63, 0xc9, 0xb2, 0xfe, 0x4d, 0xdc, 0xe3,
    0xe6, 0x10, 0x35, 0xe1, 0xf3, 0xc6, 0xff, 0x8d, 0x12, 0x59, 0xec, 0x69, 0xff, 0x29, 0x4a, 0xea,
];
const CLOSED_RECORD_REQUIRED_FIELDS: &[conduit_std::RequiredField<'static>] = &[
    conduit_std::RequiredField {
        name: "name",
        maximum_value_bytes: 8,
    },
    conduit_std::RequiredField {
        name: "count",
        maximum_value_bytes: 4,
    },
];

fn standard_data_contract(id: &str) -> &'static NodeContract<'static> {
    conduit_std::standard_node_contract(id).expect("standard data contract is published")
}

fn standard_time_contract(id: &str) -> &'static NodeContract<'static> {
    conduit_std::standard_node_contract(id).expect("standard time contract is published")
}

fn standard_state_contract(id: &str) -> &'static NodeContract<'static> {
    conduit_std::standard_node_contract(id).expect("standard state contract is published")
}

fn standard_supervision_contract(id: &str) -> &'static NodeContract<'static> {
    conduit_std::standard_node_contract(id).expect("standard supervision contract is published")
}

/// Current published bounded file-read contract.
#[must_use]
pub fn file_read_contract() -> &'static NodeContract<'static> {
    conduit_std::standard_node_contract("fs/read")
        .expect("standard file-read contract is published")
}

/// Current published finite file-chunk fixture source.
#[must_use]
pub fn file_chunk_literal_contract() -> &'static NodeContract<'static> {
    conduit_std::standard_node_contract("fs/chunk/literal")
        .expect("standard file-chunk literal contract is published")
}

/// Current published bounded file-write contract.
#[must_use]
pub fn file_write_contract() -> &'static NodeContract<'static> {
    conduit_std::standard_node_contract("fs/write")
        .expect("standard file-write contract is published")
}

/// Current published sink for retaining an exact file-write semantic result
/// on a cord that task-facing projections may observe.
#[must_use]
pub fn file_write_result_sink_contract() -> &'static NodeContract<'static> {
    conduit_std::standard_node_contract("fs/write-result/sink")
        .expect("standard file-write-result sink contract is published")
}

/// Current published bounded file-watch contract.
#[must_use]
pub fn file_watch_contract() -> &'static NodeContract<'static> {
    conduit_std::standard_node_contract("fs/watch")
        .expect("standard file-watch contract is published")
}
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
pub const STDOUT_STREAM_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("io/stdout-stream"),
    config: EMPTY_CONFIG,
    inputs: &[BYTES_STREAM_INPUT],
    outputs: &[],
};
pub const STDERR_STREAM_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("io/stderr-stream"),
    config: EMPTY_CONFIG,
    inputs: &[BYTES_STREAM_INPUT],
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
pub const DISCARD_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("flow/discard"),
    config: EMPTY_CONFIG,
    inputs: &[DISCARD_TEXT_INPUT],
    outputs: &[],
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
    inputs: &[
        named_stream_input("candidate"),
        named_stream_input("permit"),
    ],
    outputs: &[named_stream_output("admitted")],
};
pub const SELECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.std/select"),
    config: SELECT_CONFIG,
    inputs: &[
        STREAM_INPUT_TEXT_1,
        STREAM_INPUT_TEXT_2,
        named_stream_input("selector"),
    ],
    outputs: &[named_stream_output("selected")],
};
pub const TAKE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("flow/take"),
    config: EMPTY_CONFIG,
    inputs: &[named_text_input("item")],
    outputs: &[named_text_output("taken")],
};
pub const SKIP_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("flow/skip"),
    config: EMPTY_CONFIG,
    inputs: &[named_text_input("item")],
    outputs: &[named_text_output("retained")],
};
pub const FILTER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("flow/filter"),
    config: EMPTY_CONFIG,
    inputs: &[named_text_input("candidate")],
    outputs: &[named_text_output("accepted")],
};
pub const FALLBACK_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("flow/fallback"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_PRIMARY, INPUT_FALLBACK],
    outputs: &[named_text_output("selected")],
};
pub const PROBE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("test/probe"),
    config: EMPTY_CONFIG,
    inputs: &[named_text_input("observed")],
    outputs: &[named_text_output("forwarded")],
};
pub const LOG_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("observe/log"),
    config: EMPTY_CONFIG,
    inputs: &[named_text_input("message")],
    outputs: &[named_text_output("forwarded")],
};
pub const ASSERT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("test/assertion"),
    config: EMPTY_CONFIG,
    inputs: &[named_text_input("candidate")],
    outputs: &[named_text_output("verified")],
};
pub const RECORD_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("test/record"),
    config: EMPTY_CONFIG,
    inputs: &[named_text_input("observed")],
    outputs: &[named_text_output("recorded")],
};
pub const REPLAY_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("test/replay"),
    config: EMPTY_CONFIG,
    inputs: &[],
    outputs: &[named_text_output("recorded")],
};
pub const FAULT_SOURCE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("test/fault-source"),
    config: EMPTY_CONFIG,
    inputs: &[],
    outputs: &[named_text_output("failure")],
};
pub const GPIO_PIN_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("device/gpio/pin"),
    config: EMPTY_CONFIG,
    inputs: &[named_text_input("command")],
    outputs: &[named_text_output("state")],
};
pub const SERIAL_PORT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("device/serial/port"),
    config: EMPTY_CONFIG,
    inputs: &[named_text_input("transmit")],
    outputs: &[named_text_output("received")],
};
pub const COUNTER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("state/counter"),
    config: EMPTY_CONFIG,
    inputs: &[named_text_input("event")],
    outputs: &[named_text_output("count")],
};
pub const HEALTH_GATE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("supervision/health-gate"),
    config: EMPTY_CONFIG,
    inputs: &[named_text_input("observation")],
    outputs: &[named_text_output("healthy")],
};
/// Minimal bounded hosted HTTP listener boundary.
///
/// Rich HTTP request/response/route contracts remain in `conduit-http`; this
/// source-facing node owns one bounded listener for its exact run and waits
/// between independently bounded exchanges until the run is stopped.
pub const HTTP_LISTENER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/http/listen"),
    config: HTTP_LISTENER_CONFIG,
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

    fn record(value: impl Into<Vec<u8>>) -> Self {
        Self {
            value_type: RECORD_TYPE,
            bytes: value.into(),
        }
    }

    fn validation_decision(value: impl Into<Vec<u8>>) -> Self {
        Self {
            value_type: VALIDATION_DECISION_TYPE,
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
    /// Browser/UI presentation text.
    ///
    /// This is intentionally distinct from process standard output. Only an
    /// explicit `display/text.text` sink writes here.
    pub display: &'a mut dyn Write,
}

/// Fixed-capacity host I/O retained by a persistent exact-run session.
///
/// All four channels share one preallocated store. Their live portions are
/// compacted in a deterministic channel order, so input, stdout, stderr, and
/// display output can coexist without a hidden growable host queue.
pub struct ExactRunIo {
    storage: Vec<u8>,
    input_len: usize,
    output_len: usize,
    error_len: usize,
    display_len: usize,
}

/// Owned host boundaries supplied together for an arranged exact session
/// whose evidence provider is pinned by the plan.
pub struct ExactEvidenceSessionResources {
    pub io: ExactRunIo,
    pub evidence_provider: Box<dyn ExactEvidenceProvider>,
}

#[derive(Clone, Copy)]
enum ExactRunIoChannel {
    Input,
    Output,
    Error,
    Display,
}

impl ExactRunIo {
    /// Creates one empty owned I/O boundary with exactly the plan-admitted
    /// host-buffer capacity.
    pub fn new(capacity_bytes: u64) -> Result<Self, RuntimeError> {
        let capacity = usize::try_from(capacity_bytes).map_err(|_| {
            RuntimeError::new("CND-RUN-009", "host I/O capacity does not fit the platform")
        })?;
        let mut storage = Vec::new();
        storage
            .try_reserve_exact(capacity)
            .map_err(|_| RuntimeError::new("CND-SCH-005", "host I/O allocation failed"))?;
        storage.resize(capacity, 0);
        Ok(Self {
            storage,
            input_len: 0,
            output_len: 0,
            error_len: 0,
            display_len: 0,
        })
    }

    /// Creates an owned I/O boundary using the plan's exact aggregate
    /// host-buffer allowance.
    pub fn for_plan(plan: &ExecutionPlan<'_>) -> Result<Self, RuntimeError> {
        Self::new(exact_host_io_capacity(plan)?)
    }

    /// Adds bounded host input to the active session. This is an explicit host
    /// action; it never creates a new plan epoch or scheduler run.
    pub fn push_input(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        self.append(ExactRunIoChannel::Input, bytes)
    }

    #[must_use]
    pub fn capacity_bytes(&self) -> u64 {
        u64::try_from(self.storage.len()).unwrap_or(u64::MAX)
    }

    #[must_use]
    pub fn input(&self) -> &[u8] {
        self.channel(ExactRunIoChannel::Input)
    }

    #[must_use]
    pub fn output(&self) -> &[u8] {
        self.channel(ExactRunIoChannel::Output)
    }

    #[must_use]
    pub fn error(&self) -> &[u8] {
        self.channel(ExactRunIoChannel::Error)
    }

    #[must_use]
    pub fn display(&self) -> &[u8] {
        self.channel(ExactRunIoChannel::Display)
    }

    fn read_input(&mut self, destination: &mut [u8]) -> usize {
        let count = destination.len().min(self.input_len);
        destination[..count].copy_from_slice(&self.storage[..count]);
        let total = self.total_len();
        self.storage.copy_within(count..total, 0);
        self.input_len -= count;
        count
    }

    fn append(&mut self, channel: ExactRunIoChannel, bytes: &[u8]) -> Result<(), RuntimeError> {
        let next = self
            .total_len()
            .checked_add(bytes.len())
            .ok_or_else(|| RuntimeError::new("CND-SCH-005", "host I/O size overflowed"))?;
        if next > self.storage.len() {
            return Err(RuntimeError::new(
                "CND-SCH-005",
                "host I/O exceeded its plan-admitted capacity",
            ));
        }
        let (start, length) = self.channel_range(channel);
        let insert_at = start + length;
        let total = self.total_len();
        self.storage
            .copy_within(insert_at..total, insert_at + bytes.len());
        self.storage[insert_at..insert_at + bytes.len()].copy_from_slice(bytes);
        match channel {
            ExactRunIoChannel::Input => self.input_len += bytes.len(),
            ExactRunIoChannel::Output => self.output_len += bytes.len(),
            ExactRunIoChannel::Error => self.error_len += bytes.len(),
            ExactRunIoChannel::Display => self.display_len += bytes.len(),
        }
        Ok(())
    }

    fn channel(&self, channel: ExactRunIoChannel) -> &[u8] {
        let (start, length) = self.channel_range(channel);
        &self.storage[start..start + length]
    }

    fn channel_range(&self, channel: ExactRunIoChannel) -> (usize, usize) {
        match channel {
            ExactRunIoChannel::Input => (0, self.input_len),
            ExactRunIoChannel::Output => (self.input_len, self.output_len),
            ExactRunIoChannel::Error => (self.input_len + self.output_len, self.error_len),
            ExactRunIoChannel::Display => (
                self.input_len + self.output_len + self.error_len,
                self.display_len,
            ),
        }
    }

    fn total_len(&self) -> usize {
        self.input_len + self.output_len + self.error_len + self.display_len
    }
}

struct ExactRunIoReader(Rc<RefCell<ExactRunIo>>);

impl Read for ExactRunIoReader {
    fn read(&mut self, destination: &mut [u8]) -> std::io::Result<usize> {
        Ok(self.0.borrow_mut().read_input(destination))
    }
}

struct ExactRunIoWriter {
    io: Rc<RefCell<ExactRunIo>>,
    channel: ExactRunIoChannel,
}

impl Write for ExactRunIoWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.io
            .borrow_mut()
            .append(self.channel, bytes)
            .map_err(|error| std::io::Error::other(error.message))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn with_exact_run_io<T>(
    io: &Rc<RefCell<ExactRunIo>>,
    operation: impl FnOnce(&mut RunIo<'_>) -> T,
) -> T {
    let mut input = ExactRunIoReader(Rc::clone(io));
    let mut output = ExactRunIoWriter {
        io: Rc::clone(io),
        channel: ExactRunIoChannel::Output,
    };
    let mut error = ExactRunIoWriter {
        io: Rc::clone(io),
        channel: ExactRunIoChannel::Error,
    };
    let mut display = ExactRunIoWriter {
        io: Rc::clone(io),
        channel: ExactRunIoChannel::Display,
    };
    operation(&mut RunIo {
        input: &mut input,
        output: &mut output,
        error: &mut error,
        display: &mut display,
    })
}

#[derive(Clone)]
enum HostedRunIo<'r, 'i> {
    Owned(Rc<RefCell<ExactRunIo>>),
    Borrowed(Rc<RefCell<&'r mut RunIo<'i>>>),
}

impl HostedRunIo<'_, '_> {
    fn read_input(&self, destination: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Owned(io) => Ok(io.borrow_mut().read_input(destination)),
            Self::Borrowed(io) => io.borrow_mut().input.read(destination),
        }
    }

    fn write_channel(&self, channel: ExactRunIoChannel, bytes: &[u8]) -> std::io::Result<()> {
        match self {
            Self::Owned(io) => io
                .borrow_mut()
                .append(channel, bytes)
                .map_err(|error| std::io::Error::other(error.message)),
            Self::Borrowed(io) => {
                let mut io = io.borrow_mut();
                match channel {
                    ExactRunIoChannel::Input => Err(std::io::Error::other(
                        "hosted implementations cannot write exact-run input",
                    )),
                    ExactRunIoChannel::Output => io.output.write_all(bytes),
                    ExactRunIoChannel::Error => io.error.write_all(bytes),
                    ExactRunIoChannel::Display => io.display.write_all(bytes),
                }
            }
        }
    }

    fn with_run_io<T>(&self, operation: impl FnOnce(&mut RunIo<'_>) -> T) -> T {
        match self {
            Self::Owned(io) => with_exact_run_io(io, operation),
            Self::Borrowed(io) => operation(&mut io.borrow_mut()),
        }
    }
}

fn exact_host_io_capacity(plan: &ExecutionPlan<'_>) -> Result<u64, RuntimeError> {
    plan.nodes.iter().try_fold(0_u64, |total, node| {
        let profile = node.execution_profile.ok_or_else(|| {
            RuntimeError::new(
                "CND-RUN-009",
                format!(
                    "planned node `{}` has no execution profile",
                    node.instance.as_str()
                ),
            )
        })?;
        total
            .checked_add(profile.limits.max_host_buffer_bytes)
            .ok_or_else(|| RuntimeError::new("CND-SCH-005", "host I/O capacity overflowed"))
    })
}

fn exact_host_value_slot_capacity(plan: &ExecutionPlan<'_>) -> Result<u32, RuntimeError> {
    let queued = plan
        .cords
        .iter()
        .try_fold(0_u64, |total, cord| {
            total.checked_add(u64::from(cord.flow.capacity.items()))
        })
        .ok_or_else(|| RuntimeError::new("CND-SCH-005", "value slot capacity overflowed"))?;
    let retained = plan.nodes.iter().try_fold(0_u64, |total, node| {
        let profile = node.execution_profile.ok_or_else(|| {
            RuntimeError::new(
                "CND-RUN-009",
                format!(
                    "planned node `{}` has no execution profile",
                    node.instance.as_str()
                ),
            )
        })?;
        total
            .checked_add(u64::from(profile.limits.max_input_leases))
            .and_then(|total| total.checked_add(u64::from(profile.limits.max_output_reservations)))
            .and_then(|total| total.checked_add(u64::from(profile.limits.max_retained_values)))
            .ok_or_else(|| RuntimeError::new("CND-SCH-005", "value slot capacity overflowed"))
    })?;
    queued
        .checked_add(retained)
        .and_then(|total| u32::try_from(total).ok())
        .ok_or_else(|| RuntimeError::new("CND-SCH-005", "value slot capacity overflowed"))
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
    StdinStream,
    Uppercase,
    DataEncodeUtf8,
    DataDecodeUtf8,
    RecordLiteral,
    FileChunkLiteral,
    ValidateClosedRecord,
    ValidationDecisionAssert,
    FrameLengthU32Be,
    DeframeLengthU32Be,
    Ticker,
    TimeDelay,
    TimeTimeout,
    TimeDebounce,
    TimeThrottle,
    StateCell,
    StateDeduplicate,
    StateCache,
    SupervisionRetry,
    SupervisionCircuitBreaker,
    Stdout,
    Stderr,
    StdoutStream,
    StderrStream,
    DisplayText,
    Discard,
    PassThrough,
    Tee,
    Merge,
    ControlMerge,
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
    pub implementation_version: String,
    pub implementation_identity: SemanticHash,
    pub artifact_id: String,
    pub artifact_digest: conduit_core::ArtifactDigest,
    pub artifacts: Vec<ManagedArtifactIdentity>,
    pub implementation: HostedPrimitiveImplementation,
    pub managed_lifecycle: Option<ManagedComponentDescriptor>,
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
    ) -> Result<&ExactHostedBinding, RuntimeError> {
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
        if binding.artifacts.is_empty()
            || binding.artifacts.iter().any(|installed| {
                artifacts.iter().all(|planned| {
                    planned.id.as_str() != installed.id
                        || planned.digest.to_string() != installed.digest
                })
            })
        {
            return Err(RuntimeError::new(
                "CND-RUN-008",
                format!(
                    "installed implementation `{}` lacks its complete exact artifact set",
                    node.implementation.id
                ),
            ));
        }
        Ok(binding)
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
    /// Fresh grant/revocation observations supplied by the run authority
    /// boundary. Effects cannot infer an active grant from the sealed plan.
    pub grant_observations: &'a [ExactGrantObservation<'a>],
}

fn validate_run_arrangement(
    plan: &ExecutionPlan<'_>,
    arrangement: &ResolvedExecutionArrangement,
    plan_epoch: u64,
) -> Result<(), RuntimeError> {
    if arrangement.plan_epoch != plan_epoch {
        return Err(RuntimeError::new(
            "CND-RUN-012",
            "physical execution arrangement belongs to a different plan epoch",
        ));
    }
    arrangement
        .validate_for_plan(plan)
        .map_err(|error| RuntimeError::new(error.code(), error.to_string()))
}

/// One use-time grant observation. The immutable grant remains in the exact
/// plan; this separate fact can revoke it before any hosted effect occurs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactGrantObservation<'a> {
    pub grant: Id<'a>,
    pub status: GrantStatus<'a>,
    /// Currently observed resource binding and lease identities. These are
    /// live host facts, not authority values copied from the plan.
    pub resource_binding: Id<'a>,
    pub resource_lease: Id<'a>,
    pub lease_valid_until_tick: u64,
    pub lease_available: bool,
}

/// Owned, current authority facts supplied whenever a hosted session resumes
/// a retained timer or host-operation wait. This is intentionally separate
/// from the borrowed Start context: a persistent session owns no plan arena
/// and must not retain stale host observations between wakes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactHostedServiceUseObservation {
    /// Whether the exact selected provider remains available at this wake.
    /// This is a live host observation, separate from grant and lease state.
    pub provider_available: bool,
    pub grant_id: String,
    pub grant_active: bool,
    pub resource_binding_id: String,
    pub resource_lease_id: String,
    pub lease_valid_until_tick: u64,
    pub lease_available: bool,
}

/// Owned form of one plan-selected descriptor pin. Hosted providers receive
/// this through the executor binding; panel source cannot manufacture it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactPinnedDescriptor {
    pub id: String,
    pub schema_version: u32,
    pub semantic_hash: SemanticHash,
}

impl ExactPinnedDescriptor {
    fn as_contract(&self) -> PinnedDescriptor<'_> {
        PinnedDescriptor {
            id: Id(&self.id),
            schema_version: self.schema_version,
            semantic_hash: self.semantic_hash,
        }
    }
}

/// Owned exact lease copied from the sealed plan for an effectful hosted
/// provider. The semantic identity is retained separately and rechecked by
/// providers before mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactResourceLeaseBinding {
    pub identity: SemanticHash,
    pub schema_version: u32,
    pub id: String,
    pub resource_binding: String,
    pub holder: String,
    pub run: String,
    pub epoch: u64,
    pub scope: String,
    pub sharing: conduit_core::ResourceSharingMode,
    pub reservation: conduit_core::PlanResourceBudget,
    pub time_basis: String,
    pub issued_at_tick: u64,
    pub expires_at_tick: u64,
    pub revocation_grace_ticks: u64,
    pub cleanup_ticks: u64,
    pub maximum_operations: u32,
    pub maximum_evidence_events: u32,
    pub cleanup_escalation: ExactPinnedDescriptor,
    pub foreign_retention: conduit_core::ForeignRetention,
}

impl ExactResourceLeaseBinding {
    pub fn with_contract<T>(
        &self,
        use_contract: impl FnOnce(conduit_core::ResourceLeaseContract<'_>) -> T,
    ) -> T {
        use_contract(conduit_core::ResourceLeaseContract {
            schema_version: self.schema_version,
            id: Id(&self.id),
            resource_binding: Id(&self.resource_binding),
            holder: conduit_core::InstancePath::new(&self.holder)
                .expect("exact hosted lease holder was validated by the sealed plan"),
            run: Id(&self.run),
            epoch: self.epoch,
            scope: Id(&self.scope),
            sharing: self.sharing,
            reservation: self.reservation,
            time_basis: Id(&self.time_basis),
            issued_at_tick: self.issued_at_tick,
            expires_at_tick: self.expires_at_tick,
            revocation_grace_ticks: self.revocation_grace_ticks,
            cleanup_ticks: self.cleanup_ticks,
            maximum_operations: self.maximum_operations,
            maximum_evidence_events: self.maximum_evidence_events,
            cleanup_escalation: self.cleanup_escalation.as_contract(),
            foreign_retention: self.foreign_retention,
        })
    }
}

/// Owned exact commit profile copied from the sealed plan for an effectful
/// hosted provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactEffectCommitBinding {
    pub identity: SemanticHash,
    pub schema_version: u32,
    pub id: String,
    pub operation: String,
    pub resource_lease: String,
    pub commit_boundary: ExactPinnedDescriptor,
    pub idempotency: conduit_core::EffectIdempotency,
    pub unknown_commit: conduit_core::UnknownCommitPolicy,
    pub discontinuity: conduit_core::EffectDiscontinuity,
    pub cleanup: ExactPinnedDescriptor,
    pub maximum_attempts: u16,
    pub evidence_events_per_attempt: u16,
}

impl ExactEffectCommitBinding {
    pub fn with_contract<T>(
        &self,
        use_contract: impl FnOnce(conduit_core::EffectCommitProfile<'_>) -> T,
    ) -> T {
        use_contract(conduit_core::EffectCommitProfile {
            schema_version: self.schema_version,
            id: Id(&self.id),
            operation: Id(&self.operation),
            resource_lease: Id(&self.resource_lease),
            commit_boundary: self.commit_boundary.as_contract(),
            idempotency: self.idempotency,
            unknown_commit: self.unknown_commit,
            discontinuity: self.discontinuity,
            cleanup: self.cleanup.as_contract(),
            maximum_attempts: self.maximum_attempts,
            evidence_events_per_attempt: self.evidence_events_per_attempt,
        })
    }
}

impl<'a> From<ExactGrantObservation<'a>> for ExactHostedServiceUseObservation {
    fn from(observation: ExactGrantObservation<'a>) -> Self {
        Self {
            provider_available: true,
            grant_id: observation.grant.to_string(),
            grant_active: matches!(observation.status, GrantStatus::Active),
            resource_binding_id: observation.resource_binding.to_string(),
            resource_lease_id: observation.resource_lease.to_string(),
            lease_valid_until_tick: observation.lease_valid_until_tick,
            lease_available: observation.lease_available,
        }
    }
}

/// Converts freshly observed borrowed grant facts into the owned, bounded
/// wake input required by a persistent hosted session.
#[must_use]
pub fn hosted_service_use_observations(
    observations: &[ExactGrantObservation<'_>],
) -> Vec<ExactHostedServiceUseObservation> {
    observations.iter().copied().map(Into::into).collect()
}

/// One exact authority binding projected for a selected hosted service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactHostedServiceAuthority {
    pub effect_hash: SemanticHash,
    pub action: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub resource_binding_id: String,
    pub resource_lease_id: String,
    pub resource_lease_identity: SemanticHash,
    pub resource_lease_run_id: String,
    pub resource_lease_epoch: u64,
    pub resource_lease_time_basis: String,
    pub commit_profile_identity: SemanticHash,
    pub resource_lease: ExactResourceLeaseBinding,
    pub commit_profile: ExactEffectCommitBinding,
    pub grant_id: String,
    /// The last exact authority/lease tick at which this effect may begin a
    /// hosted operation. This is derived from the pinned grant, capability,
    /// resource lease, and fresh use-time lease observation at Start.
    pub valid_until_tick: u64,
    pub check_at_use: bool,
    pub constraints: Vec<(String, SemanticHash)>,
}

/// Executor-built facts supplied to a hosted service before invocation.
/// Source cannot construct this value or replace its plan-derived contents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactHostedServiceBinding {
    pub plan_identity: SemanticHash,
    pub plan_epoch: u64,
    pub run_id: String,
    pub instance: String,
    pub implementation_id: String,
    pub implementation_version: String,
    pub implementation_identity: SemanticHash,
    pub artifact_id: String,
    pub artifacts: Vec<ManagedArtifactIdentity>,
    pub host_id: String,
    pub host_boot_id: String,
    pub host_observation_id: String,
    pub host_observation_valid_until_tick: u64,
    pub managed_lifecycle: Option<ManagedComponentDescriptor>,
    pub use_time_tick: u64,
    pub authorities: Vec<ExactHostedServiceAuthority>,
}

/// Exact time supplied to one bounded hosted-provider step.
///
/// The scheduler owns this value. A provider can request a later wake, but it
/// cannot advance the clock itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostedServiceStepContext {
    pub tick: u64,
}

/// One named wake a hosted provider may request after a bounded step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedServiceInterest {
    Timer {
        subject: Id<'static>,
        deadline_tick: u64,
    },
    HostOperation {
        subject: Id<'static>,
    },
}

/// Outcome of one bounded hosted-provider step.
///
/// `Produced` leaves the run live after its outputs have reached their exact
/// cords. `Waiting` registers one or more exact scheduler interests; it never
/// spins or advances a real host clock. The selected execution profile bounds
/// their number and kinds. `Completed` ends only this provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostedServiceStep {
    Produced {
        outputs: Vec<Value>,
    },
    Waiting {
        interests: Vec<HostedServiceInterest>,
    },
    Completed {
        outputs: Vec<Value>,
    },
}

/// Outcome of one bounded hosted-provider cleanup step.
///
/// Cleanup uses the same exact timer and named host-operation interests as
/// ordinary provider work. `Waiting` therefore yields control to the host and
/// remains subject to the plan-pinned cancellation deadline; it never blocks
/// the scheduler thread or hides a cleanup task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostedServiceCleanup {
    Complete,
    Waiting {
        interests: Vec<HostedServiceInterest>,
    },
}

impl HostedServiceCleanup {
    #[must_use]
    pub const fn complete() -> Self {
        Self::Complete
    }

    #[must_use]
    pub fn waiting(interest: HostedServiceInterest) -> Self {
        Self::Waiting {
            interests: vec![interest],
        }
    }

    #[must_use]
    pub fn waiting_for(interests: Vec<HostedServiceInterest>) -> Self {
        Self::Waiting { interests }
    }
}

impl HostedServiceStep {
    #[must_use]
    pub fn completed(outputs: Vec<Value>) -> Self {
        Self::Completed { outputs }
    }

    #[must_use]
    pub fn produced(outputs: Vec<Value>) -> Self {
        Self::Produced { outputs }
    }

    #[must_use]
    pub fn waiting(interest: HostedServiceInterest) -> Self {
        Self::Waiting {
            interests: vec![interest],
        }
    }

    #[must_use]
    pub fn waiting_for(interests: Vec<HostedServiceInterest>) -> Self {
        Self::Waiting { interests }
    }
}

/// Hashes one domain-owned, plan-visible hosted-effect constraint. The id
/// supplies the semantic domain; the value remains exact opaque bytes.
#[must_use]
pub fn hosted_effect_constraint_hash(id: &str, value: &[u8]) -> SemanticHash {
    let mut hash = Sha256::new();
    hash.update(b"conduit.hosted-effect-constraint\0");
    hash.update(id.as_bytes());
    hash.update(b"\0");
    hash.update(value);
    SemanticHash::from_bytes(hash.finalize().into())
}

fn validate_use_time_grants(
    plan: &ExecutionPlan<'_>,
    context: ExactRunContext<'_>,
) -> Result<(), RuntimeError> {
    let required = plan
        .authorities
        .iter()
        .map(|authority| authority.grant.id.as_str())
        .collect::<BTreeSet<_>>();
    let supplied = context
        .grant_observations
        .iter()
        .map(|observation| observation.grant.as_str())
        .collect::<BTreeSet<_>>();
    if required != supplied || supplied.len() != context.grant_observations.len() {
        return Err(RuntimeError::new(
            "CND-RUN-010",
            "use-time grant observations do not exactly cover the plan authorities",
        ));
    }
    for authority in plan.authorities {
        let observation = context
            .grant_observations
            .iter()
            .find(|observation| observation.grant == authority.grant.id)
            .expect("exact grant inventory was checked");
        conduit_core::validate_authority_at_use(
            authority.binding,
            authority.effect,
            context.validation.now,
            authority.capability,
            conduit_core::ObservedGrant {
                grant: authority.grant,
                status: observation.status,
            },
        )
        .map_err(|error| RuntimeError::new("CND-RUN-010", error.to_string()))?;
        let resource = plan
            .resources
            .iter()
            .find(|resource| {
                resource.node == authority.node && resource.resource == authority.binding.resource
            })
            .ok_or_else(|| {
                RuntimeError::new(
                    "CND-RUN-010",
                    "use-time authority lacks its exact resource binding",
                )
            })?;
        let lease = resource.lease.ok_or_else(|| {
            RuntimeError::new(
                "CND-RUN-010",
                "use-time authority lacks its exact resource lease",
            )
        })?;
        if observation.resource_binding != resource.id
            || observation.resource_lease != lease.id
            || !observation.lease_available
            || observation.lease_valid_until_tick > lease.expires_at_tick
            || context.validation.now.tick >= observation.lease_valid_until_tick
        {
            return Err(RuntimeError::new(
                "CND-RUN-010",
                "use-time resource binding or lease observation drifted",
            ));
        }
    }
    Ok(())
}

fn exact_host_service_binding(
    plan: &ExecutionPlan<'_>,
    node: &ResolvedPlanNode<'_>,
    installed: &ExactHostedBinding,
    context: ExactRunContext<'_>,
) -> Result<ExactHostedServiceBinding, RuntimeError> {
    let authorities = plan
        .authorities
        .iter()
        .filter(|authority| authority.node == node.instance)
        .map(|authority| {
            let resource = plan
                .resources
                .iter()
                .find(|resource| {
                    resource.node == node.instance
                        && resource.resource == authority.binding.resource
                        && resource.host_observation == node.host_observation
                })
                .ok_or_else(|| {
                    RuntimeError::new(
                        "CND-RUN-010",
                        "hosted service authority lacks its exact resource binding",
                    )
                })?;
            let lease = resource.lease.ok_or_else(|| {
                RuntimeError::new(
                    "CND-RUN-010",
                    "hosted service authority lacks its exact resource lease",
                )
            })?;
            let commit_profile = authority.commit_profile.ok_or_else(|| {
                RuntimeError::new(
                    "CND-RUN-010",
                    "hosted service effect lacks its exact commit profile",
                )
            })?;
            let observation = context
                .grant_observations
                .iter()
                .find(|observation| observation.grant == authority.grant.id)
                .expect("exact grant inventory was checked before binding");
            let valid_until_tick = authority
                .capability
                .valid_until_tick
                .min(authority.grant.expires_at_tick)
                .min(lease.expires_at_tick)
                .min(observation.lease_valid_until_tick);
            Ok(ExactHostedServiceAuthority {
                effect_hash: authority.effect_hash,
                action: authority.effect.action.to_string(),
                resource_kind: authority.binding.resource.kind.to_string(),
                resource_id: authority.binding.resource.id.to_string(),
                resource_binding_id: resource.id.to_string(),
                resource_lease_id: lease.id.to_string(),
                resource_lease_identity: lease.semantic_hash().map_err(|_| {
                    RuntimeError::new(
                        "CND-RUN-010",
                        "hosted service resource lease identity is invalid",
                    )
                })?,
                resource_lease_run_id: lease.run.to_string(),
                resource_lease_epoch: lease.epoch,
                resource_lease_time_basis: lease.time_basis.to_string(),
                commit_profile_identity: commit_profile.semantic_hash().map_err(|_| {
                    RuntimeError::new(
                        "CND-RUN-010",
                        "hosted service commit profile identity is invalid",
                    )
                })?,
                resource_lease: ExactResourceLeaseBinding {
                    identity: lease.semantic_hash().map_err(|_| {
                        RuntimeError::new(
                            "CND-RUN-010",
                            "hosted service resource lease identity is invalid",
                        )
                    })?,
                    schema_version: lease.schema_version,
                    id: lease.id.to_string(),
                    resource_binding: lease.resource_binding.to_string(),
                    holder: lease.holder.as_str().to_owned(),
                    run: lease.run.to_string(),
                    epoch: lease.epoch,
                    scope: lease.scope.to_string(),
                    sharing: lease.sharing,
                    reservation: lease.reservation,
                    time_basis: lease.time_basis.to_string(),
                    issued_at_tick: lease.issued_at_tick,
                    expires_at_tick: lease.expires_at_tick,
                    revocation_grace_ticks: lease.revocation_grace_ticks,
                    cleanup_ticks: lease.cleanup_ticks,
                    maximum_operations: lease.maximum_operations,
                    maximum_evidence_events: lease.maximum_evidence_events,
                    cleanup_escalation: ExactPinnedDescriptor {
                        id: lease.cleanup_escalation.id.to_string(),
                        schema_version: lease.cleanup_escalation.schema_version,
                        semantic_hash: lease.cleanup_escalation.semantic_hash,
                    },
                    foreign_retention: lease.foreign_retention,
                },
                commit_profile: ExactEffectCommitBinding {
                    identity: commit_profile.semantic_hash().map_err(|_| {
                        RuntimeError::new(
                            "CND-RUN-010",
                            "hosted service commit profile identity is invalid",
                        )
                    })?,
                    schema_version: commit_profile.schema_version,
                    id: commit_profile.id.to_string(),
                    operation: commit_profile.operation.to_string(),
                    resource_lease: commit_profile.resource_lease.to_string(),
                    commit_boundary: ExactPinnedDescriptor {
                        id: commit_profile.commit_boundary.id.to_string(),
                        schema_version: commit_profile.commit_boundary.schema_version,
                        semantic_hash: commit_profile.commit_boundary.semantic_hash,
                    },
                    idempotency: commit_profile.idempotency,
                    unknown_commit: commit_profile.unknown_commit,
                    discontinuity: commit_profile.discontinuity,
                    cleanup: ExactPinnedDescriptor {
                        id: commit_profile.cleanup.id.to_string(),
                        schema_version: commit_profile.cleanup.schema_version,
                        semantic_hash: commit_profile.cleanup.semantic_hash,
                    },
                    maximum_attempts: commit_profile.maximum_attempts,
                    evidence_events_per_attempt: commit_profile.evidence_events_per_attempt,
                },
                grant_id: authority.grant.id.to_string(),
                valid_until_tick,
                check_at_use: authority.binding.check_at_use,
                constraints: authority
                    .effect
                    .constraints
                    .iter()
                    .map(|constraint| (constraint.id.to_string(), constraint.semantic_hash))
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    let host_observation = plan
        .host_observations
        .iter()
        .find(|observation| observation.id == node.host_observation)
        .ok_or_else(|| {
            RuntimeError::new("CND-RUN-007", "planned host boot observation is absent")
        })?;
    Ok(ExactHostedServiceBinding {
        plan_identity: plan.identity,
        plan_epoch: context.plan_epoch,
        run_id: context.run_id.to_string(),
        instance: node.instance.as_str().to_owned(),
        implementation_id: node.implementation.id.to_string(),
        implementation_version: installed.implementation_version.clone(),
        implementation_identity: node.implementation.semantic_hash,
        artifact_id: node.artifact.to_string(),
        artifacts: installed.artifacts.clone(),
        host_id: node.host.to_string(),
        host_boot_id: host_observation.boot_id.to_string(),
        host_observation_id: node.host_observation.to_string(),
        host_observation_valid_until_tick: host_observation.valid_until_tick,
        managed_lifecycle: installed.managed_lifecycle.clone(),
        use_time_tick: context.validation.now.tick,
        authorities,
    })
}

fn managed_component_machine(
    binding: &ExactHostedServiceBinding,
    semantic_contract: &str,
) -> Result<Option<ManagedComponentMachine>, RuntimeError> {
    let Some(descriptor) = binding.managed_lifecycle.clone() else {
        return Ok(None);
    };
    let identity = ManagedComponentIdentity {
        component: binding.instance.clone(),
        semantic_contract: semantic_contract.to_owned(),
        implementation_id: binding.implementation_id.clone(),
        implementation_version: binding.implementation_version.clone(),
        implementation_identity: binding.implementation_identity.to_string(),
        artifacts: binding.artifacts.clone(),
        host_id: binding.host_id.clone(),
        host_boot_id: binding.host_boot_id.clone(),
        host_observation_id: binding.host_observation_id.clone(),
        run_id: binding.run_id.clone(),
        plan_identity: binding.plan_identity.to_string(),
        plan_epoch: binding.plan_epoch,
        activation_generation: 1,
        resources: binding
            .authorities
            .iter()
            .map(|authority| authority.resource_binding_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        grants: binding
            .authorities
            .iter()
            .map(|authority| authority.grant_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        leases: binding
            .authorities
            .iter()
            .map(|authority| authority.resource_lease_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    };
    ManagedComponentMachine::new(
        descriptor,
        identity,
        binding.use_time_tick,
        binding
            .authorities
            .iter()
            .map(|authority| authority.valid_until_tick)
            .min()
            .unwrap_or(binding.host_observation_valid_until_tick)
            .min(binding.host_observation_valid_until_tick),
    )
    .map(Some)
    .map_err(|error| RuntimeError::new(error.code, error.to_string()))
}

fn managed_tick(binding: &ExactHostedServiceBinding, scheduler_tick: u64) -> u64 {
    binding.use_time_tick.saturating_add(scheduler_tick)
}

fn begin_managed_executor_transition(
    machine: &mut ManagedComponentMachine,
    action: ManagedLifecycleAction,
    tick: u64,
    causation: &str,
) -> Result<String, RuntimeError> {
    let observation = machine.observation();
    let request_id = format!(
        "{}/executor-{:?}-{}",
        observation.identity.component, action, observation.sequence
    )
    .to_ascii_lowercase();
    let request = ManagedLifecycleRequest {
        schema_version: MANAGED_COMPONENT_SCHEMA_VERSION,
        request_id: request_id.clone(),
        component: observation.identity.component.clone(),
        action,
        expected_plan_epoch: observation.identity.plan_epoch,
        expected_activation_generation: observation.identity.activation_generation,
        expected_observation_sequence: observation.sequence,
        issued_at_tick: tick,
        deadline_tick: tick
            .checked_add(machine.descriptor().maximum_request_ticks)
            .ok_or_else(|| RuntimeError::new("CND-MCL-013", "lifecycle deadline overflowed"))?,
        causation: causation.to_owned(),
    };
    let authority = ManagedLifecycleAuthority {
        requester: "conduit/exact-hosted-executor".to_owned(),
        authority_id: observation.identity.run_id.clone(),
        provider: ManagedProviderAvailability::Available,
        grant: ManagedGrantState::Active,
        resources: ManagedResourceState::Available,
        leases: ManagedLeaseState::Current,
        not_before_tick: tick,
        expires_at_tick: tick.saturating_add(1),
        actions: vec![action],
        inhibit_asserted: false,
    };
    machine
        .request(request, &authority, tick)
        .map_err(|error| RuntimeError::new(error.code, error.to_string()))?;
    Ok(request_id)
}

fn apply_managed_executor_event(
    machine: &Rc<RefCell<ManagedComponentMachine>>,
    request_id: &str,
    event: ManagedProviderEvent,
    tick: u64,
) -> Result<(), RuntimeError> {
    machine
        .borrow_mut()
        .apply_provider_event(request_id, event, tick)
        .map_err(|error| RuntimeError::new(error.code, error.to_string()))
}

fn begin_managed_stop(
    machine: &Rc<RefCell<ManagedComponentMachine>>,
    tick: u64,
    causation: &str,
    in_flight: u32,
) -> Result<String, RuntimeError> {
    let request_id = begin_managed_executor_transition(
        &mut machine.borrow_mut(),
        ManagedLifecycleAction::Stop,
        tick,
        causation,
    )?;
    apply_managed_executor_event(
        machine,
        &request_id,
        ManagedProviderEvent::AdmissionClosed { in_flight },
        tick,
    )?;
    Ok(request_id)
}

fn begin_managed_cleanup(
    machine: &Rc<RefCell<ManagedComponentMachine>>,
    request_id: &str,
    tick: u64,
) -> Result<(), RuntimeError> {
    if machine.borrow().observation().state == ManagedLifecycleState::Quiescing {
        apply_managed_executor_event(
            machine,
            request_id,
            ManagedProviderEvent::Quiesced {
                drained: 0,
                cancelled: 0,
            },
            tick,
        )?;
    }
    if machine.borrow().observation().state == ManagedLifecycleState::Inactive {
        apply_managed_executor_event(
            machine,
            request_id,
            ManagedProviderEvent::CleanupStarted,
            tick,
        )?;
    }
    Ok(())
}

fn set_managed_readiness(
    managed: &Option<Rc<RefCell<ManagedComponentMachine>>>,
    readiness: ManagedRuntimeReadiness,
    tick: u64,
    causation: &str,
) -> Result<(), RuntimeError> {
    let Some(machine) = managed else {
        return Ok(());
    };
    let mut machine = machine.borrow_mut();
    if machine.observation().state != ManagedLifecycleState::Active
        || machine.observation().readiness == readiness
    {
        return Ok(());
    }
    machine
        .set_readiness(readiness, tick, causation)
        .map_err(|error| RuntimeError::new(error.code, error.to_string()))
}

fn validate_hosted_service_use_time(
    binding: &ExactHostedServiceBinding,
    scheduler_tick: u64,
) -> Result<(), RuntimeError> {
    let tick = binding
        .use_time_tick
        .checked_add(scheduler_tick)
        .ok_or_else(|| RuntimeError::new("CND-RUN-010", "hosted use-time tick overflowed"))?;
    if binding
        .authorities
        .iter()
        .any(|authority| authority.check_at_use && tick >= authority.valid_until_tick)
    {
        return Err(RuntimeError::new(
            "CND-RUN-010",
            "hosted use-time grant, capability, or lease is stale",
        ));
    }
    Ok(())
}

fn validate_hosted_service_wake(
    binding: &ExactHostedServiceBinding,
    scheduler_tick: u64,
    grant_observations: &[ExactHostedServiceUseObservation],
) -> Result<(), RuntimeError> {
    let tick = binding
        .use_time_tick
        .checked_add(scheduler_tick)
        .ok_or_else(|| RuntimeError::new("CND-RUN-010", "hosted use-time tick overflowed"))?;
    validate_hosted_service_use_time(binding, scheduler_tick)?;
    for authority in &binding.authorities {
        let observation = grant_observations
            .iter()
            .find(|observation| observation.grant_id == authority.grant_id)
            .ok_or_else(|| {
                RuntimeError::new(
                    "CND-RUN-010",
                    "hosted wake lacks its fresh grant observation",
                )
            })?;
        if !observation.provider_available {
            return Err(RuntimeError::new(
                "CND-RUN-012",
                "hosted provider is unavailable at the retained wake",
            ));
        }
        if !observation.grant_active
            || observation.resource_binding_id != authority.resource_binding_id
            || observation.resource_lease_id != authority.resource_lease_id
            || !observation.lease_available
            || tick >= observation.lease_valid_until_tick
        {
            return Err(RuntimeError::new(
                "CND-RUN-010",
                "hosted wake grant, resource binding, or lease is no longer exact",
            ));
        }
    }
    Ok(())
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
    pub hosted_lane_batch: Option<HostedLaneBatchEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct HostedLaneBatchEvidence {
    pub commit_domain: String,
    pub active_lanes: Vec<u16>,
    pub proposal_slots_used: u16,
    pub proposal_slots_capacity: u16,
    pub proposal_bytes_used: u64,
    pub proposal_bytes_capacity: u64,
    pub committed_tickets: Vec<u64>,
    pub physical_completion_order: Vec<HostedLaneObservation>,
}

/// Hosted exact-run ownership retained across cooperative scheduler turns.
///
/// It owns the admitted executor, implementation state, and one fixed,
/// plan-admitted host-I/O store. No caller stack frame or plan arena is
/// retained after Start returns.
pub struct ExactHostedRunSession {
    session: ExactRunSession<HostedSchedulerDriver<'static, 'static>>,
    parallel_lanes: Option<HostedProductionLanes>,
    host_failure: Rc<RefCell<Option<RuntimeError>>>,
    io: Rc<RefCell<ExactRunIo>>,
    watches: Rc<RefCell<watch::HostedWatchRuntime>>,
    managed_components: Vec<Rc<RefCell<ManagedComponentMachine>>>,
}

type StartedHostedSession<'r, 'i> = (
    ExactRunSession<HostedSchedulerDriver<'r, 'i>>,
    Option<HostedProductionLanes>,
    Rc<RefCell<Option<RuntimeError>>>,
    Rc<RefCell<watch::HostedWatchRuntime>>,
    Vec<Rc<RefCell<ManagedComponentMachine>>>,
);

impl ExactHostedRunSession {
    #[must_use]
    pub fn identity(&self) -> &ExactRunIdentity {
        self.session.identity()
    }

    #[must_use]
    pub fn state(&self) -> ExactRunState {
        self.session.state()
    }

    #[must_use]
    pub fn scheduler_status(&self) -> SchedulerStatus {
        self.session.scheduler_status()
    }

    /// Current typed observations for only those exact implementations that
    /// explicitly offered the managed-component facet.
    #[must_use]
    pub fn managed_component_observations(&self) -> Vec<ManagedComponentObservation> {
        self.managed_components
            .iter()
            .map(|machine| machine.borrow().observation().clone())
            .collect()
    }

    /// Reads a bounded lifecycle-evidence window for one exact component.
    pub fn read_managed_component_evidence(
        &self,
        component: &str,
        cursor: u64,
        maximum_events: u32,
    ) -> Result<Vec<ManagedLifecycleEvidence>, RuntimeError> {
        let machine = self
            .managed_components
            .iter()
            .find(|machine| machine.borrow().observation().identity.component == component)
            .ok_or_else(|| {
                RuntimeError::new("CND-MCL-010", "component does not offer managed lifecycle")
            })?;
        let machine = machine.borrow();
        if maximum_events == 0 || maximum_events > machine.descriptor().maximum_retained_events {
            return Err(RuntimeError::new(
                "CND-MCL-033",
                "managed lifecycle evidence request exceeds its finite bound",
            ));
        }
        if cursor < machine.earliest_evidence_sequence() {
            return Err(RuntimeError::new(
                "CND-MCL-013",
                "managed lifecycle evidence cursor predates retained evidence",
            ));
        }
        Ok(machine
            .evidence()
            .filter(|event| event.sequence >= cursor)
            .take(maximum_events as usize)
            .cloned()
            .collect())
    }

    pub fn pump(
        &mut self,
        quantum: u64,
        grant_observations: &[ExactHostedServiceUseObservation],
    ) -> Result<ExactRunPump, RuntimeError> {
        let parallel_lanes = &mut self.parallel_lanes;
        self.session
            .pump_with_authority_using(
                quantum,
                grant_observations,
                |executor, remaining, grants| {
                    let Some(parallel_lanes) = parallel_lanes.as_mut() else {
                        return Ok(false);
                    };
                    parallel_lanes.drive(executor, remaining, grants)
                },
            )
            .map_err(|error| self.take_scheduler_error(error))
    }

    pub fn advance_to(
        &mut self,
        tick: u64,
        grant_observations: &[ExactHostedServiceUseObservation],
    ) -> Result<ExactRunPump, RuntimeError> {
        self.session
            .advance_to_with_authority(tick, grant_observations)
            .map_err(|error| self.take_scheduler_error(error))
    }

    pub fn notify_host_operation(
        &mut self,
        subject: Id<'_>,
        grant_observations: &[ExactHostedServiceUseObservation],
    ) -> Result<ExactRunPump, RuntimeError> {
        self.session
            .notify_host_operation_with_authority(subject, grant_observations)
            .map_err(|error| self.take_scheduler_error(error))
    }

    pub fn cancel(&mut self, stop: conduit_core::StopPolicy) -> Result<ExactRunPump, RuntimeError> {
        if let Some(mut parallel_lanes) = self.parallel_lanes.take() {
            parallel_lanes.cancel();
        }
        let pump = self
            .session
            .cancel(stop)
            .map_err(|error| self.take_scheduler_error(error))?;
        if let Some(error) = self.host_failure.borrow_mut().take() {
            return Err(error);
        }
        Ok(pump)
    }

    /// Apply an observed loss of one admitted hosted lane. The arrangement is
    /// not silently shrunk; the next production batch fails and fences the
    /// exact session through the normal host-failure boundary.
    pub fn observe_hosted_lane_loss(&mut self, lane: u16) -> Result<(), RuntimeError> {
        let Some(parallel_lanes) = self.parallel_lanes.as_mut() else {
            return Err(RuntimeError::new(
                "CND-LAN-005",
                "exact session has no active hosted lane provider",
            ));
        };
        parallel_lanes.observe_lane_loss(lane)
    }

    #[must_use]
    pub fn next_timer_deadline(&self) -> Option<u64> {
        self.session.next_timer_deadline()
    }

    #[must_use]
    pub fn allocation(&self) -> SchedulerAllocation {
        self.session.allocation()
    }

    /// Runtime budget reserved in the supplied session registry for this run.
    #[must_use]
    pub fn reserved_session_bytes(&self) -> u64 {
        self.session.reserved_session_bytes()
    }

    #[must_use]
    pub fn high_water(&self) -> SchedulerHighWater {
        self.session.high_water()
    }

    /// Current and high-water occupancy of this session's fixed value arena.
    #[must_use]
    pub fn value_storage_usage(&self) -> Option<ValueStorageUsage> {
        self.session.value_storage_usage()
    }

    pub fn scheduler_events(&self) -> impl Iterator<Item = &SchedulerEvent> {
        self.session.scheduler_events()
    }

    #[must_use]
    pub fn hosted_lane_batch(&self) -> Option<HostedLaneBatchEvidence> {
        self.parallel_lanes
            .as_ref()
            .and_then(HostedProductionLanes::batch_evidence)
    }

    #[must_use]
    pub fn retained_event_cursor(&self) -> u64 {
        self.session.retained_event_cursor()
    }

    /// One-past-the-end monotonic event cursor for this hosted exact session.
    #[must_use]
    pub fn next_event_cursor(&self) -> u64 {
        self.session.next_event_cursor()
    }

    pub fn read_scheduler_events(
        &self,
        cursor: u64,
        maximum_events: u32,
    ) -> Result<SchedulerEventBatch, RuntimeError> {
        self.session
            .read_scheduler_events(cursor, maximum_events)
            .map_err(|error| RuntimeError::new(error.code(), error.to_string()))
    }

    /// Projects one bounded read-only exact-evidence delta without releasing
    /// the scheduler's retained observation prefix.
    pub fn read_exact_evidence(
        &self,
        cursor: u64,
        maximum_events: u32,
    ) -> Result<ExactEvidenceBatch, RuntimeError> {
        self.session
            .read_exact_evidence(cursor, maximum_events)
            .map_err(|error| RuntimeError::new(error.code(), error.to_string()))
    }

    /// Attaches one exact-plan-admitted structural Watch. Public material is
    /// copied into its fixed preview buffer; protected material remains
    /// redacted until a later exact reveal-authority boundary authorizes it.
    pub fn attach_watch(
        &mut self,
        watch_id: &str,
        authority: &ExactWatchUseAuthority,
    ) -> Result<(), RuntimeError> {
        let identity = self.session.identity().clone();
        self.watches
            .borrow_mut()
            .attach(&identity, watch_id, authority)
    }

    /// Stops future observation without mutating the run or discarding the
    /// Watch's already retained bounded window.
    pub fn detach_watch(
        &mut self,
        watch_id: &str,
        authority: &ExactWatchUseAuthority,
    ) -> Result<(), RuntimeError> {
        let identity = self.session.identity().clone();
        self.watches
            .borrow_mut()
            .detach(&identity, watch_id, authority)
    }

    /// Reads one bounded caller-owned Watch delta from its isolated window.
    pub fn read_watch(
        &self,
        watch_id: &str,
        cursor: u64,
        maximum_records: u32,
        authority: &ExactWatchUseAuthority,
    ) -> Result<ExactWatchBatch, RuntimeError> {
        let identity = self.session.identity().clone();
        self.watches
            .borrow()
            .read(&identity, watch_id, cursor, maximum_records, authority)
    }

    #[must_use]
    pub fn watch_usage(&self) -> ExactWatchUsage {
        self.watches.borrow().usage()
    }

    pub fn acknowledge_scheduler_events_through(
        &mut self,
        cursor: u64,
    ) -> Result<(), RuntimeError> {
        self.session
            .acknowledge_scheduler_events_through(cursor)
            .map_err(|error| RuntimeError::new(error.code(), error.to_string()))
    }

    /// Commits one bounded exact-evidence batch and releases its scheduler
    /// prefix only after the plan-selected provider returns an exact receipt.
    pub fn drain_exact_evidence(
        &mut self,
        cursor: u64,
        maximum_events: u32,
    ) -> Result<ExactEvidenceBatch, ExactEvidenceDrainError> {
        self.session.drain_exact_evidence(cursor, maximum_events)
    }

    #[must_use]
    pub fn exact_evidence(&self) -> Vec<ExactEvidenceRecord> {
        self.session.exact_evidence()
    }

    /// Releases this hosted session only after the executor reached a terminal
    /// state. A nonterminal error leaves the session unchanged.
    pub fn finalize(&mut self) -> Result<(), ExactRunState> {
        match self.session.finalize() {
            Ok(_) => Ok(()),
            Err(state) => Err(state),
        }
    }

    /// Appends bounded input to this active session's owned host-I/O store.
    pub fn push_input(&self, bytes: &[u8]) -> Result<(), RuntimeError> {
        self.io.borrow_mut().push_input(bytes)
    }

    /// Reads one bounded host-I/O snapshot without exposing the mutable store.
    pub fn with_io<T>(&self, operation: impl FnOnce(&ExactRunIo) -> T) -> T {
        operation(&self.io.borrow())
    }

    fn take_scheduler_error(&self, error: SchedulerError) -> RuntimeError {
        self.host_failure
            .borrow_mut()
            .take()
            .unwrap_or_else(|| RuntimeError::new(error.code(), error.to_string()))
    }
}

pub trait Handler {
    /// Installs the exact executor binding for this provider. Pure providers
    /// need no binding; effectful providers override this and fail closed when
    /// execution is attempted without it.
    fn bind_exact(&mut self, _binding: ExactHostedServiceBinding) -> Result<(), RuntimeError> {
        Ok(())
    }

    /// Validates and binds the provider during the scheduler prepare phase.
    fn prepare(
        &mut self,
        _node: &Node,
        binding: ExactHostedServiceBinding,
    ) -> Result<(), RuntimeError> {
        self.bind_exact(binding)
    }

    /// Activates an already prepared provider when the exact run starts.
    fn start(&mut self, _node: &Node) -> Result<(), RuntimeError> {
        Ok(())
    }

    /// Performs one bounded provider step through the one current hosted
    /// adapter. Existing finite request/response handlers inherit this
    /// implementation; long-lived providers override it to produce values or
    /// register one exact wake interest at a time.
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        self.run(node, inputs, io).map(HostedServiceStep::completed)
    }

    /// Begins the provider's bounded stop disposition.
    fn cancel(
        &mut self,
        _node: &Node,
        _stop: conduit_core::StopPolicy,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }

    /// Releases provider-owned finite resources after cancellation or natural
    /// completion.
    fn cleanup(
        &mut self,
        _node: &Node,
        _context: HostedServiceStepContext,
    ) -> Result<HostedServiceCleanup, RuntimeError> {
        Ok(HostedServiceCleanup::Complete)
    }

    /// Finite convenience implementation used by the default `step` adapter.
    /// Long-lived providers override `step` and need not implement this.
    fn run(
        &mut self,
        _node: &Node,
        _inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        Err(RuntimeError::new(
            "CND-RUN-004",
            "hosted provider implements neither a bounded step nor finite run",
        ))
    }
}

pub type HandlerFactory = fn() -> Box<dyn Handler>;
pub type ConfigValidator = fn(&Node) -> Result<(), ResolutionError>;
type OwnedHandlerFactory = Arc<dyn Fn() -> Box<dyn Handler> + Send + Sync>;
type OwnedConfigValidator =
    Arc<dyn for<'a> Fn(&'a Node) -> Result<(), ResolutionError> + Send + Sync>;

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

/// One exact artifact offered by a generically installed implementation.
///
/// Discovery and observation happen outside the registry. This value carries
/// only already-observed, content-addressed facts into installation; adding it
/// cannot execute discovery, initialize a provider, or grant authority.
pub struct InstalledArtifactRegistration {
    pub id: String,
    pub digest: ArtifactDigest,
    pub media_type: String,
    pub byte_size: u64,
    pub target: Option<String>,
    pub abi: Option<String>,
    pub builder: String,
    pub source_digest: ArtifactDigest,
    pub build_recipe_digest: ArtifactDigest,
    pub reproducible: bool,
    pub license_expressions: Vec<String>,
    pub role: String,
    pub required: bool,
}

/// One host capability an installed implementation requires before it may be
/// selected. The implementation declares the predicate; a caller-owned host
/// observation supplies (or omits) the matching current fact independently.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledCapabilityRequirement {
    pub interface: PinnedDescriptor<'static>,
    pub mode: String,
    pub subject: Option<String>,
    pub details: Option<SemanticHash>,
    pub minimum_capacity: conduit_core::PlanResourceBudget,
}

/// Generic installation request for any implementation of any semantic node.
///
/// Contracts, implementation manifests, artifacts, adapters, and authority
/// requirements remain distinct. Domain crates supply the semantic contract
/// and adapter callback; host code supplies observed artifact facts. The same
/// resolver path is used for native, process, WASM, FFI, firmware, and remote
/// implementations.
pub struct InstalledImplementationRegistration<F = HandlerFactory, V = ConfigValidator> {
    pub contract: &'static NodeContract<'static>,
    pub implementation_id: String,
    pub implementation_version: String,
    pub executor: ExecutorKind,
    pub entrypoint_name: String,
    pub entrypoint_adapter: String,
    pub entrypoint_abi: String,
    pub entrypoint_protocol_version: u32,
    pub execution_profile: PinnedDescriptor<'static>,
    pub artifacts: Vec<InstalledArtifactRegistration>,
    /// Host-observed capabilities required by this implementation. Installing
    /// the implementation never manufactures the matching observation.
    pub required_capabilities: Vec<InstalledCapabilityRequirement>,
    pub required_authorities: Vec<SemanticHash>,
    pub required_effects: Vec<SemanticHash>,
    pub minimum_plan_version: u32,
    pub maximum_plan_version: u32,
    pub minimum_runtime_protocol: u32,
    pub maximum_runtime_protocol: u32,
    pub coexistence_memory_bytes: u64,
    /// Optional portable managed-component facet offered by this exact
    /// implementation. It is hashed into the implementation manifest as a
    /// provided interface and never inferred from executor or process type.
    pub managed_lifecycle: Option<ManagedComponentDescriptor>,
    pub factory: F,
    pub validate_config: V,
}

pub struct RegisteredExecutable {
    pub manifest: &'static ImplementationManifest<'static>,
    pub artifacts: &'static [&'static ArtifactManifest<'static>],
    pub implementation: HostedPrimitiveImplementation,
    pub managed_lifecycle: Option<&'static ManagedComponentDescriptor>,
    required_capabilities: &'static [InstalledCapabilityRequirement],
    factory: OwnedHandlerFactory,
    validate_config: OwnedConfigValidator,
}

impl fmt::Debug for RegisteredExecutable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegisteredExecutable")
            .field("manifest", &self.manifest.id)
            .field("artifacts", &self.artifacts.len())
            .field("implementation", &self.implementation)
            .field("managed_lifecycle", &self.managed_lifecycle)
            .field("required_capabilities", &self.required_capabilities)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct CompatibilityExecutable {
    factory: HandlerFactory,
    validate_config: ConfigValidator,
}

#[derive(Debug)]
struct RegisteredNode {
    contract: &'static NodeContract<'static>,
    executables: Vec<RegisteredExecutable>,
    compatibility_executable: Option<CompatibilityExecutable>,
}

/// Exact compiled-in provider facts independently trusted by the hosted runtime.
#[derive(Clone)]
pub struct InstalledHostedProvider {
    pub contract: &'static NodeContract<'static>,
    pub manifest: &'static ImplementationManifest<'static>,
    /// Complete exact artifact set named by the implementation manifest.
    pub artifacts: &'static [&'static ArtifactManifest<'static>],
    /// Primary executable artifact selected for the plan node binding.
    pub artifact: &'static ArtifactManifest<'static>,
    pub implementation: HostedPrimitiveImplementation,
    pub managed_lifecycle: Option<&'static ManagedComponentDescriptor>,
    pub required_capabilities: &'static [InstalledCapabilityRequirement],
    validate_config: OwnedConfigValidator,
}

impl fmt::Debug for InstalledHostedProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InstalledHostedProvider")
            .field("contract", &self.contract.id)
            .field("manifest", &self.manifest.id)
            .field("artifacts", &self.artifacts.len())
            .field("artifact", &self.artifact.id)
            .field("implementation", &self.implementation)
            .field("managed_lifecycle", &self.managed_lifecycle)
            .field("required_capabilities", &self.required_capabilities)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
struct HostedProviderDefinition {
    installed: InstalledHostedProvider,
    artifacts: &'static [&'static ArtifactManifest<'static>],
    factory: HandlerFactory,
}

impl RegisteredNode {
    fn select_executable(&self, source: &Node) -> Option<&RegisteredExecutable> {
        self.executables
            .iter()
            .find(|executable| (executable.validate_config)(source).is_ok())
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
    /// Validates package-resolved semantic bindings against the exact
    /// descriptors known to this checker, then resolves contract topology.
    ///
    /// Package import success remains independent of installed providers,
    /// host observations, artifact acquisition, grants, and authority.
    pub fn resolve_package_contracts<'a>(
        &'a self,
        imports: &'a conduit_panel::PackageImportResolution,
    ) -> Result<ResolvedPanel<'a>, ResolutionError> {
        for binding in imports.bindings() {
            let expected = match binding.kind {
                conduit_panel::ContractExportKind::Node
                | conduit_panel::ContractExportKind::Composite
                | conduit_panel::ContractExportKind::Adapter => self
                    .get_registered_node(&binding.canonical_id)
                    .map(|registered| {
                        OwnedNodeSchema::from_contract(registered.contract)
                            .semantic_hash()
                            .to_string()
                    }),
                conduit_panel::ContractExportKind::Interface => self
                    .interfaces
                    .get(&binding.canonical_id)
                    .map(|interface| interface.semantic_hash.to_string()),
                conduit_panel::ContractExportKind::Type => self
                    .type_reference(&binding.canonical_id)
                    .map(|reference| reference.semantic_hash.to_string()),
            }
            .ok_or_else(|| {
                ResolutionError::new(
                    "CND-IPK-004",
                    format!(
                        "imported canonical contract `{}` is not known to the supplied checker catalog",
                        binding.canonical_id
                    ),
                )
            })?;
            if expected != binding.descriptor_hash {
                return Err(ResolutionError::new(
                    "CND-IPK-005",
                    format!(
                        "imported canonical contract `{}` descriptor differs: package `{}`, checker `{expected}`",
                        binding.canonical_id, binding.descriptor_hash
                    ),
                ));
            }
        }
        self.resolve_contracts(imports.panel())
    }

    /// Register one linked, source-attested host-service implementation.
    ///
    /// The returned executable identity is derived from the exact semantic
    /// contract and linked source bytes; callers cannot substitute a planner
    /// candidate name for installed code.
    pub fn register_compiled_in_host_service(
        &mut self,
        service: CompiledInHostService,
    ) -> Result<(), RegistryError> {
        self.register_compiled_in_host_primitive_with_lifecycle(
            service,
            HostedPrimitiveImplementation::HostedService,
            None,
        )
    }

    /// Registers one compiled-in implementation that explicitly offers the
    /// portable managed-component facet. Execution still uses the same hosted
    /// provider adapter; this descriptor adds observation and request facts,
    /// not another runtime.
    pub fn register_managed_compiled_in_host_service(
        &mut self,
        service: CompiledInHostService,
        descriptor: ManagedComponentDescriptor,
    ) -> Result<(), RegistryError> {
        self.register_compiled_in_host_primitive_with_lifecycle(
            service,
            HostedPrimitiveImplementation::HostedService,
            Some(descriptor),
        )
    }

    /// Register one linked provider against an explicit hosted primitive.
    ///
    /// The association is an installed implementation fact, never inferred
    /// from the semantic contract identity.
    pub fn register_compiled_in_host_primitive(
        &mut self,
        service: CompiledInHostService,
        implementation: HostedPrimitiveImplementation,
    ) -> Result<(), RegistryError> {
        self.register_compiled_in_host_primitive_with_lifecycle(service, implementation, None)
    }

    fn register_compiled_in_host_primitive_with_lifecycle(
        &mut self,
        service: CompiledInHostService,
        implementation: HostedPrimitiveImplementation,
        managed_lifecycle: Option<ManagedComponentDescriptor>,
    ) -> Result<(), RegistryError> {
        let source_digest = ArtifactDigest::from_bytes(Sha256::digest(service.source_bytes).into());
        let mut artifact = ArtifactManifest {
            schema_version: 0,
            identity: SemanticHash::from_bytes([0; 32]),
            id: Id(service.artifact_id),
            digest: source_digest,
            media_type: "application/vnd.conduit.compiled-in-provider",
            byte_size: u64::try_from(service.source_bytes.len()).map_err(|_| RegistryError {
                code: "CND-REG-008",
                message: "linked host-service artifact is too large".to_owned(),
            })?,
            target: Some(Id(std::env::consts::ARCH)),
            abi: Some(Id("conduit/rust-in-process")),
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
        let managed_lifecycle = managed_lifecycle
            .map(|descriptor| {
                descriptor.validate().map_err(|error| RegistryError {
                    code: "CND-REG-007",
                    message: error.to_string(),
                })?;
                Ok::<_, RegistryError>(&*Box::leak(Box::new(descriptor)))
            })
            .transpose()?;
        let provided_interfaces: &'static [ManifestInterface<'static>] =
            managed_lifecycle.map_or(&[], |descriptor| {
                Box::leak(Box::new([
                    ManifestInterface {
                        interface: PinnedDescriptor {
                            id: Id(MANAGED_COMPONENT_INTERFACE_ID),
                            schema_version: MANAGED_COMPONENT_SCHEMA_VERSION,
                            semantic_hash: managed_component_interface_hash(),
                        },
                        entrypoint: Id(service.entrypoint),
                    },
                    ManifestInterface {
                        interface: PinnedDescriptor {
                            id: Id(descriptor.id.as_str()),
                            schema_version: descriptor.schema_version,
                            semantic_hash: descriptor
                                .semantic_hash()
                                .expect("validated managed descriptor has a semantic hash"),
                        },
                        entrypoint: Id(service.entrypoint),
                    },
                ]))
            });
        let mut manifest = ImplementationManifest {
            schema_version: 0,
            identity: SemanticHash::from_bytes([0; 32]),
            id: Id(service.implementation_id),
            implementation_version: "1",
            semantic_contract: PinnedDescriptor {
                id: service.contract.id,
                schema_version: 0,
                semantic_hash: OwnedNodeSchema::from_contract(service.contract).semantic_hash(),
            },
            executor: ExecutorKind::NativeInProcess,
            entrypoint: ManifestEntrypoint {
                name: Id(service.entrypoint),
                adapter: Id("conduit/host-service-step"),
                abi: Id("conduit/host-service"),
                protocol_version: 0,
            },
            execution_profile: PinnedDescriptor {
                id: Id("conduit/hosted-primitive-profile"),
                schema_version: 0,
                semantic_hash: SemanticHash::from_bytes(
                    Sha256::digest(b"conduit/hosted-primitive-profile").into(),
                ),
            },
            artifacts: references,
            required_interfaces: &[],
            provided_interfaces,
            required_authorities: service.required_authorities,
            required_effects: &[],
            minimum_plan_version: 0,
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
        self.register_executable_provider_with_implementation(
            service.contract,
            RegisteredExecutable {
                manifest,
                artifacts,
                required_capabilities: &[],
                factory: Arc::new(service.factory),
                validate_config: Arc::new(service.validate_config),
                implementation,
                managed_lifecycle,
            },
        )
    }

    /// Returns the finite executable provider inventory registered in this
    /// exact host registry.
    #[must_use]
    pub fn installed_providers(&self) -> Vec<InstalledHostedProvider> {
        let mut providers = Vec::new();
        for node in self.nodes.values() {
            for executable in &node.executables {
                let Some(artifact_ref) = executable.manifest.artifacts.first() else {
                    continue;
                };
                let artifact = executable
                    .artifacts
                    .iter()
                    .copied()
                    .find(|artifact| {
                        artifact.id == artifact_ref.id && artifact.digest == artifact_ref.digest
                    })
                    .expect("registered executable references a known artifact");
                providers.push(InstalledHostedProvider {
                    contract: node.contract,
                    manifest: executable.manifest,
                    artifacts: executable.artifacts,
                    artifact,
                    implementation: executable.implementation,
                    managed_lifecycle: executable.managed_lifecycle,
                    required_capabilities: executable.required_capabilities,
                    validate_config: Arc::clone(&executable.validate_config),
                });
            }
        }
        providers
    }

    /// Returns only implementations whose declared supported subset accepts
    /// every authored instance of their contract in this exact source.
    ///
    /// Inventory and source compatibility are different facts: an installed
    /// implementation must not become a compile candidate merely because it
    /// implements the same contract for some other profile.
    pub fn installed_providers_for_panel(
        &self,
        panel: &Panel,
    ) -> Result<Vec<InstalledHostedProvider>, ResolutionError> {
        let expanded = expand_panel(panel, self)?;
        Ok(self
            .installed_providers()
            .into_iter()
            .filter(|provider| {
                expanded
                    .nodes
                    .iter()
                    .filter(|source| {
                        self.get_registered_node(&source.kind)
                            .is_some_and(|node| node.contract.id == provider.contract.id)
                    })
                    .all(|source| (provider.validate_config)(source).is_ok())
            })
            .collect())
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
        self.register_executable_provider_with_implementation(
            contract,
            RegisteredExecutable {
                manifest,
                artifacts,
                required_capabilities: &[],
                factory: Arc::new(factory),
                validate_config: Arc::new(validate_config),
                implementation: HostedPrimitiveImplementation::HostedService,
                managed_lifecycle: None,
            },
        )
    }

    /// Installs one exact implementation through the shared implementation
    /// manifest and artifact path.
    ///
    /// This is the owned hosted convenience for implementations observed at
    /// runtime (for example an exact executable, WASM component, dynamic
    /// library, firmware image, or remote adapter artifact). It deliberately
    /// does not discover artifacts or create host observations, conformance
    /// results, resources, effects, or grants.
    pub fn register_installed_implementation<F, V>(
        &mut self,
        registration: InstalledImplementationRegistration<F, V>,
    ) -> Result<(), RegistryError>
    where
        F: Fn() -> Box<dyn Handler> + Send + Sync + 'static,
        V: for<'a> Fn(&'a Node) -> Result<(), ResolutionError> + Send + Sync + 'static,
    {
        fn leak(value: String) -> &'static str {
            Box::leak(value.into_boxed_str())
        }

        if registration.artifacts.is_empty() {
            return Err(RegistryError {
                code: "CND-REG-008",
                message: "installed implementation has no exact artifact".to_owned(),
            });
        }

        let mut artifacts = Vec::with_capacity(registration.artifacts.len());
        let mut references = Vec::with_capacity(registration.artifacts.len());
        for observed in registration.artifacts {
            let id = Id(leak(observed.id));
            let role = Id(leak(observed.role));
            let licenses = observed
                .license_expressions
                .into_iter()
                .map(leak)
                .collect::<Vec<_>>();
            let licenses: &'static [&'static str] = Box::leak(licenses.into_boxed_slice());
            let mut artifact = ArtifactManifest {
                schema_version: 0,
                identity: SemanticHash::from_bytes([0; 32]),
                id,
                digest: observed.digest,
                media_type: leak(observed.media_type),
                byte_size: observed.byte_size,
                target: observed.target.map(leak).map(Id),
                abi: observed.abi.map(leak).map(Id),
                provenance: ArtifactProvenance {
                    builder: Id(leak(observed.builder)),
                    source_digest: observed.source_digest,
                    build_recipe_digest: observed.build_recipe_digest,
                    reproducible: observed.reproducible,
                },
                signatures: &[],
                license_expressions: licenses,
                notices: &[],
                sbom: None,
                source: None,
                related_artifacts: &[],
                locations: &[],
            };
            let mut scratch =
                vec![SemanticHash::from_bytes([0; 32]); artifact.identity_fact_count()];
            artifact.identity =
                artifact
                    .computed_semantic_hash(&mut scratch)
                    .map_err(|_| RegistryError {
                        code: "CND-REG-008",
                        message: format!("installed artifact `{id}` has an invalid identity"),
                    })?;
            let artifact = &*Box::leak(Box::new(artifact));
            references.push(ManifestArtifactRef {
                id,
                digest: observed.digest,
                role,
                required: observed.required,
            });
            artifacts.push(artifact);
        }
        let artifacts: &'static [&'static ArtifactManifest<'static>] =
            Box::leak(artifacts.into_boxed_slice());
        let references: &'static [ManifestArtifactRef<'static>] =
            Box::leak(references.into_boxed_slice());
        let required_authorities: &'static [SemanticHash] =
            Box::leak(registration.required_authorities.into_boxed_slice());
        let required_effects: &'static [SemanticHash] =
            Box::leak(registration.required_effects.into_boxed_slice());
        let required_capabilities: &'static [InstalledCapabilityRequirement] =
            Box::leak(registration.required_capabilities.into_boxed_slice());
        let entrypoint_name = leak(registration.entrypoint_name);
        let managed_lifecycle = registration
            .managed_lifecycle
            .map(|descriptor| {
                descriptor.validate().map_err(|error| RegistryError {
                    code: "CND-REG-010",
                    message: error.to_string(),
                })?;
                Ok::<_, RegistryError>(&*Box::leak(Box::new(descriptor)))
            })
            .transpose()?;
        let provided_interfaces: &'static [ManifestInterface<'static>] =
            managed_lifecycle.map_or(&[], |descriptor| {
                Box::leak(Box::new([
                    ManifestInterface {
                        interface: PinnedDescriptor {
                            id: Id(MANAGED_COMPONENT_INTERFACE_ID),
                            schema_version: MANAGED_COMPONENT_SCHEMA_VERSION,
                            semantic_hash: managed_component_interface_hash(),
                        },
                        entrypoint: Id(entrypoint_name),
                    },
                    ManifestInterface {
                        interface: PinnedDescriptor {
                            id: Id(descriptor.id.as_str()),
                            schema_version: descriptor.schema_version,
                            semantic_hash: descriptor
                                .semantic_hash()
                                .expect("validated managed descriptor has a semantic hash"),
                        },
                        entrypoint: Id(entrypoint_name),
                    },
                ]))
            });
        let mut manifest = ImplementationManifest {
            schema_version: 0,
            identity: SemanticHash::from_bytes([0; 32]),
            id: Id(leak(registration.implementation_id)),
            implementation_version: leak(registration.implementation_version),
            semantic_contract: PinnedDescriptor {
                id: registration.contract.id,
                schema_version: 0,
                semantic_hash: OwnedNodeSchema::from_contract(registration.contract)
                    .semantic_hash(),
            },
            executor: registration.executor,
            entrypoint: ManifestEntrypoint {
                name: Id(entrypoint_name),
                adapter: Id(leak(registration.entrypoint_adapter)),
                abi: Id(leak(registration.entrypoint_abi)),
                protocol_version: registration.entrypoint_protocol_version,
            },
            execution_profile: registration.execution_profile,
            artifacts: references,
            required_interfaces: &[],
            provided_interfaces,
            required_authorities,
            required_effects,
            minimum_plan_version: registration.minimum_plan_version,
            maximum_plan_version: registration.maximum_plan_version,
            minimum_runtime_protocol: registration.minimum_runtime_protocol,
            maximum_runtime_protocol: registration.maximum_runtime_protocol,
            replacement: ReplacementSupport::Cold,
            coexistence_memory_bytes: registration.coexistence_memory_bytes,
            reproducibility: None,
        };
        let mut scratch = vec![SemanticHash::from_bytes([0; 32]); manifest.identity_fact_count()];
        manifest.identity =
            manifest
                .computed_semantic_hash(&mut scratch)
                .map_err(|_| RegistryError {
                    code: "CND-REG-007",
                    message: format!(
                        "installed implementation `{}` has an invalid identity",
                        manifest.id
                    ),
                })?;
        let manifest = &*Box::leak(Box::new(manifest));
        self.register_executable_provider_with_implementation(
            registration.contract,
            RegisteredExecutable {
                manifest,
                artifacts,
                required_capabilities,
                factory: Arc::new(registration.factory),
                validate_config: Arc::new(registration.validate_config),
                implementation: HostedPrimitiveImplementation::HostedService,
                managed_lifecycle,
            },
        )
    }

    fn register_executable_provider_with_implementation(
        &mut self,
        contract: &'static NodeContract<'static>,
        executable: RegisteredExecutable,
    ) -> Result<(), RegistryError> {
        let RegisteredExecutable {
            manifest,
            artifacts,
            implementation,
            managed_lifecycle,
            required_capabilities,
            factory,
            validate_config,
        } = executable;
        let canonical_target_id = self.resolve_canonical_id(contract.id.as_str())?.to_owned();
        let manifest_target_canonical = self
            .resolve_canonical_id(manifest.semantic_contract.id.as_str())?
            .to_owned();

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

        let registered = self
            .nodes
            .get_mut(canonical_target_id.as_str())
            .expect("canonical contract should exist after resolution");
        registered.executables.push(RegisteredExecutable {
            manifest,
            artifacts,
            implementation,
            managed_lifecycle,
            required_capabilities,
            factory,
            validate_config,
        });
        Ok(())
    }

    /// Registers a semantic contract as contract-only.
    pub fn register_contract_only(&mut self, contract: &'static NodeContract<'static>) {
        self.nodes
            .entry(contract.id.as_str())
            .or_insert_with(|| RegisteredNode {
                contract,
                executables: Vec::new(),
                compatibility_executable: None,
            });
    }

    /// Returns semantic port metadata known for one authored contract without
    /// requiring the surrounding graph to resolve.
    #[must_use]
    pub fn authored_node_view(&self, panel: &Panel, contract_id: &str) -> Option<AuthoredNodeView> {
        fn project(
            registry: &Registry,
            panel: &Panel,
            contract_id: &str,
            visiting: &mut Vec<String>,
        ) -> Option<AuthoredNodeView> {
            if let Some(registered) = registry.get_registered_node(contract_id) {
                return Some(AuthoredNodeView {
                    contract_id: registered.contract.id.as_str().to_owned(),
                    contract_identity: Some(
                        OwnedNodeSchema::from_contract(registered.contract)
                            .semantic_hash()
                            .to_string(),
                    ),
                    inputs: registered
                        .contract
                        .inputs
                        .iter()
                        .map(resolved_port_view)
                        .collect(),
                    outputs: registered
                        .contract
                        .outputs
                        .iter()
                        .map(resolved_port_view)
                        .collect(),
                });
            }
            if visiting.iter().any(|candidate| candidate == contract_id) {
                return None;
            }
            let definition = panel
                .definitions
                .iter()
                .find(|definition| definition.id == contract_id)?;
            visiting.push(contract_id.to_owned());
            let mut inputs = Vec::new();
            let mut outputs = Vec::new();
            for export in &definition.exports {
                let target = definition
                    .nodes
                    .iter()
                    .find(|child| child.id == export.target.node)
                    .and_then(|child| project(registry, panel, &child.kind, visiting))
                    .and_then(|child| {
                        let ports = match export.direction {
                            ExportDirection::Input => child.inputs,
                            ExportDirection::Output => child.outputs,
                        };
                        ports.into_iter().find(|port| port.id == export.target.port)
                    });
                let mut view = target.unwrap_or_else(|| ResolvedPortView {
                    id: export.target.port.clone(),
                    type_id: "unresolved-composite-export".to_owned(),
                    delivery: "unknown",
                    connections: "unknown",
                    values: "unknown",
                    temporal: "unknown",
                    terminal: "unknown",
                    presence: "unknown",
                    sensitivity: "unknown",
                    loss_acceptance: "unknown",
                });
                view.id.clone_from(&export.id);
                match export.direction {
                    ExportDirection::Input => inputs.push(view),
                    ExportDirection::Output => outputs.push(view),
                }
            }
            visiting.pop();
            Some(AuthoredNodeView {
                contract_id: contract_id.to_owned(),
                // The enclosing source owns a composite definition identity;
                // no primitive registry identity is substituted here.
                contract_identity: None,
                inputs,
                outputs,
            })
        }
        project(self, panel, contract_id, &mut Vec::new())
    }

    /// Checks one complete authored cord and reports an element-scoped result
    /// even when another cord prevents whole-graph resolution.
    #[must_use]
    pub fn assess_authored_cord(&self, panel: &Panel, cord: &Cord) -> AuthoredCordAssessment {
        let unique_node = |id: &str| {
            let mut matches = panel.nodes.iter().filter(|node| node.id == id);
            let first = matches.next();
            (first, matches.next().is_none())
        };
        let (from_node, from_unique) = unique_node(&cord.from.node);
        let (to_node, to_unique) = unique_node(&cord.to.node);
        if !from_unique || !to_unique {
            return authored_cord_failure(
                "invalid",
                "CND-ID-002",
                "a cord endpoint names a duplicate node",
                "Duplicate node identities prevent this endpoint from naming one authored node.",
            );
        }
        let Some(from_node) = from_node else {
            return authored_cord_failure(
                "unresolved",
                "CND-ID-003",
                format!("unknown source node `{}`", cord.from.node),
                "The cord source names no authored node in the current revision.",
            );
        };
        let Some(to_node) = to_node else {
            return authored_cord_failure(
                "unresolved",
                "CND-ID-003",
                format!("unknown destination node `{}`", cord.to.node),
                "The cord destination names no authored node in the current revision.",
            );
        };
        let Some(from_view) = self.authored_node_view(panel, &from_node.kind) else {
            return authored_cord_failure(
                "unresolved",
                "CND-IMP-001",
                format!("unresolved source contract `{}`", from_node.kind),
                "The source contract is not known, so no output port is inferred.",
            );
        };
        let Some(to_view) = self.authored_node_view(panel, &to_node.kind) else {
            return authored_cord_failure(
                "unresolved",
                "CND-IMP-001",
                format!("unresolved destination contract `{}`", to_node.kind),
                "The destination contract is not known, so no receiving port is inferred.",
            );
        };
        if !from_view
            .outputs
            .iter()
            .any(|port| port.id == cord.from.port)
        {
            let wrong_direction = from_view
                .inputs
                .iter()
                .any(|port| port.id == cord.from.port);
            return authored_cord_failure(
                if wrong_direction {
                    "wrong-direction"
                } else {
                    "unresolved"
                },
                "CND-CMP-003",
                format!(
                    "invalid source endpoint `{}.{}`",
                    cord.from.node, cord.from.port
                ),
                if wrong_direction {
                    "Receiving port used as a source; a cord must begin at an outgoing port."
                } else {
                    "The explicit source port is unknown or is not exported by this contract."
                },
            );
        }
        if !to_view.inputs.iter().any(|port| port.id == cord.to.port) {
            let wrong_direction = to_view.outputs.iter().any(|port| port.id == cord.to.port);
            return authored_cord_failure(
                if wrong_direction {
                    "wrong-direction"
                } else {
                    "unresolved"
                },
                "CND-CMP-003",
                format!(
                    "dangling or wrong-direction port mapping `{}.{}`",
                    cord.to.node, cord.to.port
                ),
                if wrong_direction {
                    "Outgoing port used as destination; a cord must terminate at a receiving port."
                } else {
                    "The explicit destination port is unknown or is not exported by this contract."
                },
            );
        }
        if let Err(error) = resolve_flow(cord) {
            return authored_cord_failure(
                "invalid-bounds",
                error.code,
                error.message,
                "The authored capacity, watermarks, byte limits, or pressure policy is invalid.",
            );
        }
        let from_contract = self.get_registered_node(&from_node.kind).and_then(|node| {
            node.contract
                .outputs
                .iter()
                .find(|port| port.id.as_str() == cord.from.port)
        });
        let to_contract = self.get_registered_node(&to_node.kind).and_then(|node| {
            node.contract
                .inputs
                .iter()
                .find(|port| port.id.as_str() == cord.to.port)
        });
        if let (Some(producer), Some(consumer)) = (from_contract, to_contract) {
            let decision = assess_port_connection(
                *consumer,
                *producer,
                assess_type_contract_exact(consumer.value_type, producer.value_type),
            );
            if decision.outcome != CompatibilityOutcome::Compatible {
                return AuthoredCordAssessment {
                    state: "incompatible",
                    code: "CND-TYP-001",
                    message: format!(
                        "complete PortContracts are incompatible: {}",
                        decision.reason.as_str()
                    ),
                    explanation:
                        "The complete directional PortContracts are incompatible even if their payload names look similar."
                            .to_owned(),
                    producer_type: Some(producer.value_type.contract_id.as_str().to_owned()),
                    consumer_type: Some(consumer.value_type.contract_id.as_str().to_owned()),
                };
            }
            return AuthoredCordAssessment {
                state: "valid",
                code: "CND-TYP-EXACT",
                message: "complete PortContracts are compatible".to_owned(),
                explanation: "The authored output-to-input connection is compatible.".to_owned(),
                producer_type: Some(producer.value_type.contract_id.as_str().to_owned()),
                consumer_type: Some(consumer.value_type.contract_id.as_str().to_owned()),
            };
        }
        AuthoredCordAssessment {
            state: "valid",
            code: "CND-TYP-EXACT",
            message: "authored composite boundary directions are valid".to_owned(),
            explanation: "The authored connection uses exported source and destination ports."
                .to_owned(),
            producer_type: None,
            consumer_type: None,
        }
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
            let expected_hash = OwnedNodeSchema::from_contract(registered.contract).semantic_hash();
            let mut reasons = Vec::new();
            if registered.executables.is_empty() {
                reasons.push("CND-RES-008".to_owned());
            }
            for executable in &registered.executables {
                let manifest_canonical = self
                    .resolve_canonical_id(executable.manifest.semantic_contract.id.as_str())
                    .unwrap_or(executable.manifest.semantic_contract.id.as_str());

                if manifest_canonical != canonical_id {
                    reasons.push("CND-RES-008".to_owned());
                } else if executable.manifest.semantic_contract.semantic_hash != expected_hash {
                    reasons.push("CND-RES-002".to_owned());
                } else {
                    return NodeAvailability {
                        contract_id: canonical_id.to_owned(),
                        state: AvailabilityState::ProviderAvailable,
                        reason_code: "CND-AVL-002".to_owned(),
                        implementation_id: Some(executable.manifest.id.to_string()),
                        host_id: None,
                        plan_identity: None,
                        run_id: None,
                        rejection_reasons: vec!["CND-RES-025".to_owned()],
                    };
                }
            }
            if reasons.is_empty() {
                reasons.push("CND-RES-008".to_owned());
            }
            if registered.compatibility_executable.is_some() {
                reasons.push("CND-RES-025".to_owned());
            }
            // Contract-only or invalid provider facts; no executable satisfies
            // availability requirements.
            NodeAvailability {
                contract_id: canonical_id.to_owned(),
                state: AvailabilityState::ContractOnly,
                reason_code: "CND-AVL-001".to_owned(),
                implementation_id: None,
                host_id: None,
                plan_identity: None,
                run_id: None,
                rejection_reasons: reasons,
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
            &STDIN_STREAM_CONTRACT,
            || Box::new(Stdin),
            validate_empty_config,
        );
        install(
            &UPPERCASE_CONTRACT,
            || Box::new(Uppercase),
            validate_empty_config,
        );
        install(
            standard_data_contract("std/record/literal"),
            || Box::new(RecordLiteral),
            validate_record_literal,
        );
        install(
            standard_data_contract("std/data/validate-closed-record"),
            || Box::new(ValidateClosedRecord),
            validate_closed_record_config,
        );
        install(
            standard_data_contract("std/testing/assert-validation-decision"),
            || Box::new(ValidationDecisionAssert),
            validate_validation_decision_assert,
        );
        install(
            standard_data_contract("std/data/encode-utf8"),
            || Box::new(EncodeUtf8),
            validate_data_codec,
        );
        install(
            standard_data_contract("std/data/decode-utf8"),
            || Box::new(DecodeUtf8),
            validate_data_codec,
        );
        install(
            standard_data_contract("std/data/frame-length-u32be"),
            || Box::new(FrameLengthU32Be),
            validate_data_framing,
        );
        install(
            standard_data_contract("std/data/deframe-length-u32be"),
            || Box::new(DeframeLengthU32Be),
            validate_data_framing,
        );
        install(
            standard_time_contract("time/ticker"),
            || Box::new(Ticker::default()),
            validate_ticker,
        );
        for (id, validate) in [
            ("time/delay", validate_time_delay as ConfigValidator),
            ("time/timeout", validate_time_timeout as ConfigValidator),
            ("time/debounce", validate_time_debounce as ConfigValidator),
            ("time/throttle", validate_time_throttle as ConfigValidator),
        ] {
            install(
                standard_time_contract(id),
                || Box::new(TimeCompatibilityHandler),
                validate,
            );
        }
        for (id, validate) in [
            ("state/cell", validate_state_cell as ConfigValidator),
            (
                "state/deduplicate",
                validate_state_deduplicate as ConfigValidator,
            ),
            ("state/cache", validate_state_cache as ConfigValidator),
        ] {
            install(
                standard_state_contract(id),
                || Box::new(StateCompatibilityHandler),
                validate,
            );
        }
        for (id, validate) in [
            (
                "supervision/retry",
                validate_supervision_retry as ConfigValidator,
            ),
            (
                "supervision/circuit-breaker",
                validate_supervision_circuit_breaker as ConfigValidator,
            ),
        ] {
            install(
                standard_supervision_contract(id),
                || Box::new(StateCompatibilityHandler),
                validate,
            );
        }
        install(&STDOUT_CONTRACT, || Box::new(Stdout), validate_empty_config);
        install(&STDERR_CONTRACT, || Box::new(Stderr), validate_empty_config);
        install(
            &STDOUT_STREAM_CONTRACT,
            || Box::new(Stdout),
            validate_empty_config,
        );
        install(
            &STDERR_STREAM_CONTRACT,
            || Box::new(Stderr),
            validate_empty_config,
        );
        install(
            &DISPLAY_TEXT_CONTRACT,
            || Box::new(DisplayText),
            validate_empty_config,
        );
        install(
            &DISCARD_CONTRACT,
            || Box::new(DiscardHandler),
            validate_empty_config,
        );
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
                .register_executable_provider_with_implementation(
                    definition.installed.contract,
                    RegisteredExecutable {
                        manifest: definition.installed.manifest,
                        artifacts: definition.artifacts,
                        required_capabilities: definition.installed.required_capabilities,
                        factory: Arc::new(definition.factory),
                        validate_config: Arc::clone(&definition.installed.validate_config),
                        implementation: definition.installed.implementation,
                        managed_lifecycle: definition.installed.managed_lifecycle,
                    },
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
                    .map(|definition| definition.installed.clone())
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
            schema_version: 0,
            identity: SemanticHash::from_bytes([0; 32]),
            id: Id("conduit/hosted-primitives-artifact"),
            digest: artifact_digest,
            media_type: "application/vnd.conduit.compiled-in-provider",
            byte_size: u64::try_from(source_bytes.len()).expect("source length fits u64"),
            target: Some(Id(std::env::consts::ARCH)),
            abi: Some(Id("conduit/rust-in-process")),
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
            SemanticHash::from_bytes(Sha256::digest(b"conduit/hosted-primitive-profile").into());

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
                &STDIN_STREAM_CONTRACT,
                "stdin-stream",
                HostedPrimitiveImplementation::StdinStream,
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
                standard_data_contract("std/record/literal"),
                "data-record-literal",
                HostedPrimitiveImplementation::RecordLiteral,
                || Box::new(RecordLiteral),
                validate_record_literal,
            ),
            (
                file_chunk_literal_contract(),
                "file-chunk-literal",
                HostedPrimitiveImplementation::FileChunkLiteral,
                || Box::new(FileChunkLiteral),
                validate_file_chunk_literal,
            ),
            (
                standard_data_contract("std/data/validate-closed-record"),
                "data-validate-closed-record",
                HostedPrimitiveImplementation::ValidateClosedRecord,
                || Box::new(ValidateClosedRecord),
                validate_closed_record_config,
            ),
            (
                standard_data_contract("std/testing/assert-validation-decision"),
                "data-assert-validation-decision",
                HostedPrimitiveImplementation::ValidationDecisionAssert,
                || Box::new(ValidationDecisionAssert),
                validate_validation_decision_assert,
            ),
            (
                standard_data_contract("std/data/encode-utf8"),
                "data-encode-utf8",
                HostedPrimitiveImplementation::DataEncodeUtf8,
                || Box::new(EncodeUtf8),
                validate_data_codec,
            ),
            (
                standard_data_contract("std/data/decode-utf8"),
                "data-decode-utf8",
                HostedPrimitiveImplementation::DataDecodeUtf8,
                || Box::new(DecodeUtf8),
                validate_data_codec,
            ),
            (
                standard_data_contract("std/data/frame-length-u32be"),
                "data-frame-length-u32be",
                HostedPrimitiveImplementation::FrameLengthU32Be,
                || Box::new(FrameLengthU32Be),
                validate_data_framing,
            ),
            (
                standard_data_contract("std/data/deframe-length-u32be"),
                "data-deframe-length-u32be",
                HostedPrimitiveImplementation::DeframeLengthU32Be,
                || Box::new(DeframeLengthU32Be),
                validate_data_framing,
            ),
            (
                standard_time_contract("time/ticker"),
                "time-ticker",
                HostedPrimitiveImplementation::Ticker,
                || Box::new(Ticker::default()),
                validate_ticker,
            ),
            (
                standard_time_contract("time/delay"),
                "time-delay",
                HostedPrimitiveImplementation::TimeDelay,
                || Box::new(TimeCompatibilityHandler),
                validate_time_delay,
            ),
            (
                standard_time_contract("time/timeout"),
                "time-timeout",
                HostedPrimitiveImplementation::TimeTimeout,
                || Box::new(TimeCompatibilityHandler),
                validate_time_timeout,
            ),
            (
                standard_time_contract("time/debounce"),
                "time-debounce",
                HostedPrimitiveImplementation::TimeDebounce,
                || Box::new(TimeCompatibilityHandler),
                validate_time_debounce,
            ),
            (
                standard_time_contract("time/throttle"),
                "time-throttle",
                HostedPrimitiveImplementation::TimeThrottle,
                || Box::new(TimeCompatibilityHandler),
                validate_time_throttle,
            ),
            (
                standard_state_contract("state/cell"),
                "state-cell",
                HostedPrimitiveImplementation::StateCell,
                || Box::new(StateCompatibilityHandler),
                validate_state_cell,
            ),
            (
                standard_state_contract("state/deduplicate"),
                "state-deduplicate",
                HostedPrimitiveImplementation::StateDeduplicate,
                || Box::new(StateCompatibilityHandler),
                validate_state_deduplicate,
            ),
            (
                standard_state_contract("state/cache"),
                "state-cache",
                HostedPrimitiveImplementation::StateCache,
                || Box::new(StateCompatibilityHandler),
                validate_state_cache,
            ),
            (
                standard_supervision_contract("supervision/retry"),
                "supervision-retry",
                HostedPrimitiveImplementation::SupervisionRetry,
                || Box::new(StateCompatibilityHandler),
                validate_supervision_retry,
            ),
            (
                standard_supervision_contract("supervision/circuit-breaker"),
                "supervision-circuit-breaker",
                HostedPrimitiveImplementation::SupervisionCircuitBreaker,
                || Box::new(StateCompatibilityHandler),
                validate_supervision_circuit_breaker,
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
                &STDOUT_STREAM_CONTRACT,
                "stdout-stream",
                HostedPrimitiveImplementation::StdoutStream,
                || Box::new(Stdout),
                validate_empty_config,
            ),
            (
                &STDERR_STREAM_CONTRACT,
                "stderr-stream",
                HostedPrimitiveImplementation::StderrStream,
                || Box::new(Stderr),
                validate_empty_config,
            ),
            (
                &DISPLAY_TEXT_CONTRACT,
                "display-text",
                HostedPrimitiveImplementation::DisplayText,
                || Box::new(DisplayText),
                validate_empty_config,
            ),
            (
                &DISCARD_CONTRACT,
                "discard",
                HostedPrimitiveImplementation::Discard,
                || Box::new(DiscardHandler),
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
                        Box::leak(format!("conduit/hosted-{entrypoint}").into_boxed_str());
                    let artifact_references = Box::leak(Box::new([ManifestArtifactRef {
                        id: artifact.id,
                        digest: artifact.digest,
                        role: Id("executable"),
                        required: true,
                    }]));
                    let mut manifest = ImplementationManifest {
                        schema_version: 0,
                        identity: SemanticHash::from_bytes([0; 32]),
                        id: Id(implementation_id),
                        implementation_version: "1",
                        semantic_contract: PinnedDescriptor {
                            id: contract.id,
                            schema_version: 0,
                            semantic_hash: OwnedNodeSchema::from_contract(contract).semantic_hash(),
                        },
                        executor: ExecutorKind::NativeInProcess,
                        entrypoint: ManifestEntrypoint {
                            name: Id(entrypoint),
                            adapter: Id("conduit/hosted-primitive-step"),
                            abi: Id("conduit/hosted-primitive"),
                            protocol_version: 0,
                        },
                        execution_profile: PinnedDescriptor {
                            id: Id("conduit/hosted-primitive-profile"),
                            schema_version: 0,
                            semantic_hash: profile_hash,
                        },
                        artifacts: artifact_references,
                        required_interfaces: &[],
                        provided_interfaces: &[],
                        required_authorities: &[],
                        required_effects: &[],
                        minimum_plan_version: 0,
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
                            artifacts,
                            artifact,
                            implementation: *implementation,
                            managed_lifecycle: None,
                            required_capabilities: &[],
                            validate_config: Arc::new(*validate_config),
                        },
                        artifacts,
                        factory: *factory,
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
                executables: Vec::new(),
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
            STDIN_STREAM_CONTRACT.id.as_str(),
            honest_primitive(
                &STDIN_STREAM_CONTRACT,
                || Box::new(Stdin),
                validate_empty_config,
            ),
        );
        nodes.insert(
            UPPERCASE_CONTRACT.id.as_str(),
            honest_primitive(
                &UPPERCASE_CONTRACT,
                || Box::new(Uppercase),
                validate_empty_config,
            ),
        );
        for (contract, factory, validate) in [
            (
                standard_data_contract("std/record/literal"),
                (|| Box::new(RecordLiteral) as Box<dyn Handler>) as HandlerFactory,
                validate_record_literal as ConfigValidator,
            ),
            (
                file_chunk_literal_contract(),
                (|| Box::new(FileChunkLiteral) as Box<dyn Handler>) as HandlerFactory,
                validate_file_chunk_literal as ConfigValidator,
            ),
            (
                standard_data_contract("std/data/validate-closed-record"),
                (|| Box::new(ValidateClosedRecord) as Box<dyn Handler>) as HandlerFactory,
                validate_closed_record_config as ConfigValidator,
            ),
            (
                standard_data_contract("std/testing/assert-validation-decision"),
                (|| Box::new(ValidationDecisionAssert) as Box<dyn Handler>) as HandlerFactory,
                validate_validation_decision_assert as ConfigValidator,
            ),
            (
                standard_data_contract("std/data/encode-utf8"),
                (|| Box::new(EncodeUtf8) as Box<dyn Handler>) as HandlerFactory,
                validate_data_codec as ConfigValidator,
            ),
            (
                standard_data_contract("std/data/decode-utf8"),
                (|| Box::new(DecodeUtf8) as Box<dyn Handler>) as HandlerFactory,
                validate_data_codec as ConfigValidator,
            ),
            (
                standard_data_contract("std/data/frame-length-u32be"),
                (|| Box::new(FrameLengthU32Be) as Box<dyn Handler>) as HandlerFactory,
                validate_data_framing as ConfigValidator,
            ),
            (
                standard_data_contract("std/data/deframe-length-u32be"),
                (|| Box::new(DeframeLengthU32Be) as Box<dyn Handler>) as HandlerFactory,
                validate_data_framing as ConfigValidator,
            ),
        ] {
            nodes.insert(
                contract.id.as_str(),
                honest_primitive(contract, factory, validate),
            );
        }
        nodes.insert(
            STDOUT_CONTRACT.id.as_str(),
            honest_primitive(&STDOUT_CONTRACT, || Box::new(Stdout), validate_empty_config),
        );
        nodes.insert(
            STDERR_CONTRACT.id.as_str(),
            honest_primitive(&STDERR_CONTRACT, || Box::new(Stderr), validate_empty_config),
        );
        nodes.insert(
            STDOUT_STREAM_CONTRACT.id.as_str(),
            honest_primitive(
                &STDOUT_STREAM_CONTRACT,
                || Box::new(Stdout),
                validate_empty_config,
            ),
        );
        nodes.insert(
            STDERR_STREAM_CONTRACT.id.as_str(),
            honest_primitive(
                &STDERR_STREAM_CONTRACT,
                || Box::new(Stderr),
                validate_empty_config,
            ),
        );
        nodes.insert(
            DISPLAY_TEXT_CONTRACT.id.as_str(),
            honest_primitive(
                &DISPLAY_TEXT_CONTRACT,
                || Box::new(DisplayText),
                validate_empty_config,
            ),
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
            &TAKE_CONTRACT,
            &SKIP_CONTRACT,
            &FILTER_CONTRACT,
            &PROBE_CONTRACT,
            &LOG_CONTRACT,
            &ASSERT_CONTRACT,
            &RECORD_CONTRACT,
            &REPLAY_CONTRACT,
            &FAULT_SOURCE_CONTRACT,
            file_read_contract(),
            file_write_contract(),
            file_watch_contract(),
            &GPIO_PIN_CONTRACT,
            &SERIAL_PORT_CONTRACT,
            standard_state_contract("state/cell"),
            &COUNTER_CONTRACT,
            standard_state_contract("state/deduplicate"),
            standard_state_contract("state/cache"),
            standard_supervision_contract("supervision/retry"),
            standard_supervision_contract("supervision/circuit-breaker"),
            &HEALTH_GATE_CONTRACT,
            &HTTP_LISTENER_CONTRACT,
        ];

        for &contract in contract_only_list {
            nodes.insert(
                contract.id.as_str(),
                RegisteredNode {
                    contract,
                    executables: Vec::new(),
                    compatibility_executable: None,
                },
            );
        }
        for entry in conduit_std::STANDARD_CATALOG {
            nodes
                .entry(entry.contract.id.as_str())
                .or_insert(RegisteredNode {
                    contract: &entry.contract,
                    executables: Vec::new(),
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
            schema_version: 0,
            principal_path: OwnedPrincipalPath::none(),
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
            schema_version: 0,
            principal_path: OwnedPrincipalPath::none(),
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
            let executable = if self.allow_installed_resolution {
                definition
                    .select_executable(&source)
                    // Preserve the provider's configuration diagnostic when
                    // every installed implementation rejects this node.
                    // Registration order is deterministic for one exact
                    // catalog, and no handler can run after validation fails.
                    .or_else(|| definition.executables.first())
            } else {
                None
            };
            if let Some(executable) = executable {
                (executable.validate_config)(&source)?;
            } else if let Some(executable) = &definition.compatibility_executable {
                (executable.validate_config)(&source)?;
            } else if !require_executable {
                validate_contract_config(&source)?;
            } else {
                return Err(ResolutionError::new(
                    "CND-IMP-001",
                    format!("no ready implementation for `{}`", source.kind),
                ));
            }
            nodes.push(ResolvedNode {
                source,
                definition,
                executable,
            });
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
        ("std/id" | "std/reference/any" | "fs/resource", SourceValue::Reference(value))
        | ("std/id" | "std/reference/any" | "fs/resource", SourceValue::ContractReference(value)) =>
        {
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
            schema_version: 0,
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
                format!("recursive definition: {cycle}"),
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
    executable: Option<&'a RegisteredExecutable>,
}

impl<'a> ResolvedNode<'a> {
    fn new_handler(&self) -> Box<dyn Handler> {
        if let Some(executable) = self.executable {
            return (executable.factory)();
        }
        let executable = self
            .definition
            .compatibility_executable
            .as_ref()
            .expect("resolved node has executable implementation");
        (executable.factory)()
    }
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
    pub values: &'static str,
    pub temporal: &'static str,
    pub terminal: &'static str,
    pub presence: &'static str,
    pub sensitivity: &'static str,
    pub loss_acceptance: &'static str,
}

/// Contract facts which are authoritative before a whole authored graph
/// resolves. This contains no implementation, placement, or plan facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoredNodeView {
    pub contract_id: String,
    pub contract_identity: Option<String>,
    pub inputs: Vec<ResolvedPortView>,
    pub outputs: Vec<ResolvedPortView>,
}

/// Element-scoped result of checking one authored cord independently of the
/// rest of the source graph.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoredCordAssessment {
    pub state: &'static str,
    pub code: &'static str,
    pub message: String,
    pub explanation: String,
    pub producer_type: Option<String>,
    pub consumer_type: Option<String>,
}

fn authored_cord_failure(
    state: &'static str,
    code: &'static str,
    message: impl Into<String>,
    explanation: impl Into<String>,
) -> AuthoredCordAssessment {
    AuthoredCordAssessment {
        state,
        code,
        message: message.into(),
        explanation: explanation.into(),
        producer_type: None,
        consumer_type: None,
    }
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
    pub temporal: TemporalContract,
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
        let source_hash = conduit_panel::semantic_source_hash(self.source);
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
            writeln!(explanation, "  instance {}: {}", node.id, node.kind)
                .expect("writing to String cannot fail");
        }
        let mut composites = self.logical_composites.iter().collect::<Vec<_>>();
        composites.sort_by(|left, right| left.path.cmp(&right.path));
        for composite in composites {
            writeln!(
                explanation,
                "  composite {}: {}",
                composite.path, composite.definition
            )
            .expect("writing to String cannot fail");
            for (child_path, definition) in &composite.children {
                writeln!(explanation, "    child {child_path}: {definition}")
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
                "  node {index}: {}: {} -> hosted builtin",
                node.source.id, node.definition.contract.id
            )
            .expect("writing to String cannot fail");
            for port in node.definition.contract.inputs {
                writeln!(
                    explanation,
                    "    input  {}: {} {:?} {:?}",
                    port.id, port.value_type.contract_id, port.delivery, port.connections
                )
                .expect("writing to String cannot fail");
            }
            for port in node.definition.contract.outputs {
                writeln!(
                    explanation,
                    "    output {}: {} {:?} {:?}",
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
        self.run_exact_report_controlled(plan, None, bindings, context, None, io)
    }

    /// Executes only after the separately identified physical arrangement is
    /// proven to match this exact logical plan and run epoch.
    pub fn run_exact_report_arranged<'p, 'r, 'i>(
        &self,
        plan: &'p ExecutionPlan<'p>,
        arrangement: &ResolvedExecutionArrangement,
        bindings: &ExactHostedBindings,
        context: ExactRunContext<'p>,
        io: &'r mut RunIo<'i>,
    ) -> Result<ExactExecutionReport, RuntimeError> {
        validate_run_arrangement(plan, arrangement, context.plan_epoch)?;
        self.run_exact_report_controlled(plan, Some(arrangement), bindings, context, None, io)
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
        self.run_exact_report_controlled(plan, None, bindings, context, Some(stop), io)
    }

    /// Arranged equivalent of [`Self::cancel_exact_report`].
    pub fn cancel_exact_report_arranged<'p, 'r, 'i>(
        &self,
        plan: &'p ExecutionPlan<'p>,
        arrangement: &ResolvedExecutionArrangement,
        bindings: &ExactHostedBindings,
        context: ExactRunContext<'p>,
        stop: conduit_core::StopPolicy,
        io: &'r mut RunIo<'i>,
    ) -> Result<ExactExecutionReport, RuntimeError> {
        validate_run_arrangement(plan, arrangement, context.plan_epoch)?;
        self.run_exact_report_controlled(plan, Some(arrangement), bindings, context, Some(stop), io)
    }

    /// Atomically admits and starts one persistent exact-run session. It does
    /// not execute a node step; callers drive it through bounded `pump` turns.
    pub fn start_exact_session<'p>(
        &self,
        plan: &'p ExecutionPlan<'p>,
        bindings: &ExactHostedBindings,
        context: ExactRunContext<'p>,
        sessions: &ExactRunSessionRegistry,
        io: ExactRunIo,
    ) -> Result<ExactHostedRunSession, RuntimeError> {
        if io.capacity_bytes() != exact_host_io_capacity(plan)? {
            return Err(RuntimeError::new(
                "CND-RUN-009",
                "owned host I/O capacity does not match the exact plan",
            ));
        }
        let io = Rc::new(RefCell::new(io));
        let (session, parallel_lanes, host_failure, watches, managed_components) = self
            .start_exact_session_with_io(
                plan,
                bindings,
                context,
                sessions,
                HostedRunIo::Owned(Rc::clone(&io)),
                None,
                None,
            )?;
        Ok(ExactHostedRunSession {
            session,
            parallel_lanes,
            host_failure,
            io,
            watches,
            managed_components,
        })
    }

    /// Starts a persistent session only after exact physical-arrangement
    /// admission for the run epoch.
    pub fn start_exact_session_arranged<'p>(
        &self,
        plan: &'p ExecutionPlan<'p>,
        arrangement: &ResolvedExecutionArrangement,
        bindings: &ExactHostedBindings,
        context: ExactRunContext<'p>,
        sessions: &ExactRunSessionRegistry,
        io: ExactRunIo,
    ) -> Result<ExactHostedRunSession, RuntimeError> {
        validate_run_arrangement(plan, arrangement, context.plan_epoch)?;
        if io.capacity_bytes() != exact_host_io_capacity(plan)? {
            return Err(RuntimeError::new(
                "CND-RUN-009",
                "owned host I/O capacity does not match the exact plan",
            ));
        }
        let io = Rc::new(RefCell::new(io));
        let (session, parallel_lanes, host_failure, watches, managed_components) = self
            .start_exact_session_with_io(
                plan,
                bindings,
                context,
                sessions,
                HostedRunIo::Owned(Rc::clone(&io)),
                Some(arrangement),
                None,
            )?;
        Ok(ExactHostedRunSession {
            session,
            parallel_lanes,
            host_failure,
            io,
            watches,
            managed_components,
        })
    }

    /// Starts one persistent session with the evidence provider selected by
    /// the exact plan. The provider is owned by the session and cannot be
    /// substituted by a later Patchbay/UI drain request.
    pub fn start_exact_session_with_evidence_provider<'p>(
        &self,
        plan: &'p ExecutionPlan<'p>,
        bindings: &ExactHostedBindings,
        context: ExactRunContext<'p>,
        sessions: &ExactRunSessionRegistry,
        io: ExactRunIo,
        evidence_provider: Box<dyn ExactEvidenceProvider>,
    ) -> Result<ExactHostedRunSession, RuntimeError> {
        if io.capacity_bytes() != exact_host_io_capacity(plan)? {
            return Err(RuntimeError::new(
                "CND-RUN-009",
                "owned host I/O capacity does not match the exact plan",
            ));
        }
        let evidence_binding = exact_evidence_provider_binding(plan)?;
        let io = Rc::new(RefCell::new(io));
        let (session, parallel_lanes, host_failure, watches, managed_components) = self
            .start_exact_session_with_io(
                plan,
                bindings,
                context,
                sessions,
                HostedRunIo::Owned(Rc::clone(&io)),
                None,
                Some((evidence_binding, evidence_provider)),
            )?;
        Ok(ExactHostedRunSession {
            session,
            parallel_lanes,
            host_failure,
            io,
            watches,
            managed_components,
        })
    }

    /// Evidence-provider session start with exact physical-arrangement
    /// admission.
    #[allow(clippy::too_many_arguments)]
    pub fn start_exact_session_with_evidence_provider_arranged<'p>(
        &self,
        plan: &'p ExecutionPlan<'p>,
        arrangement: &ResolvedExecutionArrangement,
        bindings: &ExactHostedBindings,
        context: ExactRunContext<'p>,
        sessions: &ExactRunSessionRegistry,
        resources: ExactEvidenceSessionResources,
    ) -> Result<ExactHostedRunSession, RuntimeError> {
        validate_run_arrangement(plan, arrangement, context.plan_epoch)?;
        if resources.io.capacity_bytes() != exact_host_io_capacity(plan)? {
            return Err(RuntimeError::new(
                "CND-RUN-009",
                "owned host I/O capacity does not match the exact plan",
            ));
        }
        let evidence_binding = exact_evidence_provider_binding(plan)?;
        let io = Rc::new(RefCell::new(resources.io));
        let (session, parallel_lanes, host_failure, watches, managed_components) = self
            .start_exact_session_with_io(
                plan,
                bindings,
                context,
                sessions,
                HostedRunIo::Owned(Rc::clone(&io)),
                Some(arrangement),
                Some((evidence_binding, resources.evidence_provider)),
            )?;
        Ok(ExactHostedRunSession {
            session,
            parallel_lanes,
            host_failure,
            io,
            watches,
            managed_components,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn start_exact_session_with_io<'p, 'r, 'i>(
        &self,
        plan: &'p ExecutionPlan<'p>,
        bindings: &ExactHostedBindings,
        context: ExactRunContext<'p>,
        sessions: &ExactRunSessionRegistry,
        io: HostedRunIo<'r, 'i>,
        arrangement: Option<&ResolvedExecutionArrangement>,
        evidence_provider: Option<(ExactEvidenceProviderBinding, Box<dyn ExactEvidenceProvider>)>,
    ) -> Result<StartedHostedSession<'r, 'i>, RuntimeError> {
        let admission = sessions
            .admit(context.reservation.available_runtime_memory_bytes)
            .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
        validate_hosted_execution_plan(plan, context.validation)
            .map_err(|error| RuntimeError::new(error.code.as_str(), error.to_string()))?;
        validate_use_time_grants(plan, context)?;
        let topology = self
            .exact_topology()
            .map_err(|error| RuntimeError::new(error.code, error.message))?;
        if context.semantic_source_hash != plan.source_semantic_hash
            || topology.source_semantic_hash != plan.source_semantic_hash
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
                || source.max_queued_bytes != planned.flow.capacity.max_queued_bytes()
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
        let maximum_value_store_slots = exact_host_value_slot_capacity(plan)?;
        let store = Rc::new(RefCell::new(HostValueStore::with_limits(
            maximum_value_store_bytes,
            maximum_value_store_slots,
        )?));
        let watches = Rc::new(RefCell::new(watch::HostedWatchRuntime::from_plan(plan)?));
        let host_failure = Rc::new(RefCell::new(None));
        let mut managed_components = Vec::new();
        let mut scheduled_nodes = Vec::with_capacity(plan.nodes.len());
        for (node_index, planned) in plan.nodes.iter().enumerate() {
            let installed_binding = bindings.resolve(planned, plan.artifacts)?;
            let implementation = installed_binding.implementation;
            let expected_contract = match implementation {
                HostedPrimitiveImplementation::Literal => "std/literal",
                HostedPrimitiveImplementation::FormatValuesLiteral => "std/format-values/literal",
                HostedPrimitiveImplementation::Format => "std/text/format",
                HostedPrimitiveImplementation::Lines => "std/text/lines",
                HostedPrimitiveImplementation::Join => "std/text/join",
                HostedPrimitiveImplementation::Stdin => "io/stdin",
                HostedPrimitiveImplementation::StdinStream => "io/stdin-stream",
                HostedPrimitiveImplementation::Uppercase => "text/uppercase",
                HostedPrimitiveImplementation::DataEncodeUtf8 => "std/data/encode-utf8",
                HostedPrimitiveImplementation::DataDecodeUtf8 => "std/data/decode-utf8",
                HostedPrimitiveImplementation::RecordLiteral => "std/record/literal",
                HostedPrimitiveImplementation::FileChunkLiteral => "fs/chunk/literal",
                HostedPrimitiveImplementation::ValidateClosedRecord => {
                    "std/data/validate-closed-record"
                }
                HostedPrimitiveImplementation::ValidationDecisionAssert => {
                    "std/testing/assert-validation-decision"
                }
                HostedPrimitiveImplementation::FrameLengthU32Be => "std/data/frame-length-u32be",
                HostedPrimitiveImplementation::DeframeLengthU32Be => {
                    "std/data/deframe-length-u32be"
                }
                HostedPrimitiveImplementation::Ticker => "time/ticker",
                HostedPrimitiveImplementation::TimeDelay => "time/delay",
                HostedPrimitiveImplementation::TimeTimeout => "time/timeout",
                HostedPrimitiveImplementation::TimeDebounce => "time/debounce",
                HostedPrimitiveImplementation::TimeThrottle => "time/throttle",
                HostedPrimitiveImplementation::StateCell => "state/cell",
                HostedPrimitiveImplementation::StateDeduplicate => "state/deduplicate",
                HostedPrimitiveImplementation::StateCache => "state/cache",
                HostedPrimitiveImplementation::SupervisionRetry => "supervision/retry",
                HostedPrimitiveImplementation::SupervisionCircuitBreaker => {
                    "supervision/circuit-breaker"
                }
                HostedPrimitiveImplementation::Stdout => "io/stdout",
                HostedPrimitiveImplementation::Stderr => "io/stderr",
                HostedPrimitiveImplementation::StdoutStream => "io/stdout-stream",
                HostedPrimitiveImplementation::StderrStream => "io/stderr-stream",
                HostedPrimitiveImplementation::DisplayText => "display/text",
                HostedPrimitiveImplementation::Discard => "flow/discard",
                HostedPrimitiveImplementation::PassThrough => "flow/identity",
                HostedPrimitiveImplementation::Tee => "conduit.std/tee",
                HostedPrimitiveImplementation::Merge => "conduit.std/merge",
                HostedPrimitiveImplementation::ControlMerge => "conduit.media/control/merge",
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
                HostedPrimitiveImplementation::StdinStream => {
                    HostedNodeKind::Stdin { emitted: false }
                }
                HostedPrimitiveImplementation::Uppercase => HostedNodeKind::Uppercase,
                HostedPrimitiveImplementation::DataEncodeUtf8
                | HostedPrimitiveImplementation::DataDecodeUtf8 => HostedNodeKind::DataUtf8 {
                    utf8: conduit_std::Utf8State::new(),
                    pending: None,
                    cursor: 0,
                    validated: false,
                    maximum_input_bytes: source_usize(&resolved.source, "maximum_input_bytes")?,
                    maximum_output_bytes: source_usize(&resolved.source, "maximum_output_bytes")?,
                },
                HostedPrimitiveImplementation::RecordLiteral => HostedNodeKind::Literal {
                    value: encode_record_literal(&resolved.source)?,
                    emitted: false,
                },
                HostedPrimitiveImplementation::FileChunkLiteral => {
                    let Some(SourceValue::Bytes(value)) = resolved.source.config_value("value")
                    else {
                        return Err(RuntimeError::new(
                            "CND-FS-001",
                            "file chunk literal value disappeared",
                        ));
                    };
                    HostedNodeKind::Literal {
                        value: value.clone(),
                        emitted: false,
                    }
                }
                HostedPrimitiveImplementation::ValidateClosedRecord => {
                    let output_cord = |port: &str| {
                        plan.cords.iter().position(|cord| {
                            cord.from.node == planned.instance && cord.from.port.as_str() == port
                        })
                    };
                    HostedNodeKind::ValidateClosedRecord {
                        candidate_cord: output_cord("candidate"),
                        decision_cord: output_cord("decision"),
                        candidate: None,
                        decision: None,
                        maximum_fields: source_usize(&resolved.source, "maximum_fields")?,
                        maximum_field_name_bytes: source_usize(
                            &resolved.source,
                            "maximum_field_name_bytes",
                        )?,
                        maximum_field_value_bytes: source_usize(
                            &resolved.source,
                            "maximum_field_value_bytes",
                        )?,
                        maximum_work: source_usize(&resolved.source, "maximum_work")?,
                    }
                }
                HostedPrimitiveImplementation::ValidationDecisionAssert => {
                    HostedNodeKind::ValidationDecisionAssert {
                        expected: resolved
                            .source
                            .config("expected")
                            .expect("validated expected decision")
                            .to_owned(),
                    }
                }
                HostedPrimitiveImplementation::FrameLengthU32Be => {
                    HostedNodeKind::FrameLengthU32Be {
                        input: None,
                        cursor: 0,
                        output: Vec::new(),
                        pending_output: None,
                        maximum_frame_bytes: source_usize(&resolved.source, "maximum_frame_bytes")?,
                        maximum_output_bytes: source_usize(
                            &resolved.source,
                            "maximum_output_bytes",
                        )?,
                    }
                }
                HostedPrimitiveImplementation::DeframeLengthU32Be => {
                    let maximum_frame_bytes =
                        source_usize(&resolved.source, "maximum_frame_bytes")?;
                    HostedNodeKind::DeframeLengthU32Be {
                        decoder: conduit_std::LengthU32BeDecoder::new(maximum_frame_bytes),
                        input: None,
                        cursor: 0,
                        pending_output: None,
                        terminal_seen: false,
                    }
                }
                HostedPrimitiveImplementation::TimeDelay
                | HostedPrimitiveImplementation::TimeTimeout
                | HostedPrimitiveImplementation::TimeDebounce
                | HostedPrimitiveImplementation::TimeThrottle => {
                    let behavior = match implementation {
                        HostedPrimitiveImplementation::TimeDelay => TimeBehavior::Delay {
                            drop_at_terminal: resolved.source.config("terminal") == Some("drop"),
                        },
                        HostedPrimitiveImplementation::TimeTimeout => TimeBehavior::Timeout,
                        HostedPrimitiveImplementation::TimeDebounce => TimeBehavior::Debounce {
                            mode: if resolved.source.config("mode") == Some("leading") {
                                conduit_std::DebounceMode::Leading
                            } else {
                                conduit_std::DebounceMode::Trailing
                            },
                            flush_at_terminal: resolved.source.config("terminal") == Some("flush"),
                        },
                        HostedPrimitiveImplementation::TimeThrottle => TimeBehavior::Throttle {
                            mode: if resolved.source.config("mode") == Some("leading") {
                                conduit_std::ThrottleMode::LeadingBlock
                            } else {
                                conduit_std::ThrottleMode::TrailingCoalesce
                            },
                            flush_at_terminal: resolved.source.config("terminal") == Some("flush"),
                        },
                        _ => unreachable!("time implementation selected"),
                    };
                    HostedNodeKind::TimeTransform {
                        behavior,
                        duration_ticks: u64::try_from(source_usize(
                            &resolved.source,
                            "duration_ticks",
                        )?)
                        .map_err(|_| {
                            RuntimeError::new("CND-TIM-001", "duration does not fit u64")
                        })?,
                        deadline_tick: None,
                        retained: None,
                        pending_output: None,
                        terminal_seen: false,
                    }
                }
                HostedPrimitiveImplementation::StateCell => HostedNodeKind::StateCell {
                    update_cord: planned_input_cord(plan, planned.instance, "update")?,
                    command_cord: plan.cords.iter().position(|cord| {
                        cord.to.node == planned.instance && cord.to.port.as_str() == "command"
                    }),
                    initial_bytes: (resolved.source.config("initialization") == Some("value"))
                        .then(|| {
                            resolved
                                .source
                                .config("initial")
                                .unwrap_or_default()
                                .as_bytes()
                                .to_vec()
                        }),
                    initial_value: None,
                    current: None,
                    pending_output: None,
                    emit_initial: resolved.source.config("emission") == Some("initial-and-update"),
                    initialized: false,
                },
                HostedPrimitiveImplementation::StateDeduplicate => {
                    HostedNodeKind::StateDeduplicate {
                        state: conduit_std::DeduplicateState::new(
                            source_usize(&resolved.source, "maximum_entries")?,
                            u64::try_from(source_usize(&resolved.source, "maximum_bytes")?)
                                .map_err(|_| {
                                    RuntimeError::new(
                                        "CND-STA-002",
                                        "deduplicate byte bound does not fit u64",
                                    )
                                })?,
                        )
                        .map_err(state_runtime_error)?,
                        pending_output: None,
                    }
                }
                HostedPrimitiveImplementation::StateCache => HostedNodeKind::StateCache {
                    state: conduit_std::CacheState::new(
                        source_usize(&resolved.source, "maximum_entries")?,
                        u64::try_from(source_usize(&resolved.source, "maximum_total_bytes")?)
                            .map_err(|_| {
                                RuntimeError::new(
                                    "CND-STA-002",
                                    "cache byte bound does not fit u64",
                                )
                            })?,
                    )
                    .map_err(state_runtime_error)?,
                    envelopes: Vec::with_capacity(conduit_std::STATE_MAX_ENTRIES),
                    retained_values: Vec::with_capacity(conduit_std::STATE_MAX_ENTRIES),
                    pending_output: None,
                    maximum_key_bytes: source_usize(&resolved.source, "maximum_key_bytes")?,
                    maximum_value_bytes: source_usize(&resolved.source, "maximum_value_bytes")?,
                },
                HostedPrimitiveImplementation::SupervisionRetry => {
                    let to_u64 = |key| {
                        u64::try_from(source_usize(&resolved.source, key)?).map_err(|_| {
                            RuntimeError::new(
                                "CND-SVP-001",
                                format!("supervision `{key}` does not fit u64"),
                            )
                        })
                    };
                    let maximum_attempts =
                        u16::try_from(source_usize(&resolved.source, "maximum_attempts")?)
                            .map_err(|_| {
                                RuntimeError::new(
                                    "CND-SVP-001",
                                    "retry attempt bound does not fit u16",
                                )
                            })?;
                    HostedNodeKind::SupervisionRetry {
                        request_cord: planned_input_cord(plan, planned.instance, "request")?,
                        terminal_cord: planned_input_cord(plan, planned.instance, "terminal")?,
                        entropy_cord: plan.cords.iter().position(|cord| {
                            cord.to.node == planned.instance && cord.to.port.as_str() == "entropy"
                        }),
                        request: None,
                        pending_output: None,
                        pending_outcome: None,
                        state: None,
                        awaiting_outcome: false,
                        maximum_attempts,
                        deadline_ticks: to_u64("deadline_ticks")?,
                        policy: conduit_std::BackoffPolicy {
                            mode: if resolved.source.config("backoff") == Some("fixed") {
                                conduit_std::BackoffMode::Fixed
                            } else {
                                conduit_std::BackoffMode::Exponential
                            },
                            initial_ticks: to_u64("initial_backoff_ticks")?,
                            maximum_ticks: to_u64("maximum_backoff_ticks")?,
                            jitter_ticks: to_u64("jitter_ticks")?,
                        },
                        permission: match resolved.source.config("idempotency") {
                            Some("idempotent") => conduit_std::RetryPermission::Idempotent,
                            Some("reconcile-before-retry") => {
                                conduit_std::RetryPermission::ReconcileBeforeRetry
                            }
                            _ => conduit_std::RetryPermission::Forbidden,
                        },
                        committed_replay_permitted: resolved.source.config("committed_replay")
                            == Some("permit"),
                        generation: 0,
                    }
                }
                HostedPrimitiveImplementation::SupervisionCircuitBreaker => {
                    let maximum_observations =
                        source_usize(&resolved.source, "maximum_observations")?;
                    let failure_threshold = source_usize(&resolved.source, "failure_threshold")?;
                    let cooldown_ticks = u64::try_from(source_usize(
                        &resolved.source,
                        "cooldown_ticks",
                    )?)
                    .map_err(|_| {
                        RuntimeError::new("CND-SVP-001", "breaker cooldown does not fit u64")
                    })?;
                    let maximum_half_open_probes =
                        u16::try_from(source_usize(&resolved.source, "maximum_half_open_probes")?)
                            .map_err(|_| {
                                RuntimeError::new(
                                    "CND-SVP-001",
                                    "breaker probe bound does not fit u16",
                                )
                            })?;
                    HostedNodeKind::SupervisionCircuitBreaker {
                        request_cord: planned_input_cord(plan, planned.instance, "request")?,
                        terminal_cord: planned_input_cord(plan, planned.instance, "terminal")?,
                        pending_output: None,
                        state: conduit_std::CircuitBreakerState::new(
                            maximum_observations,
                            failure_threshold,
                            cooldown_ticks,
                            maximum_half_open_probes,
                        )
                        .map_err(supervision_runtime_error)?,
                        awaiting_outcome: false,
                    }
                }
                HostedPrimitiveImplementation::Stdout => HostedNodeKind::Stdout,
                HostedPrimitiveImplementation::Stderr => HostedNodeKind::Stderr,
                HostedPrimitiveImplementation::StdoutStream => HostedNodeKind::Stdout,
                HostedPrimitiveImplementation::StderrStream => HostedNodeKind::Stderr,
                HostedPrimitiveImplementation::DisplayText => HostedNodeKind::DisplayText,
                HostedPrimitiveImplementation::Discard => HostedNodeKind::Discard,
                HostedPrimitiveImplementation::PassThrough => HostedNodeKind::PassThrough,
                HostedPrimitiveImplementation::Tee => HostedNodeKind::Tee {
                    isolated: resolved.source.config("mode") == Some("isolated"),
                    retained: None,
                    delivered: [false; 2],
                },
                HostedPrimitiveImplementation::Merge
                | HostedPrimitiveImplementation::ControlMerge => HostedNodeKind::Merge {
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
                    input: planned_input_cord(plan, planned.instance, "candidate")?,
                    control: planned_input_cord(plan, planned.instance, "permit")?,
                    open: resolved.source.config("initial") == Some("open"),
                },
                HostedPrimitiveImplementation::Select => HostedNodeKind::Select {
                    inputs: [
                        planned_input_cord(plan, planned.instance, "left")?,
                        planned_input_cord(plan, planned.instance, "right")?,
                    ],
                    control: planned_input_cord(plan, planned.instance, "selector")?,
                    selected: usize::from(resolved.source.config("initial") == Some("right")),
                },
                HostedPrimitiveImplementation::Fallback => {
                    HostedNodeKind::Fallback { emitted: false }
                }
                HostedPrimitiveImplementation::Ticker
                | HostedPrimitiveImplementation::HostedService => {
                    let binding =
                        exact_host_service_binding(plan, planned, installed_binding, context)?;
                    let input_cords = resolved
                        .definition
                        .contract
                        .inputs
                        .iter()
                        .flat_map(|port| {
                            plan.cords
                                .iter()
                                .enumerate()
                                .filter(move |(_, cord)| {
                                    cord.to.node == planned.instance && cord.to.port == port.id
                                })
                                .map(move |(cord, _)| (cord, port.value_type))
                        })
                        .collect();
                    let output_routes = resolved
                        .definition
                        .contract
                        .outputs
                        .iter()
                        .map(|port| {
                            (
                                port.value_type,
                                plan.cords
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, cord)| {
                                        cord.from.node == planned.instance
                                            && cord.from.port == port.id
                                    })
                                    .map(|(cord, _)| cord)
                                    .collect(),
                            )
                        })
                        .collect();
                    let managed =
                        managed_component_machine(&binding, planned.contract.id.as_str())?.map(
                            |machine| {
                                let machine = Rc::new(RefCell::new(machine));
                                managed_components.push(Rc::clone(&machine));
                                machine
                            },
                        );
                    HostedNodeKind::HostedService {
                        handler: resolved.new_handler(),
                        node: resolved.source.clone(),
                        binding,
                        managed,
                        managed_stop_request: None,
                        input_cords,
                        inputs: Vec::new(),
                        output_routes,
                        pending_outputs: Vec::new(),
                        completion_pending: false,
                        cleanup: None,
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
                    watches: Rc::clone(&watches),
                    io: io.clone(),
                    in_cords,
                    out_cords,
                    maximum_input_bytes,
                    cancellation_ticks: profile.limits.cancellation_ticks,
                    host_failure: Rc::clone(&host_failure),
                },
                machine,
            });
            debug_assert_eq!(scheduled_nodes.len(), node_index + 1);
        }
        let parallel_lanes = arrangement
            .map(|arrangement| {
                HostedProductionLanes::admit(plan, arrangement, Rc::clone(&host_failure))
            })
            .transpose()?
            .flatten();
        let executor = DeterministicExecutor::start(
            plan,
            context.validation,
            context.scheduler_policy,
            context.reservation,
            scheduled_nodes,
        )
        .map_err(|error| {
            host_failure
                .borrow_mut()
                .take()
                .unwrap_or_else(|| RuntimeError::new(error.code(), error.to_string()))
        })?;
        let identity = ExactRunIdentity {
            plan_identity: plan.identity,
            source_semantic_hash: plan.source_semantic_hash,
            plan_epoch: context.plan_epoch,
            run_id: context.run_id.as_str().to_owned(),
        };
        let session = if let Some((binding, provider)) = evidence_provider {
            ExactRunSession::new_with_evidence_provider(
                admission, identity, executor, binding, provider,
            )?
        } else {
            ExactRunSession::new(admission, identity, executor)
        };
        Ok((
            session,
            parallel_lanes,
            host_failure,
            watches,
            managed_components,
        ))
    }

    fn run_exact_report_controlled<'p, 'r, 'i>(
        &self,
        plan: &'p ExecutionPlan<'p>,
        arrangement: Option<&ResolvedExecutionArrangement>,
        bindings: &ExactHostedBindings,
        context: ExactRunContext<'p>,
        initial_stop: Option<conduit_core::StopPolicy>,
        io: &'r mut RunIo<'i>,
    ) -> Result<ExactExecutionReport, RuntimeError> {
        let sessions =
            ExactRunSessionRegistry::new(1, context.reservation.available_runtime_memory_bytes)
                .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
        let borrowed_io = Rc::new(RefCell::new(io));
        let (mut session, mut parallel_lanes, host_failure, _watches, _managed_components) = self
            .start_exact_session_with_io(
            plan,
            bindings,
            context,
            &sessions,
            HostedRunIo::Borrowed(borrowed_io),
            arrangement,
            None,
        )?;
        if let Some(stop) = initial_stop {
            if let Some(mut parallel_lanes) = parallel_lanes.take() {
                parallel_lanes.cancel();
            }
            session.cancel(stop).map_err(|error| {
                host_failure
                    .borrow_mut()
                    .take()
                    .unwrap_or_else(|| RuntimeError::new(error.code(), error.to_string()))
            })?;
            if let Some(error) = host_failure.borrow_mut().take() {
                return Err(error);
            }
        }
        let quantum = context.scheduler_policy.max_decisions.max(1);
        let status = loop {
            session
                .pump_with_authority_using(quantum, &[], |executor, remaining, grants| {
                    let Some(parallel_lanes) = parallel_lanes.as_mut() else {
                        return Ok(false);
                    };
                    parallel_lanes.drive(executor, remaining, grants)
                })
                .map_err(|error| {
                    host_failure
                        .borrow_mut()
                        .take()
                        .unwrap_or_else(|| RuntimeError::new(error.code(), error.to_string()))
                })?;
            match session.scheduler_status() {
                SchedulerStatus::Running => continue,
                SchedulerStatus::Stalled => {
                    let Some(deadline) = session.next_timer_deadline() else {
                        break SchedulerStatus::Stalled;
                    };
                    session.advance_to(deadline).map_err(|error| {
                        host_failure
                            .borrow_mut()
                            .take()
                            .unwrap_or_else(|| RuntimeError::new(error.code(), error.to_string()))
                    })?;
                }
                terminal => break terminal,
            }
        };
        let allocation = session.allocation();
        let high_water = session.high_water();
        let scheduler_events: Vec<SchedulerEvent> = session.scheduler_events().copied().collect();
        let evidence = session.exact_evidence();
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
        let hosted_lane_batch = parallel_lanes
            .as_ref()
            .and_then(HostedProductionLanes::batch_evidence);
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
                hosted_lane_batch,
            }),
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
                hosted_lane_batch,
            }),
            SchedulerStatus::Cancelled => Err(RuntimeError::new(
                "CND-RUN-006",
                "exact executor run cancelled",
            )),
            SchedulerStatus::Failed(_) => Err(RuntimeError::new(
                "CND-RUN-005",
                "exact executor run failed",
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
                let mut handler = resolved.new_handler();
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

#[derive(Clone, Copy)]
struct HostValueSlot {
    generation: u32,
    offset: u32,
    length: u32,
    occupied: bool,
    marked_live: bool,
    retired: bool,
}

#[derive(Clone, Copy)]
struct HostValueFreeSpan {
    offset: u32,
    length: u32,
}

/// Fixed-capacity, generation-safe payload storage for one hosted session.
///
/// Handles name a slot and generation rather than a grow-only vector index.
/// The scheduler marks values reachable from cords and node state after every
/// turn, then returns every unmarked slot and byte range to this arena.
struct HostValueStore {
    arena: Vec<u8>,
    slots: Vec<HostValueSlot>,
    free_slots: Vec<u32>,
    free_spans: Vec<HostValueFreeSpan>,
    retained_bytes: u64,
    high_water_bytes: u64,
    high_water_slots: u32,
    active_slots: u32,
    maximum_bytes: u64,
}

impl HostValueStore {
    fn with_limits(maximum_bytes: u64, maximum_slots: u32) -> Result<Self, RuntimeError> {
        let capacity = usize::try_from(maximum_bytes).map_err(|_| {
            RuntimeError::new(
                "CND-RUN-009",
                "value-store capacity does not fit the platform",
            )
        })?;
        let slots = usize::try_from(maximum_slots).map_err(|_| {
            RuntimeError::new(
                "CND-RUN-009",
                "value-store slot count does not fit the platform",
            )
        })?;
        let mut arena = Vec::new();
        arena
            .try_reserve_exact(capacity)
            .map_err(|_| RuntimeError::new("CND-SCH-005", "value-store allocation failed"))?;
        arena.resize(capacity, 0);

        let mut slot_table = Vec::new();
        slot_table
            .try_reserve_exact(slots)
            .map_err(|_| RuntimeError::new("CND-SCH-005", "value-store allocation failed"))?;
        slot_table.resize(
            slots,
            HostValueSlot {
                generation: 1,
                offset: 0,
                length: 0,
                occupied: false,
                marked_live: false,
                retired: false,
            },
        );
        let mut free_slots = Vec::new();
        free_slots
            .try_reserve_exact(slots)
            .map_err(|_| RuntimeError::new("CND-SCH-005", "value-store allocation failed"))?;
        for slot in (0..maximum_slots).rev() {
            free_slots.push(slot);
        }
        let mut free_spans = Vec::new();
        free_spans
            .try_reserve_exact(slots.saturating_add(1))
            .map_err(|_| RuntimeError::new("CND-SCH-005", "value-store allocation failed"))?;
        if maximum_bytes != 0 {
            free_spans.push(HostValueFreeSpan {
                offset: 0,
                length: u32::try_from(maximum_bytes).map_err(|_| {
                    RuntimeError::new(
                        "CND-RUN-009",
                        "value-store capacity exceeds handle representation",
                    )
                })?,
            });
        }
        Ok(Self {
            arena,
            slots: slot_table,
            free_slots,
            free_spans,
            retained_bytes: 0,
            high_water_bytes: 0,
            high_water_slots: 0,
            active_slots: 0,
            maximum_bytes,
        })
    }

    fn store(&mut self, bytes: &[u8]) -> Option<u64> {
        let length = u32::try_from(bytes.len()).ok()?;
        let next_bytes = self.retained_bytes.checked_add(u64::from(length))?;
        if next_bytes > self.maximum_bytes {
            return None;
        }
        let slot_index = self.free_slots.pop()?;
        let offset = match self.allocate_range(length) {
            Some(offset) => offset,
            None => {
                self.free_slots.push(slot_index);
                return None;
            }
        };
        let slot = self.slots.get_mut(usize::try_from(slot_index).ok()?)?;
        debug_assert!(!slot.occupied && !slot.retired);
        let start = usize::try_from(offset).ok()?;
        let end = start.checked_add(bytes.len())?;
        self.arena.get_mut(start..end)?.copy_from_slice(bytes);
        slot.offset = offset;
        slot.length = length;
        slot.occupied = true;
        slot.marked_live = false;
        self.retained_bytes = next_bytes;
        self.active_slots = self.active_slots.checked_add(1)?;
        self.high_water_bytes = self.high_water_bytes.max(self.retained_bytes);
        self.high_water_slots = self.high_water_slots.max(self.active_slots);
        Some(encode_host_value_handle(slot_index, slot.generation))
    }

    fn get(&self, handle: u64) -> Option<&[u8]> {
        let (slot_index, generation) = decode_host_value_handle(handle)?;
        let slot = self.slots.get(usize::try_from(slot_index).ok()?)?;
        if !slot.occupied || slot.generation != generation {
            return None;
        }
        let start = usize::try_from(slot.offset).ok()?;
        let end = start.checked_add(usize::try_from(slot.length).ok()?)?;
        self.arena.get(start..end)
    }

    fn begin_reconciliation(&mut self) {
        for slot in &mut self.slots {
            if slot.occupied {
                slot.marked_live = false;
            }
        }
    }

    fn mark_live(&mut self, value: RuntimeValue) {
        let Some((slot_index, generation)) = decode_host_value_handle(value.handle) else {
            return;
        };
        let Ok(slot_index) = usize::try_from(slot_index) else {
            return;
        };
        let Some(slot) = self.slots.get_mut(slot_index) else {
            return;
        };
        if slot.occupied && slot.generation == generation {
            slot.marked_live = true;
        }
    }

    fn finish_reconciliation(&mut self) {
        for slot_index in 0..self.slots.len() {
            if self.slots[slot_index].occupied && !self.slots[slot_index].marked_live {
                self.release_slot(slot_index);
            }
        }
    }

    fn usage(&self) -> ValueStorageUsage {
        ValueStorageUsage {
            resident_slots: self.active_slots,
            resident_bytes: self.retained_bytes,
            high_water_slots: self.high_water_slots,
            high_water_bytes: self.high_water_bytes,
            maximum_slots: u32::try_from(self.slots.len()).unwrap_or(u32::MAX),
            maximum_bytes: self.maximum_bytes,
        }
    }

    fn allocate_range(&mut self, length: u32) -> Option<u32> {
        if length == 0 {
            return Some(0);
        }
        let index = self
            .free_spans
            .iter()
            .position(|span| span.length >= length)?;
        let span = &mut self.free_spans[index];
        let offset = span.offset;
        if span.length == length {
            self.free_spans.remove(index);
        } else {
            span.offset = span.offset.checked_add(length)?;
            span.length -= length;
        }
        Some(offset)
    }

    fn release_slot(&mut self, slot_index: usize) {
        let slot = self.slots[slot_index];
        debug_assert!(slot.occupied);
        self.retained_bytes -= u64::from(slot.length);
        self.active_slots = self.active_slots.saturating_sub(1);
        if slot.length != 0 {
            self.release_range(HostValueFreeSpan {
                offset: slot.offset,
                length: slot.length,
            });
        }
        let slot = &mut self.slots[slot_index];
        slot.occupied = false;
        slot.marked_live = false;
        slot.offset = 0;
        slot.length = 0;
        if let Some(next_generation) = slot.generation.checked_add(1) {
            slot.generation = next_generation;
            self.free_slots
                .push(u32::try_from(slot_index).expect("slot index is bounded"));
        } else {
            slot.retired = true;
        }
    }

    fn release_range(&mut self, range: HostValueFreeSpan) {
        let index = self
            .free_spans
            .partition_point(|span| span.offset < range.offset);
        self.free_spans.insert(index, range);
        if index > 0 {
            let previous = self.free_spans[index - 1];
            if previous.offset.checked_add(previous.length) == Some(range.offset) {
                self.free_spans[index - 1].length = previous
                    .length
                    .checked_add(range.length)
                    .expect("bounded arena span cannot overflow");
                self.free_spans.remove(index);
            }
        }
        let index = self
            .free_spans
            .partition_point(|span| span.offset <= range.offset)
            .saturating_sub(1);
        if index + 1 < self.free_spans.len() {
            let current = self.free_spans[index];
            let next = self.free_spans[index + 1];
            if current.offset.checked_add(current.length) == Some(next.offset) {
                self.free_spans[index].length = current
                    .length
                    .checked_add(next.length)
                    .expect("bounded arena span cannot overflow");
                self.free_spans.remove(index + 1);
            }
        }
    }
}

fn encode_host_value_handle(slot: u32, generation: u32) -> u64 {
    (u64::from(generation) << 32) | u64::from(slot)
}

fn decode_host_value_handle(handle: u64) -> Option<(u32, u32)> {
    let generation = (handle >> 32) as u32;
    (generation != 0).then_some((handle as u32, generation))
}

#[derive(Clone, Copy)]
enum TimeBehavior {
    Delay {
        drop_at_terminal: bool,
    },
    Timeout,
    Debounce {
        mode: conduit_std::DebounceMode,
        flush_at_terminal: bool,
    },
    Throttle {
        mode: conduit_std::ThrottleMode,
        flush_at_terminal: bool,
    },
}

enum StateCacheRequest<'a> {
    Put { key: &'a [u8], value: &'a [u8] },
    Get { key: &'a [u8] },
    Invalidate { key: &'a [u8] },
    Reset,
}

struct HostedServiceCleanupState {
    deadline_tick: u64,
    /// Cleanup may be polled once by the synchronous Abort request before a
    /// `StepIo` exists. Any returned interests are installed by the next
    /// bounded scheduler step and then cleared.
    initial_interests: Option<Vec<HostedServiceInterest>>,
}

fn parse_state_cache_request(bytes: &[u8]) -> Result<StateCacheRequest<'_>, RuntimeError> {
    if bytes == b"reset" {
        return Ok(StateCacheRequest::Reset);
    }
    if let Some(rest) = bytes.strip_prefix(b"get:")
        && !rest.is_empty()
    {
        return Ok(StateCacheRequest::Get { key: rest });
    }
    if let Some(rest) = bytes.strip_prefix(b"invalidate:")
        && !rest.is_empty()
    {
        return Ok(StateCacheRequest::Invalidate { key: rest });
    }
    if let Some(rest) = bytes.strip_prefix(b"put:")
        && let Some(separator) = rest.iter().position(|byte| *byte == b'=')
        && separator > 0
    {
        return Ok(StateCacheRequest::Put {
            key: &rest[..separator],
            value: &rest[separator + 1..],
        });
    }
    Err(RuntimeError::new(
        "CND-STA-021",
        "cache request must be put:key=value, get:key, invalidate:key, or reset",
    ))
}

fn state_response_value(
    store: &RefCell<HostValueStore>,
    bytes: &[u8],
    envelope: RuntimeValueEnvelope,
) -> Result<RuntimeValue, RuntimeError> {
    let handle = store.borrow_mut().store(bytes).ok_or_else(|| {
        RuntimeError::new(
            "conduit/value-store-bound-exceeded",
            "state response exceeded the exact value-store bound",
        )
    })?;
    Ok(RuntimeValue {
        handle,
        accounted_bytes: bytes.len() as u32,
        envelope,
    })
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
    DisplayText,
    Discard,
    PassThrough,
    DataUtf8 {
        utf8: conduit_std::Utf8State,
        pending: Option<RuntimeValue>,
        cursor: usize,
        validated: bool,
        maximum_input_bytes: usize,
        maximum_output_bytes: usize,
    },
    ValidateClosedRecord {
        candidate_cord: Option<usize>,
        decision_cord: Option<usize>,
        candidate: Option<RuntimeValue>,
        decision: Option<RuntimeValue>,
        maximum_fields: usize,
        maximum_field_name_bytes: usize,
        maximum_field_value_bytes: usize,
        maximum_work: usize,
    },
    ValidationDecisionAssert {
        expected: String,
    },
    FrameLengthU32Be {
        input: Option<RuntimeValue>,
        cursor: usize,
        output: Vec<u8>,
        pending_output: Option<RuntimeValue>,
        maximum_frame_bytes: usize,
        maximum_output_bytes: usize,
    },
    DeframeLengthU32Be {
        decoder: conduit_std::LengthU32BeDecoder<{ conduit_std::DATA_MAX_FRAME_BYTES }>,
        input: Option<RuntimeValue>,
        cursor: usize,
        pending_output: Option<RuntimeValue>,
        terminal_seen: bool,
    },
    StateCell {
        update_cord: usize,
        command_cord: Option<usize>,
        initial_bytes: Option<Vec<u8>>,
        initial_value: Option<RuntimeValue>,
        current: Option<RuntimeValue>,
        pending_output: Option<RuntimeValue>,
        emit_initial: bool,
        initialized: bool,
    },
    StateDeduplicate {
        state: conduit_std::DeduplicateState<{ conduit_std::STATE_MAX_ENTRIES }>,
        pending_output: Option<RuntimeValue>,
    },
    StateCache {
        state: conduit_std::CacheState<{ conduit_std::STATE_MAX_ENTRIES }>,
        envelopes: Vec<(conduit_std::StateIdentity, RuntimeValueEnvelope)>,
        retained_values: Vec<(conduit_std::StateIdentity, RuntimeValue)>,
        pending_output: Option<RuntimeValue>,
        maximum_key_bytes: usize,
        maximum_value_bytes: usize,
    },
    SupervisionRetry {
        request_cord: usize,
        terminal_cord: usize,
        entropy_cord: Option<usize>,
        request: Option<RuntimeValue>,
        pending_output: Option<RuntimeValue>,
        pending_outcome: Option<conduit_std::AttemptOutcome>,
        state: Option<conduit_std::RetryState>,
        awaiting_outcome: bool,
        maximum_attempts: u16,
        deadline_ticks: u64,
        policy: conduit_std::BackoffPolicy,
        permission: conduit_std::RetryPermission,
        committed_replay_permitted: bool,
        generation: u32,
    },
    SupervisionCircuitBreaker {
        request_cord: usize,
        terminal_cord: usize,
        pending_output: Option<RuntimeValue>,
        state: conduit_std::CircuitBreakerState<{ conduit_std::SUPERVISION_MAX_OBSERVATIONS }>,
        awaiting_outcome: bool,
    },
    TimeTransform {
        behavior: TimeBehavior,
        duration_ticks: u64,
        deadline_tick: Option<u64>,
        retained: Option<RuntimeValue>,
        pending_output: Option<RuntimeValue>,
        terminal_seen: bool,
    },
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
        binding: ExactHostedServiceBinding,
        managed: Option<Rc<RefCell<ManagedComponentMachine>>>,
        managed_stop_request: Option<String>,
        input_cords: Vec<(usize, TypeContractRef<'static>)>,
        inputs: Vec<Option<Value>>,
        output_routes: Vec<(TypeContractRef<'static>, Vec<usize>)>,
        pending_outputs: Vec<HostedServiceOutput>,
        completion_pending: bool,
        cleanup: Option<HostedServiceCleanupState>,
        completed: bool,
    },
}

impl HostedNodeKind {
    fn mark_retained_values(&self, store: &mut HostValueStore) {
        let mut mark = |value: Option<RuntimeValue>| {
            if let Some(value) = value {
                store.mark_live(value);
            }
        };
        match self {
            Self::Format {
                template,
                values,
                output,
                ..
            } => {
                mark(*template);
                mark(*values);
                mark(*output);
            }
            Self::Lines {
                input,
                pending_output,
                ..
            }
            | Self::FrameLengthU32Be {
                input,
                pending_output,
                ..
            }
            | Self::DeframeLengthU32Be {
                input,
                pending_output,
                ..
            } => {
                mark(*input);
                mark(*pending_output);
            }
            Self::Join {
                inputs,
                pending_output,
                ..
            } => {
                for value in inputs {
                    mark(Some(*value));
                }
                mark(*pending_output);
            }
            Self::DataUtf8 { pending, .. }
            | Self::StateDeduplicate {
                pending_output: pending,
                ..
            }
            | Self::SupervisionCircuitBreaker {
                pending_output: pending,
                ..
            } => mark(*pending),
            Self::StateCache {
                retained_values,
                pending_output,
                ..
            } => {
                for (_, value) in retained_values {
                    mark(Some(*value));
                }
                mark(*pending_output);
            }
            Self::ValidateClosedRecord {
                candidate,
                decision,
                ..
            } => {
                mark(*candidate);
                mark(*decision);
            }
            Self::StateCell {
                initial_value,
                current,
                pending_output,
                ..
            } => {
                mark(*initial_value);
                mark(*current);
                mark(*pending_output);
            }
            Self::SupervisionRetry {
                request,
                pending_output,
                ..
            } => {
                mark(*request);
                mark(*pending_output);
            }
            Self::TimeTransform {
                retained,
                pending_output,
                ..
            } => {
                mark(*retained);
                mark(*pending_output);
            }
            Self::Tee { retained, .. } => mark(*retained),
            Self::Zip { left, right, .. } => {
                mark(*left);
                mark(*right);
            }
            Self::HostedService {
                pending_outputs, ..
            } => {
                for output in pending_outputs {
                    mark(Some(output.value));
                }
            }
            Self::Literal { .. }
            | Self::Stdin { .. }
            | Self::Uppercase
            | Self::Stdout
            | Self::Stderr
            | Self::DisplayText
            | Self::Discard
            | Self::PassThrough
            | Self::ValidationDecisionAssert { .. }
            | Self::Merge { .. }
            | Self::Gate { .. }
            | Self::Select { .. }
            | Self::Fallback { .. } => {}
        }
    }
}

struct HostedServiceOutput {
    value: RuntimeValue,
    cords: Vec<usize>,
    next_cord: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ValidationSendOutcome {
    Published,
    Blocked,
}

fn stage_validation_output(
    status: Result<SendStatus, SchedulerError>,
    pending: &mut Option<RuntimeValue>,
) -> Result<ValidationSendOutcome, Id<'static>> {
    match status {
        Ok(SendStatus::Reserved) => {
            *pending = None;
            Ok(ValidationSendOutcome::Published)
        }
        Ok(SendStatus::WouldBlock) => Ok(ValidationSendOutcome::Blocked),
        Ok(_) | Err(_) => Err(Id("std/data/validate-closed-record-output-rejected")),
    }
}

struct HostedSchedulerDriver<'r, 'i> {
    kind: HostedNodeKind,
    store: Rc<RefCell<HostValueStore>>,
    watches: Rc<RefCell<watch::HostedWatchRuntime>>,
    io: HostedRunIo<'r, 'i>,
    in_cords: Vec<usize>,
    out_cords: Vec<usize>,
    maximum_input_bytes: u32,
    cancellation_ticks: u64,
    host_failure: Rc<RefCell<Option<RuntimeError>>>,
}

#[cfg(not(target_family = "wasm"))]
enum HostedParallelJob {
    Literal(Vec<u8>),
}

#[cfg(not(target_family = "wasm"))]
enum HostedParallelProposal {
    Literal(Vec<u8>),
}

#[cfg(not(target_family = "wasm"))]
impl HostedLaneJob for HostedParallelJob {
    type Proposal = HostedParallelProposal;

    fn compute(self) -> Self::Proposal {
        match self {
            Self::Literal(value) => HostedParallelProposal::Literal(value),
        }
    }

    fn proposal_bytes(proposal: &Self::Proposal) -> u64 {
        match proposal {
            HostedParallelProposal::Literal(value) => {
                u64::try_from(value.len()).unwrap_or(u64::MAX)
            }
        }
    }
}

#[cfg(not(target_family = "wasm"))]
impl scheduler::ProposedSchedulerNode for HostedSchedulerDriver<'_, '_> {
    type Job = HostedParallelJob;
    type Proposal = HostedParallelProposal;

    fn proposed_step_ready(&self) -> bool {
        matches!(self.kind, HostedNodeKind::Literal { emitted: false, .. })
            && !self.out_cords.is_empty()
    }

    fn take_proposed_step(&mut self) -> Option<Self::Job> {
        match &mut self.kind {
            HostedNodeKind::Literal {
                value,
                emitted: false,
            } if !self.out_cords.is_empty() => {
                Some(HostedParallelJob::Literal(std::mem::take(value)))
            }
            _ => None,
        }
    }

    fn commit_proposed_step(
        &mut self,
        proposal: Self::Proposal,
        io: &mut StepIo<'_>,
    ) -> SchedulerStep {
        match (&mut self.kind, proposal) {
            (
                HostedNodeKind::Literal {
                    value,
                    emitted: false,
                },
                HostedParallelProposal::Literal(proposed),
            ) if value.is_empty() => {
                *value = proposed;
                <Self as SchedulerNode>::step(self, io)
            }
            _ => SchedulerStep::Failed {
                code: Id("conduit/hosted-lane-stale-proposal"),
            },
        }
    }
}

#[cfg(not(target_family = "wasm"))]
enum HostedRunLaneJob {
    Step { node: usize, job: HostedParallelJob },
    Idle,
}

#[cfg(not(target_family = "wasm"))]
enum HostedRunLaneProposal {
    Step {
        node: usize,
        proposal: HostedParallelProposal,
    },
    Idle,
}

#[cfg(not(target_family = "wasm"))]
impl HostedLaneJob for HostedRunLaneJob {
    type Proposal = HostedRunLaneProposal;

    fn compute(self) -> Self::Proposal {
        match self {
            Self::Step { node, job } => HostedRunLaneProposal::Step {
                node,
                proposal: job.compute(),
            },
            Self::Idle => HostedRunLaneProposal::Idle,
        }
    }

    fn proposal_bytes(proposal: &Self::Proposal) -> u64 {
        match proposal {
            HostedRunLaneProposal::Step { proposal, .. } => {
                HostedParallelJob::proposal_bytes(proposal)
            }
            HostedRunLaneProposal::Idle => 0,
        }
    }
}

#[cfg(not(target_family = "wasm"))]
impl HostedRunLaneJob {
    fn proposal_bytes(&self) -> u64 {
        match self {
            Self::Step {
                job: HostedParallelJob::Literal(value),
                ..
            } => u64::try_from(value.len()).unwrap_or(u64::MAX),
            Self::Idle => 0,
        }
    }
}

#[cfg(not(target_family = "wasm"))]
struct HostedProductionLanes {
    coordinator: FixedHostedExecutionCoordinator<HostedRunLaneJob>,
    node_lanes: Vec<Option<u16>>,
    lane_count: u16,
    selected: Vec<(usize, u16)>,
    assignments: Vec<HostedLaneAssignment<HostedRunLaneJob>>,
    committed_tickets: Vec<u64>,
    active_lanes: Vec<u16>,
    observations: Vec<HostedLaneObservation>,
    proposal_slots_capacity: u16,
    proposal_bytes_used: u64,
    proposal_bytes_capacity: u64,
    host_failure: Rc<RefCell<Option<RuntimeError>>>,
}

#[cfg(not(target_family = "wasm"))]
impl HostedProductionLanes {
    fn admit(
        plan: &ExecutionPlan<'_>,
        arrangement: &ResolvedExecutionArrangement,
        host_failure: Rc<RefCell<Option<RuntimeError>>>,
    ) -> Result<Option<Self>, RuntimeError> {
        let Some(placement) = arrangement
            .placements
            .iter()
            .find(|placement| placement.provider.id == FIXED_HOSTED_LANE_PROVIDER_ID)
        else {
            return Ok(None);
        };
        let Some(domain) = arrangement.commit_domains.first() else {
            return Err(RuntimeError::new(
                "CND-LAN-009",
                "fixed hosted execution has no deterministic commit domain",
            ));
        };
        let lane_ids = arrangement
            .lanes
            .iter()
            .filter(|lane| {
                lane.placement == placement.id
                    && arrangement.regions.iter().any(|region| {
                        region.placement == placement.id
                            && region.lane == lane.id
                            && region.commit_domain == domain.id
                            && region.independent
                    })
            })
            .map(|lane| lane.id.as_str())
            .collect::<Vec<_>>();
        let lane_count = u16::try_from(lane_ids.len())
            .map_err(|_| RuntimeError::new("CND-LAN-001", "hosted lane population exceeds u16"))?;
        if lane_count < 3 {
            return Ok(None);
        }
        let coordinator =
            FixedHostedExecutionCoordinator::admit(arrangement, &placement.id, &domain.id, 1)
                .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
        let mut node_lanes = Vec::new();
        node_lanes
            .try_reserve_exact(plan.nodes.len())
            .map_err(|_| {
                RuntimeError::new(
                    "CND-LAN-001",
                    "hosted node placement storage is unavailable",
                )
            })?;
        node_lanes.resize(plan.nodes.len(), None);
        for region in arrangement.regions.iter().filter(|region| {
            region.independent
                && region.placement == placement.id
                && region.commit_domain == domain.id
        }) {
            let Some(lane) = lane_ids.iter().position(|lane| *lane == region.lane) else {
                continue;
            };
            for member in &region.members {
                let Some(node) = plan
                    .nodes
                    .iter()
                    .position(|planned| planned.instance.as_str() == member)
                else {
                    return Err(RuntimeError::new(
                        "CND-LAN-007",
                        "hosted region member is absent from the exact plan",
                    ));
                };
                node_lanes[node] = Some(u16::try_from(lane).map_err(|_| {
                    RuntimeError::new("CND-LAN-001", "hosted lane index exceeds u16")
                })?);
            }
        }
        let mut selected = Vec::new();
        let mut assignments = Vec::new();
        let mut committed_tickets = Vec::new();
        let mut active_lanes = Vec::new();
        let mut observations = Vec::new();
        selected
            .try_reserve_exact(usize::from(lane_count))
            .map_err(|_| RuntimeError::new("CND-LAN-001", "lane selection storage unavailable"))?;
        assignments
            .try_reserve_exact(usize::from(lane_count))
            .map_err(|_| RuntimeError::new("CND-LAN-001", "lane assignment storage unavailable"))?;
        committed_tickets
            .try_reserve_exact(usize::from(lane_count))
            .map_err(|_| RuntimeError::new("CND-LAN-001", "lane commit storage unavailable"))?;
        active_lanes
            .try_reserve_exact(usize::from(lane_count))
            .map_err(|_| RuntimeError::new("CND-LAN-001", "lane activity storage unavailable"))?;
        observations
            .try_reserve_exact(usize::from(lane_count))
            .map_err(|_| RuntimeError::new("CND-LAN-001", "lane evidence storage unavailable"))?;
        Ok(Some(Self {
            coordinator,
            node_lanes,
            lane_count,
            selected,
            assignments,
            committed_tickets,
            active_lanes,
            observations,
            proposal_slots_capacity: domain.proposal_slots,
            proposal_bytes_used: 0,
            proposal_bytes_capacity: domain.maximum_proposal_bytes,
            host_failure,
        }))
    }

    fn drive(
        &mut self,
        executor: &mut DeterministicExecutor<HostedSchedulerDriver<'_, '_>>,
        maximum_decisions: u64,
        grant_observations: &[ExactHostedServiceUseObservation],
    ) -> Result<bool, SchedulerError> {
        let maximum = executor
            .ready_len()
            .min(usize::try_from(maximum_decisions).unwrap_or(usize::MAX));
        self.selected.clear();
        for offset in 0..maximum {
            if self.selected.len() == usize::from(self.lane_count) {
                break;
            }
            let Some(node) = executor.ready_node_at(offset) else {
                break;
            };
            let Some(lane) = self.node_lanes.get(node).copied().flatten() else {
                continue;
            };
            if !executor.proposed_step_ready(node)
                || self.selected.iter().any(|(_, selected)| *selected == lane)
            {
                continue;
            }
            self.selected.push((node, lane));
        }
        if self.selected.len() < 3 {
            return Ok(false);
        }

        self.assignments.clear();
        for &(node, lane) in &self.selected {
            let Some(job) = executor.take_proposed_step(node) else {
                return executor
                    .fail_proposed_execution(SchedulerError::StepContractViolation)
                    .map(|_| false);
            };
            self.assignments.push(HostedLaneAssignment {
                lane,
                job: HostedRunLaneJob::Step { node, job },
            });
        }
        for lane in 0..self.lane_count {
            if !self.selected.iter().any(|(_, selected)| *selected == lane) {
                self.assignments.push(HostedLaneAssignment {
                    lane,
                    job: HostedRunLaneJob::Idle,
                });
            }
        }
        let proposal_bytes_used = self
            .assignments
            .iter()
            .try_fold(0_u64, |total, assignment| {
                total.checked_add(assignment.job.proposal_bytes())
            });
        let Some(proposal_bytes_used) = proposal_bytes_used else {
            return executor
                .fail_proposed_execution(SchedulerError::ArithmeticOverflow)
                .map(|_| false);
        };

        let mut scheduler_error = None;
        let result = self.coordinator.compute_assigned_and_commit(
            self.assignments.drain(..),
            |_, proposal| match proposal {
                HostedRunLaneProposal::Step { node, proposal } => {
                    while executor.ready_node_at(0) != Some(node) {
                        if let Err(error) = executor.run_one_with_authority(grant_observations) {
                            scheduler_error = Some(error);
                            return Err(());
                        }
                    }
                    executor
                        .commit_proposed_front(node, proposal, grant_observations)
                        .map(|_| ())
                        .map_err(|error| {
                            scheduler_error = Some(error);
                        })
                }
                HostedRunLaneProposal::Idle => Ok(()),
            },
        );
        match result {
            Ok(batch) => {
                self.committed_tickets.clear();
                self.committed_tickets.extend(
                    batch
                        .committed_tickets
                        .iter()
                        .take(self.selected.len())
                        .copied(),
                );
                self.active_lanes.clear();
                self.active_lanes
                    .extend(self.selected.iter().map(|(_, lane)| *lane));
                self.observations.clear();
                self.observations
                    .extend_from_slice(batch.physical_completion_order);
                self.proposal_bytes_used = proposal_bytes_used;
                Ok(true)
            }
            Err(error) => {
                let scheduler_error = scheduler_error.unwrap_or(SchedulerError::NodeFailed);
                *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                    error.code(),
                    format!("production hosted lane batch failed: {error}"),
                ));
                executor
                    .fail_proposed_execution(scheduler_error)
                    .map(|_| false)
            }
        }
    }

    fn cancel(&mut self) {
        let _ = self.coordinator.cancel();
    }

    fn observe_lane_loss(&mut self, lane: u16) -> Result<(), RuntimeError> {
        self.coordinator
            .observe_lane_loss(lane)
            .map_err(|error| RuntimeError::new(error.code(), error.to_string()))
    }

    fn batch_evidence(&self) -> Option<HostedLaneBatchEvidence> {
        (!self.observations.is_empty()).then(|| HostedLaneBatchEvidence {
            commit_domain: self.coordinator.commit_domain().to_owned(),
            active_lanes: self.active_lanes.clone(),
            proposal_slots_used: u16::try_from(self.active_lanes.len()).unwrap_or(u16::MAX),
            proposal_slots_capacity: self.proposal_slots_capacity,
            proposal_bytes_used: self.proposal_bytes_used,
            proposal_bytes_capacity: self.proposal_bytes_capacity,
            committed_tickets: self.committed_tickets.clone(),
            physical_completion_order: self.observations.clone(),
        })
    }
}

#[cfg(target_family = "wasm")]
struct HostedProductionLanes;

#[cfg(target_family = "wasm")]
impl HostedProductionLanes {
    fn admit(
        _plan: &ExecutionPlan<'_>,
        _arrangement: &ResolvedExecutionArrangement,
        _host_failure: Rc<RefCell<Option<RuntimeError>>>,
    ) -> Result<Option<Self>, RuntimeError> {
        Ok(None)
    }

    fn drive(
        &mut self,
        _executor: &mut DeterministicExecutor<HostedSchedulerDriver<'_, '_>>,
        _maximum_decisions: u64,
        _grant_observations: &[ExactHostedServiceUseObservation],
    ) -> Result<bool, SchedulerError> {
        Ok(false)
    }

    fn cancel(&mut self) {}

    fn observe_lane_loss(&mut self, _lane: u16) -> Result<(), RuntimeError> {
        Err(RuntimeError::new(
            "CND-LAN-005",
            "hosted lanes are unavailable on this target",
        ))
    }

    fn batch_evidence(&self) -> Option<HostedLaneBatchEvidence> {
        None
    }
}

fn begin_hosted_service_cleanup(
    cleanup: &mut Option<HostedServiceCleanupState>,
    tick: u64,
    cancellation_ticks: u64,
    initial_interests: Option<Vec<HostedServiceInterest>>,
) -> Result<(), RuntimeError> {
    if cleanup.is_some() {
        return Ok(());
    }
    let deadline_tick = tick.checked_add(cancellation_ticks).ok_or_else(|| {
        RuntimeError::new("CND-RUN-013", "hosted provider cleanup deadline overflowed")
    })?;
    *cleanup = Some(HostedServiceCleanupState {
        deadline_tick,
        initial_interests,
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn drive_hosted_service_cleanup(
    handler: &mut dyn Handler,
    node: &Node,
    binding: &ExactHostedServiceBinding,
    managed: &Option<Rc<RefCell<ManagedComponentMachine>>>,
    managed_stop_request: &mut Option<String>,
    cleanup: &mut Option<HostedServiceCleanupState>,
    completed: &mut bool,
    cancellation_ticks: u64,
    io: &mut StepIo<'_>,
    host_failure: &Rc<RefCell<Option<RuntimeError>>>,
) -> SchedulerStep {
    let lifecycle_tick = managed_tick(binding, io.tick());
    if let Some(machine) = managed {
        if managed_stop_request.is_none() {
            match begin_managed_stop(machine, lifecycle_tick, "hosted-service-completed", 0) {
                Ok(request_id) => *managed_stop_request = Some(request_id),
                Err(error) => {
                    *host_failure.borrow_mut() = Some(error);
                    return SchedulerStep::Failed {
                        code: Id("conduit/managed-lifecycle-projection-failed"),
                    };
                }
            }
        }
        if let Some(request_id) = managed_stop_request.as_deref()
            && let Err(error) = begin_managed_cleanup(machine, request_id, lifecycle_tick)
        {
            *host_failure.borrow_mut() = Some(error);
            return SchedulerStep::Failed {
                code: Id("conduit/managed-lifecycle-projection-failed"),
            };
        }
    }
    if let Err(error) = begin_hosted_service_cleanup(cleanup, io.tick(), cancellation_ticks, None) {
        *host_failure.borrow_mut() = Some(error);
        return SchedulerStep::Failed {
            code: Id("conduit/host-service-cleanup-timeout"),
        };
    }
    let state = cleanup
        .as_mut()
        .expect("hosted cleanup state was initialized");
    if io.tick() > state.deadline_tick {
        if let (Some(machine), Some(request_id)) = (managed, managed_stop_request.as_deref()) {
            let _ = apply_managed_executor_event(
                machine,
                request_id,
                ManagedProviderEvent::Failed {
                    reason: ManagedLifecycleReason::CleanupTimeout,
                    cleanup: ManagedCleanupState::TimedOut,
                },
                lifecycle_tick,
            );
        }
        *host_failure.borrow_mut() = Some(RuntimeError::new(
            "CND-RUN-013",
            format!(
                "hosted provider `{}` exceeded its exact cleanup deadline",
                node.id
            ),
        ));
        return SchedulerStep::Failed {
            code: Id("conduit/host-service-cleanup-timeout"),
        };
    }
    let outcome = if let Some(interests) = state.initial_interests.take() {
        Ok(HostedServiceCleanup::Waiting { interests })
    } else {
        handler.cleanup(node, HostedServiceStepContext { tick: io.tick() })
    };
    match outcome {
        Ok(HostedServiceCleanup::Complete) => {
            if let (Some(machine), Some(request_id)) = (managed, managed_stop_request.as_deref())
                && let Err(error) = apply_managed_executor_event(
                    machine,
                    request_id,
                    ManagedProviderEvent::CleanupComplete {
                        released_resources: binding
                            .authorities
                            .iter()
                            .map(|authority| authority.resource_binding_id.clone())
                            .collect(),
                    },
                    lifecycle_tick,
                )
            {
                *host_failure.borrow_mut() = Some(error);
                return SchedulerStep::Failed {
                    code: Id("conduit/managed-lifecycle-projection-failed"),
                };
            }
            *completed = true;
            *cleanup = None;
            SchedulerStep::Completed
        }
        Ok(HostedServiceCleanup::Waiting { interests }) => {
            if interests.is_empty() {
                *host_failure.borrow_mut() = Some(RuntimeError::new(
                    "CND-RUN-013",
                    format!(
                        "hosted provider `{}` returned cleanup Waiting without an exact interest",
                        node.id
                    ),
                ));
                return SchedulerStep::Failed {
                    code: Id("conduit/host-service-cleanup-invalid"),
                };
            }
            for interest in interests {
                let wait = match interest {
                    HostedServiceInterest::Timer {
                        subject,
                        deadline_tick,
                    } if deadline_tick <= state.deadline_tick => {
                        io.wait_for_timer(subject, deadline_tick)
                    }
                    HostedServiceInterest::Timer { .. } => {
                        if let (Some(machine), Some(request_id)) =
                            (managed, managed_stop_request.as_deref())
                        {
                            let _ = apply_managed_executor_event(
                                machine,
                                request_id,
                                ManagedProviderEvent::Failed {
                                    reason: ManagedLifecycleReason::CleanupTimeout,
                                    cleanup: ManagedCleanupState::TimedOut,
                                },
                                lifecycle_tick,
                            );
                        }
                        *host_failure.borrow_mut() = Some(RuntimeError::new(
                            "CND-RUN-013",
                            format!(
                                "hosted provider `{}` requested cleanup after its exact deadline",
                                node.id
                            ),
                        ));
                        return SchedulerStep::Failed {
                            code: Id("conduit/host-service-cleanup-timeout"),
                        };
                    }
                    HostedServiceInterest::HostOperation { subject } => {
                        io.wait_for_host_operation(subject)
                    }
                };
                if let Err(error) = wait {
                    *host_failure.borrow_mut() = Some(RuntimeError::new(
                        "CND-RUN-013",
                        format!(
                            "hosted provider `{}` registered an invalid cleanup wake: {error}",
                            node.id
                        ),
                    ));
                    return SchedulerStep::Failed {
                        code: Id("conduit/host-service-cleanup-invalid"),
                    };
                }
            }
            SchedulerStep::Pending
        }
        Err(error) => {
            if let (Some(machine), Some(request_id)) = (managed, managed_stop_request.as_deref()) {
                let _ = apply_managed_executor_event(
                    machine,
                    request_id,
                    ManagedProviderEvent::Failed {
                        reason: ManagedLifecycleReason::CleanupFailed,
                        cleanup: ManagedCleanupState::Failed,
                    },
                    lifecycle_tick,
                );
            }
            *host_failure.borrow_mut() = Some(error);
            SchedulerStep::Failed {
                code: Id("conduit/host-service-cleanup-failed"),
            }
        }
    }
}

impl SchedulerNode for HostedSchedulerDriver<'_, '_> {
    fn prepare(&mut self) -> Result<conduit_core::LifecycleUsage, Id<'static>> {
        if let HostedNodeKind::HostedService {
            handler,
            node,
            binding,
            managed,
            ..
        } = &mut self.kind
        {
            let tick = binding.use_time_tick;
            let request_id = if let Some(machine) = managed {
                match begin_managed_executor_transition(
                    &mut machine.borrow_mut(),
                    ManagedLifecycleAction::Prepare,
                    tick,
                    "exact-run-prepare",
                ) {
                    Ok(request_id) => Some(request_id),
                    Err(error) => {
                        *self.host_failure.borrow_mut() = Some(error);
                        return Err(Id("conduit/managed-lifecycle-projection-failed"));
                    }
                }
            } else {
                None
            };
            if let Err(error) = handler.prepare(node, binding.clone()) {
                if let (Some(machine), Some(request_id)) = (managed, request_id.as_deref()) {
                    let _ = apply_managed_executor_event(
                        machine,
                        request_id,
                        ManagedProviderEvent::Failed {
                            reason: ManagedLifecycleReason::PreparationFailed,
                            cleanup: ManagedCleanupState::NotRequired,
                        },
                        tick,
                    );
                }
                *self.host_failure.borrow_mut() = Some(error);
                return Err(Id("conduit/host-service-prepare-failed"));
            }
            if let (Some(machine), Some(request_id)) = (managed, request_id.as_deref())
                && let Err(error) = apply_managed_executor_event(
                    machine,
                    request_id,
                    ManagedProviderEvent::Prepared {
                        resource_evidence: binding
                            .authorities
                            .iter()
                            .map(|authority| authority.resource_binding_id.clone())
                            .collect(),
                    },
                    tick,
                )
            {
                *self.host_failure.borrow_mut() = Some(error);
                return Err(Id("conduit/managed-lifecycle-projection-failed"));
            }
        }
        Ok(conduit_core::LifecycleUsage::default())
    }

    fn start(&mut self) -> Result<conduit_core::LifecycleUsage, Id<'static>> {
        if let HostedNodeKind::HostedService {
            handler,
            node,
            binding,
            managed,
            ..
        } = &mut self.kind
        {
            let tick = binding.use_time_tick;
            let request_id = if let Some(machine) = managed {
                match begin_managed_executor_transition(
                    &mut machine.borrow_mut(),
                    ManagedLifecycleAction::Activate,
                    tick,
                    "exact-run-start",
                ) {
                    Ok(request_id) => Some(request_id),
                    Err(error) => {
                        *self.host_failure.borrow_mut() = Some(error);
                        return Err(Id("conduit/managed-lifecycle-projection-failed"));
                    }
                }
            } else {
                None
            };
            if let Err(error) = handler.start(node) {
                if let (Some(machine), Some(request_id)) = (managed, request_id.as_deref()) {
                    let _ = apply_managed_executor_event(
                        machine,
                        request_id,
                        ManagedProviderEvent::Failed {
                            reason: ManagedLifecycleReason::ActivationFailed,
                            cleanup: ManagedCleanupState::Required,
                        },
                        tick,
                    );
                }
                *self.host_failure.borrow_mut() = Some(error);
                return Err(Id("conduit/host-service-start-failed"));
            }
            if let (Some(machine), Some(request_id)) = (managed, request_id.as_deref())
                && let Err(error) = apply_managed_executor_event(
                    machine,
                    request_id,
                    ManagedProviderEvent::Activated,
                    tick,
                )
            {
                *self.host_failure.borrow_mut() = Some(error);
                return Err(Id("conduit/managed-lifecycle-projection-failed"));
            }
        }
        Ok(conduit_core::LifecycleUsage::default())
    }

    fn cancel(&mut self, stop: conduit_core::StopPolicy, tick: u64) {
        if let HostedNodeKind::HostedService {
            handler,
            node,
            binding,
            managed,
            managed_stop_request,
            inputs,
            pending_outputs,
            completion_pending,
            cleanup,
            completed,
            ..
        } = &mut self.kind
        {
            if let Err(error) = handler.cancel(node, stop) {
                *self.host_failure.borrow_mut() = Some(error);
                return;
            }
            let lifecycle_tick = managed_tick(binding, tick);
            if let Some(machine) = managed
                && managed_stop_request.is_none()
            {
                match begin_managed_stop(
                    machine,
                    lifecycle_tick,
                    if stop == conduit_core::StopPolicy::Abort {
                        "exact-run-abort"
                    } else {
                        "exact-run-drain"
                    },
                    u32::try_from(inputs.iter().filter(|input| input.is_some()).count())
                        .unwrap_or(u32::MAX),
                ) {
                    Ok(request_id) => *managed_stop_request = Some(request_id),
                    Err(error) => {
                        *self.host_failure.borrow_mut() = Some(error);
                        return;
                    }
                }
            }
            if stop == conduit_core::StopPolicy::Abort {
                inputs.clear();
                pending_outputs.clear();
                *completion_pending = false;
                if let (Some(machine), Some(request_id)) =
                    (managed.as_ref(), managed_stop_request.as_deref())
                    && let Err(error) = begin_managed_cleanup(machine, request_id, lifecycle_tick)
                {
                    *self.host_failure.borrow_mut() = Some(error);
                    return;
                }
                match handler.cleanup(node, HostedServiceStepContext { tick }) {
                    Ok(HostedServiceCleanup::Complete) => {
                        if let (Some(machine), Some(request_id)) =
                            (managed.as_ref(), managed_stop_request.as_deref())
                            && let Err(error) = apply_managed_executor_event(
                                machine,
                                request_id,
                                ManagedProviderEvent::CleanupComplete {
                                    released_resources: binding
                                        .authorities
                                        .iter()
                                        .map(|authority| authority.resource_binding_id.clone())
                                        .collect(),
                                },
                                lifecycle_tick,
                            )
                        {
                            *self.host_failure.borrow_mut() = Some(error);
                            return;
                        }
                        *completed = true;
                        *cleanup = None;
                    }
                    Ok(HostedServiceCleanup::Waiting { interests }) => {
                        if let Err(error) = begin_hosted_service_cleanup(
                            cleanup,
                            tick,
                            self.cancellation_ticks,
                            Some(interests),
                        ) {
                            *self.host_failure.borrow_mut() = Some(error);
                            *completed = true;
                        }
                    }
                    Err(error) => {
                        if let (Some(machine), Some(request_id)) =
                            (managed.as_ref(), managed_stop_request.as_deref())
                        {
                            let _ = apply_managed_executor_event(
                                machine,
                                request_id,
                                ManagedProviderEvent::Failed {
                                    reason: ManagedLifecycleReason::CleanupFailed,
                                    cleanup: ManagedCleanupState::Failed,
                                },
                                lifecycle_tick,
                            );
                        }
                        *self.host_failure.borrow_mut() = Some(error);
                        *completed = true;
                    }
                }
            }
        }
    }

    fn cancellation_pending(&self) -> bool {
        matches!(
            self.kind,
            HostedNodeKind::HostedService {
                cleanup: Some(_),
                completed: false,
                ..
            }
        )
    }

    fn validate_deadlines(&mut self, tick: u64) -> Result<(), Id<'static>> {
        if let HostedNodeKind::HostedService {
            node,
            binding,
            managed,
            managed_stop_request,
            cleanup: Some(cleanup),
            completed: false,
            ..
        } = &self.kind
            && tick > cleanup.deadline_tick
        {
            if let (Some(machine), Some(request_id)) = (managed, managed_stop_request.as_deref()) {
                let _ = apply_managed_executor_event(
                    machine,
                    request_id,
                    ManagedProviderEvent::Failed {
                        reason: ManagedLifecycleReason::CleanupTimeout,
                        cleanup: ManagedCleanupState::TimedOut,
                    },
                    managed_tick(binding, tick),
                );
            }
            *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                "CND-RUN-013",
                format!(
                    "hosted provider `{}` exceeded its exact cleanup deadline",
                    node.id
                ),
            ));
            return Err(Id("conduit/host-service-cleanup-timeout"));
        }
        Ok(())
    }

    fn validate_wake(
        &mut self,
        tick: u64,
        grant_observations: &[ExactHostedServiceUseObservation],
    ) -> Result<(), Id<'static>> {
        if let HostedNodeKind::HostedService { binding, .. } = &self.kind
            && let Err(error) = validate_hosted_service_wake(binding, tick, grant_observations)
        {
            *self.host_failure.borrow_mut() = Some(error);
            return Err(Id("conduit/host-service-use-time-stale"));
        }
        Ok(())
    }

    fn observe_committed_value(&mut self, cord: usize, value: RuntimeValue, tick: u64) {
        let store = self.store.borrow();
        self.watches.borrow_mut().observe(cord, value, tick, &store);
    }

    fn step(&mut self, io: &mut StepIo<'_>) -> SchedulerStep {
        match &mut self.kind {
            HostedNodeKind::Literal { value, emitted } => {
                if *emitted {
                    return SchedulerStep::Completed;
                }
                if self.out_cords.is_empty() {
                    return SchedulerStep::Completed;
                }
                let Some(handle) = self.store.borrow_mut().store(value) else {
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
                    let Some(handle) = self.store.borrow_mut().store(&formatted) else {
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
                    let Some(handle) = self.store.borrow_mut().store(&bytes) else {
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
                let Some(handle) = self.store.borrow_mut().store(&joined) else {
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
                        let extra_read = match self.io.read_input(&mut extra) {
                            Ok(read) => read,
                            Err(error) => {
                                *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                    "CND-RUN-004",
                                    format!("failed to read exact stdin: {error}"),
                                ));
                                return SchedulerStep::Failed {
                                    code: Id("io/stdin-read-error"),
                                };
                            }
                        };
                        if extra_read == 0 {
                            break;
                        }
                        return SchedulerStep::Failed {
                            code: Id("io/stdin-bound-exceeded"),
                        };
                    }
                    let read_limit = remaining.min(chunk.len());
                    let read = match self.io.read_input(&mut chunk[..read_limit]) {
                        Ok(read) => read,
                        Err(error) => {
                            *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                "CND-RUN-004",
                                format!("failed to read exact stdin: {error}"),
                            ));
                            return SchedulerStep::Failed {
                                code: Id("io/stdin-read-error"),
                            };
                        }
                    };
                    if read == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&chunk[..read]);
                }
                let Some(handle) = self.store.borrow_mut().store(&bytes) else {
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
                binding,
                managed,
                managed_stop_request,
                input_cords,
                inputs,
                output_routes,
                pending_outputs,
                completion_pending,
                cleanup,
                completed,
                ..
            } => {
                if *completed {
                    return SchedulerStep::Completed;
                }
                if cleanup.is_some() {
                    return drive_hosted_service_cleanup(
                        handler.as_mut(),
                        node,
                        binding,
                        managed,
                        managed_stop_request,
                        cleanup,
                        completed,
                        self.cancellation_ticks,
                        io,
                        &self.host_failure,
                    );
                }
                if !pending_outputs.is_empty() {
                    let mut progressed = false;
                    for output in pending_outputs.iter_mut() {
                        while output.next_cord < output.cords.len() {
                            match io.send(output.cords[output.next_cord], output.value, None) {
                                Ok(SendStatus::Reserved) => {
                                    output.next_cord += 1;
                                    progressed = true;
                                }
                                Ok(SendStatus::WouldBlock) => break,
                                status => {
                                    *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                        "CND-RUN-004",
                                        format!(
                                            "host service `{}` output was rejected by its exact cord: {status:?}",
                                            node.id,
                                        ),
                                    ));
                                    return SchedulerStep::Failed {
                                        code: Id("conduit/host-service-output-failed"),
                                    };
                                }
                            }
                        }
                    }
                    if pending_outputs
                        .iter()
                        .all(|output| output.next_cord == output.cords.len())
                    {
                        pending_outputs.clear();
                        if *completion_pending {
                            return drive_hosted_service_cleanup(
                                handler.as_mut(),
                                node,
                                binding,
                                managed,
                                managed_stop_request,
                                cleanup,
                                completed,
                                self.cancellation_ticks,
                                io,
                                &self.host_failure,
                            );
                        }
                        return SchedulerStep::Progress;
                    }
                    if !progressed {
                        for output in pending_outputs.iter() {
                            if output.next_cord < output.cords.len() {
                                let _ = io.wait_for_output(output.cords[output.next_cord]);
                            }
                        }
                    }
                    return if progressed {
                        SchedulerStep::Progress
                    } else {
                        SchedulerStep::Pending
                    };
                }
                if inputs.is_empty() && !input_cords.is_empty() {
                    inputs.resize_with(input_cords.len(), || None);
                }
                for (position, (cord, value_type)) in input_cords.iter().enumerate() {
                    if inputs[position].is_some() {
                        continue;
                    }
                    match io.receive(*cord) {
                        Ok(Some(value)) => {
                            let bytes = self
                                .store
                                .borrow()
                                .get(value.handle)
                                .unwrap_or_default()
                                .to_vec();
                            inputs[position] = Some(Value {
                                value_type: *value_type,
                                bytes,
                            });
                        }
                        Ok(None) => {
                            if matches!(io.input_state(*cord), Ok(FlowQueueState::Completed)) {
                                *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                    "CND-RUN-004",
                                    format!(
                                        "host service `{}` completed without required input",
                                        node.id
                                    ),
                                ));
                                return SchedulerStep::Failed {
                                    code: Id("conduit/host-service-missing-input"),
                                };
                            }
                            let _ = io.wait_for_input(*cord);
                        }
                        Err(error) => {
                            *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                "CND-RUN-004",
                                format!(
                                    "host service `{}` could not receive exact input: {error}",
                                    node.id
                                ),
                            ));
                            return SchedulerStep::Failed {
                                code: Id("conduit/host-service-input-failed"),
                            };
                        }
                    }
                }
                if inputs.iter().any(Option::is_none) {
                    if let Err(error) = set_managed_readiness(
                        managed,
                        ManagedRuntimeReadiness::Waiting,
                        managed_tick(binding, io.tick()),
                        "scheduler-waiting-for-input",
                    ) {
                        *self.host_failure.borrow_mut() = Some(error);
                        return SchedulerStep::Failed {
                            code: Id("conduit/managed-lifecycle-projection-failed"),
                        };
                    }
                    return SchedulerStep::Pending;
                }
                if let Err(error) = validate_hosted_service_use_time(binding, io.tick()) {
                    *self.host_failure.borrow_mut() = Some(error);
                    return SchedulerStep::Failed {
                        code: Id("conduit/host-service-use-time-stale"),
                    };
                }
                if let Err(error) = set_managed_readiness(
                    managed,
                    ManagedRuntimeReadiness::Ready,
                    managed_tick(binding, io.tick()),
                    "scheduler-dispatched-provider",
                ) {
                    *self.host_failure.borrow_mut() = Some(error);
                    return SchedulerStep::Failed {
                        code: Id("conduit/managed-lifecycle-projection-failed"),
                    };
                }
                let values = inputs.iter().flatten().cloned().collect::<Vec<_>>();
                let step = match self.io.with_run_io(|run_io| {
                    handler.step(
                        node,
                        &values,
                        HostedServiceStepContext { tick: io.tick() },
                        run_io,
                    )
                }) {
                    Ok(step) => step,
                    Err(error) => {
                        *self.host_failure.borrow_mut() = Some(error);
                        return SchedulerStep::Failed {
                            code: Id("conduit/host-service-failed"),
                        };
                    }
                };
                inputs.clear();
                let (outputs, terminal) = match step {
                    HostedServiceStep::Produced { outputs } => {
                        if let Err(error) = set_managed_readiness(
                            managed,
                            ManagedRuntimeReadiness::Ready,
                            managed_tick(binding, io.tick()),
                            "provider-produced",
                        ) {
                            *self.host_failure.borrow_mut() = Some(error);
                            return SchedulerStep::Failed {
                                code: Id("conduit/managed-lifecycle-projection-failed"),
                            };
                        }
                        (outputs, false)
                    }
                    HostedServiceStep::Completed { outputs } => {
                        if let Err(error) = set_managed_readiness(
                            managed,
                            ManagedRuntimeReadiness::Ready,
                            managed_tick(binding, io.tick()),
                            "provider-completed-work",
                        ) {
                            *self.host_failure.borrow_mut() = Some(error);
                            return SchedulerStep::Failed {
                                code: Id("conduit/managed-lifecycle-projection-failed"),
                            };
                        }
                        (outputs, true)
                    }
                    HostedServiceStep::Waiting { interests } => {
                        if let Err(error) = set_managed_readiness(
                            managed,
                            ManagedRuntimeReadiness::Waiting,
                            managed_tick(binding, io.tick()),
                            "provider-waiting",
                        ) {
                            *self.host_failure.borrow_mut() = Some(error);
                            return SchedulerStep::Failed {
                                code: Id("conduit/managed-lifecycle-projection-failed"),
                            };
                        }
                        for interest in interests {
                            let wake = match interest {
                                HostedServiceInterest::Timer {
                                    subject,
                                    deadline_tick,
                                } => io.wait_for_timer(subject, deadline_tick),
                                HostedServiceInterest::HostOperation { subject } => {
                                    io.wait_for_host_operation(subject)
                                }
                            };
                            if let Err(error) = wake {
                                *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                    "CND-RUN-004",
                                    format!(
                                        "host service `{}` registered an invalid exact wake: {error}",
                                        node.id
                                    ),
                                ));
                                return SchedulerStep::Failed {
                                    code: Id("conduit/host-service-wake-invalid"),
                                };
                            }
                        }
                        return SchedulerStep::Pending;
                    }
                };
                if outputs.len() != output_routes.len()
                    || outputs
                        .iter()
                        .zip(output_routes.iter())
                        .any(|(output, (expected, _))| output.value_type != *expected)
                {
                    *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                        "CND-RUN-004",
                        format!(
                            "host service `{}` emitted outputs outside its exact contract",
                            node.id
                        ),
                    ));
                    return SchedulerStep::Failed {
                        code: Id("conduit/host-service-output-mismatch"),
                    };
                }
                for (output, (_, cords)) in outputs.into_iter().zip(output_routes.iter()) {
                    if cords.is_empty() {
                        continue;
                    }
                    let accounted_bytes = match u32::try_from(output.bytes.len()) {
                        Ok(bytes) => bytes,
                        Err(_) => {
                            return SchedulerStep::Failed {
                                code: Id("conduit/value-store-bound-exceeded"),
                            };
                        }
                    };
                    let Some(handle) = self.store.borrow_mut().store(&output.bytes) else {
                        return SchedulerStep::Failed {
                            code: Id("conduit/value-store-bound-exceeded"),
                        };
                    };
                    pending_outputs.push(HostedServiceOutput {
                        value: RuntimeValue {
                            handle,
                            accounted_bytes,
                            envelope: RuntimeValueEnvelope::EMPTY,
                        },
                        cords: cords.clone(),
                        next_cord: 0,
                    });
                }
                *completion_pending = terminal;
                if pending_outputs.is_empty() {
                    if terminal {
                        drive_hosted_service_cleanup(
                            handler.as_mut(),
                            node,
                            binding,
                            managed,
                            managed_stop_request,
                            cleanup,
                            completed,
                            self.cancellation_ticks,
                            io,
                            &self.host_failure,
                        )
                    } else {
                        SchedulerStep::Progress
                    }
                } else {
                    // A provider cannot claim progress merely by retaining an
                    // output in its own state: the current scheduler step
                    // must either reserve the first exact cord or register
                    // the corresponding output wait. Subsequent branches are
                    // flushed by the pending-output path above.
                    let output = pending_outputs
                        .first_mut()
                        .expect("nonempty hosted-service outputs have a first value");
                    let out_cord = output
                        .cords
                        .get(output.next_cord)
                        .copied()
                        .expect("hosted-service pending output has an exact cord");
                    match io.send(out_cord, output.value, None) {
                        Ok(SendStatus::Reserved) => {
                            output.next_cord += 1;
                            if pending_outputs
                                .iter()
                                .all(|output| output.next_cord == output.cords.len())
                            {
                                pending_outputs.clear();
                                if *completion_pending {
                                    drive_hosted_service_cleanup(
                                        handler.as_mut(),
                                        node,
                                        binding,
                                        managed,
                                        managed_stop_request,
                                        cleanup,
                                        completed,
                                        self.cancellation_ticks,
                                        io,
                                        &self.host_failure,
                                    )
                                } else {
                                    SchedulerStep::Progress
                                }
                            } else {
                                SchedulerStep::Progress
                            }
                        }
                        Ok(SendStatus::WouldBlock) => {
                            let _ = io.wait_for_output(out_cord);
                            SchedulerStep::Pending
                        }
                        status => {
                            *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                "CND-RUN-004",
                                format!(
                                    "host service `{}` output was rejected by its exact cord: {status:?}",
                                    node.id,
                                ),
                            ));
                            SchedulerStep::Failed {
                                code: Id("conduit/host-service-output-failed"),
                            }
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
                    let Some(handle) = self.store.borrow_mut().store(&upper_bytes) else {
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
                    if self
                        .io
                        .write_channel(ExactRunIoChannel::Output, bytes)
                        .is_err()
                    {
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
                    if self
                        .io
                        .write_channel(ExactRunIoChannel::Error, bytes)
                        .is_err()
                    {
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
            HostedNodeKind::DisplayText => {
                let in_cord = match self.in_cords.first() {
                    Some(&c) => c,
                    None => return SchedulerStep::Completed,
                };
                if let Ok(Some(val)) = io.receive(in_cord) {
                    let store = self.store.borrow();
                    let bytes = store.get(val.handle).unwrap_or(&[]);
                    if std::str::from_utf8(bytes).is_err()
                        || self
                            .io
                            .write_channel(ExactRunIoChannel::Display, bytes)
                            .is_err()
                    {
                        return SchedulerStep::Failed {
                            code: Id("display/text-write-error"),
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
            HostedNodeKind::Discard => {
                let in_cord = match self.in_cords.first() {
                    Some(&cord) => cord,
                    None => return SchedulerStep::Completed,
                };
                if matches!(io.receive(in_cord), Ok(Some(_))) {
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
            HostedNodeKind::DataUtf8 {
                utf8,
                pending,
                cursor,
                validated,
                maximum_input_bytes,
                maximum_output_bytes,
            } => {
                let Some(&in_cord) = self.in_cords.first() else {
                    return SchedulerStep::Completed;
                };
                let Some(&out_cord) = self.out_cords.first() else {
                    return SchedulerStep::Completed;
                };
                if pending.is_none() {
                    match io.receive(in_cord) {
                        Ok(Some(value)) => {
                            if value.accounted_bytes as usize > *maximum_input_bytes
                                || value.accounted_bytes as usize > *maximum_output_bytes
                            {
                                *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                    "CND-DAT-012",
                                    "UTF-8 value exceeded the exact codec bounds",
                                ));
                                return SchedulerStep::Failed {
                                    code: Id("CND-DAT-012"),
                                };
                            }
                            *pending = Some(value);
                            *cursor = 0;
                            *validated = false;
                            utf8.reset();
                        }
                        _ if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) => {
                            return SchedulerStep::Completed;
                        }
                        _ => {
                            let _ = io.wait_for_input(in_cord);
                            return SchedulerStep::Pending;
                        }
                    }
                }
                let value = pending.expect("UTF-8 input is retained");
                while !*validated && io.remaining_work() > 0 {
                    let byte = {
                        let store = self.store.borrow();
                        store
                            .get(value.handle)
                            .and_then(|bytes| bytes.get(*cursor))
                            .copied()
                    };
                    let Some(byte) = byte else {
                        if *cursor != value.accounted_bytes as usize || utf8.finish().is_err() {
                            *self.host_failure.borrow_mut() =
                                Some(RuntimeError::new("CND-DAT-005", "value is not exact UTF-8"));
                            return SchedulerStep::Failed {
                                code: Id("CND-DAT-005"),
                            };
                        }
                        *validated = true;
                        break;
                    };
                    if io.consume_work(1).is_err() || utf8.push_byte(byte).is_err() {
                        *self.host_failure.borrow_mut() =
                            Some(RuntimeError::new("CND-DAT-005", "value is not exact UTF-8"));
                        return SchedulerStep::Failed {
                            code: Id("CND-DAT-005"),
                        };
                    }
                    *cursor += 1;
                }
                if !*validated {
                    return if io.record_host_progress().is_ok() {
                        SchedulerStep::Progress
                    } else {
                        SchedulerStep::Failed {
                            code: Id("conduit/step-work-bound-exceeded"),
                        }
                    };
                }
                match io.send(out_cord, value, None) {
                    Ok(SendStatus::Reserved) => {
                        *pending = None;
                        *cursor = 0;
                        *validated = false;
                        utf8.reset();
                        SchedulerStep::Progress
                    }
                    Ok(_) | Err(_) => {
                        let _ = io.wait_for_output(out_cord);
                        SchedulerStep::Pending
                    }
                }
            }
            HostedNodeKind::ValidateClosedRecord {
                candidate_cord,
                decision_cord,
                candidate,
                decision,
                maximum_fields,
                maximum_field_name_bytes,
                maximum_field_value_bytes,
                maximum_work,
            } => {
                if candidate.is_some() || decision.is_some() {
                    let mut blocked = false;
                    let mut published = false;
                    if let Some(candidate_value) = *candidate {
                        if let Some(cord) = *candidate_cord {
                            match stage_validation_output(
                                io.send(cord, candidate_value, None),
                                candidate,
                            ) {
                                Ok(ValidationSendOutcome::Published) => published = true,
                                Ok(ValidationSendOutcome::Blocked) => {
                                    blocked = true;
                                    let _ = io.wait_for_output(cord);
                                }
                                Err(code) => return SchedulerStep::Failed { code },
                            }
                        } else {
                            *candidate = None;
                        }
                    }
                    if let Some(decision_value) = *decision {
                        if let Some(cord) = *decision_cord {
                            match stage_validation_output(
                                io.send(cord, decision_value, None),
                                decision,
                            ) {
                                Ok(ValidationSendOutcome::Published) => published = true,
                                Ok(ValidationSendOutcome::Blocked) => {
                                    blocked = true;
                                    let _ = io.wait_for_output(cord);
                                }
                                Err(code) => return SchedulerStep::Failed { code },
                            }
                        } else {
                            *decision = None;
                        }
                    }
                    return if published {
                        SchedulerStep::Progress
                    } else if blocked {
                        SchedulerStep::Pending
                    } else if io.record_host_progress().is_ok() {
                        SchedulerStep::Progress
                    } else {
                        SchedulerStep::Failed {
                            code: Id("conduit/step-work-bound-exceeded"),
                        }
                    };
                }
                let Some(&in_cord) = self.in_cords.first() else {
                    return SchedulerStep::Completed;
                };
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
                if value.accounted_bytes as usize > *maximum_work
                    || io.consume_work(value.accounted_bytes).is_err()
                {
                    *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                        "CND-DAT-014",
                        "record validation exceeded exact work",
                    ));
                    return SchedulerStep::Failed {
                        code: Id("CND-DAT-014"),
                    };
                }
                let structural = {
                    let store = self.store.borrow();
                    let Some(bytes) = store.get(value.handle) else {
                        return SchedulerStep::Failed {
                            code: Id("conduit/value-store-missing"),
                        };
                    };
                    conduit_std::validate_closed_record_bytes(
                        bytes,
                        CLOSED_RECORD_REQUIRED_FIELDS,
                        *maximum_fields,
                        *maximum_field_name_bytes,
                        *maximum_field_value_bytes,
                        *maximum_work,
                    )
                };
                let structural = match structural {
                    Ok(decision) => decision,
                    Err(error) => {
                        *self.host_failure.borrow_mut() = Some(data_boundary_runtime_error(error));
                        return SchedulerStep::Failed {
                            code: Id(error.code()),
                        };
                    }
                };
                let encoded = encode_structural_decision(structural);
                let accounted_bytes = encoded.len() as u32;
                let Some(handle) = self.store.borrow_mut().store(&encoded) else {
                    return SchedulerStep::Failed {
                        code: Id("conduit/value-store-bound-exceeded"),
                    };
                };
                *candidate = Some(value);
                *decision = Some(RuntimeValue {
                    handle,
                    accounted_bytes,
                    envelope: RuntimeValueEnvelope::EMPTY,
                });
                SchedulerStep::Progress
            }
            HostedNodeKind::ValidationDecisionAssert { expected } => {
                let Some(&in_cord) = self.in_cords.first() else {
                    return SchedulerStep::Completed;
                };
                match io.receive(in_cord) {
                    Ok(Some(value)) => {
                        let actual = {
                            let store = self.store.borrow();
                            store
                                .get(value.handle)
                                .ok_or_else(|| {
                                    RuntimeError::new(
                                        "conduit/value-store-missing",
                                        "validation decision value is missing",
                                    )
                                })
                                .and_then(validation_decision_name)
                        };
                        match actual {
                            Ok(actual) if actual == expected => SchedulerStep::Progress,
                            Ok(actual) => {
                                *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                    "CND-DAT-015",
                                    format!("expected `{expected}` but received `{actual}`"),
                                ));
                                SchedulerStep::Failed {
                                    code: Id("CND-DAT-015"),
                                }
                            }
                            Err(error) => {
                                *self.host_failure.borrow_mut() = Some(error);
                                SchedulerStep::Failed {
                                    code: Id("CND-DAT-013"),
                                }
                            }
                        }
                    }
                    _ if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) => {
                        SchedulerStep::Completed
                    }
                    _ => {
                        let _ = io.wait_for_input(in_cord);
                        SchedulerStep::Pending
                    }
                }
            }
            HostedNodeKind::FrameLengthU32Be {
                input,
                cursor,
                output,
                pending_output,
                maximum_frame_bytes,
                maximum_output_bytes,
            } => {
                let Some(&in_cord) = self.in_cords.first() else {
                    return SchedulerStep::Completed;
                };
                let Some(&out_cord) = self.out_cords.first() else {
                    return SchedulerStep::Completed;
                };
                if let Some(value) = *pending_output {
                    return match io.send(out_cord, value, None) {
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
                if input.is_none() {
                    match io.receive(in_cord) {
                        Ok(Some(value)) => {
                            if value.accounted_bytes as usize > *maximum_frame_bytes {
                                *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                    "CND-DAT-001",
                                    "frame payload exceeded the exact bound",
                                ));
                                return SchedulerStep::Failed {
                                    code: Id("CND-DAT-001"),
                                };
                            }
                            *input = Some(value);
                            *cursor = 0;
                            output.clear();
                        }
                        _ if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) => {
                            return SchedulerStep::Completed;
                        }
                        _ => {
                            let _ = io.wait_for_input(in_cord);
                            return SchedulerStep::Pending;
                        }
                    }
                }
                let value = input.expect("frame input is retained");
                let payload_len = value.accounted_bytes as usize;
                let total = conduit_std::LENGTH_U32BE_PREFIX_BYTES + payload_len;
                if total > *maximum_output_bytes {
                    *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                        "CND-DAT-002",
                        "framed output exceeded the exact bound",
                    ));
                    return SchedulerStep::Failed {
                        code: Id("CND-DAT-002"),
                    };
                }
                let prefix = (payload_len as u32).to_be_bytes();
                while *cursor < total && io.remaining_work() > 0 {
                    let byte = if *cursor < prefix.len() {
                        prefix[*cursor]
                    } else {
                        let payload_index = *cursor - prefix.len();
                        let store = self.store.borrow();
                        let Some(byte) = store
                            .get(value.handle)
                            .and_then(|bytes| bytes.get(payload_index))
                            .copied()
                        else {
                            return SchedulerStep::Failed {
                                code: Id("conduit/value-store-missing"),
                            };
                        };
                        byte
                    };
                    if io.consume_work(1).is_err() {
                        return SchedulerStep::Failed {
                            code: Id("conduit/step-work-bound-exceeded"),
                        };
                    }
                    output.push(byte);
                    *cursor += 1;
                }
                if *cursor < total {
                    return if io.record_host_progress().is_ok() {
                        SchedulerStep::Progress
                    } else {
                        SchedulerStep::Failed {
                            code: Id("conduit/step-work-bound-exceeded"),
                        }
                    };
                }
                let bytes = core::mem::take(output);
                let accounted_bytes = bytes.len() as u32;
                let Some(handle) = self.store.borrow_mut().store(&bytes) else {
                    return SchedulerStep::Failed {
                        code: Id("conduit/value-store-bound-exceeded"),
                    };
                };
                *input = None;
                *cursor = 0;
                *pending_output = Some(RuntimeValue {
                    handle,
                    accounted_bytes,
                    envelope: value.envelope,
                });
                if io.record_host_progress().is_ok() {
                    SchedulerStep::Progress
                } else {
                    SchedulerStep::Failed {
                        code: Id("conduit/step-work-bound-exceeded"),
                    }
                }
            }
            HostedNodeKind::DeframeLengthU32Be {
                decoder,
                input,
                cursor,
                pending_output,
                terminal_seen,
            } => {
                let Some(&in_cord) = self.in_cords.first() else {
                    return SchedulerStep::Completed;
                };
                let Some(&out_cord) = self.out_cords.first() else {
                    return SchedulerStep::Completed;
                };
                if let Some(value) = *pending_output {
                    return match io.send(out_cord, value, None) {
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
                if input.is_none() && !*terminal_seen {
                    match io.receive(in_cord) {
                        Ok(Some(value)) => {
                            *input = Some(value);
                            *cursor = 0;
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
                if let Some(value) = *input {
                    while *cursor < value.accounted_bytes as usize && io.remaining_work() > 0 {
                        let byte = {
                            let store = self.store.borrow();
                            store
                                .get(value.handle)
                                .and_then(|bytes| bytes.get(*cursor))
                                .copied()
                        };
                        let Some(byte) = byte else {
                            return SchedulerStep::Failed {
                                code: Id("conduit/value-store-missing"),
                            };
                        };
                        if io.consume_work(1).is_err() {
                            return SchedulerStep::Failed {
                                code: Id("conduit/step-work-bound-exceeded"),
                            };
                        }
                        *cursor += 1;
                        match decoder.push_byte(byte) {
                            Ok(true) => {
                                let length =
                                    decoder.ready_len().expect("ready decoder has exact length");
                                let mut bytes = vec![0; length];
                                decoder
                                    .take_ready(&mut bytes)
                                    .map_err(data_boundary_runtime_error)
                                    .unwrap_or_else(|error| {
                                        *self.host_failure.borrow_mut() = Some(error);
                                        None
                                    });
                                if self.host_failure.borrow().is_some() {
                                    return SchedulerStep::Failed {
                                        code: Id("CND-DAT-002"),
                                    };
                                }
                                let Some(handle) = self.store.borrow_mut().store(&bytes) else {
                                    return SchedulerStep::Failed {
                                        code: Id("conduit/value-store-bound-exceeded"),
                                    };
                                };
                                *pending_output = Some(RuntimeValue {
                                    handle,
                                    accounted_bytes: length as u32,
                                    envelope: value.envelope,
                                });
                                if *cursor == value.accounted_bytes as usize {
                                    *input = None;
                                    *cursor = 0;
                                }
                                return if io.record_host_progress().is_ok() {
                                    SchedulerStep::Progress
                                } else {
                                    SchedulerStep::Failed {
                                        code: Id("conduit/step-work-bound-exceeded"),
                                    }
                                };
                            }
                            Ok(false) => {}
                            Err(error) => {
                                *self.host_failure.borrow_mut() =
                                    Some(data_boundary_runtime_error(error));
                                return SchedulerStep::Failed {
                                    code: Id(error.code()),
                                };
                            }
                        }
                    }
                    if input.is_some() && *cursor == value.accounted_bytes as usize {
                        *input = None;
                        *cursor = 0;
                        return if io.record_host_progress().is_ok() {
                            SchedulerStep::Progress
                        } else {
                            SchedulerStep::Failed {
                                code: Id("conduit/step-work-bound-exceeded"),
                            }
                        };
                    }
                    if input.is_some() {
                        return if io.record_host_progress().is_ok() {
                            SchedulerStep::Progress
                        } else {
                            SchedulerStep::Failed {
                                code: Id("conduit/step-work-bound-exceeded"),
                            }
                        };
                    }
                }
                if *terminal_seen {
                    match decoder.finish() {
                        Ok(()) => SchedulerStep::Completed,
                        Err(error) => {
                            *self.host_failure.borrow_mut() =
                                Some(data_boundary_runtime_error(error));
                            SchedulerStep::Failed {
                                code: Id(error.code()),
                            }
                        }
                    }
                } else {
                    SchedulerStep::Progress
                }
            }
            HostedNodeKind::StateCell {
                update_cord,
                command_cord,
                initial_bytes,
                initial_value,
                current,
                pending_output,
                emit_initial,
                initialized,
            } => {
                let Some(&out_cord) = self.out_cords.first() else {
                    return SchedulerStep::Completed;
                };
                if !*initialized {
                    *initialized = true;
                    if let Some(bytes) = initial_bytes.take() {
                        let Some(handle) = self.store.borrow_mut().store(&bytes) else {
                            return SchedulerStep::Failed {
                                code: Id("conduit/value-store-bound-exceeded"),
                            };
                        };
                        let value = RuntimeValue {
                            handle,
                            accounted_bytes: bytes.len() as u32,
                            envelope: RuntimeValueEnvelope::EMPTY,
                        };
                        *initial_value = Some(value);
                        *current = Some(value);
                        if *emit_initial {
                            *pending_output = Some(value);
                        }
                    }
                    return recorded_time_progress(io);
                }
                if let Some(value) = *pending_output {
                    return match io.send(out_cord, value, None) {
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
                if let Some(command_cord) = *command_cord
                    && let Ok(Some(command)) = io.receive(command_cord)
                {
                    let command_bytes = self
                        .store
                        .borrow()
                        .get(command.handle)
                        .unwrap_or_default()
                        .to_vec();
                    match command_bytes.as_slice() {
                        b"get" => *pending_output = *current,
                        b"reset" => {
                            *current = *initial_value;
                            *pending_output = *current;
                        }
                        _ => {
                            *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                "CND-STA-020",
                                "cell command must be `get` or `reset`",
                            ));
                            return SchedulerStep::Failed {
                                code: Id("CND-STA-020"),
                            };
                        }
                    }
                    return SchedulerStep::Progress;
                }
                if let Ok(Some(value)) = io.receive(*update_cord) {
                    *current = Some(value);
                    *pending_output = Some(value);
                    return SchedulerStep::Progress;
                }
                let update_complete =
                    matches!(io.input_state(*update_cord), Ok(FlowQueueState::Completed));
                let command_complete = command_cord.is_none_or(|cord| {
                    matches!(io.input_state(cord), Ok(FlowQueueState::Completed))
                });
                if update_complete && command_complete {
                    return SchedulerStep::Completed;
                }
                if !update_complete {
                    let _ = io.wait_for_input(*update_cord);
                }
                if let Some(command_cord) = *command_cord
                    && !command_complete
                {
                    let _ = io.wait_for_input(command_cord);
                }
                SchedulerStep::Pending
            }
            HostedNodeKind::StateDeduplicate {
                state,
                pending_output,
            } => {
                let Some(&in_cord) = self.in_cords.first() else {
                    return SchedulerStep::Completed;
                };
                let Some(&out_cord) = self.out_cords.first() else {
                    return SchedulerStep::Completed;
                };
                if let Some(value) = *pending_output {
                    return match io.send(out_cord, value, None) {
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
                if let Ok(Some(value)) = io.receive(in_cord) {
                    let identity: conduit_std::StateIdentity = {
                        let store = self.store.borrow();
                        Sha256::digest(store.get(value.handle).unwrap_or_default()).into()
                    };
                    match state.admit(identity, value.accounted_bytes) {
                        Ok(conduit_std::DeduplicateDecision::Unique { .. }) => {
                            *pending_output = Some(value);
                        }
                        Ok(conduit_std::DeduplicateDecision::Duplicate) => {}
                        Err(error) => {
                            *self.host_failure.borrow_mut() =
                                Some(state_runtime_error(error).clone());
                            return SchedulerStep::Failed {
                                code: Id(error.code()),
                            };
                        }
                    }
                    return SchedulerStep::Progress;
                }
                if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) {
                    return SchedulerStep::Completed;
                }
                let _ = io.wait_for_input(in_cord);
                SchedulerStep::Pending
            }
            HostedNodeKind::StateCache {
                state,
                envelopes,
                retained_values,
                pending_output,
                maximum_key_bytes,
                maximum_value_bytes,
            } => {
                let Some(&in_cord) = self.in_cords.first() else {
                    return SchedulerStep::Completed;
                };
                let Some(&out_cord) = self.out_cords.first() else {
                    return SchedulerStep::Completed;
                };
                if let Some(value) = *pending_output {
                    return match io.send(out_cord, value, None) {
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
                if let Ok(Some(request_value)) = io.receive(in_cord) {
                    let request_bytes = self
                        .store
                        .borrow()
                        .get(request_value.handle)
                        .unwrap_or_default()
                        .to_vec();
                    let request = match parse_state_cache_request(&request_bytes) {
                        Ok(request) => request,
                        Err(error) => {
                            *self.host_failure.borrow_mut() = Some(error.clone());
                            return SchedulerStep::Failed {
                                code: Id("CND-STA-021"),
                            };
                        }
                    };
                    let response = match request {
                        StateCacheRequest::Put { key, value } => {
                            if key.len() > *maximum_key_bytes || value.len() > *maximum_value_bytes
                            {
                                *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                    "CND-STA-002",
                                    "cache request exceeds its exact key/value bound",
                                ));
                                return SchedulerStep::Failed {
                                    code: Id("CND-STA-002"),
                                };
                            }
                            let identity: conduit_std::StateIdentity = Sha256::digest(key).into();
                            let Some(handle) = self.store.borrow_mut().store(value) else {
                                return SchedulerStep::Failed {
                                    code: Id("conduit/value-store-bound-exceeded"),
                                };
                            };
                            let retained_value = RuntimeValue {
                                handle,
                                accounted_bytes: u32::try_from(value.len()).unwrap_or(u32::MAX),
                                envelope: RuntimeValueEnvelope::EMPTY,
                            };
                            match state.insert(conduit_std::CacheEntry {
                                key: identity,
                                value_handle: handle,
                                value_bytes: value.len() as u32,
                            }) {
                                Ok(conduit_std::CacheInsert::Inserted { evicted }) => {
                                    if let Some(evicted) = evicted {
                                        envelopes.retain(|(key, _)| *key != evicted);
                                        retained_values.retain(|(key, _)| *key != evicted);
                                    }
                                }
                                Ok(conduit_std::CacheInsert::Updated) => {
                                    envelopes.retain(|(key, _)| *key != identity);
                                    retained_values.retain(|(key, _)| *key != identity);
                                }
                                Err(error) => {
                                    *self.host_failure.borrow_mut() =
                                        Some(state_runtime_error(error).clone());
                                    return SchedulerStep::Failed {
                                        code: Id(error.code()),
                                    };
                                }
                            }
                            envelopes.push((identity, request_value.envelope));
                            retained_values.push((identity, retained_value));
                            state_response_value(&self.store, b"stored", request_value.envelope)
                        }
                        StateCacheRequest::Get { key } => {
                            if key.len() > *maximum_key_bytes {
                                *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                    "CND-STA-002",
                                    "cache key exceeds its exact bound",
                                ));
                                return SchedulerStep::Failed {
                                    code: Id("CND-STA-002"),
                                };
                            }
                            let identity: conduit_std::StateIdentity = Sha256::digest(key).into();
                            if let Some(entry) = state.lookup(identity) {
                                let envelope = envelopes
                                    .iter()
                                    .find(|(key, _)| *key == identity)
                                    .map_or(RuntimeValueEnvelope::EMPTY, |(_, envelope)| *envelope);
                                Ok(RuntimeValue {
                                    handle: entry.value_handle,
                                    accounted_bytes: entry.value_bytes,
                                    envelope,
                                })
                            } else {
                                state_response_value(&self.store, b"miss", request_value.envelope)
                            }
                        }
                        StateCacheRequest::Invalidate { key } => {
                            let identity: conduit_std::StateIdentity = Sha256::digest(key).into();
                            let removed = state.invalidate(identity);
                            if removed {
                                envelopes.retain(|(key, _)| *key != identity);
                                retained_values.retain(|(key, _)| *key != identity);
                            }
                            state_response_value(
                                &self.store,
                                if removed { b"invalidated" } else { b"miss" },
                                request_value.envelope,
                            )
                        }
                        StateCacheRequest::Reset => {
                            state.restart();
                            envelopes.clear();
                            retained_values.clear();
                            state_response_value(&self.store, b"reset", request_value.envelope)
                        }
                    };
                    match response {
                        Ok(response) => *pending_output = Some(response),
                        Err(error) => {
                            *self.host_failure.borrow_mut() = Some(error.clone());
                            return SchedulerStep::Failed {
                                code: Id("conduit/value-store-bound-exceeded"),
                            };
                        }
                    }
                    return SchedulerStep::Progress;
                }
                if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) {
                    return SchedulerStep::Completed;
                }
                let _ = io.wait_for_input(in_cord);
                SchedulerStep::Pending
            }
            HostedNodeKind::SupervisionRetry {
                request_cord,
                terminal_cord,
                entropy_cord,
                request,
                pending_output,
                pending_outcome,
                state,
                awaiting_outcome,
                maximum_attempts,
                deadline_ticks,
                policy,
                permission,
                committed_replay_permitted,
                generation,
            } => {
                let Some(&out_cord) = self.out_cords.first() else {
                    return SchedulerStep::Completed;
                };
                if let Some(value) = *pending_output {
                    return match io.send(out_cord, value, None) {
                        Ok(SendStatus::Reserved) => {
                            *pending_output = None;
                            *awaiting_outcome = true;
                            SchedulerStep::Progress
                        }
                        Ok(_) | Err(_) => {
                            let _ = io.wait_for_output(out_cord);
                            SchedulerStep::Pending
                        }
                    };
                }
                if let Some(outcome) = *pending_outcome {
                    let entropy = if policy.jitter_ticks == 0 {
                        None
                    } else {
                        let Some(entropy_cord) = *entropy_cord else {
                            *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                "CND-SVP-008",
                                "retry jitter requires an exact injected entropy cord",
                            ));
                            return SchedulerStep::Failed {
                                code: Id("CND-SVP-008"),
                            };
                        };
                        let entropy = match io.receive(entropy_cord) {
                            Ok(Some(value)) => {
                                let bytes = self
                                    .store
                                    .borrow()
                                    .get(value.handle)
                                    .unwrap_or_default()
                                    .to_vec();
                                match std::str::from_utf8(&bytes)
                                    .ok()
                                    .and_then(|text| text.parse::<u64>().ok())
                                {
                                    Some(entropy) => entropy,
                                    None => {
                                        *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                            "CND-SVP-008",
                                            "injected entropy must be one exact u64",
                                        ));
                                        return SchedulerStep::Failed {
                                            code: Id("CND-SVP-008"),
                                        };
                                    }
                                }
                            }
                            _ if matches!(
                                io.input_state(entropy_cord),
                                Ok(FlowQueueState::Completed)
                            ) =>
                            {
                                *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                    "CND-SVP-008",
                                    "retry entropy ended before the retry decision",
                                ));
                                return SchedulerStep::Failed {
                                    code: Id("CND-SVP-008"),
                                };
                            }
                            _ => {
                                let _ = io.wait_for_input(entropy_cord);
                                return SchedulerStep::Pending;
                            }
                        };
                        Some(entropy)
                    };
                    let Some(retry) = state.as_mut() else {
                        return SchedulerStep::Failed {
                            code: Id("CND-SVP-009"),
                        };
                    };
                    let decision = match retry.observe(io.tick(), outcome, entropy) {
                        Ok(decision) => decision,
                        Err(error) => {
                            *self.host_failure.borrow_mut() =
                                Some(supervision_runtime_error(error));
                            return SchedulerStep::Failed {
                                code: Id(error.code()),
                            };
                        }
                    };
                    *pending_outcome = None;
                    match decision {
                        conduit_std::RetryDecision::Succeeded { .. } => {
                            *request = None;
                            *state = None;
                            *awaiting_outcome = false;
                            return recorded_time_progress(io);
                        }
                        conduit_std::RetryDecision::Retry { .. } => {
                            *awaiting_outcome = false;
                            return recorded_time_progress(io);
                        }
                        conduit_std::RetryDecision::Exhausted { .. } => {
                            *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                "CND-SVP-005",
                                "retry attempt budget is exhausted",
                            ));
                            return SchedulerStep::Failed {
                                code: Id("CND-SVP-005"),
                            };
                        }
                    }
                }
                if !*awaiting_outcome
                    && let (Some(retry), Some(value)) = (state.as_mut(), *request)
                    && retry.next_not_before_tick().is_some()
                {
                    match retry.ready(io.tick()) {
                        Ok(true) => {
                            *pending_output = Some(value);
                            return recorded_time_progress(io);
                        }
                        Ok(false) => {
                            let deadline = retry
                                .next_not_before_tick()
                                .expect("pending retry owns one exact timer");
                            let _ = io.wait_for_timer(Id("conduit/supervision-backoff"), deadline);
                            return SchedulerStep::Pending;
                        }
                        Err(error) => {
                            *self.host_failure.borrow_mut() =
                                Some(supervision_runtime_error(error));
                            return SchedulerStep::Failed {
                                code: Id(error.code()),
                            };
                        }
                    }
                }
                if *awaiting_outcome {
                    match io.receive(*terminal_cord) {
                        Ok(Some(value)) => {
                            let bytes = self
                                .store
                                .borrow()
                                .get(value.handle)
                                .unwrap_or_default()
                                .to_vec();
                            *pending_outcome = match bytes.as_slice() {
                                b"success" => Some(conduit_std::AttemptOutcome::Succeeded),
                                b"eligible-failure" => {
                                    Some(conduit_std::AttemptOutcome::EligibleFailure)
                                }
                                b"committed-failure" => {
                                    Some(conduit_std::AttemptOutcome::CommittedFailure)
                                }
                                b"cancel" => {
                                    if let Some(retry) = state.as_mut() {
                                        retry.cancel();
                                    }
                                    *request = None;
                                    *state = None;
                                    *awaiting_outcome = false;
                                    return SchedulerStep::Progress;
                                }
                                _ => {
                                    *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                        "CND-SVP-020",
                                        "retry terminal must be success, eligible-failure, committed-failure, or cancel",
                                    ));
                                    return SchedulerStep::Failed {
                                        code: Id("CND-SVP-020"),
                                    };
                                }
                            };
                            return SchedulerStep::Progress;
                        }
                        _ if matches!(
                            io.input_state(*terminal_cord),
                            Ok(FlowQueueState::Completed)
                        ) =>
                        {
                            *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                "CND-SVP-020",
                                "retry terminal stream ended with an active attempt",
                            ));
                            return SchedulerStep::Failed {
                                code: Id("CND-SVP-020"),
                            };
                        }
                        _ => {
                            let _ = io.wait_for_input(*terminal_cord);
                            return SchedulerStep::Pending;
                        }
                    }
                }
                match io.receive(*request_cord) {
                    Ok(Some(value)) => {
                        let now = io.tick();
                        let Some(deadline_tick) = now.checked_add(*deadline_ticks) else {
                            return SchedulerStep::Failed {
                                code: Id("CND-SVP-003"),
                            };
                        };
                        *generation = match generation.checked_add(1) {
                            Some(generation) => generation,
                            None => {
                                return SchedulerStep::Failed {
                                    code: Id("CND-SVP-001"),
                                };
                            }
                        };
                        let retry = match conduit_std::RetryState::new(
                            *maximum_attempts,
                            deadline_tick,
                            *policy,
                            *permission,
                            *committed_replay_permitted,
                            *generation,
                        ) {
                            Ok(retry) => retry,
                            Err(error) => {
                                *self.host_failure.borrow_mut() =
                                    Some(supervision_runtime_error(error));
                                return SchedulerStep::Failed {
                                    code: Id(error.code()),
                                };
                            }
                        };
                        *request = Some(value);
                        *state = Some(retry);
                        *pending_output = Some(value);
                        SchedulerStep::Progress
                    }
                    _ if matches!(io.input_state(*request_cord), Ok(FlowQueueState::Completed))
                        && matches!(
                            io.input_state(*terminal_cord),
                            Ok(FlowQueueState::Completed)
                        ) =>
                    {
                        SchedulerStep::Completed
                    }
                    _ => {
                        let _ = io.wait_for_input(*request_cord);
                        SchedulerStep::Pending
                    }
                }
            }
            HostedNodeKind::SupervisionCircuitBreaker {
                request_cord,
                terminal_cord,
                pending_output,
                state,
                awaiting_outcome,
            } => {
                let Some(&out_cord) = self.out_cords.first() else {
                    return SchedulerStep::Completed;
                };
                if let Some(value) = *pending_output {
                    return match io.send(out_cord, value, None) {
                        Ok(SendStatus::Reserved) => {
                            *pending_output = None;
                            *awaiting_outcome = true;
                            SchedulerStep::Progress
                        }
                        Ok(_) | Err(_) => {
                            let _ = io.wait_for_output(out_cord);
                            SchedulerStep::Pending
                        }
                    };
                }
                if *awaiting_outcome {
                    match io.receive(*terminal_cord) {
                        Ok(Some(value)) => {
                            let bytes = self
                                .store
                                .borrow()
                                .get(value.handle)
                                .unwrap_or_default()
                                .to_vec();
                            if bytes == b"reset" {
                                state.reset();
                                *awaiting_outcome = false;
                                return SchedulerStep::Progress;
                            }
                            let outcome = match bytes.as_slice() {
                                b"success" => conduit_std::BreakerOutcome::Success,
                                b"eligible-failure" | b"committed-failure" => {
                                    conduit_std::BreakerOutcome::CountedFailure
                                }
                                b"ignored-failure" => conduit_std::BreakerOutcome::IgnoredFailure,
                                b"cancel" => {
                                    state.cancel();
                                    return SchedulerStep::Completed;
                                }
                                _ => {
                                    *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                        "CND-SVP-021",
                                        "breaker terminal must be success, eligible-failure, committed-failure, ignored-failure, reset, or cancel",
                                    ));
                                    return SchedulerStep::Failed {
                                        code: Id("CND-SVP-021"),
                                    };
                                }
                            };
                            if let Err(error) = state.observe(io.tick(), outcome) {
                                *self.host_failure.borrow_mut() =
                                    Some(supervision_runtime_error(error));
                                return SchedulerStep::Failed {
                                    code: Id(error.code()),
                                };
                            }
                            *awaiting_outcome = false;
                            return SchedulerStep::Progress;
                        }
                        _ if matches!(
                            io.input_state(*terminal_cord),
                            Ok(FlowQueueState::Completed)
                        ) =>
                        {
                            *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                "CND-SVP-021",
                                "breaker terminal stream ended with an admitted request",
                            ));
                            return SchedulerStep::Failed {
                                code: Id("CND-SVP-021"),
                            };
                        }
                        _ => {
                            let _ = io.wait_for_input(*terminal_cord);
                            return SchedulerStep::Pending;
                        }
                    }
                }
                if let conduit_std::BreakerState::Open { until_tick } = state.state()
                    && io.tick() < until_tick
                {
                    let _ =
                        io.wait_for_timer(Id("conduit/supervision-breaker-cooldown"), until_tick);
                    return SchedulerStep::Pending;
                }
                match io.receive(*request_cord) {
                    Ok(Some(value)) => match state.admit(io.tick()) {
                        Ok(conduit_std::BreakerAdmission::Admitted) => {
                            *pending_output = Some(value);
                            SchedulerStep::Progress
                        }
                        Ok(conduit_std::BreakerAdmission::RejectedOpen { until_tick }) => {
                            let _ = io.wait_for_timer(
                                Id("conduit/supervision-breaker-cooldown"),
                                until_tick,
                            );
                            SchedulerStep::Pending
                        }
                        Ok(conduit_std::BreakerAdmission::RejectedProbeLimit) => {
                            let _ = io.wait_for_input(*terminal_cord);
                            SchedulerStep::Pending
                        }
                        Err(error) => {
                            *self.host_failure.borrow_mut() =
                                Some(supervision_runtime_error(error));
                            SchedulerStep::Failed {
                                code: Id(error.code()),
                            }
                        }
                    },
                    _ if matches!(io.input_state(*request_cord), Ok(FlowQueueState::Completed))
                        && matches!(
                            io.input_state(*terminal_cord),
                            Ok(FlowQueueState::Completed)
                        ) =>
                    {
                        SchedulerStep::Completed
                    }
                    _ => {
                        let _ = io.wait_for_input(*request_cord);
                        SchedulerStep::Pending
                    }
                }
            }
            HostedNodeKind::TimeTransform {
                behavior,
                duration_ticks,
                deadline_tick,
                retained,
                pending_output,
                terminal_seen,
            } => {
                let Some(&in_cord) = self.in_cords.first() else {
                    return SchedulerStep::Completed;
                };
                let Some(&out_cord) = self.out_cords.first() else {
                    return SchedulerStep::Completed;
                };
                let now = io.tick();
                let arm = |host_failure: &RefCell<Option<RuntimeError>>| {
                    conduit_std::exact_deadline(now, *duration_ticks).map_err(|error| {
                        let runtime = time_runtime_error(error);
                        *host_failure.borrow_mut() = Some(runtime.clone());
                        runtime
                    })
                };
                if let Some(value) = *pending_output {
                    return match io.send(out_cord, value, None) {
                        Ok(SendStatus::Reserved) => {
                            *pending_output = None;
                            if matches!(behavior, TimeBehavior::Timeout) {
                                let Ok(deadline) = arm(&self.host_failure) else {
                                    return SchedulerStep::Failed {
                                        code: Id("CND-TIM-002"),
                                    };
                                };
                                *deadline_tick = Some(deadline);
                            }
                            SchedulerStep::Progress
                        }
                        Ok(_) | Err(_) => {
                            let _ = io.wait_for_output(out_cord);
                            SchedulerStep::Pending
                        }
                    };
                }

                match *behavior {
                    TimeBehavior::Delay { drop_at_terminal } => {
                        if retained.is_some()
                            && matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed))
                            && drop_at_terminal
                        {
                            *retained = None;
                            *deadline_tick = None;
                            return SchedulerStep::Completed;
                        }
                        if let Some(value) = *retained {
                            let deadline = deadline_tick.expect("delayed value owns a timer");
                            if now >= deadline {
                                *retained = None;
                                *deadline_tick = None;
                                *pending_output = Some(value);
                                return recorded_time_progress(io);
                            }
                            let _ = io.wait_for_timer(Id("conduit/time-timer"), deadline);
                            return SchedulerStep::Pending;
                        }
                        match io.receive(in_cord) {
                            Ok(Some(value)) => {
                                let Ok(deadline) = arm(&self.host_failure) else {
                                    return SchedulerStep::Failed {
                                        code: Id("CND-TIM-002"),
                                    };
                                };
                                *retained = Some(value);
                                *deadline_tick = Some(deadline);
                                SchedulerStep::Progress
                            }
                            _ if matches!(
                                io.input_state(in_cord),
                                Ok(FlowQueueState::Completed)
                            ) =>
                            {
                                SchedulerStep::Completed
                            }
                            _ => {
                                let _ = io.wait_for_input(in_cord);
                                SchedulerStep::Pending
                            }
                        }
                    }
                    TimeBehavior::Timeout => {
                        if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) {
                            return SchedulerStep::Completed;
                        }
                        if let Ok(Some(value)) = io.receive(in_cord) {
                            *pending_output = Some(value);
                            return SchedulerStep::Progress;
                        }
                        if deadline_tick.is_none() {
                            let Ok(deadline) = arm(&self.host_failure) else {
                                return SchedulerStep::Failed {
                                    code: Id("CND-TIM-002"),
                                };
                            };
                            *deadline_tick = Some(deadline);
                            return recorded_time_progress(io);
                        }
                        let deadline = deadline_tick.expect("timeout owns a timer");
                        if now >= deadline {
                            *self.host_failure.borrow_mut() = Some(RuntimeError::new(
                                "CND-TIM-020",
                                "the exact inactivity timeout elapsed",
                            ));
                            return SchedulerStep::Failed {
                                code: Id("CND-TIM-020"),
                            };
                        }
                        let _ = io.wait_for_input(in_cord);
                        let _ = io.wait_for_timer(Id("conduit/time-timer"), deadline);
                        SchedulerStep::Pending
                    }
                    TimeBehavior::Debounce {
                        mode,
                        flush_at_terminal,
                    } => {
                        if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) {
                            *terminal_seen = true;
                        }
                        if *terminal_seen {
                            *deadline_tick = None;
                            if flush_at_terminal {
                                *pending_output = retained.take();
                                return if pending_output.is_some() {
                                    recorded_time_progress(io)
                                } else {
                                    SchedulerStep::Completed
                                };
                            }
                            *retained = None;
                            return SchedulerStep::Completed;
                        }
                        if let Ok(Some(value)) = io.receive(in_cord) {
                            let leading_window_active = deadline_tick.is_some();
                            let Ok(deadline) = arm(&self.host_failure) else {
                                return SchedulerStep::Failed {
                                    code: Id("CND-TIM-002"),
                                };
                            };
                            *deadline_tick = Some(deadline);
                            match mode {
                                conduit_std::DebounceMode::Leading if !leading_window_active => {
                                    *pending_output = Some(value);
                                }
                                conduit_std::DebounceMode::Leading => {}
                                conduit_std::DebounceMode::Trailing => {
                                    *retained = Some(value);
                                }
                            }
                            return SchedulerStep::Progress;
                        }
                        if let Some(deadline) = *deadline_tick {
                            if now >= deadline {
                                *deadline_tick = None;
                                match mode {
                                    conduit_std::DebounceMode::Leading => {
                                        *retained = None;
                                    }
                                    conduit_std::DebounceMode::Trailing => {
                                        *pending_output = retained.take();
                                    }
                                }
                                return recorded_time_progress(io);
                            }
                            let _ = io.wait_for_timer(Id("conduit/time-timer"), deadline);
                        }
                        let _ = io.wait_for_input(in_cord);
                        SchedulerStep::Pending
                    }
                    TimeBehavior::Throttle {
                        mode,
                        flush_at_terminal,
                    } => {
                        if matches!(io.input_state(in_cord), Ok(FlowQueueState::Completed)) {
                            *terminal_seen = true;
                        }
                        if *terminal_seen {
                            *deadline_tick = None;
                            if flush_at_terminal {
                                *pending_output = retained.take();
                                return if pending_output.is_some() {
                                    recorded_time_progress(io)
                                } else {
                                    SchedulerStep::Completed
                                };
                            }
                            *retained = None;
                            return SchedulerStep::Completed;
                        }
                        if let Some(deadline) = *deadline_tick
                            && now >= deadline
                        {
                            *deadline_tick = None;
                            if matches!(mode, conduit_std::ThrottleMode::TrailingCoalesce) {
                                *pending_output = retained.take();
                            }
                            return recorded_time_progress(io);
                        }
                        match mode {
                            conduit_std::ThrottleMode::LeadingBlock => {
                                if let Some(deadline) = *deadline_tick {
                                    let _ = io.wait_for_timer(Id("conduit/time-timer"), deadline);
                                    return SchedulerStep::Pending;
                                }
                                if let Ok(Some(value)) = io.receive(in_cord) {
                                    let Ok(deadline) = arm(&self.host_failure) else {
                                        return SchedulerStep::Failed {
                                            code: Id("CND-TIM-002"),
                                        };
                                    };
                                    *deadline_tick = Some(deadline);
                                    *pending_output = Some(value);
                                    return SchedulerStep::Progress;
                                }
                            }
                            conduit_std::ThrottleMode::TrailingCoalesce => {
                                if let Ok(Some(value)) = io.receive(in_cord) {
                                    if deadline_tick.is_none() {
                                        let Ok(deadline) = arm(&self.host_failure) else {
                                            return SchedulerStep::Failed {
                                                code: Id("CND-TIM-002"),
                                            };
                                        };
                                        *deadline_tick = Some(deadline);
                                    }
                                    *retained = Some(value);
                                    return SchedulerStep::Progress;
                                }
                                if let Some(deadline) = *deadline_tick {
                                    let _ = io.wait_for_timer(Id("conduit/time-timer"), deadline);
                                }
                            }
                        }
                        let _ = io.wait_for_input(in_cord);
                        SchedulerStep::Pending
                    }
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

    fn begin_value_reconciliation(&mut self) {
        self.store.borrow_mut().begin_reconciliation();
    }

    fn mark_value_live(&mut self, value: RuntimeValue) {
        self.store.borrow_mut().mark_live(value);
    }

    fn mark_retained_values(&mut self) {
        let mut store = self.store.borrow_mut();
        self.kind.mark_retained_values(&mut store);
    }

    fn finish_value_reconciliation(&mut self) {
        self.store.borrow_mut().finish_reconciliation();
    }

    fn value_storage_usage(&self) -> Option<ValueStorageUsage> {
        Some(self.store.borrow().usage())
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
        temporal: port.temporal,
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
        schema_version: 0,
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
        values: port.values.as_str(),
        temporal: port.temporal.as_str(),
        terminal: port.terminal.as_str(),
        presence: port.presence.as_str(),
        sensitivity: port.sensitivity.as_str(),
        loss_acceptance: port.flow.loss.as_str(),
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
    /// Construct one stable provider/configuration resolution failure.
    #[must_use]
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
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
                .all(|cord| {
                    nodes[cord.from_node].definition.contract.outputs[cord.from_port].temporal
                        == TemporalContract::RetainedState
                        || completed[cord.from_node]
                })
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

fn required_time_bound(node: &Node, key: &str, maximum: u64) -> Result<u64, ResolutionError> {
    let Some(conduit_panel::SourceValue::Integer(value)) = node.config_value(key) else {
        return Err(ResolutionError::new(
            "CND-TIM-010",
            format!("node `{}` requires integer `{key}`", node.id),
        ));
    };
    let value = u64::try_from(*value).map_err(|_| {
        ResolutionError::new(
            "CND-TIM-012",
            format!("node `{}` has negative `{key}`", node.id),
        )
    })?;
    if value > maximum {
        return Err(ResolutionError::new(
            "CND-TIM-012",
            format!("node `{}` exceeds `{key}` provider bound", node.id),
        ));
    }
    Ok(value)
}

fn validate_time_common(node: &Node, expected: &[&str]) -> Result<(), ResolutionError> {
    if node.config.len() != expected.len()
        || node
            .config
            .iter()
            .any(|entry| !expected.contains(&entry.key.as_str()))
    {
        return Err(ResolutionError::new(
            "CND-TIM-010",
            format!("node `{}` has an incomplete exact clock profile", node.id),
        ));
    }
    if node.config("clock") != Some(TIME_CLOCK_DESCRIPTOR)
        || required_time_bound(node, "clock_schema_version", u32::MAX as u64)? != 0
    {
        return Err(ResolutionError::new(
            "CND-TIM-011",
            format!(
                "node `{}` requests an unsupported clock descriptor",
                node.id
            ),
        ));
    }
    let Some(conduit_panel::SourceValue::Bytes(hash)) = node.config_value("clock_hash") else {
        return Err(ResolutionError::new(
            "CND-TIM-010",
            format!("node `{}` requires bytes `clock_hash`", node.id),
        ));
    };
    if hash.as_slice() != TIME_CLOCK_HASH {
        return Err(ResolutionError::new(
            "CND-TIM-011",
            format!("node `{}` requests an unsupported clock hash", node.id),
        ));
    }
    if required_time_bound(node, "resolution_ticks", 1)? != 1 {
        return Err(ResolutionError::new(
            "CND-TIM-011",
            format!("node `{}` requires one-tick clock resolution", node.id),
        ));
    }
    required_time_bound(node, "duration_ticks", conduit_std::TIME_MAX_DURATION_TICKS)?;
    if node.config("discontinuity") != Some("fail") {
        return Err(ResolutionError::new(
            "CND-TIM-011",
            format!("node `{}` must fail on clock discontinuity", node.id),
        ));
    }
    Ok(())
}

fn validate_ticker(node: &Node) -> Result<(), ResolutionError> {
    const EXPECTED: &[&str] = &["duration_ticks", "time_basis", "maximum_pending"];
    if node.config.len() != EXPECTED.len()
        || node
            .config
            .iter()
            .any(|entry| !EXPECTED.contains(&entry.key.as_str()))
    {
        return Err(ResolutionError::new(
            "CND-TIM-010",
            format!("node `{}` has an incomplete ticker profile", node.id),
        ));
    }
    if node.config("time_basis") != Some(TIME_CLOCK_DESCRIPTOR) {
        return Err(ResolutionError::new(
            "CND-TIM-011",
            format!("node `{}` requests an unsupported ticker clock", node.id),
        ));
    }
    let duration_ticks =
        required_time_bound(node, "duration_ticks", conduit_std::TIME_MAX_DURATION_TICKS)?;
    if duration_ticks == 0 || required_time_bound(node, "maximum_pending", 1)? != 1 {
        return Err(ResolutionError::new(
            "CND-TIM-012",
            format!(
                "node `{}` requires one nonzero bounded ticker interval and one pending wake",
                node.id
            ),
        ));
    }
    Ok(())
}

fn validate_time_delay(node: &Node) -> Result<(), ResolutionError> {
    validate_time_common(
        node,
        &[
            "clock",
            "clock_schema_version",
            "clock_hash",
            "resolution_ticks",
            "duration_ticks",
            "maximum_pending",
            "terminal",
            "discontinuity",
        ],
    )?;
    if required_time_bound(node, "maximum_pending", 1)? != 1
        || !matches!(node.config("terminal"), Some("drain" | "drop"))
    {
        return Err(ResolutionError::new(
            "CND-TIM-012",
            format!(
                "node `{}` has unsupported delay bounds or terminal policy",
                node.id
            ),
        ));
    }
    Ok(())
}

fn validate_time_timeout(node: &Node) -> Result<(), ResolutionError> {
    validate_time_common(
        node,
        &[
            "clock",
            "clock_schema_version",
            "clock_hash",
            "resolution_ticks",
            "duration_ticks",
            "condition",
            "reset",
            "late",
            "discontinuity",
        ],
    )?;
    if node.config("condition") != Some("inactivity")
        || node.config("reset") != Some("each-value")
        || node.config("late") != Some("timeout")
    {
        return Err(ResolutionError::new(
            "CND-TIM-011",
            format!("node `{}` requests unsupported timeout semantics", node.id),
        ));
    }
    Ok(())
}

fn validate_time_debounce(node: &Node) -> Result<(), ResolutionError> {
    validate_time_common(
        node,
        &[
            "clock",
            "clock_schema_version",
            "clock_hash",
            "resolution_ticks",
            "duration_ticks",
            "mode",
            "loss",
            "terminal",
            "maximum_retained",
            "discontinuity",
        ],
    )?;
    if !matches!(node.config("mode"), Some("leading" | "trailing"))
        || node.config("loss") != Some("coalesce")
        || !matches!(node.config("terminal"), Some("flush" | "drop"))
        || required_time_bound(node, "maximum_retained", 1)? != 1
    {
        return Err(ResolutionError::new(
            "CND-TIM-012",
            format!("node `{}` has unsupported debounce semantics", node.id),
        ));
    }
    Ok(())
}

fn validate_time_throttle(node: &Node) -> Result<(), ResolutionError> {
    validate_time_common(
        node,
        &[
            "clock",
            "clock_schema_version",
            "clock_hash",
            "resolution_ticks",
            "duration_ticks",
            "mode",
            "overflow",
            "terminal",
            "maximum_retained",
            "discontinuity",
        ],
    )?;
    let mode = node.config("mode");
    let overflow = node.config("overflow");
    if !matches!(
        (mode, overflow),
        (Some("leading"), Some("block")) | (Some("trailing"), Some("coalesce"))
    ) || !matches!(node.config("terminal"), Some("flush" | "drop"))
        || required_time_bound(node, "maximum_retained", 1)? != 1
    {
        return Err(ResolutionError::new(
            "CND-TIM-012",
            format!("node `{}` has unsupported throttle semantics", node.id),
        ));
    }
    Ok(())
}

fn required_state_bound(node: &Node, key: &str, maximum: u64) -> Result<u64, ResolutionError> {
    let Some(conduit_panel::SourceValue::Integer(value)) = node.config_value(key) else {
        return Err(ResolutionError::new(
            "CND-STA-010",
            format!("node `{}` requires integer `{key}`", node.id),
        ));
    };
    let value = u64::try_from(*value).map_err(|_| {
        ResolutionError::new(
            "CND-STA-012",
            format!("node `{}` has negative `{key}`", node.id),
        )
    })?;
    if value == 0 || value > maximum {
        return Err(ResolutionError::new(
            "CND-STA-012",
            format!("node `{}` exceeds `{key}` provider bound", node.id),
        ));
    }
    Ok(value)
}

fn validate_state_descriptor(
    node: &Node,
    descriptor_key: &str,
    version_key: &str,
    hash_key: &str,
    expected_descriptor: &str,
    expected_hash: &[u8; 32],
) -> Result<(), ResolutionError> {
    let version_is_zero = matches!(
        node.config_value(version_key),
        Some(conduit_panel::SourceValue::Integer(0))
    );
    if node.config(descriptor_key) != Some(expected_descriptor) || !version_is_zero {
        return Err(ResolutionError::new(
            "CND-STA-011",
            format!("node `{}` requests an unsupported descriptor", node.id),
        ));
    }
    let Some(conduit_panel::SourceValue::Bytes(hash)) = node.config_value(hash_key) else {
        return Err(ResolutionError::new(
            "CND-STA-010",
            format!("node `{}` requires bytes `{hash_key}`", node.id),
        ));
    };
    if hash.as_slice() != expected_hash {
        return Err(ResolutionError::new(
            "CND-STA-011",
            format!("node `{}` requests a stale descriptor hash", node.id),
        ));
    }
    Ok(())
}

fn validate_exact_state_fields(node: &Node, expected: &[&str]) -> Result<(), ResolutionError> {
    if node.config.len() != expected.len()
        || node
            .config
            .iter()
            .any(|entry| !expected.contains(&entry.key.as_str()))
    {
        return Err(ResolutionError::new(
            "CND-STA-010",
            format!("node `{}` has an incomplete exact state profile", node.id),
        ));
    }
    Ok(())
}

fn validate_state_cell(node: &Node) -> Result<(), ResolutionError> {
    validate_exact_state_fields(
        node,
        &[
            "state_schema",
            "state_schema_version",
            "state_schema_hash",
            "initialization",
            "initial",
            "maximum_value_bytes",
            "emission",
            "reset",
            "terminal",
            "restart",
            "checkpoint",
        ],
    )?;
    validate_state_descriptor(
        node,
        "state_schema",
        "state_schema_version",
        "state_schema_hash",
        STATE_TEXT_SCHEMA_DESCRIPTOR,
        STATE_TEXT_SCHEMA_HASH,
    )?;
    let maximum = required_state_bound(
        node,
        "maximum_value_bytes",
        conduit_std::STATE_MAX_VALUE_BYTES,
    )?;
    if u64::try_from(node.config("initial").unwrap_or_default().len()).unwrap_or(u64::MAX) > maximum
        || !matches!(node.config("initialization"), Some("empty" | "value"))
        || !matches!(
            node.config("emission"),
            Some("on-update" | "initial-and-update")
        )
        || node.config("reset") != Some("initial")
        || node.config("terminal") != Some("complete")
        || node.config("restart") != Some("initial")
        || node.config("checkpoint") != Some("unsupported")
    {
        return Err(ResolutionError::new(
            "CND-STA-012",
            format!("node `{}` has unsupported cell semantics", node.id),
        ));
    }
    Ok(())
}

fn validate_state_deduplicate(node: &Node) -> Result<(), ResolutionError> {
    validate_exact_state_fields(
        node,
        &[
            "equality",
            "equality_schema_version",
            "equality_hash",
            "maximum_entries",
            "maximum_bytes",
            "eviction",
            "duplicate",
            "reset",
            "terminal",
            "restart",
            "checkpoint",
        ],
    )?;
    validate_state_descriptor(
        node,
        "equality",
        "equality_schema_version",
        "equality_hash",
        STATE_EQUALITY_DESCRIPTOR,
        STATE_EQUALITY_HASH,
    )?;
    required_state_bound(
        node,
        "maximum_entries",
        conduit_std::STATE_MAX_ENTRIES as u64,
    )?;
    required_state_bound(node, "maximum_bytes", conduit_std::STATE_MAX_VALUE_BYTES)?;
    if node.config("eviction") != Some("fifo")
        || node.config("duplicate") != Some("drop")
        || node.config("reset") != Some("clear")
        || node.config("terminal") != Some("complete")
        || node.config("restart") != Some("empty")
        || node.config("checkpoint") != Some("unsupported")
    {
        return Err(ResolutionError::new(
            "CND-STA-012",
            format!("node `{}` has unsupported deduplicate semantics", node.id),
        ));
    }
    Ok(())
}

fn validate_state_cache(node: &Node) -> Result<(), ResolutionError> {
    validate_exact_state_fields(
        node,
        &[
            "request_schema",
            "request_schema_version",
            "request_schema_hash",
            "key_equality",
            "key_equality_schema_version",
            "key_equality_hash",
            "maximum_entries",
            "maximum_key_bytes",
            "maximum_value_bytes",
            "maximum_total_bytes",
            "eviction",
            "ttl",
            "sensitivity",
            "restart",
            "checkpoint",
        ],
    )?;
    validate_state_descriptor(
        node,
        "request_schema",
        "request_schema_version",
        "request_schema_hash",
        STATE_CACHE_REQUEST_DESCRIPTOR,
        STATE_CACHE_REQUEST_HASH,
    )?;
    validate_state_descriptor(
        node,
        "key_equality",
        "key_equality_schema_version",
        "key_equality_hash",
        STATE_EQUALITY_DESCRIPTOR,
        STATE_EQUALITY_HASH,
    )?;
    required_state_bound(
        node,
        "maximum_entries",
        conduit_std::STATE_MAX_ENTRIES as u64,
    )?;
    let maximum_key = required_state_bound(node, "maximum_key_bytes", 4096)?;
    let maximum_value = required_state_bound(
        node,
        "maximum_value_bytes",
        conduit_std::STATE_MAX_VALUE_BYTES,
    )?;
    let maximum_total = required_state_bound(
        node,
        "maximum_total_bytes",
        conduit_std::STATE_MAX_VALUE_BYTES,
    )?;
    if maximum_key > maximum_total
        || maximum_value > maximum_total
        || node.config("eviction") != Some("fifo")
        || node.config("ttl") != Some("none")
        || node.config("sensitivity") != Some("preserve")
        || node.config("restart") != Some("empty")
        || node.config("checkpoint") != Some("unsupported")
    {
        return Err(ResolutionError::new(
            "CND-STA-012",
            format!("node `{}` has unsupported cache semantics", node.id),
        ));
    }
    Ok(())
}

fn required_data_reference<'a>(
    node: &'a Node,
    key: &str,
    expected: &str,
) -> Result<&'a str, ResolutionError> {
    let value = node.config(key).ok_or_else(|| {
        ResolutionError::new(
            "CND-DAT-010",
            format!("node `{}` requires exact `{key}` descriptor", node.id),
        )
    })?;
    if value != expected {
        return Err(ResolutionError::new(
            "CND-DAT-011",
            format!("node `{}` requests unsupported `{key}` `{value}`", node.id),
        ));
    }
    Ok(value)
}

fn required_data_bound(node: &Node, key: &str, maximum: u64) -> Result<u64, ResolutionError> {
    let Some(conduit_panel::SourceValue::Integer(value)) = node.config_value(key) else {
        return Err(ResolutionError::new(
            "CND-DAT-010",
            format!("node `{}` requires integer `{key}`", node.id),
        ));
    };
    let value = u64::try_from(*value).map_err(|_| {
        ResolutionError::new(
            "CND-DAT-012",
            format!("node `{}` has negative `{key}`", node.id),
        )
    })?;
    if value > maximum {
        return Err(ResolutionError::new(
            "CND-DAT-012",
            format!("node `{}` exceeds `{key}` provider bound", node.id),
        ));
    }
    Ok(value)
}

fn required_data_descriptor_pin(
    node: &Node,
    prefix: &str,
    expected_id: &str,
    expected_hash: &[u8; 32],
) -> Result<(), ResolutionError> {
    required_data_reference(node, prefix, expected_id)?;
    let version_key = if prefix == "schema" {
        "schema_version".to_owned()
    } else {
        format!("{prefix}_schema_version")
    };
    if required_data_bound(node, &version_key, u32::MAX as u64)? != 0 {
        return Err(ResolutionError::new(
            "CND-DAT-011",
            format!(
                "node `{}` requests unsupported `{prefix}` schema version",
                node.id
            ),
        ));
    }
    let hash_key = format!("{prefix}_hash");
    let Some(conduit_panel::SourceValue::Bytes(hash)) = node.config_value(&hash_key) else {
        return Err(ResolutionError::new(
            "CND-DAT-010",
            format!("node `{}` requires bytes `{hash_key}`", node.id),
        ));
    };
    if hash.as_slice() != expected_hash {
        return Err(ResolutionError::new(
            "CND-DAT-011",
            format!("node `{}` requests unsupported `{prefix}` hash", node.id),
        ));
    }
    Ok(())
}

fn validate_data_config_keys(node: &Node, expected: &[&str]) -> Result<(), ResolutionError> {
    if node.config.len() != expected.len() {
        return Err(ResolutionError::new(
            "CND-DAT-010",
            format!("node `{}` has an incomplete data-boundary profile", node.id),
        ));
    }
    if let Some(entry) = node
        .config
        .iter()
        .find(|entry| !expected.contains(&entry.key.as_str()))
    {
        return Err(ResolutionError::new(
            "CND-DAT-010",
            format!("node `{}` has unknown field `{}`", node.id, entry.key),
        ));
    }
    Ok(())
}

fn validate_data_codec(node: &Node) -> Result<(), ResolutionError> {
    validate_data_config_keys(
        node,
        &[
            "codec",
            "codec_schema_version",
            "codec_hash",
            "maximum_input_bytes",
            "maximum_output_bytes",
        ],
    )?;
    required_data_descriptor_pin(
        node,
        "codec",
        DATA_UTF8_CODEC_DESCRIPTOR,
        DATA_UTF8_CODEC_HASH,
    )?;
    let input = required_data_bound(
        node,
        "maximum_input_bytes",
        conduit_std::DATA_MAX_FRAME_BYTES as u64,
    )?;
    let output = required_data_bound(
        node,
        "maximum_output_bytes",
        conduit_std::DATA_MAX_FRAME_BYTES as u64,
    )?;
    if output < input {
        return Err(ResolutionError::new(
            "CND-DAT-012",
            format!(
                "node `{}` output bound is smaller than its input bound",
                node.id
            ),
        ));
    }
    Ok(())
}

fn validate_data_framing(node: &Node) -> Result<(), ResolutionError> {
    validate_data_config_keys(
        node,
        &[
            "framing",
            "framing_schema_version",
            "framing_hash",
            "maximum_frame_bytes",
            "maximum_partial_bytes",
            "maximum_output_bytes",
        ],
    )?;
    required_data_descriptor_pin(
        node,
        "framing",
        DATA_LENGTH_U32BE_DESCRIPTOR,
        DATA_LENGTH_U32BE_HASH,
    )?;
    let frame = required_data_bound(
        node,
        "maximum_frame_bytes",
        conduit_std::DATA_MAX_FRAME_BYTES as u64,
    )?;
    let storage_maximum =
        conduit_std::DATA_MAX_FRAME_BYTES as u64 + conduit_std::LENGTH_U32BE_PREFIX_BYTES as u64;
    let partial = required_data_bound(node, "maximum_partial_bytes", storage_maximum)?;
    let output = required_data_bound(node, "maximum_output_bytes", storage_maximum)?;
    let framed = frame + conduit_std::LENGTH_U32BE_PREFIX_BYTES as u64;
    if partial < framed
        || (node.kind == "std/data/frame-length-u32be" && output < framed)
        || (node.kind == "std/data/deframe-length-u32be" && output < frame)
    {
        return Err(ResolutionError::new(
            "CND-DAT-012",
            format!("node `{}` has inconsistent framing bounds", node.id),
        ));
    }
    Ok(())
}

fn validate_record_bounds(node: &Node) -> Result<(), ResolutionError> {
    let fields = required_data_bound(
        node,
        "maximum_fields",
        conduit_std::DATA_MAX_RECORD_FIELDS as u64,
    )?;
    let names = required_data_bound(
        node,
        "maximum_field_name_bytes",
        conduit_std::DATA_MAX_FIELD_NAME_BYTES as u64,
    )?;
    let values = required_data_bound(
        node,
        "maximum_field_value_bytes",
        conduit_std::DATA_MAX_FIELD_VALUE_BYTES as u64,
    )?;
    let work = required_data_bound(
        node,
        "maximum_work",
        conduit_std::DATA_MAX_RECORD_BYTES as u64,
    )?;
    let minimum_work = 2_u64
        .checked_add(fields.saturating_mul(6 + names + values))
        .ok_or_else(|| ResolutionError::new("CND-DAT-012", "record work bound overflowed"))?;
    if work < minimum_work {
        return Err(ResolutionError::new(
            "CND-DAT-012",
            format!("node `{}` has insufficient structural work", node.id),
        ));
    }
    Ok(())
}

fn validate_record_literal(node: &Node) -> Result<(), ResolutionError> {
    validate_data_config_keys(
        node,
        &[
            "fields",
            "maximum_fields",
            "maximum_field_name_bytes",
            "maximum_field_value_bytes",
            "maximum_work",
        ],
    )?;
    let Some(SourceValue::Record(fields)) = node.config_value("fields") else {
        return Err(ResolutionError::new(
            "CND-DAT-010",
            format!("node `{}` requires record `fields`", node.id),
        ));
    };
    if fields
        .iter()
        .any(|(_, value)| !matches!(value, SourceValue::Text(_)))
    {
        return Err(ResolutionError::new(
            "CND-DAT-013",
            format!("node `{}` record literal values must be text", node.id),
        ));
    }
    validate_record_bounds(node)
}

fn validate_closed_record_config(node: &Node) -> Result<(), ResolutionError> {
    validate_data_config_keys(
        node,
        &[
            "schema",
            "schema_version",
            "schema_hash",
            "maximum_fields",
            "maximum_field_name_bytes",
            "maximum_field_value_bytes",
            "maximum_work",
        ],
    )?;
    required_data_descriptor_pin(
        node,
        "schema",
        DATA_CLOSED_RECORD_SCHEMA_DESCRIPTOR,
        DATA_CLOSED_RECORD_SCHEMA_HASH,
    )?;
    validate_record_bounds(node)
}

fn validate_validation_decision_assert(node: &Node) -> Result<(), ResolutionError> {
    validate_data_config_keys(node, &["expected"])?;
    let expected = node.config("expected").ok_or_else(|| {
        ResolutionError::new(
            "CND-DAT-010",
            format!("node `{}` requires text `expected`", node.id),
        )
    })?;
    if !matches!(
        expected,
        "accepted" | "rejected-missing-field" | "rejected-unknown-field"
    ) {
        return Err(ResolutionError::new(
            "CND-DAT-011",
            format!(
                "node `{}` requests unsupported decision `{expected}`",
                node.id
            ),
        ));
    }
    Ok(())
}

fn required_supervision_bound(
    node: &Node,
    key: &str,
    maximum: u64,
    allow_zero: bool,
) -> Result<u64, ResolutionError> {
    let Some(SourceValue::Integer(value)) = node.config_value(key) else {
        return Err(ResolutionError::new(
            "CND-SVP-010",
            format!("node `{}` requires integer `{key}`", node.id),
        ));
    };
    let value = u64::try_from(*value).map_err(|_| {
        ResolutionError::new(
            "CND-SVP-012",
            format!("node `{}` has negative `{key}`", node.id),
        )
    })?;
    if (!allow_zero && value == 0) || value > maximum {
        return Err(ResolutionError::new(
            "CND-SVP-012",
            format!("node `{}` exceeds `{key}` provider bound", node.id),
        ));
    }
    Ok(value)
}

fn validate_supervision_descriptor(
    node: &Node,
    descriptor_key: &str,
    version_key: &str,
    hash_key: &str,
    expected_descriptor: &str,
    expected_hash: &[u8; 32],
) -> Result<(), ResolutionError> {
    if node.config(descriptor_key) != Some(expected_descriptor)
        || !matches!(
            node.config_value(version_key),
            Some(SourceValue::Integer(0))
        )
    {
        return Err(ResolutionError::new(
            "CND-SVP-011",
            format!("node `{}` requests an unsupported descriptor", node.id),
        ));
    }
    let Some(SourceValue::Bytes(hash)) = node.config_value(hash_key) else {
        return Err(ResolutionError::new(
            "CND-SVP-010",
            format!("node `{}` requires bytes `{hash_key}`", node.id),
        ));
    };
    if hash.as_slice() != expected_hash {
        return Err(ResolutionError::new(
            "CND-SVP-011",
            format!("node `{}` requests a stale descriptor hash", node.id),
        ));
    }
    Ok(())
}

fn validate_exact_supervision_fields(
    node: &Node,
    expected: &[&str],
) -> Result<(), ResolutionError> {
    if node.config.len() != expected.len()
        || node
            .config
            .iter()
            .any(|entry| !expected.contains(&entry.key.as_str()))
    {
        return Err(ResolutionError::new(
            "CND-SVP-010",
            format!(
                "node `{}` has an incomplete exact supervision profile",
                node.id
            ),
        ));
    }
    Ok(())
}

fn validate_supervision_clock(node: &Node) -> Result<(), ResolutionError> {
    validate_supervision_descriptor(
        node,
        "clock",
        "clock_schema_version",
        "clock_hash",
        TIME_CLOCK_DESCRIPTOR,
        TIME_CLOCK_HASH,
    )
}

fn validate_supervision_retry(node: &Node) -> Result<(), ResolutionError> {
    validate_exact_supervision_fields(
        node,
        &[
            "terminal_schema",
            "terminal_schema_version",
            "terminal_schema_hash",
            "clock",
            "clock_schema_version",
            "clock_hash",
            "maximum_attempts",
            "deadline_ticks",
            "idempotency",
            "committed_replay",
            "backoff",
            "initial_backoff_ticks",
            "maximum_backoff_ticks",
            "jitter",
            "jitter_ticks",
            "entropy",
            "entropy_schema_version",
            "entropy_hash",
            "maximum_pending",
            "cancellation",
            "exhaustion",
            "restart",
            "checkpoint",
        ],
    )?;
    validate_supervision_descriptor(
        node,
        "terminal_schema",
        "terminal_schema_version",
        "terminal_schema_hash",
        SUPERVISION_TERMINAL_DESCRIPTOR,
        SUPERVISION_TERMINAL_HASH,
    )?;
    validate_supervision_clock(node)?;
    validate_supervision_descriptor(
        node,
        "entropy",
        "entropy_schema_version",
        "entropy_hash",
        SUPERVISION_ENTROPY_DESCRIPTOR,
        SUPERVISION_ENTROPY_HASH,
    )?;
    let attempts = required_supervision_bound(
        node,
        "maximum_attempts",
        u64::from(conduit_std::SUPERVISION_MAX_ATTEMPTS),
        false,
    )?;
    let deadline = required_supervision_bound(
        node,
        "deadline_ticks",
        conduit_std::SUPERVISION_MAX_DURATION_TICKS,
        false,
    )?;
    let initial = required_supervision_bound(
        node,
        "initial_backoff_ticks",
        conduit_std::SUPERVISION_MAX_DURATION_TICKS,
        false,
    )?;
    let maximum = required_supervision_bound(
        node,
        "maximum_backoff_ticks",
        conduit_std::SUPERVISION_MAX_DURATION_TICKS,
        false,
    )?;
    let jitter = required_supervision_bound(
        node,
        "jitter_ticks",
        conduit_std::SUPERVISION_MAX_DURATION_TICKS,
        true,
    )?;
    let idempotency = node.config("idempotency");
    if attempts == 0
        || deadline <= initial
        || maximum < initial
        || maximum >= deadline
        || jitter > maximum
        || !matches!(
            idempotency,
            Some("forbidden" | "idempotent" | "reconcile-before-retry")
        )
        || !matches!(node.config("committed_replay"), Some("forbid" | "permit"))
        || (node.config("committed_replay") == Some("permit") && idempotency != Some("idempotent"))
        || !matches!(node.config("backoff"), Some("fixed" | "exponential"))
        || !matches!(node.config("jitter"), Some("none" | "injected"))
        || (node.config("jitter") == Some("none") && jitter != 0)
        || (node.config("jitter") == Some("injected") && jitter == 0)
        || required_supervision_bound(node, "maximum_pending", 1, false)? != 1
        || node.config("cancellation") != Some("discard")
        || node.config("exhaustion") != Some("terminal")
        || node.config("restart") != Some("new-generation")
        || node.config("checkpoint") != Some("unsupported")
    {
        return Err(ResolutionError::new(
            "CND-SVP-012",
            format!("node `{}` has unsupported retry semantics", node.id),
        ));
    }
    Ok(())
}

fn validate_supervision_circuit_breaker(node: &Node) -> Result<(), ResolutionError> {
    validate_exact_supervision_fields(
        node,
        &[
            "terminal_schema",
            "terminal_schema_version",
            "terminal_schema_hash",
            "clock",
            "clock_schema_version",
            "clock_hash",
            "counted_outcomes",
            "maximum_observations",
            "failure_threshold",
            "cooldown_ticks",
            "maximum_half_open_probes",
            "maximum_pending",
            "reset",
            "terminal",
            "restart",
            "checkpoint",
        ],
    )?;
    validate_supervision_descriptor(
        node,
        "terminal_schema",
        "terminal_schema_version",
        "terminal_schema_hash",
        SUPERVISION_TERMINAL_DESCRIPTOR,
        SUPERVISION_TERMINAL_HASH,
    )?;
    validate_supervision_clock(node)?;
    let observations = required_supervision_bound(
        node,
        "maximum_observations",
        conduit_std::SUPERVISION_MAX_OBSERVATIONS as u64,
        false,
    )?;
    let threshold = required_supervision_bound(
        node,
        "failure_threshold",
        conduit_std::SUPERVISION_MAX_OBSERVATIONS as u64,
        false,
    )?;
    required_supervision_bound(
        node,
        "cooldown_ticks",
        conduit_std::SUPERVISION_MAX_DURATION_TICKS,
        false,
    )?;
    required_supervision_bound(node, "maximum_half_open_probes", u64::from(u16::MAX), false)?;
    if threshold > observations
        || node.config("counted_outcomes") != Some("failed")
        || required_supervision_bound(node, "maximum_pending", 1, false)? != 1
        || node.config("reset") != Some("explicit")
        || node.config("terminal") != Some("complete")
        || node.config("restart") != Some("closed")
        || node.config("checkpoint") != Some("unsupported")
    {
        return Err(ResolutionError::new(
            "CND-SVP-012",
            format!(
                "node `{}` has unsupported circuit-breaker semantics",
                node.id
            ),
        ));
    }
    Ok(())
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

fn validate_file_chunk_literal(node: &Node) -> Result<(), ResolutionError> {
    if node.config.len() != 2 || !matches!(node.config_value("value"), Some(SourceValue::Bytes(_)))
    {
        return Err(ResolutionError::new(
            "CND-FS-001",
            format!(
                "file chunk literal `{}` requires exact `value` and `maximum_bytes` fields",
                node.id
            ),
        ));
    }
    let maximum = match node.config_value("maximum_bytes") {
        Some(SourceValue::Integer(value)) => usize::try_from(*value).ok(),
        _ => None,
    };
    let value_length = match node.config_value("value") {
        Some(SourceValue::Bytes(value)) => value.len(),
        _ => 0,
    };
    if maximum.is_none_or(|maximum| {
        maximum == 0 || maximum > conduit_std::FILESYSTEM_MAX_FILE_BYTES || value_length > maximum
    }) {
        return Err(ResolutionError::new(
            "CND-FS-006",
            format!("file chunk literal `{}` exceeds its exact bound", node.id),
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

struct FileChunkLiteral;

impl Handler for FileChunkLiteral {
    fn run(
        &mut self,
        node: &Node,
        _inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let Some(SourceValue::Bytes(bytes)) = node.config_value("value") else {
            return Err(RuntimeError::new(
                "CND-FS-001",
                "file chunk literal value disappeared",
            ));
        };
        Ok(vec![Value {
            value_type: file_chunk_literal_contract().outputs[0].value_type,
            bytes: bytes.clone(),
        }])
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

#[derive(Default)]
struct Ticker {
    duration_ticks: u64,
    next_tick: u64,
    deadline_tick: Option<u64>,
}

impl Handler for Ticker {
    fn prepare(
        &mut self,
        node: &Node,
        binding: ExactHostedServiceBinding,
    ) -> Result<(), RuntimeError> {
        self.bind_exact(binding)?;
        let Some(SourceValue::Integer(duration_ticks)) = node.config_value("duration_ticks") else {
            return Err(RuntimeError::new(
                "CND-TIM-010",
                "ticker duration disappeared after exact validation",
            ));
        };
        self.duration_ticks = u64::try_from(*duration_ticks).map_err(|_| {
            RuntimeError::new(
                "CND-TIM-012",
                "ticker duration is not representable by the hosted clock",
            )
        })?;
        if self.duration_ticks == 0 {
            return Err(RuntimeError::new(
                "CND-TIM-012",
                "ticker interval must be nonzero",
            ));
        }
        Ok(())
    }

    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        if !inputs.is_empty() {
            return Err(RuntimeError::new(
                "CND-RUN-004",
                "ticker does not accept input values",
            ));
        }
        if let Some(deadline_tick) = self.deadline_tick {
            if context.tick < deadline_tick {
                return Ok(HostedServiceStep::waiting(HostedServiceInterest::Timer {
                    subject: Id("conduit/time-ticker"),
                    deadline_tick,
                }));
            }
            self.deadline_tick = None;
        }
        let tick = self.next_tick;
        self.next_tick = self.next_tick.checked_add(1).ok_or_else(|| {
            RuntimeError::new(
                "CND-TIM-020",
                "ticker count exhausted its u64 representation",
            )
        })?;
        let deadline_tick = context
            .tick
            .checked_add(self.duration_ticks)
            .ok_or_else(|| RuntimeError::new("CND-TIM-020", "ticker deadline overflowed"))?;
        self.deadline_tick = Some(deadline_tick);
        let value_type = conduit_std::standard_node_contract("time/ticker")
            .expect("ticker is in the standard catalog")
            .outputs[0]
            .value_type;
        Ok(HostedServiceStep::produced(vec![Value {
            value_type,
            bytes: format!("{tick}\n").into_bytes(),
        }]))
    }
}

struct TimeCompatibilityHandler;

impl Handler for TimeCompatibilityHandler {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let value = inputs
            .first()
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "time input is missing"))?;
        Ok(vec![value.clone()])
    }
}

struct StateCompatibilityHandler;

impl Handler for StateCompatibilityHandler {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        Ok(inputs.first().cloned().into_iter().collect())
    }
}

fn encode_record_literal(node: &Node) -> Result<Vec<u8>, RuntimeError> {
    let Some(SourceValue::Record(fields)) = node.config_value("fields") else {
        return Err(RuntimeError::new(
            "CND-DAT-010",
            "record literal disappeared",
        ));
    };
    let fields = fields
        .iter()
        .map(|(name, value)| {
            let SourceValue::Text(value) = value else {
                return Err(RuntimeError::new(
                    "CND-DAT-013",
                    "record literal value is not text",
                ));
            };
            Ok(conduit_std::StructuralField {
                name,
                value: value.as_bytes(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let maximum_work =
        runtime_data_bound(node, "maximum_work", conduit_std::DATA_MAX_RECORD_BYTES)?;
    let mut output = vec![0; maximum_work];
    let length = conduit_std::encode_closed_record(
        &fields,
        &mut output,
        runtime_data_bound(node, "maximum_fields", conduit_std::DATA_MAX_RECORD_FIELDS)?,
        runtime_data_bound(
            node,
            "maximum_field_name_bytes",
            conduit_std::DATA_MAX_FIELD_NAME_BYTES,
        )?,
        runtime_data_bound(
            node,
            "maximum_field_value_bytes",
            conduit_std::DATA_MAX_FIELD_VALUE_BYTES,
        )?,
        maximum_work,
    )
    .map_err(data_boundary_runtime_error)?;
    output.truncate(length);
    Ok(output)
}

struct RecordLiteral;

impl Handler for RecordLiteral {
    fn run(
        &mut self,
        node: &Node,
        _inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        Ok(vec![Value::record(encode_record_literal(node)?)])
    }
}

fn encode_structural_decision(decision: conduit_std::StructuralDecision) -> Vec<u8> {
    match decision {
        conduit_std::StructuralDecision::Accepted => vec![0],
        conduit_std::StructuralDecision::Rejected {
            field_index,
            reason,
        } => {
            let reason = match reason {
                conduit_std::StructuralRejection::MissingRequiredField => 1,
                conduit_std::StructuralRejection::UnknownField => 2,
            };
            let mut bytes = vec![1, reason, u8::from(field_index.is_some())];
            if let Some(index) = field_index {
                bytes.extend_from_slice(&(index as u16).to_be_bytes());
            }
            bytes
        }
    }
}

struct ValidateClosedRecord;

impl Handler for ValidateClosedRecord {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let input = inputs
            .first()
            .filter(|value| value.value_type == RECORD_TYPE)
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "record candidate is missing"))?;
        let decision = conduit_std::validate_closed_record_bytes(
            &input.bytes,
            CLOSED_RECORD_REQUIRED_FIELDS,
            runtime_data_bound(node, "maximum_fields", conduit_std::DATA_MAX_RECORD_FIELDS)?,
            runtime_data_bound(
                node,
                "maximum_field_name_bytes",
                conduit_std::DATA_MAX_FIELD_NAME_BYTES,
            )?,
            runtime_data_bound(
                node,
                "maximum_field_value_bytes",
                conduit_std::DATA_MAX_FIELD_VALUE_BYTES,
            )?,
            runtime_data_bound(node, "maximum_work", conduit_std::DATA_MAX_RECORD_BYTES)?,
        )
        .map_err(data_boundary_runtime_error)?;
        Ok(vec![
            Value::record(input.bytes.clone()),
            Value::validation_decision(encode_structural_decision(decision)),
        ])
    }
}

fn validation_decision_name(bytes: &[u8]) -> Result<&'static str, RuntimeError> {
    match bytes {
        [0] => Ok("accepted"),
        [1, 1, 0] => Ok("rejected-missing-field"),
        [1, 2, 1, _, _] => Ok("rejected-unknown-field"),
        _ => Err(RuntimeError::new(
            "CND-DAT-013",
            "validation decision representation is malformed",
        )),
    }
}

struct ValidationDecisionAssert;

impl Handler for ValidationDecisionAssert {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let input = inputs
            .first()
            .filter(|value| value.value_type == VALIDATION_DECISION_TYPE)
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "validation decision is missing"))?;
        let actual = validation_decision_name(&input.bytes)?;
        let expected = node
            .config("expected")
            .ok_or_else(|| RuntimeError::new("CND-DAT-010", "expected decision disappeared"))?;
        if actual != expected {
            return Err(RuntimeError::new(
                "CND-DAT-015",
                format!("expected `{expected}` but received `{actual}`"),
            ));
        }
        Ok(Vec::new())
    }
}

impl Handler for EncodeUtf8 {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let input = inputs
            .first()
            .filter(|value| value.value_type == TEXT_TYPE)
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "UTF-8 encoder text input missing"))?;
        let maximum = runtime_data_bound(
            node,
            "maximum_output_bytes",
            conduit_std::DATA_MAX_FRAME_BYTES,
        )?;
        let text = std::str::from_utf8(&input.bytes)
            .map_err(|_| RuntimeError::new("CND-DAT-005", "input is not exact UTF-8 text"))?;
        let mut output = vec![0; maximum];
        let length =
            conduit_std::encode_utf8(text, &mut output).map_err(data_boundary_runtime_error)?;
        output.truncate(length);
        Ok(vec![Value::bytes(output)])
    }
}

struct DecodeUtf8;

impl Handler for DecodeUtf8 {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let input = inputs
            .first()
            .filter(|value| value.value_type == BYTES_TYPE)
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "UTF-8 decoder byte input missing"))?;
        let maximum = runtime_data_bound(
            node,
            "maximum_output_bytes",
            conduit_std::DATA_MAX_FRAME_BYTES,
        )?;
        let mut output = vec![0; maximum];
        let length = conduit_std::decode_utf8(&input.bytes, &mut output)
            .map_err(data_boundary_runtime_error)?;
        output.truncate(length);
        Ok(vec![Value::text(output)])
    }
}

fn runtime_data_bound(node: &Node, key: &str, _maximum: usize) -> Result<usize, RuntimeError> {
    match node.config_value(key) {
        Some(conduit_panel::SourceValue::Integer(value)) => usize::try_from(*value)
            .map_err(|_| RuntimeError::new("CND-DAT-012", "data bound is out of range")),
        _ => Err(RuntimeError::new(
            "CND-DAT-010",
            format!("node `{}` lost exact `{key}`", node.id),
        )),
    }
}

fn data_boundary_runtime_error(error: conduit_std::DataBoundaryError) -> RuntimeError {
    RuntimeError::new(error.code(), error.code())
}

fn time_runtime_error(error: conduit_std::TimeError) -> RuntimeError {
    RuntimeError::new(error.code(), error.code())
}

fn state_runtime_error(error: conduit_std::StateError) -> RuntimeError {
    RuntimeError::new(error.code(), error.code())
}

fn supervision_runtime_error(error: conduit_std::SupervisionError) -> RuntimeError {
    RuntimeError::new(error.code(), error.code())
}

fn recorded_time_progress(io: &mut StepIo<'_>) -> SchedulerStep {
    if io.record_host_progress().is_ok() {
        SchedulerStep::Progress
    } else {
        SchedulerStep::Failed {
            code: Id("conduit/step-work-bound-exceeded"),
        }
    }
}

struct FrameLengthU32Be;

impl Handler for FrameLengthU32Be {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let input = inputs
            .first()
            .filter(|value| value.value_type == BYTES_TYPE)
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "frame payload is missing"))?;
        let maximum_frame = runtime_data_bound(
            node,
            "maximum_frame_bytes",
            conduit_std::DATA_MAX_FRAME_BYTES,
        )?;
        let maximum_output = runtime_data_bound(
            node,
            "maximum_output_bytes",
            conduit_std::DATA_MAX_FRAME_BYTES + conduit_std::LENGTH_U32BE_PREFIX_BYTES,
        )?;
        let mut output = vec![0; maximum_output];
        let length = conduit_std::encode_length_u32be(&input.bytes, &mut output, maximum_frame)
            .map_err(data_boundary_runtime_error)?;
        output.truncate(length);
        Ok(vec![Value::bytes(output)])
    }
}

struct DeframeLengthU32Be;

impl Handler for DeframeLengthU32Be {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let maximum_frame = runtime_data_bound(
            node,
            "maximum_frame_bytes",
            conduit_std::DATA_MAX_FRAME_BYTES,
        )?;
        let mut decoder =
            conduit_std::LengthU32BeDecoder::<{ conduit_std::DATA_MAX_FRAME_BYTES }>::new(
                maximum_frame,
            );
        let mut output = vec![0; maximum_frame];
        let mut ready_length = None;
        for input in inputs {
            if input.value_type != BYTES_TYPE {
                return Err(RuntimeError::new(
                    "CND-RUN-004",
                    "deframe chunk is not exact bytes",
                ));
            }
            for &byte in &input.bytes {
                if decoder
                    .push_byte(byte)
                    .map_err(data_boundary_runtime_error)?
                {
                    if ready_length.is_some() {
                        return Err(RuntimeError::new(
                            "CND-DAT-013",
                            "finite compatibility execution produced more than one frame",
                        ));
                    }
                    ready_length = decoder
                        .take_ready(&mut output)
                        .map_err(data_boundary_runtime_error)?;
                }
            }
        }
        decoder.finish().map_err(data_boundary_runtime_error)?;
        let length = ready_length.ok_or_else(|| {
            RuntimeError::new("CND-DAT-003", "input terminated without a complete frame")
        })?;
        output.truncate(length);
        Ok(vec![Value::bytes(output)])
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

struct DisplayText;

impl Handler for DisplayText {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let input = inputs
            .first()
            .filter(|value| value.value_type == TEXT_TYPE)
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "display text input missing"))?;
        std::str::from_utf8(&input.bytes)
            .map_err(|error| RuntimeError::new("CND-RUN-005", error.to_string()))?;
        io.display
            .write_all(&input.bytes)
            .map_err(|error| RuntimeError::new("CND-RUN-005", error.to_string()))?;
        Ok(Vec::new())
    }
}

struct DiscardHandler;

impl Handler for DiscardHandler {
    fn run(
        &mut self,
        _node: &Node,
        _inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
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
            "typed supervisors run through the bounded supervision scheduler, not the displaced one-shot executor",
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
    fn host_value_store_reclaims_bytes_and_rejects_a_stale_generation() {
        let mut store = HostValueStore::with_limits(8, 1).unwrap();
        let first = store.store(b"value").unwrap();
        assert_eq!(store.get(first), Some(&b"value"[..]));
        assert_eq!(store.usage().resident_bytes, 5);

        store.begin_reconciliation();
        store.finish_reconciliation();
        assert_eq!(store.usage().resident_slots, 0);
        assert_eq!(store.usage().resident_bytes, 0);
        assert_eq!(store.get(first), None);

        let second = store.store(b"next").unwrap();
        assert_ne!(first, second);
        assert_eq!(store.get(first), None);
        assert_eq!(store.get(second), Some(&b"next"[..]));
        assert_eq!(store.usage().high_water_slots, 1);
        assert_eq!(store.usage().high_water_bytes, 5);
    }

    #[test]
    fn host_value_store_reuses_one_fixed_slot_for_a_million_released_values() {
        let mut store = HostValueStore::with_limits(1, 1).unwrap();
        let mut last = 0;
        for _ in 0..1_000_000 {
            last = store.store(b"x").expect("released slot is reusable");
            store.begin_reconciliation();
            store.finish_reconciliation();
            assert_eq!(store.usage().resident_slots, 0);
            assert_eq!(store.usage().resident_bytes, 0);
        }
        assert_eq!(store.get(last), None, "released generation stays stale");
        assert_eq!(store.usage().high_water_slots, 1);
        assert_eq!(store.usage().high_water_bytes, 1);
        assert_eq!(store.usage().maximum_slots, 1);
        assert_eq!(store.usage().maximum_bytes, 1);
    }

    #[test]
    fn million_value_pass_through_and_shared_tee_stay_inside_one_exact_slot() {
        let mut store = HostValueStore::with_limits(1, 1).unwrap();
        for _ in 0..1_000_000 {
            let handle = store.store(b"x").expect("the released slot is reusable");
            let value = RuntimeValue {
                handle,
                accounted_bytes: 1,
                envelope: RuntimeValueEnvelope::EMPTY,
            };

            // A pass-through keeps the accepted handle. A shared-handle tee
            // may expose that same handle to both branches, so duplicate marks
            // must remain one resident value rather than hidden payload copies.
            store.begin_reconciliation();
            store.mark_live(value);
            store.mark_live(value);
            store.finish_reconciliation();
            assert_eq!(store.usage().resident_slots, 1);
            assert_eq!(store.usage().resident_bytes, 1);

            // Once queues and node retention drain, the one value is released
            // exactly once and the next turn returns to the same baseline.
            store.begin_reconciliation();
            store.finish_reconciliation();
            store.finish_reconciliation();
            assert_eq!(store.get(handle), None);
            assert_eq!(store.usage().resident_slots, 0);
            assert_eq!(store.usage().resident_bytes, 0);
        }
        assert_eq!(store.usage().high_water_slots, 1);
        assert_eq!(store.usage().high_water_bytes, 1);
        assert_eq!(store.usage().maximum_slots, 1);
        assert_eq!(store.usage().maximum_bytes, 1);
    }

    #[test]
    fn value_reconciliation_covers_retention_replacement_drop_and_terminal_cleanup() {
        let mut store = HostValueStore::with_limits(8, 8).unwrap();
        let coupled = store.store(b"c").unwrap();
        let isolated = store.store(b"i").unwrap();
        let coalesced_old = store.store(b"o").unwrap();
        let coalesced_latest = store.store(b"n").unwrap();
        let joined = store.store(b"j").unwrap();
        let node_state = store.store(b"s").unwrap();
        let disposable = store.store(b"d").unwrap();
        let rejected = store.store(b"r").unwrap();
        let value = |handle| RuntimeValue {
            handle,
            accounted_bytes: 1,
            envelope: RuntimeValueEnvelope::EMPTY,
        };

        store.begin_reconciliation();
        store.mark_live(value(coupled));
        store.mark_live(value(coupled));
        store.mark_live(value(isolated));
        store.mark_live(value(coalesced_latest));
        store.mark_live(value(joined));
        store.mark_live(value(node_state));
        store.finish_reconciliation();

        for retained in [coupled, isolated, coalesced_latest, joined, node_state] {
            assert!(store.get(retained).is_some());
        }
        for released in [coalesced_old, disposable, rejected] {
            assert_eq!(store.get(released), None);
        }
        assert_eq!(store.usage().resident_slots, 5);
        assert_eq!(store.usage().resident_bytes, 5);

        // Abort, failed staged publication, node completion, and session
        // disposal all converge on the same no-owner reconciliation. Repeating
        // cleanup cannot release a later generation or underflow accounting.
        store.begin_reconciliation();
        store.finish_reconciliation();
        store.finish_reconciliation();
        assert_eq!(store.usage().resident_slots, 0);
        assert_eq!(store.usage().resident_bytes, 0);
        for stale in [coupled, isolated, coalesced_latest, joined, node_state] {
            assert_eq!(store.get(stale), None);
        }
    }

    #[test]
    fn value_store_zero_and_exhausted_bounds_fail_deterministically() {
        let mut absent = HostValueStore::with_limits(0, 0).unwrap();
        assert_eq!(absent.store(b""), None);
        assert_eq!(absent.store(b"x"), None);

        let mut bounded = HostValueStore::with_limits(1, 1).unwrap();
        let resident = bounded.store(b"x").unwrap();
        assert_eq!(bounded.store(b"y"), None, "the sole slot is occupied");
        assert_eq!(bounded.store(b"xx"), None, "the byte bound is exact");
        assert_eq!(bounded.get(resident), Some(&b"x"[..]));
        assert_eq!(bounded.usage().resident_slots, 1);
        assert_eq!(bounded.usage().resident_bytes, 1);
        assert_eq!(bounded.usage().high_water_slots, 1);
        assert_eq!(bounded.usage().high_water_bytes, 1);
    }

    #[test]
    fn host_value_store_retires_a_slot_before_generation_wraparound() {
        let mut store = HostValueStore::with_limits(1, 1).unwrap();
        store.slots[0].generation = u32::MAX;
        let last_generation = store.store(b"x").unwrap();

        store.begin_reconciliation();
        store.finish_reconciliation();

        assert_eq!(store.get(last_generation), None);
        assert!(store.slots[0].retired);
        assert!(store.store(b"x").is_none());
    }

    #[test]
    fn validation_outputs_remain_exact_under_asymmetric_pressure() {
        let value = RuntimeValue {
            handle: 7,
            accounted_bytes: 8,
            envelope: RuntimeValueEnvelope::EMPTY,
        };
        for publish_candidate_first in [true, false] {
            let mut candidate = Some(value);
            let mut decision = Some(value);
            let (first, blocked) = if publish_candidate_first {
                (&mut candidate, &mut decision)
            } else {
                (&mut decision, &mut candidate)
            };

            assert_eq!(
                stage_validation_output(Ok(SendStatus::Reserved), first),
                Ok(ValidationSendOutcome::Published)
            );
            assert!(first.is_none());
            assert_eq!(
                stage_validation_output(Ok(SendStatus::WouldBlock), blocked),
                Ok(ValidationSendOutcome::Blocked)
            );
            assert_eq!(*blocked, Some(value));
            assert_eq!(
                stage_validation_output(Ok(SendStatus::Reserved), blocked),
                Ok(ValidationSendOutcome::Published)
            );
            assert!(candidate.is_none());
            assert!(decision.is_none());
        }
    }

    #[test]
    fn format_uses_typed_inputs_named_indexed_and_escaped_placeholders() {
        let panel = parse(
            r#"
                panel 0
                template: std/literal {
                    value = "{worker} = {{status: {1}; count={2}}}"
                }
                values: std/format-values/literal {
                    values = list(
                        record(name="worker", value="alpha"),
                        record(name="ready", value=true),
                        record(name="count", value=-7)
                    )
                }
                message: std/text/format
                encoded: std/data/encode-utf8 { codec = ref("conduit.codec/utf-8") codec_schema_version = 0 codec_hash = bytes("f219297cb276bc91eccddb346a8b21e7edd4414b8844014108513747ae11bf53") maximum_input_bytes = 4096 maximum_output_bytes = 4096 }
                output: io/stdout
                template.value > message.template
                values.values > message.values
                message.text > encoded.text
                encoded.bytes > output.bytes
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
                display: &mut Vec::new(),
            })
            .unwrap();
        assert_eq!(output, b"alpha = {status: true; count=-7}");
    }

    #[test]
    fn format_rejects_missing_and_extra_values_at_execution() {
        let panel = parse(
            r#"
                panel 0
                template: std/literal {
                    value = "{} {}"
                }
                values: std/format-values/literal {
                    values = list("only-one")
                }
                message: std/text/format
                encoded: std/data/encode-utf8 { codec = ref("conduit.codec/utf-8") codec_schema_version = 0 codec_hash = bytes("f219297cb276bc91eccddb346a8b21e7edd4414b8844014108513747ae11bf53") maximum_input_bytes = 4096 maximum_output_bytes = 4096 }
                output: io/stdout
                template.value > message.template
                values.values > message.values
                message.text > encoded.text
                encoded.bytes > output.bytes
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
                display: &mut Vec::new(),
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
                panel 0
                greeting: std/literal {
                    value = "Hello from Conduit.\n"
                }
                shout: text/uppercase
                encoded: std/data/encode-utf8 { codec = ref("conduit.codec/utf-8") codec_schema_version = 0 codec_hash = bytes("f219297cb276bc91eccddb346a8b21e7edd4414b8844014108513747ae11bf53") maximum_input_bytes = 4096 maximum_output_bytes = 4096 }
                output: io/stdout
                greeting.value > shout.text
                shout.text > encoded.text
                encoded.bytes > output.bytes
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
                display: &mut Vec::new(),
            })
            .expect("panel runs");

        assert_eq!(output, b"HELLO FROM CONDUIT.\n");
        assert!(error.is_empty());
        assert_eq!(summary.nodes_completed, 4);
        assert_eq!(summary.cords_conducted, 3);
    }

    #[test]
    fn data_boundary_profile_round_trips_and_rejects_unpinned_codecs() {
        let panel = parse(include_str!(
            "../../../examples/data-boundary-round-trip.panel"
        ))
        .expect("data-boundary example parses");
        let registry = Registry::hosted_primitives();
        let resolved = registry.resolve(&panel).expect("exact profiles resolve");
        let mut input = &b""[..];
        let mut output = Vec::new();
        let mut error = Vec::new();
        let mut display = Vec::new();
        let summary = resolved
            .run_batch(&mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
                display: &mut display,
            })
            .expect("data boundaries run");
        assert_eq!(display, b"representation boundaries stay explicit");
        assert_eq!(summary.nodes_completed, 6);
        assert_eq!(summary.cords_conducted, 5);

        let unsupported = parse(
            r#"
                panel 0
                encoded: std/data/encode-utf8 {
                    codec = ref("conduit.codec/utf-8")
                    codec_schema_version = 0
                    codec_hash = bytes("0000000000000000000000000000000000000000000000000000000000000000")
                    maximum_input_bytes = 64
                    maximum_output_bytes = 64
                }
            "#,
        )
        .expect("unsupported profile parses");
        let failure = registry
            .resolve(&unsupported)
            .expect_err("unsupported codec hash fails resolution");
        assert_eq!(failure.code, "CND-DAT-011");
    }

    #[test]
    fn rejects_unknown_implementations() {
        let panel = parse("panel 0\nmystery: example/missing").expect("panel parses");
        let error = Registry::compatibility_demo()
            .resolve(&panel)
            .expect_err("missing implementation");
        assert_eq!(error.code, "CND-IMP-001");
    }

    #[test]
    fn source_only_module_group_and_pool_forms_require_explicit_lowering() {
        for source in [
            "panel 0\nimport \"./child.panel\" as child",
            "panel 0\nport-group > routes: fixture/request indexed max 8",
            "panel 0\npool sessions: fixture/handler { maximum = 8 admission = reject deadline_ms = 1000 idle_timeout_ms = 5000 supervision = isolate cleanup = abort }",
            "panel 0\napp { child: std/literal }\nroot app",
            "panel 0\nsource: std/literal using ready",
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
            "panel 0\na: io/stdin\nb: io/stdout\n\
             a.bytes > b.bytes {\n\
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
            "panel 0\na: io/stdin\nb: io/stdout\n\
             a.bytes > b.bytes {\n\
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
                panel 0
                input: io/stdin
                output: io/stdout
                input.bytes > output.bytes
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
                display: &mut Vec::new(),
            })
            .expect("panel runs");

        assert_eq!(output, b"pipe friendly");
    }

    #[test]
    fn nested_composites_bind_parameters_export_ports_and_preserve_views() {
        let panel = parse(
            r#"
                panel 0
                example/literal-line{
                    source: std/literal
                    export text > = source.value
                    bind value = source.value
                }
                example/upper-line{
                    source: example/literal-line
                    upper: text/uppercase
                    source.text > upper.text
                    export text > = upper.text
                    bind value = source.value
                }
                line: example/upper-line { value = "mixed Case" }
                encoded: std/data/encode-utf8 { codec = ref("conduit.codec/utf-8") codec_schema_version = 0 codec_hash = bytes("f219297cb276bc91eccddb346a8b21e7edd4414b8844014108513747ae11bf53") maximum_input_bytes = 4096 maximum_output_bytes = 4096 }
                stdout: io/stdout
                stderr: io/stderr
                line.text > encoded.text
                encoded.bytes > stdout.bytes
                encoded.bytes > stderr.bytes
            "#,
        )
        .expect("nested composite parses");
        let registry = Registry::compatibility_demo();
        let resolved = registry.resolve(&panel).expect("composite resolves");
        let logical = resolved.explain_logical();
        let expanded = resolved.explain_expanded();
        assert!(logical.contains("line: example/upper-line"));
        assert!(logical.contains("line/source: example/literal-line"));
        assert!(logical.contains("child line/upper: text/uppercase"));
        assert!(logical.contains("export output text -> line.upper.text"));
        assert!(logical.contains("bind value -> line/source.value"));
        assert!(expanded.contains("line.source.source: std/literal"));
        assert!(expanded.contains("line.upper: text/uppercase"));
        assert!(!expanded.contains("example/upper-line -> hosted builtin"));

        let mut input = &b""[..];
        let mut output = Vec::new();
        let mut error = Vec::new();
        let summary = resolved
            .run_batch(&mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
                display: &mut Vec::new(),
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
                panel 0
                example/uppercase{
                    worker: text/uppercase
                    export > text = worker.text
                    export text > = worker.text
                }
                source: std/literal { value = "boundary" }
                transform: example/uppercase
                encoded: std/data/encode-utf8 { codec = ref("conduit.codec/utf-8") codec_schema_version = 0 codec_hash = bytes("f219297cb276bc91eccddb346a8b21e7edd4414b8844014108513747ae11bf53") maximum_input_bytes = 4096 maximum_output_bytes = 4096 }
                sink: io/stdout
                source.value > transform.text
                transform.text > encoded.text
                encoded.bytes > sink.bytes
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
                display: &mut Vec::new(),
            })
            .expect("same primitive implementation runs");
        assert_eq!(output, b"BOUNDARY");
    }

    #[test]
    fn contract_only_http_service_is_not_executable() {
        let panel = parse(
            "panel 0\n\
             server: net/http/listen {\n\
               listen = \"127.0.0.1:0\"\n\
               method = \"GET\"\n\
               path = \"/health\"\n\
               response = \"ok\"\n\
               deadline_ticks = \"1000\"\n\
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
                "panel 0\nexample/a { b: example/b }\n\
                 example/b{ a: example/a }\n\
                 root: example/a",
                None,
                Some("CND-CMP-005"),
            ),
            (
                "panel 0\nexample/a {\n\
                   source: io/stdin\n\
                   export bytes > = source.bytes\n\
                   export bytes > = source.bytes\n\
                 }\nroot: example/a",
                Some("CND-SRC-002"),
                None,
            ),
            (
                "panel 0\nexample/a {\n\
                   source: io/stdin\n\
                   export bytes > = missing.bytes\n\
                 }\nroot: example/a",
                Some("CND-SRC-009"),
                None,
            ),
            (
                "panel 0\nexample/a {\n\
                   source: io/stdin\n\
                   export > bytes = source.bytes\n\
                 }\nroot: example/a",
                None,
                Some("CND-CMP-003"),
            ),
            (
                "panel 0\nexample/a {\n\
                   source: std/literal\n\
                   export value > = source.value\n\
                   bind value = source.missing\n\
                 }\nroot: example/a { value = x }",
                None,
                Some("CND-CMP-003"),
            ),
            (
                "panel 0\nexample/a {\n\
                   source: io/stdin\n\
                   export bytes > = source.bytes\n\
                 }\nroot: example/a\nsink: io/stdout\n\
                 root.source.bytes > sink.bytes",
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
