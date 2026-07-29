//! Hosted exact-plan compilation over explicit immutable inputs.
//!
//! This crate performs no discovery, fetch, provisioning, secret resolution,
//! grant acquisition, implementation loading, or execution.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use bumpalo::Bump;
use conduit_core::{
    ArtifactDigest, ArtifactManifest, ArtifactProvenance, AuthorityConstraintRef, AuthorityGrant,
    AuthorityScope, AuthorityTime, BlockingFairness, BoundednessProfile, CancellationGuarantee,
    DelegationPolicy, Direction, EXECUTION_PLAN_SCHEMA_VERSION_V3, EffectRequirement,
    ExecutionLimits, ExecutionPlan, ExecutionProfile, ExecutorKind, FlowCapacity, FlowPolicy,
    FlowWatermarks, GrantStatus, HandleDisposition, HostCapability, Id, ImplementationManifest,
    InstancePath, ManifestArtifactRef, ManifestEntrypoint, MemoryAccounting, MemoryCategory,
    MemoryClaim, ObservedGrant, OwnershipModel, PassportStatus, PassportStatusObservation,
    PinnedDescriptor, PlanArtifact, PlanAuthority, PlanCompositeMapping, PlanExportBinding,
    PlanHostObservation, PlanInstancePool, PlanPortGroup, PlanPortGroupMember, PlanResourceBinding,
    PlanResourceBudget, PlanValidationContext, Pressure, ReplacementSupport, ReportCapability,
    ReportMembership, ReportResource, ReportTopology, ResolvedAuthorityBinding, ResolvedPlanCord,
    ResolvedPlanNode, ResolvedPlanPort, ResourceRef, ResourceSelector, SampleSchedule,
    SemanticHash, StopPolicy, TypeContractRef, ValueRepresentation, resolve_authority,
};
use conduit_panel::{LoadedModule, ModuleGraph, ModuleLoader, SourcePressure};
use conduit_runtime::{
    CandidateAuthority, CapabilityPredicate, ExactTopologyView, HostResolverPolicy,
    LiteralValidationError, OwnedNodeSchema, OwnedPortReference, OwnedSemanticValue,
    OwnedTypeReference, PlacementCandidate, PlacementRequest, Registry, ResolverTiePolicy,
    ResourcePredicate, SourceContractCatalog, TopologyPredicate, lower_source_v2,
    resolve_host_placement, seal_resolved_execution_plan, validate_hosted_execution_plan,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

pub const COMPILE_INPUT_SCHEMA: &str = "conduit.compile-input/v1";
pub const COMPILE_INPUT_SCHEMA_VERSION: u16 = 1;
pub const PLAN_DOCUMENT_SCHEMA: &str = "conduit.execution-plan/v3";

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
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompileModuleDocument {
    pub canonical_uri: String,
    pub content_hash: String,
    pub source: String,
}

/// Exact finite semantic catalog snapshot used during lowering.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompileCatalogDocument {
    pub identity: String,
    pub nodes: Vec<PinDocument>,
    pub types: Vec<PinDocument>,
    pub ports: Vec<PinDocument>,
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
    pub modules: Vec<CompileModuleDocument>,
    pub catalog: CompileCatalogDocument,
    #[serde(default)]
    pub pool_bindings: Vec<PoolBindingDocument>,
    pub source_semantic_hash: String,
    pub resolver: PinDocument,
    pub resolver_policy_hash: String,
    pub time_basis: String,
    pub current_tick: u64,
    pub plan_budget: BudgetDocument,
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
    modules: &'a [CompileModuleDocument],
    catalog: &'a CompileCatalogDocument,
    pool_bindings: &'a [PoolBindingDocument],
    source_semantic_hash: &'a str,
    resolver: &'a PinDocument,
    resolver_policy_hash: &'a str,
    time_basis: &'a str,
    current_tick: u64,
    plan_budget: BudgetDocument,
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
            candidate.implementation.required_authorities = candidate
                .authorities
                .iter()
                .map(|authority| authority.requirement.clone())
                .collect();
            candidate.implementation.required_effects = candidate
                .authorities
                .iter()
                .map(|authority| authority.effect_hash.clone())
                .collect();
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
            modules: &canonical.modules,
            catalog: &canonical.catalog,
            pool_bindings: &canonical.pool_bindings,
            source_semantic_hash: &canonical.source_semantic_hash,
            resolver: &canonical.resolver,
            resolver_policy_hash: &canonical.resolver_policy_hash,
            time_basis: &canonical.time_basis,
            current_tick: canonical.current_tick,
            plan_budget: canonical.plan_budget,
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
        if self.candidates.is_empty() || self.candidates.len() > 4096 {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        if self.modules.is_empty() || self.modules.len() > 256 {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        if self.pool_bindings.len() > 4096 {
            return Err(CompileError::new(CompileReason::InvalidInput));
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
        }
        let aggregate_module_bytes = self
            .modules
            .iter()
            .try_fold(0_u64, |total, module| {
                total.checked_add(module.source.len() as u64)
            })
            .ok_or_else(|| CompileError::new(CompileReason::InvalidInput))?;
        if aggregate_module_bytes > 32 * 1024 * 1024
            || self.modules.iter().any(|module| {
                module.source.len() > 8 * 1024 * 1024
                    || module.content_hash != content_hash(&module.source)
            })
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
        if self.identity != self.computed_identity()? {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        if policy_hash(self)? != self.resolver_policy_hash {
            return Err(CompileError::new(CompileReason::InvalidInput));
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
}

/// Returns the finite built-in semantic catalog accepted by the reference
/// compiler. The returned identity pins the exact provider snapshot.
pub fn builtin_catalog_document() -> Result<CompileCatalogDocument, CompileError> {
    let registry = Registry::default();
    let mut catalog = CompileCatalogDocument {
        identity: String::new(),
        nodes: ["conduit/literal", "conduit/stdout", "conduit/uppercase"]
            .into_iter()
            .map(|id| {
                let schema = registry
                    .node_schema(id)
                    .ok_or_else(|| CompileError::new(CompileReason::InvalidInput))?;
                Ok(PinDocument {
                    id: id.to_owned(),
                    schema_version: 1,
                    semantic_hash: schema.semantic_hash().to_string(),
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?,
        types: ["conduit/text.utf8"]
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
        ports: ["conduit/input-text", "conduit/output-text"]
            .into_iter()
            .map(|id| {
                let reference = registry
                    .port_contract(id)
                    .ok_or_else(|| CompileError::new(CompileReason::InvalidInput))?;
                Ok(PinDocument {
                    id: id.to_owned(),
                    schema_version: 1,
                    semantic_hash: reference.semantic_hash.to_string(),
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
}

impl SourceContractCatalog for PinnedCatalog<'_> {
    fn node_schema(&self, id: &str) -> Option<OwnedNodeSchema> {
        let pin = self.exact_pin(&self.document.nodes, id)?;
        let schema = self.registry.node_schema(id)?;
        (pin.schema_version == 1 && pin.semantic_hash == schema.semantic_hash().to_string())
            .then_some(schema)
    }

    fn type_reference(&self, id: &str) -> Option<OwnedTypeReference> {
        let pin = self.exact_pin(&self.document.types, id)?;
        let reference = self.registry.type_reference(id)?;
        (pin.schema_version == reference.schema_version
            && pin.semantic_hash == reference.semantic_hash.to_string())
        .then_some(reference)
    }

    fn port_contract(&self, id: &str) -> Option<OwnedPortReference> {
        let pin = self.exact_pin(&self.document.ports, id)?;
        let reference = self.registry.port_contract(id)?;
        (pin.schema_version == 1 && pin.semantic_hash == reference.semantic_hash.to_string())
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
    {
        return Err(CompileError::new(CompileReason::InvalidInput));
    }
    if catalog.identity != catalog_identity(catalog)? {
        return Err(CompileError::new(CompileReason::InvalidInput));
    }
    let registry = Registry::default();
    let mut ids = BTreeSet::new();
    for pin in &catalog.nodes {
        if !ids.insert(pin.id.as_str())
            || pin.schema_version != 1
            || registry
                .node_schema(&pin.id)
                .is_none_or(|schema| schema.semantic_hash().to_string() != pin.semantic_hash)
        {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
    }
    ids.clear();
    for pin in &catalog.types {
        if !ids.insert(pin.id.as_str())
            || registry.type_reference(&pin.id).is_none_or(|reference| {
                reference.schema_version != pin.schema_version
                    || reference.semantic_hash.to_string() != pin.semantic_hash
            })
        {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
    }
    ids.clear();
    for pin in &catalog.ports {
        if !ids.insert(pin.id.as_str())
            || pin.schema_version != 1
            || registry
                .port_contract(&pin.id)
                .is_none_or(|reference| reference.semantic_hash.to_string() != pin.semantic_hash)
        {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
    }
    Ok(())
}

fn catalog_identity(catalog: &CompileCatalogDocument) -> Result<String, CompileError> {
    let mut canonical = catalog.clone();
    canonicalize_catalog(&mut canonical);
    let bytes = serde_json::to_vec(&CatalogIdentityProjection {
        nodes: &canonical.nodes,
        types: &canonical.types,
        ports: &canonical.ports,
    })
    .map_err(|_| CompileError::new(CompileReason::InvalidInput))?;
    Ok(format!("sha256:{}", hex(&Sha256::digest(bytes))))
}

fn canonicalize_catalog(catalog: &mut CompileCatalogDocument) {
    catalog.nodes.sort_by(|left, right| left.id.cmp(&right.id));
    catalog.types.sort_by(|left, right| left.id.cmp(&right.id));
    catalog.ports.sort_by(|left, right| left.id.cmp(&right.id));
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

fn lower_compile_source(
    graph: &ModuleGraph,
    catalog: &CompileCatalogDocument,
) -> Result<conduit_runtime::LoweredSourceV2, CompileError> {
    lower_source_v2(graph, &PinnedCatalog::new(catalog)?)
        .map_err(|_| CompileError::new(CompileReason::LoweringFailed))
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanHostDocument {
    pub id: String,
    pub host: String,
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
    pub host_observations: Vec<PlanHostDocument>,
    pub resources: Vec<PlanResourceDocument>,
    pub artifacts: Vec<PlanArtifactDocument>,
    pub nodes: Vec<PlanNodeDocument>,
    pub cords: Vec<PlanCordDocument>,
    pub authorities: Vec<PlanAuthorityDocument>,
    pub composites: Vec<PlanCompositeDocument>,
    pub port_groups: Vec<PlanPortGroupDocument>,
    pub instance_pools: Vec<PlanInstancePoolDocument>,
    pub unresolved_selectors: Vec<String>,
}

impl ExactPlanDocument {
    pub fn validate(&self) -> Result<(), CompileError> {
        let arena = Bump::new();
        let plan = self.as_plan(&arena)?;
        validate_hosted_execution_plan(
            &plan,
            PlanValidationContext {
                supported_schema_version: EXECUTION_PLAN_SCHEMA_VERSION_V3,
                now: AuthorityTime {
                    basis: Id(&self.time_basis),
                    tick: self.created_at_tick,
                },
            },
        )
        .map_err(|_| CompileError::new(CompileReason::PlanInvalid))
    }

    fn as_plan<'a>(&'a self, arena: &'a Bump) -> Result<ExecutionPlan<'a>, CompileError> {
        if self.schema != PLAN_DOCUMENT_SCHEMA
            || self.schema_version != EXECUTION_PLAN_SCHEMA_VERSION_V3
            || !self.unresolved_selectors.is_empty()
        {
            return Err(CompileError::new(CompileReason::PlanInvalid));
        }
        let hosts = self
            .host_observations
            .iter()
            .map(|host| {
                Ok(PlanHostObservation {
                    id: id(&host.id)?,
                    host: id(&host.host)?,
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
                })
            })
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
                })
            })
            .collect::<Result<Vec<_>, CompileError>>()?;
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
                Ok(PlanInstancePool {
                    instance: instance(&pool.instance)?,
                    template_hash: parse_hash(&pool.template_hash)?,
                    derived_identity_hash: parse_hash(&pool.derived_identity_hash)?,
                    maximum_live: pool.maximum_live,
                    maximum_queued: pool.maximum_queued,
                    admission_policy: pin(&pool.admission_policy)?,
                    supervision_policy: pin(&pool.supervision_policy)?,
                    per_instance_budget: pool.per_instance_budget.into(),
                    authority_grants: arena.alloc_slice_copy(&authority_grants),
                    maximum_instance_ticks: pool.maximum_instance_ticks,
                    implementation_set_hash: parse_hash(&pool.implementation_set_hash)?,
                    correlation_slots: pool.correlation_slots,
                    worst_case_budget: pool.worst_case_budget.into(),
                    child_nodes: pool.child_nodes,
                    child_cords: pool.child_cords,
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
            artifacts: arena.alloc_slice_copy(&artifacts),
            nodes: arena.alloc_slice_copy(&nodes),
            cords: arena.alloc_slice_copy(&cords),
            distributed_cords: &[],
            fanouts: &[],
            merges: &[],
            event_streams: &[],
            runtime_evidence: None,
            jobs: &[],
            satisfaction_proofs: &[],
            authorities: arena.alloc_slice_copy(&authorities),
            composites: arena.alloc_slice_copy(&composites),
            port_groups: arena.alloc_slice_copy(&port_groups),
            instance_pools: arena.alloc_slice_copy(&instance_pools),
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
    let panel = executable_panel(graph)?;
    let registry = Registry::default();
    let resolved = registry
        .resolve(&panel)
        .map_err(|_| CompileError::new(CompileReason::SourceInvalid))?;
    let mut topology = resolved
        .exact_topology()
        .map_err(|_| CompileError::new(CompileReason::SourceInvalid))?;
    topology.source_semantic_hash = lowered.semantic_hash;
    compile_topology(&topology, &lowered, input)
}

fn executable_panel(graph: &ModuleGraph) -> Result<conduit_panel::Panel, CompileError> {
    let modules = graph
        .modules
        .iter()
        .map(|module| (module.canonical_uri.as_str(), module))
        .collect::<BTreeMap<_, _>>();
    let mut definitions = Vec::new();
    for module in &graph.modules {
        for source in &module.panel.definitions {
            let mut definition = source.clone();
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
    let (mut nodes, cords) = match graph.selected_root.as_deref() {
        None => (entry.panel.nodes.clone(), entry.panel.cords.clone()),
        Some(selected) => {
            if let Some(node) = entry.panel.nodes.iter().find(|node| node.id == selected) {
                (vec![node.clone()], Vec::new())
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
                        config: Vec::new(),
                        source_span: root.source_span,
                    }],
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
        definitions,
        nodes,
        cords,
        roots: Vec::new(),
        selected_root: None,
        port_groups: Vec::new(),
        pools: Vec::new(),
    })
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
    if kind.starts_with("module.h") {
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
        let import = module
            .imports
            .iter()
            .find(|import| import.alias == alias)
            .ok_or_else(|| CompileError::new(CompileReason::LoweringFailed))?;
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
    lowered: &conduit_runtime::LoweredSourceV2,
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
                schema_version: 1,
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
            if !required_resources.contains(&authority.capability.resource.id) {
                if resource_bindings
                    .iter()
                    .any(|resource| resource.id == authority.capability.resource.id)
                {
                    return Err(CompileError::new(CompileReason::PlanInvalid));
                }
                resource_bindings.push(PlanResourceBinding {
                    id: authority.capability.resource.id,
                    node: instance_path,
                    resource: authority.capability.resource,
                    host_observation: candidate.report.id,
                });
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
    for member in &lowered.group_ports {
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
    if lowered.pools.len() != input.pool_bindings.len() {
        return Err(CompileError::new(CompileReason::BudgetInvalid));
    }
    let mut instance_pools = Vec::with_capacity(lowered.pools.len());
    let mut seen_pool_bindings = BTreeSet::new();
    for pool in &lowered.pools {
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
        instance_pools.push(PlanInstancePool {
            instance: instance(pool_path)?,
            template_hash: pool.template_contract_hash,
            derived_identity_hash: pool.semantic_hash,
            maximum_live: pool.maximum,
            maximum_queued: match pool.admission {
                conduit_panel::PoolAdmission::QueueBounded(maximum) => maximum,
                conduit_panel::PoolAdmission::Reject
                | conduit_panel::PoolAdmission::Block
                | conduit_panel::PoolAdmission::Fail => 0,
            },
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
        });
    }
    // This v1 compile-input/document workflow has no field for live
    // distributed-session requirements. Fail closed instead of emitting an
    // older plan schema whose cross-host cord would have hidden transport
    // semantics. A planner using the core schema-9 API must supply an exact
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
    let mut plan = ExecutionPlan {
        schema_version: EXECUTION_PLAN_SCHEMA_VERSION_V3,
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
        artifacts: &artifacts,
        nodes: &nodes,
        cords: &cords,
        distributed_cords: &[],
        fanouts: &[],
        merges: &[],
        event_streams: &[],
        runtime_evidence: None,
        jobs: &[],
        satisfaction_proofs: &[],
        authorities: &plan_authorities,
        composites: &composites,
        port_groups: &port_groups,
        instance_pools: &instance_pools,
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
        supported_schema_version: EXECUTION_PLAN_SCHEMA_VERSION_V3,
        now: plan.created_at,
    };
    if let Err(error) = validate_hosted_execution_plan(&plan, validation_context) {
        return Err(CompileError::new(
            if error.code == conduit_core::PlanDiagnosticCode::BudgetExceeded {
                CompileReason::BudgetInvalid
            } else {
                CompileReason::PlanInvalid
            },
        ));
    }
    seal_resolved_execution_plan(&resolution, &plan, validation_context)
        .map_err(|_| CompileError::new(CompileReason::PlanInvalid))?;
    let document = plan_document(&plan, topology)?;
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
        required_interfaces: &[],
        provided_interfaces: &[],
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
            Ok(PreparedAuthority {
                requirement: parse_hash(&authority.requirement)?,
                effect_hash,
                grant_hash,
                effect,
                capability,
                grant,
                binding,
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
        id: id(&document.id)?,
        host: id(&document.host)?,
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
        .map(|candidate| parse_hash(&candidate.host_report.reporter.semantic_hash))
        .collect::<Result<Vec<_>, _>>()?;
    trusted_reporters.sort_by_key(SemanticHash::to_string);
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
        plan_version: EXECUTION_PLAN_SCHEMA_VERSION_V3,
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
    match document.status.as_str() {
        "active" | "revoked" => Ok(()),
        _ => Err(CompileError::new(CompileReason::InvalidInput)),
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
        action: id(&document.action)?,
        resource,
        requester: instance(&document.requester)?,
        audience: id(&document.audience)?,
        constraints: authority_constraints(&document.constraints, arena)?,
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
        required_interfaces: &[],
        provided_interfaces: &[],
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

fn plan_document(
    plan: &ExecutionPlan<'_>,
    topology: &ExactTopologyView,
) -> Result<ExactPlanDocument, CompileError> {
    let mut hosts = plan
        .host_observations
        .iter()
        .map(|host| PlanHostDocument {
            id: host.id.to_string(),
            host: host.host.to_string(),
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
        })
        .collect::<Vec<_>>();
    resources.sort_by(|left, right| left.id.cmp(&right.id));
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
        })
        .collect();
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
        })
        .collect();
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
        host_observations: hosts,
        resources,
        artifacts,
        nodes,
        cords,
        authorities,
        composites,
        port_groups,
        instance_pools,
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
        action: effect.action.to_string(),
        resource_kind,
        resource_id,
        requester: effect.requester.as_str().to_owned(),
        audience: effect.audience.to_string(),
        constraints: constraint_documents(effect.constraints),
        check_at_use: effect.check_at_use,
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
    input.trusted_entities.sort();
    input.trusted_status_reporters.sort();
    input.implementation_preference.sort();
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
            b"conduit/plan-port-group/v1\0".as_slice(),
            logical_path.as_bytes(),
        ]
        .concat(),
    );
    format!("root/group.h{}", hex(&digest))
}

fn plan_pool_path(logical_path: &str) -> String {
    let digest = Sha256::digest(
        [
            b"conduit/plan-instance-pool/v1\0".as_slice(),
            logical_path.as_bytes(),
        ]
        .concat(),
    );
    format!("root/pool.h{}", hex(&digest))
}

fn plan_group_member_id(member: &conduit_runtime::LoweredGroupPort) -> String {
    let digest = Sha256::digest(
        [
            b"conduit/plan-port-group-member/v1\0".as_slice(),
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
            b"conduit/plan-port-group-template/v1\0".as_slice(),
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

fn id(value: &str) -> Result<Id<'_>, CompileError> {
    Id::new(value).map_err(|_| CompileError::new(CompileReason::InvalidInput))
}

fn instance(value: &str) -> Result<InstancePath<'_>, CompileError> {
    InstancePath::new(value).map_err(|_| CompileError::new(CompileReason::PlanInvalid))
}

fn direction(value: &str) -> Result<Direction, CompileError> {
    match value {
        "input" => Ok(Direction::Input),
        "output" => Ok(Direction::Output),
        _ => Err(CompileError::new(CompileReason::PlanInvalid)),
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
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileError {
    reason: CompileReason,
}

impl CompileError {
    const fn new(reason: CompileReason) -> Self {
        Self { reason }
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.reason.code()
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
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for CompileError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use conduit_core::{ARTIFACT_MANIFEST_SCHEMA_VERSION, IMPLEMENTATION_MANIFEST_SCHEMA_VERSION};
    use conduit_panel::parse;

    fn hash(byte: u8) -> String {
        SemanticHash::from_bytes([byte; 32]).to_string()
    }

    fn pin_doc(id: &str, byte: u8) -> PinDocument {
        PinDocument {
            id: id.to_owned(),
            schema_version: 1,
            semantic_hash: hash(byte),
        }
    }

    fn profile_doc(ordinal: u8) -> ExecutionProfileDocument {
        ExecutionProfileDocument {
            id: format!("fixture/execution-profile-{ordinal}"),
            schema_version: 1,
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
                    schema_version: 1,
                    semantic_hash: contract_hash.to_string(),
                },
                executor: "native-in-process".to_owned(),
                entrypoint_name: "run".to_owned(),
                entrypoint_adapter: "conduit/native-step".to_owned(),
                entrypoint_abi: "conduit/native-v1".to_owned(),
                runtime_protocol_version: 1,
                execution_profile: pin_doc("fixture/execution-profile", 30),
                artifacts: vec![ArtifactReferenceDocument {
                    id: artifact_id.clone(),
                    digest: artifact_digest.clone(),
                    role: "implementation".to_owned(),
                    required: true,
                }],
                required_authorities: Vec::new(),
                required_effects: Vec::new(),
                minimum_plan_version: 1,
                maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION_V3,
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
                supported_executors: vec!["native-in-process".to_owned()],
                supported_targets: Vec::new(),
                supported_abis: Vec::new(),
                minimum_plan_version: 1,
                maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION_V3,
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
        let topology = Registry::default()
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
            modules: vec![CompileModuleDocument {
                canonical_uri: "mem://compile/entry.panel".to_owned(),
                content_hash: String::new(),
                source: source.to_owned(),
            }],
            catalog: builtin_catalog_document().unwrap(),
            pool_bindings: Vec::new(),
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
        let source = "panel 1\n\
             composite fixture/uppercase {\n\
               node upper : conduit/uppercase\n\
               export input in = upper.in\n\
               export output out = upper.out\n\
             }\n\
             node source : conduit/literal { value = \"hello\" }\n\
             node transform : fixture/uppercase\n\
             node sink : conduit/stdout\n\
             cord source.out -> transform.in\n\
             cord transform.out -> sink.in\n";
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
        let child = "panel 1\n\
                     composite fixture/pipeline {\n\
                       node source : conduit/literal { value = \"module\" }\n\
                       node upper : conduit/uppercase using ready\n\
                       node sink : conduit/stdout\n\
                       cord source.out -> upper.in\n\
                       cord upper.out -> sink.in\n\
                     }\n";
        let entry = "panel 1\n\
                     import \"./child.panel\" as child\n\
                     composite fixture/app {\n\
                       node pipeline : child.fixture/pipeline\n\
                     }\n\
                     root fixture/app\n";
        let mut input = CompileInput {
            schema: COMPILE_INPUT_SCHEMA.to_owned(),
            schema_version: COMPILE_INPUT_SCHEMA_VERSION,
            identity: String::new(),
            entry_uri: "mem://compile/root.panel".to_owned(),
            selected_root: Some("fixture/app".to_owned()),
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
        let executable = executable_panel(&graph).unwrap();
        let topology = Registry::default()
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
            .find(|candidate| candidate.implementation.semantic_contract.id == "conduit/literal")
            .unwrap();
        let host = candidate.host_report.host.clone();
        candidate.authorities.push(AuthorityDecisionDocument {
            requirement: hash(101),
            effect_hash: String::new(),
            grant_hash: String::new(),
            effect: EffectRequirementDocument {
                id: "fixture/read".to_owned(),
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
    fn finite_port_groups_are_retained_as_plan_visible_expansions() {
        let base = include_str!("../../../examples/hello.panel");
        let source =
            format!("{base}\nport-group lanes output : conduit/output-text indexed max 2\n");
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
             composite fixture/worker {{\n\
               node source : conduit/literal {{ value = \"pool\" }}\n\
               node sink : conduit/stdout\n\
               cord source.out -> sink.in\n\
             }}\n\
             pool workers : fixture/worker {{ maximum = 2 admission = reject deadline_ms = 1000 idle_timeout_ms = 5000 supervision = isolate cleanup = abort }}\n"
        );
        let panel = parse(&source).unwrap();
        let base_panel = parse(base).unwrap();
        let mut input = compile_input(base, &base_panel);
        input.modules[0].source = source.clone();
        input.modules[0].content_hash = content_hash(&source);
        let graph = resolve_source_graph(&input).unwrap();
        let lowered = lower_compile_source(&graph, &input.catalog).unwrap();
        assert_eq!(lowered.pools.len(), 1);
        input.pool_bindings.push(PoolBindingDocument {
            pool_semantic_hash: lowered.pools[0].semantic_hash.to_string(),
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
            correlation_slots: 2,
            worst_case_budget: BudgetDocument {
                memory_bytes: 32,
                timers: 2,
                evidence_bytes: 32,
                ..BudgetDocument::default()
            },
            child_nodes: 2,
            child_cords: 1,
        });
        input.seal().unwrap();

        let plan = compile_panel(&panel, &input).unwrap();
        assert_eq!(plan.instance_pools.len(), 1);
        assert_eq!(plan.instance_pools[0].maximum_live, 2);
        assert_eq!(plan.instance_pools[0].maximum_queued, 0);
        plan.validate().unwrap();

        let mut missing = input;
        missing.pool_bindings.clear();
        missing.seal().unwrap();
        assert_eq!(
            compile_panel(&panel, &missing).unwrap_err().code(),
            "CND-CMP-007"
        );
    }
}
