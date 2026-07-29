//! Exact runnable-plan identity and allocator-free portable validation.

use core::convert::Infallible;
use core::fmt;

use crate::canonical::semantic_hash_with_hash_set;
use crate::{
    AuthorityGrant, AuthorityTime, CanonicalDescriptor, CanonicalError, CanonicalValue,
    CheckpointProviderCapabilities, DescriptorRef, Direction, DuplicationRule, EffectRequirement,
    EventProviderCapabilities, EventStreamContract, ExecutionProfile, FanOutMode, FieldDisposition,
    FlowPolicy, GrantStatus, HostCapability, Id, InstancePath, JobContract, MapField,
    MergeOrdering, MergeTerminalPolicy, ObservedGrant, OwnershipModel, Pressure,
    ResolvedAuthorityBinding, RetentionPolicy, RuntimeEvidencePolicy, SatisfactionProof,
    SatisfactionRole, SemanticHash, SubscriberCoupling, TypeContractRef, validate_authority_at_use,
    validate_job_contract, validate_plan_execution_profile, validate_runtime_evidence_policy,
    validate_satisfaction_proof, validate_stream_contract,
};

/// Latest exact schema supported by the portable validator.
///
/// Schema 2 adds explicit port-group maximum and direction. Schema 3 adds one
/// exact implementation execution profile per primitive node. Schema 4 adds
/// structural flow, schema 5 adds Resonance streams, and schema 6 adds durable
/// jobs, schema 7 adds implicit-satisfaction proof bindings, and schema 8 adds
/// one exact runtime-evidence recording policy. Earlier schemas remain
/// readable with frozen identities.
pub const EXECUTION_PLAN_SCHEMA_VERSION: u32 = 8;
pub const EXECUTION_PLAN_SCHEMA_VERSION_V1: u32 = 1;
pub const EXECUTION_PLAN_SCHEMA_VERSION_V2: u32 = 2;
pub const EXECUTION_PLAN_SCHEMA_VERSION_V3: u32 = 3;
pub const EXECUTION_PLAN_SCHEMA_VERSION_V4: u32 = 4;
pub const EXECUTION_PLAN_SCHEMA_VERSION_V5: u32 = 5;
pub const EXECUTION_PLAN_SCHEMA_VERSION_V6: u32 = 6;
pub const EXECUTION_PLAN_SCHEMA_VERSION_V7: u32 = 7;

/// One exact, versioned descriptor dependency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PinnedDescriptor<'a> {
    pub id: Id<'a>,
    pub schema_version: u32,
    pub semantic_hash: SemanticHash,
}

/// SHA-256 content digest, distinct from a semantic descriptor hash.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ArtifactDigest([u8; 32]);

impl ArtifactDigest {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ArtifactDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("sha256:")?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// One content-addressed implementation artifact pinned by the plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanArtifact<'a> {
    pub id: Id<'a>,
    pub digest: ArtifactDigest,
}

/// Fresh host report used during resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanHostObservation<'a> {
    pub id: Id<'a>,
    pub host: Id<'a>,
    pub semantic_hash: SemanticHash,
    pub time_basis: Id<'a>,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
}

/// One concrete host resource selected independently of permission to use it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanResourceBinding<'a> {
    pub id: Id<'a>,
    pub node: InstancePath<'a>,
    pub resource: crate::ResourceRef<'a>,
    pub host_observation: Id<'a>,
}

/// Exact resource ceiling or allocation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlanResourceBudget {
    pub memory_bytes: u64,
    pub storage_bytes: u64,
    pub cpu_units: u32,
    pub timers: u16,
    pub transports: u16,
    pub checkpoints: u16,
    pub evidence_bytes: u64,
}

impl PlanResourceBudget {
    pub const ZERO: Self = Self {
        memory_bytes: 0,
        storage_bytes: 0,
        cpu_units: 0,
        timers: 0,
        transports: 0,
        checkpoints: 0,
        evidence_bytes: 0,
    };

    fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            memory_bytes: self.memory_bytes.checked_add(other.memory_bytes)?,
            storage_bytes: self.storage_bytes.checked_add(other.storage_bytes)?,
            cpu_units: self.cpu_units.checked_add(other.cpu_units)?,
            timers: self.timers.checked_add(other.timers)?,
            transports: self.transports.checked_add(other.transports)?,
            checkpoints: self.checkpoints.checked_add(other.checkpoints)?,
            evidence_bytes: self.evidence_bytes.checked_add(other.evidence_bytes)?,
        })
    }

    fn checked_mul(self, count: u16) -> Option<Self> {
        let count_u64 = u64::from(count);
        let count_u32 = u32::from(count);
        Some(Self {
            memory_bytes: self.memory_bytes.checked_mul(count_u64)?,
            storage_bytes: self.storage_bytes.checked_mul(count_u64)?,
            cpu_units: self.cpu_units.checked_mul(count_u32)?,
            timers: self.timers.checked_mul(count)?,
            transports: self.transports.checked_mul(count)?,
            checkpoints: self.checkpoints.checked_mul(count)?,
            evidence_bytes: self.evidence_bytes.checked_mul(count_u64)?,
        })
    }

    fn fits_within(self, ceiling: Self) -> bool {
        self.memory_bytes <= ceiling.memory_bytes
            && self.storage_bytes <= ceiling.storage_bytes
            && self.cpu_units <= ceiling.cpu_units
            && self.timers <= ceiling.timers
            && self.transports <= ceiling.transports
            && self.checkpoints <= ceiling.checkpoints
            && self.evidence_bytes <= ceiling.evidence_bytes
    }
}

/// One fully selected primitive instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedPlanNode<'a> {
    pub instance: InstancePath<'a>,
    pub contract: PinnedDescriptor<'a>,
    pub implementation: PinnedDescriptor<'a>,
    pub lifecycle_policy: PinnedDescriptor<'a>,
    /// Exact bounded execution profile in plan schema 3; absent in v1/v2.
    pub execution_profile: Option<&'a ExecutionProfile<'a>>,
    pub artifact: Id<'a>,
    pub host_observation: Id<'a>,
    pub host: Id<'a>,
    pub allocation: PlanResourceBudget,
    pub required_resources: &'a [Id<'a>],
    pub required_effects: &'a [SemanticHash],
}

/// Exact endpoint facts retained without loading a registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedPlanPort<'a> {
    pub node: InstancePath<'a>,
    pub port: Id<'a>,
    pub direction: Direction,
    pub port_contract_hash: SemanticHash,
    pub value_type: TypeContractRef<'a>,
}

/// One exact bounded queue and its pinned endpoint contracts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedPlanCord<'a> {
    pub id: Id<'a>,
    pub from: ResolvedPlanPort<'a>,
    pub to: ResolvedPlanPort<'a>,
    pub flow: FlowPolicy<'a>,
    pub queue_memory_bytes: u64,
}

/// One plan-visible fan-out group. Coupled groups publish atomically to every
/// branch. Isolated groups name an ordinary duplicator and its input cord.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanFanOut<'a> {
    pub id: Id<'a>,
    pub producer: ResolvedPlanPort<'a>,
    pub mode: FanOutMode,
    pub branches: &'a [Id<'a>],
    pub duplicator: Option<InstancePath<'a>>,
    pub duplicator_input: Option<Id<'a>>,
    pub duplication: DuplicationRule<'a>,
}

/// One deterministic merge input and its policy-owned priority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanMergeInput<'a> {
    pub cord: Id<'a>,
    pub ordinal: u16,
    pub priority: u16,
}

/// One explicit ordinary merge node and exact ordering/terminal policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanMerge<'a> {
    pub id: Id<'a>,
    pub node: InstancePath<'a>,
    pub inputs: &'a [PlanMergeInput<'a>],
    pub ordering: MergeOrdering<'a>,
    pub terminal: MergeTerminalPolicy,
}

/// One explicit event stream, its publisher, resolved provider capability,
/// and complete plan-owned resource allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanEventStream<'a> {
    pub publisher: InstancePath<'a>,
    pub contract: EventStreamContract<'a>,
    pub provider_capabilities: EventProviderCapabilities,
    pub allocation: PlanResourceBudget,
}

/// One finite durable job with its owner, checkpoint provider, and exact
/// non-stream allocation. Its immutable progress stream is a separate
/// `PlanEventStream` referenced by the contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanJob<'a> {
    pub owner: InstancePath<'a>,
    pub contract: JobContract<'a>,
    pub checkpoint_provider_capabilities: Option<CheckpointProviderCapabilities>,
    pub allocation: PlanResourceBudget,
}

/// Exact plan location justified by a satisfaction proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanSatisfactionSubject<'a> {
    /// A non-exact output-to-input cord relation.
    Cord(Id<'a>),
    /// An implementation selected for one semantic node contract.
    Implementation(InstancePath<'a>),
    /// A host capability report satisfying a requirement for one node.
    HostCapability {
        node: InstancePath<'a>,
        host_observation: Id<'a>,
    },
}

/// One accepted proof retained by the exact runnable plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanSatisfactionProof<'a> {
    pub subject: PlanSatisfactionSubject<'a>,
    pub proof: SatisfactionProof<'a>,
}

/// Exact authority material used by one selected node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanAuthority<'a> {
    pub node: InstancePath<'a>,
    pub effect_hash: SemanticHash,
    pub grant_hash: SemanticHash,
    pub effect: EffectRequirement<'a>,
    pub capability: HostCapability<'a>,
    pub grant: AuthorityGrant<'a>,
    pub binding: ResolvedAuthorityBinding<'a>,
}

/// One logical boundary export retained alongside primitive expansion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanExportBinding<'a> {
    pub boundary_port: Id<'a>,
    pub member: InstancePath<'a>,
    pub member_port: Id<'a>,
    pub direction: Direction,
}

/// Exact logical-to-expanded composite provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanCompositeMapping<'a> {
    pub instance: InstancePath<'a>,
    pub definition_hash: SemanticHash,
    pub members: &'a [InstancePath<'a>],
    pub exports: &'a [PlanExportBinding<'a>],
}

/// One deterministic member derived from a port-group template.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanPortGroupMember<'a> {
    pub id: Id<'a>,
    pub ordinal: u16,
    pub port_contract_hash: SemanticHash,
}

/// Exact expansion of one port-group template.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanPortGroup<'a> {
    pub instance: InstancePath<'a>,
    pub template_hash: SemanticHash,
    /// Authored semantic maximum. This is normative in plan schema 2.
    pub maximum: u16,
    /// Direction of every complete member contract. Normative in schema 2.
    pub direction: Direction,
    pub members: &'a [PlanPortGroupMember<'a>],
}

