#![no_std]

//! Allocator-free semantic contracts and execution-plan validation.
//!
//! This crate intentionally contains no parser, registry, dynamic loader,
//! transport, filesystem, or product concepts. A hosted planner can lower rich
//! source and manifests into these borrowed structures; constrained firmware
//! can validate and execute the resulting bounded plan.

use core::fmt;

mod authority;
mod canonical;
mod compatibility;
mod composite;
mod config;
mod diagnostic;
mod evidence;
mod execution_plan;
mod flow;
mod implementation;
mod job;
mod lifecycle;
mod port;
mod resonance;
mod satisfaction;
mod scheduler;
mod structural;
mod type_contract;

pub use authority::{
    AuthorityConstraintRef, AuthorityDenial, AuthorityEvent, AuthorityEventKind, AuthorityGrant,
    AuthorityReason, AuthorityScope, AuthorityTime, DelegationPolicy, EffectRequirement,
    EvidenceValue, GrantStatus, HostCapability, MAX_AUTHORITY_CONSTRAINTS, NodeEffectSet,
    ObservedGrant, PlacedEffect, RedactedValueMetadata, ResolvedAuthorityBinding, ResourceRef,
    ResourceSelector, SensitivityDecision, SensitivityDisposition, SensitivityReason,
    SensitivityUse, aggregate_composite_effect_sets, assess_sensitivity, authority_bound_event,
    authority_denial_event, authority_terminal_cause, resolve_authority, resolve_authority_plan,
    validate_authority_at_use,
};
pub use canonical::{
    CANONICAL_FORM_VERSION, CANONICAL_MAGIC, CanonicalDescriptor, CanonicalError, CanonicalSink,
    CanonicalValue, FieldDisposition, MAX_CANONICAL_DEPTH, MapField, SEMANTIC_HASH_DOMAIN,
    SemanticHash,
};
pub use compatibility::{
    CompatibilityClass, CompatibilityDecision, CompatibilityOutcome, CompatibilityQuery,
    CompatibilityReason, DescriptorRef, MigrationRef, RecordField, RecordSchema,
    UnknownFieldPolicy, ValueAcceptance, assess_exact, assess_migration, assess_reader_acceptance,
};
pub use composite::{
    CompositeChild, CompositeConfigBinding, CompositeDefinition, CompositeError, CompositeExport,
    DefinitionDependencies, InstancePath, validate_composite, validate_definition_dependencies,
};
pub use config::{
    ConfigContract, ConfigContractError, ConfigContractIdentityError, ConfigFieldContract,
    ConfigIdentity, ConfigMutability, ConfigRequirement,
};
pub use diagnostic::{
    DIAGNOSTIC_SCHEMA_VERSION, Diagnostic, DiagnosticArgument, DiagnosticArgumentValue,
    DiagnosticContractError, DiagnosticEdit, DiagnosticFix, DiagnosticRelated, DiagnosticSeverity,
    DiagnosticSpan, FixApplicability,
};
pub use evidence::{
    EXECUTION_EVENT_SCHEMA_VERSION, EventCorrelation, EventPayload, EventPayloadShape,
    EventRelations, EventTerminality, EventTime, EventTimeKind, EvidenceError, EvidencePolicy,
    EvidenceReason, ExecutionEvent, ExecutionEventKind, MAX_EVENT_DERIVATIONS,
    validate_event_stream, validate_execution_event,
};
pub use execution_plan::{
    ArtifactDigest, EXECUTION_PLAN_SCHEMA_VERSION, EXECUTION_PLAN_SCHEMA_VERSION_V1,
    EXECUTION_PLAN_SCHEMA_VERSION_V2, EXECUTION_PLAN_SCHEMA_VERSION_V3,
    EXECUTION_PLAN_SCHEMA_VERSION_V4, EXECUTION_PLAN_SCHEMA_VERSION_V5,
    EXECUTION_PLAN_SCHEMA_VERSION_V6, ExecutionPlan, PinnedDescriptor, PlanArtifact, PlanAuthority,
    PlanCollection, PlanCompositeMapping, PlanDiagnosticCode, PlanEventStream, PlanExportBinding,
    PlanFanOut, PlanHostObservation, PlanIdentityError, PlanInstancePool, PlanJob, PlanMerge,
    PlanMergeInput, PlanPortGroup, PlanPortGroupMember, PlanResourceBinding, PlanResourceBudget,
    PlanSatisfactionProof, PlanSatisfactionSubject, PlanValidationContext, PlanValidationError,
    ResolvedPlanCord, ResolvedPlanNode, ResolvedPlanPort, UnresolvedPlanConstraint,
    UnresolvedPlanKind, validate_execution_plan,
};
pub use flow::{
    BlockingFairness, BoundedFlowQueue, FlowCapacity, FlowEvent, FlowEventKind, FlowEvents,
    FlowOffer, FlowPolicy, FlowPolicyDecision, FlowPolicyError, FlowPolicyReason, FlowQueueState,
    FlowTypeFacts, FlowWatermarks, OfferDisposition, OfferTransition, PopTransition, Pressure,
    QueueError, SampleSchedule, TraitProof,
};
pub use implementation::{
    BoundednessProfile, CancellationGuarantee, CheckpointRequest, ExecutionLimits,
    ExecutionProfile, HandleDisposition, HostOperationContext, HostOperationRequest,
    ImplementationError, ImplementationMachine, InstancePhase, InstantiationContext,
    LifecycleUsage, MemoryAccounting, MemoryCategory, MemoryClaim, OwnershipModel, PortTransaction,
    PrepareOutcome, ProfileIdentityError, PublicationMode, StepObservation, StepOutcome,
    StepOutcomeKind, StepUsage, TransactionResolution, TransactionState, ValueRepresentation,
    WakeInterest, WakeInterestKind, prepare_all, start_all, validate_host_operation,
    validate_instantiation, validate_plan_execution_profile,
};
pub use job::{
    CHECKPOINT_SCHEMA_VERSION, CancellationCheckpointPolicy, CheckpointCommit, CheckpointEnvelope,
    CheckpointError, CheckpointMigration, CheckpointProviderCapabilities, CheckpointRecovery,
    CheckpointStateKind, CheckpointStateRef, CheckpointStatus, DeliveryClaim, DuplicatePolicy,
    DurableCommit, JOB_CONTRACT_VERSION, JobAttemptMachine, JobContract, JobError, JobEvidenceKind,
    JobEvidenceRecord, JobIdentity, JobPhase, RecoveryDecision, RestartPolicy,
    ResultValidationDecision, ResultValidationPolicy, ResumeTarget, ValidationOutcome, WorkLease,
    validate_checkpoint_resume, validate_job_contract, validate_job_evidence_envelope,
    validate_result_decision,
};
pub use lifecycle::{
    CancellationDelivery, CancellationOutcome, CancellationRegistration, CancellationScope,
    CauseRef, CordLifecycle, CordState, LifecycleError, LifecycleEvent, LifecycleMachine,
    LifecycleState, ManagedSubject, ReplicaIdentity, ReplicaPoolContract, ReplicaState, StopPolicy,
    SubjectState, SupervisionPolicy, TerminalCause, TerminalCauseCode, TerminalClass,
    TerminalResolution, cancel_scope, cord_transition_allowed, derive_composite,
    managed_transition_allowed, replica_transition_allowed, resolve_terminal,
};
pub use port::{
    ConnectionCardinality, Delivery, Direction, LossAcceptance, PortCompatibilityDecision,
    PortCompatibilityReason, PortContract, PortFlowConstraints, Presence, Sensitivity,
    TemporalContract, TerminalContract, ValueCardinality, assess_port_connection,
    assess_port_substitution,
};
pub use resonance::{
    AppendCommit, AppendOutcome, AppendRecovery, BoundedEventRing, EventClass, EventPayloadRef,
    EventProviderCapabilities, EventStreamContract, EvidenceStreamExtension, ProjectionContract,
    ProjectionSnapshot, RESONANCE_CONTRACT_VERSION, ReadOutcome, ReplayDelivery, ReplayStart,
    ResonanceEnvelope, ResonanceError, ResonanceRelations, RetentionPolicy, StreamEntry,
    SubscriberCoupling, SubscriptionContract, extend_execution_event, validate_envelope,
    validate_projection, validate_projection_snapshot, validate_stream_contract,
    validate_subscription,
};
pub use satisfaction::{
    ExplicitSatisfactionRequirement, SATISFACTION_PROOF_SCHEMA_VERSION, SatisfactionCandidate,
    SatisfactionFacet, SatisfactionIdentityError, SatisfactionMethod, SatisfactionObligation,
    SatisfactionPin, SatisfactionProof, SatisfactionProofError, SatisfactionReason,
    SatisfactionRole, SatisfactionSelection, select_satisfaction_candidate,
    validate_port_satisfaction_proof, validate_satisfaction_proof,
};
pub use scheduler::{
    PoolPopulation, ReadyQueueDiscipline, RestartAssessment, RestartDecision,
    SCHEDULER_CONTRACT_VERSION, SchedulerContractError, SchedulerDecisionReason, SchedulerPolicy,
    assess_restart,
};
pub use structural::{
    AdapterContract, DuplicationRule, FanOutMode, LateValuePolicy, MergeCandidate, MergeOrdering,
    MergeSelection, MergeTerminalPolicy, StructuralError, StructuralNodeKind, StructuralNodeLimits,
    select_merge,
};
pub use type_contract::{TypeContractRef, TypeContractRefError, assess_type_contract_exact};

