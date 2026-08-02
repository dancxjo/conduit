//! Hosted exact-plan compilation over explicit immutable inputs.
//!
//! This crate performs no discovery, fetch, provisioning, secret resolution,
//! grant acquisition, implementation loading, or execution.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use bumpalo::Bump;
use conduit_core::{
    AdministrativeApproval, AdministrativeApprovalStatus, AdministrativeApprover,
    AdministrativeCommit, AdministrativeExecution, AdministrativePrincipal, AdministrativeProof,
    AdministrativeProposal, AdministrativeSubject, AdmittedSupervisionAction, ArtifactDigest,
    ArtifactManifest, ArtifactProvenance, AuthorityConstraintRef, AuthorityGrant, AuthorityScope,
    AuthorityTime, BlockingFairness, BoundednessProfile, CancellationGuarantee, ClockRounding,
    CommitOrdering, ConfigFieldContract, ConfigIdentity, ConfigMutability, ConfigRequirement,
    ContainmentContext, ContainmentPolicy, ContainmentReason, DeadlineContract, DelegationEnvelope,
    DelegationPolicy, Direction, DistributionProvider, EXECUTION_PLAN_SCHEMA_VERSION,
    EffectClassBinding, EffectClassTraits, EffectCommitProfile, EffectDiscontinuity,
    EffectFlowBinding, EffectIdempotency, EffectRequirement, ExecutionGuarantee, ExecutionLane,
    ExecutionLimits, ExecutionPlacement, ExecutionPlan, ExecutionProfile, ExecutorKind,
    FeedbackBoundaryKind, FeedbackInitialization, FeedbackReplayGapPolicy, FeedbackTerminalPolicy,
    FlowCapacity, FlowPolicy, FlowWatermarks, ForeignRetention, GenesisReason, GrantStatus,
    HandleDisposition, HazardClosureContext, HazardClosureLimits, HazardClosurePolicy,
    HazardClosureReason, HazardPermit, HazardProofKind, HazardProofNode, HazardousHostBinding,
    HazardousHostProfile, HostCapability, HostDistributionKind, Id, ImplementationConfinement,
    ImplementationManifest, InhibitLatchState, InhibitObservation, InstancePath, IsolationProfile,
    MAX_HAZARD_PROOF_NODES, ManifestArtifactRef, ManifestEntrypoint, ManifestInterface,
    MemoryAccounting, MemoryCategory, MemoryClaim, ObservedGrant, OperatingEnvelopeLimit,
    OwnershipModel, PassportStatus, PassportStatusObservation, PersistentBudgetPolicy,
    PinnedDescriptor, PlanArtifact, PlanAuthority, PlanClockConversion, PlanCompositeMapping,
    PlanEvidenceProviderBinding, PlanExportBinding, PlanFeedbackBoundary, PlanHazardClosure,
    PlanHostObservation, PlanInstancePool, PlanPolicyBudget, PlanPoolRuntime, PlanPortGroup,
    PlanPortGroupMember, PlanResourceBinding, PlanResourceBudget, PlanSupervision,
    PlanSupervisionTarget, PlanValidationContext, PlanWorkload, PolicyBudgetAnchor,
    PolicyBudgetAvailability, PolicyBudgetLease, PolicyBudgetLimits, PolicyBudgetReason,
    PolicyBudgetStatus, PolicyLeaseRule, PoolAdmissionPolicy, PoolCleanupPolicy, PoolContract,
    PoolGenerationReservation, PoolReservationProfile, PoolSupervisionPolicy, Pressure,
    ProviderAvailability, ProviderRequirement, ProviderRiskTraits, ProviderSelection,
    ReferenceDistributionProfile, ReplacementSupport, ReportCapability, ReportMembership,
    ReportResource, ReportTopology, ResolvedAuthorityBinding, ResolvedPlanCord, ResolvedPlanNode,
    ResolvedPlanPort, ResourceLeaseContract, ResourceRef, ResourceSelector, ResourceSharingMode,
    RollingLimit, SampleSchedule, SemanticHash, Sensitivity, StopPolicy, SupervisionActionKind,
    SupervisionContract, SupervisionFailureMode, SupervisionLimits, SupervisionScope,
    TemporalContract, ToxicCombinationRule, ToxicEffectPattern, ToxicFlowRequirement,
    TraitRequirement, TypeContractRef, UnknownCommitPolicy, ValueEnvelopePolicy,
    ValueRepresentation, WatchAdmission, WatchRetention, WatchSubject, WorkloadBudget,
    WorkloadCapability, WorkloadContract, WorkloadEvidenceKind, WorkloadGuarantee, WorkloadLimit,
    analyze_effect_closure, assess_provider_requirement, resolve_authority,
    validate_administrative_proof, validate_effect_commit_profile, validate_reference_distribution,
    validate_resource_lease,
};
use conduit_panel::{LoadedModule, ModuleGraph, ModuleLoader, SourcePressure};
use conduit_runtime::{
    CandidateAuthority, CapabilityPredicate, ExactTopologyView, ExecutionArrangementPolicy,
    HostResolverPolicy, LiteralValidationError, OwnedConfigFieldSchema, OwnedConfigRequirement,
    OwnedInterfaceContract, OwnedNodeContract, OwnedNodeSchema, OwnedPortContract,
    OwnedPortReference, OwnedSemanticValue, OwnedTypeReference, PlacementCandidate,
    PlacementRequest, Registry, ResolvedExecutionArrangement, ResolvedExecutionBoundary,
    ResolvedExecutionCommitDomain, ResolvedExecutionDescriptor, ResolvedExecutionLane,
    ResolvedExecutionPlacement, ResolvedExecutionRegion, ResolverTiePolicy, ResourcePredicate,
    SourceContractCatalog, TopologyPredicate, lower_source, resolve_execution_arrangement,
    resolve_host_placement, seal_resolved_execution_plan, validate_hosted_execution_plan,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

mod installed_profile;

pub use installed_profile::{
    ExternalHostServiceAuthorityObservationInput, HostServiceAuthorityObservationInput,
    InstalledHostObservationInput, InstalledProfile, ObservedHostServiceAuthority,
    fixture_host_service_authority_observation, observed_external_host_service_authority,
    observed_host_service_authority, observed_host_service_constraints,
};

pub const COMPILE_INPUT_SCHEMA: &str = "conduit.compile-input";
pub const COMPILE_INPUT_SCHEMA_VERSION: u16 = 0;
pub const PLAN_DOCUMENT_SCHEMA: &str = "conduit.execution-plan";
pub const REFERENCE_DISTRIBUTION_DOCUMENT_SCHEMA: &str = "conduit.reference-distribution";
pub const MAXIMUM_COMPILE_INPUT_DOCUMENT_BYTES: u64 = 16 * 1024 * 1024;
pub const MAXIMUM_COMPILE_ENTRY_SOURCE_BYTES: u64 = 4 * 1024 * 1024;
pub const MAXIMUM_COMPILE_MODULE_SOURCE_BYTES: u64 = 4 * 1024 * 1024;
pub const MAXIMUM_COMPILE_MODULE_CLOSURE_BYTES: u64 = 8 * 1024 * 1024;
pub const MAXIMUM_COMPILE_MODULES: u16 = 256;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetDocument {
    pub memory_bytes: u64,
    pub storage_bytes: u64,
    pub cpu_units: u32,
    pub timers: u16,
    pub transports: u16,
    pub checkpoints: u16,
    pub evidence_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionArrangementPolicyDocument {
    pub plan_epoch: u64,
    pub boundary_realization: PinDocument,
    pub maximum_proposal_bytes: u64,
    pub maximum_head_of_line_ticks: u64,
    pub cancellation_slots: u16,
    pub evidence_slots: u32,
}

/// Current fixed-hosted arrangement policy used by the installed Conduit
/// profile. Other hosts must supply their own exact realization pin and bounds.
#[must_use]
pub fn fixed_hosted_execution_arrangement_policy() -> ExecutionArrangementPolicyDocument {
    ExecutionArrangementPolicyDocument {
        plan_epoch: 1,
        boundary_realization: PinDocument {
            id: "conduit/fixed-hosted-mailbox".to_owned(),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([57; 32]).to_string(),
        },
        maximum_proposal_bytes: 1024 * 1024,
        maximum_head_of_line_ticks: 256,
        cancellation_slots: 64,
        evidence_slots: 4096,
    }
}

impl From<BudgetDocument> for PlanResourceBudget {
    fn from(value: BudgetDocument) -> Self {
        Self {
            memory_bytes: value.memory_bytes,
            storage_bytes: value.storage_bytes,
            cpu_units: value.cpu_units,
            timers: value.timers,
            transports: value.transports,
            checkpoints: value.checkpoints,
            evidence_bytes: value.evidence_bytes,
        }
    }
}

impl From<PlanResourceBudget> for BudgetDocument {
    fn from(value: PlanResourceBudget) -> Self {
        Self {
            memory_bytes: value.memory_bytes,
            storage_bytes: value.storage_bytes,
            cpu_units: value.cpu_units,
            timers: value.timers,
            transports: value.transports,
            checkpoints: value.checkpoints,
            evidence_bytes: value.evidence_bytes,
        }
    }
}

impl From<ExecutionLimitsDocument> for ExecutionLimits {
    fn from(value: ExecutionLimitsDocument) -> Self {
        Self {
            max_step_work: value.max_step_work,
            max_retained_values: value.max_retained_values,
            max_retained_bytes: value.max_retained_bytes,
            max_scratch_bytes: value.max_scratch_bytes,
            max_input_leases: value.max_input_leases,
            max_input_bytes: value.max_input_bytes,
            max_output_reservations: value.max_output_reservations,
            max_output_bytes: value.max_output_bytes,
            max_transactions: value.max_transactions,
            max_fragments_per_step: value.max_fragments_per_step,
            max_pending_operations: value.max_pending_operations,
            max_timers: value.max_timers,
            max_child_tasks: value.max_child_tasks,
            max_host_buffer_bytes: value.max_host_buffer_bytes,
            max_foreign_queue_items: value.max_foreign_queue_items,
            max_foreign_queue_bytes: value.max_foreign_queue_bytes,
            max_checkpoint_bytes: value.max_checkpoint_bytes,
            implementation_memory_bytes: value.implementation_memory_bytes,
            cancellation_ticks: value.cancellation_ticks,
        }
    }
}

impl From<ExecutionLimits> for ExecutionLimitsDocument {
    fn from(value: ExecutionLimits) -> Self {
        Self {
            max_step_work: value.max_step_work,
            max_retained_values: value.max_retained_values,
            max_retained_bytes: value.max_retained_bytes,
            max_scratch_bytes: value.max_scratch_bytes,
            max_input_leases: value.max_input_leases,
            max_input_bytes: value.max_input_bytes,
            max_output_reservations: value.max_output_reservations,
            max_output_bytes: value.max_output_bytes,
            max_transactions: value.max_transactions,
            max_fragments_per_step: value.max_fragments_per_step,
            max_pending_operations: value.max_pending_operations,
            max_timers: value.max_timers,
            max_child_tasks: value.max_child_tasks,
            max_host_buffer_bytes: value.max_host_buffer_bytes,
            max_foreign_queue_items: value.max_foreign_queue_items,
            max_foreign_queue_bytes: value.max_foreign_queue_bytes,
            max_checkpoint_bytes: value.max_checkpoint_bytes,
            implementation_memory_bytes: value.implementation_memory_bytes,
            cancellation_ticks: value.cancellation_ticks,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PinDocument {
    pub id: String,
    pub schema_version: u32,
    pub semantic_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactReferenceDocument {
    pub id: String,
    pub digest: String,
    pub role: String,
    pub required: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationInterfaceDocument {
    pub interface: PinDocument,
    pub entrypoint: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationDocument {
    pub schema_version: u32,
    pub identity: String,
    pub id: String,
    pub implementation_version: String,
    pub semantic_contract: PinDocument,
    pub executor: String,
    pub entrypoint_name: String,
    pub entrypoint_adapter: String,
    pub entrypoint_abi: String,
    pub runtime_protocol_version: u32,
    pub execution_profile: PinDocument,
    pub artifacts: Vec<ArtifactReferenceDocument>,
    pub required_interfaces: Vec<ImplementationInterfaceDocument>,
    pub provided_interfaces: Vec<ImplementationInterfaceDocument>,
    #[serde(default)]
    pub required_authorities: Vec<String>,
    #[serde(default)]
    pub required_effects: Vec<String>,
    pub minimum_plan_version: u32,
    pub maximum_plan_version: u32,
    pub minimum_runtime_protocol: u32,
    pub maximum_runtime_protocol: u32,
    pub coexistence_memory_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionLimitsDocument {
    pub max_step_work: u32,
    pub max_retained_values: u16,
    pub max_retained_bytes: u64,
    pub max_scratch_bytes: u32,
    pub max_input_leases: u16,
    pub max_input_bytes: u64,
    pub max_output_reservations: u16,
    pub max_output_bytes: u64,
    pub max_transactions: u16,
    pub max_fragments_per_step: u16,
    pub max_pending_operations: u16,
    pub max_timers: u16,
    pub max_child_tasks: u16,
    pub max_host_buffer_bytes: u64,
    pub max_foreign_queue_items: u16,
    pub max_foreign_queue_bytes: u64,
    pub max_checkpoint_bytes: u64,
    pub implementation_memory_bytes: u64,
    pub cancellation_ticks: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValueRepresentationDocument {
    pub direction: String,
    pub port: String,
    pub semantic_type: PinDocument,
    pub representation: PinDocument,
    pub ownership: String,
    pub disposition: String,
    pub max_bytes: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryClaimDocument {
    pub category: String,
    pub accounting: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionProfileDocument {
    pub id: String,
    pub schema_version: u32,
    pub semantic_hash: String,
    pub boundedness: String,
    pub cancellation: String,
    pub step_bound_enforced: bool,
    pub limits: ExecutionLimitsDocument,
    pub representations: Vec<ValueRepresentationDocument>,
    pub memory_claims: Vec<MemoryClaimDocument>,
    pub checkpoint: Option<PinDocument>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDocument {
    pub schema_version: u32,
    pub identity: String,
    pub id: String,
    pub digest: String,
    pub media_type: String,
    pub byte_size: u64,
    pub target: Option<String>,
    pub abi: Option<String>,
    pub builder: String,
    pub source_digest: String,
    pub build_recipe_digest: String,
    pub reproducible: bool,
    #[serde(default)]
    pub license_expressions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReportCapabilityDocument {
    pub interface: PinDocument,
    pub mode: String,
    pub subject: String,
    pub details: String,
    pub capacity: BudgetDocument,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReportResourceDocument {
    pub kind: String,
    pub id: String,
    pub descriptor: PinDocument,
    pub capacity: BudgetDocument,
    pub exclusive: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReportTopologyDocument {
    pub id: String,
    pub contract: PinDocument,
    pub from: String,
    pub to: String,
    pub maximum_transfer_unit: u32,
    pub maximum_sessions: u32,
    pub reachable: bool,
    pub details: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPlacementObservationDocument {
    pub id: String,
    pub provider: PinDocument,
    pub authority_boundary: PinDocument,
    pub resource_boundary: PinDocument,
    pub lifecycle_boundary: PinDocument,
    pub failure_boundary: PinDocument,
    pub generation: u64,
    pub isolation: String,
    pub memory_containment: String,
    pub regain_control: String,
    pub effect_fencing: String,
    pub stop_execution: String,
    pub reclaim_resources: String,
    pub maximum_regain_control_ticks: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionLaneObservationDocument {
    pub id: String,
    pub placement: String,
    pub placement_generation: u64,
    pub generation: u64,
    pub independent_progress: String,
    pub simultaneous_execution: String,
    pub preemption: String,
    pub termination: String,
    pub ready_slots: u16,
    pub wake_slots: u16,
    pub proposal_slots: u16,
    pub commit_slots: u16,
    pub timer_slots: u16,
    pub scratch_bytes: u32,
    pub stack_bytes: u32,
    pub evidence_slots: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReportMembershipDocument {
    pub realm: String,
    pub entity: String,
    pub passport: String,
    pub status_reporter: PinDocument,
    pub status_time_basis: String,
    pub status_observed_at_tick: u64,
    pub status_valid_until_tick: u64,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostReportDocument {
    pub schema_version: u32,
    pub identity: String,
    pub id: String,
    pub host: String,
    pub boot_id: String,
    pub reporter: PinDocument,
    pub trust: PinDocument,
    pub membership: Option<ReportMembershipDocument>,
    pub time_basis: String,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub available: BudgetDocument,
    #[serde(default)]
    pub capabilities: Vec<ReportCapabilityDocument>,
    #[serde(default)]
    pub resources: Vec<ReportResourceDocument>,
    #[serde(default)]
    pub topology: Vec<ReportTopologyDocument>,
    pub execution_placements: Vec<ExecutionPlacementObservationDocument>,
    pub execution_lanes: Vec<ExecutionLaneObservationDocument>,
    pub supported_executors: Vec<String>,
    #[serde(default)]
    pub supported_targets: Vec<String>,
    #[serde(default)]
    pub supported_abis: Vec<String>,
    pub minimum_plan_version: u32,
    pub maximum_plan_version: u32,
    #[serde(default)]
    pub current_constraints: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRequirementDocument {
    pub interface: PinDocument,
    pub mode: String,
    pub subject: Option<String>,
    pub details: Option<String>,
    pub minimum_capacity: BudgetDocument,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRequirementDocument {
    pub kind: String,
    pub id: Option<String>,
    pub descriptor: Option<PinDocument>,
    pub minimum_capacity: BudgetDocument,
    pub require_exclusive: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TopologyRequirementDocument {
    pub contract: PinDocument,
    pub from: String,
    pub to: String,
    pub minimum_transfer_unit: u32,
    pub minimum_sessions: u32,
    pub details: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityConstraintDocument {
    pub id: String,
    pub semantic_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectRequirementDocument {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub administrative_class: Option<PinDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_budget_class: Option<PinDocument>,
    pub action: String,
    pub resource_kind: String,
    pub resource_id: Option<String>,
    pub requester: String,
    pub audience: String,
    pub constraints: Vec<AuthorityConstraintDocument>,
    pub check_at_use: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostCapabilityDocument {
    pub id: String,
    pub action: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub host: String,
    pub time_basis: String,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityGrantDocument {
    pub id: String,
    pub action: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub scope_root: String,
    pub scope_descendants: bool,
    pub audience: String,
    pub constraints: Vec<AuthorityConstraintDocument>,
    pub time_basis: String,
    pub not_before_tick: u64,
    pub expires_at_tick: u64,
    pub issued_for_host: String,
    pub delegation: String,
    pub audit_id: String,
    pub terminal_policy: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityDecisionDocument {
    pub requirement: String,
    pub effect_hash: String,
    pub grant_hash: String,
    pub effect: EffectRequirementDocument,
    pub capability: HostCapabilityDocument,
    pub grant: AuthorityGrantDocument,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub administrative_subject: Option<AdministrativeSubjectDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containment: Option<AdministrativeProofDocument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_budgets: Vec<PolicyBudgetBindingDocument>,
    pub resource_lease: ResourceLeaseDocument,
    pub commit_profile: EffectCommitProfileDocument,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdministrativePrincipalDocument {
    pub realm: String,
    pub entity: String,
    pub key: String,
    pub profile: PinDocument,
    pub source_plan: String,
    pub source_epoch: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdministrativeSubjectDocument {
    pub realm: String,
    pub entity: String,
    pub plan: String,
    pub epoch: u64,
    pub artifact: Option<String>,
    pub budget: Option<PinDocument>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DelegationEnvelopeDocument {
    pub action: String,
    pub resource_kind: String,
    pub resource_id: Option<String>,
    pub audience: String,
    pub time_basis: String,
    pub not_before_tick: u64,
    pub expires_at_tick: u64,
    pub remaining_depth: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdministrativeApproverDocument {
    pub realm: String,
    pub entity: String,
    pub key: String,
    pub profile: PinDocument,
    pub failure_domain: PinDocument,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContainmentPolicyDocument {
    pub schema_version: u32,
    pub identity: String,
    pub descriptor: PinDocument,
    pub effect_class: PinDocument,
    pub approvers: Vec<AdministrativeApproverDocument>,
    pub committer: AdministrativeApproverDocument,
    pub executor: AdministrativeApproverDocument,
    pub minimum_approvals: u8,
    pub minimum_failure_domains: u8,
    pub requester_independence: bool,
    pub beneficiary_independence: bool,
    pub successor_independence: bool,
    pub delegation_ceiling: Option<DelegationEnvelopeDocument>,
    pub ceremony: Option<PinDocument>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdministrativeProposalDocument {
    pub schema_version: u32,
    pub identity: String,
    pub id: String,
    pub effect_class: PinDocument,
    pub operation: PinDocument,
    pub requester: AdministrativePrincipalDocument,
    pub subject: AdministrativeSubjectDocument,
    pub beneficiaries: Vec<AdministrativeSubjectDocument>,
    pub predecessor_plan: Option<String>,
    pub delegation: Option<DelegationEnvelopeDocument>,
    pub protected_handle: Option<PinDocument>,
    pub ceremony: Option<PinDocument>,
    pub time_basis: String,
    pub created_at_tick: u64,
    pub expires_at_tick: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdministrativeApprovalDocument {
    pub schema_version: u32,
    pub identity: String,
    pub id: String,
    pub proposal_identity: String,
    pub policy_identity: String,
    pub approver: AdministrativePrincipalDocument,
    pub failure_domain: PinDocument,
    pub time_basis: String,
    pub issued_at_tick: u64,
    pub expires_at_tick: u64,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdministrativeCommitDocument {
    pub schema_version: u32,
    pub identity: String,
    pub id: String,
    pub proposal_identity: String,
    pub policy_identity: String,
    pub approvals: Vec<String>,
    pub committed_by: AdministrativePrincipalDocument,
    pub committed_at_tick: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdministrativeExecutionDocument {
    pub schema_version: u32,
    pub identity: String,
    pub id: String,
    pub proposal_identity: String,
    pub commit_identity: String,
    pub executor: AdministrativePrincipalDocument,
    pub time_basis: String,
    pub not_before_tick: u64,
    pub expires_at_tick: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdministrativeProofDocument {
    pub proposal: AdministrativeProposalDocument,
    pub policy: ContainmentPolicyDocument,
    pub approvals: Vec<AdministrativeApprovalDocument>,
    pub commit: AdministrativeCommitDocument,
    pub execution: AdministrativeExecutionDocument,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectClassBindingDocument {
    pub identity: String,
    pub descriptor: PinDocument,
    pub persistence: bool,
    pub delegation: bool,
    pub distributed: bool,
    pub administrative: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToxicEffectPatternDocument {
    pub id: String,
    pub class: PinDocument,
    pub resource_kind: Option<String>,
    pub resource_id: Option<String>,
    pub audience: Option<String>,
    pub host: Option<String>,
    pub realm: Option<String>,
    pub budget: Option<PinDocument>,
    pub persistence: String,
    pub delegation: String,
    pub distributed: String,
    pub administrative: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToxicFlowRequirementDocument {
    pub from_pattern: u8,
    pub to_pattern: u8,
    pub transfer: PinDocument,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToxicCombinationRuleDocument {
    pub identity: String,
    pub descriptor: PinDocument,
    pub patterns: Vec<ToxicEffectPatternDocument>,
    pub flows: Vec<ToxicFlowRequirementDocument>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HazardClosureLimitsDocument {
    pub maximum_effects: u16,
    pub maximum_classes: u8,
    pub maximum_rules: u8,
    pub maximum_patterns_per_rule: u8,
    pub maximum_flows: u8,
    pub maximum_permits: u8,
    pub maximum_proof_nodes: u8,
    pub maximum_search_steps: u32,
}

impl From<HazardClosureLimitsDocument> for HazardClosureLimits {
    fn from(value: HazardClosureLimitsDocument) -> Self {
        Self {
            maximum_effects: value.maximum_effects,
            maximum_classes: value.maximum_classes,
            maximum_rules: value.maximum_rules,
            maximum_patterns_per_rule: value.maximum_patterns_per_rule,
            maximum_flows: value.maximum_flows,
            maximum_permits: value.maximum_permits,
            maximum_proof_nodes: value.maximum_proof_nodes,
            maximum_search_steps: value.maximum_search_steps,
        }
    }
}

impl From<HazardClosureLimits> for HazardClosureLimitsDocument {
    fn from(value: HazardClosureLimits) -> Self {
        Self {
            maximum_effects: value.maximum_effects,
            maximum_classes: value.maximum_classes,
            maximum_rules: value.maximum_rules,
            maximum_patterns_per_rule: value.maximum_patterns_per_rule,
            maximum_flows: value.maximum_flows,
            maximum_permits: value.maximum_permits,
            maximum_proof_nodes: value.maximum_proof_nodes,
            maximum_search_steps: value.maximum_search_steps,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HazardClosurePolicyDocument {
    pub schema_version: u32,
    pub identity: String,
    pub descriptor: PinDocument,
    pub permit_class: PinDocument,
    pub classes: Vec<EffectClassBindingDocument>,
    pub rules: Vec<ToxicCombinationRuleDocument>,
    pub limits: HazardClosureLimitsDocument,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectFlowBindingDocument {
    pub from_effect: String,
    pub to_effect: String,
    pub transfer: PinDocument,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HazardPermitDocument {
    pub identity: String,
    pub descriptor: PinDocument,
    pub policy_identity: String,
    pub rule_identity: String,
    pub plan_subject: String,
    pub epoch: u64,
    pub scope_identity: String,
    pub time_basis: String,
    pub not_before_tick: u64,
    pub expires_at_tick: u64,
    pub approval: AdministrativeProofDocument,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OperatingEnvelopeLimitDocument {
    pub dimension: PinDocument,
    pub minimum: i64,
    pub maximum: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HazardousHostProfileDocument {
    pub schema_version: u32,
    pub identity: String,
    pub descriptor: PinDocument,
    pub safe_state: PinDocument,
    pub inhibit_boundary: PinDocument,
    pub watchdog: PinDocument,
    pub effect_boundary: PinDocument,
    pub command_effect_class: PinDocument,
    pub clear_effect_class: PinDocument,
    pub clear_operation: PinDocument,
    pub clear_ceremony: PinDocument,
    pub time_basis: String,
    pub maximum_command_horizon_ticks: u64,
    pub maximum_observation_age_ticks: u64,
    pub maximum_evidence_records: u32,
    pub require_physical_presence_to_clear: bool,
    pub require_isolated_implementation: bool,
    pub envelope: Vec<OperatingEnvelopeLimitDocument>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InhibitObservationDocument {
    pub schema_version: u32,
    pub identity: String,
    pub profile_identity: String,
    pub host: String,
    pub safe_state: PinDocument,
    pub inhibit_boundary: PinDocument,
    pub watchdog: PinDocument,
    pub effect_boundary: PinDocument,
    pub time_basis: String,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub latch_generation: u64,
    pub latch_state: String,
    pub independent_from_plan: bool,
    pub local_safe_path: bool,
    pub survives_executor_loss: bool,
    pub survives_partition: bool,
    pub graph_cannot_replace: bool,
    pub confinement: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HazardousHostBindingDocument {
    pub host: String,
    pub profile: HazardousHostProfileDocument,
    pub observation: InhibitObservationDocument,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HazardClosureDocument {
    pub epoch: u64,
    pub plan_subject: String,
    pub policy: HazardClosurePolicyDocument,
    pub flows: Vec<EffectFlowBindingDocument>,
    pub permits: Vec<HazardPermitDocument>,
    pub decision_identity: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hazardous_hosts: Vec<HazardousHostBindingDocument>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyBudgetLimitsDocument {
    pub current_stock: Option<u64>,
    pub rolling_units: Option<u64>,
    pub rolling_window_ticks: Option<u64>,
    pub lifetime: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyLeaseRuleDocument {
    pub maximum_ticks: u64,
    pub renewal_authority: PinDocument,
    pub offline_allowed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistentBudgetPolicyDocument {
    pub schema_version: u32,
    pub identity: String,
    pub descriptor: PinDocument,
    pub owner: PinDocument,
    pub subject: PinDocument,
    pub anchor_kind: String,
    pub anchor_id: String,
    pub action: String,
    pub resource_class: PinDocument,
    pub time_basis: String,
    pub limits: PolicyBudgetLimitsDocument,
    pub reservation_ttl_ticks: u64,
    pub lease: Option<PolicyLeaseRuleDocument>,
    pub audit_id: String,
    pub persistence_profile: PinDocument,
    pub maximum_reservations: u16,
    pub maximum_evidence_events: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyBudgetStatusDocument {
    pub schema_version: u32,
    pub identity: String,
    pub policy_identity: String,
    pub ledger: PinDocument,
    pub checkpoint: String,
    pub sequence: u64,
    pub current_stock: u64,
    pub rolling_window_start: u64,
    pub rolling_committed: u64,
    pub lifetime_committed: u64,
    pub reserved: u64,
    pub evidence_remaining: u32,
    pub availability: String,
    pub time_basis: String,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyBudgetLeaseDocument {
    pub schema_version: u32,
    pub identity: String,
    pub policy_identity: String,
    pub holder: PinDocument,
    pub renewal_authority: PinDocument,
    pub time_basis: String,
    pub issued_at_tick: u64,
    pub expires_at_tick: u64,
    pub offline: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyBudgetBindingDocument {
    pub policy: PersistentBudgetPolicyDocument,
    pub status: PolicyBudgetStatusDocument,
    pub lease: Option<PolicyBudgetLeaseDocument>,
    pub required_units: u64,
    pub check_at_use: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompileModuleDocument {
    pub canonical_uri: String,
    pub content_hash: String,
    pub source: String,
}

/// Identity-bound source and explicit-module-closure limits.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompileSourceLimits {
    pub maximum_entry_source_bytes: u64,
    pub maximum_module_source_bytes: u64,
    pub maximum_module_closure_bytes: u64,
    pub maximum_modules: u16,
}

impl Default for CompileSourceLimits {
    fn default() -> Self {
        Self {
            maximum_entry_source_bytes: MAXIMUM_COMPILE_ENTRY_SOURCE_BYTES,
            maximum_module_source_bytes: MAXIMUM_COMPILE_MODULE_SOURCE_BYTES,
            maximum_module_closure_bytes: MAXIMUM_COMPILE_MODULE_CLOSURE_BYTES,
            maximum_modules: MAXIMUM_COMPILE_MODULES,
        }
    }
}

/// Exact finite semantic catalog snapshot used during lowering.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompileCatalogDocument {
    pub identity: String,
    pub nodes: Vec<PinDocument>,
    pub types: Vec<PinDocument>,
    pub ports: Vec<PinDocument>,
    pub external_leaf_contracts: Vec<ExternalLeafContractDocument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interfaces: Vec<PinDocument>,
}

/// Complete config-free domain leaf contract sealed into compile input.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalLeafContractDocument {
    pub id: String,
    pub config: Vec<ExternalConfigFieldDocument>,
    pub inputs: Vec<ExternalPortContractDocument>,
    pub outputs: Vec<ExternalPortContractDocument>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalConfigFieldDocument {
    pub key: String,
    pub value_type: PinDocument,
    pub requirement: String,
    pub sensitivity: String,
    pub mutability: String,
    pub identity: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalPortContractDocument {
    pub id: String,
    pub direction: String,
    pub value_type: PinDocument,
    pub presence: String,
    pub connections: String,
    pub values: String,
    pub delivery: String,
    pub temporal: String,
    pub terminal: String,
    pub sensitivity: String,
    pub loss: String,
}

impl ExternalLeafContractDocument {
    fn from_contract(contract: &conduit_core::NodeContract<'_>) -> Option<Self> {
        Some(Self {
            id: contract.id.as_str().to_owned(),
            config: contract
                .config
                .fields
                .iter()
                .copied()
                .map(ExternalConfigFieldDocument::from_contract)
                .collect::<Option<_>>()?,
            inputs: contract
                .inputs
                .iter()
                .map(ExternalPortContractDocument::from_contract)
                .collect(),
            outputs: contract
                .outputs
                .iter()
                .map(ExternalPortContractDocument::from_contract)
                .collect(),
        })
    }

    fn to_schema(&self) -> Option<OwnedNodeSchema> {
        Some(OwnedNodeSchema {
            id: self.id.clone(),
            fields: self
                .config
                .iter()
                .map(ExternalConfigFieldDocument::to_owned)
                .collect::<Option<_>>()?,
        })
    }

    fn to_owned(&self) -> Option<OwnedNodeContract> {
        Some(OwnedNodeContract {
            id: self.id.clone(),
            inputs: self
                .inputs
                .iter()
                .map(ExternalPortContractDocument::to_owned)
                .collect::<Option<_>>()?,
            outputs: self
                .outputs
                .iter()
                .map(ExternalPortContractDocument::to_owned)
                .collect::<Option<_>>()?,
        })
    }
}

impl ExternalConfigFieldDocument {
    fn from_contract(field: ConfigFieldContract<'_>) -> Option<Self> {
        let requirement = match field.requirement {
            ConfigRequirement::Required => "required",
            ConfigRequirement::Optional => "optional",
            ConfigRequirement::Defaulted(_) => return None,
        };
        Some(Self {
            key: field.key.as_str().to_owned(),
            value_type: PinDocument {
                id: field.value_type.contract_id.as_str().to_owned(),
                schema_version: field.value_type.schema_version,
                semantic_hash: field.value_type.semantic_hash.to_string(),
            },
            requirement: requirement.to_owned(),
            sensitivity: field.sensitivity.as_str().to_owned(),
            mutability: field.mutability.as_str().to_owned(),
            identity: field.identity.as_str().to_owned(),
        })
    }

    fn to_owned(&self) -> Option<OwnedConfigFieldSchema> {
        Some(OwnedConfigFieldSchema {
            key: self.key.clone(),
            value_type: OwnedTypeReference {
                id: self.value_type.id.clone(),
                schema_version: self.value_type.schema_version,
                semantic_hash: parse_hash(&self.value_type.semantic_hash).ok()?,
            },
            requirement: match self.requirement.as_str() {
                "required" => OwnedConfigRequirement::Required,
                "optional" => OwnedConfigRequirement::Optional,
                _ => return None,
            },
            sensitivity: match self.sensitivity.as_str() {
                "public" => Sensitivity::Public,
                "restricted" => Sensitivity::Restricted,
                "secret" => Sensitivity::Secret,
                _ => return None,
            },
            mutability: match self.mutability.as_str() {
                "pre-start" => ConfigMutability::PreStart,
                "runtime" => ConfigMutability::Runtime,
                _ => return None,
            },
            identity: match self.identity.as_str() {
                "semantic" => ConfigIdentity::Semantic,
                "plan" => ConfigIdentity::Plan,
                _ => return None,
            },
            default_origin: None,
        })
    }

    fn to_core(&self) -> Option<ConfigFieldContract<'_>> {
        let owned = self.to_owned()?;
        Some(ConfigFieldContract {
            key: Id::new(&self.key).ok()?,
            value_type: TypeContractRef {
                contract_id: Id::new(&self.value_type.id).ok()?,
                schema_version: self.value_type.schema_version,
                semantic_hash: parse_hash(&self.value_type.semantic_hash).ok()?,
            },
            requirement: match owned.requirement {
                OwnedConfigRequirement::Required => ConfigRequirement::Required,
                OwnedConfigRequirement::Optional => ConfigRequirement::Optional,
                OwnedConfigRequirement::Defaulted(_) => return None,
            },
            sensitivity: owned.sensitivity,
            mutability: owned.mutability,
            identity: owned.identity,
        })
    }
}

impl ExternalPortContractDocument {
    fn from_contract(port: &conduit_core::PortContract<'_>) -> Self {
        Self {
            id: port.id.as_str().to_owned(),
            direction: port.direction.as_str().to_owned(),
            value_type: PinDocument {
                id: port.value_type.contract_id.as_str().to_owned(),
                schema_version: port.value_type.schema_version,
                semantic_hash: port.value_type.semantic_hash.to_string(),
            },
            presence: port.presence.as_str().to_owned(),
            connections: port.connections.as_str().to_owned(),
            values: port.values.as_str().to_owned(),
            delivery: port.delivery.as_str().to_owned(),
            temporal: port.temporal.as_str().to_owned(),
            terminal: port.terminal.as_str().to_owned(),
            sensitivity: port.sensitivity.as_str().to_owned(),
            loss: port.flow.loss.as_str().to_owned(),
        }
    }

    fn to_owned(&self) -> Option<OwnedPortContract> {
        Some(OwnedPortContract {
            id: self.id.clone(),
            direction: match self.direction.as_str() {
                "input" => Direction::Input,
                "output" => Direction::Output,
                _ => return None,
            },
            value_type: OwnedTypeReference {
                id: self.value_type.id.clone(),
                schema_version: self.value_type.schema_version,
                semantic_hash: parse_hash(&self.value_type.semantic_hash).ok()?,
            },
            presence: match self.presence.as_str() {
                "required" => conduit_core::Presence::Required,
                "optional" => conduit_core::Presence::Optional,
                _ => return None,
            },
            connections: match self.connections.as_str() {
                "exactly-one" => conduit_core::ConnectionCardinality::ExactlyOne,
                "zero-or-one" => conduit_core::ConnectionCardinality::ZeroOrOne,
                "one-or-more" => conduit_core::ConnectionCardinality::OneOrMore,
                "zero-or-more" => conduit_core::ConnectionCardinality::ZeroOrMore,
                _ => return None,
            },
            values: match self.values.as_str() {
                "exactly-one" => conduit_core::ValueCardinality::ExactlyOne,
                "zero-or-one" => conduit_core::ValueCardinality::ZeroOrOne,
                "one-or-more" => conduit_core::ValueCardinality::OneOrMore,
                "zero-or-more" => conduit_core::ValueCardinality::ZeroOrMore,
                _ => return None,
            },
            delivery: match self.delivery.as_str() {
                "stream" => conduit_core::Delivery::Stream,
                "latest-state" => conduit_core::Delivery::LatestState,
                "finite-batch" => conduit_core::Delivery::FiniteBatch,
                "artifact-reference" => conduit_core::Delivery::ArtifactReference,
                "control" => conduit_core::Delivery::Control,
                _ => return None,
            },
            temporal: match self.temporal.as_str() {
                "progressive" => conduit_core::TemporalContract::Progressive,
                "committed" => conduit_core::TemporalContract::Committed,
                "retained-state" => conduit_core::TemporalContract::RetainedState,
                "atemporal" => conduit_core::TemporalContract::Atemporal,
                _ => return None,
            },
            terminal: match self.terminal.as_str() {
                "finite" => conduit_core::TerminalContract::Finite,
                "open-ended" => conduit_core::TerminalContract::OpenEnded,
                "either" => conduit_core::TerminalContract::Either,
                _ => return None,
            },
            sensitivity: match self.sensitivity.as_str() {
                "public" => Sensitivity::Public,
                "restricted" => Sensitivity::Restricted,
                "secret" => Sensitivity::Secret,
                _ => return None,
            },
            loss: match self.loss.as_str() {
                "lossless-only" => conduit_core::LossAcceptance::LosslessOnly,
                "type-contract-defined" => conduit_core::LossAcceptance::TypeContractDefined,
                _ => return None,
            },
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PoolBindingDocument {
    pub pool_semantic_hash: String,
    pub admission_policy: PinDocument,
    pub supervision_policy: PinDocument,
    pub per_instance_budget: BudgetDocument,
    pub authority_grants: Vec<String>,
    pub maximum_instance_ticks: u64,
    pub implementation_set_hash: String,
    pub correlation_slots: u16,
    pub worst_case_budget: BudgetDocument,
    pub child_nodes: u16,
    pub child_cords: u16,
    /// Host-resolved runtime reservation facts. Required for current pool
    /// plans; absent only in pre-runtime inputs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<PoolRuntimeBindingDocument>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PoolReservationDocument {
    pub resources: BudgetDocument,
    pub child_nodes: u16,
    pub child_cords: u16,
    pub state_bytes: u64,
    pub scheduler_slots: u16,
    pub host_operations: u16,
    pub cancellation_scopes: u16,
}

impl From<PoolReservationDocument> for PoolReservationProfile {
    fn from(value: PoolReservationDocument) -> Self {
        Self {
            resources: value.resources.into(),
            child_nodes: value.child_nodes,
            child_cords: value.child_cords,
            state_bytes: value.state_bytes,
            scheduler_slots: value.scheduler_slots,
            host_operations: value.host_operations,
            cancellation_scopes: value.cancellation_scopes,
        }
    }
}

impl From<PoolReservationProfile> for PoolReservationDocument {
    fn from(value: PoolReservationProfile) -> Self {
        Self {
            resources: value.resources.into(),
            child_nodes: value.child_nodes,
            child_cords: value.child_cords,
            state_bytes: value.state_bytes,
            scheduler_slots: value.scheduler_slots,
            host_operations: value.host_operations,
            cancellation_scopes: value.cancellation_scopes,
        }
    }
}

fn pool_runtime_mirrored_profile(pool: &PoolBindingDocument) -> PoolReservationDocument {
    let runtime = pool
        .runtime
        .as_ref()
        .expect("caller checked runtime profile");
    PoolReservationDocument {
        resources: pool.per_instance_budget,
        child_nodes: pool.child_nodes,
        child_cords: pool.child_cords,
        state_bytes: runtime.per_instance.state_bytes,
        scheduler_slots: runtime.per_instance.scheduler_slots,
        host_operations: runtime.per_instance.host_operations,
        cancellation_scopes: runtime.per_instance.cancellation_scopes,
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PoolRuntimeBindingDocument {
    /// Exact conversion from current source milliseconds to the selected
    /// monotonic plan time basis.
    pub ticks_per_millisecond: u32,
    pub cleanup_ticks: u64,
    pub maximum_evidence_events: u16,
    pub fallback_target: Option<String>,
    pub per_instance: PoolReservationDocument,
    pub queued: PoolReservationDocument,
    pub candidate_maximum_live: u16,
    pub rollback_maximum_live: u16,
    pub generation_reserved: PoolReservationDocument,
    pub total_reserved: PoolReservationDocument,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanPoolRuntimeDocument {
    pub admission: String,
    pub supervision: String,
    pub maximum_attempts: u16,
    pub backoff_ticks: u64,
    pub fallback_target: Option<String>,
    pub cleanup: String,
    pub deadline_ticks: u64,
    pub idle_timeout_ticks: u64,
    pub cleanup_ticks: u64,
    pub maximum_evidence_events: u16,
    pub per_instance: PoolReservationDocument,
    pub queued: PoolReservationDocument,
    pub candidate_maximum_live: u16,
    pub rollback_maximum_live: u16,
    pub generation_reserved_slots: u16,
    pub generation_reserved: PoolReservationDocument,
    pub total_reserved: PoolReservationDocument,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisionLimitsDocument {
    pub maximum_observations: u16,
    pub maximum_decisions: u16,
    pub maximum_in_flight: u16,
    pub maximum_cause_depth: u16,
    pub maximum_nested_depth: u8,
    pub maximum_handler_ticks: u64,
    pub maximum_recovery_ticks: u64,
    pub restart_window_ticks: u64,
    pub backoff_ticks: u64,
    pub cooldown_ticks: u64,
    pub operator_wait_ticks: u64,
    pub maximum_evidence_events: u16,
    pub observation_bytes: u32,
    pub decision_bytes: u32,
    pub scratch_bytes: u32,
}

impl From<SupervisionLimitsDocument> for SupervisionLimits {
    fn from(value: SupervisionLimitsDocument) -> Self {
        Self {
            maximum_observations: value.maximum_observations,
            maximum_decisions: value.maximum_decisions,
            maximum_in_flight: value.maximum_in_flight,
            maximum_cause_depth: value.maximum_cause_depth,
            maximum_nested_depth: value.maximum_nested_depth,
            maximum_handler_ticks: value.maximum_handler_ticks,
            maximum_recovery_ticks: value.maximum_recovery_ticks,
            restart_window_ticks: value.restart_window_ticks,
            backoff_ticks: value.backoff_ticks,
            cooldown_ticks: value.cooldown_ticks,
            operator_wait_ticks: value.operator_wait_ticks,
            maximum_evidence_events: value.maximum_evidence_events,
            observation_bytes: value.observation_bytes,
            decision_bytes: value.decision_bytes,
            scratch_bytes: value.scratch_bytes,
        }
    }
}

impl From<SupervisionLimits> for SupervisionLimitsDocument {
    fn from(value: SupervisionLimits) -> Self {
        Self {
            maximum_observations: value.maximum_observations,
            maximum_decisions: value.maximum_decisions,
            maximum_in_flight: value.maximum_in_flight,
            maximum_cause_depth: value.maximum_cause_depth,
            maximum_nested_depth: value.maximum_nested_depth,
            maximum_handler_ticks: value.maximum_handler_ticks,
            maximum_recovery_ticks: value.maximum_recovery_ticks,
            restart_window_ticks: value.restart_window_ticks,
            backoff_ticks: value.backoff_ticks,
            cooldown_ticks: value.cooldown_ticks,
            operator_wait_ticks: value.operator_wait_ticks,
            maximum_evidence_events: value.maximum_evidence_events,
            observation_bytes: value.observation_bytes,
            decision_bytes: value.decision_bytes,
            scratch_bytes: value.scratch_bytes,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisionActionDocument {
    pub kind: String,
    pub target: Option<String>,
    pub maximum_uses: u16,
    pub permits_effect_replay: bool,
    pub preserves_required_guarantees: bool,
    pub requires_new_epoch: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisionTargetDocument {
    pub choice: String,
    pub target: String,
}

/// Exact planner input for one expanded source supervision binding.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisionBindingDocument {
    pub instance: String,
    pub source_binding_hash: String,
    pub id: String,
    pub scope: String,
    pub subject: String,
    pub handler: String,
    pub members: Vec<String>,
    pub failure_mode: String,
    pub outer: Option<String>,
    pub policy: PinDocument,
    pub observation_contract: PinDocument,
    pub decision_contract: PinDocument,
    pub actions: Vec<SupervisionActionDocument>,
    pub action_targets: Vec<SupervisionTargetDocument>,
    pub limits: SupervisionLimitsDocument,
    pub allocation: BudgetDocument,
    pub deadline_timer: String,
    pub backoff_timer: String,
    pub cooldown_timer: String,
    pub cleanup: String,
    pub required_behavior: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProviderRiskTraitsDocument {
    pub enrollment_issuer: bool,
    pub unrestricted_native_execution: bool,
    pub remote_artifact_installation: bool,
    pub firmware_mutation: bool,
    pub unrestricted_network: bool,
    pub realm_root_administration: bool,
    pub remote_plan_activation: bool,
    pub actuating_effects: bool,
}

impl From<ProviderRiskTraitsDocument> for ProviderRiskTraits {
    fn from(value: ProviderRiskTraitsDocument) -> Self {
        Self {
            enrollment_issuer: value.enrollment_issuer,
            unrestricted_native_execution: value.unrestricted_native_execution,
            remote_artifact_installation: value.remote_artifact_installation,
            firmware_mutation: value.firmware_mutation,
            unrestricted_network: value.unrestricted_network,
            realm_root_administration: value.realm_root_administration,
            remote_plan_activation: value.remote_plan_activation,
            actuating_effects: value.actuating_effects,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistributionProviderDocument {
    pub provider: PinDocument,
    pub artifact: Option<String>,
    pub availability: String,
    pub traits: ProviderRiskTraitsDocument,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderRequirementDocument {
    pub provider: PinDocument,
    pub traits: ProviderRiskTraitsDocument,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceDistributionDocument {
    pub schema: String,
    pub schema_version: u32,
    pub identity: String,
    pub descriptor: PinDocument,
    pub kind: String,
    pub genesis_profile: String,
    pub control_recorder: PinDocument,
    pub provider_enablement_effect_class: PinDocument,
    pub provider_enablement_operation: PinDocument,
    pub providers: Vec<DistributionProviderDocument>,
    pub maximum_provider_enablement_ticks: u64,
    pub maximum_provider_install_attempts: u16,
    pub maximum_evidence_events: u32,
    #[serde(default)]
    pub requirements: Vec<ProviderRequirementDocument>,
}

impl ReferenceDistributionDocument {
    pub fn seal(&mut self) -> Result<(), CompileError> {
        seal_distribution(self)
    }

    pub fn validate(&self) -> Result<(), CompileError> {
        validate_distribution_document(self)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateDocument {
    pub implementation: ImplementationDocument,
    pub execution_profile: ExecutionProfileDocument,
    pub artifacts: Vec<ArtifactDocument>,
    pub host_report: HostReportDocument,
    pub allocation: BudgetDocument,
    pub lifecycle_policy: PinDocument,
    #[serde(default)]
    pub capabilities: Vec<CapabilityRequirementDocument>,
    #[serde(default)]
    pub resources: Vec<ResourceRequirementDocument>,
    #[serde(default)]
    pub topology: Vec<TopologyRequirementDocument>,
    #[serde(default)]
    pub granted_authorities: Vec<String>,
    #[serde(default)]
    pub authorities: Vec<AuthorityDecisionDocument>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompileInput {
    pub schema: String,
    pub schema_version: u16,
    pub identity: String,
    pub entry_uri: String,
    pub selected_root: Option<String>,
    pub source_limits: CompileSourceLimits,
    pub modules: Vec<CompileModuleDocument>,
    pub catalog: CompileCatalogDocument,
    #[serde(default)]
    pub pool_bindings: Vec<PoolBindingDocument>,
    #[serde(default)]
    pub supervision_bindings: Vec<SupervisionBindingDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hazard_closure: Option<HazardClosureDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distribution: Option<ReferenceDistributionDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_provider: Option<EvidenceProviderBindingDocument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub watch_admissions: Vec<WatchAdmissionDocument>,
    pub source_semantic_hash: String,
    pub resolver: PinDocument,
    pub resolver_policy_hash: String,
    pub time_basis: String,
    pub current_tick: u64,
    pub plan_budget: BudgetDocument,
    pub execution_arrangement: ExecutionArrangementPolicyDocument,
    pub maximum_authority_bindings: u32,
    pub maximum_transition_memory_bytes: u64,
    pub maximum_search_states: usize,
    pub tie_policy: String,
    pub required_realm: Option<String>,
    #[serde(default)]
    pub trusted_entities: Vec<String>,
    #[serde(default)]
    pub trusted_status_reporters: Vec<String>,
    pub require_active_passport: bool,
    #[serde(default)]
    pub implementation_preference: Vec<String>,
    pub candidates: Vec<CandidateDocument>,
}

#[derive(Serialize)]
struct CompileIdentityProjection<'a> {
    schema: &'a str,
    schema_version: u16,
    entry_uri: &'a str,
    selected_root: &'a Option<String>,
    source_limits: CompileSourceLimits,
    modules: &'a [CompileModuleDocument],
    catalog: &'a CompileCatalogDocument,
    pool_bindings: &'a [PoolBindingDocument],
    supervision_bindings: &'a [SupervisionBindingDocument],
    hazard_closure: &'a Option<HazardClosureDocument>,
    distribution: &'a Option<ReferenceDistributionDocument>,
    evidence_provider: &'a Option<EvidenceProviderBindingDocument>,
    watch_admissions: &'a [WatchAdmissionDocument],
    source_semantic_hash: &'a str,
    resolver: &'a PinDocument,
    resolver_policy_hash: &'a str,
    time_basis: &'a str,
    current_tick: u64,
    plan_budget: BudgetDocument,
    execution_arrangement: &'a ExecutionArrangementPolicyDocument,
    maximum_authority_bindings: u32,
    maximum_transition_memory_bytes: u64,
    maximum_search_states: usize,
    tie_policy: &'a str,
    required_realm: &'a Option<String>,
    trusted_entities: &'a [String],
    trusted_status_reporters: &'a [String],
    require_active_passport: bool,
    implementation_preference: &'a [String],
    candidates: &'a [CandidateDocument],
}

impl CompileInput {
    pub fn seal(&mut self) -> Result<(), CompileError> {
        self.validate_source_limits()?;
        self.validate_module_source_limits()?;
        canonicalize_compile_input(self);
        self.catalog.identity = catalog_identity(&self.catalog)?;
        for module in &mut self.modules {
            module.content_hash = content_hash(&module.source);
        }
        self.source_semantic_hash =
            lower_compile_source(&resolve_source_graph(self)?, &self.catalog)?
                .semantic_hash
                .to_string();
        for candidate in &mut self.candidates {
            seal_execution_profile(&mut candidate.execution_profile)?;
            candidate.implementation.execution_profile = PinDocument {
                id: candidate.execution_profile.id.clone(),
                schema_version: candidate.execution_profile.schema_version,
                semantic_hash: candidate.execution_profile.semantic_hash.clone(),
            };
            for authority in &mut candidate.authorities {
                seal_authority_decision(authority)?;
            }
            candidate.implementation.required_authorities.extend(
                candidate
                    .authorities
                    .iter()
                    .map(|authority| authority.requirement.clone()),
            );
            candidate.implementation.required_authorities.sort();
            candidate.implementation.required_authorities.dedup();
            candidate.implementation.required_effects.extend(
                candidate
                    .authorities
                    .iter()
                    .map(|authority| authority.effect_hash.clone()),
            );
            candidate.implementation.required_effects.sort();
            candidate.implementation.required_effects.dedup();
            candidate.granted_authorities = candidate
                .authorities
                .iter()
                .filter(|authority| authority.status == "active")
                .map(|authority| authority.requirement.clone())
                .collect();
            for artifact in &mut candidate.artifacts {
                artifact.identity = artifact_identity(artifact)?;
            }
            candidate.implementation.identity = implementation_identity(&candidate.implementation)?;
            candidate.host_report.identity = report_identity(&candidate.host_report)?;
        }
        if let Some(closure) = &mut self.hazard_closure {
            seal_hazard_closure(closure)?;
        }
        if let Some(distribution) = &mut self.distribution {
            seal_distribution(distribution)?;
        }
        self.resolver_policy_hash = policy_hash(self)?;
        self.identity = self.computed_identity()?;
        self.validate()
    }

    pub fn computed_identity(&self) -> Result<String, CompileError> {
        let mut canonical = self.clone();
        canonicalize_compile_input(&mut canonical);
        let bytes = serde_json::to_vec(&CompileIdentityProjection {
            schema: &canonical.schema,
            schema_version: canonical.schema_version,
            entry_uri: &canonical.entry_uri,
            selected_root: &canonical.selected_root,
            source_limits: canonical.source_limits,
            modules: &canonical.modules,
            catalog: &canonical.catalog,
            pool_bindings: &canonical.pool_bindings,
            supervision_bindings: &canonical.supervision_bindings,
            hazard_closure: &canonical.hazard_closure,
            distribution: &canonical.distribution,
            evidence_provider: &canonical.evidence_provider,
            watch_admissions: &canonical.watch_admissions,
            source_semantic_hash: &canonical.source_semantic_hash,
            resolver: &canonical.resolver,
            resolver_policy_hash: &canonical.resolver_policy_hash,
            time_basis: &canonical.time_basis,
            current_tick: canonical.current_tick,
            plan_budget: canonical.plan_budget,
            execution_arrangement: &canonical.execution_arrangement,
            maximum_authority_bindings: canonical.maximum_authority_bindings,
            maximum_transition_memory_bytes: canonical.maximum_transition_memory_bytes,
            maximum_search_states: canonical.maximum_search_states,
            tie_policy: &canonical.tie_policy,
            required_realm: &canonical.required_realm,
            trusted_entities: &canonical.trusted_entities,
            trusted_status_reporters: &canonical.trusted_status_reporters,
            require_active_passport: canonical.require_active_passport,
            implementation_preference: &canonical.implementation_preference,
            candidates: &canonical.candidates,
        })
        .map_err(|_| CompileError::new(CompileReason::InvalidInput))?;
        Ok(format!("sha256:{}", hex(&Sha256::digest(bytes))))
    }

    pub fn validate(&self) -> Result<(), CompileError> {
        if self.schema != COMPILE_INPUT_SCHEMA
            || self.schema_version != COMPILE_INPUT_SCHEMA_VERSION
        {
            return Err(CompileError::new(CompileReason::UnsupportedInput));
        }
        self.validate_source_limits()?;
        if self.candidates.is_empty() || self.candidates.len() > 4096 {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        if self.execution_arrangement.plan_epoch == 0
            || self.execution_arrangement.maximum_proposal_bytes == 0
            || self.execution_arrangement.maximum_head_of_line_ticks == 0
            || self.execution_arrangement.cancellation_slots == 0
            || self.execution_arrangement.evidence_slots == 0
        {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        pin(&self.execution_arrangement.boundary_realization)?;
        self.validate_module_source_limits()?;
        if self.pool_bindings.len() > 4096 {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        if self.supervision_bindings.len() > 4096 {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        for supervision in &self.supervision_bindings {
            instance(&supervision.instance)?;
            parse_hash(&supervision.source_binding_hash)?;
            id(&supervision.id)?;
            instance(&supervision.subject)?;
            instance(&supervision.handler)?;
            supervision
                .members
                .iter()
                .map(|member| instance(member))
                .collect::<Result<Vec<_>, _>>()?;
            supervision_failure_mode(&supervision.failure_mode)?;
            supervision.outer.as_deref().map(id).transpose()?;
            pin(&supervision.policy)?;
            pin(&supervision.observation_contract)?;
            pin(&supervision.decision_contract)?;
            id(&supervision.deadline_timer)?;
            id(&supervision.backoff_timer)?;
            id(&supervision.cooldown_timer)?;
            let limits: SupervisionLimits = supervision.limits.into();
            limits
                .validate()
                .map_err(|_| CompileError::new(CompileReason::BudgetInvalid))?;
            for action in &supervision.actions {
                supervision_action(action)?;
            }
            for target in &supervision.action_targets {
                id(&target.choice)?;
                instance(&target.target)?;
            }
        }
        for pool in &self.pool_bindings {
            parse_hash(&pool.pool_semantic_hash)?;
            pin(&pool.admission_policy)?;
            pin(&pool.supervision_policy)?;
            parse_hash(&pool.implementation_set_hash)?;
            pool.authority_grants
                .iter()
                .map(|grant| id(grant))
                .collect::<Result<Vec<_>, _>>()?;
            let runtime = pool
                .runtime
                .as_ref()
                .ok_or_else(|| CompileError::new(CompileReason::BudgetInvalid))?;
            runtime
                .fallback_target
                .as_deref()
                .map(instance)
                .transpose()?;
            if runtime.ticks_per_millisecond == 0
                || runtime.cleanup_ticks == 0
                || runtime.maximum_evidence_events == 0
                || runtime.per_instance != pool_runtime_mirrored_profile(pool)
                || runtime.total_reserved.resources != pool.worst_case_budget
            {
                return Err(CompileError::new(CompileReason::BudgetInvalid));
            }
        }
        if self
            .modules
            .iter()
            .any(|module| module.content_hash != content_hash(&module.source))
        {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        resolve_source_graph(self)?;
        validate_catalog(&self.catalog)?;
        if self.maximum_authority_bindings > 4096
            || self.maximum_search_states == 0
            || self.maximum_search_states > 1_000_000
        {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        parse_hash(&self.source_semantic_hash)?;
        parse_hash(&self.resolver_policy_hash)?;
        pin(&self.resolver)?;
        Id::new(&self.time_basis).map_err(|_| CompileError::new(CompileReason::InvalidInput))?;
        self.required_realm.as_deref().map(id).transpose()?;
        self.trusted_entities
            .iter()
            .map(|entity| id(entity))
            .collect::<Result<Vec<_>, _>>()?;
        self.trusted_status_reporters
            .iter()
            .map(|reporter| parse_hash(reporter))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(closure) = &self.hazard_closure {
            let arena = Bump::new();
            let policy = hazard_closure_policy(&closure.policy, &arena)?;
            let _flows = closure
                .flows
                .iter()
                .map(effect_flow_binding)
                .collect::<Result<Vec<_>, _>>()?;
            let permits = closure
                .permits
                .iter()
                .map(|permit| {
                    hazard_permit(
                        permit,
                        &arena,
                        AuthorityTime {
                            basis: id(&self.time_basis)?,
                            tick: self.current_tick,
                        },
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let subject = parse_hash(&closure.plan_subject)?;
            parse_hash(&closure.decision_identity)?;
            if closure.epoch == 0
                || permits.iter().any(|permit| {
                    permit.policy_identity != policy.identity
                        || permit.plan_subject != subject
                        || permit.epoch != closure.epoch
                })
            {
                return Err(CompileError::new(CompileReason::HazardClosure(
                    HazardClosureReason::PermitScopeMismatch,
                )));
            }
        }
        if let Some(distribution) = &self.distribution {
            validate_distribution_document(distribution)?;
        }
        if self.identity != self.computed_identity()? {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        if policy_hash(self)? != self.resolver_policy_hash {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        Ok(())
    }

    /// Validate only the caller-selected source limits before reading the
    /// separate entry source or parsing any module source.
    pub fn validate_source_limits(&self) -> Result<(), CompileError> {
        if self.schema != COMPILE_INPUT_SCHEMA
            || self.schema_version != COMPILE_INPUT_SCHEMA_VERSION
        {
            return Err(CompileError::new(CompileReason::UnsupportedInput));
        }
        let limits = self.source_limits;
        if limits.maximum_entry_source_bytes == 0
            || limits.maximum_entry_source_bytes > MAXIMUM_COMPILE_ENTRY_SOURCE_BYTES
            || limits.maximum_module_source_bytes == 0
            || limits.maximum_module_source_bytes > MAXIMUM_COMPILE_MODULE_SOURCE_BYTES
            || limits.maximum_module_closure_bytes == 0
            || limits.maximum_module_closure_bytes > MAXIMUM_COMPILE_MODULE_CLOSURE_BYTES
            || limits.maximum_modules == 0
            || limits.maximum_modules > MAXIMUM_COMPILE_MODULES
            || limits.maximum_entry_source_bytes > limits.maximum_module_source_bytes
            || limits.maximum_module_source_bytes > limits.maximum_module_closure_bytes
        {
            return Err(CompileError::new(CompileReason::SourceLimitExceeded));
        }
        Ok(())
    }

    fn validate_module_source_limits(&self) -> Result<(), CompileError> {
        if self.modules.is_empty()
            || self.modules.len() > usize::from(self.source_limits.maximum_modules)
        {
            return Err(CompileError::new(CompileReason::SourceLimitExceeded));
        }
        let aggregate_module_bytes = self.modules.iter().try_fold(0_u64, |total, module| {
            u64::try_from(module.source.len())
                .ok()
                .and_then(|bytes| total.checked_add(bytes))
                .ok_or_else(|| CompileError::new(CompileReason::SourceLimitExceeded))
        })?;
        if aggregate_module_bytes > self.source_limits.maximum_module_closure_bytes
            || self.modules.iter().any(|module| {
                u64::try_from(module.source.len()).map_or(true, |bytes| {
                    bytes > self.source_limits.maximum_module_source_bytes
                        || (module.canonical_uri == self.entry_uri
                            && bytes > self.source_limits.maximum_entry_source_bytes)
                })
            })
        {
            return Err(CompileError::new(CompileReason::SourceLimitExceeded));
        }
        Ok(())
    }
}

struct ExplicitModuleLoader<'a> {
    modules: &'a [CompileModuleDocument],
}

impl ModuleLoader for ExplicitModuleLoader<'_> {
    fn load(&self, canonical_uri: &str) -> Result<Option<LoadedModule>, String> {
        Ok(self
            .modules
            .iter()
            .find(|module| module.canonical_uri == canonical_uri)
            .map(|module| LoadedModule {
                canonical_uri: module.canonical_uri.clone(),
                source: module.source.clone(),
            }))
    }
}

#[derive(Serialize)]
struct CatalogIdentityProjection<'a> {
    nodes: &'a [PinDocument],
    types: &'a [PinDocument],
    ports: &'a [PinDocument],
    external_leaf_contracts: &'a [ExternalLeafContractDocument],
}

/// Returns the finite built-in semantic catalog accepted by the reference
/// compiler. The returned identity pins the exact provider snapshot.
pub fn builtin_catalog_document() -> Result<CompileCatalogDocument, CompileError> {
    let registry = Registry::default();
    let mut catalog = CompileCatalogDocument {
        identity: String::new(),
        nodes: [
            "std/literal",
            "std/format-values/literal",
            "std/text/format",
            "std/text/lines",
            "std/text/join",
            "std/record/literal",
            "std/data/validate-closed-record",
            "std/testing/assert-validation-decision",
            "std/data/encode-utf8",
            "std/data/decode-utf8",
            "std/data/frame-length-u32be",
            "std/data/deframe-length-u32be",
            "time/delay",
            "time/timeout",
            "time/debounce",
            "time/throttle",
            "state/cell",
            "state/deduplicate",
            "state/cache",
            "io/stdout",
            "display/text",
            "text/uppercase",
            "supervision/supervisor",
            "net/http/listen",
            "fs/chunk/literal",
            "fs/read",
            "fs/write",
            "fs/watch",
        ]
        .into_iter()
        .map(|id| {
            let schema = registry
                .node_schema(id)
                .ok_or_else(|| CompileError::new(CompileReason::InvalidInput))?;
            Ok(PinDocument {
                id: id.to_owned(),
                schema_version: 0,
                semantic_hash: schema.semantic_hash().to_string(),
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?,
        types: [
            "std/text",
            "std/bytes",
            "std/record",
            "std/validation-decision",
            "std/reference/any",
            "std/format-values",
            "std/list/text",
            "std/u64",
            "std/bool",
            "std/terminal",
            "supervision/decision",
            "fs/resource",
            "fs/chunk",
            "fs/read-result",
            "fs/write-result",
            "fs/event",
        ]
        .into_iter()
        .map(|id| {
            let reference = registry
                .type_reference(id)
                .ok_or_else(|| CompileError::new(CompileReason::InvalidInput))?;
            Ok(PinDocument {
                id: id.to_owned(),
                schema_version: reference.schema_version,
                semantic_hash: reference.semantic_hash.to_string(),
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?,
        ports: [
            "conduit/input-text",
            "conduit/output-text",
            "terminal",
            "decision",
            "chunk",
        ]
        .into_iter()
        .map(|id| {
            let reference = registry
                .port_contract(id)
                .ok_or_else(|| CompileError::new(CompileReason::InvalidInput))?;
            Ok(PinDocument {
                id: id.to_owned(),
                schema_version: 0,
                semantic_hash: reference.semantic_hash.to_string(),
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?,
        external_leaf_contracts: Vec::new(),
        interfaces: ["conduit/stream-sink", "conduit/text-processor"]
            .into_iter()
            .map(|id| {
                let contract = registry
                    .interface_contract(id)
                    .ok_or_else(|| CompileError::new(CompileReason::InvalidInput))?;
                Ok(PinDocument {
                    id: id.to_owned(),
                    schema_version: 0,
                    semantic_hash: contract.semantic_hash.to_string(),
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?,
    };
    canonicalize_catalog(&mut catalog);
    catalog.identity = catalog_identity(&catalog)?;
    validate_catalog(&catalog)?;
    Ok(catalog)
}

struct PinnedCatalog<'a> {
    document: &'a CompileCatalogDocument,
    registry: Registry,
}

impl<'a> PinnedCatalog<'a> {
    fn new(document: &'a CompileCatalogDocument) -> Result<Self, CompileError> {
        validate_catalog(document)?;
        Ok(Self {
            document,
            registry: Registry::default(),
        })
    }

    fn exact_pin<'b>(&'b self, pins: &'b [PinDocument], id: &str) -> Option<&'b PinDocument> {
        pins.iter().find(|pin| pin.id == id)
    }

    fn external_contract(&self, id: &str) -> Option<OwnedNodeContract> {
        self.document
            .external_leaf_contracts
            .iter()
            .find(|contract| contract.id == id)?
            .to_owned()
    }

    fn external_type_reference(&self, id: &str) -> Option<OwnedTypeReference> {
        let used = self
            .document
            .external_leaf_contracts
            .iter()
            .any(|contract| {
                contract
                    .config
                    .iter()
                    .any(|field| field.value_type.id == id)
                    || contract
                        .inputs
                        .iter()
                        .chain(&contract.outputs)
                        .any(|port| port.value_type.id == id)
            });
        let pin = used.then(|| self.exact_pin(&self.document.types, id))??;
        Some(OwnedTypeReference {
            id: pin.id.clone(),
            schema_version: pin.schema_version,
            semantic_hash: parse_hash(&pin.semantic_hash).ok()?,
        })
    }
}

impl SourceContractCatalog for PinnedCatalog<'_> {
    fn node_schema(&self, id: &str) -> Option<OwnedNodeSchema> {
        let pin = self.exact_pin(&self.document.nodes, id)?;
        let schema = self.registry.node_schema(id).or_else(|| {
            self.document
                .external_leaf_contracts
                .iter()
                .find(|contract| contract.id == id)?
                .to_schema()
        })?;
        (pin.schema_version == 0 && pin.semantic_hash == schema.semantic_hash().to_string())
            .then_some(schema)
    }

    fn node_contract(&self, id: &str) -> Option<OwnedNodeContract> {
        let pin = self.exact_pin(&self.document.nodes, id)?;
        if let Some(contract) = self.external_contract(id) {
            let schema = self
                .document
                .external_leaf_contracts
                .iter()
                .find(|document| document.id == id)?
                .to_schema()?;
            return (pin.schema_version == 0
                && pin.semantic_hash == schema.semantic_hash().to_string())
            .then_some(contract);
        }
        let contract = self.registry.node_contract(id)?;
        (pin.schema_version == 0 && pin.semantic_hash == contract.semantic_hash().to_string())
            .then_some(contract)
    }

    fn interface_contract(&self, id: &str) -> Option<OwnedInterfaceContract> {
        let pin = self.exact_pin(&self.document.interfaces, id)?;
        let contract = self.registry.interface_contract(id)?;
        (pin.schema_version == 0 && pin.semantic_hash == contract.semantic_hash.to_string())
            .then_some(contract)
    }

    fn type_reference(&self, id: &str) -> Option<OwnedTypeReference> {
        let pin = self.exact_pin(&self.document.types, id)?;
        let reference = self
            .registry
            .type_reference(id)
            .or_else(|| self.external_type_reference(id))?;
        (pin.schema_version == reference.schema_version
            && pin.semantic_hash == reference.semantic_hash.to_string())
        .then_some(reference)
    }

    fn port_contract(&self, id: &str) -> Option<OwnedPortReference> {
        let pin = self.exact_pin(&self.document.ports, id)?;
        let reference = self.registry.port_contract(id)?;
        (pin.schema_version == 0 && pin.semantic_hash == reference.semantic_hash.to_string())
            .then_some(reference)
    }

    fn validate_literal(
        &self,
        expected: &OwnedTypeReference,
        source: &conduit_panel::SourceValue,
    ) -> Result<OwnedSemanticValue, LiteralValidationError> {
        if self.type_reference(&expected.id).as_ref() != Some(expected) {
            return Err(LiteralValidationError::ProviderUnavailable);
        }
        self.registry.validate_literal(expected, source)
    }

    fn validate_default(
        &self,
        expected: &OwnedTypeReference,
        value: &OwnedSemanticValue,
    ) -> Result<(), LiteralValidationError> {
        if self.type_reference(&expected.id).as_ref() != Some(expected) {
            return Err(LiteralValidationError::ProviderUnavailable);
        }
        self.registry.validate_default(expected, value)
    }
}

fn validate_catalog(catalog: &CompileCatalogDocument) -> Result<(), CompileError> {
    if catalog.nodes.is_empty()
        || catalog.types.is_empty()
        || catalog.nodes.len() > 4096
        || catalog.types.len() > 4096
        || catalog.ports.len() > 4096
        || catalog.external_leaf_contracts.len() > 4096
    {
        return Err(CompileError::new(CompileReason::InvalidInput));
    }
    if catalog.identity != catalog_identity(catalog)? {
        return Err(CompileError::new(CompileReason::InvalidInput));
    }
    let registry = Registry::default();
    let mut external_ids = BTreeSet::new();
    let mut external_types = BTreeSet::new();
    for contract in &catalog.external_leaf_contracts {
        if !custom_leaf_id(&contract.id)
            || registry.node_schema(&contract.id).is_some()
            || !external_ids.insert(contract.id.as_str())
            || contract.to_owned().is_none()
            || contract.to_schema().is_none()
            || !catalog.nodes.iter().any(|pin| pin.id == contract.id)
            || contract.inputs.iter().any(|port| port.direction != "input")
            || contract
                .outputs
                .iter()
                .any(|port| port.direction != "output")
        {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        for value_type in contract.config.iter().map(|field| &field.value_type).chain(
            contract
                .inputs
                .iter()
                .chain(&contract.outputs)
                .map(|port| &port.value_type),
        ) {
            external_types.insert(value_type.id.as_str());
            if !catalog.types.iter().any(|pin| pin == value_type) {
                return Err(CompileError::new(CompileReason::InvalidInput));
            }
        }
    }
    let mut ids = BTreeSet::new();
    for pin in &catalog.nodes {
        let external = external_ids.contains(pin.id.as_str());
        let schema = registry.node_schema(&pin.id).or_else(|| {
            external
                .then(|| {
                    catalog
                        .external_leaf_contracts
                        .iter()
                        .find(|contract| contract.id == pin.id)
                        .and_then(ExternalLeafContractDocument::to_schema)
                })
                .flatten()
        });
        if !ids.insert(pin.id.as_str())
            || pin.schema_version != 0
            || schema.is_none_or(|schema| schema.semantic_hash().to_string() != pin.semantic_hash)
        {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
    }
    ids.clear();
    for pin in &catalog.types {
        if !ids.insert(pin.id.as_str())
            || registry.type_reference(&pin.id).map_or_else(
                || {
                    !external_types.contains(pin.id.as_str())
                        || parse_hash(&pin.semantic_hash).is_err()
                },
                |reference| {
                    reference.schema_version != pin.schema_version
                        || reference.semantic_hash.to_string() != pin.semantic_hash
                },
            )
        {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
    }
    ids.clear();
    for pin in &catalog.ports {
        if !ids.insert(pin.id.as_str())
            || pin.schema_version != 0
            || registry
                .port_contract(&pin.id)
                .is_none_or(|reference| reference.semantic_hash.to_string() != pin.semantic_hash)
        {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
    }
    Ok(())
}

fn custom_leaf_id(id: &str) -> bool {
    id.contains('/') && Id::new(id).is_ok()
}

fn catalog_identity(catalog: &CompileCatalogDocument) -> Result<String, CompileError> {
    let mut canonical = catalog.clone();
    canonicalize_catalog(&mut canonical);
    let bytes = serde_json::to_vec(&CatalogIdentityProjection {
        nodes: &canonical.nodes,
        types: &canonical.types,
        ports: &canonical.ports,
        external_leaf_contracts: &canonical.external_leaf_contracts,
    })
    .map_err(|_| CompileError::new(CompileReason::InvalidInput))?;
    Ok(format!("sha256:{}", hex(&Sha256::digest(bytes))))
}

fn canonicalize_catalog(catalog: &mut CompileCatalogDocument) {
    catalog.nodes.sort_by(|left, right| left.id.cmp(&right.id));
    catalog.types.sort_by(|left, right| left.id.cmp(&right.id));
    catalog.ports.sort_by(|left, right| left.id.cmp(&right.id));
    catalog
        .external_leaf_contracts
        .sort_by(|left, right| left.id.cmp(&right.id));
}

fn resolve_source_graph(input: &CompileInput) -> Result<ModuleGraph, CompileError> {
    let loader = ExplicitModuleLoader {
        modules: &input.modules,
    };
    let graph =
        conduit_panel::resolve_modules(&input.entry_uri, input.selected_root.as_deref(), &loader)
            .map_err(|_| CompileError::new(CompileReason::SourceInvalid))?;
    if graph.modules.len() != input.modules.len()
        || graph.modules.iter().any(|resolved| {
            !input.modules.iter().any(|module| {
                module.canonical_uri == resolved.canonical_uri
                    && module.content_hash == resolved.content_hash
                    && module.source == resolved.source
            })
        })
    {
        return Err(CompileError::new(CompileReason::SourceInvalid));
    }
    Ok(graph)
}

struct CompileLoweredTopologyBase {
    topology: conduit_runtime::LoweredTopology,
    supervisions: Vec<conduit_runtime::LoweredSupervision>,
    semantic_hash: SemanticHash,
}

fn lower_compile_source(
    graph: &ModuleGraph,
    catalog: &CompileCatalogDocument,
) -> Result<CompileLoweredTopologyBase, CompileError> {
    let catalog = PinnedCatalog::new(catalog)?;
    let lowered = lower_source(graph, &catalog)
        .map_err(|_| CompileError::new(CompileReason::LoweringFailed))?;
    Ok(CompileLoweredTopologyBase {
        topology: *lowered.supervised.topology,
        supervisions: lowered.supervised.supervisions,
        semantic_hash: lowered.semantic_hash,
    })
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanHostDocument {
    pub id: String,
    pub host: String,
    pub boot_id: String,
    pub semantic_hash: String,
    pub time_basis: String,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanArtifactDocument {
    pub id: String,
    pub digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanResourceDocument {
    pub id: String,
    pub node: String,
    pub kind: String,
    pub resource: String,
    pub host_observation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease: Option<ResourceLeaseDocument>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceProviderBindingDocument {
    pub implementation: PinDocument,
    pub artifact: PlanArtifactDocument,
    pub host_observation: PlanHostDocument,
    pub store_kind: String,
    pub store_id: String,
    pub store_generation: u64,
    pub grant_hash: String,
    pub time_basis: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceLeaseDocument {
    pub schema_version: u32,
    pub id: String,
    pub resource_binding: String,
    pub holder: String,
    pub run: String,
    pub epoch: u64,
    pub scope: String,
    pub sharing: String,
    pub maximum_holders: u16,
    pub reservation: BudgetDocument,
    pub time_basis: String,
    pub issued_at_tick: u64,
    pub expires_at_tick: u64,
    pub revocation_grace_ticks: u64,
    pub cleanup_ticks: u64,
    pub maximum_operations: u32,
    pub maximum_evidence_events: u32,
    pub cleanup_escalation: PinDocument,
    pub foreign_retention: String,
    pub foreign_maximum_bytes: u64,
    pub foreign_release_ticks: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkloadBudgetDocument {
    pub work_units: Option<u64>,
    pub tasks: Option<u64>,
    pub processes: Option<u64>,
    pub descriptors: Option<u64>,
    pub connections: Option<u64>,
    pub storage_bytes: Option<u64>,
    pub device_operations: Option<u64>,
    pub network_bytes: Option<u64>,
    pub callbacks: Option<u64>,
    pub foreign_queue_items: Option<u64>,
    pub transition_overlap_work_units: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeadlineContractDocument {
    pub time_basis: String,
    pub relative_deadline_ticks: u64,
    pub maximum_jitter_ticks: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkloadContractDocument {
    pub schema_version: u32,
    pub id: String,
    pub service: String,
    pub node: String,
    pub guarantee: String,
    pub budget: WorkloadBudgetDocument,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<DeadlineContractDocument>,
    pub maximum_evidence_events: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkloadCapabilityDocument {
    pub id: String,
    pub identity: String,
    pub host_observation: String,
    pub evidence_kind: String,
    pub time_basis: String,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub capacity: WorkloadBudgetDocument,
    pub maximum_deadline_ticks: u64,
    pub maximum_jitter_ticks: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanWorkloadDocument {
    pub contract: WorkloadContractDocument,
    pub capability: WorkloadCapabilityDocument,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanNodeDocument {
    pub instance: String,
    pub contract: PinDocument,
    pub implementation: PinDocument,
    pub lifecycle_policy: PinDocument,
    pub execution_profile: ExecutionProfileDocument,
    pub artifact: String,
    pub host_observation: String,
    pub host: String,
    pub allocation: BudgetDocument,
    pub required_resources: Vec<String>,
    pub required_effects: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PressureDocument {
    BlockFifo,
    Reject,
    Coalesce { relation: String },
    Sample { every: u32, offset: u32 },
    DropDisposable,
    Disconnect,
    Fail,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanPortDocument {
    pub node: String,
    pub port: String,
    pub direction: String,
    pub port_contract_hash: String,
    pub value_type_id: String,
    pub value_type_schema_version: u32,
    pub value_type_semantic_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanCordDocument {
    pub id: String,
    pub from: PlanPortDocument,
    pub to: PlanPortDocument,
    pub capacity_items: u16,
    pub max_value_bytes: u32,
    pub max_queued_bytes: u64,
    pub low_watermark_items: u16,
    pub high_watermark_items: u16,
    pub pressure: PressureDocument,
    pub queue_memory_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValueEnvelopePolicyDocument {
    pub cord: String,
    pub representation: PinDocument,
    pub maximum_payload_bytes: u32,
    pub maximum_envelope_bytes: u32,
    pub maximum_fragments: u16,
    pub maximum_fragment_bytes: u32,
    pub maximum_timestamps: u8,
    pub clock_domains: Vec<String>,
    pub identity_allowed: bool,
    pub correlation_allowed: bool,
    pub causation_allowed: bool,
    pub provenance_allowed: bool,
    pub sensitivity_ceiling: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WatchAdmissionDocument {
    pub id: String,
    pub subject_kind: String,
    pub operator: String,
    pub control_grant_hash: String,
    pub lease: String,
    pub cord: Option<String>,
    pub node: Option<String>,
    pub port: Option<String>,
    pub direction: Option<String>,
    pub representation: PinDocument,
    pub maximum_preview_bytes: u32,
    pub maximum_history: u16,
    pub minimum_tick_interval: u64,
    pub retention: String,
    pub sensitivity_ceiling: String,
    pub reveal_action: Option<String>,
    pub reveal_grant_hash: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClockConversionDocument {
    pub id: String,
    pub source: String,
    pub destination: String,
    pub numerator: u64,
    pub denominator: u64,
    pub offset_ticks: i64,
    pub rounding: String,
    pub maximum_uncertainty_ticks: u64,
    pub observed_time_basis: String,
    pub observed_tick: u64,
    pub valid_until_tick: u64,
    pub authority: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackBoundaryDocument {
    pub id: String,
    pub node: String,
    pub cord: String,
    pub kind: String,
    pub initialization: String,
    pub initial_items: u16,
    pub initial_bytes: u64,
    pub maximum_retained_items: u16,
    pub maximum_retained_bytes: u64,
    pub delay_ticks: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<String>,
    pub replay_gap: String,
    pub cancellation: PinDocument,
    pub terminal: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanExportDocument {
    pub boundary_port: String,
    pub member: String,
    pub member_port: String,
    pub direction: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanCompositeDocument {
    pub instance: String,
    pub definition_hash: String,
    pub members: Vec<String>,
    pub exports: Vec<PlanExportDocument>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanAuthorityBindingDocument {
    pub effect_id: String,
    pub capability_id: String,
    pub grant_id: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub host: String,
    pub audit_id: String,
    pub time_basis: String,
    pub validated_at_tick: u64,
    pub check_at_use: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanAuthorityDocument {
    pub node: String,
    pub effect_hash: String,
    pub grant_hash: String,
    pub effect: EffectRequirementDocument,
    pub capability: HostCapabilityDocument,
    pub grant: AuthorityGrantDocument,
    pub binding: PlanAuthorityBindingDocument,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub administrative_subject: Option<AdministrativeSubjectDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containment: Option<AdministrativeProofDocument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_budgets: Vec<PolicyBudgetBindingDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_profile: Option<EffectCommitProfileDocument>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectCommitProfileDocument {
    pub schema_version: u32,
    pub id: String,
    pub operation: String,
    pub resource_lease: String,
    pub commit_boundary: PinDocument,
    pub idempotency: String,
    pub unknown_commit: String,
    pub discontinuity: String,
    pub cleanup: PinDocument,
    pub maximum_attempts: u16,
    pub evidence_events_per_attempt: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanPortGroupMemberDocument {
    pub id: String,
    pub ordinal: u16,
    pub port_contract_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanPortGroupDocument {
    pub instance: String,
    pub template_hash: String,
    pub maximum: u16,
    pub direction: String,
    pub members: Vec<PlanPortGroupMemberDocument>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanInstancePoolDocument {
    pub instance: String,
    pub template_hash: String,
    pub derived_identity_hash: String,
    pub maximum_live: u16,
    pub maximum_queued: u16,
    pub admission_policy: PinDocument,
    pub supervision_policy: PinDocument,
    pub per_instance_budget: BudgetDocument,
    pub authority_grants: Vec<String>,
    pub maximum_instance_ticks: u64,
    pub implementation_set_hash: String,
    pub correlation_slots: u16,
    pub worst_case_budget: BudgetDocument,
    pub child_nodes: u16,
    pub child_cords: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<PlanPoolRuntimeDocument>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedExecutionPlacementDocument {
    pub id: String,
    pub host_observation: String,
    pub provider: PinDocument,
    pub authority_boundary: PinDocument,
    pub resource_boundary: PinDocument,
    pub lifecycle_boundary: PinDocument,
    pub failure_boundary: PinDocument,
    pub generation: u64,
    pub isolation: String,
    pub memory_containment: String,
    pub regain_control: String,
    pub effect_fencing: String,
    pub stop_execution: String,
    pub reclaim_resources: String,
    pub maximum_regain_control_ticks: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedExecutionLaneDocument {
    pub id: String,
    pub placement: String,
    pub placement_generation: u64,
    pub generation: u64,
    pub independent_progress: String,
    pub simultaneous_execution: String,
    pub preemption: String,
    pub termination: String,
    pub ready_slots: u16,
    pub wake_slots: u16,
    pub proposal_slots: u16,
    pub commit_slots: u16,
    pub timer_slots: u16,
    pub scratch_bytes: u32,
    pub stack_bytes: u32,
    pub evidence_slots: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedExecutionRegionDocument {
    pub id: String,
    pub members: Vec<String>,
    pub placement: String,
    pub placement_generation: u64,
    pub lane: String,
    pub lane_generation: u64,
    pub commit_domain: String,
    pub independent: bool,
    pub maximum_in_flight_proposals: u16,
    pub scratch_bytes: u32,
    pub retained_state_bytes: u64,
    pub pending_operation_slots: u16,
    pub timer_slots: u16,
    pub evidence_slots: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedExecutionBoundaryDocument {
    pub cord: String,
    pub from_region: String,
    pub to_region: String,
    pub realization: PinDocument,
    pub generation: u64,
    pub from_placement_generation: u64,
    pub to_placement_generation: u64,
    pub capacity_items: u16,
    pub capacity_bytes: u64,
    pub wake_slots: u16,
    pub evidence_slots: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedExecutionCommitDomainDocument {
    pub id: String,
    pub ordering: String,
    pub proposal_slots: u16,
    pub commit_slots: u16,
    pub maximum_proposal_bytes: u64,
    pub maximum_head_of_line_ticks: u64,
    pub cancellation_slots: u16,
    pub evidence_slots: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedExecutionArrangementDocument {
    pub identity: String,
    pub plan_identity: String,
    pub resolution_identity: String,
    pub plan_epoch: u64,
    pub placements: Vec<ResolvedExecutionPlacementDocument>,
    pub lanes: Vec<ResolvedExecutionLaneDocument>,
    pub regions: Vec<ResolvedExecutionRegionDocument>,
    pub boundaries: Vec<ResolvedExecutionBoundaryDocument>,
    pub commit_domains: Vec<ResolvedExecutionCommitDomainDocument>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExactPlanDocument {
    pub schema: String,
    pub schema_version: u32,
    pub identity: String,
    pub source_semantic_hash: String,
    pub resolver: PinDocument,
    pub resolver_policy_hash: String,
    pub time_basis: String,
    pub created_at_tick: u64,
    pub budget: BudgetDocument,
    /// Separately identified physical arrangement for this exact logical plan.
    pub execution_arrangement: ResolvedExecutionArrangementDocument,
    pub host_observations: Vec<PlanHostDocument>,
    pub resources: Vec<PlanResourceDocument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workloads: Vec<PlanWorkloadDocument>,
    pub artifacts: Vec<PlanArtifactDocument>,
    pub nodes: Vec<PlanNodeDocument>,
    pub cords: Vec<PlanCordDocument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub value_envelopes: Vec<ValueEnvelopePolicyDocument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub watch_admissions: Vec<WatchAdmissionDocument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clock_conversions: Vec<ClockConversionDocument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub feedback_boundaries: Vec<FeedbackBoundaryDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_provider: Option<EvidenceProviderBindingDocument>,
    pub authorities: Vec<PlanAuthorityDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hazard_closure: Option<HazardClosureDocument>,
    pub composites: Vec<PlanCompositeDocument>,
    pub port_groups: Vec<PlanPortGroupDocument>,
    pub instance_pools: Vec<PlanInstancePoolDocument>,
    #[serde(default)]
    pub supervisions: Vec<SupervisionBindingDocument>,
    pub unresolved_selectors: Vec<String>,
}

impl ExactPlanDocument {
    pub fn validate(&self) -> Result<(), CompileError> {
        let arena = Bump::new();
        let plan = self.as_plan(&arena)?;
        validate_hosted_execution_plan(
            &plan,
            PlanValidationContext {
                supported_schema_version: self.schema_version,
                now: AuthorityTime {
                    basis: Id(&self.time_basis),
                    tick: self.created_at_tick,
                },
            },
        )
        .map_err(|_| CompileError::new(CompileReason::PlanInvalid))?;
        self.execution_arrangement()?
            .validate_for_plan(&plan)
            .map_err(|_| CompileError::new(CompileReason::ExecutionArrangement))
    }

    /// Decode the separately identified physical arrangement without
    /// rediscovering host providers or changing the logical plan.
    pub fn execution_arrangement(&self) -> Result<ResolvedExecutionArrangement, CompileError> {
        let document = &self.execution_arrangement;
        Ok(ResolvedExecutionArrangement {
            identity: parse_hash(&document.identity)?,
            plan_identity: parse_hash(&document.plan_identity)?,
            resolution_identity: parse_hash(&document.resolution_identity)?,
            plan_epoch: document.plan_epoch,
            placements: document
                .placements
                .iter()
                .map(|placement| {
                    Ok(ResolvedExecutionPlacement {
                        id: placement.id.clone(),
                        host_observation: placement.host_observation.clone(),
                        provider: resolved_execution_descriptor(&placement.provider)?,
                        authority_boundary: resolved_execution_descriptor(
                            &placement.authority_boundary,
                        )?,
                        resource_boundary: resolved_execution_descriptor(
                            &placement.resource_boundary,
                        )?,
                        lifecycle_boundary: resolved_execution_descriptor(
                            &placement.lifecycle_boundary,
                        )?,
                        failure_boundary: resolved_execution_descriptor(
                            &placement.failure_boundary,
                        )?,
                        generation: placement.generation,
                        isolation: isolation_profile(&placement.isolation)?,
                        memory_containment: execution_guarantee(&placement.memory_containment)?,
                        regain_control: execution_guarantee(&placement.regain_control)?,
                        effect_fencing: execution_guarantee(&placement.effect_fencing)?,
                        stop_execution: execution_guarantee(&placement.stop_execution)?,
                        reclaim_resources: execution_guarantee(&placement.reclaim_resources)?,
                        maximum_regain_control_ticks: placement.maximum_regain_control_ticks,
                    })
                })
                .collect::<Result<Vec<_>, CompileError>>()?,
            lanes: document
                .lanes
                .iter()
                .map(|lane| {
                    Ok(ResolvedExecutionLane {
                        id: lane.id.clone(),
                        placement: lane.placement.clone(),
                        placement_generation: lane.placement_generation,
                        generation: lane.generation,
                        independent_progress: execution_guarantee(&lane.independent_progress)?,
                        simultaneous_execution: execution_guarantee(&lane.simultaneous_execution)?,
                        preemption: execution_guarantee(&lane.preemption)?,
                        termination: execution_guarantee(&lane.termination)?,
                        ready_slots: lane.ready_slots,
                        wake_slots: lane.wake_slots,
                        proposal_slots: lane.proposal_slots,
                        commit_slots: lane.commit_slots,
                        timer_slots: lane.timer_slots,
                        scratch_bytes: lane.scratch_bytes,
                        stack_bytes: lane.stack_bytes,
                        evidence_slots: lane.evidence_slots,
                    })
                })
                .collect::<Result<Vec<_>, CompileError>>()?,
            regions: document
                .regions
                .iter()
                .map(|region| ResolvedExecutionRegion {
                    id: region.id.clone(),
                    members: region.members.clone(),
                    placement: region.placement.clone(),
                    placement_generation: region.placement_generation,
                    lane: region.lane.clone(),
                    lane_generation: region.lane_generation,
                    commit_domain: region.commit_domain.clone(),
                    independent: region.independent,
                    maximum_in_flight_proposals: region.maximum_in_flight_proposals,
                    scratch_bytes: region.scratch_bytes,
                    retained_state_bytes: region.retained_state_bytes,
                    pending_operation_slots: region.pending_operation_slots,
                    timer_slots: region.timer_slots,
                    evidence_slots: region.evidence_slots,
                })
                .collect(),
            boundaries: document
                .boundaries
                .iter()
                .map(|boundary| {
                    Ok(ResolvedExecutionBoundary {
                        cord: boundary.cord.clone(),
                        from_region: boundary.from_region.clone(),
                        to_region: boundary.to_region.clone(),
                        realization: resolved_execution_descriptor(&boundary.realization)?,
                        generation: boundary.generation,
                        from_placement_generation: boundary.from_placement_generation,
                        to_placement_generation: boundary.to_placement_generation,
                        capacity_items: boundary.capacity_items,
                        capacity_bytes: boundary.capacity_bytes,
                        wake_slots: boundary.wake_slots,
                        evidence_slots: boundary.evidence_slots,
                    })
                })
                .collect::<Result<Vec<_>, CompileError>>()?,
            commit_domains: document
                .commit_domains
                .iter()
                .map(|domain| {
                    let ordering = match domain.ordering.as_str() {
                        "deterministic-frontier" => CommitOrdering::DeterministicFrontier,
                        "independent-frontier" => CommitOrdering::IndependentFrontier,
                        _ => return Err(CompileError::new(CompileReason::InvalidInput)),
                    };
                    Ok(ResolvedExecutionCommitDomain {
                        id: domain.id.clone(),
                        ordering,
                        proposal_slots: domain.proposal_slots,
                        commit_slots: domain.commit_slots,
                        maximum_proposal_bytes: domain.maximum_proposal_bytes,
                        maximum_head_of_line_ticks: domain.maximum_head_of_line_ticks,
                        cancellation_slots: domain.cancellation_slots,
                        evidence_slots: domain.evidence_slots,
                    })
                })
                .collect::<Result<Vec<_>, CompileError>>()?,
        })
    }

    /// Compute the exact policy/permit subject for this plan's resolved
    /// authority facts and caller-declared stage transfers.
    pub fn effect_closure_subject(
        &self,
        epoch: u64,
        flows: &[EffectFlowBindingDocument],
    ) -> Result<String, CompileError> {
        let arena = Bump::new();
        let plan = self.as_plan(&arena)?;
        let flows = flows
            .iter()
            .map(effect_flow_binding)
            .collect::<Result<Vec<_>, _>>()?;
        conduit_core::effect_closure_subject(plan.authorities, &flows, epoch, plan.created_at.basis)
            .map(|identity| identity.to_string())
            .map_err(|_| {
                CompileError::new(CompileReason::HazardClosure(
                    HazardClosureReason::IdentityMismatch,
                ))
            })
    }

    /// Borrow this sealed document as the exact portable execution plan.
    ///
    /// The caller owns the arena for the complete borrow. This conversion
    /// validates document/schema structure but does not resolve, select,
    /// fetch, provision, or synthesize any binding.
    pub fn as_plan<'a>(&'a self, arena: &'a Bump) -> Result<ExecutionPlan<'a>, CompileError> {
        let supported_document = self.schema == PLAN_DOCUMENT_SCHEMA
            && self.schema_version == EXECUTION_PLAN_SCHEMA_VERSION;
        if !supported_document || !self.unresolved_selectors.is_empty() {
            return Err(CompileError::new(CompileReason::PlanInvalid));
        }
        let hosts = self
            .host_observations
            .iter()
            .map(|host| {
                Ok(PlanHostObservation {
                    id: id(&host.id)?,
                    host: id(&host.host)?,
                    boot_id: id(&host.boot_id)?,
                    semantic_hash: parse_hash(&host.semantic_hash)?,
                    time_basis: id(&host.time_basis)?,
                    observed_at_tick: host.observed_at_tick,
                    valid_until_tick: host.valid_until_tick,
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let artifacts = self
            .artifacts
            .iter()
            .map(|artifact| {
                Ok(PlanArtifact {
                    id: id(&artifact.id)?,
                    digest: parse_digest(&artifact.digest)?,
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let evidence_provider = self
            .evidence_provider
            .as_ref()
            .map(|provider| {
                if !self.artifacts.contains(&provider.artifact)
                    || !self.host_observations.contains(&provider.host_observation)
                {
                    return Err(CompileError::new(CompileReason::PlanInvalid));
                }
                Ok(PlanEvidenceProviderBinding {
                    implementation: pin(&provider.implementation)?,
                    artifact: id(&provider.artifact.id)?,
                    host_observation: id(&provider.host_observation.id)?,
                    store: ResourceRef {
                        kind: id(&provider.store_kind)?,
                        id: id(&provider.store_id)?,
                    },
                    store_generation: provider.store_generation,
                    grant_hash: parse_hash(&provider.grant_hash)?,
                    time_basis: id(&provider.time_basis)?,
                })
            })
            .transpose()?;
        let resources = self
            .resources
            .iter()
            .map(|resource| {
                Ok(PlanResourceBinding {
                    id: id(&resource.id)?,
                    node: instance(&resource.node)?,
                    resource: ResourceRef {
                        kind: id(&resource.kind)?,
                        id: id(&resource.resource)?,
                    },
                    host_observation: id(&resource.host_observation)?,
                    lease: resource.lease.as_ref().map(resource_lease).transpose()?,
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let workloads = self
            .workloads
            .iter()
            .map(workload_binding)
            .collect::<Result<Vec<_>, CompileError>>()?;
        let nodes = self
            .nodes
            .iter()
            .map(|node| {
                let required_resources = node
                    .required_resources
                    .iter()
                    .map(|resource| id(resource))
                    .collect::<Result<Vec<_>, _>>()?;
                let required_effects = node
                    .required_effects
                    .iter()
                    .map(|effect| parse_hash(effect))
                    .collect::<Result<Vec<_>, _>>()?;
                let profile = execution_profile(&node.execution_profile, arena)?;
                Ok(ResolvedPlanNode {
                    instance: instance(&node.instance)?,
                    contract: pin(&node.contract)?,
                    implementation: pin(&node.implementation)?,
                    lifecycle_policy: pin(&node.lifecycle_policy)?,
                    execution_profile: Some(arena.alloc(profile)),
                    artifact: id(&node.artifact)?,
                    host_observation: id(&node.host_observation)?,
                    host: id(&node.host)?,
                    allocation: node.allocation.into(),
                    required_resources: arena.alloc_slice_copy(&required_resources),
                    required_effects: arena.alloc_slice_copy(&required_effects),
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let cords = self
            .cords
            .iter()
            .map(|cord| {
                Ok(ResolvedPlanCord {
                    id: id(&cord.id)?,
                    from: port_document(&cord.from)?,
                    to: port_document(&cord.to)?,
                    flow: flow_document(cord)?,
                    queue_memory_bytes: cord.queue_memory_bytes,
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let value_envelopes = self
            .value_envelopes
            .iter()
            .map(|policy| {
                let clock_domains = policy
                    .clock_domains
                    .iter()
                    .map(|clock| id(clock))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ValueEnvelopePolicy {
                    cord: id(&policy.cord)?,
                    representation: pin(&policy.representation)?,
                    maximum_payload_bytes: policy.maximum_payload_bytes,
                    maximum_envelope_bytes: policy.maximum_envelope_bytes,
                    maximum_fragments: policy.maximum_fragments,
                    maximum_fragment_bytes: policy.maximum_fragment_bytes,
                    maximum_timestamps: policy.maximum_timestamps,
                    clock_domains: arena.alloc_slice_copy(&clock_domains),
                    identity_allowed: policy.identity_allowed,
                    correlation_allowed: policy.correlation_allowed,
                    causation_allowed: policy.causation_allowed,
                    provenance_allowed: policy.provenance_allowed,
                    sensitivity_ceiling: sensitivity(&policy.sensitivity_ceiling)?,
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let watch_admissions =
            self.watch_admissions
                .iter()
                .map(|watch| {
                    let subject =
                        match watch.subject_kind.as_str() {
                            "cord" => WatchSubject::Cord(id(watch
                                .cord
                                .as_deref()
                                .ok_or_else(|| CompileError::new(CompileReason::PlanInvalid))?)?),
                            "node-port" => WatchSubject::NodePort {
                                node: instance(watch.node.as_deref().ok_or_else(|| {
                                    CompileError::new(CompileReason::PlanInvalid)
                                })?)?,
                                port: id(watch.port.as_deref().ok_or_else(|| {
                                    CompileError::new(CompileReason::PlanInvalid)
                                })?)?,
                                direction: direction(watch.direction.as_deref().ok_or_else(
                                    || CompileError::new(CompileReason::PlanInvalid),
                                )?)?,
                            },
                            _ => return Err(CompileError::new(CompileReason::PlanInvalid)),
                        };
                    let retention = match watch.retention.as_str() {
                        "latest" => WatchRetention::Latest,
                        "ring" => WatchRetention::Ring,
                        "sample" => WatchRetention::Sample,
                        _ => return Err(CompileError::new(CompileReason::PlanInvalid)),
                    };
                    Ok(WatchAdmission {
                        id: id(&watch.id)?,
                        subject,
                        operator: id(&watch.operator)?,
                        control_grant_hash: parse_hash(&watch.control_grant_hash)?,
                        lease: id(&watch.lease)?,
                        representation: pin(&watch.representation)?,
                        maximum_preview_bytes: watch.maximum_preview_bytes,
                        maximum_history: watch.maximum_history,
                        minimum_tick_interval: watch.minimum_tick_interval,
                        retention,
                        sensitivity_ceiling: sensitivity(&watch.sensitivity_ceiling)?,
                        reveal_action: watch.reveal_action.as_deref().map(id).transpose()?,
                        reveal_grant_hash: watch
                            .reveal_grant_hash
                            .as_deref()
                            .map(parse_hash)
                            .transpose()?,
                    })
                })
                .collect::<Result<Vec<_>, CompileError>>()?;
        let clock_conversions = self
            .clock_conversions
            .iter()
            .map(|conversion| {
                Ok(PlanClockConversion {
                    id: id(&conversion.id)?,
                    source: id(&conversion.source)?,
                    destination: id(&conversion.destination)?,
                    numerator: conversion.numerator,
                    denominator: conversion.denominator,
                    offset_ticks: conversion.offset_ticks,
                    rounding: clock_rounding(&conversion.rounding)?,
                    maximum_uncertainty_ticks: conversion.maximum_uncertainty_ticks,
                    observed_at: AuthorityTime {
                        basis: id(&conversion.observed_time_basis)?,
                        tick: conversion.observed_tick,
                    },
                    valid_until_tick: conversion.valid_until_tick,
                    authority: id(&conversion.authority)?,
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let feedback_boundaries = self
            .feedback_boundaries
            .iter()
            .map(|boundary| {
                Ok(PlanFeedbackBoundary {
                    id: id(&boundary.id)?,
                    node: instance(&boundary.node)?,
                    cord: id(&boundary.cord)?,
                    kind: feedback_kind(&boundary.kind)?,
                    initialization: feedback_initialization(&boundary.initialization)?,
                    initial_items: boundary.initial_items,
                    initial_bytes: boundary.initial_bytes,
                    maximum_retained_items: boundary.maximum_retained_items,
                    maximum_retained_bytes: boundary.maximum_retained_bytes,
                    delay_ticks: boundary.delay_ticks,
                    clock: boundary.clock.as_deref().map(id).transpose()?,
                    replay_gap: feedback_replay_gap(&boundary.replay_gap)?,
                    cancellation: pin(&boundary.cancellation)?,
                    terminal: feedback_terminal(&boundary.terminal)?,
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let composites = self
            .composites
            .iter()
            .map(|composite| {
                let members = composite
                    .members
                    .iter()
                    .map(|member| instance(member))
                    .collect::<Result<Vec<_>, _>>()?;
                let exports = composite
                    .exports
                    .iter()
                    .map(|export| {
                        Ok(PlanExportBinding {
                            boundary_port: id(&export.boundary_port)?,
                            member: instance(&export.member)?,
                            member_port: id(&export.member_port)?,
                            direction: direction(&export.direction)?,
                        })
                    })
                    .collect::<Result<Vec<_>, CompileError>>()?;
                Ok(PlanCompositeMapping {
                    instance: instance(&composite.instance)?,
                    definition_hash: parse_hash(&composite.definition_hash)?,
                    members: arena.alloc_slice_copy(&members),
                    exports: arena.alloc_slice_copy(&exports),
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let authorities = self
            .authorities
            .iter()
            .map(|authority| {
                let policy_budgets = authority
                    .policy_budgets
                    .iter()
                    .map(policy_budget_binding)
                    .collect::<Result<Vec<_>, CompileError>>()?;
                Ok(PlanAuthority {
                    node: instance(&authority.node)?,
                    effect_hash: parse_hash(&authority.effect_hash)?,
                    grant_hash: parse_hash(&authority.grant_hash)?,
                    effect: effect_requirement(&authority.effect, arena)?,
                    capability: host_capability(&authority.capability)?,
                    grant: authority_grant(&authority.grant, arena)?,
                    binding: ResolvedAuthorityBinding {
                        effect_id: id(&authority.binding.effect_id)?,
                        capability_id: id(&authority.binding.capability_id)?,
                        grant_id: id(&authority.binding.grant_id)?,
                        resource: ResourceRef {
                            kind: id(&authority.binding.resource_kind)?,
                            id: id(&authority.binding.resource_id)?,
                        },
                        host: id(&authority.binding.host)?,
                        audit_id: id(&authority.binding.audit_id)?,
                        time_basis: id(&authority.binding.time_basis)?,
                        validated_at_tick: authority.binding.validated_at_tick,
                        check_at_use: authority.binding.check_at_use,
                    },
                    administrative_subject: authority
                        .administrative_subject
                        .as_ref()
                        .map(administrative_subject)
                        .transpose()?,
                    containment: match (
                        authority.administrative_subject.as_ref(),
                        authority.containment.as_ref(),
                    ) {
                        (Some(subject), Some(proof)) => {
                            let subject = administrative_subject(subject)?;
                            Some(administrative_proof(
                                proof,
                                subject,
                                arena,
                                AuthorityTime {
                                    basis: id(&self.time_basis)?,
                                    tick: self.created_at_tick,
                                },
                            )?)
                        }
                        (None, None) => None,
                        _ => return Err(CompileError::new(CompileReason::PlanInvalid)),
                    },
                    policy_budgets: arena.alloc_slice_copy(&policy_budgets),
                    commit_profile: authority
                        .commit_profile
                        .as_ref()
                        .map(effect_commit_profile)
                        .transpose()?,
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let hazard_closure = self
            .hazard_closure
            .as_ref()
            .map(|closure| {
                plan_hazard_closure(
                    closure,
                    arena.alloc_slice_copy(&authorities),
                    arena,
                    AuthorityTime {
                        basis: id(&self.time_basis)?,
                        tick: self.created_at_tick,
                    },
                )
            })
            .transpose()?;
        let port_groups = self
            .port_groups
            .iter()
            .map(|group| {
                let members = group
                    .members
                    .iter()
                    .map(|member| {
                        Ok(PlanPortGroupMember {
                            id: id(&member.id)?,
                            ordinal: member.ordinal,
                            port_contract_hash: parse_hash(&member.port_contract_hash)?,
                        })
                    })
                    .collect::<Result<Vec<_>, CompileError>>()?;
                Ok(PlanPortGroup {
                    instance: instance(&group.instance)?,
                    template_hash: parse_hash(&group.template_hash)?,
                    maximum: group.maximum,
                    direction: direction(&group.direction)?,
                    members: arena.alloc_slice_copy(&members),
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let instance_pools = self
            .instance_pools
            .iter()
            .map(|pool| {
                let authority_grants = pool
                    .authority_grants
                    .iter()
                    .map(|grant| id(grant))
                    .collect::<Result<Vec<_>, _>>()?;
                let pool_instance = instance(&pool.instance)?;
                let template_hash = parse_hash(&pool.template_hash)?;
                let implementation_set_hash = parse_hash(&pool.implementation_set_hash)?;
                let runtime = pool
                    .runtime
                    .as_ref()
                    .map(|runtime| {
                        plan_pool_runtime(
                            runtime,
                            pool_instance,
                            template_hash,
                            implementation_set_hash,
                            pool.maximum_live,
                            pool.maximum_queued,
                        )
                    })
                    .transpose()?;
                Ok(PlanInstancePool {
                    instance: pool_instance,
                    template_hash,
                    derived_identity_hash: parse_hash(&pool.derived_identity_hash)?,
                    maximum_live: pool.maximum_live,
                    maximum_queued: pool.maximum_queued,
                    admission_policy: pin(&pool.admission_policy)?,
                    supervision_policy: pin(&pool.supervision_policy)?,
                    per_instance_budget: pool.per_instance_budget.into(),
                    authority_grants: arena.alloc_slice_copy(&authority_grants),
                    maximum_instance_ticks: pool.maximum_instance_ticks,
                    implementation_set_hash,
                    correlation_slots: pool.correlation_slots,
                    worst_case_budget: pool.worst_case_budget.into(),
                    child_nodes: pool.child_nodes,
                    child_cords: pool.child_cords,
                    runtime,
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let supervisions = self
            .supervisions
            .iter()
            .map(|supervision| {
                let actions = supervision
                    .actions
                    .iter()
                    .map(supervision_action)
                    .collect::<Result<Vec<_>, CompileError>>()?;
                let action_targets = supervision
                    .action_targets
                    .iter()
                    .map(|target| {
                        Ok(PlanSupervisionTarget {
                            choice: id(&target.choice)?,
                            target: instance(&target.target)?,
                        })
                    })
                    .collect::<Result<Vec<_>, CompileError>>()?;
                let members = supervision
                    .members
                    .iter()
                    .map(|member| instance(member))
                    .collect::<Result<Vec<_>, CompileError>>()?;
                Ok(PlanSupervision {
                    instance: instance(&supervision.instance)?,
                    source_binding_hash: parse_hash(&supervision.source_binding_hash)?,
                    contract: SupervisionContract {
                        schema_version: 0,
                        id: id(&supervision.id)?,
                        scope: supervision_scope(&supervision.scope)?,
                        subject: instance(&supervision.subject)?,
                        handler: instance(&supervision.handler)?,
                        members: arena.alloc_slice_copy(&members),
                        failure_mode: supervision_failure_mode(&supervision.failure_mode)?,
                        outer: supervision.outer.as_deref().map(id).transpose()?,
                        actions: arena.alloc_slice_copy(&actions),
                        limits: supervision.limits.into(),
                        cleanup: stop_policy(&supervision.cleanup)?,
                        required_behavior: supervision.required_behavior,
                    },
                    policy: pin(&supervision.policy)?,
                    observation_contract: pin(&supervision.observation_contract)?,
                    decision_contract: pin(&supervision.decision_contract)?,
                    action_targets: arena.alloc_slice_copy(&action_targets),
                    allocation: supervision.allocation.into(),
                    deadline_timer: id(&supervision.deadline_timer)?,
                    backoff_timer: id(&supervision.backoff_timer)?,
                    cooldown_timer: id(&supervision.cooldown_timer)?,
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        Ok(ExecutionPlan {
            schema_version: self.schema_version,
            identity: parse_hash(&self.identity)?,
            source_semantic_hash: parse_hash(&self.source_semantic_hash)?,
            resolver: pin(&self.resolver)?,
            resolver_policy_hash: parse_hash(&self.resolver_policy_hash)?,
            created_at: AuthorityTime {
                basis: id(&self.time_basis)?,
                tick: self.created_at_tick,
            },
            budget: self.budget.into(),
            host_observations: arena.alloc_slice_copy(&hosts),
            resources: arena.alloc_slice_copy(&resources),
            workloads: arena.alloc_slice_copy(&workloads),
            artifacts: arena.alloc_slice_copy(&artifacts),
            nodes: arena.alloc_slice_copy(&nodes),
            cords: arena.alloc_slice_copy(&cords),
            value_envelopes: arena.alloc_slice_copy(&value_envelopes),
            clock_conversions: arena.alloc_slice_copy(&clock_conversions),
            feedback_boundaries: arena.alloc_slice_copy(&feedback_boundaries),
            distributed_cords: &[],
            fanouts: &[],
            merges: &[],
            event_streams: &[],
            runtime_evidence: None,
            evidence_provider,
            watch_admissions: arena.alloc_slice_copy(&watch_admissions),
            jobs: &[],
            satisfaction_proofs: &[],
            authorities: arena.alloc_slice_copy(&authorities),
            hazard_closure,
            composites: arena.alloc_slice_copy(&composites),
            port_groups: arena.alloc_slice_copy(&port_groups),
            instance_pools: arena.alloc_slice_copy(&instance_pools),
            supervisions: arena.alloc_slice_copy(&supervisions),
            unresolved: &[],
        })
    }
}

pub fn compile_panel(
    panel: &conduit_panel::Panel,
    input: &CompileInput,
) -> Result<ExactPlanDocument, CompileError> {
    input.validate()?;
    let graph = resolve_source_graph(input)?;
    let entry = graph
        .modules
        .iter()
        .find(|module| module.canonical_uri == graph.entry_uri)
        .ok_or_else(|| CompileError::new(CompileReason::SourceInvalid))?;
    if &entry.panel != panel {
        return Err(CompileError::new(CompileReason::SourceInvalid));
    }
    compile_graph(&graph, input)
}

pub fn compile_source(
    source: &str,
    input: &CompileInput,
) -> Result<ExactPlanDocument, CompileError> {
    input.validate()?;
    let graph = resolve_source_graph(input)?;
    let entry = graph
        .modules
        .iter()
        .find(|module| module.canonical_uri == graph.entry_uri)
        .ok_or_else(|| CompileError::new(CompileReason::SourceInvalid))?;
    if entry.source.as_bytes() != source.as_bytes() {
        return Err(CompileError::new(CompileReason::SourceInvalid));
    }
    compile_graph(&graph, input)
}

fn compile_graph(
    graph: &ModuleGraph,
    input: &CompileInput,
) -> Result<ExactPlanDocument, CompileError> {
    let lowered = lower_compile_source(graph, &input.catalog)?;
    if lowered.semantic_hash != parse_hash(&input.source_semantic_hash)? {
        return Err(CompileError::new(CompileReason::InvalidInput));
    }
    let panel = executable_panel(graph, &lowered.supervisions)?;
    let registry = registry_for_catalog(&input.catalog)?;
    let resolved = registry
        .resolve_contracts(&panel)
        .map_err(|_| CompileError::new(CompileReason::SourceInvalid))?;
    let mut topology = resolved
        .exact_topology()
        .map_err(|_| CompileError::new(CompileReason::SourceInvalid))?;
    let entry = graph
        .modules
        .iter()
        .find(|module| module.canonical_uri == graph.entry_uri)
        .ok_or_else(|| CompileError::new(CompileReason::SourceInvalid))?;
    topology.source_semantic_hash = parse_hash(&conduit_panel::semantic_source_hash(&entry.panel))?;
    compile_topology(&topology, &lowered, input)
}

fn registry_for_catalog(catalog: &CompileCatalogDocument) -> Result<Registry, CompileError> {
    let mut registry = Registry::default();
    for pin in &catalog.nodes {
        if registry
            .contracts()
            .any(|contract| contract.id.as_str() == pin.id)
        {
            continue;
        }
        let document: &'static ExternalLeafContractDocument = Box::leak(Box::new(
            catalog
                .external_leaf_contracts
                .iter()
                .find(|contract| contract.id == pin.id)
                .ok_or_else(|| CompileError::new(CompileReason::InvalidInput))?
                .clone(),
        ));
        let owned: &'static OwnedNodeContract = Box::leak(Box::new(
            document
                .to_owned()
                .ok_or_else(|| CompileError::new(CompileReason::InvalidInput))?,
        ));
        let inputs = owned
            .inputs
            .iter()
            .map(OwnedPortContract::to_core)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| CompileError::new(CompileReason::InvalidInput))?;
        let outputs = owned
            .outputs
            .iter()
            .map(OwnedPortContract::to_core)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| CompileError::new(CompileReason::InvalidInput))?;
        let fields = document
            .config
            .iter()
            .map(ExternalConfigFieldDocument::to_core)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| CompileError::new(CompileReason::InvalidInput))?;
        let contract = Box::leak(Box::new(conduit_core::NodeContract {
            id: Id::new(&owned.id).map_err(|_| CompileError::new(CompileReason::InvalidInput))?,
            config: conduit_core::ConfigContract {
                fields: Box::leak(fields.into_boxed_slice()),
            },
            inputs: Box::leak(inputs.into_boxed_slice()),
            outputs: Box::leak(outputs.into_boxed_slice()),
        }));
        let schema = OwnedNodeSchema::from_contract(contract);
        if pin.schema_version != 0 || schema.semantic_hash().to_string() != pin.semantic_hash {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        registry.register_contract_only(contract);
    }
    Ok(registry)
}

fn executable_panel(
    graph: &ModuleGraph,
    lowered_supervisions: &[conduit_runtime::LoweredSupervision],
) -> Result<conduit_panel::Panel, CompileError> {
    let modules = graph
        .modules
        .iter()
        .map(|module| (module.canonical_uri.as_str(), module))
        .collect::<BTreeMap<_, _>>();
    let mut definitions = Vec::new();
    for module in &graph.modules {
        for source in &module.panel.definitions {
            let mut definition = source.clone();
            annotate_supervisions(
                &mut definition.supervisions,
                &format!("{}/definition/{}", module.canonical_uri, source.id),
                lowered_supervisions,
            )?;
            definition.id = compiled_definition_id(module, &source.id)?;
            definition.port_groups.clear();
            definition.pools.clear();
            for node in &mut definition.nodes {
                prepare_source_node(node, module, &modules)?;
            }
            definitions.push(definition);
        }
    }

    let entry = modules
        .get(graph.entry_uri.as_str())
        .copied()
        .ok_or_else(|| CompileError::new(CompileReason::SourceInvalid))?;
    let (mut nodes, cords, supervisions) = match graph.selected_root.as_deref() {
        None => (entry.panel.nodes.clone(), entry.panel.cords.clone(), {
            let mut supervisions = entry.panel.supervisions.clone();
            annotate_supervisions(
                &mut supervisions,
                &entry.canonical_uri,
                lowered_supervisions,
            )?;
            supervisions
        }),
        Some(selected) => {
            if let Some(node) = entry.panel.nodes.iter().find(|node| node.id == selected) {
                (vec![node.clone()], Vec::new(), Vec::new())
            } else if let Some(definition) = entry
                .panel
                .definitions
                .iter()
                .find(|definition| definition.id == selected)
            {
                let root = entry
                    .panel
                    .roots
                    .iter()
                    .find(|root| root.target == selected)
                    .ok_or_else(|| CompileError::new(CompileReason::SourceInvalid))?;
                (
                    vec![conduit_panel::Node {
                        id: "selected".to_owned(),
                        kind: compiled_definition_id(entry, &definition.id)?,
                        constraint: None,
                        constraint_span: None,
                        implements: Vec::new(),
                        config: Vec::new(),
                        expression: None,
                        source_span: root.source_span,
                    }],
                    Vec::new(),
                    Vec::new(),
                )
            } else {
                return Err(CompileError::new(CompileReason::SourceInvalid));
            }
        }
    };
    for node in &mut nodes {
        prepare_source_node(node, entry, &modules)?;
    }
    Ok(conduit_panel::Panel {
        version: entry.panel.version,
        imports: Vec::new(),
        package_imports: Vec::new(),
        interfaces: Vec::new(),
        definitions,
        nodes,
        cords,
        roots: Vec::new(),
        selected_root: None,
        port_groups: Vec::new(),
        pools: Vec::new(),
        supervisions,
    })
}

fn annotate_supervisions(
    bindings: &mut [conduit_panel::SupervisionBinding],
    owner_path: &str,
    lowered: &[conduit_runtime::LoweredSupervision],
) -> Result<(), CompileError> {
    for binding in bindings {
        let path = format!("{owner_path}/supervision/{}", binding.subject);
        let exact = lowered
            .iter()
            .find(|candidate| candidate.path == path)
            .ok_or_else(|| CompileError::new(CompileReason::LoweringFailed))?;
        binding.resolved_identity = Some(exact.semantic_hash.to_string());
    }
    Ok(())
}

fn prepare_source_node(
    node: &mut conduit_panel::Node,
    module: &conduit_panel::ResolvedModule,
    modules: &BTreeMap<&str, &conduit_panel::ResolvedModule>,
) -> Result<(), CompileError> {
    if node
        .constraint
        .as_deref()
        .is_some_and(|constraint| constraint != "ready")
    {
        return Err(CompileError::new(CompileReason::UnresolvedSelector));
    }
    node.constraint = None;
    node.constraint_span = None;
    node.kind = rewrite_node_kind(&node.kind, module, modules)?;
    Ok(())
}

fn rewrite_node_kind(
    kind: &str,
    module: &conduit_panel::ResolvedModule,
    modules: &BTreeMap<&str, &conduit_panel::ResolvedModule>,
) -> Result<String, CompileError> {
    if kind.starts_with("module.h")
        || kind.starts_with("conduit.std/")
        || kind.starts_with("conduit.host/")
    {
        return Ok(kind.to_owned());
    }
    if module
        .panel
        .definitions
        .iter()
        .any(|definition| definition.id == kind)
    {
        return compiled_definition_id(module, kind);
    }
    if let Some((alias, symbol)) = kind.split_once('.') {
        let Some(import) = module.imports.iter().find(|import| import.alias == alias) else {
            return kind
                .contains('/')
                .then(|| kind.to_owned())
                .ok_or_else(|| CompileError::new(CompileReason::LoweringFailed));
        };
        let imported = modules
            .get(import.canonical_uri.as_str())
            .copied()
            .ok_or_else(|| CompileError::new(CompileReason::LoweringFailed))?;
        if !imported
            .panel
            .definitions
            .iter()
            .any(|definition| definition.id == symbol)
        {
            return Err(CompileError::new(CompileReason::LoweringFailed));
        }
        return compiled_definition_id(imported, symbol);
    }
    Ok(kind.to_owned())
}

fn compiled_definition_id(
    module: &conduit_panel::ResolvedModule,
    definition: &str,
) -> Result<String, CompileError> {
    let digest = module
        .content_hash
        .strip_prefix("sha256:")
        .ok_or_else(|| CompileError::new(CompileReason::SourceInvalid))?;
    let definition = definition.replace('/', ".");
    let value = format!("module.h{digest}/{definition}");
    Id::new(&value).map_err(|_| CompileError::new(CompileReason::LoweringFailed))?;
    Ok(value)
}

fn compile_topology(
    topology: &ExactTopologyView,
    lowered: &CompileLoweredTopologyBase,
    input: &CompileInput,
) -> Result<ExactPlanDocument, CompileError> {
    let arena = Bump::new();
    let prepared = input
        .candidates
        .iter()
        .map(|candidate| {
            prepare_candidate(
                candidate,
                &arena,
                AuthorityTime {
                    basis: id(&input.time_basis)?,
                    tick: input.current_tick,
                },
            )
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let policy = resolver_policy(input, &arena)?;
    let mut requests = Vec::with_capacity(topology.nodes.len());
    for node in &topology.nodes {
        let candidates = prepared
            .iter()
            .filter(|candidate| {
                candidate.manifest.semantic_contract.id.as_str() == node.contract_id
                    && candidate.manifest.semantic_contract.semantic_hash == node.contract_hash
            })
            .map(|candidate| candidate.placement)
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Err(CompileError::new(CompileReason::UnresolvedSelector));
        }
        requests.push(PlacementRequest {
            instance: instance(&node.instance)?,
            semantic_contract: PinnedDescriptor {
                id: id(&node.contract_id)?,
                schema_version: 0,
                semantic_hash: node.contract_hash,
            },
            candidates: arena.alloc_slice_copy(&candidates),
        });
    }
    let resolution = resolve_host_placement(&requests, policy)
        .map_err(|_| CompileError::new(CompileReason::ResolutionFailed))?;

    let mut host_observations = Vec::new();
    let mut resource_bindings = Vec::new();
    let mut artifacts = Vec::new();
    let mut nodes = Vec::new();
    let mut plan_authorities = Vec::new();
    let mut transition_memory_bytes = 0_u64;
    let mut seen_hosts = BTreeSet::new();
    let mut seen_artifacts = BTreeSet::new();
    for node in &topology.nodes {
        let binding = resolution
            .bindings
            .iter()
            .find(|binding| binding.instance == node.instance)
            .ok_or_else(|| CompileError::new(CompileReason::PlanInvalid))?;
        let candidate = prepared
            .iter()
            .find(|candidate| {
                candidate.manifest.id.as_str() == binding.implementation_id
                    && candidate.report.id.as_str() == binding.report_id
            })
            .ok_or_else(|| CompileError::new(CompileReason::PlanInvalid))?;
        transition_memory_bytes = transition_memory_bytes
            .checked_add(candidate.manifest.coexistence_memory_bytes)
            .ok_or_else(|| CompileError::new(CompileReason::BudgetInvalid))?;
        if seen_hosts.insert(binding.report_id.as_str()) {
            host_observations.push(PlanHostObservation {
                id: candidate.report.id,
                host: candidate.report.host,
                boot_id: candidate.report.boot_id,
                semantic_hash: candidate.report.identity,
                time_basis: candidate.report.time_basis,
                observed_at_tick: candidate.report.observed_at_tick,
                valid_until_tick: candidate.report.valid_until_tick,
            });
        }
        for artifact in candidate.manifest.artifacts {
            if seen_artifacts.insert(artifact.id.as_str()) {
                artifacts.push(PlanArtifact {
                    id: artifact.id,
                    digest: artifact.digest,
                });
            }
        }
        let primary_artifact = candidate
            .manifest
            .artifacts
            .iter()
            .find(|artifact| artifact.required)
            .ok_or_else(|| CompileError::new(CompileReason::ResolutionFailed))?;
        let mut required_resources = binding
            .resource_ids
            .iter()
            .map(|resource_id| {
                let resource = candidate
                    .report
                    .resources
                    .iter()
                    .find(|resource| resource.resource.id.as_str() == resource_id)
                    .ok_or_else(|| CompileError::new(CompileReason::PlanInvalid))?;
                resource_bindings.push(PlanResourceBinding {
                    id: resource.resource.id,
                    node: instance(&node.instance)?,
                    resource: resource.resource,
                    host_observation: candidate.report.id,
                    lease: None,
                });
                id(resource_id)
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let instance_path = instance(&node.instance)?;
        let selected_authorities = candidate
            .authorities
            .iter()
            .filter(|authority| authority.effect.requester == instance_path)
            .collect::<Vec<_>>();
        if selected_authorities.len() != candidate.authorities.len() {
            return Err(CompileError::new(CompileReason::ResolutionFailed));
        }
        let mut required_effects = Vec::with_capacity(selected_authorities.len());
        for authority in selected_authorities {
            let binding = authority
                .binding
                .ok_or_else(|| CompileError::new(CompileReason::ResolutionFailed))?;
            let resource_index = if let Some(index) = resource_bindings
                .iter()
                .position(|resource| resource.id == authority.capability.resource.id)
            {
                if resource_bindings[index].node != instance_path
                    || resource_bindings[index].resource != authority.capability.resource
                {
                    return Err(CompileError::new(CompileReason::PlanInvalid));
                }
                index
            } else {
                resource_bindings.push(PlanResourceBinding {
                    id: authority.capability.resource.id,
                    node: instance_path,
                    resource: authority.capability.resource,
                    host_observation: candidate.report.id,
                    lease: Some(authority.resource_lease),
                });
                resource_bindings.len() - 1
            };
            if resource_bindings[resource_index].lease.is_some()
                && resource_bindings[resource_index].lease != Some(authority.resource_lease)
            {
                return Err(CompileError::new(CompileReason::PlanInvalid));
            }
            resource_bindings[resource_index].lease = Some(authority.resource_lease);
            if !required_resources.contains(&authority.capability.resource.id) {
                required_resources.push(authority.capability.resource.id);
            }
            required_effects.push(authority.effect_hash);
            plan_authorities.push(PlanAuthority {
                node: instance_path,
                effect_hash: authority.effect_hash,
                grant_hash: authority.grant_hash,
                effect: authority.effect,
                capability: authority.capability,
                grant: authority.grant,
                binding,
                administrative_subject: authority.administrative_subject,
                containment: authority.containment,
                policy_budgets: authority.policy_budgets,
                commit_profile: Some(authority.commit_profile),
            });
        }
        nodes.push(ResolvedPlanNode {
            instance: instance_path,
            contract: candidate.manifest.semantic_contract,
            implementation: PinnedDescriptor {
                id: candidate.manifest.id,
                schema_version: candidate.manifest.schema_version,
                semantic_hash: candidate.manifest.identity,
            },
            lifecycle_policy: pin(&candidate.document.lifecycle_policy)?,
            execution_profile: Some(candidate.profile),
            artifact: primary_artifact.id,
            host_observation: candidate.report.id,
            host: candidate.report.host,
            allocation: candidate.document.allocation.into(),
            required_resources: arena.alloc_slice_copy(&required_resources),
            required_effects: arena.alloc_slice_copy(&required_effects),
        });
    }
    let evidence_provider = input
        .evidence_provider
        .as_ref()
        .map(|provider| {
            let provider_host = PlanHostObservation {
                id: id(&provider.host_observation.id)?,
                host: id(&provider.host_observation.host)?,
                boot_id: id(&provider.host_observation.boot_id)?,
                semantic_hash: parse_hash(&provider.host_observation.semantic_hash)?,
                time_basis: id(&provider.host_observation.time_basis)?,
                observed_at_tick: provider.host_observation.observed_at_tick,
                valid_until_tick: provider.host_observation.valid_until_tick,
            };
            if let Some(existing) = host_observations
                .iter()
                .find(|host| host.id == provider_host.id)
            {
                if existing != &provider_host {
                    return Err(CompileError::new(CompileReason::PlanInvalid));
                }
            } else {
                host_observations.push(provider_host);
            }

            let provider_artifact = PlanArtifact {
                id: id(&provider.artifact.id)?,
                digest: parse_digest(&provider.artifact.digest)?,
            };
            if let Some(existing) = artifacts
                .iter()
                .find(|artifact| artifact.id == provider_artifact.id)
            {
                if existing != &provider_artifact {
                    return Err(CompileError::new(CompileReason::PlanInvalid));
                }
            } else {
                artifacts.push(provider_artifact);
            }

            Ok(PlanEvidenceProviderBinding {
                implementation: pin(&provider.implementation)?,
                artifact: provider_artifact.id,
                host_observation: provider_host.id,
                store: ResourceRef {
                    kind: id(&provider.store_kind)?,
                    id: id(&provider.store_id)?,
                },
                store_generation: provider.store_generation,
                grant_hash: parse_hash(&provider.grant_hash)?,
                time_basis: id(&provider.time_basis)?,
            })
        })
        .transpose()?;
    if plan_authorities.len() > input.maximum_authority_bindings as usize
        || transition_memory_bytes > input.maximum_transition_memory_bytes
    {
        return Err(CompileError::new(CompileReason::BudgetInvalid));
    }
    let cords = topology
        .cords
        .iter()
        .map(|cord| {
            Ok(ResolvedPlanCord {
                id: id(&cord.id)?,
                from: topology_port(&cord.from_node, &cord.from_port)?,
                to: topology_port(&cord.to_node, &cord.to_port)?,
                flow: topology_flow(cord)?,
                queue_memory_bytes: cord.max_queued_bytes,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let cancellation_hash = SemanticHash::from_bytes(
        Sha256::digest(b"conduit.feedback/cancel-drop-retained|0|abort-drop|terminal-drop").into(),
    );
    let feedback_boundaries = topology
        .cords
        .iter()
        .filter(|cord| cord.from_port.temporal == TemporalContract::RetainedState)
        .map(|cord| {
            let boundary_id = arena.alloc_str(&format!("feedback/{}", cord.id));
            Ok(PlanFeedbackBoundary {
                id: id(boundary_id)?,
                node: instance(&cord.from_node)?,
                cord: id(&cord.id)?,
                kind: FeedbackBoundaryKind::State,
                initialization: FeedbackInitialization::Empty,
                initial_items: 0,
                initial_bytes: 0,
                maximum_retained_items: 1,
                maximum_retained_bytes: u64::from(cord.max_value_bytes),
                delay_ticks: 0,
                clock: None,
                replay_gap: FeedbackReplayGapPolicy::Fail,
                cancellation: PinnedDescriptor {
                    id: Id("conduit.feedback/cancel-drop-retained"),
                    schema_version: 0,
                    semantic_hash: cancellation_hash,
                },
                terminal: FeedbackTerminalPolicy::DropRetained,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let composites = topology
        .composites
        .iter()
        .map(|composite| {
            let members = composite
                .members
                .iter()
                .map(|member| instance(member))
                .collect::<Result<Vec<_>, _>>()?;
            let exports = composite
                .exports
                .iter()
                .map(|export| {
                    Ok(PlanExportBinding {
                        boundary_port: id(&export.boundary_port)?,
                        member: instance(&export.member)?,
                        member_port: id(&export.member_port)?,
                        direction: export.direction,
                    })
                })
                .collect::<Result<Vec<_>, CompileError>>()?;
            Ok(PlanCompositeMapping {
                instance: instance(&composite.instance)?,
                definition_hash: composite.definition_hash,
                members: arena.alloc_slice_copy(&members),
                exports: arena.alloc_slice_copy(&exports),
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let mut lowered_groups = BTreeMap::<String, Vec<&conduit_runtime::LoweredGroupPort>>::new();
    for member in &lowered.topology.group_ports {
        lowered_groups
            .entry(member.logical_group_path.clone())
            .or_default()
            .push(member);
    }
    let mut port_groups = Vec::with_capacity(lowered_groups.len());
    for members in lowered_groups.values_mut() {
        members.sort_by_key(|member| member.ordinal);
        let first = members
            .first()
            .ok_or_else(|| CompileError::new(CompileReason::PlanInvalid))?;
        if members.iter().any(|member| {
            member.group_id != first.group_id
                || member.group_maximum != first.group_maximum
                || member.direction != first.direction
                || member.port_contract != first.port_contract
        }) {
            return Err(CompileError::new(CompileReason::PlanInvalid));
        }
        let plan_members = members
            .iter()
            .map(|member| {
                let member_id = arena.alloc_str(&plan_group_member_id(member));
                Ok(PlanPortGroupMember {
                    id: id(member_id)?,
                    ordinal: member.ordinal,
                    port_contract_hash: member.port_contract.semantic_hash,
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let group_path = arena.alloc_str(&plan_group_path(&first.logical_group_path));
        port_groups.push(PlanPortGroup {
            instance: instance(group_path)?,
            template_hash: plan_group_template_hash(first),
            maximum: first.group_maximum,
            direction: match first.direction {
                conduit_panel::ExportDirection::Input => Direction::Input,
                conduit_panel::ExportDirection::Output => Direction::Output,
            },
            members: arena.alloc_slice_copy(&plan_members),
        });
    }
    if lowered.topology.pools.len() != input.pool_bindings.len() {
        return Err(CompileError::new(CompileReason::BudgetInvalid));
    }
    let mut instance_pools = Vec::with_capacity(lowered.topology.pools.len());
    let mut seen_pool_bindings = BTreeSet::new();
    for pool in &lowered.topology.pools {
        let pool_hash = pool.semantic_hash.to_string();
        let binding = input
            .pool_bindings
            .iter()
            .find(|binding| binding.pool_semantic_hash == pool_hash)
            .ok_or_else(|| CompileError::new(CompileReason::BudgetInvalid))?;
        if !seen_pool_bindings.insert(binding.pool_semantic_hash.as_str()) {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        let authority_grants = binding
            .authority_grants
            .iter()
            .map(|grant| id(grant))
            .collect::<Result<Vec<_>, _>>()?;
        let pool_path = arena.alloc_str(&plan_pool_path(&pool.path));
        let runtime_binding = binding
            .runtime
            .as_ref()
            .ok_or_else(|| CompileError::new(CompileReason::BudgetInvalid))?;
        let ticks_per_millisecond = u64::from(runtime_binding.ticks_per_millisecond);
        if ticks_per_millisecond == 0 {
            return Err(CompileError::new(CompileReason::BudgetInvalid));
        }
        let deadline_ticks = pool
            .deadline_ms
            .checked_mul(ticks_per_millisecond)
            .ok_or_else(|| CompileError::new(CompileReason::BudgetInvalid))?;
        let idle_timeout_ticks = pool
            .idle_timeout_ms
            .checked_mul(ticks_per_millisecond)
            .ok_or_else(|| CompileError::new(CompileReason::BudgetInvalid))?;
        let maximum_queued = match pool.admission {
            conduit_panel::PoolAdmission::QueueBounded(maximum) => maximum,
            conduit_panel::PoolAdmission::Reject
            | conduit_panel::PoolAdmission::Block
            | conduit_panel::PoolAdmission::Fail => 0,
        };
        let admission = match pool.admission {
            conduit_panel::PoolAdmission::Reject => PoolAdmissionPolicy::Reject,
            conduit_panel::PoolAdmission::Block => PoolAdmissionPolicy::Block,
            conduit_panel::PoolAdmission::QueueBounded(_) => PoolAdmissionPolicy::QueueBounded,
            conduit_panel::PoolAdmission::Fail => PoolAdmissionPolicy::Fail,
        };
        let supervision = match &pool.supervision {
            conduit_panel::PoolSupervision::FailTogether
                if runtime_binding.fallback_target.is_none() =>
            {
                PoolSupervisionPolicy::FailTogether
            }
            conduit_panel::PoolSupervision::Isolate
                if runtime_binding.fallback_target.is_none() =>
            {
                PoolSupervisionPolicy::Isolate
            }
            conduit_panel::PoolSupervision::RestartBounded {
                attempts,
                backoff_ms,
            } if runtime_binding.fallback_target.is_none() => {
                PoolSupervisionPolicy::RestartBounded {
                    maximum_attempts: *attempts,
                    backoff_ticks: backoff_ms
                        .checked_mul(ticks_per_millisecond)
                        .ok_or_else(|| CompileError::new(CompileReason::BudgetInvalid))?,
                }
            }
            conduit_panel::PoolSupervision::Fallback(_) => PoolSupervisionPolicy::Fallback {
                target: instance(
                    runtime_binding
                        .fallback_target
                        .as_deref()
                        .ok_or_else(|| CompileError::new(CompileReason::BudgetInvalid))?,
                )?,
            },
            conduit_panel::PoolSupervision::Escalate
                if runtime_binding.fallback_target.is_none() =>
            {
                PoolSupervisionPolicy::Escalate
            }
            _ => return Err(CompileError::new(CompileReason::BudgetInvalid)),
        };
        let per_instance: PoolReservationProfile = runtime_binding.per_instance.into();
        let total_reserved: PoolReservationProfile = runtime_binding.total_reserved.into();
        if per_instance.resources != PlanResourceBudget::from(binding.per_instance_budget)
            || per_instance.child_nodes != binding.child_nodes
            || per_instance.child_cords != binding.child_cords
            || total_reserved.resources != PlanResourceBudget::from(binding.worst_case_budget)
        {
            return Err(CompileError::new(CompileReason::BudgetInvalid));
        }
        let generation_reserved_slots = pool
            .maximum
            .checked_add(runtime_binding.candidate_maximum_live)
            .and_then(|value| value.checked_add(runtime_binding.rollback_maximum_live))
            .ok_or_else(|| CompileError::new(CompileReason::BudgetInvalid))?;
        let runtime = PlanPoolRuntime {
            contract: PoolContract {
                pool: instance(pool_path)?,
                template_hash: pool.template_contract_hash,
                implementation_set_hash: parse_hash(&binding.implementation_set_hash)?,
                maximum_live: pool.maximum,
                maximum_queued,
                admission,
                supervision,
                cleanup: match pool.cleanup {
                    conduit_panel::PoolCleanup::Drain => PoolCleanupPolicy::Drain,
                    conduit_panel::PoolCleanup::Abort => PoolCleanupPolicy::Abort,
                },
                deadline_ticks,
                idle_timeout_ticks,
                cleanup_ticks: runtime_binding.cleanup_ticks,
                reservation: per_instance,
                total_reservation: total_reserved,
                maximum_evidence_events: runtime_binding.maximum_evidence_events,
            },
            queued_reservation: runtime_binding.queued.into(),
            generation_reservation: PoolGenerationReservation {
                old_maximum_live: pool.maximum,
                candidate_maximum_live: runtime_binding.candidate_maximum_live,
                rollback_maximum_live: runtime_binding.rollback_maximum_live,
                reserved_slots: generation_reserved_slots,
                per_instance,
                reserved_resources: runtime_binding.generation_reserved.into(),
            },
        };
        instance_pools.push(PlanInstancePool {
            instance: instance(pool_path)?,
            template_hash: pool.template_contract_hash,
            derived_identity_hash: pool.semantic_hash,
            maximum_live: pool.maximum,
            maximum_queued,
            admission_policy: pin(&binding.admission_policy)?,
            supervision_policy: pin(&binding.supervision_policy)?,
            per_instance_budget: binding.per_instance_budget.into(),
            authority_grants: arena.alloc_slice_copy(&authority_grants),
            maximum_instance_ticks: binding.maximum_instance_ticks,
            implementation_set_hash: parse_hash(&binding.implementation_set_hash)?,
            correlation_slots: binding.correlation_slots,
            worst_case_budget: binding.worst_case_budget.into(),
            child_nodes: binding.child_nodes,
            child_cords: binding.child_cords,
            runtime: Some(runtime),
        });
    }
    if topology.supervisions.len() != input.supervision_bindings.len() {
        return Err(CompileError::new(CompileReason::BudgetInvalid));
    }
    let mut supervisions = Vec::with_capacity(topology.supervisions.len());
    let mut seen_supervision_bindings = BTreeSet::new();
    for topology_binding in &topology.supervisions {
        let binding = input
            .supervision_bindings
            .iter()
            .find(|binding| {
                binding.instance == topology_binding.instance
                    && binding.source_binding_hash
                        == topology_binding.source_binding_hash.to_string()
            })
            .ok_or_else(|| CompileError::new(CompileReason::BudgetInvalid))?;
        if !seen_supervision_bindings.insert(binding.instance.as_str())
            || binding.subject != topology_binding.subject
            || binding.handler != topology_binding.handler
        {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        let actions = binding
            .actions
            .iter()
            .map(supervision_action)
            .collect::<Result<Vec<_>, CompileError>>()?;
        let action_targets = binding
            .action_targets
            .iter()
            .map(|target| {
                Ok(PlanSupervisionTarget {
                    choice: id(&target.choice)?,
                    target: instance(&target.target)?,
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
        let members = binding
            .members
            .iter()
            .map(|member| instance(member))
            .collect::<Result<Vec<_>, CompileError>>()?;
        supervisions.push(PlanSupervision {
            instance: instance(&binding.instance)?,
            source_binding_hash: parse_hash(&binding.source_binding_hash)?,
            contract: SupervisionContract {
                schema_version: 0,
                id: id(&binding.id)?,
                scope: supervision_scope(&binding.scope)?,
                subject: instance(&binding.subject)?,
                handler: instance(&binding.handler)?,
                members: arena.alloc_slice_copy(&members),
                failure_mode: supervision_failure_mode(&binding.failure_mode)?,
                outer: binding.outer.as_deref().map(id).transpose()?,
                actions: arena.alloc_slice_copy(&actions),
                limits: binding.limits.into(),
                cleanup: stop_policy(&binding.cleanup)?,
                required_behavior: binding.required_behavior,
            },
            policy: pin(&binding.policy)?,
            observation_contract: pin(&binding.observation_contract)?,
            decision_contract: pin(&binding.decision_contract)?,
            action_targets: arena.alloc_slice_copy(&action_targets),
            allocation: binding.allocation.into(),
            deadline_timer: id(&binding.deadline_timer)?,
            backoff_timer: id(&binding.backoff_timer)?,
            cooldown_timer: id(&binding.cooldown_timer)?,
        });
    }
    // The current compile-input/document workflow has no field for live
    // distributed-session requirements. Fail closed instead of emitting an
    // plan whose cross-host cord would have hidden transport semantics. A
    // planner using the current core API must supply an exact
    // `PlanDistributedCord` for each such cord.
    if cords.iter().any(|cord| {
        let writer = nodes.iter().find(|node| node.instance == cord.from.node);
        let reader = nodes.iter().find(|node| node.instance == cord.to.node);
        writer
            .zip(reader)
            .is_some_and(|(writer, reader)| writer.host != reader.host)
    }) {
        return Err(CompileError::new(CompileReason::PlanInvalid));
    }
    let hazard_closure = input
        .hazard_closure
        .as_ref()
        .map(|closure| {
            plan_hazard_closure(
                closure,
                &plan_authorities,
                &arena,
                AuthorityTime {
                    basis: policy.time_basis,
                    tick: policy.current_tick,
                },
            )
        })
        .transpose()?;
    let watch_admissions =
        input
            .watch_admissions
            .iter()
            .map(|watch| {
                let subject =
                    match watch.subject_kind.as_str() {
                        "cord" => WatchSubject::Cord(id(watch
                            .cord
                            .as_deref()
                            .ok_or_else(|| CompileError::new(CompileReason::PlanInvalid))?)?),
                        "node-port" => {
                            WatchSubject::NodePort {
                                node: instance(watch.node.as_deref().ok_or_else(|| {
                                    CompileError::new(CompileReason::PlanInvalid)
                                })?)?,
                                port: id(watch.port.as_deref().ok_or_else(|| {
                                    CompileError::new(CompileReason::PlanInvalid)
                                })?)?,
                                direction: direction(watch.direction.as_deref().ok_or_else(
                                    || CompileError::new(CompileReason::PlanInvalid),
                                )?)?,
                            }
                        }
                        _ => return Err(CompileError::new(CompileReason::PlanInvalid)),
                    };
                let retention = match watch.retention.as_str() {
                    "latest" => WatchRetention::Latest,
                    "ring" => WatchRetention::Ring,
                    "sample" => WatchRetention::Sample,
                    _ => return Err(CompileError::new(CompileReason::PlanInvalid)),
                };
                Ok(WatchAdmission {
                    id: id(&watch.id)?,
                    subject,
                    operator: id(&watch.operator)?,
                    control_grant_hash: parse_hash(&watch.control_grant_hash)?,
                    lease: id(&watch.lease)?,
                    representation: pin(&watch.representation)?,
                    maximum_preview_bytes: watch.maximum_preview_bytes,
                    maximum_history: watch.maximum_history,
                    minimum_tick_interval: watch.minimum_tick_interval,
                    retention,
                    sensitivity_ceiling: sensitivity(&watch.sensitivity_ceiling)?,
                    reveal_action: watch.reveal_action.as_deref().map(id).transpose()?,
                    reveal_grant_hash: watch
                        .reveal_grant_hash
                        .as_deref()
                        .map(parse_hash)
                        .transpose()?,
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
    let plan_schema_version = EXECUTION_PLAN_SCHEMA_VERSION;
    let mut plan = ExecutionPlan {
        schema_version: plan_schema_version,
        identity: SemanticHash::from_bytes([0; 32]),
        source_semantic_hash: topology.source_semantic_hash,
        resolver: policy.resolver,
        resolver_policy_hash: policy.policy_hash,
        created_at: AuthorityTime {
            basis: policy.time_basis,
            tick: policy.current_tick,
        },
        budget: input.plan_budget.into(),
        host_observations: &host_observations,
        resources: &resource_bindings,
        workloads: &[],
        artifacts: &artifacts,
        nodes: &nodes,
        cords: &cords,
        value_envelopes: &[],
        clock_conversions: &[],
        feedback_boundaries: &feedback_boundaries,
        distributed_cords: &[],
        fanouts: &[],
        merges: &[],
        event_streams: &[],
        runtime_evidence: None,
        evidence_provider,
        watch_admissions: &watch_admissions,
        jobs: &[],
        satisfaction_proofs: &[],
        authorities: &plan_authorities,
        hazard_closure,
        composites: &composites,
        port_groups: &port_groups,
        instance_pools: &instance_pools,
        supervisions: &supervisions,
        unresolved: &[],
    };
    let mut scratch = vec![
        SemanticHash::from_bytes([0; 32]);
        plan.identity_fact_count()
            .map_err(|_| CompileError::new(CompileReason::PlanInvalid))?
    ];
    plan.identity = plan
        .semantic_hash(&mut scratch)
        .map_err(|_| CompileError::new(CompileReason::PlanInvalid))?;
    let validation_context = PlanValidationContext {
        supported_schema_version: plan_schema_version,
        now: plan.created_at,
    };
    if let Err(error) = validate_hosted_execution_plan(&plan, validation_context) {
        return Err(CompileError::new(match error.code {
            conduit_core::PlanDiagnosticCode::BudgetExceeded => CompileReason::BudgetInvalid,
            conduit_core::PlanDiagnosticCode::PolicyBudget(reason) => {
                CompileReason::PolicyBudget(reason)
            }
            conduit_core::PlanDiagnosticCode::HazardClosure(reason) => {
                CompileReason::HazardClosure(reason)
            }
            _ => CompileReason::PlanInvalid,
        }));
    }
    seal_resolved_execution_plan(&resolution, &plan, validation_context)
        .map_err(|_| CompileError::new(CompileReason::PlanInvalid))?;
    let execution_arrangement = resolve_execution_arrangement(
        &plan,
        &resolution,
        validation_context,
        ExecutionArrangementPolicy {
            plan_epoch: input.execution_arrangement.plan_epoch,
            boundary_realization: pin(&input.execution_arrangement.boundary_realization)?,
            maximum_proposal_bytes: input.execution_arrangement.maximum_proposal_bytes,
            maximum_head_of_line_ticks: input.execution_arrangement.maximum_head_of_line_ticks,
            cancellation_slots: input.execution_arrangement.cancellation_slots,
            evidence_slots: input.execution_arrangement.evidence_slots,
        },
    )
    .map_err(|_| CompileError::new(CompileReason::ExecutionArrangement))?;
    let document = plan_document(&plan, topology, &execution_arrangement)?;
    document.validate()?;
    Ok(document)
}

struct PreparedCandidate<'a> {
    document: &'a CandidateDocument,
    manifest: &'a ImplementationManifest<'a>,
    profile: &'a ExecutionProfile<'a>,
    report: &'a conduit_core::CapabilityReport<'a>,
    placement: PlacementCandidate<'a>,
    authorities: Vec<PreparedAuthority<'a>>,
}

#[derive(Clone, Copy)]
struct PreparedAuthority<'a> {
    requirement: SemanticHash,
    effect_hash: SemanticHash,
    grant_hash: SemanticHash,
    effect: EffectRequirement<'a>,
    capability: HostCapability<'a>,
    grant: AuthorityGrant<'a>,
    binding: Option<ResolvedAuthorityBinding<'a>>,
    administrative_subject: Option<AdministrativeSubject<'a>>,
    containment: Option<AdministrativeProof<'a>>,
    policy_budgets: &'a [PlanPolicyBudget<'a>],
    resource_lease: ResourceLeaseContract<'a>,
    commit_profile: EffectCommitProfile<'a>,
}

fn prepare_candidate<'a>(
    document: &'a CandidateDocument,
    arena: &'a Bump,
    time: AuthorityTime<'a>,
) -> Result<PreparedCandidate<'a>, CompileError> {
    let profile = arena.alloc(execution_profile(&document.execution_profile, arena)?);
    let mut profile_scratch =
        vec![SemanticHash::from_bytes([0; 32]); profile.identity_fact_count()];
    profile
        .validate(&mut profile_scratch)
        .map_err(|_| CompileError::new(CompileReason::InvalidInput))?;
    if pin(&document.implementation.execution_profile)?.semantic_hash != profile.semantic_hash {
        return Err(CompileError::new(CompileReason::InvalidInput));
    }
    let artifacts = document
        .artifacts
        .iter()
        .map(|artifact| artifact_manifest(artifact, arena))
        .collect::<Result<Vec<_>, CompileError>>()?;
    let artifact_refs = document
        .implementation
        .artifacts
        .iter()
        .map(|artifact| {
            Ok(ManifestArtifactRef {
                id: id(&artifact.id)?,
                digest: parse_digest(&artifact.digest)?,
                role: id(&artifact.role)?,
                required: artifact.required,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let required_interfaces =
        implementation_interfaces(&document.implementation.required_interfaces, arena)?;
    let provided_interfaces =
        implementation_interfaces(&document.implementation.provided_interfaces, arena)?;
    let manifest = arena.alloc(ImplementationManifest {
        schema_version: document.implementation.schema_version,
        identity: parse_hash(&document.implementation.identity)?,
        id: id(&document.implementation.id)?,
        implementation_version: &document.implementation.implementation_version,
        semantic_contract: pin(&document.implementation.semantic_contract)?,
        executor: executor(&document.implementation.executor)?,
        entrypoint: ManifestEntrypoint {
            name: id(&document.implementation.entrypoint_name)?,
            adapter: id(&document.implementation.entrypoint_adapter)?,
            abi: id(&document.implementation.entrypoint_abi)?,
            protocol_version: document.implementation.runtime_protocol_version,
        },
        execution_profile: pin(&document.implementation.execution_profile)?,
        artifacts: arena.alloc_slice_copy(&artifact_refs),
        required_interfaces,
        provided_interfaces,
        required_authorities: arena.alloc_slice_copy(
            &document
                .implementation
                .required_authorities
                .iter()
                .map(|hash| parse_hash(hash))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        required_effects: arena.alloc_slice_copy(
            &document
                .implementation
                .required_effects
                .iter()
                .map(|hash| parse_hash(hash))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        minimum_plan_version: document.implementation.minimum_plan_version,
        maximum_plan_version: document.implementation.maximum_plan_version,
        minimum_runtime_protocol: document.implementation.minimum_runtime_protocol,
        maximum_runtime_protocol: document.implementation.maximum_runtime_protocol,
        replacement: ReplacementSupport::Cold,
        coexistence_memory_bytes: document.implementation.coexistence_memory_bytes,
        reproducibility: None,
    });
    let report = arena.alloc(capability_report(&document.host_report, arena)?);
    let prepared_authorities = document
        .authorities
        .iter()
        .map(|authority| {
            let effect = effect_requirement(&authority.effect, arena)?;
            let capability = host_capability(&authority.capability)?;
            let grant = authority_grant(&authority.grant, arena)?;
            let observed = ObservedGrant {
                grant,
                status: if authority.status == "active" {
                    GrantStatus::Active
                } else {
                    GrantStatus::Revoked {
                        at_tick: time.tick,
                        reason: Id("compile/revoked"),
                    }
                },
            };
            let binding =
                resolve_authority(effect, report.host, time, &[capability], &[observed]).ok();
            let effect_hash = effect
                .semantic_hash()
                .map_err(|_| CompileError::new(CompileReason::InvalidInput))?;
            let grant_hash = grant
                .semantic_hash()
                .map_err(|_| CompileError::new(CompileReason::InvalidInput))?;
            if parse_hash(&authority.effect_hash)? != effect_hash
                || parse_hash(&authority.grant_hash)? != grant_hash
            {
                return Err(CompileError::new(CompileReason::InvalidInput));
            }
            let (administrative_subject, containment) = match (
                effect.administrative_class,
                authority.administrative_subject.as_ref(),
                authority.containment.as_ref(),
            ) {
                (None, None, None) => (None, None),
                (Some(_), Some(subject), Some(proof)) => {
                    let subject = administrative_subject(subject)?;
                    let proof = administrative_proof(proof, subject, arena, time)?;
                    (Some(subject), Some(proof))
                }
                (Some(_), _, None) => {
                    return Err(CompileError::new(CompileReason::Containment(
                        ContainmentReason::ApprovalMissing,
                    )));
                }
                _ => {
                    return Err(CompileError::new(CompileReason::Containment(
                        ContainmentReason::EffectClassMismatch,
                    )));
                }
            };
            let policy_budgets = authority
                .policy_budgets
                .iter()
                .map(policy_budget_binding)
                .collect::<Result<Vec<_>, CompileError>>()?;
            let resource_lease = resource_lease(&authority.resource_lease)?;
            validate_resource_lease(resource_lease)
                .map_err(|_| CompileError::new(CompileReason::InvalidInput))?;
            let commit_profile = effect_commit_profile(&authority.commit_profile)?;
            validate_effect_commit_profile(commit_profile, resource_lease)
                .map_err(|_| CompileError::new(CompileReason::InvalidInput))?;
            if resource_lease.resource_binding != capability.resource.id
                || resource_lease.holder != effect.requester
                || resource_lease.run != effect.audience
                || resource_lease.time_basis != capability.time_basis
                || commit_profile.operation != effect.action
            {
                return Err(CompileError::new(CompileReason::InvalidInput));
            }
            Ok(PreparedAuthority {
                requirement: parse_hash(&authority.requirement)?,
                effect_hash,
                grant_hash,
                effect,
                capability,
                grant,
                binding,
                administrative_subject,
                containment,
                policy_budgets: arena.alloc_slice_copy(&policy_budgets),
                resource_lease,
                commit_profile,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let authorities = document
        .implementation
        .required_authorities
        .iter()
        .map(|requirement| {
            let requirement_hash = parse_hash(requirement)?;
            let authority = prepared_authorities
                .iter()
                .find(|authority| authority.requirement == requirement_hash);
            Ok(CandidateAuthority {
                requirement: requirement_hash,
                grant: authority
                    .and_then(|authority| authority.binding.map(|_| authority.grant.id)),
                allowed: authority.is_some_and(|authority| authority.binding.is_some()),
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let capabilities = document
        .capabilities
        .iter()
        .map(|required| {
            Ok(CapabilityPredicate {
                interface: pin(&required.interface)?,
                mode: id(&required.mode)?,
                subject: required.subject.as_deref().map(id).transpose()?,
                details: required.details.as_deref().map(parse_hash).transpose()?,
                minimum_capacity: required.minimum_capacity.into(),
                satisfaction_proof: None,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let resources = document
        .resources
        .iter()
        .map(|required| {
            Ok(ResourcePredicate {
                kind: id(&required.kind)?,
                id: required.id.as_deref().map(id).transpose()?,
                descriptor: required.descriptor.as_ref().map(pin).transpose()?,
                minimum_capacity: required.minimum_capacity.into(),
                require_exclusive: required.require_exclusive,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let topology = document
        .topology
        .iter()
        .map(|required| {
            Ok(TopologyPredicate {
                contract: pin(&required.contract)?,
                from: id(&required.from)?,
                to: id(&required.to)?,
                minimum_transfer_unit: required.minimum_transfer_unit,
                minimum_sessions: required.minimum_sessions,
                details: required.details.as_deref().map(parse_hash).transpose()?,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let placement = PlacementCandidate {
        manifest,
        artifacts: arena.alloc_slice_copy(&artifacts),
        report,
        allocation: document.allocation.into(),
        capabilities: arena.alloc_slice_copy(&capabilities),
        resources: arena.alloc_slice_copy(&resources),
        topology: arena.alloc_slice_copy(&topology),
        authorities: arena.alloc_slice_copy(&authorities),
    };
    Ok(PreparedCandidate {
        document,
        manifest,
        profile,
        report,
        placement,
        authorities: prepared_authorities,
    })
}

fn artifact_manifest<'a>(
    document: &'a ArtifactDocument,
    arena: &'a Bump,
) -> Result<&'a ArtifactManifest<'a>, CompileError> {
    let licenses = document
        .license_expressions
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    Ok(arena.alloc(ArtifactManifest {
        schema_version: document.schema_version,
        identity: parse_hash(&document.identity)?,
        id: id(&document.id)?,
        digest: parse_digest(&document.digest)?,
        media_type: &document.media_type,
        byte_size: document.byte_size,
        target: document.target.as_deref().map(id).transpose()?,
        abi: document.abi.as_deref().map(id).transpose()?,
        provenance: ArtifactProvenance {
            builder: id(&document.builder)?,
            source_digest: parse_digest(&document.source_digest)?,
            build_recipe_digest: parse_digest(&document.build_recipe_digest)?,
            reproducible: document.reproducible,
        },
        signatures: &[],
        license_expressions: arena.alloc_slice_copy(&licenses),
        notices: &[],
        sbom: None,
        source: None,
        related_artifacts: &[],
        locations: &[],
    }))
}

fn capability_report<'a>(
    document: &'a HostReportDocument,
    arena: &'a Bump,
) -> Result<conduit_core::CapabilityReport<'a>, CompileError> {
    let membership = document
        .membership
        .as_ref()
        .map(|membership| {
            let passport = parse_hash(&membership.passport)?;
            let realm = id(&membership.realm)?;
            let entity = id(&membership.entity)?;
            Ok(ReportMembership {
                realm,
                entity,
                passport,
                status: PassportStatusObservation {
                    passport,
                    realm,
                    entity,
                    reporter: pin(&membership.status_reporter)?,
                    time_basis: id(&membership.status_time_basis)?,
                    observed_at_tick: membership.status_observed_at_tick,
                    valid_until_tick: membership.status_valid_until_tick,
                    status: passport_status(&membership.status)?,
                },
            })
        })
        .transpose()?;
    let capabilities = document
        .capabilities
        .iter()
        .map(|capability| {
            Ok(ReportCapability {
                interface: pin(&capability.interface)?,
                mode: id(&capability.mode)?,
                subject: id(&capability.subject)?,
                details: parse_hash(&capability.details)?,
                capacity: capability.capacity.into(),
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let resources = document
        .resources
        .iter()
        .map(|resource| {
            Ok(ReportResource {
                resource: ResourceRef {
                    kind: id(&resource.kind)?,
                    id: id(&resource.id)?,
                },
                descriptor: pin(&resource.descriptor)?,
                capacity: resource.capacity.into(),
                exclusive: resource.exclusive,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let topology = document
        .topology
        .iter()
        .map(|edge| {
            Ok(ReportTopology {
                id: id(&edge.id)?,
                contract: pin(&edge.contract)?,
                from: id(&edge.from)?,
                to: id(&edge.to)?,
                maximum_transfer_unit: edge.maximum_transfer_unit,
                maximum_sessions: edge.maximum_sessions,
                reachable: edge.reachable,
                details: parse_hash(&edge.details)?,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let report_id = id(&document.id)?;
    let execution_placements = document
        .execution_placements
        .iter()
        .map(|placement| {
            Ok(ExecutionPlacement {
                id: id(&placement.id)?,
                host_observation: report_id,
                provider: pin(&placement.provider)?,
                authority_boundary: pin(&placement.authority_boundary)?,
                resource_boundary: pin(&placement.resource_boundary)?,
                lifecycle_boundary: pin(&placement.lifecycle_boundary)?,
                failure_boundary: pin(&placement.failure_boundary)?,
                generation: placement.generation,
                isolation: isolation_profile(&placement.isolation)?,
                memory_containment: execution_guarantee(&placement.memory_containment)?,
                regain_control: execution_guarantee(&placement.regain_control)?,
                effect_fencing: execution_guarantee(&placement.effect_fencing)?,
                stop_execution: execution_guarantee(&placement.stop_execution)?,
                reclaim_resources: execution_guarantee(&placement.reclaim_resources)?,
                maximum_regain_control_ticks: placement.maximum_regain_control_ticks,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let execution_lanes = document
        .execution_lanes
        .iter()
        .map(|lane| {
            Ok(ExecutionLane {
                id: id(&lane.id)?,
                placement: id(&lane.placement)?,
                placement_generation: lane.placement_generation,
                generation: lane.generation,
                independent_progress: execution_guarantee(&lane.independent_progress)?,
                simultaneous_execution: execution_guarantee(&lane.simultaneous_execution)?,
                preemption: execution_guarantee(&lane.preemption)?,
                termination: execution_guarantee(&lane.termination)?,
                ready_slots: lane.ready_slots,
                wake_slots: lane.wake_slots,
                proposal_slots: lane.proposal_slots,
                commit_slots: lane.commit_slots,
                timer_slots: lane.timer_slots,
                scratch_bytes: lane.scratch_bytes,
                stack_bytes: lane.stack_bytes,
                evidence_slots: lane.evidence_slots,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let executors = document
        .supported_executors
        .iter()
        .map(|value| executor(value))
        .collect::<Result<Vec<_>, _>>()?;
    let targets = document
        .supported_targets
        .iter()
        .map(|value| id(value))
        .collect::<Result<Vec<_>, _>>()?;
    let abis = document
        .supported_abis
        .iter()
        .map(|value| id(value))
        .collect::<Result<Vec<_>, _>>()?;
    let current_constraints = document
        .current_constraints
        .iter()
        .map(|value| parse_hash(value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(conduit_core::CapabilityReport {
        schema_version: document.schema_version,
        identity: parse_hash(&document.identity)?,
        id: report_id,
        host: id(&document.host)?,
        boot_id: id(&document.boot_id)?,
        reporter: pin(&document.reporter)?,
        trust: pin(&document.trust)?,
        membership,
        time_basis: id(&document.time_basis)?,
        observed_at_tick: document.observed_at_tick,
        valid_until_tick: document.valid_until_tick,
        available: document.available.into(),
        capabilities: arena.alloc_slice_copy(&capabilities),
        resources: arena.alloc_slice_copy(&resources),
        topology: arena.alloc_slice_copy(&topology),
        execution_placements: arena.alloc_slice_copy(&execution_placements),
        execution_lanes: arena.alloc_slice_copy(&execution_lanes),
        supported_executors: arena.alloc_slice_copy(&executors),
        supported_targets: arena.alloc_slice_copy(&targets),
        supported_abis: arena.alloc_slice_copy(&abis),
        minimum_plan_version: document.minimum_plan_version,
        maximum_plan_version: document.maximum_plan_version,
        current_constraints: arena.alloc_slice_copy(&current_constraints),
    })
}

fn resolver_policy<'a>(
    input: &'a CompileInput,
    arena: &'a Bump,
) -> Result<HostResolverPolicy<'a>, CompileError> {
    let mut trusted_reporters = input
        .candidates
        .iter()
        .map(|candidate| pin(&candidate.host_report.reporter))
        .collect::<Result<Vec<_>, _>>()?;
    trusted_reporters.sort_by(|left, right| {
        (
            left.id.as_str(),
            left.schema_version,
            left.semantic_hash.as_bytes(),
        )
            .cmp(&(
                right.id.as_str(),
                right.schema_version,
                right.semantic_hash.as_bytes(),
            ))
    });
    trusted_reporters.dedup();
    let mut report_trust = input
        .candidates
        .iter()
        .map(|candidate| parse_hash(&candidate.host_report.trust.semantic_hash))
        .collect::<Result<Vec<_>, _>>()?;
    report_trust.sort_by_key(SemanticHash::to_string);
    report_trust.dedup();
    let mut allowed = input
        .candidates
        .iter()
        .map(|candidate| id(&candidate.implementation.id))
        .collect::<Result<Vec<_>, _>>()?;
    allowed.sort_by_key(|value| value.as_str());
    allowed.dedup();
    let preference = input
        .implementation_preference
        .iter()
        .map(|value| id(value))
        .collect::<Result<Vec<_>, _>>()?;
    let trusted_entities = input
        .trusted_entities
        .iter()
        .map(|value| id(value))
        .collect::<Result<Vec<_>, _>>()?;
    let trusted_status_reporters = input
        .trusted_status_reporters
        .iter()
        .map(|value| parse_hash(value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HostResolverPolicy {
        resolver: pin(&input.resolver)?,
        policy_hash: parse_hash(&input.resolver_policy_hash)?,
        time_basis: id(&input.time_basis)?,
        current_tick: input.current_tick,
        plan_version: EXECUTION_PLAN_SCHEMA_VERSION,
        trusted_reporters: arena.alloc_slice_copy(&trusted_reporters),
        trusted_report_trust: arena.alloc_slice_copy(&report_trust),
        required_realm: input.required_realm.as_deref().map(id).transpose()?,
        trusted_entities: arena.alloc_slice_copy(&trusted_entities),
        trusted_status_reporters: arena.alloc_slice_copy(&trusted_status_reporters),
        require_active_passport: input.require_active_passport,
        allowed_implementations: arena.alloc_slice_copy(&allowed),
        implementation_preference: arena.alloc_slice_copy(&preference),
        tie_policy: tie_policy(&input.tie_policy)?,
        maximum_search_states: input.maximum_search_states,
    })
}

fn policy_hash(input: &CompileInput) -> Result<String, CompileError> {
    let arena = Bump::new();
    let mut copy = input.clone();
    copy.resolver_policy_hash = SemanticHash::from_bytes([0; 32]).to_string();
    let policy = resolver_policy(&copy, &arena)?;
    let hash = policy
        .computed_semantic_hash()
        .map_err(|_| CompileError::new(CompileReason::InvalidInput))?;
    Ok(hash.to_string())
}

fn artifact_identity(document: &ArtifactDocument) -> Result<String, CompileError> {
    let arena = Bump::new();
    let mut document = document.clone();
    document.identity = SemanticHash::from_bytes([0; 32]).to_string();
    let manifest = artifact_manifest(&document, &arena)?;
    let mut scratch = vec![SemanticHash::from_bytes([0; 32]); manifest.identity_fact_count()];
    manifest
        .computed_semantic_hash(&mut scratch)
        .map(|hash| hash.to_string())
        .map_err(|_| CompileError::new(CompileReason::InvalidInput))
}

fn seal_authority_decision(document: &mut AuthorityDecisionDocument) -> Result<(), CompileError> {
    parse_hash(&document.requirement)?;
    let arena = Bump::new();
    let effect = effect_requirement(&document.effect, &arena)?;
    let grant = authority_grant(&document.grant, &arena)?;
    document.effect_hash = effect
        .semantic_hash()
        .map_err(|_| CompileError::new(CompileReason::InvalidInput))?
        .to_string();
    document.grant_hash = grant
        .semantic_hash()
        .map_err(|_| CompileError::new(CompileReason::InvalidInput))?
        .to_string();
    match (
        document.effect.administrative_class.as_ref(),
        document.administrative_subject.as_ref(),
        document.containment.as_mut(),
    ) {
        (None, None, None) => {}
        (Some(effect_class), Some(subject), Some(proof)) => {
            if &proof.proposal.effect_class != effect_class
                || &proof.policy.effect_class != effect_class
                || &proof.proposal.subject != subject
            {
                return Err(CompileError::new(CompileReason::Containment(
                    ContainmentReason::EffectClassMismatch,
                )));
            }
            seal_administrative_proof(proof)?;
        }
        (Some(_), _, None) => {
            return Err(CompileError::new(CompileReason::Containment(
                ContainmentReason::ApprovalMissing,
            )));
        }
        _ => {
            return Err(CompileError::new(CompileReason::Containment(
                ContainmentReason::EffectClassMismatch,
            )));
        }
    }
    match (
        document.effect.policy_budget_class.as_ref(),
        document.policy_budgets.is_empty(),
    ) {
        (None, true) => {}
        (Some(class), false) => {
            for binding in &mut document.policy_budgets {
                if &binding.policy.resource_class != class
                    || binding.policy.action != document.effect.action
                {
                    return Err(CompileError::new(CompileReason::InvalidInput));
                }
                seal_policy_budget_binding(binding)?;
            }
        }
        _ => return Err(CompileError::new(CompileReason::InvalidInput)),
    }
    let lease = resource_lease(&document.resource_lease)?;
    validate_resource_lease(lease).map_err(|_| CompileError::new(CompileReason::InvalidInput))?;
    let commit_profile = effect_commit_profile(&document.commit_profile)?;
    validate_effect_commit_profile(commit_profile, lease)
        .map_err(|_| CompileError::new(CompileReason::InvalidInput))?;
    if document.effect.resource_id.as_deref() != Some(&document.resource_lease.resource_binding)
        || document.effect.requester != document.resource_lease.holder
        || document.effect.audience != document.resource_lease.run
        || document.capability.time_basis != document.resource_lease.time_basis
        || document.effect.action != document.commit_profile.operation
    {
        return Err(CompileError::new(CompileReason::InvalidInput));
    }
    match document.status.as_str() {
        "active" | "revoked" => Ok(()),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn seal_policy_budget_binding(
    document: &mut PolicyBudgetBindingDocument,
) -> Result<(), CompileError> {
    document.policy.identity = SemanticHash::from_bytes([0; 32]).to_string();
    let policy = persistent_budget_policy(&document.policy)?;
    document.policy.identity = policy
        .computed_semantic_hash()
        .map_err(|_| CompileError::new(CompileReason::InvalidInput))?
        .to_string();
    document
        .status
        .policy_identity
        .clone_from(&document.policy.identity);
    document.status.identity = SemanticHash::from_bytes([0; 32]).to_string();
    let status = policy_budget_status(&document.status)?;
    document.status.identity = status
        .computed_semantic_hash()
        .map_err(|_| CompileError::new(CompileReason::InvalidInput))?
        .to_string();
    if let Some(lease) = &mut document.lease {
        lease.policy_identity.clone_from(&document.policy.identity);
        lease.identity = SemanticHash::from_bytes([0; 32]).to_string();
        let lease_value = policy_budget_lease(lease)?;
        lease.identity = lease_value
            .computed_semantic_hash()
            .map_err(|_| CompileError::new(CompileReason::InvalidInput))?
            .to_string();
    }
    Ok(())
}

fn seal_administrative_proof(
    document: &mut AdministrativeProofDocument,
) -> Result<(), CompileError> {
    let zero = SemanticHash::from_bytes([0; 32]).to_string();

    document.policy.identity.clone_from(&zero);
    {
        let arena = Bump::new();
        let policy = containment_policy(&document.policy, &arena)?;
        document.policy.identity = policy
            .computed_semantic_hash()
            .map_err(|_| CompileError::new(CompileReason::InvalidInput))?
            .to_string();
    }

    document.proposal.identity.clone_from(&zero);
    {
        let arena = Bump::new();
        let proposal = administrative_proposal(&document.proposal, &arena)?;
        document.proposal.identity = proposal
            .computed_semantic_hash()
            .map_err(|_| CompileError::new(CompileReason::InvalidInput))?
            .to_string();
    }

    for approval in &mut document.approvals {
        approval
            .proposal_identity
            .clone_from(&document.proposal.identity);
        approval
            .policy_identity
            .clone_from(&document.policy.identity);
        approval.identity.clone_from(&zero);
        let value = administrative_approval(approval)?;
        approval.identity = value
            .computed_semantic_hash()
            .map_err(|_| CompileError::new(CompileReason::InvalidInput))?
            .to_string();
    }

    document
        .commit
        .proposal_identity
        .clone_from(&document.proposal.identity);
    document
        .commit
        .policy_identity
        .clone_from(&document.policy.identity);
    document.commit.approvals = document
        .approvals
        .iter()
        .map(|approval| approval.identity.clone())
        .collect();
    document.commit.identity.clone_from(&zero);
    {
        let arena = Bump::new();
        let commit = administrative_commit(&document.commit, &arena)?;
        document.commit.identity = commit
            .computed_semantic_hash()
            .map_err(|_| CompileError::new(CompileReason::InvalidInput))?
            .to_string();
    }

    document
        .execution
        .proposal_identity
        .clone_from(&document.proposal.identity);
    document
        .execution
        .commit_identity
        .clone_from(&document.commit.identity);
    document.execution.identity.clone_from(&zero);
    let execution = administrative_execution(&document.execution)?;
    document.execution.identity = execution
        .computed_semantic_hash()
        .map_err(|_| CompileError::new(CompileReason::InvalidInput))?
        .to_string();
    Ok(())
}

fn seal_hazard_closure(document: &mut HazardClosureDocument) -> Result<(), CompileError> {
    let zero = SemanticHash::from_bytes([0; 32]).to_string();
    for class in &mut document.policy.classes {
        class.identity.clone_from(&zero);
        class.identity = effect_class_binding(class)?
            .computed_semantic_hash()
            .map_err(|_| {
                CompileError::new(CompileReason::HazardClosure(
                    HazardClosureReason::InvalidDescriptor,
                ))
            })?
            .to_string();
    }
    for rule in &mut document.policy.rules {
        rule.identity.clone_from(&zero);
        let arena = Bump::new();
        let patterns = rule
            .patterns
            .iter()
            .map(toxic_effect_pattern)
            .collect::<Result<Vec<_>, _>>()?;
        let flows = rule
            .flows
            .iter()
            .map(toxic_flow_requirement)
            .collect::<Result<Vec<_>, _>>()?;
        let value = ToxicCombinationRule {
            identity: SemanticHash::from_bytes([0; 32]),
            descriptor: pin(&rule.descriptor)?,
            patterns: arena.alloc_slice_copy(&patterns),
            flows: arena.alloc_slice_copy(&flows),
        };
        rule.identity = value
            .computed_semantic_hash()
            .map_err(|_| {
                CompileError::new(CompileReason::HazardClosure(
                    HazardClosureReason::RuleInvalid,
                ))
            })?
            .to_string();
    }
    document.policy.identity.clone_from(&zero);
    {
        let arena = Bump::new();
        let policy = hazard_closure_policy(&document.policy, &arena)?;
        document.policy.identity = policy
            .computed_semantic_hash()
            .map_err(|_| {
                CompileError::new(CompileReason::HazardClosure(
                    HazardClosureReason::InvalidDescriptor,
                ))
            })?
            .to_string();
    }
    for permit in &mut document.permits {
        permit.policy_identity.clone_from(&document.policy.identity);
        seal_administrative_proof(&mut permit.approval)?;
        permit.identity.clone_from(&zero);
        let arena = Bump::new();
        let now = AuthorityTime {
            basis: id(&permit.time_basis)?,
            tick: permit.not_before_tick,
        };
        permit.identity = hazard_permit(permit, &arena, now)?
            .computed_semantic_hash()
            .map_err(|_| {
                CompileError::new(CompileReason::HazardClosure(
                    HazardClosureReason::PermitApprovalInvalid,
                ))
            })?
            .to_string();
    }
    for binding in &mut document.hazardous_hosts {
        binding.profile.identity.clone_from(&zero);
        binding.observation.profile_identity.clone_from(&zero);
        binding.observation.identity.clone_from(&zero);
        {
            let arena = Bump::new();
            let value = hazardous_host_binding(binding, &arena)?;
            let mut scratch = vec![SemanticHash::from_bytes([0; 32]); value.profile.envelope.len()];
            binding.profile.identity = value
                .profile
                .computed_semantic_hash(&mut scratch)
                .map_err(|_| CompileError::new(CompileReason::PlanInvalid))?
                .to_string();
        }
        binding
            .observation
            .profile_identity
            .clone_from(&binding.profile.identity);
        {
            let arena = Bump::new();
            let value = hazardous_host_binding(binding, &arena)?;
            binding.observation.identity = value
                .observation
                .computed_semantic_hash()
                .map_err(|_| CompileError::new(CompileReason::PlanInvalid))?
                .to_string();
        }
    }
    parse_hash(&document.plan_subject)?;
    parse_hash(&document.decision_identity)?;
    Ok(())
}

fn seal_distribution(document: &mut ReferenceDistributionDocument) -> Result<(), CompileError> {
    document.identity = SemanticHash::from_bytes([0; 32]).to_string();
    let arena = Bump::new();
    let profile = reference_distribution(document, &arena)?;
    let mut scratch = vec![SemanticHash::from_bytes([0; 32]); profile.identity_fact_count()];
    document.identity = profile
        .computed_semantic_hash(&mut scratch)
        .map_err(|_| CompileError::new(CompileReason::Genesis(GenesisReason::InvalidDescriptor)))?
        .to_string();
    validate_distribution_document(document)
}

fn validate_distribution_document(
    document: &ReferenceDistributionDocument,
) -> Result<(), CompileError> {
    if document.schema != REFERENCE_DISTRIBUTION_DOCUMENT_SCHEMA {
        return Err(CompileError::new(CompileReason::Genesis(
            GenesisReason::UnsupportedVersion,
        )));
    }
    let arena = Bump::new();
    let profile = reference_distribution(document, &arena)?;
    let mut scratch = vec![SemanticHash::from_bytes([0; 32]); profile.identity_fact_count()];
    validate_reference_distribution(profile, &mut scratch)
        .map_err(|reason| CompileError::new(CompileReason::Genesis(reason)))?;
    for requirement in &document.requirements {
        let requirement = ProviderRequirement {
            provider: pin(&requirement.provider)?,
            traits: requirement.traits.into(),
        };
        let decision = assess_provider_requirement(profile, requirement)
            .map_err(|reason| CompileError::new(CompileReason::Genesis(reason)))?;
        if decision.selection != ProviderSelection::Available {
            return Err(CompileError::provider(
                GenesisReason::ProviderUnavailable,
                decision.provider.id.as_str(),
                decision.selection,
            ));
        }
    }
    Ok(())
}

fn reference_distribution<'a>(
    document: &'a ReferenceDistributionDocument,
    arena: &'a Bump,
) -> Result<ReferenceDistributionProfile<'a>, CompileError> {
    let providers = document
        .providers
        .iter()
        .map(|provider| {
            Ok(DistributionProvider {
                provider: pin(&provider.provider)?,
                artifact: provider.artifact.as_deref().map(parse_digest).transpose()?,
                availability: provider_availability(&provider.availability)?,
                traits: provider.traits.into(),
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    Ok(ReferenceDistributionProfile {
        schema_version: document.schema_version,
        identity: parse_hash(&document.identity)?,
        descriptor: pin(&document.descriptor)?,
        kind: distribution_kind(&document.kind)?,
        genesis_profile: parse_hash(&document.genesis_profile)?,
        control_recorder: pin(&document.control_recorder)?,
        provider_enablement_effect_class: pin(&document.provider_enablement_effect_class)?,
        provider_enablement_operation: pin(&document.provider_enablement_operation)?,
        providers: arena.alloc_slice_copy(&providers),
        maximum_provider_enablement_ticks: document.maximum_provider_enablement_ticks,
        maximum_provider_install_attempts: document.maximum_provider_install_attempts,
        maximum_evidence_events: document.maximum_evidence_events,
    })
}

fn distribution_kind(value: &str) -> Result<HostDistributionKind, CompileError> {
    match value {
        "hosted" => Ok(HostDistributionKind::Hosted),
        "browser" => Ok(HostDistributionKind::Browser),
        "constrained" => Ok(HostDistributionKind::Constrained),
        _ => Err(CompileError::new(CompileReason::Genesis(
            GenesisReason::InvalidDescriptor,
        ))),
    }
}

fn provider_availability(value: &str) -> Result<ProviderAvailability, CompileError> {
    match value {
        "absent" => Ok(ProviderAvailability::Absent),
        "disabled" => Ok(ProviderAvailability::Disabled),
        "enabled" => Ok(ProviderAvailability::Enabled),
        "unsupported" => Ok(ProviderAvailability::Unsupported),
        _ => Err(CompileError::new(CompileReason::Genesis(
            GenesisReason::InvalidDescriptor,
        ))),
    }
}

fn authority_constraints<'a>(
    documents: &'a [AuthorityConstraintDocument],
    arena: &'a Bump,
) -> Result<&'a [AuthorityConstraintRef<'a>], CompileError> {
    if documents.len() > conduit_core::MAX_AUTHORITY_CONSTRAINTS {
        return Err(CompileError::new(CompileReason::InvalidInput));
    }
    let constraints = documents
        .iter()
        .map(|constraint| {
            Ok(AuthorityConstraintRef {
                id: id(&constraint.id)?,
                semantic_hash: parse_hash(&constraint.semantic_hash)?,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    Ok(arena.alloc_slice_copy(&constraints))
}

fn effect_requirement<'a>(
    document: &'a EffectRequirementDocument,
    arena: &'a Bump,
) -> Result<EffectRequirement<'a>, CompileError> {
    let resource = match document.resource_id.as_deref() {
        Some(resource_id) => ResourceSelector::Exact(ResourceRef {
            kind: id(&document.resource_kind)?,
            id: id(resource_id)?,
        }),
        None => ResourceSelector::Kind(id(&document.resource_kind)?),
    };
    Ok(EffectRequirement {
        id: id(&document.id)?,
        administrative_class: document
            .administrative_class
            .as_ref()
            .map(pin)
            .transpose()?,
        policy_budget_class: document.policy_budget_class.as_ref().map(pin).transpose()?,
        action: id(&document.action)?,
        resource,
        requester: instance(&document.requester)?,
        audience: id(&document.audience)?,
        constraints: authority_constraints(&document.constraints, arena)?,
        check_at_use: document.check_at_use,
    })
}

fn administrative_principal(
    document: &AdministrativePrincipalDocument,
) -> Result<AdministrativePrincipal<'_>, CompileError> {
    Ok(AdministrativePrincipal {
        realm: id(&document.realm)?,
        entity: id(&document.entity)?,
        key: id(&document.key)?,
        profile: pin(&document.profile)?,
        source_plan: parse_hash(&document.source_plan)?,
        source_epoch: document.source_epoch,
    })
}

fn administrative_subject(
    document: &AdministrativeSubjectDocument,
) -> Result<AdministrativeSubject<'_>, CompileError> {
    Ok(AdministrativeSubject {
        realm: id(&document.realm)?,
        entity: id(&document.entity)?,
        plan: parse_hash(&document.plan)?,
        epoch: document.epoch,
        artifact: document.artifact.as_deref().map(parse_digest).transpose()?,
        budget: document.budget.as_ref().map(pin).transpose()?,
    })
}

fn delegation_envelope(
    document: &DelegationEnvelopeDocument,
) -> Result<DelegationEnvelope<'_>, CompileError> {
    let resource = match document.resource_id.as_deref() {
        Some(resource_id) => ResourceSelector::Exact(ResourceRef {
            kind: id(&document.resource_kind)?,
            id: id(resource_id)?,
        }),
        None => ResourceSelector::Kind(id(&document.resource_kind)?),
    };
    Ok(DelegationEnvelope {
        action: id(&document.action)?,
        resource,
        audience: id(&document.audience)?,
        time_basis: id(&document.time_basis)?,
        not_before_tick: document.not_before_tick,
        expires_at_tick: document.expires_at_tick,
        remaining_depth: document.remaining_depth,
    })
}

fn containment_policy<'a>(
    document: &'a ContainmentPolicyDocument,
    arena: &'a Bump,
) -> Result<ContainmentPolicy<'a>, CompileError> {
    let approvers = document
        .approvers
        .iter()
        .map(|approver| {
            Ok(AdministrativeApprover {
                realm: id(&approver.realm)?,
                entity: id(&approver.entity)?,
                key: id(&approver.key)?,
                profile: pin(&approver.profile)?,
                failure_domain: pin(&approver.failure_domain)?,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    Ok(ContainmentPolicy {
        schema_version: document.schema_version,
        identity: parse_hash(&document.identity)?,
        descriptor: pin(&document.descriptor)?,
        effect_class: pin(&document.effect_class)?,
        approvers: arena.alloc_slice_copy(&approvers),
        committer: administrative_approver(&document.committer)?,
        executor: administrative_approver(&document.executor)?,
        minimum_approvals: document.minimum_approvals,
        minimum_failure_domains: document.minimum_failure_domains,
        requester_independence: document.requester_independence,
        beneficiary_independence: document.beneficiary_independence,
        successor_independence: document.successor_independence,
        delegation_ceiling: document
            .delegation_ceiling
            .as_ref()
            .map(delegation_envelope)
            .transpose()?,
        ceremony: document.ceremony.as_ref().map(pin).transpose()?,
    })
}

fn administrative_approver(
    document: &AdministrativeApproverDocument,
) -> Result<AdministrativeApprover<'_>, CompileError> {
    Ok(AdministrativeApprover {
        realm: id(&document.realm)?,
        entity: id(&document.entity)?,
        key: id(&document.key)?,
        profile: pin(&document.profile)?,
        failure_domain: pin(&document.failure_domain)?,
    })
}

fn administrative_proposal<'a>(
    document: &'a AdministrativeProposalDocument,
    arena: &'a Bump,
) -> Result<AdministrativeProposal<'a>, CompileError> {
    let beneficiaries = document
        .beneficiaries
        .iter()
        .map(administrative_subject)
        .collect::<Result<Vec<_>, CompileError>>()?;
    Ok(AdministrativeProposal {
        schema_version: document.schema_version,
        identity: parse_hash(&document.identity)?,
        id: id(&document.id)?,
        effect_class: pin(&document.effect_class)?,
        operation: pin(&document.operation)?,
        requester: administrative_principal(&document.requester)?,
        subject: administrative_subject(&document.subject)?,
        beneficiaries: arena.alloc_slice_copy(&beneficiaries),
        predecessor_plan: document
            .predecessor_plan
            .as_deref()
            .map(parse_hash)
            .transpose()?,
        delegation: document
            .delegation
            .as_ref()
            .map(delegation_envelope)
            .transpose()?,
        protected_handle: document.protected_handle.as_ref().map(pin).transpose()?,
        ceremony: document.ceremony.as_ref().map(pin).transpose()?,
        time_basis: id(&document.time_basis)?,
        created_at_tick: document.created_at_tick,
        expires_at_tick: document.expires_at_tick,
    })
}

fn administrative_approval(
    document: &AdministrativeApprovalDocument,
) -> Result<AdministrativeApproval<'_>, CompileError> {
    Ok(AdministrativeApproval {
        schema_version: document.schema_version,
        identity: parse_hash(&document.identity)?,
        id: id(&document.id)?,
        proposal_identity: parse_hash(&document.proposal_identity)?,
        policy_identity: parse_hash(&document.policy_identity)?,
        approver: administrative_principal(&document.approver)?,
        failure_domain: pin(&document.failure_domain)?,
        time_basis: id(&document.time_basis)?,
        issued_at_tick: document.issued_at_tick,
        expires_at_tick: document.expires_at_tick,
        status: match document.status.as_str() {
            "current" => AdministrativeApprovalStatus::Current,
            "revoked" => AdministrativeApprovalStatus::Revoked,
            _ => return Err(CompileError::new(CompileReason::InvalidInput)),
        },
    })
}

fn administrative_commit<'a>(
    document: &'a AdministrativeCommitDocument,
    arena: &'a Bump,
) -> Result<AdministrativeCommit<'a>, CompileError> {
    let approvals = document
        .approvals
        .iter()
        .map(|approval| parse_hash(approval))
        .collect::<Result<Vec<_>, CompileError>>()?;
    Ok(AdministrativeCommit {
        schema_version: document.schema_version,
        identity: parse_hash(&document.identity)?,
        id: id(&document.id)?,
        proposal_identity: parse_hash(&document.proposal_identity)?,
        policy_identity: parse_hash(&document.policy_identity)?,
        approvals: arena.alloc_slice_copy(&approvals),
        committed_by: administrative_principal(&document.committed_by)?,
        committed_at_tick: document.committed_at_tick,
    })
}

fn administrative_execution(
    document: &AdministrativeExecutionDocument,
) -> Result<AdministrativeExecution<'_>, CompileError> {
    Ok(AdministrativeExecution {
        schema_version: document.schema_version,
        identity: parse_hash(&document.identity)?,
        id: id(&document.id)?,
        proposal_identity: parse_hash(&document.proposal_identity)?,
        commit_identity: parse_hash(&document.commit_identity)?,
        executor: administrative_principal(&document.executor)?,
        time_basis: id(&document.time_basis)?,
        not_before_tick: document.not_before_tick,
        expires_at_tick: document.expires_at_tick,
    })
}

fn administrative_proof<'a>(
    document: &'a AdministrativeProofDocument,
    subject: AdministrativeSubject<'a>,
    arena: &'a Bump,
    now: AuthorityTime<'a>,
) -> Result<AdministrativeProof<'a>, CompileError> {
    let approvals = document
        .approvals
        .iter()
        .map(administrative_approval)
        .collect::<Result<Vec<_>, CompileError>>()?;
    let proof = AdministrativeProof {
        proposal: administrative_proposal(&document.proposal, arena)?,
        policy: containment_policy(&document.policy, arena)?,
        approvals: arena.alloc_slice_copy(&approvals),
        commit: administrative_commit(&document.commit, arena)?,
        execution: administrative_execution(&document.execution)?,
    };
    validate_administrative_proof(
        proof,
        ContainmentContext {
            subject,
            time_basis: now.basis,
            now_tick: now.tick,
        },
    )
    .map_err(|reason| CompileError::new(CompileReason::Containment(reason)))?;
    Ok(proof)
}

fn trait_requirement(value: &str) -> Result<TraitRequirement, CompileError> {
    match value {
        "any" => Ok(TraitRequirement::Any),
        "required" => Ok(TraitRequirement::Required),
        "forbidden" => Ok(TraitRequirement::Forbidden),
        _ => Err(CompileError::new(CompileReason::HazardClosure(
            HazardClosureReason::RuleInvalid,
        ))),
    }
}

fn effect_class_binding(
    document: &EffectClassBindingDocument,
) -> Result<EffectClassBinding<'_>, CompileError> {
    let descriptor = pin(&document.descriptor)?;
    Ok(EffectClassBinding {
        identity: parse_hash(&document.identity)?,
        descriptor,
        constraint: AuthorityConstraintRef {
            id: descriptor.id,
            semantic_hash: descriptor.semantic_hash,
        },
        traits: EffectClassTraits {
            persistence: document.persistence,
            delegation: document.delegation,
            distributed: document.distributed,
            administrative: document.administrative,
        },
    })
}

fn toxic_effect_pattern(
    document: &ToxicEffectPatternDocument,
) -> Result<ToxicEffectPattern<'_>, CompileError> {
    let resource = match (
        document.resource_kind.as_deref(),
        document.resource_id.as_deref(),
    ) {
        (None, None) => None,
        (Some(kind), None) => Some(ResourceSelector::Kind(id(kind)?)),
        (Some(kind), Some(resource)) => Some(ResourceSelector::Exact(ResourceRef {
            kind: id(kind)?,
            id: id(resource)?,
        })),
        (None, Some(_)) => {
            return Err(CompileError::new(CompileReason::HazardClosure(
                HazardClosureReason::RuleInvalid,
            )));
        }
    };
    Ok(ToxicEffectPattern {
        id: id(&document.id)?,
        class: pin(&document.class)?,
        resource,
        audience: document.audience.as_deref().map(id).transpose()?,
        host: document.host.as_deref().map(id).transpose()?,
        realm: document.realm.as_deref().map(id).transpose()?,
        budget: document.budget.as_ref().map(pin).transpose()?,
        persistence: trait_requirement(&document.persistence)?,
        delegation: trait_requirement(&document.delegation)?,
        distributed: trait_requirement(&document.distributed)?,
        administrative: trait_requirement(&document.administrative)?,
    })
}

fn toxic_flow_requirement(
    document: &ToxicFlowRequirementDocument,
) -> Result<ToxicFlowRequirement<'_>, CompileError> {
    Ok(ToxicFlowRequirement {
        from_pattern: document.from_pattern,
        to_pattern: document.to_pattern,
        transfer: pin(&document.transfer)?,
    })
}

fn hazard_closure_policy<'a>(
    document: &'a HazardClosurePolicyDocument,
    arena: &'a Bump,
) -> Result<HazardClosurePolicy<'a>, CompileError> {
    let classes = document
        .classes
        .iter()
        .map(effect_class_binding)
        .collect::<Result<Vec<_>, _>>()?;
    let mut rules = Vec::with_capacity(document.rules.len());
    for rule in &document.rules {
        let patterns = rule
            .patterns
            .iter()
            .map(toxic_effect_pattern)
            .collect::<Result<Vec<_>, _>>()?;
        let flows = rule
            .flows
            .iter()
            .map(toxic_flow_requirement)
            .collect::<Result<Vec<_>, _>>()?;
        rules.push(ToxicCombinationRule {
            identity: parse_hash(&rule.identity)?,
            descriptor: pin(&rule.descriptor)?,
            patterns: arena.alloc_slice_copy(&patterns),
            flows: arena.alloc_slice_copy(&flows),
        });
    }
    Ok(HazardClosurePolicy {
        schema_version: document.schema_version,
        identity: parse_hash(&document.identity)?,
        descriptor: pin(&document.descriptor)?,
        permit_class: pin(&document.permit_class)?,
        classes: arena.alloc_slice_copy(&classes),
        rules: arena.alloc_slice_copy(&rules),
        limits: document.limits.into(),
    })
}

fn effect_flow_binding(
    document: &EffectFlowBindingDocument,
) -> Result<EffectFlowBinding<'_>, CompileError> {
    Ok(EffectFlowBinding {
        from_effect: id(&document.from_effect)?,
        to_effect: id(&document.to_effect)?,
        transfer: pin(&document.transfer)?,
    })
}

fn hazard_permit<'a>(
    document: &'a HazardPermitDocument,
    arena: &'a Bump,
    now: AuthorityTime<'a>,
) -> Result<HazardPermit<'a>, CompileError> {
    let subject = administrative_subject(&document.approval.proposal.subject)?;
    let approval = administrative_proof(&document.approval, subject, arena, now).map_err(|_| {
        CompileError::new(CompileReason::HazardClosure(
            HazardClosureReason::PermitApprovalInvalid,
        ))
    })?;
    Ok(HazardPermit {
        identity: parse_hash(&document.identity)?,
        descriptor: pin(&document.descriptor)?,
        policy_identity: parse_hash(&document.policy_identity)?,
        rule_identity: parse_hash(&document.rule_identity)?,
        plan_subject: parse_hash(&document.plan_subject)?,
        epoch: document.epoch,
        scope_identity: parse_hash(&document.scope_identity)?,
        time_basis: id(&document.time_basis)?,
        not_before_tick: document.not_before_tick,
        expires_at_tick: document.expires_at_tick,
        approval,
    })
}

fn plan_hazard_closure<'a>(
    document: &'a HazardClosureDocument,
    authorities: &'a [PlanAuthority<'a>],
    arena: &'a Bump,
    now: AuthorityTime<'a>,
) -> Result<PlanHazardClosure<'a>, CompileError> {
    let policy = hazard_closure_policy(&document.policy, arena)?;
    let flows = document
        .flows
        .iter()
        .map(effect_flow_binding)
        .collect::<Result<Vec<_>, _>>()?;
    let permits = document
        .permits
        .iter()
        .map(|permit| hazard_permit(permit, arena, now))
        .collect::<Result<Vec<_>, _>>()?;
    let flows = arena.alloc_slice_copy(&flows);
    let permits = arena.alloc_slice_copy(&permits);
    let plan_subject = parse_hash(&document.plan_subject)?;
    let mut proof = vec![None::<HazardProofNode<'a>>; MAX_HAZARD_PROOF_NODES];
    let report = analyze_effect_closure(
        policy,
        authorities,
        flows,
        permits,
        HazardClosureContext {
            plan_subject,
            epoch: document.epoch,
            time: now,
        },
        &mut proof,
    )
    .map_err(|denial| CompileError::hazard(denial, &proof))?;
    let decision_identity = parse_hash(&document.decision_identity)?;
    if report.decision_identity != decision_identity {
        return Err(CompileError::new(CompileReason::HazardClosure(
            HazardClosureReason::IdentityMismatch,
        )));
    }
    let hazardous_hosts = document
        .hazardous_hosts
        .iter()
        .map(|binding| hazardous_host_binding(binding, arena))
        .collect::<Result<Vec<_>, CompileError>>()?;
    Ok(PlanHazardClosure {
        epoch: document.epoch,
        plan_subject,
        policy,
        flows,
        permits,
        decision_identity,
        hazardous_hosts: arena.alloc_slice_copy(&hazardous_hosts),
    })
}

fn hazardous_host_binding<'a>(
    document: &'a HazardousHostBindingDocument,
    arena: &'a Bump,
) -> Result<HazardousHostBinding<'a>, CompileError> {
    let envelope = document
        .profile
        .envelope
        .iter()
        .map(|limit| {
            Ok(OperatingEnvelopeLimit {
                dimension: pin(&limit.dimension)?,
                minimum: limit.minimum,
                maximum: limit.maximum,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let profile = HazardousHostProfile {
        schema_version: document.profile.schema_version,
        identity: parse_hash(&document.profile.identity)?,
        descriptor: pin(&document.profile.descriptor)?,
        safe_state: pin(&document.profile.safe_state)?,
        inhibit_boundary: pin(&document.profile.inhibit_boundary)?,
        watchdog: pin(&document.profile.watchdog)?,
        effect_boundary: pin(&document.profile.effect_boundary)?,
        command_effect_class: pin(&document.profile.command_effect_class)?,
        clear_effect_class: pin(&document.profile.clear_effect_class)?,
        clear_operation: pin(&document.profile.clear_operation)?,
        clear_ceremony: pin(&document.profile.clear_ceremony)?,
        time_basis: id(&document.profile.time_basis)?,
        maximum_command_horizon_ticks: document.profile.maximum_command_horizon_ticks,
        maximum_observation_age_ticks: document.profile.maximum_observation_age_ticks,
        maximum_evidence_records: document.profile.maximum_evidence_records,
        require_physical_presence_to_clear: document.profile.require_physical_presence_to_clear,
        require_isolated_implementation: document.profile.require_isolated_implementation,
        envelope: arena.alloc_slice_copy(&envelope),
    };
    let observation = InhibitObservation {
        schema_version: document.observation.schema_version,
        identity: parse_hash(&document.observation.identity)?,
        profile_identity: parse_hash(&document.observation.profile_identity)?,
        host: id(&document.observation.host)?,
        safe_state: pin(&document.observation.safe_state)?,
        inhibit_boundary: pin(&document.observation.inhibit_boundary)?,
        watchdog: pin(&document.observation.watchdog)?,
        effect_boundary: pin(&document.observation.effect_boundary)?,
        time_basis: id(&document.observation.time_basis)?,
        observed_at_tick: document.observation.observed_at_tick,
        valid_until_tick: document.observation.valid_until_tick,
        latch_generation: document.observation.latch_generation,
        latch_state: match document.observation.latch_state.as_str() {
            "safe-disarmed" => InhibitLatchState::SafeDisarmed,
            "inhibited" => InhibitLatchState::Inhibited,
            _ => return Err(CompileError::new(CompileReason::PlanInvalid)),
        },
        independent_from_plan: document.observation.independent_from_plan,
        local_safe_path: document.observation.local_safe_path,
        survives_executor_loss: document.observation.survives_executor_loss,
        survives_partition: document.observation.survives_partition,
        graph_cannot_replace: document.observation.graph_cannot_replace,
        confinement: match document.observation.confinement.as_str() {
            "effect-boundary-enforced" => ImplementationConfinement::EffectBoundaryEnforced,
            "unconfined-native" => ImplementationConfinement::UnconfinedNative,
            _ => return Err(CompileError::new(CompileReason::PlanInvalid)),
        },
    };
    Ok(HazardousHostBinding {
        host: id(&document.host)?,
        profile,
        observation,
    })
}

fn persistent_budget_policy(
    document: &PersistentBudgetPolicyDocument,
) -> Result<PersistentBudgetPolicy<'_>, CompileError> {
    let rolling = match (
        document.limits.rolling_units,
        document.limits.rolling_window_ticks,
    ) {
        (Some(units), Some(window_ticks)) => Some(RollingLimit {
            units,
            window_ticks,
        }),
        (None, None) => None,
        _ => return Err(CompileError::new(CompileReason::InvalidInput)),
    };
    Ok(PersistentBudgetPolicy {
        schema_version: document.schema_version,
        identity: parse_hash(&document.identity)?,
        descriptor: pin(&document.descriptor)?,
        owner: pin(&document.owner)?,
        subject: pin(&document.subject)?,
        anchor: match document.anchor_kind.as_str() {
            "realm" => PolicyBudgetAnchor::Realm(id(&document.anchor_id)?),
            "host" => PolicyBudgetAnchor::Host(id(&document.anchor_id)?),
            "site" => PolicyBudgetAnchor::Site(id(&document.anchor_id)?),
            _ => return Err(CompileError::new(CompileReason::InvalidInput)),
        },
        action: id(&document.action)?,
        resource_class: pin(&document.resource_class)?,
        time_basis: id(&document.time_basis)?,
        limits: PolicyBudgetLimits {
            current_stock: document.limits.current_stock,
            rolling,
            lifetime: document.limits.lifetime,
        },
        reservation_ttl_ticks: document.reservation_ttl_ticks,
        lease: document
            .lease
            .as_ref()
            .map(|lease| {
                Ok(PolicyLeaseRule {
                    maximum_ticks: lease.maximum_ticks,
                    renewal_authority: pin(&lease.renewal_authority)?,
                    offline_allowed: lease.offline_allowed,
                })
            })
            .transpose()?,
        audit_id: id(&document.audit_id)?,
        persistence_profile: pin(&document.persistence_profile)?,
        maximum_reservations: document.maximum_reservations,
        maximum_evidence_events: document.maximum_evidence_events,
    })
}

fn policy_budget_status(
    document: &PolicyBudgetStatusDocument,
) -> Result<PolicyBudgetStatus<'_>, CompileError> {
    Ok(PolicyBudgetStatus {
        schema_version: document.schema_version,
        identity: parse_hash(&document.identity)?,
        policy_identity: parse_hash(&document.policy_identity)?,
        ledger: pin(&document.ledger)?,
        checkpoint: parse_hash(&document.checkpoint)?,
        sequence: document.sequence,
        current_stock: document.current_stock,
        rolling_window_start: document.rolling_window_start,
        rolling_committed: document.rolling_committed,
        lifetime_committed: document.lifetime_committed,
        reserved: document.reserved,
        evidence_remaining: document.evidence_remaining,
        availability: match document.availability.as_str() {
            "available" => PolicyBudgetAvailability::Available,
            "unavailable" => PolicyBudgetAvailability::Unavailable,
            "retention-gap" => PolicyBudgetAvailability::RetentionGap,
            _ => return Err(CompileError::new(CompileReason::InvalidInput)),
        },
        time_basis: id(&document.time_basis)?,
        observed_at_tick: document.observed_at_tick,
        valid_until_tick: document.valid_until_tick,
    })
}

fn policy_budget_lease(
    document: &PolicyBudgetLeaseDocument,
) -> Result<PolicyBudgetLease<'_>, CompileError> {
    Ok(PolicyBudgetLease {
        schema_version: document.schema_version,
        identity: parse_hash(&document.identity)?,
        policy_identity: parse_hash(&document.policy_identity)?,
        holder: pin(&document.holder)?,
        renewal_authority: pin(&document.renewal_authority)?,
        time_basis: id(&document.time_basis)?,
        issued_at_tick: document.issued_at_tick,
        expires_at_tick: document.expires_at_tick,
        offline: document.offline,
    })
}

fn policy_budget_binding(
    document: &PolicyBudgetBindingDocument,
) -> Result<PlanPolicyBudget<'_>, CompileError> {
    Ok(PlanPolicyBudget {
        policy: persistent_budget_policy(&document.policy)?,
        status: policy_budget_status(&document.status)?,
        lease: document
            .lease
            .as_ref()
            .map(policy_budget_lease)
            .transpose()?,
        required_units: document.required_units,
        check_at_use: document.check_at_use,
    })
}

fn host_capability(document: &HostCapabilityDocument) -> Result<HostCapability<'_>, CompileError> {
    Ok(HostCapability {
        id: id(&document.id)?,
        action: id(&document.action)?,
        resource: ResourceRef {
            kind: id(&document.resource_kind)?,
            id: id(&document.resource_id)?,
        },
        host: id(&document.host)?,
        time_basis: id(&document.time_basis)?,
        observed_at_tick: document.observed_at_tick,
        valid_until_tick: document.valid_until_tick,
    })
}

fn authority_grant<'a>(
    document: &'a AuthorityGrantDocument,
    arena: &'a Bump,
) -> Result<AuthorityGrant<'a>, CompileError> {
    Ok(AuthorityGrant {
        id: id(&document.id)?,
        action: id(&document.action)?,
        resource: ResourceRef {
            kind: id(&document.resource_kind)?,
            id: id(&document.resource_id)?,
        },
        scope: AuthorityScope {
            root: instance(&document.scope_root)?,
            descendants: document.scope_descendants,
        },
        audience: id(&document.audience)?,
        constraints: authority_constraints(&document.constraints, arena)?,
        time_basis: id(&document.time_basis)?,
        not_before_tick: document.not_before_tick,
        expires_at_tick: document.expires_at_tick,
        issued_for_host: id(&document.issued_for_host)?,
        delegation: delegation_policy(&document.delegation)?,
        audit_id: id(&document.audit_id)?,
        terminal_policy: stop_policy(&document.terminal_policy)?,
    })
}

fn implementation_identity(document: &ImplementationDocument) -> Result<String, CompileError> {
    let arena = Bump::new();
    let refs = document
        .artifacts
        .iter()
        .map(|artifact| {
            Ok(ManifestArtifactRef {
                id: id(&artifact.id)?,
                digest: parse_digest(&artifact.digest)?,
                role: id(&artifact.role)?,
                required: artifact.required,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let required_interfaces = implementation_interfaces(&document.required_interfaces, &arena)?;
    let provided_interfaces = implementation_interfaces(&document.provided_interfaces, &arena)?;
    let manifest = ImplementationManifest {
        schema_version: document.schema_version,
        identity: SemanticHash::from_bytes([0; 32]),
        id: id(&document.id)?,
        implementation_version: &document.implementation_version,
        semantic_contract: pin(&document.semantic_contract)?,
        executor: executor(&document.executor)?,
        entrypoint: ManifestEntrypoint {
            name: id(&document.entrypoint_name)?,
            adapter: id(&document.entrypoint_adapter)?,
            abi: id(&document.entrypoint_abi)?,
            protocol_version: document.runtime_protocol_version,
        },
        execution_profile: pin(&document.execution_profile)?,
        artifacts: arena.alloc_slice_copy(&refs),
        required_interfaces,
        provided_interfaces,
        required_authorities: arena.alloc_slice_copy(
            &document
                .required_authorities
                .iter()
                .map(|hash| parse_hash(hash))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        required_effects: arena.alloc_slice_copy(
            &document
                .required_effects
                .iter()
                .map(|hash| parse_hash(hash))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        minimum_plan_version: document.minimum_plan_version,
        maximum_plan_version: document.maximum_plan_version,
        minimum_runtime_protocol: document.minimum_runtime_protocol,
        maximum_runtime_protocol: document.maximum_runtime_protocol,
        replacement: ReplacementSupport::Cold,
        coexistence_memory_bytes: document.coexistence_memory_bytes,
        reproducibility: None,
    };
    let mut scratch = vec![SemanticHash::from_bytes([0; 32]); manifest.identity_fact_count()];
    manifest
        .computed_semantic_hash(&mut scratch)
        .map(|hash| hash.to_string())
        .map_err(|_| CompileError::new(CompileReason::InvalidInput))
}

fn report_identity(document: &HostReportDocument) -> Result<String, CompileError> {
    let arena = Bump::new();
    let mut document = document.clone();
    document.identity = SemanticHash::from_bytes([0; 32]).to_string();
    let report = capability_report(&document, &arena)?;
    let mut scratch = vec![SemanticHash::from_bytes([0; 32]); report.identity_fact_count()];
    report
        .computed_semantic_hash(&mut scratch)
        .map(|hash| hash.to_string())
        .map_err(|_| CompileError::new(CompileReason::InvalidInput))
}

fn resource_lease_document(lease: ResourceLeaseContract<'_>) -> ResourceLeaseDocument {
    let (sharing, maximum_holders) = match lease.sharing {
        ResourceSharingMode::Exclusive => ("exclusive", 1),
        ResourceSharingMode::SharedRead => ("shared-read", u16::MAX),
        ResourceSharingMode::SharedBounded { maximum_holders } => {
            ("shared-bounded", maximum_holders)
        }
    };
    let (foreign_retention, foreign_maximum_bytes, foreign_release_ticks) =
        match lease.foreign_retention {
            ForeignRetention::None => ("none", 0, 0),
            ForeignRetention::Bounded {
                maximum_bytes,
                release_ticks,
            } => ("bounded", maximum_bytes, release_ticks),
            ForeignRetention::ObservedOnly => ("observed-only", 0, 0),
            ForeignRetention::Unsupported => ("unsupported", 0, 0),
        };
    ResourceLeaseDocument {
        schema_version: lease.schema_version,
        id: lease.id.to_string(),
        resource_binding: lease.resource_binding.to_string(),
        holder: lease.holder.as_str().to_owned(),
        run: lease.run.to_string(),
        epoch: lease.epoch,
        scope: lease.scope.to_string(),
        sharing: sharing.to_owned(),
        maximum_holders,
        reservation: lease.reservation.into(),
        time_basis: lease.time_basis.to_string(),
        issued_at_tick: lease.issued_at_tick,
        expires_at_tick: lease.expires_at_tick,
        revocation_grace_ticks: lease.revocation_grace_ticks,
        cleanup_ticks: lease.cleanup_ticks,
        maximum_operations: lease.maximum_operations,
        maximum_evidence_events: lease.maximum_evidence_events,
        cleanup_escalation: pin_document(lease.cleanup_escalation),
        foreign_retention: foreign_retention.to_owned(),
        foreign_maximum_bytes,
        foreign_release_ticks,
    }
}

fn effect_commit_profile_document(profile: EffectCommitProfile<'_>) -> EffectCommitProfileDocument {
    EffectCommitProfileDocument {
        schema_version: profile.schema_version,
        id: profile.id.to_string(),
        operation: profile.operation.to_string(),
        resource_lease: profile.resource_lease.to_string(),
        commit_boundary: pin_document(profile.commit_boundary),
        idempotency: match profile.idempotency {
            EffectIdempotency::None => "none",
            EffectIdempotency::SameKeySameEffect => "same-key-same-effect",
            EffectIdempotency::ReconcileBeforeRetry => "reconcile-before-retry",
        }
        .to_owned(),
        unknown_commit: match profile.unknown_commit {
            UnknownCommitPolicy::Fail => "fail",
            UnknownCommitPolicy::Reconcile => "reconcile",
            UnknownCommitPolicy::RetrySameIdempotencyKey => "retry-same-idempotency-key",
        }
        .to_owned(),
        discontinuity: match profile.discontinuity {
            EffectDiscontinuity::FailedBeforeCommit => "failed-before-commit",
            EffectDiscontinuity::CommitUnknown => "commit-unknown",
            EffectDiscontinuity::ReconcileRequired => "reconcile-required",
        }
        .to_owned(),
        cleanup: pin_document(profile.cleanup),
        maximum_attempts: profile.maximum_attempts,
        evidence_events_per_attempt: profile.evidence_events_per_attempt,
    }
}

fn workload_limit_document(limit: WorkloadLimit) -> Option<u64> {
    match limit {
        WorkloadLimit::Finite(value) => Some(value),
        WorkloadLimit::Unsupported => None,
    }
}

fn workload_budget_document(budget: WorkloadBudget) -> WorkloadBudgetDocument {
    WorkloadBudgetDocument {
        work_units: workload_limit_document(budget.work_units),
        tasks: workload_limit_document(budget.tasks),
        processes: workload_limit_document(budget.processes),
        descriptors: workload_limit_document(budget.descriptors),
        connections: workload_limit_document(budget.connections),
        storage_bytes: workload_limit_document(budget.storage_bytes),
        device_operations: workload_limit_document(budget.device_operations),
        network_bytes: workload_limit_document(budget.network_bytes),
        callbacks: workload_limit_document(budget.callbacks),
        foreign_queue_items: workload_limit_document(budget.foreign_queue_items),
        transition_overlap_work_units: workload_limit_document(
            budget.transition_overlap_work_units,
        ),
    }
}

fn workload_document(workload: PlanWorkload<'_>) -> PlanWorkloadDocument {
    PlanWorkloadDocument {
        contract: WorkloadContractDocument {
            schema_version: workload.contract.schema_version,
            id: workload.contract.id.to_string(),
            service: workload.contract.service.to_string(),
            node: workload.contract.node.as_str().to_owned(),
            guarantee: workload.contract.guarantee.as_str().to_owned(),
            budget: workload_budget_document(workload.contract.budget),
            deadline: workload
                .contract
                .deadline
                .map(|deadline| DeadlineContractDocument {
                    time_basis: deadline.time_basis.to_string(),
                    relative_deadline_ticks: deadline.relative_deadline_ticks,
                    maximum_jitter_ticks: deadline.maximum_jitter_ticks,
                }),
            maximum_evidence_events: workload.contract.maximum_evidence_events,
        },
        capability: WorkloadCapabilityDocument {
            id: workload.capability.id.to_string(),
            identity: workload.capability.identity.to_string(),
            host_observation: workload.capability.host_observation.to_string(),
            evidence_kind: workload.capability.evidence_kind.as_str().to_owned(),
            time_basis: workload.capability.time_basis.to_string(),
            observed_at_tick: workload.capability.observed_at_tick,
            valid_until_tick: workload.capability.valid_until_tick,
            capacity: workload_budget_document(workload.capability.capacity),
            maximum_deadline_ticks: workload.capability.maximum_deadline_ticks,
            maximum_jitter_ticks: workload.capability.maximum_jitter_ticks,
        },
    }
}

fn resolved_execution_pin_document(
    pin: &conduit_runtime::ResolvedExecutionDescriptor,
) -> PinDocument {
    PinDocument {
        id: pin.id.clone(),
        schema_version: pin.schema_version,
        semantic_hash: pin.semantic_hash.to_string(),
    }
}

fn resolved_execution_arrangement_document(
    arrangement: &ResolvedExecutionArrangement,
) -> ResolvedExecutionArrangementDocument {
    ResolvedExecutionArrangementDocument {
        identity: arrangement.identity.to_string(),
        plan_identity: arrangement.plan_identity.to_string(),
        resolution_identity: arrangement.resolution_identity.to_string(),
        plan_epoch: arrangement.plan_epoch,
        placements: arrangement
            .placements
            .iter()
            .map(|placement| ResolvedExecutionPlacementDocument {
                id: placement.id.clone(),
                host_observation: placement.host_observation.clone(),
                provider: resolved_execution_pin_document(&placement.provider),
                authority_boundary: resolved_execution_pin_document(&placement.authority_boundary),
                resource_boundary: resolved_execution_pin_document(&placement.resource_boundary),
                lifecycle_boundary: resolved_execution_pin_document(&placement.lifecycle_boundary),
                failure_boundary: resolved_execution_pin_document(&placement.failure_boundary),
                generation: placement.generation,
                isolation: placement.isolation.as_str().to_owned(),
                memory_containment: placement.memory_containment.as_str().to_owned(),
                regain_control: placement.regain_control.as_str().to_owned(),
                effect_fencing: placement.effect_fencing.as_str().to_owned(),
                stop_execution: placement.stop_execution.as_str().to_owned(),
                reclaim_resources: placement.reclaim_resources.as_str().to_owned(),
                maximum_regain_control_ticks: placement.maximum_regain_control_ticks,
            })
            .collect(),
        lanes: arrangement
            .lanes
            .iter()
            .map(|lane| ResolvedExecutionLaneDocument {
                id: lane.id.clone(),
                placement: lane.placement.clone(),
                placement_generation: lane.placement_generation,
                generation: lane.generation,
                independent_progress: lane.independent_progress.as_str().to_owned(),
                simultaneous_execution: lane.simultaneous_execution.as_str().to_owned(),
                preemption: lane.preemption.as_str().to_owned(),
                termination: lane.termination.as_str().to_owned(),
                ready_slots: lane.ready_slots,
                wake_slots: lane.wake_slots,
                proposal_slots: lane.proposal_slots,
                commit_slots: lane.commit_slots,
                timer_slots: lane.timer_slots,
                scratch_bytes: lane.scratch_bytes,
                stack_bytes: lane.stack_bytes,
                evidence_slots: lane.evidence_slots,
            })
            .collect(),
        regions: arrangement
            .regions
            .iter()
            .map(|region| ResolvedExecutionRegionDocument {
                id: region.id.clone(),
                members: region.members.clone(),
                placement: region.placement.clone(),
                placement_generation: region.placement_generation,
                lane: region.lane.clone(),
                lane_generation: region.lane_generation,
                commit_domain: region.commit_domain.clone(),
                independent: region.independent,
                maximum_in_flight_proposals: region.maximum_in_flight_proposals,
                scratch_bytes: region.scratch_bytes,
                retained_state_bytes: region.retained_state_bytes,
                pending_operation_slots: region.pending_operation_slots,
                timer_slots: region.timer_slots,
                evidence_slots: region.evidence_slots,
            })
            .collect(),
        boundaries: arrangement
            .boundaries
            .iter()
            .map(|boundary| ResolvedExecutionBoundaryDocument {
                cord: boundary.cord.clone(),
                from_region: boundary.from_region.clone(),
                to_region: boundary.to_region.clone(),
                realization: resolved_execution_pin_document(&boundary.realization),
                generation: boundary.generation,
                from_placement_generation: boundary.from_placement_generation,
                to_placement_generation: boundary.to_placement_generation,
                capacity_items: boundary.capacity_items,
                capacity_bytes: boundary.capacity_bytes,
                wake_slots: boundary.wake_slots,
                evidence_slots: boundary.evidence_slots,
            })
            .collect(),
        commit_domains: arrangement
            .commit_domains
            .iter()
            .map(|domain| ResolvedExecutionCommitDomainDocument {
                id: domain.id.clone(),
                ordering: domain.ordering.as_str().to_owned(),
                proposal_slots: domain.proposal_slots,
                commit_slots: domain.commit_slots,
                maximum_proposal_bytes: domain.maximum_proposal_bytes,
                maximum_head_of_line_ticks: domain.maximum_head_of_line_ticks,
                cancellation_slots: domain.cancellation_slots,
                evidence_slots: domain.evidence_slots,
            })
            .collect(),
    }
}

fn plan_document(
    plan: &ExecutionPlan<'_>,
    topology: &ExactTopologyView,
    execution_arrangement: &ResolvedExecutionArrangement,
) -> Result<ExactPlanDocument, CompileError> {
    let mut hosts = plan
        .host_observations
        .iter()
        .map(|host| PlanHostDocument {
            id: host.id.to_string(),
            host: host.host.to_string(),
            boot_id: host.boot_id.to_string(),
            semantic_hash: host.semantic_hash.to_string(),
            time_basis: host.time_basis.to_string(),
            observed_at_tick: host.observed_at_tick,
            valid_until_tick: host.valid_until_tick,
        })
        .collect::<Vec<_>>();
    hosts.sort_by(|left, right| left.id.cmp(&right.id));
    let mut artifacts = plan
        .artifacts
        .iter()
        .map(|artifact| PlanArtifactDocument {
            id: artifact.id.to_string(),
            digest: artifact.digest.to_string(),
        })
        .collect::<Vec<_>>();
    artifacts.sort_by(|left, right| left.id.cmp(&right.id));
    let mut resources = plan
        .resources
        .iter()
        .map(|resource| PlanResourceDocument {
            id: resource.id.to_string(),
            node: resource.node.as_str().to_owned(),
            kind: resource.resource.kind.to_string(),
            resource: resource.resource.id.to_string(),
            host_observation: resource.host_observation.to_string(),
            lease: resource.lease.map(resource_lease_document),
        })
        .collect::<Vec<_>>();
    resources.sort_by(|left, right| left.id.cmp(&right.id));
    let mut workloads = plan
        .workloads
        .iter()
        .copied()
        .map(workload_document)
        .collect::<Vec<_>>();
    workloads.sort_by(|left, right| left.contract.id.cmp(&right.contract.id));
    let mut nodes = plan
        .nodes
        .iter()
        .map(|node| PlanNodeDocument {
            instance: node.instance.as_str().to_owned(),
            contract: pin_document(node.contract),
            implementation: pin_document(node.implementation),
            lifecycle_policy: pin_document(node.lifecycle_policy),
            execution_profile: execution_profile_document(
                node.execution_profile
                    .expect("schema-3 compile plans always carry execution profiles"),
            ),
            artifact: node.artifact.to_string(),
            host_observation: node.host_observation.to_string(),
            host: node.host.to_string(),
            allocation: node.allocation.into(),
            required_resources: node
                .required_resources
                .iter()
                .map(ToString::to_string)
                .collect(),
            required_effects: node
                .required_effects
                .iter()
                .map(ToString::to_string)
                .collect(),
        })
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.instance.cmp(&right.instance));
    let cords = topology
        .cords
        .iter()
        .map(|cord| {
            let plan_cord = plan
                .cords
                .iter()
                .find(|candidate| candidate.id.as_str() == cord.id)
                .ok_or_else(|| CompileError::new(CompileReason::PlanInvalid))?;
            Ok(PlanCordDocument {
                id: cord.id.clone(),
                from: port_to_document(plan_cord.from),
                to: port_to_document(plan_cord.to),
                capacity_items: cord.capacity_items,
                max_value_bytes: cord.max_value_bytes,
                max_queued_bytes: cord.max_queued_bytes,
                low_watermark_items: cord.low_watermark_items,
                high_watermark_items: cord.high_watermark_items,
                pressure: pressure_document(&cord.pressure),
                queue_memory_bytes: plan_cord.queue_memory_bytes,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let composites = plan
        .composites
        .iter()
        .map(|composite| PlanCompositeDocument {
            instance: composite.instance.as_str().to_owned(),
            definition_hash: composite.definition_hash.to_string(),
            members: composite
                .members
                .iter()
                .map(|member| member.as_str().to_owned())
                .collect(),
            exports: composite
                .exports
                .iter()
                .map(|export| PlanExportDocument {
                    boundary_port: export.boundary_port.to_string(),
                    member: export.member.as_str().to_owned(),
                    member_port: export.member_port.to_string(),
                    direction: export.direction.as_str().to_owned(),
                })
                .collect(),
        })
        .collect();
    let authorities = plan
        .authorities
        .iter()
        .map(|authority| PlanAuthorityDocument {
            node: authority.node.as_str().to_owned(),
            effect_hash: authority.effect_hash.to_string(),
            grant_hash: authority.grant_hash.to_string(),
            effect: effect_to_document(authority.effect),
            capability: capability_to_document(authority.capability),
            grant: grant_to_document(authority.grant),
            binding: PlanAuthorityBindingDocument {
                effect_id: authority.binding.effect_id.to_string(),
                capability_id: authority.binding.capability_id.to_string(),
                grant_id: authority.binding.grant_id.to_string(),
                resource_kind: authority.binding.resource.kind.to_string(),
                resource_id: authority.binding.resource.id.to_string(),
                host: authority.binding.host.to_string(),
                audit_id: authority.binding.audit_id.to_string(),
                time_basis: authority.binding.time_basis.to_string(),
                validated_at_tick: authority.binding.validated_at_tick,
                check_at_use: authority.binding.check_at_use,
            },
            administrative_subject: authority
                .administrative_subject
                .map(administrative_subject_document),
            containment: authority.containment.map(administrative_proof_document),
            policy_budgets: authority
                .policy_budgets
                .iter()
                .copied()
                .map(policy_budget_binding_document)
                .collect(),
            commit_profile: authority.commit_profile.map(effect_commit_profile_document),
        })
        .collect();
    let hazard_closure = plan.hazard_closure.map(hazard_closure_document);
    let port_groups = plan
        .port_groups
        .iter()
        .map(|group| PlanPortGroupDocument {
            instance: group.instance.as_str().to_owned(),
            template_hash: group.template_hash.to_string(),
            maximum: group.maximum,
            direction: group.direction.as_str().to_owned(),
            members: group
                .members
                .iter()
                .map(|member| PlanPortGroupMemberDocument {
                    id: member.id.to_string(),
                    ordinal: member.ordinal,
                    port_contract_hash: member.port_contract_hash.to_string(),
                })
                .collect(),
        })
        .collect();
    let instance_pools = plan
        .instance_pools
        .iter()
        .map(|pool| PlanInstancePoolDocument {
            instance: pool.instance.as_str().to_owned(),
            template_hash: pool.template_hash.to_string(),
            derived_identity_hash: pool.derived_identity_hash.to_string(),
            maximum_live: pool.maximum_live,
            maximum_queued: pool.maximum_queued,
            admission_policy: pin_document(pool.admission_policy),
            supervision_policy: pin_document(pool.supervision_policy),
            per_instance_budget: pool.per_instance_budget.into(),
            authority_grants: pool
                .authority_grants
                .iter()
                .map(ToString::to_string)
                .collect(),
            maximum_instance_ticks: pool.maximum_instance_ticks,
            implementation_set_hash: pool.implementation_set_hash.to_string(),
            correlation_slots: pool.correlation_slots,
            worst_case_budget: pool.worst_case_budget.into(),
            child_nodes: pool.child_nodes,
            child_cords: pool.child_cords,
            runtime: pool.runtime.map(plan_pool_runtime_document),
        })
        .collect();
    let supervisions = plan
        .supervisions
        .iter()
        .map(|supervision| SupervisionBindingDocument {
            instance: supervision.instance.as_str().to_owned(),
            source_binding_hash: supervision.source_binding_hash.to_string(),
            id: supervision.contract.id.to_string(),
            scope: match supervision.contract.scope {
                SupervisionScope::Child => "child",
                SupervisionScope::NamedGroup => "named-group",
                SupervisionScope::CompositeBoundary => "composite-boundary",
                SupervisionScope::ReplicatedChild => "replicated-child",
            }
            .to_owned(),
            subject: supervision.contract.subject.as_str().to_owned(),
            handler: supervision.contract.handler.as_str().to_owned(),
            members: supervision
                .contract
                .members
                .iter()
                .map(|member| member.as_str().to_owned())
                .collect(),
            failure_mode: match supervision.contract.failure_mode {
                SupervisionFailureMode::FailTogether => "fail-together",
                SupervisionFailureMode::IsolatedOptional => "isolated-optional",
            }
            .to_owned(),
            outer: supervision.contract.outer.map(|value| value.to_string()),
            policy: pin_document(supervision.policy),
            observation_contract: pin_document(supervision.observation_contract),
            decision_contract: pin_document(supervision.decision_contract),
            actions: supervision
                .contract
                .actions
                .iter()
                .map(|action| SupervisionActionDocument {
                    kind: match action.kind {
                        SupervisionActionKind::Propagate => "propagate",
                        SupervisionActionKind::StopScope => "stop-scope",
                        SupervisionActionKind::RestartSame => "restart-same",
                        SupervisionActionKind::RetrySame => "retry-same",
                        SupervisionActionKind::ActivateDeclaredFallback => {
                            "activate-declared-fallback"
                        }
                        SupervisionActionKind::ContinueDeclaredDegradedMode => {
                            "continue-declared-degraded-mode"
                        }
                        SupervisionActionKind::RequestOperatorAction => "request-operator-action",
                    }
                    .to_owned(),
                    target: action.target.map(|value| value.to_string()),
                    maximum_uses: action.maximum_uses,
                    permits_effect_replay: action.permits_effect_replay,
                    preserves_required_guarantees: action.preserves_required_guarantees,
                    requires_new_epoch: action.requires_new_epoch,
                })
                .collect(),
            action_targets: supervision
                .action_targets
                .iter()
                .map(|target| SupervisionTargetDocument {
                    choice: target.choice.to_string(),
                    target: target.target.as_str().to_owned(),
                })
                .collect(),
            limits: supervision.contract.limits.into(),
            allocation: supervision.allocation.into(),
            deadline_timer: supervision.deadline_timer.to_string(),
            backoff_timer: supervision.backoff_timer.to_string(),
            cooldown_timer: supervision.cooldown_timer.to_string(),
            cleanup: match supervision.contract.cleanup {
                StopPolicy::Drain => "drain",
                StopPolicy::Abort => "abort",
            }
            .to_owned(),
            required_behavior: supervision.contract.required_behavior,
        })
        .collect();
    let value_envelopes = plan
        .value_envelopes
        .iter()
        .map(|policy| ValueEnvelopePolicyDocument {
            cord: policy.cord.to_string(),
            representation: pin_document(policy.representation),
            maximum_payload_bytes: policy.maximum_payload_bytes,
            maximum_envelope_bytes: policy.maximum_envelope_bytes,
            maximum_fragments: policy.maximum_fragments,
            maximum_fragment_bytes: policy.maximum_fragment_bytes,
            maximum_timestamps: policy.maximum_timestamps,
            clock_domains: policy
                .clock_domains
                .iter()
                .map(ToString::to_string)
                .collect(),
            identity_allowed: policy.identity_allowed,
            correlation_allowed: policy.correlation_allowed,
            causation_allowed: policy.causation_allowed,
            provenance_allowed: policy.provenance_allowed,
            sensitivity_ceiling: match policy.sensitivity_ceiling {
                Sensitivity::Public => "public",
                Sensitivity::Restricted => "restricted",
                Sensitivity::Secret => "secret",
            }
            .to_owned(),
        })
        .collect();
    let watch_admissions = plan
        .watch_admissions
        .iter()
        .map(|watch| {
            let (subject_kind, cord, node, port, direction) = match watch.subject {
                WatchSubject::Cord(cord) => {
                    ("cord".to_owned(), Some(cord.to_string()), None, None, None)
                }
                WatchSubject::NodePort {
                    node,
                    port,
                    direction,
                } => (
                    "node-port".to_owned(),
                    None,
                    Some(node.as_str().to_owned()),
                    Some(port.to_string()),
                    Some(direction.as_str().to_owned()),
                ),
            };
            WatchAdmissionDocument {
                id: watch.id.to_string(),
                subject_kind,
                operator: watch.operator.to_string(),
                control_grant_hash: watch.control_grant_hash.to_string(),
                lease: watch.lease.to_string(),
                cord,
                node,
                port,
                direction,
                representation: pin_document(watch.representation),
                maximum_preview_bytes: watch.maximum_preview_bytes,
                maximum_history: watch.maximum_history,
                minimum_tick_interval: watch.minimum_tick_interval,
                retention: watch.retention.as_str().to_owned(),
                sensitivity_ceiling: watch.sensitivity_ceiling.as_str().to_owned(),
                reveal_action: watch.reveal_action.map(|action| action.to_string()),
                reveal_grant_hash: watch.reveal_grant_hash.map(|hash| hash.to_string()),
            }
        })
        .collect();
    let clock_conversions = plan
        .clock_conversions
        .iter()
        .map(|conversion| ClockConversionDocument {
            id: conversion.id.to_string(),
            source: conversion.source.to_string(),
            destination: conversion.destination.to_string(),
            numerator: conversion.numerator,
            denominator: conversion.denominator,
            offset_ticks: conversion.offset_ticks,
            rounding: match conversion.rounding {
                ClockRounding::Exact => "exact",
                ClockRounding::Floor => "floor",
                ClockRounding::Ceiling => "ceiling",
            }
            .to_owned(),
            maximum_uncertainty_ticks: conversion.maximum_uncertainty_ticks,
            observed_time_basis: conversion.observed_at.basis.to_string(),
            observed_tick: conversion.observed_at.tick,
            valid_until_tick: conversion.valid_until_tick,
            authority: conversion.authority.to_string(),
        })
        .collect();
    let feedback_boundaries = plan
        .feedback_boundaries
        .iter()
        .map(|boundary| FeedbackBoundaryDocument {
            id: boundary.id.to_string(),
            node: boundary.node.as_str().to_owned(),
            cord: boundary.cord.to_string(),
            kind: match boundary.kind {
                FeedbackBoundaryKind::Delay => "delay",
                FeedbackBoundaryKind::State => "state",
            }
            .to_owned(),
            initialization: match boundary.initialization {
                FeedbackInitialization::Empty => "empty",
                FeedbackInitialization::InitialValue => "initial-value",
            }
            .to_owned(),
            initial_items: boundary.initial_items,
            initial_bytes: boundary.initial_bytes,
            maximum_retained_items: boundary.maximum_retained_items,
            maximum_retained_bytes: boundary.maximum_retained_bytes,
            delay_ticks: boundary.delay_ticks,
            clock: boundary.clock.map(|clock| clock.to_string()),
            replay_gap: match boundary.replay_gap {
                FeedbackReplayGapPolicy::Fail => "fail",
                FeedbackReplayGapPolicy::Reset => "reset",
                FeedbackReplayGapPolicy::Wait => "wait",
            }
            .to_owned(),
            cancellation: pin_document(boundary.cancellation),
            terminal: match boundary.terminal {
                FeedbackTerminalPolicy::DropRetained => "drop-retained",
                FeedbackTerminalPolicy::DrainRetained => "drain-retained",
            }
            .to_owned(),
        })
        .collect();
    let evidence_provider = plan
        .evidence_provider
        .map(|provider| {
            let artifact = plan
                .artifacts
                .iter()
                .find(|artifact| artifact.id == provider.artifact)
                .ok_or_else(|| CompileError::new(CompileReason::PlanInvalid))?;
            let host = plan
                .host_observations
                .iter()
                .find(|host| host.id == provider.host_observation)
                .ok_or_else(|| CompileError::new(CompileReason::PlanInvalid))?;
            Ok(EvidenceProviderBindingDocument {
                implementation: pin_document(provider.implementation),
                artifact: PlanArtifactDocument {
                    id: artifact.id.to_string(),
                    digest: artifact.digest.to_string(),
                },
                host_observation: PlanHostDocument {
                    id: host.id.to_string(),
                    host: host.host.to_string(),
                    boot_id: host.boot_id.to_string(),
                    semantic_hash: host.semantic_hash.to_string(),
                    time_basis: host.time_basis.to_string(),
                    observed_at_tick: host.observed_at_tick,
                    valid_until_tick: host.valid_until_tick,
                },
                store_kind: provider.store.kind.to_string(),
                store_id: provider.store.id.to_string(),
                store_generation: provider.store_generation,
                grant_hash: provider.grant_hash.to_string(),
                time_basis: provider.time_basis.to_string(),
            })
        })
        .transpose()?;
    Ok(ExactPlanDocument {
        schema: PLAN_DOCUMENT_SCHEMA.to_owned(),
        schema_version: plan.schema_version,
        identity: plan.identity.to_string(),
        source_semantic_hash: plan.source_semantic_hash.to_string(),
        resolver: pin_document(plan.resolver),
        resolver_policy_hash: plan.resolver_policy_hash.to_string(),
        time_basis: plan.created_at.basis.to_string(),
        created_at_tick: plan.created_at.tick,
        budget: plan.budget.into(),
        execution_arrangement: resolved_execution_arrangement_document(execution_arrangement),
        host_observations: hosts,
        resources,
        workloads,
        artifacts,
        nodes,
        cords,
        value_envelopes,
        watch_admissions,
        clock_conversions,
        feedback_boundaries,
        evidence_provider,
        authorities,
        hazard_closure,
        composites,
        port_groups,
        instance_pools,
        supervisions,
        unresolved_selectors: Vec::new(),
    })
}

fn port_document(document: &PlanPortDocument) -> Result<ResolvedPlanPort<'_>, CompileError> {
    Ok(ResolvedPlanPort {
        node: instance(&document.node)?,
        port: id(&document.port)?,
        direction: direction(&document.direction)?,
        port_contract_hash: parse_hash(&document.port_contract_hash)?,
        value_type: TypeContractRef {
            contract_id: id(&document.value_type_id)?,
            schema_version: document.value_type_schema_version,
            semantic_hash: parse_hash(&document.value_type_semantic_hash)?,
        },
    })
}

fn workload_limit(value: Option<u64>) -> WorkloadLimit {
    value.map_or(WorkloadLimit::Unsupported, WorkloadLimit::Finite)
}

fn workload_budget(document: &WorkloadBudgetDocument) -> WorkloadBudget {
    WorkloadBudget {
        work_units: workload_limit(document.work_units),
        tasks: workload_limit(document.tasks),
        processes: workload_limit(document.processes),
        descriptors: workload_limit(document.descriptors),
        connections: workload_limit(document.connections),
        storage_bytes: workload_limit(document.storage_bytes),
        device_operations: workload_limit(document.device_operations),
        network_bytes: workload_limit(document.network_bytes),
        callbacks: workload_limit(document.callbacks),
        foreign_queue_items: workload_limit(document.foreign_queue_items),
        transition_overlap_work_units: workload_limit(document.transition_overlap_work_units),
    }
}

fn workload_binding(document: &PlanWorkloadDocument) -> Result<PlanWorkload<'_>, CompileError> {
    let contract = &document.contract;
    let capability = &document.capability;
    Ok(PlanWorkload {
        contract: WorkloadContract {
            schema_version: contract.schema_version,
            id: id(&contract.id)?,
            service: id(&contract.service)?,
            node: instance(&contract.node)?,
            guarantee: match contract.guarantee.as_str() {
                "hard" => WorkloadGuarantee::Hard,
                "measured" => WorkloadGuarantee::Measured,
                "host-observed-best-effort" => WorkloadGuarantee::HostObservedBestEffort,
                "unsupported" => WorkloadGuarantee::Unsupported,
                _ => return Err(CompileError::new(CompileReason::PlanInvalid)),
            },
            budget: workload_budget(&contract.budget),
            deadline: contract
                .deadline
                .as_ref()
                .map(|deadline| {
                    Ok(DeadlineContract {
                        time_basis: id(&deadline.time_basis)?,
                        relative_deadline_ticks: deadline.relative_deadline_ticks,
                        maximum_jitter_ticks: deadline.maximum_jitter_ticks,
                    })
                })
                .transpose()?,
            maximum_evidence_events: contract.maximum_evidence_events,
        },
        capability: WorkloadCapability {
            id: id(&capability.id)?,
            identity: parse_hash(&capability.identity)?,
            host_observation: id(&capability.host_observation)?,
            evidence_kind: match capability.evidence_kind.as_str() {
                "exact-enforcement" => WorkloadEvidenceKind::ExactEnforcement,
                "host-observation" => WorkloadEvidenceKind::HostObservation,
                "measurement" => WorkloadEvidenceKind::Measurement,
                "benchmark" => WorkloadEvidenceKind::Benchmark,
                "none" => WorkloadEvidenceKind::None,
                _ => return Err(CompileError::new(CompileReason::PlanInvalid)),
            },
            time_basis: id(&capability.time_basis)?,
            observed_at_tick: capability.observed_at_tick,
            valid_until_tick: capability.valid_until_tick,
            capacity: workload_budget(&capability.capacity),
            maximum_deadline_ticks: capability.maximum_deadline_ticks,
            maximum_jitter_ticks: capability.maximum_jitter_ticks,
        },
    })
}

fn resource_lease(
    document: &ResourceLeaseDocument,
) -> Result<ResourceLeaseContract<'_>, CompileError> {
    let sharing = match document.sharing.as_str() {
        "exclusive" if document.maximum_holders == 1 => ResourceSharingMode::Exclusive,
        "shared-read" if document.maximum_holders == u16::MAX => ResourceSharingMode::SharedRead,
        "shared-bounded" if document.maximum_holders > 0 => ResourceSharingMode::SharedBounded {
            maximum_holders: document.maximum_holders,
        },
        _ => return Err(CompileError::new(CompileReason::PlanInvalid)),
    };
    let foreign_retention = match document.foreign_retention.as_str() {
        "none" if document.foreign_maximum_bytes == 0 && document.foreign_release_ticks == 0 => {
            ForeignRetention::None
        }
        "bounded" => ForeignRetention::Bounded {
            maximum_bytes: document.foreign_maximum_bytes,
            release_ticks: document.foreign_release_ticks,
        },
        "observed-only"
            if document.foreign_maximum_bytes == 0 && document.foreign_release_ticks == 0 =>
        {
            ForeignRetention::ObservedOnly
        }
        "unsupported"
            if document.foreign_maximum_bytes == 0 && document.foreign_release_ticks == 0 =>
        {
            ForeignRetention::Unsupported
        }
        _ => return Err(CompileError::new(CompileReason::PlanInvalid)),
    };
    Ok(ResourceLeaseContract {
        schema_version: document.schema_version,
        id: id(&document.id)?,
        resource_binding: id(&document.resource_binding)?,
        holder: instance(&document.holder)?,
        run: id(&document.run)?,
        epoch: document.epoch,
        scope: id(&document.scope)?,
        sharing,
        reservation: document.reservation.into(),
        time_basis: id(&document.time_basis)?,
        issued_at_tick: document.issued_at_tick,
        expires_at_tick: document.expires_at_tick,
        revocation_grace_ticks: document.revocation_grace_ticks,
        cleanup_ticks: document.cleanup_ticks,
        maximum_operations: document.maximum_operations,
        maximum_evidence_events: document.maximum_evidence_events,
        cleanup_escalation: pin(&document.cleanup_escalation)?,
        foreign_retention,
    })
}

fn effect_commit_profile(
    document: &EffectCommitProfileDocument,
) -> Result<EffectCommitProfile<'_>, CompileError> {
    Ok(EffectCommitProfile {
        schema_version: document.schema_version,
        id: id(&document.id)?,
        operation: id(&document.operation)?,
        resource_lease: id(&document.resource_lease)?,
        commit_boundary: pin(&document.commit_boundary)?,
        idempotency: match document.idempotency.as_str() {
            "none" => EffectIdempotency::None,
            "same-key-same-effect" => EffectIdempotency::SameKeySameEffect,
            "reconcile-before-retry" => EffectIdempotency::ReconcileBeforeRetry,
            _ => return Err(CompileError::new(CompileReason::PlanInvalid)),
        },
        unknown_commit: match document.unknown_commit.as_str() {
            "fail" => UnknownCommitPolicy::Fail,
            "reconcile" => UnknownCommitPolicy::Reconcile,
            "retry-same-idempotency-key" => UnknownCommitPolicy::RetrySameIdempotencyKey,
            _ => return Err(CompileError::new(CompileReason::PlanInvalid)),
        },
        discontinuity: match document.discontinuity.as_str() {
            "failed-before-commit" => EffectDiscontinuity::FailedBeforeCommit,
            "commit-unknown" => EffectDiscontinuity::CommitUnknown,
            "reconcile-required" => EffectDiscontinuity::ReconcileRequired,
            _ => return Err(CompileError::new(CompileReason::PlanInvalid)),
        },
        cleanup: pin(&document.cleanup)?,
        maximum_attempts: document.maximum_attempts,
        evidence_events_per_attempt: document.evidence_events_per_attempt,
    })
}

fn port_to_document(port: ResolvedPlanPort<'_>) -> PlanPortDocument {
    PlanPortDocument {
        node: port.node.as_str().to_owned(),
        port: port.port.to_string(),
        direction: port.direction.as_str().to_owned(),
        port_contract_hash: port.port_contract_hash.to_string(),
        value_type_id: port.value_type.contract_id.to_string(),
        value_type_schema_version: port.value_type.schema_version,
        value_type_semantic_hash: port.value_type.semantic_hash.to_string(),
    }
}

fn topology_port<'a>(
    node: &'a str,
    port: &'a conduit_runtime::ExactTopologyPort,
) -> Result<ResolvedPlanPort<'a>, CompileError> {
    Ok(ResolvedPlanPort {
        node: instance(node)?,
        port: id(&port.id)?,
        direction: port.direction,
        port_contract_hash: port.contract_hash,
        value_type: port.value_type,
    })
}

fn topology_flow(
    cord: &conduit_runtime::ExactTopologyCord,
) -> Result<FlowPolicy<'_>, CompileError> {
    flow(
        cord.capacity_items,
        cord.max_value_bytes,
        cord.max_queued_bytes,
        cord.low_watermark_items,
        cord.high_watermark_items,
        &cord.pressure,
    )
}

fn flow_document(cord: &PlanCordDocument) -> Result<FlowPolicy<'_>, CompileError> {
    let capacity = FlowCapacity::new(
        cord.capacity_items,
        cord.max_value_bytes,
        cord.max_queued_bytes,
    )
    .map_err(|_| CompileError::new(CompileReason::BudgetInvalid))?;
    let watermarks = FlowWatermarks::new(
        cord.low_watermark_items,
        cord.high_watermark_items,
        capacity,
    )
    .map_err(|_| CompileError::new(CompileReason::BudgetInvalid))?;
    let pressure = match &cord.pressure {
        PressureDocument::BlockFifo => Pressure::Block(BlockingFairness::Fifo),
        PressureDocument::Reject => Pressure::Reject,
        PressureDocument::Coalesce { relation } => Pressure::Coalesce {
            relation: id(relation)?,
        },
        PressureDocument::Sample { every, offset } => Pressure::Sample(
            SampleSchedule::new(*every, *offset)
                .map_err(|_| CompileError::new(CompileReason::BudgetInvalid))?,
        ),
        PressureDocument::DropDisposable => Pressure::DropDisposable,
        PressureDocument::Disconnect => Pressure::Disconnect,
        PressureDocument::Fail => Pressure::Fail,
    };
    FlowPolicy::new(capacity, pressure, watermarks)
        .map_err(|_| CompileError::new(CompileReason::BudgetInvalid))
}

fn flow<'a>(
    capacity_items: u16,
    max_value_bytes: u32,
    max_queued_bytes: u64,
    low_watermark_items: u16,
    high_watermark_items: u16,
    pressure: &'a SourcePressure,
) -> Result<FlowPolicy<'a>, CompileError> {
    let capacity = FlowCapacity::new(capacity_items, max_value_bytes, max_queued_bytes)
        .map_err(|_| CompileError::new(CompileReason::BudgetInvalid))?;
    let watermarks = FlowWatermarks::new(low_watermark_items, high_watermark_items, capacity)
        .map_err(|_| CompileError::new(CompileReason::BudgetInvalid))?;
    let pressure = match pressure {
        SourcePressure::Block => Pressure::Block(BlockingFairness::Fifo),
        SourcePressure::Reject => Pressure::Reject,
        SourcePressure::Coalesce { relation } => Pressure::Coalesce {
            relation: id(relation)?,
        },
        SourcePressure::Sample { every, offset } => Pressure::Sample(
            SampleSchedule::new(*every, *offset)
                .map_err(|_| CompileError::new(CompileReason::BudgetInvalid))?,
        ),
        SourcePressure::DropDisposable => Pressure::DropDisposable,
        SourcePressure::Disconnect => Pressure::Disconnect,
        SourcePressure::Fail => Pressure::Fail,
    };
    FlowPolicy::new(capacity, pressure, watermarks)
        .map_err(|_| CompileError::new(CompileReason::BudgetInvalid))
}

fn pressure_document(pressure: &SourcePressure) -> PressureDocument {
    match pressure {
        SourcePressure::Block => PressureDocument::BlockFifo,
        SourcePressure::Reject => PressureDocument::Reject,
        SourcePressure::Coalesce { relation } => PressureDocument::Coalesce {
            relation: relation.clone(),
        },
        SourcePressure::Sample { every, offset } => PressureDocument::Sample {
            every: *every,
            offset: *offset,
        },
        SourcePressure::DropDisposable => PressureDocument::DropDisposable,
        SourcePressure::Disconnect => PressureDocument::Disconnect,
        SourcePressure::Fail => PressureDocument::Fail,
    }
}

fn seal_execution_profile(document: &mut ExecutionProfileDocument) -> Result<(), CompileError> {
    canonicalize_execution_profile(document);
    let arena = Bump::new();
    document.semantic_hash = SemanticHash::from_bytes([0; 32]).to_string();
    let profile = execution_profile(document, &arena)?;
    let mut scratch = vec![SemanticHash::from_bytes([0; 32]); profile.identity_fact_count()];
    document.semantic_hash = profile
        .computed_semantic_hash(&mut scratch)
        .map_err(|_| CompileError::new(CompileReason::InvalidInput))?
        .to_string();
    let profile = execution_profile(document, &arena)?;
    profile
        .validate(&mut scratch)
        .map_err(|_| CompileError::new(CompileReason::InvalidInput))
}

fn execution_profile<'a>(
    document: &'a ExecutionProfileDocument,
    arena: &'a Bump,
) -> Result<ExecutionProfile<'a>, CompileError> {
    let representations = document
        .representations
        .iter()
        .map(|representation| {
            Ok(ValueRepresentation {
                direction: direction(&representation.direction)?,
                port: id(&representation.port)?,
                semantic_type: TypeContractRef {
                    contract_id: id(&representation.semantic_type.id)?,
                    schema_version: representation.semantic_type.schema_version,
                    semantic_hash: parse_hash(&representation.semantic_type.semantic_hash)?,
                },
                representation: pin(&representation.representation)?,
                ownership: ownership_model(&representation.ownership)?,
                disposition: handle_disposition(&representation.disposition)?,
                max_bytes: representation.max_bytes,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let memory_claims = document
        .memory_claims
        .iter()
        .map(|claim| {
            Ok(MemoryClaim {
                category: memory_category(&claim.category)?,
                accounting: memory_accounting(&claim.accounting)?,
                bytes: claim.bytes,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    Ok(ExecutionProfile {
        id: id(&document.id)?,
        schema_version: document.schema_version,
        semantic_hash: parse_hash(&document.semantic_hash)?,
        boundedness: boundedness_profile(&document.boundedness)?,
        cancellation: cancellation_guarantee(&document.cancellation)?,
        step_bound_enforced: document.step_bound_enforced,
        limits: document.limits.into(),
        representations: arena.alloc_slice_copy(&representations),
        memory_claims: arena.alloc_slice_copy(&memory_claims),
        checkpoint: document.checkpoint.as_ref().map(pin).transpose()?,
    })
}

fn execution_profile_document(profile: &ExecutionProfile<'_>) -> ExecutionProfileDocument {
    ExecutionProfileDocument {
        id: profile.id.to_string(),
        schema_version: profile.schema_version,
        semantic_hash: profile.semantic_hash.to_string(),
        boundedness: match profile.boundedness {
            BoundednessProfile::Hard => "hard",
            BoundednessProfile::Observed => "observed",
        }
        .to_owned(),
        cancellation: match profile.cancellation {
            CancellationGuarantee::Bounded => "bounded",
            CancellationGuarantee::Cooperative => "cooperative",
            CancellationGuarantee::Unbounded => "unbounded",
        }
        .to_owned(),
        step_bound_enforced: profile.step_bound_enforced,
        limits: profile.limits.into(),
        representations: profile
            .representations
            .iter()
            .map(|representation| ValueRepresentationDocument {
                direction: representation.direction.as_str().to_owned(),
                port: representation.port.to_string(),
                semantic_type: PinDocument {
                    id: representation.semantic_type.contract_id.to_string(),
                    schema_version: representation.semantic_type.schema_version,
                    semantic_hash: representation.semantic_type.semantic_hash.to_string(),
                },
                representation: pin_document(representation.representation),
                ownership: match representation.ownership {
                    OwnershipModel::Owned => "owned",
                    OwnershipModel::Borrowed => "borrowed",
                    OwnershipModel::SharedHandle => "shared-handle",
                    OwnershipModel::ExclusiveHandle => "exclusive-handle",
                }
                .to_owned(),
                disposition: match representation.disposition {
                    HandleDisposition::None => "none",
                    HandleDisposition::ExplicitDispose => "explicit-dispose",
                }
                .to_owned(),
                max_bytes: representation.max_bytes,
            })
            .collect(),
        memory_claims: profile
            .memory_claims
            .iter()
            .map(|claim| MemoryClaimDocument {
                category: match claim.category {
                    MemoryCategory::Retained => "retained",
                    MemoryCategory::StepScratch => "step-scratch",
                    MemoryCategory::PortTransactions => "port-transactions",
                    MemoryCategory::PendingOperations => "pending-operations",
                    MemoryCategory::HostServices => "host-services",
                    MemoryCategory::ForeignRuntime => "foreign-runtime",
                }
                .to_owned(),
                accounting: match claim.accounting {
                    MemoryAccounting::ExecutorAllocated => "executor-allocated",
                    MemoryAccounting::BackendBounded => "backend-bounded",
                    MemoryAccounting::ExternallyBounded => "externally-bounded",
                    MemoryAccounting::ObservedOnly => "observed-only",
                }
                .to_owned(),
                bytes: claim.bytes,
            })
            .collect(),
        checkpoint: profile.checkpoint.map(pin_document),
    }
}

fn canonicalize_execution_profile(document: &mut ExecutionProfileDocument) {
    document
        .representations
        .sort_by(|left, right| (&left.direction, &left.port).cmp(&(&right.direction, &right.port)));
    document
        .memory_claims
        .sort_by(|left, right| left.category.cmp(&right.category));
}

fn plan_pool_runtime_document(value: PlanPoolRuntime<'_>) -> PlanPoolRuntimeDocument {
    let (supervision, maximum_attempts, backoff_ticks, fallback_target) =
        match value.contract.supervision {
            PoolSupervisionPolicy::FailTogether => ("fail-together", 0, 0, None),
            PoolSupervisionPolicy::Isolate => ("isolate", 0, 0, None),
            PoolSupervisionPolicy::RestartBounded {
                maximum_attempts,
                backoff_ticks,
            } => ("restart-bounded", maximum_attempts, backoff_ticks, None),
            PoolSupervisionPolicy::Fallback { target } => {
                ("fallback", 0, 0, Some(target.as_str().to_owned()))
            }
            PoolSupervisionPolicy::Escalate => ("escalate", 0, 0, None),
        };
    PlanPoolRuntimeDocument {
        admission: match value.contract.admission {
            PoolAdmissionPolicy::Reject => "reject",
            PoolAdmissionPolicy::Block => "block",
            PoolAdmissionPolicy::QueueBounded => "queue-bounded",
            PoolAdmissionPolicy::Fail => "fail",
        }
        .to_owned(),
        supervision: supervision.to_owned(),
        maximum_attempts,
        backoff_ticks,
        fallback_target,
        cleanup: match value.contract.cleanup {
            PoolCleanupPolicy::Drain => "drain",
            PoolCleanupPolicy::Abort => "abort",
        }
        .to_owned(),
        deadline_ticks: value.contract.deadline_ticks,
        idle_timeout_ticks: value.contract.idle_timeout_ticks,
        cleanup_ticks: value.contract.cleanup_ticks,
        maximum_evidence_events: value.contract.maximum_evidence_events,
        per_instance: value.contract.reservation.into(),
        queued: value.queued_reservation.into(),
        candidate_maximum_live: value.generation_reservation.candidate_maximum_live,
        rollback_maximum_live: value.generation_reservation.rollback_maximum_live,
        generation_reserved_slots: value.generation_reservation.reserved_slots,
        generation_reserved: value.generation_reservation.reserved_resources.into(),
        total_reserved: value.contract.total_reservation.into(),
    }
}

fn pin_document(value: PinnedDescriptor<'_>) -> PinDocument {
    PinDocument {
        id: value.id.to_string(),
        schema_version: value.schema_version,
        semantic_hash: value.semantic_hash.to_string(),
    }
}

fn constraint_documents(
    constraints: &[AuthorityConstraintRef<'_>],
) -> Vec<AuthorityConstraintDocument> {
    constraints
        .iter()
        .map(|constraint| AuthorityConstraintDocument {
            id: constraint.id.to_string(),
            semantic_hash: constraint.semantic_hash.to_string(),
        })
        .collect()
}

fn effect_to_document(effect: EffectRequirement<'_>) -> EffectRequirementDocument {
    let (resource_kind, resource_id) = match effect.resource {
        ResourceSelector::Exact(resource) => {
            (resource.kind.to_string(), Some(resource.id.to_string()))
        }
        ResourceSelector::Kind(kind) => (kind.to_string(), None),
    };
    EffectRequirementDocument {
        id: effect.id.to_string(),
        administrative_class: effect.administrative_class.map(pin_document),
        policy_budget_class: effect.policy_budget_class.map(pin_document),
        action: effect.action.to_string(),
        resource_kind,
        resource_id,
        requester: effect.requester.as_str().to_owned(),
        audience: effect.audience.to_string(),
        constraints: constraint_documents(effect.constraints),
        check_at_use: effect.check_at_use,
    }
}

fn administrative_principal_document(
    value: AdministrativePrincipal<'_>,
) -> AdministrativePrincipalDocument {
    AdministrativePrincipalDocument {
        realm: value.realm.to_string(),
        entity: value.entity.to_string(),
        key: value.key.to_string(),
        profile: pin_document(value.profile),
        source_plan: value.source_plan.to_string(),
        source_epoch: value.source_epoch,
    }
}

fn administrative_subject_document(
    value: AdministrativeSubject<'_>,
) -> AdministrativeSubjectDocument {
    AdministrativeSubjectDocument {
        realm: value.realm.to_string(),
        entity: value.entity.to_string(),
        plan: value.plan.to_string(),
        epoch: value.epoch,
        artifact: value.artifact.map(|artifact| artifact.to_string()),
        budget: value.budget.map(pin_document),
    }
}

fn delegation_document(value: DelegationEnvelope<'_>) -> DelegationEnvelopeDocument {
    let (resource_kind, resource_id) = match value.resource {
        ResourceSelector::Exact(resource) => {
            (resource.kind.to_string(), Some(resource.id.to_string()))
        }
        ResourceSelector::Kind(kind) => (kind.to_string(), None),
    };
    DelegationEnvelopeDocument {
        action: value.action.to_string(),
        resource_kind,
        resource_id,
        audience: value.audience.to_string(),
        time_basis: value.time_basis.to_string(),
        not_before_tick: value.not_before_tick,
        expires_at_tick: value.expires_at_tick,
        remaining_depth: value.remaining_depth,
    }
}

fn administrative_proof_document(value: AdministrativeProof<'_>) -> AdministrativeProofDocument {
    AdministrativeProofDocument {
        proposal: AdministrativeProposalDocument {
            schema_version: value.proposal.schema_version,
            identity: value.proposal.identity.to_string(),
            id: value.proposal.id.to_string(),
            effect_class: pin_document(value.proposal.effect_class),
            operation: pin_document(value.proposal.operation),
            requester: administrative_principal_document(value.proposal.requester),
            subject: administrative_subject_document(value.proposal.subject),
            beneficiaries: value
                .proposal
                .beneficiaries
                .iter()
                .copied()
                .map(administrative_subject_document)
                .collect(),
            predecessor_plan: value.proposal.predecessor_plan.map(|plan| plan.to_string()),
            delegation: value.proposal.delegation.map(delegation_document),
            protected_handle: value.proposal.protected_handle.map(pin_document),
            ceremony: value.proposal.ceremony.map(pin_document),
            time_basis: value.proposal.time_basis.to_string(),
            created_at_tick: value.proposal.created_at_tick,
            expires_at_tick: value.proposal.expires_at_tick,
        },
        policy: ContainmentPolicyDocument {
            schema_version: value.policy.schema_version,
            identity: value.policy.identity.to_string(),
            descriptor: pin_document(value.policy.descriptor),
            effect_class: pin_document(value.policy.effect_class),
            approvers: value
                .policy
                .approvers
                .iter()
                .map(|approver| AdministrativeApproverDocument {
                    realm: approver.realm.to_string(),
                    entity: approver.entity.to_string(),
                    key: approver.key.to_string(),
                    profile: pin_document(approver.profile),
                    failure_domain: pin_document(approver.failure_domain),
                })
                .collect(),
            committer: AdministrativeApproverDocument {
                realm: value.policy.committer.realm.to_string(),
                entity: value.policy.committer.entity.to_string(),
                key: value.policy.committer.key.to_string(),
                profile: pin_document(value.policy.committer.profile),
                failure_domain: pin_document(value.policy.committer.failure_domain),
            },
            executor: AdministrativeApproverDocument {
                realm: value.policy.executor.realm.to_string(),
                entity: value.policy.executor.entity.to_string(),
                key: value.policy.executor.key.to_string(),
                profile: pin_document(value.policy.executor.profile),
                failure_domain: pin_document(value.policy.executor.failure_domain),
            },
            minimum_approvals: value.policy.minimum_approvals,
            minimum_failure_domains: value.policy.minimum_failure_domains,
            requester_independence: value.policy.requester_independence,
            beneficiary_independence: value.policy.beneficiary_independence,
            successor_independence: value.policy.successor_independence,
            delegation_ceiling: value.policy.delegation_ceiling.map(delegation_document),
            ceremony: value.policy.ceremony.map(pin_document),
        },
        approvals: value
            .approvals
            .iter()
            .map(|approval| AdministrativeApprovalDocument {
                schema_version: approval.schema_version,
                identity: approval.identity.to_string(),
                id: approval.id.to_string(),
                proposal_identity: approval.proposal_identity.to_string(),
                policy_identity: approval.policy_identity.to_string(),
                approver: administrative_principal_document(approval.approver),
                failure_domain: pin_document(approval.failure_domain),
                time_basis: approval.time_basis.to_string(),
                issued_at_tick: approval.issued_at_tick,
                expires_at_tick: approval.expires_at_tick,
                status: match approval.status {
                    AdministrativeApprovalStatus::Current => "current",
                    AdministrativeApprovalStatus::Revoked => "revoked",
                }
                .to_owned(),
            })
            .collect(),
        commit: AdministrativeCommitDocument {
            schema_version: value.commit.schema_version,
            identity: value.commit.identity.to_string(),
            id: value.commit.id.to_string(),
            proposal_identity: value.commit.proposal_identity.to_string(),
            policy_identity: value.commit.policy_identity.to_string(),
            approvals: value
                .commit
                .approvals
                .iter()
                .map(ToString::to_string)
                .collect(),
            committed_by: administrative_principal_document(value.commit.committed_by),
            committed_at_tick: value.commit.committed_at_tick,
        },
        execution: AdministrativeExecutionDocument {
            schema_version: value.execution.schema_version,
            identity: value.execution.identity.to_string(),
            id: value.execution.id.to_string(),
            proposal_identity: value.execution.proposal_identity.to_string(),
            commit_identity: value.execution.commit_identity.to_string(),
            executor: administrative_principal_document(value.execution.executor),
            time_basis: value.execution.time_basis.to_string(),
            not_before_tick: value.execution.not_before_tick,
            expires_at_tick: value.execution.expires_at_tick,
        },
    }
}

fn trait_requirement_document(value: TraitRequirement) -> String {
    match value {
        TraitRequirement::Any => "any",
        TraitRequirement::Required => "required",
        TraitRequirement::Forbidden => "forbidden",
    }
    .to_owned()
}

fn toxic_pattern_document(value: ToxicEffectPattern<'_>) -> ToxicEffectPatternDocument {
    let (resource_kind, resource_id) = match value.resource {
        None => (None, None),
        Some(ResourceSelector::Kind(kind)) => (Some(kind.to_string()), None),
        Some(ResourceSelector::Exact(resource)) => (
            Some(resource.kind.to_string()),
            Some(resource.id.to_string()),
        ),
    };
    ToxicEffectPatternDocument {
        id: value.id.to_string(),
        class: pin_document(value.class),
        resource_kind,
        resource_id,
        audience: value.audience.map(|value| value.to_string()),
        host: value.host.map(|value| value.to_string()),
        realm: value.realm.map(|value| value.to_string()),
        budget: value.budget.map(pin_document),
        persistence: trait_requirement_document(value.persistence),
        delegation: trait_requirement_document(value.delegation),
        distributed: trait_requirement_document(value.distributed),
        administrative: trait_requirement_document(value.administrative),
    }
}

fn hazard_closure_document(value: PlanHazardClosure<'_>) -> HazardClosureDocument {
    HazardClosureDocument {
        epoch: value.epoch,
        plan_subject: value.plan_subject.to_string(),
        policy: HazardClosurePolicyDocument {
            schema_version: value.policy.schema_version,
            identity: value.policy.identity.to_string(),
            descriptor: pin_document(value.policy.descriptor),
            permit_class: pin_document(value.policy.permit_class),
            classes: value
                .policy
                .classes
                .iter()
                .map(|class| EffectClassBindingDocument {
                    identity: class.identity.to_string(),
                    descriptor: pin_document(class.descriptor),
                    persistence: class.traits.persistence,
                    delegation: class.traits.delegation,
                    distributed: class.traits.distributed,
                    administrative: class.traits.administrative,
                })
                .collect(),
            rules: value
                .policy
                .rules
                .iter()
                .map(|rule| ToxicCombinationRuleDocument {
                    identity: rule.identity.to_string(),
                    descriptor: pin_document(rule.descriptor),
                    patterns: rule
                        .patterns
                        .iter()
                        .copied()
                        .map(toxic_pattern_document)
                        .collect(),
                    flows: rule
                        .flows
                        .iter()
                        .map(|flow| ToxicFlowRequirementDocument {
                            from_pattern: flow.from_pattern,
                            to_pattern: flow.to_pattern,
                            transfer: pin_document(flow.transfer),
                        })
                        .collect(),
                })
                .collect(),
            limits: value.policy.limits.into(),
        },
        flows: value
            .flows
            .iter()
            .map(|flow| EffectFlowBindingDocument {
                from_effect: flow.from_effect.to_string(),
                to_effect: flow.to_effect.to_string(),
                transfer: pin_document(flow.transfer),
            })
            .collect(),
        permits: value
            .permits
            .iter()
            .map(|permit| HazardPermitDocument {
                identity: permit.identity.to_string(),
                descriptor: pin_document(permit.descriptor),
                policy_identity: permit.policy_identity.to_string(),
                rule_identity: permit.rule_identity.to_string(),
                plan_subject: permit.plan_subject.to_string(),
                epoch: permit.epoch,
                scope_identity: permit.scope_identity.to_string(),
                time_basis: permit.time_basis.to_string(),
                not_before_tick: permit.not_before_tick,
                expires_at_tick: permit.expires_at_tick,
                approval: administrative_proof_document(permit.approval),
            })
            .collect(),
        decision_identity: value.decision_identity.to_string(),
        hazardous_hosts: value
            .hazardous_hosts
            .iter()
            .copied()
            .map(hazardous_host_binding_document)
            .collect(),
    }
}

fn hazardous_host_binding_document(
    value: HazardousHostBinding<'_>,
) -> HazardousHostBindingDocument {
    HazardousHostBindingDocument {
        host: value.host.to_string(),
        profile: HazardousHostProfileDocument {
            schema_version: value.profile.schema_version,
            identity: value.profile.identity.to_string(),
            descriptor: pin_document(value.profile.descriptor),
            safe_state: pin_document(value.profile.safe_state),
            inhibit_boundary: pin_document(value.profile.inhibit_boundary),
            watchdog: pin_document(value.profile.watchdog),
            effect_boundary: pin_document(value.profile.effect_boundary),
            command_effect_class: pin_document(value.profile.command_effect_class),
            clear_effect_class: pin_document(value.profile.clear_effect_class),
            clear_operation: pin_document(value.profile.clear_operation),
            clear_ceremony: pin_document(value.profile.clear_ceremony),
            time_basis: value.profile.time_basis.to_string(),
            maximum_command_horizon_ticks: value.profile.maximum_command_horizon_ticks,
            maximum_observation_age_ticks: value.profile.maximum_observation_age_ticks,
            maximum_evidence_records: value.profile.maximum_evidence_records,
            require_physical_presence_to_clear: value.profile.require_physical_presence_to_clear,
            require_isolated_implementation: value.profile.require_isolated_implementation,
            envelope: value
                .profile
                .envelope
                .iter()
                .map(|limit| OperatingEnvelopeLimitDocument {
                    dimension: pin_document(limit.dimension),
                    minimum: limit.minimum,
                    maximum: limit.maximum,
                })
                .collect(),
        },
        observation: InhibitObservationDocument {
            schema_version: value.observation.schema_version,
            identity: value.observation.identity.to_string(),
            profile_identity: value.observation.profile_identity.to_string(),
            host: value.observation.host.to_string(),
            safe_state: pin_document(value.observation.safe_state),
            inhibit_boundary: pin_document(value.observation.inhibit_boundary),
            watchdog: pin_document(value.observation.watchdog),
            effect_boundary: pin_document(value.observation.effect_boundary),
            time_basis: value.observation.time_basis.to_string(),
            observed_at_tick: value.observation.observed_at_tick,
            valid_until_tick: value.observation.valid_until_tick,
            latch_generation: value.observation.latch_generation,
            latch_state: match value.observation.latch_state {
                InhibitLatchState::SafeDisarmed => "safe-disarmed",
                InhibitLatchState::Inhibited => "inhibited",
            }
            .to_owned(),
            independent_from_plan: value.observation.independent_from_plan,
            local_safe_path: value.observation.local_safe_path,
            survives_executor_loss: value.observation.survives_executor_loss,
            survives_partition: value.observation.survives_partition,
            graph_cannot_replace: value.observation.graph_cannot_replace,
            confinement: match value.observation.confinement {
                ImplementationConfinement::EffectBoundaryEnforced => "effect-boundary-enforced",
                ImplementationConfinement::UnconfinedNative => "unconfined-native",
            }
            .to_owned(),
        },
    }
}

fn policy_budget_binding_document(value: PlanPolicyBudget<'_>) -> PolicyBudgetBindingDocument {
    let (anchor_kind, anchor_id) = match value.policy.anchor {
        PolicyBudgetAnchor::Realm(id) => ("realm", id),
        PolicyBudgetAnchor::Host(id) => ("host", id),
        PolicyBudgetAnchor::Site(id) => ("site", id),
    };
    PolicyBudgetBindingDocument {
        policy: PersistentBudgetPolicyDocument {
            schema_version: value.policy.schema_version,
            identity: value.policy.identity.to_string(),
            descriptor: pin_document(value.policy.descriptor),
            owner: pin_document(value.policy.owner),
            subject: pin_document(value.policy.subject),
            anchor_kind: anchor_kind.to_owned(),
            anchor_id: anchor_id.to_string(),
            action: value.policy.action.to_string(),
            resource_class: pin_document(value.policy.resource_class),
            time_basis: value.policy.time_basis.to_string(),
            limits: PolicyBudgetLimitsDocument {
                current_stock: value.policy.limits.current_stock,
                rolling_units: value.policy.limits.rolling.map(|limit| limit.units),
                rolling_window_ticks: value.policy.limits.rolling.map(|limit| limit.window_ticks),
                lifetime: value.policy.limits.lifetime,
            },
            reservation_ttl_ticks: value.policy.reservation_ttl_ticks,
            lease: value.policy.lease.map(|lease| PolicyLeaseRuleDocument {
                maximum_ticks: lease.maximum_ticks,
                renewal_authority: pin_document(lease.renewal_authority),
                offline_allowed: lease.offline_allowed,
            }),
            audit_id: value.policy.audit_id.to_string(),
            persistence_profile: pin_document(value.policy.persistence_profile),
            maximum_reservations: value.policy.maximum_reservations,
            maximum_evidence_events: value.policy.maximum_evidence_events,
        },
        status: PolicyBudgetStatusDocument {
            schema_version: value.status.schema_version,
            identity: value.status.identity.to_string(),
            policy_identity: value.status.policy_identity.to_string(),
            ledger: pin_document(value.status.ledger),
            checkpoint: value.status.checkpoint.to_string(),
            sequence: value.status.sequence,
            current_stock: value.status.current_stock,
            rolling_window_start: value.status.rolling_window_start,
            rolling_committed: value.status.rolling_committed,
            lifetime_committed: value.status.lifetime_committed,
            reserved: value.status.reserved,
            evidence_remaining: value.status.evidence_remaining,
            availability: match value.status.availability {
                PolicyBudgetAvailability::Available => "available",
                PolicyBudgetAvailability::Unavailable => "unavailable",
                PolicyBudgetAvailability::RetentionGap => "retention-gap",
            }
            .to_owned(),
            time_basis: value.status.time_basis.to_string(),
            observed_at_tick: value.status.observed_at_tick,
            valid_until_tick: value.status.valid_until_tick,
        },
        lease: value.lease.map(|lease| PolicyBudgetLeaseDocument {
            schema_version: lease.schema_version,
            identity: lease.identity.to_string(),
            policy_identity: lease.policy_identity.to_string(),
            holder: pin_document(lease.holder),
            renewal_authority: pin_document(lease.renewal_authority),
            time_basis: lease.time_basis.to_string(),
            issued_at_tick: lease.issued_at_tick,
            expires_at_tick: lease.expires_at_tick,
            offline: lease.offline,
        }),
        required_units: value.required_units,
        check_at_use: value.check_at_use,
    }
}

fn capability_to_document(capability: HostCapability<'_>) -> HostCapabilityDocument {
    HostCapabilityDocument {
        id: capability.id.to_string(),
        action: capability.action.to_string(),
        resource_kind: capability.resource.kind.to_string(),
        resource_id: capability.resource.id.to_string(),
        host: capability.host.to_string(),
        time_basis: capability.time_basis.to_string(),
        observed_at_tick: capability.observed_at_tick,
        valid_until_tick: capability.valid_until_tick,
    }
}

fn grant_to_document(grant: AuthorityGrant<'_>) -> AuthorityGrantDocument {
    AuthorityGrantDocument {
        id: grant.id.to_string(),
        action: grant.action.to_string(),
        resource_kind: grant.resource.kind.to_string(),
        resource_id: grant.resource.id.to_string(),
        scope_root: grant.scope.root.as_str().to_owned(),
        scope_descendants: grant.scope.descendants,
        audience: grant.audience.to_string(),
        constraints: constraint_documents(grant.constraints),
        time_basis: grant.time_basis.to_string(),
        not_before_tick: grant.not_before_tick,
        expires_at_tick: grant.expires_at_tick,
        issued_for_host: grant.issued_for_host.to_string(),
        delegation: grant.delegation.as_str().to_owned(),
        audit_id: grant.audit_id.to_string(),
        terminal_policy: match grant.terminal_policy {
            StopPolicy::Drain => "drain",
            StopPolicy::Abort => "abort",
        }
        .to_owned(),
    }
}

fn canonicalize_compile_input(input: &mut CompileInput) {
    canonicalize_catalog(&mut input.catalog);
    input
        .modules
        .sort_by(|left, right| left.canonical_uri.cmp(&right.canonical_uri));
    for pool in &mut input.pool_bindings {
        pool.authority_grants.sort();
    }
    input
        .pool_bindings
        .sort_by(|left, right| left.pool_semantic_hash.cmp(&right.pool_semantic_hash));
    for supervision in &mut input.supervision_bindings {
        supervision.members.sort();
        supervision
            .actions
            .sort_by(|left, right| (&left.kind, &left.target).cmp(&(&right.kind, &right.target)));
        supervision
            .action_targets
            .sort_by(|left, right| left.choice.cmp(&right.choice));
    }
    input
        .supervision_bindings
        .sort_by(|left, right| left.instance.cmp(&right.instance));
    input.trusted_entities.sort();
    input.trusted_status_reporters.sort();
    input.implementation_preference.sort();
    if let Some(closure) = &mut input.hazard_closure {
        closure
            .policy
            .classes
            .sort_by(|left, right| left.descriptor.id.cmp(&right.descriptor.id));
        for rule in &mut closure.policy.rules {
            rule.flows.sort_by(|left, right| {
                (left.from_pattern, left.to_pattern, &left.transfer.id).cmp(&(
                    right.from_pattern,
                    right.to_pattern,
                    &right.transfer.id,
                ))
            });
        }
        closure
            .policy
            .rules
            .sort_by(|left, right| left.descriptor.id.cmp(&right.descriptor.id));
        closure.flows.sort_by(|left, right| {
            (&left.from_effect, &left.to_effect, &left.transfer.id).cmp(&(
                &right.from_effect,
                &right.to_effect,
                &right.transfer.id,
            ))
        });
        closure
            .permits
            .sort_by(|left, right| left.descriptor.id.cmp(&right.descriptor.id));
        for permit in &mut closure.permits {
            permit.approval.policy.approvers.sort_by(|left, right| {
                (&left.realm, &left.entity, &left.key).cmp(&(
                    &right.realm,
                    &right.entity,
                    &right.key,
                ))
            });
            permit
                .approval
                .proposal
                .beneficiaries
                .sort_by(|left, right| {
                    (&left.realm, &left.entity, &left.plan, left.epoch).cmp(&(
                        &right.realm,
                        &right.entity,
                        &right.plan,
                        right.epoch,
                    ))
                });
            permit
                .approval
                .approvals
                .sort_by(|left, right| left.id.cmp(&right.id));
            permit.approval.commit.approvals.sort();
        }
    }
    if let Some(distribution) = &mut input.distribution {
        distribution.providers.sort_by(|left, right| {
            (&left.provider.id, &left.provider.semantic_hash)
                .cmp(&(&right.provider.id, &right.provider.semantic_hash))
        });
        distribution.requirements.sort_by(|left, right| {
            (&left.provider.id, &left.provider.semantic_hash)
                .cmp(&(&right.provider.id, &right.provider.semantic_hash))
        });
    }
    input.candidates.sort_by(|left, right| {
        (
            &left.implementation.id,
            &left.host_report.id,
            &left.implementation.identity,
        )
            .cmp(&(
                &right.implementation.id,
                &right.host_report.id,
                &right.implementation.identity,
            ))
    });
    for candidate in &mut input.candidates {
        canonicalize_execution_profile(&mut candidate.execution_profile);
        candidate
            .implementation
            .artifacts
            .sort_by(|left, right| left.id.cmp(&right.id));
        candidate
            .artifacts
            .sort_by(|left, right| left.id.cmp(&right.id));
        candidate.host_report.supported_executors.sort();
        candidate.host_report.supported_targets.sort();
        candidate.host_report.supported_abis.sort();
        candidate.host_report.current_constraints.sort();
        candidate.host_report.capabilities.sort_by(|left, right| {
            (
                &left.interface.id,
                &left.interface.semantic_hash,
                &left.mode,
                &left.subject,
            )
                .cmp(&(
                    &right.interface.id,
                    &right.interface.semantic_hash,
                    &right.mode,
                    &right.subject,
                ))
        });
        candidate.host_report.resources.sort_by(|left, right| {
            (&left.kind, &left.id, &left.descriptor.semantic_hash).cmp(&(
                &right.kind,
                &right.id,
                &right.descriptor.semantic_hash,
            ))
        });
        candidate
            .host_report
            .topology
            .sort_by(|left, right| left.id.cmp(&right.id));
        candidate.implementation.required_authorities.sort();
        candidate.implementation.required_effects.sort();
        candidate.capabilities.sort_by(|left, right| {
            (
                &left.interface.id,
                &left.interface.semantic_hash,
                &left.mode,
                &left.subject,
            )
                .cmp(&(
                    &right.interface.id,
                    &right.interface.semantic_hash,
                    &right.mode,
                    &right.subject,
                ))
        });
        candidate
            .resources
            .sort_by(|left, right| (&left.kind, &left.id).cmp(&(&right.kind, &right.id)));
        candidate.topology.sort_by(|left, right| {
            (&left.contract.id, &left.from, &left.to).cmp(&(
                &right.contract.id,
                &right.from,
                &right.to,
            ))
        });
        for authority in &mut candidate.authorities {
            authority
                .effect
                .constraints
                .sort_by(|left, right| left.id.cmp(&right.id));
            authority
                .grant
                .constraints
                .sort_by(|left, right| left.id.cmp(&right.id));
            if let Some(proof) = &mut authority.containment {
                proof.policy.approvers.sort_by(|left, right| {
                    (&left.realm, &left.entity, &left.key).cmp(&(
                        &right.realm,
                        &right.entity,
                        &right.key,
                    ))
                });
                proof.proposal.beneficiaries.sort_by(|left, right| {
                    (&left.realm, &left.entity, &left.plan, left.epoch).cmp(&(
                        &right.realm,
                        &right.entity,
                        &right.plan,
                        right.epoch,
                    ))
                });
                proof
                    .approvals
                    .sort_by(|left, right| left.id.cmp(&right.id));
                proof.commit.approvals.sort();
            }
            authority.policy_budgets.sort_by(|left, right| {
                (
                    &left.policy.resource_class.id,
                    &left.policy.descriptor.id,
                    &left.policy.anchor_kind,
                    &left.policy.anchor_id,
                )
                    .cmp(&(
                        &right.policy.resource_class.id,
                        &right.policy.descriptor.id,
                        &right.policy.anchor_kind,
                        &right.policy.anchor_id,
                    ))
            });
        }
        candidate.authorities.sort_by(|left, right| {
            (&left.requirement, &left.effect.requester, &left.effect.id).cmp(&(
                &right.requirement,
                &right.effect.requester,
                &right.effect.id,
            ))
        });
        candidate.granted_authorities.sort();
    }
}

fn content_hash(source: &str) -> String {
    format!("sha256:{}", hex(&Sha256::digest(source.as_bytes())))
}

fn plan_group_path(logical_path: &str) -> String {
    let digest = Sha256::digest(
        [
            b"conduit/plan-port-group\0".as_slice(),
            logical_path.as_bytes(),
        ]
        .concat(),
    );
    format!("root/group.h{}", hex(&digest))
}

fn plan_pool_path(logical_path: &str) -> String {
    let digest = Sha256::digest(
        [
            b"conduit/plan-instance-pool\0".as_slice(),
            logical_path.as_bytes(),
        ]
        .concat(),
    );
    format!("root/pool.h{}", hex(&digest))
}

fn plan_group_member_id(member: &conduit_runtime::LoweredGroupPort) -> String {
    let digest = Sha256::digest(
        [
            b"conduit/plan-port-group-member\0".as_slice(),
            member.member.as_bytes(),
        ]
        .concat(),
    );
    format!("m{}.h{}", member.ordinal, hex(&digest))
}

fn plan_group_template_hash(member: &conduit_runtime::LoweredGroupPort) -> SemanticHash {
    let direction = match member.direction {
        conduit_panel::ExportDirection::Input => b"input".as_slice(),
        conduit_panel::ExportDirection::Output => b"output".as_slice(),
    };
    let maximum = member.group_maximum.to_be_bytes();
    let digest = Sha256::digest(
        [
            b"conduit/plan-port-group-template\0".as_slice(),
            member.logical_group_path.as_bytes(),
            b"\0",
            member.group_id.as_bytes(),
            b"\0",
            direction,
            b"\0",
            maximum.as_slice(),
            member.port_contract.semantic_hash.as_bytes(),
        ]
        .concat(),
    );
    SemanticHash::from_bytes(digest.into())
}

fn pin(document: &PinDocument) -> Result<PinnedDescriptor<'_>, CompileError> {
    Ok(PinnedDescriptor {
        id: id(&document.id)?,
        schema_version: document.schema_version,
        semantic_hash: parse_hash(&document.semantic_hash)?,
    })
}

fn resolved_execution_descriptor(
    document: &PinDocument,
) -> Result<ResolvedExecutionDescriptor, CompileError> {
    let pin = pin(document)?;
    Ok(ResolvedExecutionDescriptor {
        id: pin.id.to_string(),
        schema_version: pin.schema_version,
        semantic_hash: pin.semantic_hash,
    })
}

fn implementation_interfaces<'a>(
    documents: &'a [ImplementationInterfaceDocument],
    arena: &'a Bump,
) -> Result<&'a [ManifestInterface<'a>], CompileError> {
    let interfaces = documents
        .iter()
        .map(|document| {
            Ok(ManifestInterface {
                interface: pin(&document.interface)?,
                entrypoint: id(&document.entrypoint)?,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    Ok(arena.alloc_slice_copy(&interfaces))
}

fn id(value: &str) -> Result<Id<'_>, CompileError> {
    Id::new(value).map_err(|_| CompileError::new(CompileReason::InvalidInput))
}

fn instance(value: &str) -> Result<InstancePath<'_>, CompileError> {
    InstancePath::new(value).map_err(|_| CompileError::new(CompileReason::PlanInvalid))
}

fn plan_pool_runtime<'a>(
    document: &'a PlanPoolRuntimeDocument,
    pool: InstancePath<'a>,
    template_hash: SemanticHash,
    implementation_set_hash: SemanticHash,
    maximum_live: u16,
    maximum_queued: u16,
) -> Result<PlanPoolRuntime<'a>, CompileError> {
    let admission = pool_admission(&document.admission)?;
    let supervision = match document.supervision.as_str() {
        "fail-together"
            if document.maximum_attempts == 0
                && document.backoff_ticks == 0
                && document.fallback_target.is_none() =>
        {
            PoolSupervisionPolicy::FailTogether
        }
        "isolate"
            if document.maximum_attempts == 0
                && document.backoff_ticks == 0
                && document.fallback_target.is_none() =>
        {
            PoolSupervisionPolicy::Isolate
        }
        "restart-bounded"
            if document.maximum_attempts > 0
                && document.backoff_ticks > 0
                && document.fallback_target.is_none() =>
        {
            PoolSupervisionPolicy::RestartBounded {
                maximum_attempts: document.maximum_attempts,
                backoff_ticks: document.backoff_ticks,
            }
        }
        "fallback"
            if document.maximum_attempts == 0
                && document.backoff_ticks == 0
                && document.fallback_target.is_some() =>
        {
            PoolSupervisionPolicy::Fallback {
                target: instance(
                    document
                        .fallback_target
                        .as_deref()
                        .ok_or_else(|| CompileError::new(CompileReason::PlanInvalid))?,
                )?,
            }
        }
        "escalate"
            if document.maximum_attempts == 0
                && document.backoff_ticks == 0
                && document.fallback_target.is_none() =>
        {
            PoolSupervisionPolicy::Escalate
        }
        _ => return Err(CompileError::new(CompileReason::PlanInvalid)),
    };
    let per_instance = document.per_instance.into();
    Ok(PlanPoolRuntime {
        contract: PoolContract {
            pool,
            template_hash,
            implementation_set_hash,
            maximum_live,
            maximum_queued,
            admission,
            supervision,
            cleanup: pool_cleanup(&document.cleanup)?,
            deadline_ticks: document.deadline_ticks,
            idle_timeout_ticks: document.idle_timeout_ticks,
            cleanup_ticks: document.cleanup_ticks,
            reservation: per_instance,
            total_reservation: document.total_reserved.into(),
            maximum_evidence_events: document.maximum_evidence_events,
        },
        queued_reservation: document.queued.into(),
        generation_reservation: PoolGenerationReservation {
            old_maximum_live: maximum_live,
            candidate_maximum_live: document.candidate_maximum_live,
            rollback_maximum_live: document.rollback_maximum_live,
            reserved_slots: document.generation_reserved_slots,
            per_instance,
            reserved_resources: document.generation_reserved.into(),
        },
    })
}

fn pool_admission(value: &str) -> Result<PoolAdmissionPolicy, CompileError> {
    match value {
        "reject" => Ok(PoolAdmissionPolicy::Reject),
        "block" => Ok(PoolAdmissionPolicy::Block),
        "queue-bounded" => Ok(PoolAdmissionPolicy::QueueBounded),
        "fail" => Ok(PoolAdmissionPolicy::Fail),
        _ => Err(CompileError::new(CompileReason::PlanInvalid)),
    }
}

fn pool_cleanup(value: &str) -> Result<PoolCleanupPolicy, CompileError> {
    match value {
        "drain" => Ok(PoolCleanupPolicy::Drain),
        "abort" => Ok(PoolCleanupPolicy::Abort),
        _ => Err(CompileError::new(CompileReason::PlanInvalid)),
    }
}

fn direction(value: &str) -> Result<Direction, CompileError> {
    match value {
        "input" => Ok(Direction::Input),
        "output" => Ok(Direction::Output),
        _ => Err(CompileError::new(CompileReason::PlanInvalid)),
    }
}

fn sensitivity(value: &str) -> Result<Sensitivity, CompileError> {
    match value {
        "public" => Ok(Sensitivity::Public),
        "restricted" => Ok(Sensitivity::Restricted),
        "secret" => Ok(Sensitivity::Secret),
        _ => Err(CompileError::new(CompileReason::PlanInvalid)),
    }
}

fn clock_rounding(value: &str) -> Result<ClockRounding, CompileError> {
    match value {
        "exact" => Ok(ClockRounding::Exact),
        "floor" => Ok(ClockRounding::Floor),
        "ceiling" => Ok(ClockRounding::Ceiling),
        _ => Err(CompileError::new(CompileReason::PlanInvalid)),
    }
}

fn feedback_kind(value: &str) -> Result<FeedbackBoundaryKind, CompileError> {
    match value {
        "delay" => Ok(FeedbackBoundaryKind::Delay),
        "state" => Ok(FeedbackBoundaryKind::State),
        _ => Err(CompileError::new(CompileReason::PlanInvalid)),
    }
}

fn feedback_initialization(value: &str) -> Result<FeedbackInitialization, CompileError> {
    match value {
        "empty" => Ok(FeedbackInitialization::Empty),
        "initial-value" => Ok(FeedbackInitialization::InitialValue),
        _ => Err(CompileError::new(CompileReason::PlanInvalid)),
    }
}

fn feedback_replay_gap(value: &str) -> Result<FeedbackReplayGapPolicy, CompileError> {
    match value {
        "fail" => Ok(FeedbackReplayGapPolicy::Fail),
        "reset" => Ok(FeedbackReplayGapPolicy::Reset),
        "wait" => Ok(FeedbackReplayGapPolicy::Wait),
        _ => Err(CompileError::new(CompileReason::PlanInvalid)),
    }
}

fn feedback_terminal(value: &str) -> Result<FeedbackTerminalPolicy, CompileError> {
    match value {
        "drop-retained" => Ok(FeedbackTerminalPolicy::DropRetained),
        "drain-retained" => Ok(FeedbackTerminalPolicy::DrainRetained),
        _ => Err(CompileError::new(CompileReason::PlanInvalid)),
    }
}

fn supervision_action(
    value: &SupervisionActionDocument,
) -> Result<AdmittedSupervisionAction<'_>, CompileError> {
    Ok(AdmittedSupervisionAction {
        kind: match value.kind.as_str() {
            "propagate" => SupervisionActionKind::Propagate,
            "stop-scope" => SupervisionActionKind::StopScope,
            "restart-same" => SupervisionActionKind::RestartSame,
            "retry-same" => SupervisionActionKind::RetrySame,
            "activate-declared-fallback" => SupervisionActionKind::ActivateDeclaredFallback,
            "continue-declared-degraded-mode" => {
                SupervisionActionKind::ContinueDeclaredDegradedMode
            }
            "request-operator-action" => SupervisionActionKind::RequestOperatorAction,
            _ => return Err(CompileError::new(CompileReason::InvalidInput)),
        },
        target: value.target.as_deref().map(id).transpose()?,
        maximum_uses: value.maximum_uses,
        permits_effect_replay: value.permits_effect_replay,
        preserves_required_guarantees: value.preserves_required_guarantees,
        requires_new_epoch: value.requires_new_epoch,
    })
}

fn supervision_scope(value: &str) -> Result<SupervisionScope, CompileError> {
    match value {
        "child" => Ok(SupervisionScope::Child),
        "named-group" => Ok(SupervisionScope::NamedGroup),
        "composite-boundary" => Ok(SupervisionScope::CompositeBoundary),
        "replicated-child" => Ok(SupervisionScope::ReplicatedChild),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn supervision_failure_mode(value: &str) -> Result<SupervisionFailureMode, CompileError> {
    match value {
        "fail-together" => Ok(SupervisionFailureMode::FailTogether),
        "isolated-optional" => Ok(SupervisionFailureMode::IsolatedOptional),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn executor(value: &str) -> Result<ExecutorKind, CompileError> {
    match value {
        "native-in-process" => Ok(ExecutorKind::NativeInProcess),
        "wasm-component" => Ok(ExecutorKind::WasmComponent),
        "ffi-dynamic-library" => Ok(ExecutorKind::FfiDynamicLibrary),
        "process" => Ok(ExecutorKind::Process),
        "firmware" => Ok(ExecutorKind::Firmware),
        "remote-endpoint" => Ok(ExecutorKind::RemoteEndpoint),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn execution_guarantee(value: &str) -> Result<ExecutionGuarantee, CompileError> {
    match value {
        "unsupported" => Ok(ExecutionGuarantee::Unsupported),
        "observed" => Ok(ExecutionGuarantee::Observed),
        "guaranteed" => Ok(ExecutionGuarantee::Guaranteed),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn isolation_profile(value: &str) -> Result<IsolationProfile, CompileError> {
    match value {
        "step-native" => Ok(IsolationProfile::StepNative),
        "isolated-cooperative" => Ok(IsolationProfile::IsolatedCooperative),
        "isolated-preemptible" => Ok(IsolationProfile::IsolatedPreemptible),
        "isolated-terminable" => Ok(IsolationProfile::IsolatedTerminable),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn boundedness_profile(value: &str) -> Result<BoundednessProfile, CompileError> {
    match value {
        "hard" => Ok(BoundednessProfile::Hard),
        "observed" => Ok(BoundednessProfile::Observed),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn cancellation_guarantee(value: &str) -> Result<CancellationGuarantee, CompileError> {
    match value {
        "bounded" => Ok(CancellationGuarantee::Bounded),
        "cooperative" => Ok(CancellationGuarantee::Cooperative),
        "unbounded" => Ok(CancellationGuarantee::Unbounded),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn ownership_model(value: &str) -> Result<OwnershipModel, CompileError> {
    match value {
        "owned" => Ok(OwnershipModel::Owned),
        "borrowed" => Ok(OwnershipModel::Borrowed),
        "shared-handle" => Ok(OwnershipModel::SharedHandle),
        "exclusive-handle" => Ok(OwnershipModel::ExclusiveHandle),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn handle_disposition(value: &str) -> Result<HandleDisposition, CompileError> {
    match value {
        "none" => Ok(HandleDisposition::None),
        "explicit-dispose" => Ok(HandleDisposition::ExplicitDispose),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn memory_category(value: &str) -> Result<MemoryCategory, CompileError> {
    match value {
        "retained" => Ok(MemoryCategory::Retained),
        "step-scratch" => Ok(MemoryCategory::StepScratch),
        "port-transactions" => Ok(MemoryCategory::PortTransactions),
        "pending-operations" => Ok(MemoryCategory::PendingOperations),
        "host-services" => Ok(MemoryCategory::HostServices),
        "foreign-runtime" => Ok(MemoryCategory::ForeignRuntime),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn memory_accounting(value: &str) -> Result<MemoryAccounting, CompileError> {
    match value {
        "executor-allocated" => Ok(MemoryAccounting::ExecutorAllocated),
        "backend-bounded" => Ok(MemoryAccounting::BackendBounded),
        "externally-bounded" => Ok(MemoryAccounting::ExternallyBounded),
        "observed-only" => Ok(MemoryAccounting::ObservedOnly),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn tie_policy(value: &str) -> Result<ResolverTiePolicy, CompileError> {
    match value {
        "reject-ambiguous" => Ok(ResolverTiePolicy::RejectAmbiguous),
        "lowest-canonical-identity" => Ok(ResolverTiePolicy::LowestCanonicalIdentity),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn passport_status(value: &str) -> Result<PassportStatus, CompileError> {
    match value {
        "active" => Ok(PassportStatus::Active),
        "suspended" => Ok(PassportStatus::Suspended),
        "revoked" => Ok(PassportStatus::Revoked),
        "retired" => Ok(PassportStatus::Retired),
        "compromised" => Ok(PassportStatus::Compromised),
        "gap" => Ok(PassportStatus::Gap),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn delegation_policy(value: &str) -> Result<DelegationPolicy, CompileError> {
    match value {
        "none" => Ok(DelegationPolicy::None),
        "same-host-descendants" => Ok(DelegationPolicy::SameHostDescendants),
        "cross-host-descendants" => Ok(DelegationPolicy::CrossHostDescendants),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn stop_policy(value: &str) -> Result<StopPolicy, CompileError> {
    match value {
        "drain" => Ok(StopPolicy::Drain),
        "abort" => Ok(StopPolicy::Abort),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn parse_digest(value: &str) -> Result<ArtifactDigest, CompileError> {
    Ok(ArtifactDigest::from_bytes(parse_sha256(value)?))
}

fn parse_hash(value: &str) -> Result<SemanticHash, CompileError> {
    Ok(SemanticHash::from_bytes(parse_sha256(value)?))
}

fn parse_sha256(value: &str) -> Result<[u8; 32], CompileError> {
    let value = value
        .strip_prefix("sha256:")
        .ok_or_else(|| CompileError::new(CompileReason::InvalidInput))?;
    if value.len() != 64 {
        return Err(CompileError::new(CompileReason::InvalidInput));
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Ok(bytes)
}

fn nibble(byte: u8) -> Result<u8, CompileError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    output
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileReason {
    UnsupportedInput,
    InvalidInput,
    SourceInvalid,
    LoweringFailed,
    UnresolvedSelector,
    ResolutionFailed,
    BudgetInvalid,
    PlanInvalid,
    SourceLimitExceeded,
    ExecutionArrangement,
    Containment(ContainmentReason),
    PolicyBudget(PolicyBudgetReason),
    HazardClosure(HazardClosureReason),
    Genesis(GenesisReason),
}

impl CompileReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedInput => "CND-CMP-001",
            Self::InvalidInput => "CND-CMP-002",
            Self::SourceInvalid => "CND-CMP-003",
            Self::LoweringFailed => "CND-CMP-004",
            Self::UnresolvedSelector => "CND-CMP-005",
            Self::ResolutionFailed => "CND-CMP-006",
            Self::BudgetInvalid => "CND-CMP-007",
            Self::PlanInvalid => "CND-CMP-008",
            Self::SourceLimitExceeded => "CND-CMP-009",
            Self::ExecutionArrangement => "CND-CMP-010",
            Self::Containment(reason) => reason.code(),
            Self::PolicyBudget(reason) => reason.code(),
            Self::HazardClosure(reason) => reason.code(),
            Self::Genesis(reason) => reason.code(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileHazardProofNode {
    pub parent: Option<u8>,
    pub kind: &'static str,
    pub descriptor: String,
    pub effect: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileProviderDenial {
    pub provider: String,
    pub availability: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileError {
    reason: CompileReason,
    hazard_proof: Vec<CompileHazardProofNode>,
    provider_denial: Option<CompileProviderDenial>,
}

impl CompileError {
    fn new(reason: CompileReason) -> Self {
        Self {
            reason,
            hazard_proof: Vec::new(),
            provider_denial: None,
        }
    }

    fn hazard(
        denial: conduit_core::HazardClosureDenial<'_>,
        proof: &[Option<HazardProofNode<'_>>],
    ) -> Self {
        let hazard_proof = proof
            .iter()
            .flatten()
            .map(|node| CompileHazardProofNode {
                parent: node.parent,
                kind: match node.kind {
                    HazardProofKind::Rule => "rule",
                    HazardProofKind::Effect => "effect",
                    HazardProofKind::Flow => "flow",
                    HazardProofKind::Permit => "permit",
                },
                descriptor: node.descriptor.to_string(),
                effect: node.effect.map(|effect| effect.to_string()),
            })
            .collect();
        Self {
            reason: CompileReason::HazardClosure(denial.reason),
            hazard_proof,
            provider_denial: None,
        }
    }

    fn provider(reason: GenesisReason, provider: &str, selection: ProviderSelection) -> Self {
        let availability = match selection {
            ProviderSelection::Available => "available",
            ProviderSelection::Absent => "absent",
            ProviderSelection::Disabled => "disabled",
            ProviderSelection::Unsupported => "unsupported",
            ProviderSelection::TraitMismatch => "trait-mismatch",
        };
        Self {
            reason: CompileReason::Genesis(reason),
            hazard_proof: Vec::new(),
            provider_denial: Some(CompileProviderDenial {
                provider: provider.to_owned(),
                availability,
            }),
        }
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.reason.code()
    }

    #[must_use]
    pub fn hazard_proof(&self) -> &[CompileHazardProofNode] {
        &self.hazard_proof
    }

    #[must_use]
    pub fn provider_denial(&self) -> Option<&CompileProviderDenial> {
        self.provider_denial.as_ref()
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.reason {
            CompileReason::UnsupportedInput => "unsupported compile-input schema",
            CompileReason::InvalidInput => "compile-input identity or descriptor is invalid",
            CompileReason::SourceInvalid => "module or source closure is invalid",
            CompileReason::LoweringFailed => "semantic lowering failed",
            CompileReason::UnresolvedSelector => "an implementation selector remains unresolved",
            CompileReason::ResolutionFailed => {
                "implementation, artifact, host, or authority resolution failed"
            }
            CompileReason::BudgetInvalid => {
                "resource, queue, authority, or transition budget is invalid"
            }
            CompileReason::PlanInvalid => "exact plan construction or portable validation failed",
            CompileReason::SourceLimitExceeded => {
                "entry source or explicit module closure limit exceeded"
            }
            CompileReason::ExecutionArrangement => {
                "no finite physical execution arrangement satisfies the exact plan"
            }
            CompileReason::Containment(ContainmentReason::ApprovalMissing) => {
                "administrative effect is missing its exact independent approval proof"
            }
            CompileReason::Containment(ContainmentReason::SelfSupporting) => {
                "administrative approval is supported by the requesting or benefiting subject"
            }
            CompileReason::Containment(reason) => match reason {
                ContainmentReason::SuccessorSelfAuthorized => {
                    "successor activation lacks authority independent of the active plan"
                }
                ContainmentReason::FailureDomainInsufficient => {
                    "administrative approval threshold lacks independent failure domains"
                }
                ContainmentReason::SubjectMismatch | ContainmentReason::ApprovalReplay => {
                    "administrative approval is bound to a different exact subject"
                }
                _ => "administrative containment proof is invalid or unavailable",
            },
            CompileReason::PolicyBudget(PolicyBudgetReason::CapacityExceeded) => {
                "persistent policy budget denied the protected effect"
            }
            CompileReason::PolicyBudget(PolicyBudgetReason::StaleStatus) => {
                "persistent policy budget status is stale"
            }
            CompileReason::PolicyBudget(PolicyBudgetReason::LedgerUnavailable) => {
                "persistent policy budget ledger is unavailable"
            }
            CompileReason::PolicyBudget(PolicyBudgetReason::EvidenceExhausted) => {
                "persistent policy budget evidence capacity is exhausted before the effect"
            }
            CompileReason::PolicyBudget(_) => {
                "persistent policy budget proof is invalid or unavailable"
            }
            CompileReason::HazardClosure(HazardClosureReason::PermitMissing)
            | CompileReason::HazardClosure(HazardClosureReason::ToxicCombination) => {
                "whole-plan effect closure contains a policy-forbidden combination"
            }
            CompileReason::HazardClosure(HazardClosureReason::ProofStorageExceeded) => {
                "whole-plan effect-closure proof storage is exhausted before start"
            }
            CompileReason::HazardClosure(HazardClosureReason::SearchLimitExceeded) => {
                "whole-plan effect-closure analysis exceeded its finite search bound"
            }
            CompileReason::HazardClosure(_) => {
                "whole-plan effect-closure proof or exact permit is invalid"
            }
            CompileReason::Genesis(GenesisReason::ProviderUnavailable) => {
                "required provider is intentionally absent, disabled, or unsupported"
            }
            CompileReason::Genesis(GenesisReason::DangerousProviderEnabledByDefault) => {
                "reference distribution enables a dangerous provider by default"
            }
            CompileReason::Genesis(GenesisReason::QuarantineViolated) => {
                "enrolled member is not in the required authority-free quarantine"
            }
            CompileReason::Genesis(GenesisReason::RecoveryWidened) => {
                "reset or recovery would widen authority"
            }
            CompileReason::Genesis(_) => "safe genesis or reference-distribution proof is invalid",
        };
        formatter.write_str(message)?;
        if let Some(rule) = self.hazard_proof.iter().find(|node| node.kind == "rule") {
            write!(formatter, "; rule {}", rule.descriptor)?;
            let mut effects = self
                .hazard_proof
                .iter()
                .filter_map(|node| node.effect.as_deref());
            if let Some(first) = effects.next() {
                write!(formatter, "; effects {first}")?;
                for effect in effects {
                    write!(formatter, ", {effect}")?;
                }
            }
        }
        if let Some(denial) = &self.provider_denial {
            write!(
                formatter,
                "; provider {}; availability {}",
                denial.provider, denial.availability
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for CompileError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use conduit_core::{
        ARTIFACT_MANIFEST_SCHEMA_VERSION, EFFECT_COMMIT_PROFILE_SCHEMA_VERSION,
        IMPLEMENTATION_MANIFEST_SCHEMA_VERSION, RESOURCE_LEASE_SCHEMA_VERSION,
    };
    use conduit_panel::parse;

    fn hash(byte: u8) -> String {
        SemanticHash::from_bytes([byte; 32]).to_string()
    }

    fn pin_doc(id: &str, byte: u8) -> PinDocument {
        PinDocument {
            id: id.to_owned(),
            schema_version: 0,
            semantic_hash: hash(byte),
        }
    }

    fn reseal_test_execution_arrangement(document: &mut ExactPlanDocument) {
        let mut arrangement = document.execution_arrangement().unwrap();
        arrangement.plan_identity = parse_hash(&document.identity).unwrap();
        for region in &mut arrangement.regions {
            region.independent = false;
        }
        arrangement.identity = arrangement.computed_identity();
        document.execution_arrangement = resolved_execution_arrangement_document(&arrangement);
    }

    fn current_effect_contracts(
        holder: &str,
        operation: &str,
        resource_binding: &str,
        run: &str,
        byte: u8,
    ) -> (ResourceLeaseDocument, EffectCommitProfileDocument) {
        let lease_id = format!("fixture/lease-{byte}");
        (
            ResourceLeaseDocument {
                schema_version: RESOURCE_LEASE_SCHEMA_VERSION,
                id: lease_id.clone(),
                resource_binding: resource_binding.to_owned(),
                holder: holder.to_owned(),
                run: run.to_owned(),
                epoch: 0,
                scope: format!("fixture/scope-{byte}"),
                sharing: "exclusive".to_owned(),
                maximum_holders: 1,
                reservation: BudgetDocument {
                    memory_bytes: 1,
                    ..BudgetDocument::default()
                },
                time_basis: "clock/compile".to_owned(),
                issued_at_tick: 10,
                expires_at_tick: 20,
                revocation_grace_ticks: 1,
                cleanup_ticks: 2,
                maximum_operations: 4,
                maximum_evidence_events: 8,
                cleanup_escalation: pin_doc("fixture/cleanup-escalation", byte),
                foreign_retention: "unsupported".to_owned(),
                foreign_maximum_bytes: 0,
                foreign_release_ticks: 0,
            },
            EffectCommitProfileDocument {
                schema_version: EFFECT_COMMIT_PROFILE_SCHEMA_VERSION,
                id: format!("fixture/effect-profile-{byte}"),
                operation: operation.to_owned(),
                resource_lease: lease_id,
                commit_boundary: pin_doc("fixture/commit-boundary", byte.wrapping_add(1)),
                idempotency: "reconcile-before-retry".to_owned(),
                unknown_commit: "reconcile".to_owned(),
                discontinuity: "reconcile-required".to_owned(),
                cleanup: pin_doc("fixture/cleanup", byte.wrapping_add(2)),
                maximum_attempts: 2,
                evidence_events_per_attempt: 2,
            },
        )
    }

    fn hazardous_host_doc() -> HazardousHostBindingDocument {
        HazardousHostBindingDocument {
            host: "host-local".to_owned(),
            profile: HazardousHostProfileDocument {
                schema_version: conduit_core::HAZARDOUS_HOST_PROFILE_SCHEMA_VERSION,
                identity: hash(0),
                descriptor: pin_doc("profile.fixture-hazardous-host", 147),
                safe_state: pin_doc("domain.fixture-safe-state", 148),
                inhibit_boundary: pin_doc("host.fixture-inhibit", 149),
                watchdog: pin_doc("host.fixture-watchdog", 150),
                effect_boundary: pin_doc("host.fixture-effect-boundary", 151),
                command_effect_class: pin_doc("effect.fixture-command", 152),
                clear_effect_class: pin_doc("effect.fixture-clear", 153),
                clear_operation: pin_doc("operation.fixture-clear", 154),
                clear_ceremony: pin_doc("ceremony.fixture-physical", 155),
                time_basis: "clock/compile".to_owned(),
                maximum_command_horizon_ticks: 5,
                maximum_observation_age_ticks: 10,
                maximum_evidence_records: 16,
                require_physical_presence_to_clear: true,
                require_isolated_implementation: true,
                envelope: vec![OperatingEnvelopeLimitDocument {
                    dimension: pin_doc("domain.fixture-axis", 156),
                    minimum: -5,
                    maximum: 5,
                }],
            },
            observation: InhibitObservationDocument {
                schema_version: conduit_core::INHIBIT_OBSERVATION_SCHEMA_VERSION,
                identity: hash(0),
                profile_identity: hash(0),
                host: "host-local".to_owned(),
                safe_state: pin_doc("domain.fixture-safe-state", 148),
                inhibit_boundary: pin_doc("host.fixture-inhibit", 149),
                watchdog: pin_doc("host.fixture-watchdog", 150),
                effect_boundary: pin_doc("host.fixture-effect-boundary", 151),
                time_basis: "clock/compile".to_owned(),
                observed_at_tick: 10,
                valid_until_tick: 15,
                latch_generation: 1,
                latch_state: "safe-disarmed".to_owned(),
                independent_from_plan: true,
                local_safe_path: true,
                survives_executor_loss: true,
                survives_partition: true,
                graph_cannot_replace: true,
                confinement: "effect-boundary-enforced".to_owned(),
            },
        }
    }

    fn administrative_principal_doc(
        entity: &str,
        key: &str,
        plan: u8,
    ) -> AdministrativePrincipalDocument {
        AdministrativePrincipalDocument {
            realm: "realm.alpha".to_owned(),
            entity: entity.to_owned(),
            key: key.to_owned(),
            profile: pin_doc("profile.member", 102),
            source_plan: hash(plan),
            source_epoch: 7,
        }
    }

    fn administrative_subject_doc() -> AdministrativeSubjectDocument {
        AdministrativeSubjectDocument {
            realm: "realm.alpha".to_owned(),
            entity: "target".to_owned(),
            plan: hash(104),
            epoch: 7,
            artifact: None,
            budget: None,
        }
    }

    fn administrative_proof_doc(
        effect_class: PinDocument,
    ) -> (AdministrativeSubjectDocument, AdministrativeProofDocument) {
        let subject = administrative_subject_doc();
        let approver = administrative_principal_doc("approver", "key.approver", 105);
        let policy = ContainmentPolicyDocument {
            schema_version: 0,
            identity: String::new(),
            descriptor: pin_doc("policy.containment", 106),
            effect_class: effect_class.clone(),
            approvers: vec![AdministrativeApproverDocument {
                realm: approver.realm.clone(),
                entity: approver.entity.clone(),
                key: approver.key.clone(),
                profile: approver.profile.clone(),
                failure_domain: pin_doc("failure.rack.one", 107),
            }],
            committer: AdministrativeApproverDocument {
                realm: "realm.alpha".to_owned(),
                entity: "committer".to_owned(),
                key: "key.committer".to_owned(),
                profile: pin_doc("profile.member", 102),
                failure_domain: pin_doc("failure.committer", 112),
            },
            executor: AdministrativeApproverDocument {
                realm: "realm.alpha".to_owned(),
                entity: "executor".to_owned(),
                key: "key.executor".to_owned(),
                profile: pin_doc("profile.member", 102),
                failure_domain: pin_doc("failure.executor", 113),
            },
            minimum_approvals: 1,
            minimum_failure_domains: 1,
            requester_independence: true,
            beneficiary_independence: true,
            successor_independence: true,
            delegation_ceiling: None,
            ceremony: None,
        };
        let proposal = AdministrativeProposalDocument {
            schema_version: 0,
            identity: String::new(),
            id: "proposal.one".to_owned(),
            effect_class,
            operation: pin_doc("operation.exact", 108),
            requester: administrative_principal_doc("requester", "key.requester", 103),
            subject: subject.clone(),
            beneficiaries: vec![subject.clone()],
            predecessor_plan: None,
            delegation: None,
            protected_handle: None,
            ceremony: None,
            time_basis: "clock/compile".to_owned(),
            created_at_tick: 10,
            expires_at_tick: 19,
        };
        let approval = AdministrativeApprovalDocument {
            schema_version: 0,
            identity: String::new(),
            id: "approval.one".to_owned(),
            proposal_identity: String::new(),
            policy_identity: String::new(),
            approver,
            failure_domain: pin_doc("failure.rack.one", 107),
            time_basis: "clock/compile".to_owned(),
            issued_at_tick: 10,
            expires_at_tick: 19,
            status: "current".to_owned(),
        };
        let proof = AdministrativeProofDocument {
            proposal,
            policy,
            approvals: vec![approval],
            commit: AdministrativeCommitDocument {
                schema_version: 0,
                identity: String::new(),
                id: "commit.one".to_owned(),
                proposal_identity: String::new(),
                policy_identity: String::new(),
                approvals: Vec::new(),
                committed_by: administrative_principal_doc("committer", "key.committer", 109),
                committed_at_tick: 11,
            },
            execution: AdministrativeExecutionDocument {
                schema_version: 0,
                identity: String::new(),
                id: "execution.one".to_owned(),
                proposal_identity: String::new(),
                commit_identity: String::new(),
                executor: administrative_principal_doc("executor", "key.executor", 110),
                time_basis: "clock/compile".to_owned(),
                not_before_tick: 11,
                expires_at_tick: 19,
            },
        };
        (subject, proof)
    }

    fn policy_budget_binding_doc(resource_class: PinDocument) -> PolicyBudgetBindingDocument {
        PolicyBudgetBindingDocument {
            policy: PersistentBudgetPolicyDocument {
                schema_version: 0,
                identity: String::new(),
                descriptor: pin_doc("budget.installation", 120),
                owner: pin_doc("owner.site-operations", 121),
                subject: pin_doc("subject.executable", 122),
                anchor_kind: "host".to_owned(),
                anchor_id: "fixture/host-a".to_owned(),
                action: "fixture/read".to_owned(),
                resource_class,
                time_basis: "clock/compile".to_owned(),
                limits: PolicyBudgetLimitsDocument {
                    current_stock: Some(1),
                    rolling_units: Some(1),
                    rolling_window_ticks: Some(100),
                    lifetime: Some(1),
                },
                reservation_ttl_ticks: 5,
                lease: None,
                audit_id: "audit.installation".to_owned(),
                persistence_profile: pin_doc("persistence.atomic", 123),
                maximum_reservations: 4,
                maximum_evidence_events: 16,
            },
            status: PolicyBudgetStatusDocument {
                schema_version: 0,
                identity: String::new(),
                policy_identity: String::new(),
                ledger: pin_doc("ledger.host-installation", 124),
                checkpoint: hash(125),
                sequence: 4,
                current_stock: 0,
                rolling_window_start: 10,
                rolling_committed: 0,
                lifetime_committed: 0,
                reserved: 0,
                evidence_remaining: 12,
                availability: "available".to_owned(),
                time_basis: "clock/compile".to_owned(),
                observed_at_tick: 10,
                valid_until_tick: 20,
            },
            lease: None,
            required_units: 1,
            check_at_use: true,
        }
    }

    fn profile_doc(ordinal: u8) -> ExecutionProfileDocument {
        ExecutionProfileDocument {
            id: format!("fixture/execution-profile-{ordinal}"),
            schema_version: 0,
            semantic_hash: hash(30),
            boundedness: "hard".to_owned(),
            cancellation: "bounded".to_owned(),
            step_bound_enforced: true,
            limits: ExecutionLimitsDocument {
                max_step_work: 4,
                max_transactions: 1,
                cancellation_ticks: 1,
                ..ExecutionLimitsDocument::default()
            },
            representations: Vec::new(),
            memory_claims: Vec::new(),
            checkpoint: None,
        }
    }

    fn candidate(ordinal: u8, contract_id: &str, contract_hash: SemanticHash) -> CandidateDocument {
        let artifact_id = format!("fixture/artifact-{ordinal}");
        let artifact_digest = ArtifactDigest::from_bytes([ordinal; 32]).to_string();
        CandidateDocument {
            implementation: ImplementationDocument {
                schema_version: IMPLEMENTATION_MANIFEST_SCHEMA_VERSION,
                identity: String::new(),
                id: format!("fixture/implementation-{ordinal}"),
                implementation_version: "1.0.0".to_owned(),
                semantic_contract: PinDocument {
                    id: contract_id.to_owned(),
                    schema_version: 0,
                    semantic_hash: contract_hash.to_string(),
                },
                executor: "native-in-process".to_owned(),
                entrypoint_name: "run".to_owned(),
                entrypoint_adapter: "conduit/native-step".to_owned(),
                entrypoint_abi: "conduit/native".to_owned(),
                runtime_protocol_version: 0,
                execution_profile: pin_doc("fixture/execution-profile", 30),
                artifacts: vec![ArtifactReferenceDocument {
                    id: artifact_id.clone(),
                    digest: artifact_digest.clone(),
                    role: "implementation".to_owned(),
                    required: true,
                }],
                required_interfaces: Vec::new(),
                provided_interfaces: Vec::new(),
                required_authorities: Vec::new(),
                required_effects: Vec::new(),
                minimum_plan_version: 0,
                maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION,
                minimum_runtime_protocol: 1,
                maximum_runtime_protocol: 1,
                coexistence_memory_bytes: 0,
            },
            execution_profile: profile_doc(ordinal),
            artifacts: vec![ArtifactDocument {
                schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
                identity: String::new(),
                id: artifact_id,
                digest: artifact_digest,
                media_type: "application/octet-stream".to_owned(),
                byte_size: 1,
                target: None,
                abi: None,
                builder: "fixture/builder".to_owned(),
                source_digest: ArtifactDigest::from_bytes([40; 32]).to_string(),
                build_recipe_digest: ArtifactDigest::from_bytes([41; 32]).to_string(),
                reproducible: false,
                license_expressions: vec!["MIT".to_owned()],
            }],
            host_report: HostReportDocument {
                schema_version: conduit_core::CAPABILITY_REPORT_SCHEMA_VERSION,
                identity: String::new(),
                id: format!("fixture/report-{ordinal}"),
                host: "fixture/host-local".to_owned(),
                boot_id: "fixture/host-local-boot".to_owned(),
                reporter: pin_doc("fixture/reporter", 50),
                trust: pin_doc("fixture/report-trust", 51),
                membership: None,
                time_basis: "clock/compile".to_owned(),
                observed_at_tick: 10,
                valid_until_tick: 20,
                available: BudgetDocument {
                    memory_bytes: 4096,
                    storage_bytes: 4096,
                    cpu_units: 16,
                    timers: 4,
                    transports: 4,
                    checkpoints: 4,
                    evidence_bytes: 4096,
                },
                capabilities: Vec::new(),
                resources: Vec::new(),
                topology: Vec::new(),
                execution_placements: vec![ExecutionPlacementObservationDocument {
                    id: format!("placement/compile-{ordinal}"),
                    provider: pin_doc("provider/fixed-hosted-lanes", 52),
                    authority_boundary: pin_doc("boundary/compile-authority", 53),
                    resource_boundary: pin_doc("boundary/compile-resource", 54),
                    lifecycle_boundary: pin_doc("boundary/compile-lifecycle", 55),
                    failure_boundary: pin_doc("boundary/compile-failure", 56),
                    generation: 1,
                    isolation: "step-native".to_owned(),
                    memory_containment: "observed".to_owned(),
                    regain_control: "observed".to_owned(),
                    effect_fencing: "unsupported".to_owned(),
                    stop_execution: "unsupported".to_owned(),
                    reclaim_resources: "unsupported".to_owned(),
                    maximum_regain_control_ticks: 0,
                }],
                execution_lanes: vec![ExecutionLaneObservationDocument {
                    id: format!("lane/compile-{ordinal}"),
                    placement: format!("placement/compile-{ordinal}"),
                    placement_generation: 1,
                    generation: 1,
                    independent_progress: "guaranteed".to_owned(),
                    simultaneous_execution: "guaranteed".to_owned(),
                    preemption: "observed".to_owned(),
                    termination: "unsupported".to_owned(),
                    ready_slots: 64,
                    wake_slots: 64,
                    proposal_slots: 64,
                    commit_slots: 64,
                    timer_slots: 4,
                    scratch_bytes: 1024,
                    stack_bytes: 1024,
                    evidence_slots: 512,
                }],
                supported_executors: vec!["native-in-process".to_owned()],
                supported_targets: Vec::new(),
                supported_abis: Vec::new(),
                minimum_plan_version: 0,
                maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION,
                current_constraints: Vec::new(),
            },
            allocation: BudgetDocument {
                memory_bytes: 32,
                cpu_units: 1,
                ..BudgetDocument::default()
            },
            lifecycle_policy: pin_doc("conduit/finite-lifecycle", 60),
            capabilities: Vec::new(),
            resources: Vec::new(),
            topology: Vec::new(),
            granted_authorities: Vec::new(),
            authorities: Vec::new(),
        }
    }

    fn compile_input(source: &str, panel: &conduit_panel::Panel) -> CompileInput {
        let topology = Registry::compatibility_demo()
            .resolve(panel)
            .unwrap()
            .exact_topology()
            .unwrap();
        let mut contracts = BTreeMap::new();
        for node in &topology.nodes {
            contracts
                .entry(node.contract_id.clone())
                .or_insert(node.contract_hash);
        }
        let candidates = contracts
            .into_iter()
            .enumerate()
            .map(|(index, (id, hash))| candidate(index as u8 + 1, &id, hash))
            .collect();
        let mut input = CompileInput {
            schema: COMPILE_INPUT_SCHEMA.to_owned(),
            schema_version: COMPILE_INPUT_SCHEMA_VERSION,
            identity: String::new(),
            entry_uri: "mem://compile/entry.panel".to_owned(),
            selected_root: panel.selected_root.clone(),
            source_limits: CompileSourceLimits::default(),
            modules: vec![CompileModuleDocument {
                canonical_uri: "mem://compile/entry.panel".to_owned(),
                content_hash: String::new(),
                source: source.to_owned(),
            }],
            catalog: builtin_catalog_document().unwrap(),
            pool_bindings: Vec::new(),
            supervision_bindings: Vec::new(),
            hazard_closure: None,
            distribution: None,
            evidence_provider: None,
            watch_admissions: Vec::new(),
            source_semantic_hash: topology.source_semantic_hash.to_string(),
            resolver: pin_doc("conduit/exact-compiler-resolver", 70),
            resolver_policy_hash: String::new(),
            time_basis: "clock/compile".to_owned(),
            current_tick: 12,
            plan_budget: BudgetDocument {
                memory_bytes: 2 * 1024 * 1024,
                storage_bytes: 16 * 1024,
                cpu_units: 64,
                timers: 16,
                transports: 16,
                checkpoints: 16,
                evidence_bytes: 16 * 1024,
            },
            execution_arrangement: fixed_hosted_execution_arrangement_policy(),
            maximum_authority_bindings: 64,
            maximum_transition_memory_bytes: 1024 * 1024,
            maximum_search_states: 128,
            tie_policy: "lowest-canonical-identity".to_owned(),
            required_realm: None,
            trusted_entities: Vec::new(),
            trusted_status_reporters: Vec::new(),
            require_active_passport: false,
            implementation_preference: Vec::new(),
            candidates,
        };
        input.seal().unwrap();
        input
    }

    fn evidence_provider_document() -> EvidenceProviderBindingDocument {
        EvidenceProviderBindingDocument {
            implementation: pin_doc("fixture/exact-evidence-provider", 211),
            artifact: PlanArtifactDocument {
                id: "fixture/exact-evidence-artifact".to_owned(),
                digest: ArtifactDigest::from_bytes([212; 32]).to_string(),
            },
            host_observation: PlanHostDocument {
                id: "fixture/exact-evidence-host-observation".to_owned(),
                host: "fixture/host-local".to_owned(),
                boot_id: "fixture/host-local-boot".to_owned(),
                semantic_hash: hash(213),
                time_basis: "clock/compile".to_owned(),
                observed_at_tick: 10,
                valid_until_tick: 20,
            },
            store_kind: "fixture/evidence-store".to_owned(),
            store_id: "fixture/evidence-store-a".to_owned(),
            store_generation: 4,
            grant_hash: hash(214),
            time_basis: "clock/compile".to_owned(),
        }
    }

    fn supervision_binding(
        lowered: &CompileLoweredTopologyBase,
        allocation_memory_bytes: u64,
    ) -> SupervisionBindingDocument {
        SupervisionBindingDocument {
            instance: "root/supervision/subject".to_owned(),
            source_binding_hash: lowered.supervisions[0].semantic_hash.to_string(),
            id: "supervision.subject".to_owned(),
            scope: "child".to_owned(),
            subject: "root/subject".to_owned(),
            handler: "root/handler".to_owned(),
            members: Vec::new(),
            failure_mode: "fail-together".to_owned(),
            outer: None,
            policy: pin_doc("conduit/bounded-supervision", 201),
            observation_contract: pin_doc("std/terminal", 202),
            decision_contract: pin_doc("supervision/decision", 203),
            actions: vec![
                SupervisionActionDocument {
                    kind: "propagate".to_owned(),
                    target: None,
                    maximum_uses: 4,
                    permits_effect_replay: false,
                    preserves_required_guarantees: true,
                    requires_new_epoch: false,
                },
                SupervisionActionDocument {
                    kind: "restart-same".to_owned(),
                    target: None,
                    maximum_uses: 2,
                    permits_effect_replay: false,
                    preserves_required_guarantees: true,
                    requires_new_epoch: false,
                },
                SupervisionActionDocument {
                    kind: "activate-declared-fallback".to_owned(),
                    target: Some("fallback".to_owned()),
                    maximum_uses: 1,
                    permits_effect_replay: false,
                    preserves_required_guarantees: true,
                    requires_new_epoch: false,
                },
            ],
            action_targets: vec![SupervisionTargetDocument {
                choice: "fallback".to_owned(),
                target: "root/fallback".to_owned(),
            }],
            limits: SupervisionLimitsDocument {
                maximum_observations: 4,
                maximum_decisions: 4,
                maximum_in_flight: 2,
                maximum_cause_depth: 4,
                maximum_nested_depth: 4,
                maximum_handler_ticks: 10,
                maximum_recovery_ticks: 20,
                restart_window_ticks: 10,
                backoff_ticks: 2,
                cooldown_ticks: 3,
                operator_wait_ticks: 5,
                maximum_evidence_events: 32,
                observation_bytes: 256,
                decision_bytes: 64,
                scratch_bytes: 128,
            },
            allocation: BudgetDocument {
                memory_bytes: allocation_memory_bytes,
                cpu_units: 1,
                timers: 3,
                evidence_bytes: 32,
                ..BudgetDocument::default()
            },
            deadline_timer: "supervision-deadline".to_owned(),
            backoff_timer: "supervision-backoff".to_owned(),
            cooldown_timer: "supervision-cooldown".to_owned(),
            cleanup: "abort".to_owned(),
            required_behavior: true,
        }
    }

    fn reference_distribution_document() -> ReferenceDistributionDocument {
        ReferenceDistributionDocument {
            schema: REFERENCE_DISTRIBUTION_DOCUMENT_SCHEMA.to_owned(),
            schema_version: conduit_core::DISTRIBUTION_PROFILE_SCHEMA_VERSION,
            identity: String::new(),
            descriptor: pin_doc("distribution.reference", 180),
            kind: "hosted".to_owned(),
            genesis_profile: hash(181),
            control_recorder: pin_doc("recorder.genesis", 182),
            provider_enablement_effect_class: pin_doc("effect.provider-enable", 183),
            provider_enablement_operation: pin_doc("operation.provider-enable", 184),
            providers: vec![
                DistributionProviderDocument {
                    provider: pin_doc("provider.safe", 185),
                    artifact: None,
                    availability: "enabled".to_owned(),
                    traits: ProviderRiskTraitsDocument::default(),
                },
                DistributionProviderDocument {
                    provider: pin_doc("provider.firmware", 186),
                    artifact: Some(format!("sha256:{}", "bb".repeat(32))),
                    availability: "disabled".to_owned(),
                    traits: ProviderRiskTraitsDocument {
                        firmware_mutation: true,
                        ..ProviderRiskTraitsDocument::default()
                    },
                },
            ],
            maximum_provider_enablement_ticks: 20,
            maximum_provider_install_attempts: 2,
            maximum_evidence_events: 16,
            requirements: vec![ProviderRequirementDocument {
                provider: pin_doc("provider.safe", 185),
                traits: ProviderRiskTraitsDocument::default(),
            }],
        }
    }

    #[test]
    fn identical_explicit_inputs_emit_byte_identical_portable_plans() {
        let source = include_str!("../../../examples/hello.panel");
        let panel = parse(source).unwrap();
        let input = compile_input(source, &panel);
        let first = compile_panel(&panel, &input).unwrap();
        let second = compile_panel(&panel, &input).unwrap();
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
        assert!(first.unresolved_selectors.is_empty());
        first.validate().unwrap();

        let mut mismatched_catalog = input;
        mismatched_catalog.catalog.nodes[0].semantic_hash = hash(99);
        assert_eq!(mismatched_catalog.seal().unwrap_err().code(), "CND-CMP-002");
    }

    #[test]
    fn exact_evidence_provider_round_trips_and_changes_plan_identity() {
        let source = include_str!("../../../examples/hello.panel");
        let panel = parse(source).unwrap();
        let baseline_input = compile_input(source, &panel);
        let baseline = compile_panel(&panel, &baseline_input).unwrap();

        let mut input = baseline_input;
        input.evidence_provider = Some(evidence_provider_document());
        input.seal().unwrap();
        let document = compile_panel(&panel, &input).unwrap();
        assert_ne!(document.identity, baseline.identity);
        assert_eq!(document.evidence_provider, input.evidence_provider);
        assert!(
            document
                .artifacts
                .contains(&document.evidence_provider.as_ref().unwrap().artifact)
        );
        assert!(
            document.host_observations.contains(
                &document
                    .evidence_provider
                    .as_ref()
                    .unwrap()
                    .host_observation
            )
        );

        let encoded = serde_json::to_vec(&document).unwrap();
        let decoded: ExactPlanDocument = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, document);
        decoded.validate().unwrap();

        let mut changed_input = input;
        changed_input
            .evidence_provider
            .as_mut()
            .unwrap()
            .store_generation += 1;
        changed_input.seal().unwrap();
        let changed = compile_panel(&panel, &changed_input).unwrap();
        assert_ne!(changed.identity, document.identity);
    }

    #[test]
    fn watch_admission_is_compiler_input_before_plan_identity_is_sealed() {
        let source = include_str!("../../../examples/hello.panel");
        let panel = parse(source).unwrap();
        let topology = Registry::compatibility_demo()
            .resolve(&panel)
            .unwrap()
            .exact_topology()
            .unwrap();
        let cord = &topology.cords[0];
        let baseline_input = compile_input(source, &panel);
        let baseline_input_identity = baseline_input.identity.clone();
        let baseline = compile_panel(&panel, &baseline_input).unwrap();

        let mut input = baseline_input;
        input.watch_admissions = vec![WatchAdmissionDocument {
            id: format!("watch/{}", cord.id),
            subject_kind: "cord".to_owned(),
            operator: "operator/fixture".to_owned(),
            control_grant_hash: hash(216),
            lease: format!("lease/watch/{}", cord.id),
            cord: Some(cord.id.clone()),
            node: None,
            port: None,
            direction: None,
            representation: PinDocument {
                id: cord.from_port.value_type.contract_id.to_string(),
                schema_version: cord.from_port.value_type.schema_version,
                semantic_hash: cord.from_port.value_type.semantic_hash.to_string(),
            },
            maximum_preview_bytes: cord.max_value_bytes.min(32),
            maximum_history: 1,
            minimum_tick_interval: 1,
            retention: "latest".to_owned(),
            sensitivity_ceiling: "public".to_owned(),
            reveal_action: None,
            reveal_grant_hash: None,
        }];
        input.seal().unwrap();
        assert_ne!(input.identity, baseline_input_identity);

        let watched = compile_panel(&panel, &input).unwrap();
        assert_eq!(watched.watch_admissions, input.watch_admissions);
        assert_ne!(watched.identity, baseline.identity);
        watched.validate().unwrap();
    }

    #[test]
    fn value_clock_and_feedback_plan_round_trips_exactly() {
        let source = include_str!("../../../examples/hello.panel");
        let panel = parse(source).unwrap();
        let input = compile_input(source, &panel);
        let mut document = compile_panel(&panel, &input).unwrap();
        let cord = document.cords.first_mut().unwrap();
        cord.queue_memory_bytes += u64::from(cord.capacity_items) * 16;
        document.schema = PLAN_DOCUMENT_SCHEMA.to_owned();
        document.schema_version = EXECUTION_PLAN_SCHEMA_VERSION;
        document.value_envelopes = vec![ValueEnvelopePolicyDocument {
            cord: cord.id.clone(),
            representation: pin_doc("fixture/value-bytes", 211),
            maximum_payload_bytes: cord.max_value_bytes,
            maximum_envelope_bytes: 16,
            maximum_fragments: 2,
            maximum_fragment_bytes: cord.max_value_bytes.div_ceil(2),
            maximum_timestamps: 1,
            clock_domains: vec!["clock/compile".to_owned()],
            identity_allowed: true,
            correlation_allowed: true,
            causation_allowed: true,
            provenance_allowed: true,
            sensitivity_ceiling: "restricted".to_owned(),
        }];
        document.watch_admissions = vec![WatchAdmissionDocument {
            id: "watch/compile-output".to_owned(),
            subject_kind: "cord".to_owned(),
            operator: "operator/fixture".to_owned(),
            control_grant_hash: hash(215),
            lease: "lease/watch-compile-output".to_owned(),
            cord: Some(cord.id.clone()),
            node: None,
            port: None,
            direction: None,
            representation: pin_doc("fixture/value-bytes", 211),
            maximum_preview_bytes: cord.max_value_bytes.min(32),
            maximum_history: 1,
            minimum_tick_interval: 1,
            retention: "latest".to_owned(),
            sensitivity_ceiling: "public".to_owned(),
            reveal_action: None,
            reveal_grant_hash: None,
        }];
        document.clock_conversions = vec![ClockConversionDocument {
            id: "fixture/device-to-compile-clock".to_owned(),
            source: "fixture/device-clock".to_owned(),
            destination: "clock/compile".to_owned(),
            numerator: 1,
            denominator: 1,
            offset_ticks: 0,
            rounding: "exact".to_owned(),
            maximum_uncertainty_ticks: 1,
            observed_time_basis: "clock/compile".to_owned(),
            observed_tick: 10,
            valid_until_tick: 20,
            authority: "fixture/clock-authority".to_owned(),
        }];
        document.feedback_boundaries = vec![FeedbackBoundaryDocument {
            id: "fixture/delayed-edge".to_owned(),
            node: cord.to.node.clone(),
            cord: cord.id.clone(),
            kind: "delay".to_owned(),
            initialization: "empty".to_owned(),
            initial_items: 0,
            initial_bytes: 0,
            maximum_retained_items: 1,
            maximum_retained_bytes: u64::from(cord.max_value_bytes),
            delay_ticks: 1,
            clock: Some("clock/compile".to_owned()),
            replay_gap: "fail".to_owned(),
            cancellation: pin_doc("fixture/bounded-cancellation", 212),
            terminal: "drop-retained".to_owned(),
        }];
        document.identity = {
            let arena = Bump::new();
            let plan = document.as_plan(&arena).unwrap();
            let mut scratch =
                vec![SemanticHash::from_bytes([0; 32]); plan.validation_scratch_count().unwrap()];
            plan.semantic_hash(&mut scratch).unwrap().to_string()
        };
        reseal_test_execution_arrangement(&mut document);

        document.validate().unwrap();
        let mut denied_reveal = document.clone();
        denied_reveal.watch_admissions[0].sensitivity_ceiling = "restricted".to_owned();
        denied_reveal.watch_admissions[0].reveal_action = Some("conduit/data.present".to_owned());
        denied_reveal.watch_admissions[0].reveal_grant_hash = Some(hash(217));
        denied_reveal.identity = {
            let arena = Bump::new();
            let plan = denied_reveal.as_plan(&arena).unwrap();
            let mut scratch =
                vec![SemanticHash::from_bytes([0; 32]); plan.validation_scratch_count().unwrap()];
            plan.semantic_hash(&mut scratch).unwrap().to_string()
        };
        reseal_test_execution_arrangement(&mut denied_reveal);
        let denied_arena = Bump::new();
        let denied_plan = denied_reveal.as_plan(&denied_arena).unwrap();
        let denied = validate_hosted_execution_plan(
            &denied_plan,
            PlanValidationContext {
                supported_schema_version: denied_reveal.schema_version,
                now: denied_plan.created_at,
            },
        )
        .unwrap_err();
        assert_eq!(denied.code.as_str(), "CND-WAT-004");

        let bytes = serde_json::to_vec(&document).unwrap();
        let decoded: ExactPlanDocument = serde_json::from_slice(&bytes).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded, document);
    }

    #[test]
    fn typed_supervision_compiles_and_round_trips_exactly() {
        let source = "panel 0\n\
            subject: std/literal { value = \"primary\" }\n\
            subject_sink: display/text\n\
            fallback: std/literal { value = \"fallback\" }\n\
            fallback_sink: display/text\n\
            handler: supervision/supervisor\n\
            subject.value > subject_sink.text\n\
            fallback.value > fallback_sink.text\n\
            supervise subject with handler\n";
        let panel = parse(source).unwrap();
        let mut topology_panel = panel.clone();
        topology_panel.supervisions.clear();
        let mut input = compile_input(source, &topology_panel);
        for candidate in &mut input.candidates {
            candidate.implementation.minimum_plan_version = EXECUTION_PLAN_SCHEMA_VERSION;
            candidate.implementation.maximum_plan_version = EXECUTION_PLAN_SCHEMA_VERSION;
            candidate.host_report.minimum_plan_version = EXECUTION_PLAN_SCHEMA_VERSION;
            candidate.host_report.maximum_plan_version = EXECUTION_PLAN_SCHEMA_VERSION;
        }
        input.seal().unwrap();
        let graph = resolve_source_graph(&input).unwrap();
        let lowered = lower_compile_source(&graph, &input.catalog).unwrap();
        assert_eq!(lowered.supervisions.len(), 1);
        input.source_semantic_hash = lowered.semantic_hash.to_string();
        input.supervision_bindings = vec![supervision_binding(&lowered, 768)];
        input.seal().unwrap();

        let plan = compile_panel(&panel, &input).unwrap();
        assert_eq!(plan.schema, PLAN_DOCUMENT_SCHEMA);
        assert_eq!(plan.schema_version, EXECUTION_PLAN_SCHEMA_VERSION);
        assert_eq!(plan.supervisions.len(), 1);
        assert_eq!(plan.supervisions[0].subject, "root/subject");
        assert_eq!(plan.supervisions[0].handler, "root/handler");
        plan.validate().unwrap();

        let bytes = serde_json::to_vec(&plan).unwrap();
        let decoded: ExactPlanDocument = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, plan);
        decoded.validate().unwrap();

        let mut changed = input.clone();
        changed.supervision_bindings[0].actions[1].maximum_uses = 3;
        changed.seal().unwrap();
        let changed_plan = compile_panel(&panel, &changed).unwrap();
        assert_ne!(changed_plan.identity, plan.identity);

        let mut underallocated = input;
        underallocated.supervision_bindings[0]
            .allocation
            .memory_bytes = 767;
        underallocated.seal().unwrap();
        assert_eq!(
            compile_panel(&panel, &underallocated).unwrap_err().code(),
            "CND-CMP-008"
        );
    }

    #[test]
    fn reference_distribution_is_identity_bound_and_absent_provider_is_exact() {
        let source = include_str!("../../../examples/hello.panel");
        let panel = parse(source).unwrap();
        let mut input = compile_input(source, &panel);
        input.distribution = Some(reference_distribution_document());
        input.seal().unwrap();
        input.validate().unwrap();

        let distribution = input.distribution.as_mut().unwrap();
        distribution.requirements = vec![ProviderRequirementDocument {
            provider: pin_doc("provider.firmware", 186),
            traits: ProviderRiskTraitsDocument {
                firmware_mutation: true,
                ..ProviderRiskTraitsDocument::default()
            },
        }];
        input.identity = input.computed_identity().unwrap();
        let error = input.validate().unwrap_err();
        assert_eq!(error.code(), "CND-GEN-010");
        assert_eq!(
            error.to_string(),
            "required provider is intentionally absent, disabled, or unsupported; provider provider.firmware; availability disabled"
        );
        assert_eq!(
            error.provider_denial(),
            Some(&CompileProviderDenial {
                provider: "provider.firmware".to_owned(),
                availability: "disabled",
            })
        );

        let distribution = input.distribution.as_mut().unwrap();
        distribution.requirements.clear();
        distribution
            .providers
            .iter_mut()
            .find(|provider| provider.provider.id == "provider.firmware")
            .unwrap()
            .availability = "enabled".to_owned();
        let error = input.seal().unwrap_err();
        assert_eq!(error.code(), "CND-GEN-011");
    }

    #[test]
    fn stale_reports_unresolved_contracts_and_budget_overruns_fail_closed() {
        let source = include_str!("../../../examples/hello.panel");
        let panel = parse(source).unwrap();

        let mut stale = compile_input(source, &panel);
        for candidate in &mut stale.candidates {
            candidate.host_report.valid_until_tick = 11;
        }
        stale.seal().unwrap();
        assert_eq!(
            compile_panel(&panel, &stale).unwrap_err().code(),
            "CND-CMP-006"
        );

        let mut unresolved = compile_input(source, &panel);
        unresolved.candidates.pop();
        unresolved.seal().unwrap();
        assert_eq!(
            compile_panel(&panel, &unresolved).unwrap_err().code(),
            "CND-CMP-005"
        );

        let mut over_budget = compile_input(source, &panel);
        over_budget.plan_budget.memory_bytes = 1;
        over_budget.seal().unwrap();
        assert_eq!(
            compile_panel(&panel, &over_budget).unwrap_err().code(),
            "CND-CMP-007"
        );
    }

    #[test]
    fn composite_expansion_retains_logical_membership_and_exports() {
        let source = "panel 0\n\
             fixture/uppercase{\n\
               upper: text/uppercase\n\
               export > text = upper.text\n\
               export uppercased > = upper.text\n\
             }\n\
             source: std/literal { value = \"hello\" }\n\
             transform: fixture/uppercase\n\
             sink: display/text\n\
             source.value > transform.text\n\
             transform.uppercased > sink.text\n";
        let panel = parse(source).unwrap();
        let input = compile_input(source, &panel);
        let plan = compile_panel(&panel, &input).unwrap();
        assert_eq!(plan.composites.len(), 1);
        assert_eq!(plan.composites[0].instance, "root/transform");
        assert_eq!(plan.composites[0].members, ["root/transform.upper"]);
        assert_eq!(plan.composites[0].exports.len(), 2);
        plan.validate().unwrap();
    }

    #[test]
    fn explicit_module_closure_and_selected_root_compile_without_io() {
        let child = "panel 0\n\
                     fixture/pipeline{\n\
                       source: std/literal { value = \"module\" }\n\
                       upper: text/uppercase using ready\n\
                       sink: display/text\n\
                       source.value > upper.text\n\
                       upper.text > sink.text\n\
                     }\n";
        let entry = "panel 0\n\
                     import \"./child.panel\" as child\n\
                     fixture/app{\n\
                       pipeline: child.fixture/pipeline\n\
                     }\n\
                     root fixture/app\n";
        let mut input = CompileInput {
            schema: COMPILE_INPUT_SCHEMA.to_owned(),
            schema_version: COMPILE_INPUT_SCHEMA_VERSION,
            identity: String::new(),
            entry_uri: "mem://compile/root.panel".to_owned(),
            selected_root: Some("fixture/app".to_owned()),
            source_limits: CompileSourceLimits::default(),
            modules: vec![
                CompileModuleDocument {
                    canonical_uri: "mem://compile/root.panel".to_owned(),
                    content_hash: String::new(),
                    source: entry.to_owned(),
                },
                CompileModuleDocument {
                    canonical_uri: "mem://compile/child.panel".to_owned(),
                    content_hash: String::new(),
                    source: child.to_owned(),
                },
            ],
            catalog: builtin_catalog_document().unwrap(),
            pool_bindings: Vec::new(),
            supervision_bindings: Vec::new(),
            hazard_closure: None,
            distribution: None,
            evidence_provider: None,
            watch_admissions: Vec::new(),
            source_semantic_hash: hash(1),
            resolver: pin_doc("conduit/exact-compiler-resolver", 70),
            resolver_policy_hash: String::new(),
            time_basis: "clock/compile".to_owned(),
            current_tick: 12,
            plan_budget: BudgetDocument {
                memory_bytes: 2 * 1024 * 1024,
                storage_bytes: 16 * 1024,
                cpu_units: 64,
                timers: 16,
                transports: 16,
                checkpoints: 16,
                evidence_bytes: 16 * 1024,
            },
            execution_arrangement: fixed_hosted_execution_arrangement_policy(),
            maximum_authority_bindings: 64,
            maximum_transition_memory_bytes: 1024 * 1024,
            maximum_search_states: 128,
            tie_policy: "lowest-canonical-identity".to_owned(),
            required_realm: None,
            trusted_entities: Vec::new(),
            trusted_status_reporters: Vec::new(),
            require_active_passport: false,
            implementation_preference: Vec::new(),
            candidates: Vec::new(),
        };
        for module in &mut input.modules {
            module.content_hash = content_hash(&module.source);
        }
        let loader = ExplicitModuleLoader {
            modules: &input.modules,
        };
        let graph = conduit_panel::resolve_modules(
            &input.entry_uri,
            input.selected_root.as_deref(),
            &loader,
        )
        .unwrap();
        let executable = executable_panel(&graph, &[]).unwrap();
        let topology = Registry::compatibility_demo()
            .resolve(&executable)
            .unwrap()
            .exact_topology()
            .unwrap();
        let mut contracts = BTreeMap::new();
        for node in &topology.nodes {
            contracts
                .entry(node.contract_id.clone())
                .or_insert(node.contract_hash);
        }
        input.candidates = contracts
            .into_iter()
            .enumerate()
            .map(|(index, (id, hash))| candidate(index as u8 + 1, &id, hash))
            .collect();
        input.seal().unwrap();

        let plan = compile_source(entry, &input).unwrap();
        assert_eq!(plan.nodes.len(), 3);
        assert_eq!(plan.composites.len(), 2);
        assert!(plan.unresolved_selectors.is_empty());
        plan.validate().unwrap();

        let mut incomplete = input.clone();
        incomplete
            .modules
            .retain(|module| module.canonical_uri.ends_with("root.panel"));
        incomplete.identity = incomplete.computed_identity().unwrap();
        assert_eq!(incomplete.validate().unwrap_err().code(), "CND-CMP-003");
    }

    #[test]
    fn exact_capability_resource_and_topology_predicates_are_bound() {
        let source = include_str!("../../../examples/hello.panel");
        let panel = parse(source).unwrap();
        let mut input = compile_input(source, &panel);
        let candidate = &mut input.candidates[0];
        let capability = ReportCapabilityDocument {
            interface: pin_doc("fixture/interface", 81),
            mode: "fixture/mode".to_owned(),
            subject: "fixture/subject".to_owned(),
            details: hash(82),
            capacity: BudgetDocument {
                transports: 2,
                ..BudgetDocument::default()
            },
        };
        candidate.host_report.capabilities.push(capability.clone());
        candidate.capabilities.push(CapabilityRequirementDocument {
            interface: capability.interface,
            mode: capability.mode,
            subject: Some(capability.subject),
            details: Some(capability.details),
            minimum_capacity: BudgetDocument {
                transports: 1,
                ..BudgetDocument::default()
            },
        });
        let resource = ReportResourceDocument {
            kind: "fixture/device".to_owned(),
            id: "fixture/device-a".to_owned(),
            descriptor: pin_doc("fixture/device-descriptor", 83),
            capacity: BudgetDocument {
                memory_bytes: 64,
                ..BudgetDocument::default()
            },
            exclusive: true,
        };
        candidate.host_report.resources.push(resource.clone());
        candidate.resources.push(ResourceRequirementDocument {
            kind: resource.kind,
            id: Some(resource.id),
            descriptor: Some(resource.descriptor),
            minimum_capacity: BudgetDocument {
                memory_bytes: 32,
                ..BudgetDocument::default()
            },
            require_exclusive: true,
        });
        let edge = ReportTopologyDocument {
            id: "fixture/edge".to_owned(),
            contract: pin_doc("fixture/topology", 84),
            from: "fixture/a".to_owned(),
            to: "fixture/b".to_owned(),
            maximum_transfer_unit: 1500,
            maximum_sessions: 4,
            reachable: true,
            details: hash(85),
        };
        candidate.host_report.topology.push(edge.clone());
        candidate.topology.push(TopologyRequirementDocument {
            contract: edge.contract,
            from: edge.from,
            to: edge.to,
            minimum_transfer_unit: 1280,
            minimum_sessions: 1,
            details: Some(edge.details),
        });
        input.seal().unwrap();

        let plan = compile_panel(&panel, &input).unwrap();
        assert_eq!(plan.resources.len(), 1);
        assert!(
            plan.nodes
                .iter()
                .any(|node| { node.required_resources == vec![plan.resources[0].id.clone()] })
        );
        plan.validate().unwrap();

        let mut incompatible = input;
        incompatible.candidates[0].topology[0].minimum_sessions = 5;
        incompatible.seal().unwrap();
        assert_eq!(
            compile_panel(&panel, &incompatible).unwrap_err().code(),
            "CND-CMP-006"
        );
    }

    #[test]
    fn realm_passport_policy_is_carried_into_host_resolution() {
        let source = include_str!("../../../examples/hello.panel");
        let panel = parse(source).unwrap();
        let mut input = compile_input(source, &panel);
        let reporter = pin_doc("fixture/status-reporter", 91);
        input.required_realm = Some("fixture/realm".to_owned());
        input.trusted_entities = vec!["fixture/entity".to_owned()];
        input.trusted_status_reporters = vec![reporter.semantic_hash.clone()];
        input.require_active_passport = true;
        for candidate in &mut input.candidates {
            candidate.host_report.membership = Some(ReportMembershipDocument {
                realm: "fixture/realm".to_owned(),
                entity: "fixture/entity".to_owned(),
                passport: hash(92),
                status_reporter: reporter.clone(),
                status_time_basis: "clock/compile".to_owned(),
                status_observed_at_tick: 10,
                status_valid_until_tick: 20,
                status: "active".to_owned(),
            });
        }
        input.seal().unwrap();
        compile_panel(&panel, &input).unwrap().validate().unwrap();

        let mut suspended = input;
        suspended.candidates[0]
            .host_report
            .membership
            .as_mut()
            .unwrap()
            .status = "suspended".to_owned();
        suspended.seal().unwrap();
        assert_eq!(
            compile_panel(&panel, &suspended).unwrap_err().code(),
            "CND-CMP-006"
        );
    }

    #[test]
    fn authority_is_resolved_and_round_trips_as_an_exact_plan_binding() {
        let source = include_str!("../../../examples/hello.panel");
        let panel = parse(source).unwrap();
        let mut input = compile_input(source, &panel);
        let candidate = input
            .candidates
            .iter_mut()
            .find(|candidate| {
                let id = &candidate.implementation.semantic_contract.id;
                id == "std/literal"
            })
            .unwrap();
        let host = candidate.host_report.host.clone();
        let (resource_lease, commit_profile) = current_effect_contracts(
            "root/greeting",
            "fixture/read",
            "fixture/device-a",
            "fixture/run",
            101,
        );
        candidate.authorities.push(AuthorityDecisionDocument {
            requirement: hash(101),
            effect_hash: String::new(),
            grant_hash: String::new(),
            effect: EffectRequirementDocument {
                id: "fixture/read".to_owned(),
                administrative_class: None,
                policy_budget_class: None,
                action: "fixture/read".to_owned(),
                resource_kind: "fixture/device".to_owned(),
                resource_id: Some("fixture/device-a".to_owned()),
                requester: "root/greeting".to_owned(),
                audience: "fixture/run".to_owned(),
                constraints: Vec::new(),
                check_at_use: true,
            },
            capability: HostCapabilityDocument {
                id: "fixture/read-capability".to_owned(),
                action: "fixture/read".to_owned(),
                resource_kind: "fixture/device".to_owned(),
                resource_id: "fixture/device-a".to_owned(),
                host: host.clone(),
                time_basis: "clock/compile".to_owned(),
                observed_at_tick: 10,
                valid_until_tick: 20,
            },
            grant: AuthorityGrantDocument {
                id: "fixture/read-grant".to_owned(),
                action: "fixture/read".to_owned(),
                resource_kind: "fixture/device".to_owned(),
                resource_id: "fixture/device-a".to_owned(),
                scope_root: "root/greeting".to_owned(),
                scope_descendants: false,
                audience: "fixture/run".to_owned(),
                constraints: Vec::new(),
                time_basis: "clock/compile".to_owned(),
                not_before_tick: 10,
                expires_at_tick: 20,
                issued_for_host: host,
                delegation: "none".to_owned(),
                audit_id: "fixture/read-audit".to_owned(),
                terminal_policy: "abort".to_owned(),
            },
            status: "active".to_owned(),
            administrative_subject: None,
            containment: None,
            policy_budgets: Vec::new(),
            resource_lease,
            commit_profile,
        });
        input.seal().unwrap();

        let plan = compile_panel(&panel, &input).unwrap();
        assert_eq!(plan.authorities.len(), 1);
        assert_eq!(plan.authorities[0].node, "root/greeting");
        assert!(plan.nodes.iter().any(|node| {
            node.instance == "root/greeting"
                && node.required_effects == vec![plan.authorities[0].effect_hash.clone()]
                && node
                    .required_resources
                    .contains(&"fixture/device-a".to_owned())
        }));
        plan.validate().unwrap();

        let mut effect_plan = plan.clone();
        effect_plan.schema = PLAN_DOCUMENT_SCHEMA.to_owned();
        effect_plan.schema_version = EXECUTION_PLAN_SCHEMA_VERSION;
        let authority_node = effect_plan.authorities[0].node.clone();
        let resource_id = effect_plan.authorities[0].binding.resource_id.clone();
        let resource_index = effect_plan
            .resources
            .iter()
            .position(|resource| {
                resource.node == authority_node && resource.resource == resource_id
            })
            .unwrap();
        let resource_binding = effect_plan.resources[resource_index].id.clone();
        let lease_id = "lease/fixture-read".to_owned();
        effect_plan.resources[resource_index].lease = Some(ResourceLeaseDocument {
            schema_version: RESOURCE_LEASE_SCHEMA_VERSION,
            id: lease_id.clone(),
            resource_binding,
            holder: authority_node,
            run: "fixture/run".to_owned(),
            epoch: 1,
            scope: "fixture/read-scope".to_owned(),
            sharing: "exclusive".to_owned(),
            maximum_holders: 1,
            reservation: BudgetDocument {
                memory_bytes: 1,
                ..BudgetDocument::default()
            },
            time_basis: effect_plan.time_basis.clone(),
            issued_at_tick: effect_plan.created_at_tick,
            expires_at_tick: effect_plan.authorities[0].grant.expires_at_tick,
            revocation_grace_ticks: 1,
            cleanup_ticks: 2,
            maximum_operations: 1,
            maximum_evidence_events: 4,
            cleanup_escalation: PinDocument {
                id: "fixture/force-close".to_owned(),
                schema_version: 0,
                semantic_hash: hash(102),
            },
            foreign_retention: "unsupported".to_owned(),
            foreign_maximum_bytes: 0,
            foreign_release_ticks: 0,
        });
        effect_plan.authorities[0].commit_profile = Some(EffectCommitProfileDocument {
            schema_version: EFFECT_COMMIT_PROFILE_SCHEMA_VERSION,
            id: "effect/fixture-read".to_owned(),
            operation: effect_plan.authorities[0].effect.action.clone(),
            resource_lease: lease_id,
            commit_boundary: PinDocument {
                id: "fixture/read-commit".to_owned(),
                schema_version: 0,
                semantic_hash: hash(103),
            },
            idempotency: "reconcile-before-retry".to_owned(),
            unknown_commit: "reconcile".to_owned(),
            discontinuity: "reconcile-required".to_owned(),
            cleanup: PinDocument {
                id: "fixture/read-cleanup".to_owned(),
                schema_version: 0,
                semantic_hash: hash(104),
            },
            maximum_attempts: 2,
            evidence_events_per_attempt: 2,
        });
        let computed_identity = |document: &ExactPlanDocument| {
            let arena = Bump::new();
            let exact = document.as_plan(&arena).unwrap();
            let mut scratch =
                vec![SemanticHash::from_bytes([0; 32]); exact.validation_scratch_count().unwrap()];
            exact.semantic_hash(&mut scratch).unwrap().to_string()
        };
        effect_plan.identity = computed_identity(&effect_plan);
        reseal_test_execution_arrangement(&mut effect_plan);
        effect_plan.validate().unwrap();
        let encoded = serde_json::to_vec(&effect_plan).unwrap();
        let decoded: ExactPlanDocument = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, effect_plan);
        assert_eq!(decoded.schema, PLAN_DOCUMENT_SCHEMA);

        let finite_work = WorkloadBudgetDocument {
            work_units: Some(100),
            tasks: Some(1),
            processes: None,
            descriptors: Some(1),
            connections: None,
            storage_bytes: None,
            device_operations: Some(1),
            network_bytes: None,
            callbacks: Some(1),
            foreign_queue_items: Some(1),
            transition_overlap_work_units: Some(20),
        };
        let mut workload_plan = effect_plan.clone();
        workload_plan.schema = PLAN_DOCUMENT_SCHEMA.to_owned();
        workload_plan.schema_version = EXECUTION_PLAN_SCHEMA_VERSION;
        workload_plan.workloads = vec![PlanWorkloadDocument {
            contract: WorkloadContractDocument {
                schema_version: conduit_core::WORKLOAD_CONTRACT_SCHEMA_VERSION,
                id: "workload/fixture-read".to_owned(),
                service: "service/fixture-read".to_owned(),
                node: workload_plan.nodes[0].instance.clone(),
                guarantee: "hard".to_owned(),
                budget: finite_work.clone(),
                deadline: Some(DeadlineContractDocument {
                    time_basis: workload_plan.time_basis.clone(),
                    relative_deadline_ticks: 5,
                    maximum_jitter_ticks: 1,
                }),
                maximum_evidence_events: 4,
            },
            capability: WorkloadCapabilityDocument {
                id: "capability/fixture-deadline".to_owned(),
                identity: hash(105),
                host_observation: workload_plan.host_observations[0].id.clone(),
                evidence_kind: "exact-enforcement".to_owned(),
                time_basis: workload_plan.time_basis.clone(),
                observed_at_tick: workload_plan.host_observations[0].observed_at_tick,
                valid_until_tick: workload_plan.host_observations[0].valid_until_tick,
                capacity: WorkloadBudgetDocument {
                    work_units: Some(200),
                    ..finite_work
                },
                maximum_deadline_ticks: 10,
                maximum_jitter_ticks: 1,
            },
        }];
        workload_plan.identity = computed_identity(&workload_plan);
        reseal_test_execution_arrangement(&mut workload_plan);
        workload_plan.validate().unwrap();
        let encoded = serde_json::to_vec(&workload_plan).unwrap();
        let decoded: ExactPlanDocument = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, workload_plan);
        assert_eq!(decoded.schema, PLAN_DOCUMENT_SCHEMA);

        let mut missing_commit = effect_plan.clone();
        missing_commit.authorities[0].commit_profile = None;
        missing_commit.identity = computed_identity(&missing_commit);
        reseal_test_execution_arrangement(&mut missing_commit);
        assert_eq!(missing_commit.validate().unwrap_err().code(), "CND-CMP-008");

        let mut identity_mutation = effect_plan;
        identity_mutation.authorities[0]
            .commit_profile
            .as_mut()
            .unwrap()
            .maximum_attempts = 1;
        assert_eq!(
            identity_mutation.validate().unwrap_err().code(),
            "CND-CMP-008"
        );

        let mut authority_over_budget = input.clone();
        authority_over_budget.maximum_authority_bindings = 0;
        authority_over_budget.seal().unwrap();
        assert_eq!(
            compile_panel(&panel, &authority_over_budget)
                .unwrap_err()
                .code(),
            "CND-CMP-007"
        );

        let mut transition_over_budget = input.clone();
        transition_over_budget.maximum_transition_memory_bytes = 4;
        transition_over_budget
            .candidates
            .iter_mut()
            .find(|candidate| !candidate.authorities.is_empty())
            .unwrap()
            .implementation
            .coexistence_memory_bytes = 8;
        transition_over_budget.seal().unwrap();
        assert_eq!(
            compile_panel(&panel, &transition_over_budget)
                .unwrap_err()
                .code(),
            "CND-CMP-007"
        );

        let mut revoked = input;
        revoked
            .candidates
            .iter_mut()
            .find(|candidate| !candidate.authorities.is_empty())
            .unwrap()
            .authorities[0]
            .status = "revoked".to_owned();
        revoked.seal().unwrap();
        assert_eq!(
            compile_panel(&panel, &revoked).unwrap_err().code(),
            "CND-CMP-006"
        );
    }

    #[test]
    fn administrative_effect_requires_and_emits_exact_containment_proof() {
        let source = include_str!("../../../examples/hello.panel");
        let panel = parse(source).unwrap();
        let mut input = compile_input(source, &panel);
        let candidate = input
            .candidates
            .iter_mut()
            .find(|candidate| {
                let id = &candidate.implementation.semantic_contract.id;
                id == "std/literal"
            })
            .unwrap();
        candidate.implementation.maximum_plan_version = EXECUTION_PLAN_SCHEMA_VERSION;
        candidate.host_report.maximum_plan_version = EXECUTION_PLAN_SCHEMA_VERSION;
        let host = candidate.host_report.host.clone();
        let effect_class = pin_doc("effect.admin", 101);
        let (subject, proof) = administrative_proof_doc(effect_class.clone());
        let (resource_lease, commit_profile) = current_effect_contracts(
            "root/greeting",
            "fixture/read",
            "fixture/device-a",
            "fixture/run",
            111,
        );
        candidate.authorities.push(AuthorityDecisionDocument {
            requirement: hash(111),
            effect_hash: String::new(),
            grant_hash: String::new(),
            effect: EffectRequirementDocument {
                id: "fixture/admin".to_owned(),
                administrative_class: Some(effect_class),
                policy_budget_class: None,
                action: "fixture/read".to_owned(),
                resource_kind: "fixture/device".to_owned(),
                resource_id: Some("fixture/device-a".to_owned()),
                requester: "root/greeting".to_owned(),
                audience: "fixture/run".to_owned(),
                constraints: Vec::new(),
                check_at_use: true,
            },
            capability: HostCapabilityDocument {
                id: "fixture/admin-capability".to_owned(),
                action: "fixture/read".to_owned(),
                resource_kind: "fixture/device".to_owned(),
                resource_id: "fixture/device-a".to_owned(),
                host: host.clone(),
                time_basis: "clock/compile".to_owned(),
                observed_at_tick: 10,
                valid_until_tick: 20,
            },
            grant: AuthorityGrantDocument {
                id: "fixture/admin-grant".to_owned(),
                action: "fixture/read".to_owned(),
                resource_kind: "fixture/device".to_owned(),
                resource_id: "fixture/device-a".to_owned(),
                scope_root: "root/greeting".to_owned(),
                scope_descendants: false,
                audience: "fixture/run".to_owned(),
                constraints: Vec::new(),
                time_basis: "clock/compile".to_owned(),
                not_before_tick: 10,
                expires_at_tick: 20,
                issued_for_host: host,
                delegation: "none".to_owned(),
                audit_id: "fixture/admin-audit".to_owned(),
                terminal_policy: "abort".to_owned(),
            },
            status: "active".to_owned(),
            administrative_subject: Some(subject),
            containment: Some(proof),
            policy_budgets: Vec::new(),
            resource_lease,
            commit_profile,
        });
        input.seal().unwrap();
        let plan = compile_panel(&panel, &input).unwrap();
        assert_eq!(plan.schema, PLAN_DOCUMENT_SCHEMA);
        assert_eq!(plan.schema_version, EXECUTION_PLAN_SCHEMA_VERSION);
        assert!(plan.authorities[0].containment.is_some());
        plan.validate().unwrap();

        let mut missing = input;
        let authority = missing
            .candidates
            .iter_mut()
            .find(|candidate| !candidate.authorities.is_empty())
            .unwrap()
            .authorities
            .first_mut()
            .unwrap();
        authority.containment = None;
        let error = missing.seal().unwrap_err();
        assert_eq!(error.code(), "CND-CTN-007");
        assert_eq!(
            error.to_string(),
            "administrative effect is missing its exact independent approval proof"
        );
    }

    #[test]
    fn persistent_budget_status_is_pinned_and_denial_is_distinct_from_plan_resources() {
        let source = include_str!("../../../examples/hello.panel");
        let panel = parse(source).unwrap();
        let mut input = compile_input(source, &panel);
        let candidate = input
            .candidates
            .iter_mut()
            .find(|candidate| {
                let id = &candidate.implementation.semantic_contract.id;
                id == "std/literal"
            })
            .unwrap();
        candidate.implementation.maximum_plan_version = EXECUTION_PLAN_SCHEMA_VERSION;
        candidate.host_report.maximum_plan_version = EXECUTION_PLAN_SCHEMA_VERSION;
        let host = candidate.host_report.host.clone();
        let budget_class = pin_doc("class.executable-installation", 119);
        let (resource_lease, commit_profile) = current_effect_contracts(
            "root/greeting",
            "fixture/read",
            "fixture/device-a",
            "fixture/run",
            126,
        );
        candidate.authorities.push(AuthorityDecisionDocument {
            requirement: hash(126),
            effect_hash: String::new(),
            grant_hash: String::new(),
            effect: EffectRequirementDocument {
                id: "fixture/governed".to_owned(),
                administrative_class: None,
                policy_budget_class: Some(budget_class.clone()),
                action: "fixture/read".to_owned(),
                resource_kind: "fixture/device".to_owned(),
                resource_id: Some("fixture/device-a".to_owned()),
                requester: "root/greeting".to_owned(),
                audience: "fixture/run".to_owned(),
                constraints: Vec::new(),
                check_at_use: true,
            },
            capability: HostCapabilityDocument {
                id: "fixture/governed-capability".to_owned(),
                action: "fixture/read".to_owned(),
                resource_kind: "fixture/device".to_owned(),
                resource_id: "fixture/device-a".to_owned(),
                host: host.clone(),
                time_basis: "clock/compile".to_owned(),
                observed_at_tick: 10,
                valid_until_tick: 20,
            },
            grant: AuthorityGrantDocument {
                id: "fixture/governed-grant".to_owned(),
                action: "fixture/read".to_owned(),
                resource_kind: "fixture/device".to_owned(),
                resource_id: "fixture/device-a".to_owned(),
                scope_root: "root/greeting".to_owned(),
                scope_descendants: false,
                audience: "fixture/run".to_owned(),
                constraints: Vec::new(),
                time_basis: "clock/compile".to_owned(),
                not_before_tick: 10,
                expires_at_tick: 20,
                issued_for_host: host,
                delegation: "none".to_owned(),
                audit_id: "fixture/governed-audit".to_owned(),
                terminal_policy: "abort".to_owned(),
            },
            status: "active".to_owned(),
            administrative_subject: None,
            containment: None,
            policy_budgets: vec![policy_budget_binding_doc(budget_class)],
            resource_lease,
            commit_profile,
        });
        input.seal().unwrap();
        let plan = compile_panel(&panel, &input).unwrap();
        assert_eq!(plan.schema, PLAN_DOCUMENT_SCHEMA);
        assert_eq!(plan.schema_version, EXECUTION_PLAN_SCHEMA_VERSION);
        assert_eq!(plan.authorities[0].policy_budgets.len(), 1);
        plan.validate().unwrap();

        let mut exhausted = input;
        let binding = &mut exhausted
            .candidates
            .iter_mut()
            .find(|candidate| !candidate.authorities.is_empty())
            .unwrap()
            .authorities[0]
            .policy_budgets[0];
        binding.status.lifetime_committed = 1;
        exhausted.seal().unwrap();
        let error = compile_panel(&panel, &exhausted).unwrap_err();
        assert_eq!(error.code(), "CND-PBG-008");
        assert_eq!(
            error.to_string(),
            "persistent policy budget denied the protected effect"
        );
    }

    #[test]
    fn whole_plan_hazard_closure_is_sealed_and_toxic_combinations_fail_before_start() {
        let source = include_str!("../../../examples/hello.panel");
        let panel = parse(source).unwrap();
        let mut input = compile_input(source, &panel);
        let candidate = input
            .candidates
            .iter_mut()
            .find(|candidate| {
                let id = &candidate.implementation.semantic_contract.id;
                id == "std/literal"
            })
            .unwrap();
        candidate.implementation.maximum_plan_version = EXECUTION_PLAN_SCHEMA_VERSION;
        candidate.host_report.maximum_plan_version = EXECUTION_PLAN_SCHEMA_VERSION;
        let host = candidate.host_report.host.clone();
        let present_class = pin_doc("class.present", 140);
        let absent_class = pin_doc("class.absent", 141);
        let present_constraint = AuthorityConstraintDocument {
            id: present_class.id.clone(),
            semantic_hash: present_class.semantic_hash.clone(),
        };
        let (resource_lease, commit_profile) = current_effect_contracts(
            "root/greeting",
            "fixture/read",
            "fixture/device-a",
            "fixture/run",
            142,
        );
        candidate.authorities.push(AuthorityDecisionDocument {
            requirement: hash(142),
            effect_hash: String::new(),
            grant_hash: String::new(),
            effect: EffectRequirementDocument {
                id: "fixture/classified".to_owned(),
                administrative_class: None,
                policy_budget_class: None,
                action: "fixture/read".to_owned(),
                resource_kind: "fixture/device".to_owned(),
                resource_id: Some("fixture/device-a".to_owned()),
                requester: "root/greeting".to_owned(),
                audience: "fixture/run".to_owned(),
                constraints: vec![present_constraint.clone()],
                check_at_use: true,
            },
            capability: HostCapabilityDocument {
                id: "fixture/classified-capability".to_owned(),
                action: "fixture/read".to_owned(),
                resource_kind: "fixture/device".to_owned(),
                resource_id: "fixture/device-a".to_owned(),
                host: host.clone(),
                time_basis: "clock/compile".to_owned(),
                observed_at_tick: 10,
                valid_until_tick: 20,
            },
            grant: AuthorityGrantDocument {
                id: "fixture/classified-grant".to_owned(),
                action: "fixture/read".to_owned(),
                resource_kind: "fixture/device".to_owned(),
                resource_id: "fixture/device-a".to_owned(),
                scope_root: "root/greeting".to_owned(),
                scope_descendants: false,
                audience: "fixture/run".to_owned(),
                constraints: vec![present_constraint],
                time_basis: "clock/compile".to_owned(),
                not_before_tick: 10,
                expires_at_tick: 20,
                issued_for_host: host,
                delegation: "none".to_owned(),
                audit_id: "fixture/classified-audit".to_owned(),
                terminal_policy: "abort".to_owned(),
            },
            status: "active".to_owned(),
            administrative_subject: None,
            containment: None,
            policy_budgets: Vec::new(),
            resource_lease,
            commit_profile,
        });
        input.seal().unwrap();
        let baseline = compile_panel(&panel, &input).unwrap();
        let baseline_arena = Bump::new();
        let baseline_plan = baseline.as_plan(&baseline_arena).unwrap();
        let plan_subject = conduit_core::effect_closure_subject(
            baseline_plan.authorities,
            &[],
            1,
            baseline_plan.created_at.basis,
        )
        .unwrap();

        let mut closure = HazardClosureDocument {
            epoch: 1,
            plan_subject: plan_subject.to_string(),
            policy: HazardClosurePolicyDocument {
                schema_version: conduit_core::HAZARD_CLOSURE_POLICY_SCHEMA_VERSION,
                identity: hash(0),
                descriptor: pin_doc("policy.fixture-hazard", 143),
                permit_class: pin_doc("effect.fixture-permit", 144),
                classes: vec![
                    EffectClassBindingDocument {
                        identity: hash(0),
                        descriptor: present_class.clone(),
                        persistence: false,
                        delegation: false,
                        distributed: false,
                        administrative: false,
                    },
                    EffectClassBindingDocument {
                        identity: hash(0),
                        descriptor: absent_class.clone(),
                        persistence: false,
                        delegation: false,
                        distributed: false,
                        administrative: false,
                    },
                ],
                rules: vec![ToxicCombinationRuleDocument {
                    identity: hash(0),
                    descriptor: pin_doc("rule.fixture-toxic", 145),
                    patterns: vec![ToxicEffectPatternDocument {
                        id: "stage.absent".to_owned(),
                        class: absent_class,
                        resource_kind: None,
                        resource_id: None,
                        audience: None,
                        host: None,
                        realm: None,
                        budget: None,
                        persistence: "any".to_owned(),
                        delegation: "any".to_owned(),
                        distributed: "any".to_owned(),
                        administrative: "any".to_owned(),
                    }],
                    flows: Vec::new(),
                }],
                limits: HazardClosureLimitsDocument {
                    maximum_effects: 8,
                    maximum_classes: 4,
                    maximum_rules: 4,
                    maximum_patterns_per_rule: 4,
                    maximum_flows: 4,
                    maximum_permits: 4,
                    maximum_proof_nodes: 8,
                    maximum_search_steps: 64,
                },
            },
            flows: Vec::new(),
            permits: Vec::new(),
            decision_identity: hash(146),
            hazardous_hosts: vec![hazardous_host_doc()],
        };
        seal_hazard_closure(&mut closure).unwrap();
        {
            let arena = Bump::new();
            let policy = hazard_closure_policy(&closure.policy, &arena).unwrap();
            let mut proof = [None; MAX_HAZARD_PROOF_NODES];
            let report = analyze_effect_closure(
                policy,
                baseline_plan.authorities,
                &[],
                &[],
                HazardClosureContext {
                    plan_subject,
                    epoch: 1,
                    time: baseline_plan.created_at,
                },
                &mut proof,
            )
            .unwrap();
            closure.decision_identity = report.decision_identity.to_string();
        }
        input.hazard_closure = Some(closure);
        input.seal().unwrap();
        let plan = compile_panel(&panel, &input).unwrap();
        assert_eq!(plan.schema, PLAN_DOCUMENT_SCHEMA);
        assert_eq!(plan.schema_version, EXECUTION_PLAN_SCHEMA_VERSION);
        assert!(plan.hazard_closure.is_some());
        plan.validate().unwrap();

        let mut hazardous = input.clone();
        hazardous.seal().unwrap();
        let hazardous_plan = compile_panel(&panel, &hazardous).unwrap();
        assert_eq!(hazardous_plan.schema_version, EXECUTION_PLAN_SCHEMA_VERSION);
        let hazardous_arena = Bump::new();
        let hazardous_core = hazardous_plan.as_plan(&hazardous_arena).unwrap();
        let stale = validate_hosted_execution_plan(
            &hazardous_core,
            PlanValidationContext {
                supported_schema_version: EXECUTION_PLAN_SCHEMA_VERSION,
                now: AuthorityTime {
                    basis: Id("clock/compile"),
                    tick: 15,
                },
            },
        )
        .unwrap_err();
        assert_eq!(stale.code.as_str(), "CND-INH-004");

        let mut toxic = input;
        toxic.hazard_closure.as_mut().unwrap().policy.rules[0].patterns[0].class = present_class;
        toxic.seal().unwrap();
        let error = compile_panel(&panel, &toxic).unwrap_err();
        assert_eq!(error.code(), "CND-HZD-010");
        assert_eq!(
            error.to_string(),
            "whole-plan effect closure contains a policy-forbidden combination; rule rule.fixture-toxic; effects fixture/classified"
        );
        assert_eq!(error.hazard_proof().len(), 2);
        assert_eq!(error.hazard_proof()[0].kind, "rule");
        assert_eq!(
            error.hazard_proof()[1].effect.as_deref(),
            Some("fixture/classified")
        );
    }

    #[test]
    fn finite_port_groups_are_retained_as_plan_visible_expansions() {
        let base = include_str!("../../../examples/hello.panel");
        let source = format!("{base}\nport-group lanes >: conduit/output-text indexed max 2\n");
        let panel = parse(&source).unwrap();
        let base_panel = parse(base).unwrap();
        let mut input = compile_input(base, &base_panel);
        input.modules[0].source = source.clone();
        input.seal().unwrap();

        let plan = compile_panel(&panel, &input).unwrap();
        assert_eq!(plan.port_groups.len(), 1);
        assert_eq!(plan.port_groups[0].maximum, 2);
        assert_eq!(plan.port_groups[0].direction, "output");
        assert_eq!(plan.port_groups[0].members.len(), 2);
        assert_eq!(plan.port_groups[0].members[0].ordinal, 0);
        assert_eq!(plan.port_groups[0].members[1].ordinal, 1);
        plan.validate().unwrap();
    }

    #[test]
    fn finite_pools_require_exact_budget_bindings_and_round_trip() {
        let base = include_str!("../../../examples/hello.panel");
        let source = format!(
            "{base}\n\
             fixture/worker{{\n\
               source: std/literal {{ value = \"pool\" }}\n\
               sink: display/text\n\
               source.value > sink.text\n\
             }}\n\
             pool workers: fixture/worker {{ maximum = 2 admission = queue_bounded admission_queue = 2 deadline_ms = 1000 idle_timeout_ms = 5000 supervision = isolate cleanup = abort }}\n"
        );
        let panel = parse(&source).unwrap();
        let base_panel = parse(base).unwrap();
        let mut input = compile_input(base, &base_panel);
        input.modules[0].source = source.clone();
        input.modules[0].content_hash = content_hash(&source);
        let graph = resolve_source_graph(&input).unwrap();
        let lowered = lower_compile_source(&graph, &input.catalog).unwrap();
        assert_eq!(lowered.topology.pools.len(), 1);
        input.pool_bindings.push(PoolBindingDocument {
            pool_semantic_hash: lowered.topology.pools[0].semantic_hash.to_string(),
            admission_policy: pin_doc("fixture/pool-admission", 111),
            supervision_policy: pin_doc("fixture/pool-supervision", 112),
            per_instance_budget: BudgetDocument {
                memory_bytes: 16,
                timers: 1,
                evidence_bytes: 16,
                ..BudgetDocument::default()
            },
            authority_grants: Vec::new(),
            maximum_instance_ticks: 1000,
            implementation_set_hash: hash(113),
            correlation_slots: 4,
            worst_case_budget: BudgetDocument {
                memory_bytes: 72,
                timers: 4,
                evidence_bytes: 72,
                ..BudgetDocument::default()
            },
            child_nodes: 2,
            child_cords: 1,
            runtime: Some(PoolRuntimeBindingDocument {
                ticks_per_millisecond: 1,
                cleanup_ticks: 10,
                maximum_evidence_events: 64,
                fallback_target: None,
                per_instance: PoolReservationDocument {
                    resources: BudgetDocument {
                        memory_bytes: 16,
                        timers: 1,
                        evidence_bytes: 16,
                        ..BudgetDocument::default()
                    },
                    child_nodes: 2,
                    child_cords: 1,
                    state_bytes: 8,
                    scheduler_slots: 4,
                    host_operations: 1,
                    cancellation_scopes: 3,
                },
                queued: PoolReservationDocument {
                    resources: BudgetDocument {
                        memory_bytes: 4,
                        evidence_bytes: 4,
                        ..BudgetDocument::default()
                    },
                    state_bytes: 2,
                    scheduler_slots: 1,
                    cancellation_scopes: 1,
                    ..PoolReservationDocument::default()
                },
                candidate_maximum_live: 1,
                rollback_maximum_live: 1,
                generation_reserved: PoolReservationDocument {
                    resources: BudgetDocument {
                        memory_bytes: 64,
                        timers: 4,
                        evidence_bytes: 64,
                        ..BudgetDocument::default()
                    },
                    child_nodes: 8,
                    child_cords: 4,
                    state_bytes: 32,
                    scheduler_slots: 16,
                    host_operations: 4,
                    cancellation_scopes: 12,
                },
                total_reserved: PoolReservationDocument {
                    resources: BudgetDocument {
                        memory_bytes: 72,
                        timers: 4,
                        evidence_bytes: 72,
                        ..BudgetDocument::default()
                    },
                    child_nodes: 8,
                    child_cords: 4,
                    state_bytes: 36,
                    scheduler_slots: 18,
                    host_operations: 4,
                    cancellation_scopes: 14,
                },
            }),
        });
        input.seal().unwrap();

        let plan = compile_panel(&panel, &input).unwrap();
        assert_eq!(plan.schema_version, EXECUTION_PLAN_SCHEMA_VERSION);
        assert_eq!(plan.instance_pools.len(), 1);
        assert_eq!(plan.instance_pools[0].maximum_live, 2);
        assert_eq!(plan.instance_pools[0].maximum_queued, 2);
        let runtime = plan.instance_pools[0].runtime.as_ref().unwrap();
        assert_eq!(runtime.deadline_ticks, 1000);
        assert_eq!(runtime.idle_timeout_ticks, 5000);
        assert_eq!(runtime.generation_reserved_slots, 4);
        plan.validate().unwrap();
        assert_eq!(plan.schema, PLAN_DOCUMENT_SCHEMA);
        let encoded = serde_json::to_vec(&plan).unwrap();
        let decoded: ExactPlanDocument = serde_json::from_slice(&encoded).unwrap();
        decoded.validate().unwrap();
        assert!(decoded.instance_pools[0].runtime.is_some());

        let mut missing = input;
        missing.pool_bindings.clear();
        missing.seal().unwrap();
        assert_eq!(
            compile_panel(&panel, &missing).unwrap_err().code(),
            "CND-CMP-007"
        );
    }
}