/// Exact bounded replicated-composite pool reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanInstancePool<'a> {
    pub instance: InstancePath<'a>,
    pub template_hash: SemanticHash,
    pub derived_identity_hash: SemanticHash,
    pub maximum_live: u16,
    pub maximum_queued: u16,
    pub admission_policy: PinnedDescriptor<'a>,
    pub supervision_policy: PinnedDescriptor<'a>,
    pub per_instance_budget: PlanResourceBudget,
    pub authority_grants: &'a [Id<'a>],
    pub maximum_instance_ticks: u64,
    pub implementation_set_hash: SemanticHash,
    pub correlation_slots: u16,
    pub worst_case_budget: PlanResourceBudget,
    pub child_nodes: u16,
    pub child_cords: u16,
}

/// Selector category retained only in a non-runnable draft plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnresolvedPlanKind {
    Implementation,
    Artifact,
    Host,
    Resource,
    Authority,
    Contract,
}

impl UnresolvedPlanKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Implementation => "implementation",
            Self::Artifact => "artifact",
            Self::Host => "host",
            Self::Resource => "resource",
            Self::Authority => "authority",
            Self::Contract => "contract",
        }
    }
}

/// One unresolved selector. Portable runnable validation rejects every entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnresolvedPlanConstraint<'a> {
    pub id: Id<'a>,
    pub requester: InstancePath<'a>,
    pub kind: UnresolvedPlanKind,
}

/// Exact runnable arrangement, distinct from source and evidence identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionPlan<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub source_semantic_hash: SemanticHash,
    pub resolver: PinnedDescriptor<'a>,
    pub resolver_policy_hash: SemanticHash,
    pub created_at: AuthorityTime<'a>,
    pub budget: PlanResourceBudget,
    pub host_observations: &'a [PlanHostObservation<'a>],
    pub resources: &'a [PlanResourceBinding<'a>],
    pub artifacts: &'a [PlanArtifact<'a>],
    pub nodes: &'a [ResolvedPlanNode<'a>],
    pub cords: &'a [ResolvedPlanCord<'a>],
    /// Structural plan facts introduced in schema 4.
    pub fanouts: &'a [PlanFanOut<'a>],
    pub merges: &'a [PlanMerge<'a>],
    /// Resonance stream facts introduced in schema 5.
    pub event_streams: &'a [PlanEventStream<'a>],
    /// Exact executor-evidence projection policy introduced in schema 8.
    pub runtime_evidence: Option<RuntimeEvidencePolicy<'a>>,
    /// Durable finite-job facts introduced in schema 6.
    pub jobs: &'a [PlanJob<'a>],
    /// Accepted implicit-satisfaction proofs introduced in schema 7.
    pub satisfaction_proofs: &'a [PlanSatisfactionProof<'a>],
    pub authorities: &'a [PlanAuthority<'a>],
    pub composites: &'a [PlanCompositeMapping<'a>],
    pub port_groups: &'a [PlanPortGroup<'a>],
    pub instance_pools: &'a [PlanInstancePool<'a>],
    pub unresolved: &'a [UnresolvedPlanConstraint<'a>],
}

/// Run-start observation used only for freshness validation, never identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanValidationContext<'a> {
    pub supported_schema_version: u32,
    pub now: AuthorityTime<'a>,
}

/// Exact plan collection associated with a diagnostic index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanCollection {
    Header,
    HostObservations,
    Resources,
    Artifacts,
    Nodes,
    Cords,
    FanOuts,
    Merges,
    EventStreams,
    RuntimeEvidence,
    Jobs,
    SatisfactionProofs,
    Authorities,
    Composites,
    PortGroups,
    InstancePools,
    Unresolved,
}

/// Stable portable plan-validation reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanDiagnosticCode {
    UnsupportedVersion,
    IdentityMismatch,
    InvalidDescriptor,
    DuplicateIdentity,
    DanglingReference,
    UnresolvedSelection,
    MissingArtifact,
    StaleHostObservation,
    BudgetExceeded,
    AuthorityInvalid,
    QueueInvalid,
    DirectionInvalid,
    ContractMismatch,
    StructuralInvalid,
    DuplicationUnauthorized,
    StructuralOrderingInvalid,
    EventStreamInvalid,
    RuntimeEvidenceInvalid,
    JobInvalid,
    SatisfactionInvalid,
    ScratchTooSmall,
}

impl PlanDiagnosticCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-PLN-001",
            Self::IdentityMismatch => "CND-PLN-002",
            Self::InvalidDescriptor => "CND-PLN-003",
            Self::DuplicateIdentity => "CND-ID-002",
            Self::DanglingReference => "CND-PLN-004",
            Self::UnresolvedSelection => "CND-PLN-005",
            Self::MissingArtifact => "CND-ART-001",
            Self::StaleHostObservation => "CND-HST-002",
            Self::BudgetExceeded => "CND-PLN-006",
            Self::AuthorityInvalid => "CND-AUT-007",
            Self::QueueInvalid => "CND-FLW-001",
            Self::DirectionInvalid => "CND-PRT-001",
            Self::ContractMismatch => "CND-TYP-001",
            Self::StructuralInvalid => "CND-STR-003",
            Self::DuplicationUnauthorized => "CND-STR-004",
            Self::StructuralOrderingInvalid => "CND-STR-005",
            Self::EventStreamInvalid => "CND-RSN-003",
            Self::RuntimeEvidenceInvalid => "CND-RTE-002",
            Self::JobInvalid => "CND-JOB-016",
            Self::SatisfactionInvalid => "CND-IMP-017",
            Self::ScratchTooSmall => "CND-PLN-007",
        }
    }
}

/// First deterministic portable validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanValidationError {
    pub code: PlanDiagnosticCode,
    pub collection: PlanCollection,
    pub subject_index: Option<u16>,
}

impl fmt::Display for PlanValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} in {:?}", self.code.as_str(), self.collection)?;
        if let Some(index) = self.subject_index {
            write!(formatter, " at index {index}")?;
        }
        Ok(())
    }
}

/// Plan-identity construction failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanIdentityError {
    ScratchTooSmall,
    FactCountOverflow,
    Canonical(CanonicalError<Infallible>),
}

impl From<CanonicalError<Infallible>> for PlanIdentityError {
    fn from(error: CanonicalError<Infallible>) -> Self {
        Self::Canonical(error)
    }
}

impl ExecutionPlan<'_> {
    /// Number of leaf facts required in identity scratch.
    pub fn identity_fact_count(&self) -> Result<usize, PlanIdentityError> {
        let mut count = self
            .host_observations
            .len()
            .checked_add(self.resources.len())
            .and_then(|value| value.checked_add(self.artifacts.len()))
            .and_then(|value| value.checked_add(self.nodes.len()))
            .and_then(|value| value.checked_add(self.cords.len()))
            .and_then(|value| value.checked_add(self.fanouts.len()))
            .and_then(|value| value.checked_add(self.merges.len()))
            .and_then(|value| value.checked_add(self.event_streams.len()))
            .and_then(|value| value.checked_add(usize::from(self.runtime_evidence.is_some())))
            .and_then(|value| value.checked_add(self.jobs.len()))
            .and_then(|value| value.checked_add(self.satisfaction_proofs.len()))
            .and_then(|value| value.checked_add(self.authorities.len()))
            .and_then(|value| value.checked_add(self.unresolved.len()))
            .ok_or(PlanIdentityError::FactCountOverflow)?;
        for node in self.nodes {
            count = count
                .checked_add(node.required_resources.len())
                .and_then(|value| value.checked_add(node.required_effects.len()))
                .and_then(|value| {
                    value.checked_add(usize::from(
                        self.schema_version >= 3 && node.execution_profile.is_some(),
                    ))
                })
                .ok_or(PlanIdentityError::FactCountOverflow)?;
        }
        for composite in self.composites {
            count = count
                .checked_add(1)
                .and_then(|value| value.checked_add(composite.members.len()))
                .and_then(|value| value.checked_add(composite.exports.len()))
                .ok_or(PlanIdentityError::FactCountOverflow)?;
        }
        for fanout in self.fanouts {
            count = count
                .checked_add(fanout.branches.len())
                .ok_or(PlanIdentityError::FactCountOverflow)?;
        }
        for merge in self.merges {
            count = count
                .checked_add(merge.inputs.len())
                .ok_or(PlanIdentityError::FactCountOverflow)?;
        }
        for group in self.port_groups {
            count = count
                .checked_add(1)
                .and_then(|value| value.checked_add(group.members.len()))
                .ok_or(PlanIdentityError::FactCountOverflow)?;
        }
        for pool in self.instance_pools {
            count = count
                .checked_add(1)
                .and_then(|value| value.checked_add(pool.authority_grants.len()))
                .ok_or(PlanIdentityError::FactCountOverflow)?;
        }
        Ok(count)
    }

    /// Scratch slots sufficient for both embedded profile and plan validation.
    pub fn validation_scratch_count(&self) -> Result<usize, PlanIdentityError> {
        let plan = self.identity_fact_count()?;
        Ok(self
            .nodes
            .iter()
            .filter_map(|node| node.execution_profile)
            .map(ExecutionProfile::identity_fact_count)
            .chain(
                self.satisfaction_proofs
                    .iter()
                    .map(|binding| binding.proof.identity_fact_count()),
            )
            .fold(plan, usize::max))
    }

    /// Canonical semantic identity, independent of every collection's input order.
    pub fn semantic_hash(
        &self,
        fact_hashes: &mut [SemanticHash],
    ) -> Result<SemanticHash, PlanIdentityError> {
        let needed = self.identity_fact_count()?;
        if fact_hashes.len() < needed {
            return Err(PlanIdentityError::ScratchTooSmall);
        }
        let mut cursor = 0;
        macro_rules! push {
            ($value:expr) => {{
                fact_hashes[cursor] = $value?;
                cursor += 1;
            }};
        }

        for value in self.host_observations {
            push!(hash_host_observation(*value));
        }
        for value in self.resources {
            push!(hash_resource_binding(*value));
        }
        for value in self.artifacts {
            push!(hash_artifact(*value));
        }
        for value in self.nodes {
            push!(hash_node(*value));
            if self.schema_version >= 3 {
                if let Some(profile) = value.execution_profile {
                    push!(hash_node_execution_profile(
                        value.instance,
                        profile.semantic_hash
                    ));
                }
            }
            for resource in value.required_resources {
                push!(hash_node_resource(value.instance, *resource));
            }
            for effect in value.required_effects {
                push!(hash_node_effect(value.instance, *effect));
            }
        }
        for value in self.cords {
            push!(hash_cord(*value));
        }
        for fanout in self.fanouts {
            push!(hash_fanout(*fanout));
            for branch in fanout.branches {
                push!(hash_fanout_branch(fanout.id, *branch));
            }
        }
        for merge in self.merges {
            push!(hash_merge(*merge));
            for input in merge.inputs {
                push!(hash_merge_input(merge.id, *input));
            }
        }
        for stream in self.event_streams {
            push!(hash_event_stream(*stream));
        }
        if let Some(policy) = self.runtime_evidence {
            push!(hash_runtime_evidence_policy(policy));
        }
        for job in self.jobs {
            push!(hash_job(*job));
        }
        for proof in self.satisfaction_proofs {
            push!(hash_satisfaction_proof(*proof));
        }
        for value in self.authorities {
            push!(hash_authority(*value));
        }
        for composite in self.composites {
            push!(hash_composite(*composite));
            for member in composite.members {
                push!(hash_composite_member(composite.instance, *member));
            }
            for export in composite.exports {
                push!(hash_export(composite.instance, *export));
            }
        }
        for group in self.port_groups {
            push!(hash_port_group(self.schema_version, *group));
            for member in group.members {
                push!(hash_port_group_member(group.instance, *member));
            }
        }
        for pool in self.instance_pools {
            push!(hash_instance_pool(*pool));
            for grant in pool.authority_grants {
                push!(hash_pool_authority(pool.instance, *grant));
            }
        }
        for value in self.unresolved {
            push!(hash_unresolved(*value));
        }
        debug_assert_eq!(cursor, needed);

        let fields = [
            semantic(
                "budget_memory_bytes",
                CanonicalValue::Integer(i128::from(self.budget.memory_bytes)),
            ),
            semantic(
                "budget_storage_bytes",
                CanonicalValue::Integer(i128::from(self.budget.storage_bytes)),
            ),
            semantic(
                "budget_cpu_units",
                CanonicalValue::Integer(i128::from(self.budget.cpu_units)),
            ),
            semantic(
                "budget_timers",
                CanonicalValue::Integer(i128::from(self.budget.timers)),
            ),
            semantic(
                "budget_transports",
                CanonicalValue::Integer(i128::from(self.budget.transports)),
            ),
            semantic(
                "budget_checkpoints",
                CanonicalValue::Integer(i128::from(self.budget.checkpoints)),
            ),
            semantic(
                "budget_evidence_bytes",
                CanonicalValue::Integer(i128::from(self.budget.evidence_bytes)),
            ),
            semantic(
                "created_time_basis",
                CanonicalValue::Identifier(self.created_at.basis),
            ),
            semantic(
                "created_tick",
                CanonicalValue::Integer(i128::from(self.created_at.tick)),
            ),
            semantic("resolver_id", CanonicalValue::Identifier(self.resolver.id)),
            semantic(
                "resolver_schema_version",
                CanonicalValue::Integer(i128::from(self.resolver.schema_version)),
            ),
            semantic(
                "resolver_semantic_hash",
                CanonicalValue::Bytes(self.resolver.semantic_hash.as_bytes()),
            ),
            semantic(
                "resolver_policy_hash",
                CanonicalValue::Bytes(self.resolver_policy_hash.as_bytes()),
            ),
            semantic(
                "source_semantic_hash",
                CanonicalValue::Bytes(self.source_semantic_hash.as_bytes()),
            ),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/execution-plan"),
            self.schema_version,
            &fields,
            Id("facts"),
            &fact_hashes[..needed],
        )
        .map_err(PlanIdentityError::Canonical)
    }
}