/// A stable identifier borrowed from a descriptor or resolved plan.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Id<'a>(pub &'a str);

impl<'a> Id<'a> {
    /// Creates an identifier after validating the bootstrap ASCII grammar.
    ///
    /// Local identifiers contain lowercase letters, digits, `_`, `-`, and `.`;
    /// they begin with a lowercase letter. Qualified semantic identities may
    /// additionally contain one `/`.
    pub fn new(value: &'a str) -> Result<Self, IdError> {
        if value.is_empty() {
            return Err(IdError::Empty);
        }

        let mut slash_count = 0_u8;
        let mut segment_start = true;
        for (index, byte) in value.bytes().enumerate() {
            let valid = if segment_start {
                byte.is_ascii_lowercase()
            } else {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'-' | b'.' | b'/')
            };
            if !valid {
                return Err(IdError::InvalidByte { index, byte });
            }
            if byte == b'/' {
                slash_count = slash_count.saturating_add(1);
                if slash_count > 1 || index + 1 == value.len() {
                    return Err(IdError::InvalidSlash);
                }
            }
            segment_start = matches!(byte, b'.' | b'/');
        }
        if segment_start {
            return Err(IdError::InvalidSlash);
        }

        Ok(Self(value))
    }

    /// Returns the textual identifier.
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