/// Validate exact structure, pinned identity, budgets, authority, and freshness.
pub fn validate_execution_plan(
    plan: &ExecutionPlan<'_>,
    context: PlanValidationContext<'_>,
    identity_scratch: &mut [SemanticHash],
) -> Result<(), PlanValidationError> {
    if !(EXECUTION_PLAN_SCHEMA_VERSION_V1..=EXECUTION_PLAN_SCHEMA_VERSION)
        .contains(&plan.schema_version)
        || !(EXECUTION_PLAN_SCHEMA_VERSION_V1..=EXECUTION_PLAN_SCHEMA_VERSION)
            .contains(&context.supported_schema_version)
        || plan.schema_version > context.supported_schema_version
    {
        return Err(error(
            PlanDiagnosticCode::UnsupportedVersion,
            PlanCollection::Header,
            None,
        ));
    }
    if !plan.unresolved.is_empty() {
        return Err(error(
            PlanDiagnosticCode::UnresolvedSelection,
            PlanCollection::Unresolved,
            Some(0),
        ));
    }
    if plan.nodes.is_empty()
        || !valid_pin(plan.resolver)
        || !valid_id(plan.created_at.basis)
        || !valid_id(context.now.basis)
        || plan.created_at.basis != context.now.basis
        || context.now.tick < plan.created_at.tick
    {
        return Err(error(
            PlanDiagnosticCode::InvalidDescriptor,
            PlanCollection::Header,
            None,
        ));
    }

    for (index, observation) in plan.host_observations.iter().enumerate() {
        if !valid_id(observation.id)
            || !valid_id(observation.host)
            || !valid_id(observation.time_basis)
            || observation.valid_until_tick <= observation.observed_at_tick
        {
            return Err(indexed(
                PlanDiagnosticCode::InvalidDescriptor,
                PlanCollection::HostObservations,
                index,
            ));
        }
        if plan.host_observations[..index]
            .iter()
            .any(|prior| prior.id == observation.id)
        {
            return Err(indexed(
                PlanDiagnosticCode::DuplicateIdentity,
                PlanCollection::HostObservations,
                index,
            ));
        }
        if observation.time_basis != plan.created_at.basis
            || observation.observed_at_tick > plan.created_at.tick
            || plan.created_at.tick >= observation.valid_until_tick
            || observation.time_basis != context.now.basis
            || observation.observed_at_tick > context.now.tick
            || context.now.tick >= observation.valid_until_tick
        {
            return Err(indexed(
                PlanDiagnosticCode::StaleHostObservation,
                PlanCollection::HostObservations,
                index,
            ));
        }
    }

    for (index, resource) in plan.resources.iter().enumerate() {
        if !valid_id(resource.id)
            || !valid_path(resource.node)
            || !valid_id(resource.resource.kind)
            || !valid_id(resource.resource.id)
            || !valid_id(resource.host_observation)
            || !plan.nodes.iter().any(|node| {
                node.instance == resource.node && node.required_resources.contains(&resource.id)
            })
            || !plan
                .host_observations
                .iter()
                .any(|observation| observation.id == resource.host_observation)
        {
            return Err(indexed(
                PlanDiagnosticCode::DanglingReference,
                PlanCollection::Resources,
                index,
            ));
        }
        if plan.resources[..index]
            .iter()
            .any(|prior| prior.id == resource.id)
        {
            return Err(indexed(
                PlanDiagnosticCode::DuplicateIdentity,
                PlanCollection::Resources,
                index,
            ));
        }
    }

    for (index, artifact) in plan.artifacts.iter().enumerate() {
        if !valid_id(artifact.id) {
            return Err(indexed(
                PlanDiagnosticCode::InvalidDescriptor,
                PlanCollection::Artifacts,
                index,
            ));
        }
        if plan.artifacts[..index]
            .iter()
            .any(|prior| prior.id == artifact.id)
        {
            return Err(indexed(
                PlanDiagnosticCode::DuplicateIdentity,
                PlanCollection::Artifacts,
                index,
            ));
        }
    }

    let mut allocated = PlanResourceBudget::ZERO;
    for (index, node) in plan.nodes.iter().enumerate() {
        if !valid_path(node.instance)
            || !valid_pin(node.contract)
            || !valid_pin(node.implementation)
            || !valid_pin(node.lifecycle_policy)
            || !valid_id(node.artifact)
            || !valid_id(node.host_observation)
            || !valid_id(node.host)
        {
            return Err(indexed(
                PlanDiagnosticCode::InvalidDescriptor,
                PlanCollection::Nodes,
                index,
            ));
        }
        match (plan.schema_version >= 3, node.execution_profile) {
            (true, Some(profile)) => {
                validate_plan_execution_profile(profile, node.allocation, identity_scratch)
                    .map_err(|_| {
                        indexed(
                            PlanDiagnosticCode::InvalidDescriptor,
                            PlanCollection::Nodes,
                            index,
                        )
                    })?;
            }
            (false, None) => {}
            _ => {
                return Err(indexed(
                    PlanDiagnosticCode::InvalidDescriptor,
                    PlanCollection::Nodes,
                    index,
                ));
            }
        }
        if plan.nodes[..index]
            .iter()
            .any(|prior| prior.instance == node.instance)
        {
            return Err(indexed(
                PlanDiagnosticCode::DuplicateIdentity,
                PlanCollection::Nodes,
                index,
            ));
        }
        if !plan
            .artifacts
            .iter()
            .any(|artifact| artifact.id == node.artifact)
        {
            return Err(indexed(
                PlanDiagnosticCode::MissingArtifact,
                PlanCollection::Nodes,
                index,
            ));
        }
        if !plan.host_observations.iter().any(|observation| {
            observation.id == node.host_observation && observation.host == node.host
        }) {
            return Err(indexed(
                PlanDiagnosticCode::DanglingReference,
                PlanCollection::Nodes,
                index,
            ));
        }
        allocated = allocated.checked_add(node.allocation).ok_or_else(|| {
            indexed(
                PlanDiagnosticCode::BudgetExceeded,
                PlanCollection::Nodes,
                index,
            )
        })?;
        for (resource_index, resource_id) in node.required_resources.iter().enumerate() {
            if !valid_id(*resource_id)
                || node.required_resources[..resource_index].contains(resource_id)
                || !plan.resources.iter().any(|resource| {
                    resource.id == *resource_id
                        && resource.node == node.instance
                        && resource.host_observation == node.host_observation
                })
            {
                return Err(indexed(
                    PlanDiagnosticCode::DanglingReference,
                    PlanCollection::Nodes,
                    index,
                ));
            }
        }
        for (effect_index, effect_hash) in node.required_effects.iter().enumerate() {
            if node.required_effects[..effect_index].contains(effect_hash)
                || !plan.authorities.iter().any(|authority| {
                    authority.node == node.instance && authority.effect_hash == *effect_hash
                })
            {
                return Err(indexed(
                    PlanDiagnosticCode::AuthorityInvalid,
                    PlanCollection::Nodes,
                    index,
                ));
            }
        }
    }

    for (index, cord) in plan.cords.iter().enumerate() {
        if !valid_id(cord.id)
            || !valid_port(cord.from)
            || !valid_port(cord.to)
            || FlowPolicy::new(cord.flow.capacity, cord.flow.pressure, cord.flow.watermarks)
                .is_err()
            || cord.queue_memory_bytes != cord.flow.capacity.max_queued_bytes()
        {
            return Err(indexed(
                PlanDiagnosticCode::QueueInvalid,
                PlanCollection::Cords,
                index,
            ));
        }
        if plan.cords[..index].iter().any(|prior| prior.id == cord.id) {
            return Err(indexed(
                PlanDiagnosticCode::DuplicateIdentity,
                PlanCollection::Cords,
                index,
            ));
        }
        if cord.from.direction != Direction::Output || cord.to.direction != Direction::Input {
            return Err(indexed(
                PlanDiagnosticCode::DirectionInvalid,
                PlanCollection::Cords,
                index,
            ));
        }
        if !plan
            .nodes
            .iter()
            .any(|node| node.instance == cord.from.node)
            || !plan.nodes.iter().any(|node| node.instance == cord.to.node)
        {
            return Err(indexed(
                PlanDiagnosticCode::DanglingReference,
                PlanCollection::Cords,
                index,
            ));
        }
        if cord.from.value_type != cord.to.value_type
            && (plan.schema_version < 7
                || !plan.satisfaction_proofs.iter().any(|binding| {
                    matches!(binding.subject, PlanSatisfactionSubject::Cord(id) if id == cord.id)
                }))
        {
            return Err(indexed(
                PlanDiagnosticCode::ContractMismatch,
                PlanCollection::Cords,
                index,
            ));
        }
        allocated = allocated
            .checked_add(PlanResourceBudget {
                memory_bytes: cord.queue_memory_bytes,
                ..PlanResourceBudget::ZERO
            })
            .ok_or_else(|| {
                indexed(
                    PlanDiagnosticCode::BudgetExceeded,
                    PlanCollection::Cords,
                    index,
                )
            })?;
    }

    if plan.schema_version < 4 && (!plan.fanouts.is_empty() || !plan.merges.is_empty()) {
        return Err(error(
            PlanDiagnosticCode::StructuralInvalid,
            PlanCollection::Header,
            None,
        ));
    }
    for (index, fanout) in plan.fanouts.iter().enumerate() {
        let valid_copy = match fanout.duplication {
            DuplicationRule::SharedHandle => plan
                .nodes
                .iter()
                .find(|node| node.instance == fanout.producer.node)
                .and_then(|node| node.execution_profile)
                .is_some_and(|profile| {
                    profile.representations.iter().any(|representation| {
                        representation.direction == Direction::Output
                            && representation.port == fanout.producer.port
                            && representation.ownership == OwnershipModel::SharedHandle
                    })
                }),
            DuplicationRule::Copy(pin) => valid_pin(pin),
        };
        let branches_valid = fanout.branches.len() >= 2
            && fanout
                .branches
                .iter()
                .enumerate()
                .all(|(branch_index, branch)| {
                    valid_id(*branch)
                        && !fanout.branches[..branch_index].contains(branch)
                        && plan.cords.iter().any(|cord| {
                            cord.id == *branch
                                && cord.from.node == fanout.producer.node
                                && cord.from.port == fanout.producer.port
                        })
                });
        let mode_valid = match fanout.mode {
            FanOutMode::Coupled => fanout.duplicator.is_none() && fanout.duplicator_input.is_none(),
            FanOutMode::Isolated => {
                fanout.duplicator == Some(fanout.producer.node)
                    && fanout.duplicator_input.is_some_and(|input| {
                        plan.cords.iter().any(|cord| {
                            cord.id == input
                                && cord.to.node == fanout.producer.node
                                && !fanout.branches.contains(&input)
                        })
                    })
            }
        };
        if !valid_copy {
            return Err(indexed(
                PlanDiagnosticCode::DuplicationUnauthorized,
                PlanCollection::FanOuts,
                index,
            ));
        }
        if !valid_id(fanout.id)
            || !valid_port(fanout.producer)
            || fanout.producer.direction != Direction::Output
            || !branches_valid
            || !mode_valid
        {
            return Err(indexed(
                PlanDiagnosticCode::StructuralInvalid,
                PlanCollection::FanOuts,
                index,
            ));
        }
        if plan.fanouts[..index].iter().any(|prior| {
            prior.id == fanout.id
                || prior
                    .branches
                    .iter()
                    .any(|branch| fanout.branches.contains(branch))
        }) {
            return Err(indexed(
                PlanDiagnosticCode::DuplicateIdentity,
                PlanCollection::FanOuts,
                index,
            ));
        }
    }
    if plan.schema_version >= 4 {
        for (cord_index, cord) in plan.cords.iter().enumerate() {
            if plan.cords[..cord_index]
                .iter()
                .any(|prior| prior.from == cord.from)
            {
                continue;
            }
            let outgoing = plan
                .cords
                .iter()
                .filter(|candidate| candidate.from == cord.from)
                .count();
            if outgoing > 1 {
                let exact = plan.fanouts.iter().filter(|fanout| {
                    fanout.producer == cord.from
                        && fanout.branches.len() == outgoing
                        && plan
                            .cords
                            .iter()
                            .filter(|candidate| candidate.from == cord.from)
                            .all(|candidate| fanout.branches.contains(&candidate.id))
                });
                if exact.count() != 1 {
                    return Err(indexed(
                        PlanDiagnosticCode::StructuralInvalid,
                        PlanCollection::Cords,
                        cord_index,
                    ));
                }
            }
        }
    }
    for (index, merge) in plan.merges.iter().enumerate() {
        let ordering_valid = match merge.ordering {
            MergeOrdering::Arrival | MergeOrdering::RoundRobin => {
                merge.inputs.iter().all(|input| input.priority == 0)
            }
            MergeOrdering::Priority { starvation_turns } => starvation_turns > 0,
            MergeOrdering::EventTime {
                timestamp_type,
                maximum_lateness_ticks,
                ..
            } => {
                timestamp_type.validate().is_ok()
                    && maximum_lateness_ticks > 0
                    && merge.inputs.iter().all(|input| input.priority == 0)
            }
        };
        let inputs_valid = merge.inputs.len() >= 2
            && merge.inputs.iter().enumerate().all(|(input_index, input)| {
                input.ordinal == input_index as u16
                    && valid_id(input.cord)
                    && !merge.inputs[..input_index]
                        .iter()
                        .any(|prior| prior.cord == input.cord)
                    && plan
                        .cords
                        .iter()
                        .any(|cord| cord.id == input.cord && cord.to.node == merge.node)
            });
        if !ordering_valid {
            return Err(indexed(
                PlanDiagnosticCode::StructuralOrderingInvalid,
                PlanCollection::Merges,
                index,
            ));
        }
        if !valid_id(merge.id)
            || !valid_path(merge.node)
            || !plan.nodes.iter().any(|node| node.instance == merge.node)
            || !inputs_valid
        {
            return Err(indexed(
                PlanDiagnosticCode::StructuralInvalid,
                PlanCollection::Merges,
                index,
            ));
        }
        if plan.merges[..index]
            .iter()
            .any(|prior| prior.id == merge.id)
        {
            return Err(indexed(
                PlanDiagnosticCode::DuplicateIdentity,
                PlanCollection::Merges,
                index,
            ));
        }
    }

    if plan.schema_version < 5 && !plan.event_streams.is_empty() {
        return Err(error(
            PlanDiagnosticCode::EventStreamInvalid,
            PlanCollection::Header,
            None,
        ));
    }
    for (index, stream) in plan.event_streams.iter().enumerate() {
        if !valid_path(stream.publisher)
            || !plan
                .nodes
                .iter()
                .any(|node| node.instance == stream.publisher)
            || validate_stream_contract(stream.contract, stream.provider_capabilities).is_err()
            || !event_stream_allocation_valid(*stream)
            || plan.event_streams[..index]
                .iter()
                .any(|prior| prior.contract.id == stream.contract.id)
        {
            return Err(indexed(
                PlanDiagnosticCode::EventStreamInvalid,
                PlanCollection::EventStreams,
                index,
            ));
        }
        allocated = allocated.checked_add(stream.allocation).ok_or_else(|| {
            indexed(
                PlanDiagnosticCode::BudgetExceeded,
                PlanCollection::EventStreams,
                index,
            )
        })?;
    }

    if plan.schema_version < 8 && plan.runtime_evidence.is_some() {
        return Err(error(
            PlanDiagnosticCode::RuntimeEvidenceInvalid,
            PlanCollection::Header,
            None,
        ));
    }
    if let Some(policy) = plan.runtime_evidence {
        let stream = policy.stream.and_then(|stream_id| {
            plan.event_streams
                .iter()
                .find(|stream| stream.contract.id == stream_id)
                .map(|stream| (stream.contract, stream.provider_capabilities))
        });
        if validate_runtime_evidence_policy(policy, stream).is_err() {
            return Err(error(
                PlanDiagnosticCode::RuntimeEvidenceInvalid,
                PlanCollection::RuntimeEvidence,
                None,
            ));
        }
    }

    if plan.schema_version < 6 && !plan.jobs.is_empty() {
        return Err(error(
            PlanDiagnosticCode::JobInvalid,
            PlanCollection::Header,
            None,
        ));
    }
    for (index, job) in plan.jobs.iter().enumerate() {
        let Some(stream) = plan
            .event_streams
            .iter()
            .find(|stream| stream.contract.id == job.contract.evidence_stream)
        else {
            return Err(indexed(
                PlanDiagnosticCode::JobInvalid,
                PlanCollection::Jobs,
                index,
            ));
        };
        if !valid_path(job.owner)
            || !plan.nodes.iter().any(|node| node.instance == job.owner)
            || plan.jobs[..index]
                .iter()
                .any(|prior| prior.contract.id == job.contract.id)
            || validate_job_contract(
                job.contract,
                job.checkpoint_provider_capabilities,
                stream.contract,
                stream.provider_capabilities,
            )
            .is_err()
            || !job_allocation_valid(*job)
        {
            return Err(indexed(
                PlanDiagnosticCode::JobInvalid,
                PlanCollection::Jobs,
                index,
            ));
        }
        allocated = allocated.checked_add(job.allocation).ok_or_else(|| {
            indexed(
                PlanDiagnosticCode::BudgetExceeded,
                PlanCollection::Jobs,
                index,
            )
        })?;
    }

    if plan.schema_version < 7 && !plan.satisfaction_proofs.is_empty() {
        return Err(error(
            PlanDiagnosticCode::SatisfactionInvalid,
            PlanCollection::Header,
            None,
        ));
    }
    for (index, binding) in plan.satisfaction_proofs.iter().enumerate() {
        let subject_valid = match binding.subject {
            PlanSatisfactionSubject::Cord(id) => {
                binding.proof.role == SatisfactionRole::PortConnection
                    && plan
                        .cords
                        .iter()
                        .find(|cord| cord.id == id)
                        .is_some_and(|cord| {
                            binding.proof.required
                                == DescriptorRef {
                                    kind: Id("conduit/port-contract"),
                                    schema_version: 1,
                                    semantic_hash: cord.to.port_contract_hash,
                                }
                                && binding.proof.offered
                                    == DescriptorRef {
                                        kind: Id("conduit/port-contract"),
                                        schema_version: 1,
                                        semantic_hash: cord.from.port_contract_hash,
                                    }
                                && binding.proof.obligations.iter().any(|obligation| {
                                    obligation.id == Id("semantic-type")
                                        && obligation.required_hash
                                            == cord.to.value_type.semantic_hash
                                        && obligation.offered_hash
                                            == cord.from.value_type.semantic_hash
                                })
                        })
            }
            PlanSatisfactionSubject::Implementation(instance) => {
                binding.proof.role == SatisfactionRole::Implementation
                    && plan
                        .nodes
                        .iter()
                        .find(|node| node.instance == instance)
                        .is_some_and(|node| {
                            binding.proof.required
                                == DescriptorRef {
                                    kind: node.contract.id,
                                    schema_version: node.contract.schema_version,
                                    semantic_hash: node.contract.semantic_hash,
                                }
                                && binding.proof.offered
                                    == DescriptorRef {
                                        kind: node.implementation.id,
                                        schema_version: node.implementation.schema_version,
                                        semantic_hash: node.implementation.semantic_hash,
                                    }
                        })
            }
            PlanSatisfactionSubject::HostCapability {
                node,
                host_observation,
            } => {
                binding.proof.role == SatisfactionRole::HostCapability
                    && plan.nodes.iter().any(|candidate| {
                        candidate.instance == node && candidate.host_observation == host_observation
                    })
                    && plan.host_observations.iter().any(|observation| {
                        observation.id == host_observation
                            && binding.proof.offered.semantic_hash == observation.semantic_hash
                    })
            }
        };
        if !subject_valid
            || binding.proof.outcome != crate::CompatibilityOutcome::Compatible
            || validate_satisfaction_proof(&binding.proof, identity_scratch).is_err()
            || plan.satisfaction_proofs[..index]
                .iter()
                .any(|prior| prior.subject == binding.subject)
        {
            return Err(indexed(
                PlanDiagnosticCode::SatisfactionInvalid,
                PlanCollection::SatisfactionProofs,
                index,
            ));
        }
    }

    for (index, authority) in plan.authorities.iter().enumerate() {
        let Some(node) = plan
            .nodes
            .iter()
            .find(|node| node.instance == authority.node)
        else {
            return Err(indexed(
                PlanDiagnosticCode::DanglingReference,
                PlanCollection::Authorities,
                index,
            ));
        };
        if plan.authorities[..index]
            .iter()
            .any(|prior| prior.node == authority.node && prior.effect_hash == authority.effect_hash)
            || !node.required_effects.contains(&authority.effect_hash)
            || authority.effect.requester != node.instance
            || authority.capability.host != node.host
            || !plan.resources.iter().any(|resource| {
                resource.node == authority.node
                    && resource.resource == authority.capability.resource
                    && resource.host_observation == node.host_observation
            })
            || authority.effect.semantic_hash().ok() != Some(authority.effect_hash)
            || authority.grant.semantic_hash().ok() != Some(authority.grant_hash)
            || validate_authority_at_use(
                authority.binding,
                authority.effect,
                plan.created_at,
                authority.capability,
                ObservedGrant {
                    grant: authority.grant,
                    status: GrantStatus::Active,
                },
            )
            .is_err()
            || validate_authority_at_use(
                authority.binding,
                authority.effect,
                context.now,
                authority.capability,
                ObservedGrant {
                    grant: authority.grant,
                    status: GrantStatus::Active,
                },
            )
            .is_err()
        {
            return Err(indexed(
                PlanDiagnosticCode::AuthorityInvalid,
                PlanCollection::Authorities,
                index,
            ));
        }
    }

    for (index, composite) in plan.composites.iter().enumerate() {
        if !valid_path(composite.instance)
            || composite.members.is_empty()
            || composite
                .members
                .iter()
                .any(|member| !plan.nodes.iter().any(|node| node.instance == *member))
            || has_duplicate_paths(composite.members)
            || composite.exports.iter().any(|export| {
                !valid_id(export.boundary_port)
                    || !valid_id(export.member_port)
                    || !composite.members.contains(&export.member)
            })
        {
            return Err(indexed(
                PlanDiagnosticCode::DanglingReference,
                PlanCollection::Composites,
                index,
            ));
        }
        if plan.composites[..index]
            .iter()
            .any(|prior| prior.instance == composite.instance)
        {
            return Err(indexed(
                PlanDiagnosticCode::DuplicateIdentity,
                PlanCollection::Composites,
                index,
            ));
        }
        for (export_index, export) in composite.exports.iter().enumerate() {
            if composite.exports[..export_index].iter().any(|prior| {
                prior.boundary_port == export.boundary_port && prior.direction == export.direction
            }) {
                return Err(indexed(
                    PlanDiagnosticCode::DuplicateIdentity,
                    PlanCollection::Composites,
                    index,
                ));
            }
        }
    }

    for (index, group) in plan.port_groups.iter().enumerate() {
        let invalid_v2_bounds = plan.schema_version >= 2
            && (group.maximum == 0 || group.members.len() > usize::from(group.maximum));
        if !valid_path(group.instance) || group.members.is_empty() || invalid_v2_bounds {
            return Err(indexed(
                PlanDiagnosticCode::InvalidDescriptor,
                PlanCollection::PortGroups,
                index,
            ));
        }
        if plan.port_groups[..index]
            .iter()
            .any(|prior| prior.instance == group.instance)
        {
            return Err(indexed(
                PlanDiagnosticCode::DuplicateIdentity,
                PlanCollection::PortGroups,
                index,
            ));
        }
        for (member_index, member) in group.members.iter().enumerate() {
            if !valid_id(member.id)
                || usize::from(member.ordinal) >= group.members.len()
                || group.members[..member_index]
                    .iter()
                    .any(|prior| prior.id == member.id || prior.ordinal == member.ordinal)
            {
                return Err(indexed(
                    PlanDiagnosticCode::InvalidDescriptor,
                    PlanCollection::PortGroups,
                    index,
                ));
            }
        }
    }

    for (index, pool) in plan.instance_pools.iter().enumerate() {
        let needed_slots = pool.maximum_live.checked_add(pool.maximum_queued);
        let minimum_budget = pool.per_instance_budget.checked_mul(pool.maximum_live);
        if !valid_path(pool.instance)
            || !valid_pin(pool.admission_policy)
            || !valid_pin(pool.supervision_policy)
            || pool.maximum_live == 0
            || pool.maximum_instance_ticks == 0
            || pool.child_nodes == 0
            || pool.child_cords == 0
            || needed_slots.is_none_or(|needed| pool.correlation_slots < needed)
            || minimum_budget.is_none_or(|minimum| !minimum.fits_within(pool.worst_case_budget))
            || pool.authority_grants.iter().any(|grant| {
                !valid_id(*grant)
                    || !plan
                        .authorities
                        .iter()
                        .any(|authority| authority.grant.id == *grant)
            })
            || pool
                .authority_grants
                .iter()
                .enumerate()
                .any(|(grant_index, grant)| pool.authority_grants[..grant_index].contains(grant))
        {
            return Err(indexed(
                PlanDiagnosticCode::InvalidDescriptor,
                PlanCollection::InstancePools,
                index,
            ));
        }
        if plan.instance_pools[..index]
            .iter()
            .any(|prior| prior.instance == pool.instance)
        {
            return Err(indexed(
                PlanDiagnosticCode::DuplicateIdentity,
                PlanCollection::InstancePools,
                index,
            ));
        }
        allocated = allocated
            .checked_add(pool.worst_case_budget)
            .ok_or_else(|| {
                indexed(
                    PlanDiagnosticCode::BudgetExceeded,
                    PlanCollection::InstancePools,
                    index,
                )
            })?;
    }

    if !allocated.fits_within(plan.budget) {
        return Err(error(
            PlanDiagnosticCode::BudgetExceeded,
            PlanCollection::Header,
            None,
        ));
    }

    let identity = plan.semantic_hash(identity_scratch).map_err(|failure| {
        error(
            match failure {
                PlanIdentityError::ScratchTooSmall => PlanDiagnosticCode::ScratchTooSmall,
                PlanIdentityError::FactCountOverflow | PlanIdentityError::Canonical(_) => {
                    PlanDiagnosticCode::InvalidDescriptor
                }
            },
            PlanCollection::Header,
            None,
        )
    })?;
    if identity != plan.identity {
        return Err(error(
            PlanDiagnosticCode::IdentityMismatch,
            PlanCollection::Header,
            None,
        ));
    }
    Ok(())
}

fn valid_id(id: Id<'_>) -> bool {
    Id::new(id.as_str()).is_ok()
}

fn valid_path(path: InstancePath<'_>) -> bool {
    InstancePath::new(path.as_str()).is_ok()
}

fn event_stream_allocation_valid(stream: PlanEventStream<'_>) -> bool {
    let storage_valid = match stream.contract.retention {
        RetentionPolicy::Ephemeral => {
            stream.allocation.memory_bytes
                >= stream
                    .contract
                    .subscriber_coupling
                    .flow()
                    .capacity
                    .max_queued_bytes()
        }
        RetentionPolicy::Ring { maximum_bytes, .. }
        | RetentionPolicy::CheckpointAssociated { maximum_bytes, .. } => {
            stream.allocation.memory_bytes >= maximum_bytes
        }
        RetentionPolicy::DurableAppend { maximum_bytes, .. } => {
            stream.allocation.storage_bytes >= maximum_bytes && stream.allocation.timers > 0
        }
    };
    storage_valid
        && (!stream.contract.terminal_evidence_required || stream.allocation.evidence_bytes > 0)
}

fn job_allocation_valid(job: PlanJob<'_>) -> bool {
    let timers_valid = job.allocation.timers >= 2;
    match job.contract.checkpoint_provider {
        None => {
            timers_valid && job.allocation.checkpoints == 0 && job.allocation.storage_bytes == 0
        }
        Some(_) => {
            let retained = job
                .contract
                .maximum_checkpoint_bytes
                .checked_mul(u64::from(job.contract.maximum_checkpoints));
            timers_valid
                && retained.is_some_and(|bytes| job.allocation.storage_bytes >= bytes)
                && job.allocation.memory_bytes >= job.contract.maximum_checkpoint_bytes
                && job.allocation.checkpoints >= job.contract.maximum_checkpoints
        }
    }
}