impl fmt::Display for Id<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Identifier validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdError {
    /// The identifier was empty.
    Empty,
    /// A byte is outside the portable identifier grammar.
    InvalidByte {
        /// Zero-based byte index.
        index: usize,
        /// Invalid byte.
        byte: u8,
    },
    /// A qualified identifier used `/` incorrectly.
    InvalidSlash,
}

impl fmt::Display for IdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("identifier is empty"),
            Self::InvalidByte { index, byte } => {
                write!(formatter, "invalid identifier byte {byte:#04x} at {index}")
            }
            Self::InvalidSlash => formatter.write_str("identifier has an invalid namespace slash"),
        }
    }
}

/// A semantic node contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeContract<'a> {
    /// Stable semantic contract identity.
    pub id: Id<'a>,
    /// Typed configuration fields, never patchable as live ports.
    pub config: ConfigContract<'a>,
    /// Input ports in stable contract order.
    pub inputs: &'a [PortContract<'a>],
    /// Output ports in stable contract order.
    pub outputs: &'a [PortContract<'a>],
}

/// A resolved node instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanNode<'a> {
    /// Stable local instance ID.
    pub id: Id<'a>,
    /// Exact semantic contract.
    pub contract: &'a NodeContract<'a>,
}

/// One end of a resolved cord.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Endpoint {
    /// Index into [`PlanGraph::nodes`].
    pub node: u16,
    /// Index into the node contract's direction-appropriate port slice.
    pub port: u16,
}

/// A resolved, typed, bounded cord.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanCord<'a> {
    /// Stable local cord ID.
    pub id: Id<'a>,
    /// Output endpoint.
    pub from: Endpoint,
    /// Input endpoint.
    pub to: Endpoint,
    /// Exact flow behavior.
    pub flow: FlowPolicy<'a>,
}

/// Borrowed semantic topology used before exact implementation resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanGraph<'a> {
    /// Resolved nodes.
    pub nodes: &'a [PlanNode<'a>],
    /// Resolved cords.
    pub cords: &'a [PlanCord<'a>],
}

/// Stable validation diagnostic code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticCode {
    /// `CND-ID-002`: duplicate local identity.
    DuplicateIdentity,
    /// `CND-PRT-001`: endpoint direction or index is invalid.
    InvalidEndpoint,
    /// `CND-PRT-002`: port cardinality is invalid.
    CardinalityViolation,
    /// `CND-TYP-001`: connected port types differ.
    TypeMismatch,
    /// `CND-PRT-003`: producer value counts exceed consumer acceptance.
    ValueCardinalityMismatch,
    /// `CND-PRT-004`: port delivery shapes differ.
    DeliveryMismatch,
    /// `CND-PRT-005`: temporal behavior is not accepted.
    TemporalMismatch,
    /// `CND-PRT-006`: terminal behavior is not accepted.
    TerminalMismatch,
    /// `CND-AUT-001`: sensitivity would be weakened.
    SensitivityViolation,
    /// `CND-FLW-001`: a live cord has no finite capacity.
    UnboundedCord,
    /// `CND-FLW-002`: endpoint loss constraints cannot be satisfied.
    FlowConstraintMismatch,
}

impl DiagnosticCode {
    /// Returns the stable external code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateIdentity => "CND-ID-002",
            Self::InvalidEndpoint => "CND-PRT-001",
            Self::CardinalityViolation => "CND-PRT-002",
            Self::TypeMismatch => "CND-TYP-001",
            Self::ValueCardinalityMismatch => "CND-PRT-003",
            Self::DeliveryMismatch => "CND-PRT-004",
            Self::TemporalMismatch => "CND-PRT-005",
            Self::TerminalMismatch => "CND-PRT-006",
            Self::SensitivityViolation => "CND-AUT-001",
            Self::UnboundedCord => "CND-FLW-001",
            Self::FlowConstraintMismatch => "CND-FLW-002",
        }
    }
}

/// One validation failure with an optional subject index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidationError {
    /// Stable diagnostic code.
    pub code: DiagnosticCode,
    /// Node or cord index when one subject exists.
    pub subject_index: Option<u16>,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.code.as_str())?;
        if let Some(index) = self.subject_index {
            write!(formatter, " at index {index}")?;
        }
        Ok(())
    }
}