fn valid_pin(pin: PinnedDescriptor<'_>) -> bool {
    valid_id(pin.id) && pin.schema_version > 0
}

fn valid_port(port: ResolvedPlanPort<'_>) -> bool {
    valid_path(port.node)
        && valid_id(port.port)
        && valid_id(port.value_type.contract_id)
        && port.value_type.schema_version > 0
}

fn has_duplicate_paths(paths: &[InstancePath<'_>]) -> bool {
    paths
        .iter()
        .enumerate()
        .any(|(index, path)| paths[..index].contains(path))
}

const fn error(
    code: PlanDiagnosticCode,
    collection: PlanCollection,
    subject_index: Option<u16>,
) -> PlanValidationError {
    PlanValidationError {
        code,
        collection,
        subject_index,
    }
}

fn indexed(
    code: PlanDiagnosticCode,
    collection: PlanCollection,
    index: usize,
) -> PlanValidationError {
    error(code, collection, u16::try_from(index).ok())
}

fn descriptor_hash(
    kind: Id<'_>,
    fields: &[MapField<'_>],
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    CanonicalDescriptor {
        kind,
        schema_version: 1,
        body: CanonicalValue::Map(fields),
    }
    .semantic_hash()
}

fn hash_host_observation(
    value: PlanHostObservation<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/plan-host-observation"),
        &[
            semantic("id", CanonicalValue::Identifier(value.id)),
            semantic("host", CanonicalValue::Identifier(value.host)),
            semantic(
                "semantic_hash",
                CanonicalValue::Bytes(value.semantic_hash.as_bytes()),
            ),
            semantic("time_basis", CanonicalValue::Identifier(value.time_basis)),
            semantic(
                "observed_at_tick",
                CanonicalValue::Integer(i128::from(value.observed_at_tick)),
            ),
            semantic(
                "valid_until_tick",
                CanonicalValue::Integer(i128::from(value.valid_until_tick)),
            ),
        ],
    )
}

fn hash_resource_binding(
    value: PlanResourceBinding<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/plan-resource-binding"),
        &[
            semantic("id", CanonicalValue::Identifier(value.id)),
            semantic("node", CanonicalValue::Text(value.node.as_str())),
            semantic(
                "resource_kind",
                CanonicalValue::Identifier(value.resource.kind),
            ),
            semantic("resource_id", CanonicalValue::Identifier(value.resource.id)),
            semantic(
                "host_observation",
                CanonicalValue::Identifier(value.host_observation),
            ),
        ],
    )
}