/// Validates the portable structural invariants of a resolved execution plan.
///
/// Rich hosted validation may produce several diagnostics. This allocator-free
/// boundary returns the first failure in deterministic plan order.
pub fn validate_plan_graph(plan: &PlanGraph<'_>) -> Result<(), ValidationError> {
    for (index, node) in plan.nodes.iter().enumerate() {
        if plan.nodes[..index].iter().any(|prior| prior.id == node.id) {
            return Err(ValidationError {
                code: DiagnosticCode::DuplicateIdentity,
                subject_index: u16::try_from(index).ok(),
            });
        }
    }

    for (index, cord) in plan.cords.iter().enumerate() {
        if plan.cords[..index].iter().any(|prior| prior.id == cord.id) {
            return Err(ValidationError {
                code: DiagnosticCode::DuplicateIdentity,
                subject_index: u16::try_from(index).ok(),
            });
        }
        if cord.flow.capacity.items() == 0 {
            return Err(ValidationError {
                code: DiagnosticCode::UnboundedCord,
                subject_index: u16::try_from(index).ok(),
            });
        }

        let Some(source_node) = plan.nodes.get(usize::from(cord.from.node)) else {
            return Err(ValidationError {
                code: DiagnosticCode::InvalidEndpoint,
                subject_index: u16::try_from(index).ok(),
            });
        };
        let Some(source_port) = source_node
            .contract
            .outputs
            .get(usize::from(cord.from.port))
        else {
            return Err(ValidationError {
                code: DiagnosticCode::InvalidEndpoint,
                subject_index: u16::try_from(index).ok(),
            });
        };
        let Some(target_node) = plan.nodes.get(usize::from(cord.to.node)) else {
            return Err(ValidationError {
                code: DiagnosticCode::InvalidEndpoint,
                subject_index: u16::try_from(index).ok(),
            });
        };
        let Some(target_port) = target_node.contract.inputs.get(usize::from(cord.to.port)) else {
            return Err(ValidationError {
                code: DiagnosticCode::InvalidEndpoint,
                subject_index: u16::try_from(index).ok(),
            });
        };

        let type_decision =
            assess_type_contract_exact(target_port.value_type, source_port.value_type);
        let port_decision = assess_port_connection(*target_port, *source_port, type_decision);
        if port_decision.outcome != CompatibilityOutcome::Compatible {
            let code = match port_decision.reason {
                PortCompatibilityReason::Accepted => continue,
                PortCompatibilityReason::DirectionMismatch => DiagnosticCode::InvalidEndpoint,
                PortCompatibilityReason::TypeMismatch => DiagnosticCode::TypeMismatch,
                PortCompatibilityReason::PresenceMismatch
                | PortCompatibilityReason::ConnectionCardinalityMismatch => {
                    DiagnosticCode::CardinalityViolation
                }
                PortCompatibilityReason::ValueCardinalityMismatch => {
                    DiagnosticCode::ValueCardinalityMismatch
                }
                PortCompatibilityReason::DeliveryMismatch => DiagnosticCode::DeliveryMismatch,
                PortCompatibilityReason::TemporalMismatch => DiagnosticCode::TemporalMismatch,
                PortCompatibilityReason::TerminalMismatch => DiagnosticCode::TerminalMismatch,
                PortCompatibilityReason::SensitivityViolation => {
                    DiagnosticCode::SensitivityViolation
                }
                PortCompatibilityReason::FlowConstraintMismatch => {
                    DiagnosticCode::FlowConstraintMismatch
                }
            };
            return Err(ValidationError {
                code,
                subject_index: u16::try_from(index).ok(),
            });
        }
        if cord.flow.pressure.permits_loss()
            && (source_port.flow.loss == LossAcceptance::LosslessOnly
                || target_port.flow.loss == LossAcceptance::LosslessOnly)
        {
            return Err(ValidationError {
                code: DiagnosticCode::FlowConstraintMismatch,
                subject_index: u16::try_from(index).ok(),
            });
        }
    }

    for (node_index, node) in plan.nodes.iter().enumerate() {
        for (port_index, port) in node.contract.inputs.iter().enumerate() {
            let count = plan
                .cords
                .iter()
                .filter(|cord| {
                    usize::from(cord.to.node) == node_index
                        && usize::from(cord.to.port) == port_index
                })
                .count();
            if !port.connections.accepts_count(count)
                || (port.presence == Presence::Required && count == 0)
            {
                return Err(ValidationError {
                    code: DiagnosticCode::CardinalityViolation,
                    subject_index: u16::try_from(node_index).ok(),
                });
            }
        }
        for (port_index, port) in node.contract.outputs.iter().enumerate() {
            let count = plan
                .cords
                .iter()
                .filter(|cord| {
                    usize::from(cord.from.node) == node_index
                        && usize::from(cord.from.port) == port_index
                })
                .count();
            if !port.connections.accepts_count(count)
                || (port.presence == Presence::Required && count == 0)
            {
                return Err(ValidationError {
                    code: DiagnosticCode::CardinalityViolation,
                    subject_index: u16::try_from(node_index).ok(),
                });
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: TypeContractRef<'static> = TypeContractRef {
        contract_id: Id("conduit/text.utf8"),
        schema_version: 1,
        semantic_hash: SemanticHash::from_bytes([
            0x23, 0xf6, 0xb8, 0xc6, 0xd7, 0x84, 0x79, 0x9a, 0x10, 0x09, 0xbd, 0x45, 0x32, 0x26,
            0x67, 0x0d, 0xdd, 0x91, 0x80, 0xe0, 0x06, 0xd4, 0xc2, 0x32, 0x70, 0x55, 0xcb, 0xf3,
            0x50, 0x77, 0x6e, 0x9b,
        ]),
    };
    const NO_CONFIG: ConfigContract<'static> = ConfigContract { fields: &[] };
    const CAPACITY: FlowCapacity = match FlowCapacity::new(1, 64, 64) {
        Ok(capacity) => capacity,
        Err(_) => panic!("valid test capacity"),
    };
    const WATERMARKS: FlowWatermarks = match FlowWatermarks::new(0, 1, CAPACITY) {
        Ok(watermarks) => watermarks,
        Err(_) => panic!("valid test watermarks"),
    };
    const FLOW: FlowPolicy<'static> = FlowPolicy {
        capacity: CAPACITY,
        pressure: Pressure::Block(BlockingFairness::Fifo),
        watermarks: WATERMARKS,
    };
    const OUT: PortContract<'static> = PortContract {
        id: Id("out"),
        direction: Direction::Output,
        value_type: TEXT,
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
    const INPUT: PortContract<'static> = PortContract {
        id: Id("in"),
        direction: Direction::Input,
        value_type: TEXT,
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
    const SOURCE: NodeContract<'static> = NodeContract {
        id: Id("conduit/source"),
        config: NO_CONFIG,
        inputs: &[],
        outputs: &[OUT],
    };
    const SINK: NodeContract<'static> = NodeContract {
        id: Id("conduit/sink"),
        config: NO_CONFIG,
        inputs: &[INPUT],
        outputs: &[],
    };

    #[test]
    fn validates_a_typed_bounded_plan() {
        let nodes = [
            PlanNode {
                id: Id("source"),
                contract: &SOURCE,
            },
            PlanNode {
                id: Id("sink"),
                contract: &SINK,
            },
        ];
        let cords = [PlanCord {
            id: Id("speech"),
            from: Endpoint { node: 0, port: 0 },
            to: Endpoint { node: 1, port: 0 },
            flow: FLOW,
        }];
        let plan = PlanGraph {
            nodes: &nodes,
            cords: &cords,
        };

        assert_eq!(validate_plan_graph(&plan), Ok(()));
    }

    #[test]
    fn rejects_zero_capacity() {
        assert_eq!(
            FlowCapacity::new(0, 64, 64),
            Err(FlowPolicyError::ZeroCapacity)
        );
    }

    #[test]
    fn identifier_validation_is_portable() {
        assert!(Id::new("tongues/audio.stream").is_ok());
        assert!(Id::new("Hello").is_err());
        assert!(Id::new("two/slashes/here").is_err());
        assert!(Id::new("namespace/1invalid").is_err());
        assert!(Id::new("namespace/trailing.").is_err());
    }
}