fn hash_artifact(value: PlanArtifact<'_>) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/plan-artifact"),
        &[
            semantic("id", CanonicalValue::Identifier(value.id)),
            semantic("digest", CanonicalValue::Bytes(value.digest.as_bytes())),
        ],
    )
}

fn hash_node(value: ResolvedPlanNode<'_>) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let budget = budget_fields(value.allocation);
    descriptor_hash(
        Id("conduit/plan-node"),
        &[
            semantic("instance", CanonicalValue::Text(value.instance.as_str())),
            semantic("contract_id", CanonicalValue::Identifier(value.contract.id)),
            semantic(
                "contract_version",
                CanonicalValue::Integer(i128::from(value.contract.schema_version)),
            ),
            semantic(
                "contract_hash",
                CanonicalValue::Bytes(value.contract.semantic_hash.as_bytes()),
            ),
            semantic(
                "implementation_id",
                CanonicalValue::Identifier(value.implementation.id),
            ),
            semantic(
                "implementation_version",
                CanonicalValue::Integer(i128::from(value.implementation.schema_version)),
            ),
            semantic(
                "implementation_hash",
                CanonicalValue::Bytes(value.implementation.semantic_hash.as_bytes()),
            ),
            semantic(
                "lifecycle_policy_id",
                CanonicalValue::Identifier(value.lifecycle_policy.id),
            ),
            semantic(
                "lifecycle_policy_version",
                CanonicalValue::Integer(i128::from(value.lifecycle_policy.schema_version)),
            ),
            semantic(
                "lifecycle_policy_hash",
                CanonicalValue::Bytes(value.lifecycle_policy.semantic_hash.as_bytes()),
            ),
            semantic("artifact", CanonicalValue::Identifier(value.artifact)),
            semantic(
                "host_observation",
                CanonicalValue::Identifier(value.host_observation),
            ),
            semantic("host", CanonicalValue::Identifier(value.host)),
            semantic("allocation", CanonicalValue::Map(&budget)),
        ],
    )
}

fn hash_node_execution_profile(
    node: InstancePath<'_>,
    profile_hash: SemanticHash,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/plan-node-execution-profile"),
        &[
            semantic("node", CanonicalValue::Text(node.as_str())),
            semantic(
                "execution_profile_hash",
                CanonicalValue::Bytes(profile_hash.as_bytes()),
            ),
        ],
    )
}

fn hash_node_resource(
    node: InstancePath<'_>,
    resource: Id<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/plan-node-resource"),
        &[
            semantic("node", CanonicalValue::Text(node.as_str())),
            semantic("resource", CanonicalValue::Identifier(resource)),
        ],
    )
}

fn hash_node_effect(
    node: InstancePath<'_>,
    effect_hash: SemanticHash,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/plan-node-effect"),
        &[
            semantic("node", CanonicalValue::Text(node.as_str())),
            semantic("effect_hash", CanonicalValue::Bytes(effect_hash.as_bytes())),
        ],
    )
}

fn hash_cord(value: ResolvedPlanCord<'_>) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let from = port_fields(&value.from);
    let to = port_fields(&value.to);
    let flow = flow_fields(value.flow);
    descriptor_hash(
        Id("conduit/plan-cord"),
        &[
            semantic("id", CanonicalValue::Identifier(value.id)),
            semantic("from", CanonicalValue::Map(&from)),
            semantic("to", CanonicalValue::Map(&to)),
            semantic("flow", CanonicalValue::Map(&flow)),
            semantic(
                "queue_memory_bytes",
                CanonicalValue::Integer(i128::from(value.queue_memory_bytes)),
            ),
        ],
    )
}

fn hash_fanout(value: PlanFanOut<'_>) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let producer = port_fields(&value.producer);
    let copy = match value.duplication {
        DuplicationRule::SharedHandle => None,
        DuplicationRule::Copy(pin) => Some(pin),
    };
    let copy_hash = copy.map(|pin| pin.semantic_hash);
    descriptor_hash(
        Id("conduit/plan-fanout-v1"),
        &[
            semantic("id", CanonicalValue::Identifier(value.id)),
            semantic("producer", CanonicalValue::Map(&producer)),
            semantic("mode", CanonicalValue::Identifier(Id(value.mode.as_str()))),
            semantic(
                "duplicator",
                value.duplicator.map_or(CanonicalValue::Null, |path| {
                    CanonicalValue::Text(path.as_str())
                }),
            ),
            semantic(
                "duplicator_input",
                value
                    .duplicator_input
                    .map_or(CanonicalValue::Null, CanonicalValue::Identifier),
            ),
            semantic(
                "duplication",
                CanonicalValue::Identifier(Id(if copy.is_some() {
                    "copy"
                } else {
                    "shared-handle"
                })),
            ),
            semantic(
                "copy_id",
                copy.map_or(CanonicalValue::Null, |pin| {
                    CanonicalValue::Identifier(pin.id)
                }),
            ),
            semantic(
                "copy_version",
                copy.map_or(CanonicalValue::Null, |pin| {
                    CanonicalValue::Integer(i128::from(pin.schema_version))
                }),
            ),
            semantic(
                "copy_hash",
                copy_hash.as_ref().map_or(CanonicalValue::Null, |hash| {
                    CanonicalValue::Bytes(hash.as_bytes())
                }),
            ),
        ],
    )
}

fn hash_fanout_branch(
    fanout: Id<'_>,
    cord: Id<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/plan-fanout-branch-v1"),
        &[
            semantic("fanout", CanonicalValue::Identifier(fanout)),
            semantic("cord", CanonicalValue::Identifier(cord)),
        ],
    )
}

fn hash_merge(value: PlanMerge<'_>) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let (starvation, timestamp, lateness, late) = match value.ordering {
        MergeOrdering::Arrival | MergeOrdering::RoundRobin => (0, None, 0, None),
        MergeOrdering::Priority { starvation_turns } => (starvation_turns, None, 0, None),
        MergeOrdering::EventTime {
            timestamp_type,
            maximum_lateness_ticks,
            late_values,
        } => (
            0,
            Some(timestamp_type),
            maximum_lateness_ticks,
            Some(late_values),
        ),
    };
    let timestamp_hash = timestamp.map(|value| value.semantic_hash);
    descriptor_hash(
        Id("conduit/plan-merge-v1"),
        &[
            semantic("id", CanonicalValue::Identifier(value.id)),
            semantic("node", CanonicalValue::Text(value.node.as_str())),
            semantic(
                "ordering",
                CanonicalValue::Identifier(Id(value.ordering.as_str())),
            ),
            semantic(
                "starvation_turns",
                CanonicalValue::Integer(i128::from(starvation)),
            ),
            semantic(
                "timestamp_type_id",
                timestamp.map_or(CanonicalValue::Null, |value| {
                    CanonicalValue::Identifier(value.contract_id)
                }),
            ),
            semantic(
                "timestamp_type_version",
                timestamp.map_or(CanonicalValue::Null, |value| {
                    CanonicalValue::Integer(i128::from(value.schema_version))
                }),
            ),
            semantic(
                "timestamp_type_hash",
                timestamp_hash
                    .as_ref()
                    .map_or(CanonicalValue::Null, |hash| {
                        CanonicalValue::Bytes(hash.as_bytes())
                    }),
            ),
            semantic(
                "maximum_lateness_ticks",
                CanonicalValue::Integer(i128::from(lateness)),
            ),
            semantic(
                "late_values",
                late.map_or(CanonicalValue::Null, |value| {
                    CanonicalValue::Identifier(Id(value.as_str()))
                }),
            ),
            semantic(
                "terminal",
                CanonicalValue::Identifier(Id(value.terminal.as_str())),
            ),
        ],
    )
}

fn hash_merge_input(
    merge: Id<'_>,
    input: PlanMergeInput<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/plan-merge-input-v1"),
        &[
            semantic("merge", CanonicalValue::Identifier(merge)),
            semantic("cord", CanonicalValue::Identifier(input.cord)),
            semantic(
                "ordinal",
                CanonicalValue::Integer(i128::from(input.ordinal)),
            ),
            semantic(
                "priority",
                CanonicalValue::Integer(i128::from(input.priority)),
            ),
        ],
    )
}

fn hash_event_stream(
    value: PlanEventStream<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let (retention, events, bytes, checkpoints, flush_ticks) = match value.contract.retention {
        RetentionPolicy::Ephemeral => ("ephemeral", 0, 0, 0, 0),
        RetentionPolicy::Ring {
            maximum_events,
            maximum_bytes,
        } => ("ring", u64::from(maximum_events), maximum_bytes, 0, 0),
        RetentionPolicy::CheckpointAssociated {
            maximum_events,
            maximum_bytes,
            maximum_checkpoints,
        } => (
            "checkpoint-associated",
            u64::from(maximum_events),
            maximum_bytes,
            u64::from(maximum_checkpoints),
            0,
        ),
        RetentionPolicy::DurableAppend {
            maximum_events,
            maximum_bytes,
            flush_ticks,
        } => (
            "durable-append",
            maximum_events,
            maximum_bytes,
            0,
            flush_ticks,
        ),
    };
    let (coupling, flow) = match value.contract.subscriber_coupling {
        SubscriberCoupling::Coupled(flow) => ("coupled", flow),
        SubscriberCoupling::Isolated(flow) => ("isolated", flow),
    };
    let flow_hash = descriptor_hash(Id("conduit/event-subscriber-flow-v1"), &flow_fields(flow))?;
    let allocation_hash = descriptor_hash(
        Id("conduit/event-stream-allocation-v1"),
        &budget_fields(value.allocation),
    )?;
    let capabilities = value.provider_capabilities;
    descriptor_hash(
        Id("conduit/plan-event-stream-v1"),
        &[
            semantic("id", CanonicalValue::Identifier(value.contract.id)),
            semantic("publisher", CanonicalValue::Text(value.publisher.as_str())),
            semantic(
                "event_class",
                CanonicalValue::Identifier(Id(value.contract.event_class.as_str())),
            ),
            semantic(
                "payload_type_id",
                CanonicalValue::Identifier(value.contract.payload_type.contract_id),
            ),
            semantic(
                "payload_type_version",
                CanonicalValue::Integer(i128::from(value.contract.payload_type.schema_version)),
            ),
            semantic(
                "payload_type_hash",
                CanonicalValue::Bytes(value.contract.payload_type.semantic_hash.as_bytes()),
            ),
            semantic("retention", CanonicalValue::Identifier(Id(retention))),
            semantic(
                "maximum_events",
                CanonicalValue::Integer(i128::from(events)),
            ),
            semantic("maximum_bytes", CanonicalValue::Integer(i128::from(bytes))),
            semantic(
                "maximum_checkpoints",
                CanonicalValue::Integer(i128::from(checkpoints)),
            ),
            semantic(
                "flush_ticks",
                CanonicalValue::Integer(i128::from(flush_ticks)),
            ),
            semantic("coupling", CanonicalValue::Identifier(Id(coupling))),
            semantic(
                "subscriber_flow_hash",
                CanonicalValue::Bytes(flow_hash.as_bytes()),
            ),
            semantic(
                "delivery",
                CanonicalValue::Identifier(Id(value.contract.delivery.as_str())),
            ),
            semantic(
                "maximum_publishers",
                CanonicalValue::Integer(i128::from(value.contract.maximum_publishers)),
            ),
            semantic(
                "maximum_subscribers",
                CanonicalValue::Integer(i128::from(value.contract.maximum_subscribers)),
            ),
            semantic(
                "maximum_pending_operations",
                CanonicalValue::Integer(i128::from(value.contract.maximum_pending_operations)),
            ),
            semantic(
                "maximum_projection_bytes",
                CanonicalValue::Integer(i128::from(value.contract.maximum_projection_bytes)),
            ),
            semantic(
                "provider_id",
                CanonicalValue::Identifier(value.contract.provider.id),
            ),
            semantic(
                "provider_version",
                CanonicalValue::Integer(i128::from(value.contract.provider.schema_version)),
            ),
            semantic(
                "provider_hash",
                CanonicalValue::Bytes(value.contract.provider.semantic_hash.as_bytes()),
            ),
            semantic(
                "recording_authority",
                value
                    .contract
                    .recording_authority
                    .map_or(CanonicalValue::Null, CanonicalValue::Identifier),
            ),
            semantic(
                "sensitivity",
                CanonicalValue::Identifier(Id(value.contract.sensitivity.as_str())),
            ),
            semantic(
                "terminal_evidence_required",
                CanonicalValue::Boolean(value.contract.terminal_evidence_required),
            ),
            semantic(
                "provider_ephemeral",
                CanonicalValue::Boolean(capabilities.ephemeral),
            ),
            semantic(
                "provider_retained",
                CanonicalValue::Boolean(capabilities.retained),
            ),
            semantic(
                "provider_durable",
                CanonicalValue::Boolean(capabilities.durable),
            ),
            semantic(
                "provider_checkpoint_cursor",
                CanonicalValue::Boolean(capabilities.checkpoint_cursor),
            ),
            semantic(
                "provider_integrity",
                CanonicalValue::Boolean(capabilities.integrity),
            ),
            semantic(
                "provider_redaction",
                CanonicalValue::Boolean(capabilities.redaction),
            ),
            semantic(
                "provider_maximum_events",
                CanonicalValue::Integer(i128::from(capabilities.maximum_events)),
            ),
            semantic(
                "provider_maximum_bytes",
                CanonicalValue::Integer(i128::from(capabilities.maximum_bytes)),
            ),
            semantic(
                "provider_maximum_subscribers",
                CanonicalValue::Integer(i128::from(capabilities.maximum_subscribers)),
            ),
            semantic(
                "provider_maximum_pending_operations",
                CanonicalValue::Integer(i128::from(capabilities.maximum_pending_operations)),
            ),
            semantic(
                "allocation_hash",
                CanonicalValue::Bytes(allocation_hash.as_bytes()),
            ),
        ],
    )
}

fn hash_runtime_evidence_policy(
    value: RuntimeEvidencePolicy<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/runtime-evidence-policy-v1"),
        &[
            semantic(
                "schema_version",
                CanonicalValue::Integer(i128::from(value.schema_version)),
            ),
            semantic("mode", CanonicalValue::Identifier(Id(value.mode.as_str()))),
            semantic(
                "stream",
                value
                    .stream
                    .map_or(CanonicalValue::Null, CanonicalValue::Identifier),
            ),
            semantic(
                "maximum_events",
                CanonicalValue::Integer(i128::from(value.maximum_events)),
            ),
            semantic(
                "maximum_bytes",
                CanonicalValue::Integer(i128::from(value.maximum_bytes)),
            ),
            semantic(
                "required_reserve_events",
                CanonicalValue::Integer(i128::from(value.required_reserve_events)),
            ),
            semantic(
                "required_reserve_bytes",
                CanonicalValue::Integer(i128::from(value.required_reserve_bytes)),
            ),
            semantic(
                "telemetry_period",
                CanonicalValue::Integer(i128::from(value.telemetry_period)),
            ),
            semantic(
                "telemetry_offset",
                CanonicalValue::Integer(i128::from(value.telemetry_offset)),
            ),
            semantic(
                "gap_summary_bytes",
                CanonicalValue::Integer(i128::from(value.gap_summary_bytes)),
            ),
        ],
    )
}

fn hash_job(value: PlanJob<'_>) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let contract_hash = value.contract.semantic_hash()?;
    let allocation_hash = descriptor_hash(
        Id("conduit/job-allocation-v1"),
        &budget_fields(value.allocation),
    )?;
    let capabilities = value.checkpoint_provider_capabilities;
    descriptor_hash(
        Id("conduit/plan-job-v1"),
        &[
            semantic("owner", CanonicalValue::Text(value.owner.as_str())),
            semantic(
                "contract_hash",
                CanonicalValue::Bytes(contract_hash.as_bytes()),
            ),
            semantic(
                "checkpoint_provider_durable",
                capabilities.map_or(CanonicalValue::Null, |capability| {
                    CanonicalValue::Boolean(capability.durable)
                }),
            ),
            semantic(
                "checkpoint_provider_integrity",
                capabilities.map_or(CanonicalValue::Null, |capability| {
                    CanonicalValue::Boolean(capability.integrity)
                }),
            ),
            semantic(
                "checkpoint_provider_migration",
                capabilities.map_or(CanonicalValue::Null, |capability| {
                    CanonicalValue::Boolean(capability.migration)
                }),
            ),
            semantic(
                "checkpoint_provider_maximum_checkpoints",
                capabilities.map_or(CanonicalValue::Null, |capability| {
                    CanonicalValue::Integer(i128::from(capability.maximum_checkpoints))
                }),
            ),
            semantic(
                "checkpoint_provider_maximum_checkpoint_bytes",
                capabilities.map_or(CanonicalValue::Null, |capability| {
                    CanonicalValue::Integer(i128::from(capability.maximum_checkpoint_bytes))
                }),
            ),
            semantic(
                "checkpoint_provider_maximum_state_references",
                capabilities.map_or(CanonicalValue::Null, |capability| {
                    CanonicalValue::Integer(i128::from(capability.maximum_state_references))
                }),
            ),
            semantic(
                "checkpoint_provider_maximum_pending_operations",
                capabilities.map_or(CanonicalValue::Null, |capability| {
                    CanonicalValue::Integer(i128::from(capability.maximum_pending_operations))
                }),
            ),
            semantic(
                "allocation_hash",
                CanonicalValue::Bytes(allocation_hash.as_bytes()),
            ),
        ],
    )
}

fn hash_satisfaction_proof(
    value: PlanSatisfactionProof<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let (subject_kind, subject, host_observation) = match value.subject {
        PlanSatisfactionSubject::Cord(id) => ("cord", id.as_str(), None),
        PlanSatisfactionSubject::Implementation(instance) => {
            ("implementation", instance.as_str(), None)
        }
        PlanSatisfactionSubject::HostCapability {
            node,
            host_observation,
        } => ("host-capability", node.as_str(), Some(host_observation)),
    };
    descriptor_hash(
        Id("conduit/plan-satisfaction-proof"),
        &[
            semantic("subject_kind", CanonicalValue::Identifier(Id(subject_kind))),
            semantic("subject", CanonicalValue::Text(subject)),
            semantic(
                "host_observation",
                host_observation.map_or(CanonicalValue::Null, CanonicalValue::Identifier),
            ),
            semantic(
                "proof_identity",
                CanonicalValue::Bytes(value.proof.identity.as_bytes()),
            ),
        ],
    )
}

fn hash_authority(value: PlanAuthority<'_>) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/plan-authority"),
        &[
            semantic("node", CanonicalValue::Text(value.node.as_str())),
            semantic(
                "effect_hash",
                CanonicalValue::Bytes(value.effect_hash.as_bytes()),
            ),
            semantic(
                "grant_hash",
                CanonicalValue::Bytes(value.grant_hash.as_bytes()),
            ),
            semantic(
                "capability_id",
                CanonicalValue::Identifier(value.capability.id),
            ),
            semantic(
                "capability_action",
                CanonicalValue::Identifier(value.capability.action),
            ),
            semantic(
                "resource_kind",
                CanonicalValue::Identifier(value.capability.resource.kind),
            ),
            semantic(
                "resource_id",
                CanonicalValue::Identifier(value.capability.resource.id),
            ),
            semantic(
                "capability_host",
                CanonicalValue::Identifier(value.capability.host),
            ),
            semantic(
                "capability_time_basis",
                CanonicalValue::Identifier(value.capability.time_basis),
            ),
            semantic(
                "capability_observed_at",
                CanonicalValue::Integer(i128::from(value.capability.observed_at_tick)),
            ),
            semantic(
                "capability_valid_until",
                CanonicalValue::Integer(i128::from(value.capability.valid_until_tick)),
            ),
            semantic(
                "binding_capability",
                CanonicalValue::Identifier(value.binding.capability_id),
            ),
            semantic(
                "binding_grant",
                CanonicalValue::Identifier(value.binding.grant_id),
            ),
            semantic(
                "binding_audit",
                CanonicalValue::Identifier(value.binding.audit_id),
            ),
            semantic(
                "binding_validated_at",
                CanonicalValue::Integer(i128::from(value.binding.validated_at_tick)),
            ),
            semantic(
                "binding_check_at_use",
                CanonicalValue::Boolean(value.binding.check_at_use),
            ),
        ],
    )
}

fn hash_composite(
    value: PlanCompositeMapping<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/plan-composite"),
        &[
            semantic("instance", CanonicalValue::Text(value.instance.as_str())),
            semantic(
                "definition_hash",
                CanonicalValue::Bytes(value.definition_hash.as_bytes()),
            ),
        ],
    )
}

fn hash_composite_member(
    composite: InstancePath<'_>,
    member: InstancePath<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/plan-composite-member"),
        &[
            semantic("composite", CanonicalValue::Text(composite.as_str())),
            semantic("member", CanonicalValue::Text(member.as_str())),
        ],
    )
}

fn hash_export(
    composite: InstancePath<'_>,
    value: PlanExportBinding<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/plan-export"),
        &[
            semantic("composite", CanonicalValue::Text(composite.as_str())),
            semantic(
                "boundary_port",
                CanonicalValue::Identifier(value.boundary_port),
            ),
            semantic("member", CanonicalValue::Text(value.member.as_str())),
            semantic("member_port", CanonicalValue::Identifier(value.member_port)),
            semantic(
                "direction",
                CanonicalValue::Identifier(Id(value.direction.as_str())),
            ),
        ],
    )
}

fn hash_port_group(
    plan_schema_version: u32,
    value: PlanPortGroup<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    if plan_schema_version == EXECUTION_PLAN_SCHEMA_VERSION_V1 {
        return descriptor_hash(
            Id("conduit/plan-port-group"),
            &[
                semantic("instance", CanonicalValue::Text(value.instance.as_str())),
                semantic(
                    "template_hash",
                    CanonicalValue::Bytes(value.template_hash.as_bytes()),
                ),
            ],
        );
    }
    descriptor_hash(
        Id("conduit/plan-port-group-v2"),
        &[
            semantic("instance", CanonicalValue::Text(value.instance.as_str())),
            semantic(
                "template_hash",
                CanonicalValue::Bytes(value.template_hash.as_bytes()),
            ),
            semantic(
                "maximum",
                CanonicalValue::Integer(i128::from(value.maximum)),
            ),
            semantic(
                "direction",
                CanonicalValue::Identifier(Id(value.direction.as_str())),
            ),
        ],
    )
}

fn hash_port_group_member(
    group: InstancePath<'_>,
    value: PlanPortGroupMember<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/plan-port-group-member"),
        &[
            semantic("group", CanonicalValue::Text(group.as_str())),
            semantic("id", CanonicalValue::Identifier(value.id)),
            semantic(
                "ordinal",
                CanonicalValue::Integer(i128::from(value.ordinal)),
            ),
            semantic(
                "port_contract_hash",
                CanonicalValue::Bytes(value.port_contract_hash.as_bytes()),
            ),
        ],
    )
}

fn hash_instance_pool(
    value: PlanInstancePool<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let per_instance = budget_fields(value.per_instance_budget);
    let worst_case = budget_fields(value.worst_case_budget);
    descriptor_hash(
        Id("conduit/plan-instance-pool"),
        &[
            semantic("instance", CanonicalValue::Text(value.instance.as_str())),
            semantic(
                "template_hash",
                CanonicalValue::Bytes(value.template_hash.as_bytes()),
            ),
            semantic(
                "derived_identity_hash",
                CanonicalValue::Bytes(value.derived_identity_hash.as_bytes()),
            ),
            semantic(
                "maximum_live",
                CanonicalValue::Integer(i128::from(value.maximum_live)),
            ),
            semantic(
                "maximum_queued",
                CanonicalValue::Integer(i128::from(value.maximum_queued)),
            ),
            semantic(
                "admission_policy_id",
                CanonicalValue::Identifier(value.admission_policy.id),
            ),
            semantic(
                "admission_policy_version",
                CanonicalValue::Integer(i128::from(value.admission_policy.schema_version)),
            ),
            semantic(
                "admission_policy_hash",
                CanonicalValue::Bytes(value.admission_policy.semantic_hash.as_bytes()),
            ),
            semantic(
                "supervision_policy_id",
                CanonicalValue::Identifier(value.supervision_policy.id),
            ),
            semantic(
                "supervision_policy_version",
                CanonicalValue::Integer(i128::from(value.supervision_policy.schema_version)),
            ),
            semantic(
                "supervision_policy_hash",
                CanonicalValue::Bytes(value.supervision_policy.semantic_hash.as_bytes()),
            ),
            semantic("per_instance_budget", CanonicalValue::Map(&per_instance)),
            semantic(
                "maximum_instance_ticks",
                CanonicalValue::Integer(i128::from(value.maximum_instance_ticks)),
            ),
            semantic(
                "implementation_set_hash",
                CanonicalValue::Bytes(value.implementation_set_hash.as_bytes()),
            ),
            semantic(
                "correlation_slots",
                CanonicalValue::Integer(i128::from(value.correlation_slots)),
            ),
            semantic("worst_case_budget", CanonicalValue::Map(&worst_case)),
            semantic(
                "child_nodes",
                CanonicalValue::Integer(i128::from(value.child_nodes)),
            ),
            semantic(
                "child_cords",
                CanonicalValue::Integer(i128::from(value.child_cords)),
            ),
        ],
    )
}

fn hash_pool_authority(
    pool: InstancePath<'_>,
    grant: Id<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/plan-pool-authority"),
        &[
            semantic("pool", CanonicalValue::Text(pool.as_str())),
            semantic("grant", CanonicalValue::Identifier(grant)),
        ],
    )
}

fn hash_unresolved(
    value: UnresolvedPlanConstraint<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/plan-unresolved"),
        &[
            semantic("id", CanonicalValue::Identifier(value.id)),
            semantic("requester", CanonicalValue::Text(value.requester.as_str())),
            semantic("kind", CanonicalValue::Identifier(Id(value.kind.as_str()))),
        ],
    )
}

fn budget_fields(value: PlanResourceBudget) -> [MapField<'static>; 7] {
    [
        semantic(
            "memory_bytes",
            CanonicalValue::Integer(i128::from(value.memory_bytes)),
        ),
        semantic(
            "storage_bytes",
            CanonicalValue::Integer(i128::from(value.storage_bytes)),
        ),
        semantic(
            "cpu_units",
            CanonicalValue::Integer(i128::from(value.cpu_units)),
        ),
        semantic("timers", CanonicalValue::Integer(i128::from(value.timers))),
        semantic(
            "transports",
            CanonicalValue::Integer(i128::from(value.transports)),
        ),
        semantic(
            "checkpoints",
            CanonicalValue::Integer(i128::from(value.checkpoints)),
        ),
        semantic(
            "evidence_bytes",
            CanonicalValue::Integer(i128::from(value.evidence_bytes)),
        ),
    ]
}

fn port_fields<'a>(value: &'a ResolvedPlanPort<'a>) -> [MapField<'a>; 7] {
    [
        semantic("node", CanonicalValue::Text(value.node.as_str())),
        semantic("port", CanonicalValue::Identifier(value.port)),
        semantic(
            "direction",
            CanonicalValue::Identifier(Id(value.direction.as_str())),
        ),
        semantic(
            "port_contract_hash",
            CanonicalValue::Bytes(value.port_contract_hash.as_bytes()),
        ),
        semantic(
            "type_id",
            CanonicalValue::Identifier(value.value_type.contract_id),
        ),
        semantic(
            "type_version",
            CanonicalValue::Integer(i128::from(value.value_type.schema_version)),
        ),
        semantic(
            "type_hash",
            CanonicalValue::Bytes(value.value_type.semantic_hash.as_bytes()),
        ),
    ]
}

fn flow_fields(value: FlowPolicy<'_>) -> [MapField<'_>; 10] {
    let (parameter, sample_every, sample_offset) = match value.pressure {
        Pressure::Block(_) => ("fifo", 0, 0),
        Pressure::Coalesce { relation } => (relation.as_str(), 0, 0),
        Pressure::Sample(schedule) => ("", schedule.every(), schedule.offset()),
        Pressure::Reject | Pressure::DropDisposable | Pressure::Disconnect | Pressure::Fail => {
            ("", 0, 0)
        }
    };
    [
        semantic(
            "capacity_items",
            CanonicalValue::Integer(i128::from(value.capacity.items())),
        ),
        semantic(
            "max_value_bytes",
            CanonicalValue::Integer(i128::from(value.capacity.max_value_bytes())),
        ),
        semantic(
            "max_queued_bytes",
            CanonicalValue::Integer(i128::from(value.capacity.max_queued_bytes())),
        ),
        semantic(
            "pressure",
            CanonicalValue::Identifier(Id(value.pressure.as_str())),
        ),
        semantic("pressure_parameter", CanonicalValue::Text(parameter)),
        semantic(
            "sample_every",
            CanonicalValue::Integer(i128::from(sample_every)),
        ),
        semantic(
            "sample_offset",
            CanonicalValue::Integer(i128::from(sample_offset)),
        ),
        semantic(
            "watermark_low",
            CanonicalValue::Integer(i128::from(value.watermarks.low_items())),
        ),
        semantic(
            "watermark_high",
            CanonicalValue::Integer(i128::from(value.watermarks.high_items())),
        ),
        semantic(
            "permits_loss",
            CanonicalValue::Boolean(value.pressure.permits_loss()),
        ),
    ]
}

const fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}
