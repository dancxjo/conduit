//! Hosted exact-plan compilation over explicit immutable inputs.
//!
//! This crate performs no discovery, fetch, provisioning, secret resolution,
//! grant acquisition, implementation loading, or execution.

use std::collections::BTreeSet;
use std::fmt;

use bumpalo::Bump;
use conduit_core::{
    ArtifactDigest, ArtifactManifest, ArtifactProvenance, AuthorityTime, BlockingFairness,
    Direction, EXECUTION_PLAN_SCHEMA_VERSION_V2, ExecutionPlan, ExecutorKind, FlowCapacity,
    FlowPolicy, FlowWatermarks, Id, ImplementationManifest, InstancePath, ManifestArtifactRef,
    ManifestEntrypoint, PinnedDescriptor, PlanArtifact, PlanHostObservation, PlanResourceBudget,
    PlanValidationContext, Pressure, ReplacementSupport, ResolvedPlanCord, ResolvedPlanNode,
    ResolvedPlanPort, SampleSchedule, SemanticHash, TypeContractRef,
};
use conduit_panel::SourcePressure;
use conduit_runtime::{
    CandidateAuthority, ExactTopologyView, HostResolverPolicy, PlacementCandidate,
    PlacementRequest, Registry, ResolverTiePolicy, resolve_host_placement,
    seal_resolved_execution_plan, validate_hosted_execution_plan,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

pub const COMPILE_INPUT_SCHEMA: &str = "conduit.compile-input/v1";
pub const COMPILE_INPUT_SCHEMA_VERSION: u16 = 1;
pub const PLAN_DOCUMENT_SCHEMA: &str = "conduit.execution-plan/v2";

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
    pub minimum_plan_version: u32,
    pub maximum_plan_version: u32,
    pub minimum_runtime_protocol: u32,
    pub maximum_runtime_protocol: u32,
    pub coexistence_memory_bytes: u64,
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
pub struct HostReportDocument {
    pub schema_version: u32,
    pub identity: String,
    pub id: String,
    pub host: String,
    pub reporter: PinDocument,
    pub trust: PinDocument,
    pub time_basis: String,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub available: BudgetDocument,
    pub supported_executors: Vec<String>,
    #[serde(default)]
    pub supported_targets: Vec<String>,
    #[serde(default)]
    pub supported_abis: Vec<String>,
    pub minimum_plan_version: u32,
    pub maximum_plan_version: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateDocument {
    pub implementation: ImplementationDocument,
    pub artifacts: Vec<ArtifactDocument>,
    pub host_report: HostReportDocument,
    pub allocation: BudgetDocument,
    pub lifecycle_policy: PinDocument,
    #[serde(default)]
    pub granted_authorities: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompileInput {
    pub schema: String,
    pub schema_version: u16,
    pub identity: String,
    pub source_semantic_hash: String,
    pub resolver: PinDocument,
    pub resolver_policy_hash: String,
    pub time_basis: String,
    pub current_tick: u64,
    pub plan_budget: BudgetDocument,
    pub maximum_search_states: usize,
    pub tie_policy: String,
    #[serde(default)]
    pub implementation_preference: Vec<String>,
    pub candidates: Vec<CandidateDocument>,
}

#[derive(Serialize)]
struct CompileIdentityProjection<'a> {
    schema: &'a str,
    schema_version: u16,
    source_semantic_hash: &'a str,
    resolver: &'a PinDocument,
    resolver_policy_hash: &'a str,
    time_basis: &'a str,
    current_tick: u64,
    plan_budget: BudgetDocument,
    maximum_search_states: usize,
    tie_policy: &'a str,
    implementation_preference: &'a [String],
    candidates: &'a [CandidateDocument],
}

impl CompileInput {
    pub fn seal(&mut self) -> Result<(), CompileError> {
        canonicalize_compile_input(self);
        for candidate in &mut self.candidates {
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
            source_semantic_hash: &canonical.source_semantic_hash,
            resolver: &canonical.resolver,
            resolver_policy_hash: &canonical.resolver_policy_hash,
            time_basis: &canonical.time_basis,
            current_tick: canonical.current_tick,
            plan_budget: canonical.plan_budget,
            maximum_search_states: canonical.maximum_search_states,
            tie_policy: &canonical.tie_policy,
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
        if self.maximum_search_states == 0 || self.maximum_search_states > 1_000_000 {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        parse_hash(&self.source_semantic_hash)?;
        parse_hash(&self.resolver_policy_hash)?;
        pin(&self.resolver)?;
        Id::new(&self.time_basis).map_err(|_| CompileError::new(CompileReason::InvalidInput))?;
        if self.identity != self.computed_identity()? {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        if policy_hash(self)? != self.resolver_policy_hash {
            return Err(CompileError::new(CompileReason::InvalidInput));
        }
        Ok(())
    }
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
pub struct PlanNodeDocument {
    pub instance: String,
    pub contract: PinDocument,
    pub implementation: PinDocument,
    pub lifecycle_policy: PinDocument,
    pub artifact: String,
    pub host_observation: String,
    pub host: String,
    pub allocation: BudgetDocument,
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
    pub artifacts: Vec<PlanArtifactDocument>,
    pub nodes: Vec<PlanNodeDocument>,
    pub cords: Vec<PlanCordDocument>,
    pub unresolved_selectors: Vec<String>,
}

impl ExactPlanDocument {
    pub fn validate(&self) -> Result<(), CompileError> {
        let arena = Bump::new();
        let plan = self.as_plan(&arena)?;
        validate_hosted_execution_plan(
            &plan,
            PlanValidationContext {
                supported_schema_version: EXECUTION_PLAN_SCHEMA_VERSION_V2,
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
            || self.schema_version != EXECUTION_PLAN_SCHEMA_VERSION_V2
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
        let nodes = self
            .nodes
            .iter()
            .map(|node| {
                Ok(ResolvedPlanNode {
                    instance: instance(&node.instance)?,
                    contract: pin(&node.contract)?,
                    implementation: pin(&node.implementation)?,
                    lifecycle_policy: pin(&node.lifecycle_policy)?,
                    execution_profile: None,
                    artifact: id(&node.artifact)?,
                    host_observation: id(&node.host_observation)?,
                    host: id(&node.host)?,
                    allocation: node.allocation.into(),
                    required_resources: &[],
                    required_effects: &[],
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
            resources: &[],
            artifacts: arena.alloc_slice_copy(&artifacts),
            nodes: arena.alloc_slice_copy(&nodes),
            cords: arena.alloc_slice_copy(&cords),
            fanouts: &[],
            merges: &[],
            event_streams: &[],
            runtime_evidence: None,
            jobs: &[],
            satisfaction_proofs: &[],
            authorities: &[],
            composites: &[],
            port_groups: &[],
            instance_pools: &[],
            unresolved: &[],
        })
    }
}

pub fn compile_panel(
    panel: &conduit_panel::Panel,
    input: &CompileInput,
) -> Result<ExactPlanDocument, CompileError> {
    input.validate()?;
    let registry = Registry::default();
    let resolved = registry
        .resolve(panel)
        .map_err(|_| CompileError::new(CompileReason::SourceInvalid))?;
    let topology = resolved
        .exact_topology()
        .map_err(|_| CompileError::new(CompileReason::SourceInvalid))?;
    if topology.source_semantic_hash != parse_hash(&input.source_semantic_hash)? {
        return Err(CompileError::new(CompileReason::InvalidInput));
    }
    if topology.logical_composites > 0 {
        return Err(CompileError::new(CompileReason::UnresolvedSelector));
    }
    compile_topology(&topology, input)
}

fn compile_topology(
    topology: &ExactTopologyView,
    input: &CompileInput,
) -> Result<ExactPlanDocument, CompileError> {
    let arena = Bump::new();
    let prepared = input
        .candidates
        .iter()
        .map(|candidate| prepare_candidate(candidate, &arena))
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
    let mut artifacts = Vec::new();
    let mut nodes = Vec::new();
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
        nodes.push(ResolvedPlanNode {
            instance: instance(&node.instance)?,
            contract: candidate.manifest.semantic_contract,
            implementation: PinnedDescriptor {
                id: candidate.manifest.id,
                schema_version: candidate.manifest.schema_version,
                semantic_hash: candidate.manifest.identity,
            },
            lifecycle_policy: pin(&candidate.document.lifecycle_policy)?,
            execution_profile: None,
            artifact: primary_artifact.id,
            host_observation: candidate.report.id,
            host: candidate.report.host,
            allocation: candidate.document.allocation.into(),
            required_resources: &[],
            required_effects: &[],
        });
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
    let mut plan = ExecutionPlan {
        schema_version: EXECUTION_PLAN_SCHEMA_VERSION_V2,
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
        resources: &[],
        artifacts: &artifacts,
        nodes: &nodes,
        cords: &cords,
        fanouts: &[],
        merges: &[],
        event_streams: &[],
        runtime_evidence: None,
        jobs: &[],
        satisfaction_proofs: &[],
        authorities: &[],
        composites: &[],
        port_groups: &[],
        instance_pools: &[],
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
    seal_resolved_execution_plan(
        &resolution,
        &plan,
        PlanValidationContext {
            supported_schema_version: EXECUTION_PLAN_SCHEMA_VERSION_V2,
            now: plan.created_at,
        },
    )
    .map_err(|_| CompileError::new(CompileReason::PlanInvalid))?;
    let document = plan_document(&plan, topology, &prepared)?;
    document.validate()?;
    Ok(document)
}

struct PreparedCandidate<'a> {
    document: &'a CandidateDocument,
    manifest: &'a ImplementationManifest<'a>,
    report: &'a conduit_core::CapabilityReport<'a>,
    placement: PlacementCandidate<'a>,
}

fn prepare_candidate<'a>(
    document: &'a CandidateDocument,
    arena: &'a Bump,
) -> Result<PreparedCandidate<'a>, CompileError> {
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
        required_effects: &[],
        minimum_plan_version: document.implementation.minimum_plan_version,
        maximum_plan_version: document.implementation.maximum_plan_version,
        minimum_runtime_protocol: document.implementation.minimum_runtime_protocol,
        maximum_runtime_protocol: document.implementation.maximum_runtime_protocol,
        replacement: ReplacementSupport::Cold,
        coexistence_memory_bytes: document.implementation.coexistence_memory_bytes,
        reproducibility: None,
    });
    let report = arena.alloc(capability_report(&document.host_report, arena)?);
    let authorities = document
        .implementation
        .required_authorities
        .iter()
        .map(|requirement| {
            let requirement_hash = parse_hash(requirement)?;
            let grant = document
                .granted_authorities
                .iter()
                .find(|grant| parse_hash(grant).ok() == Some(requirement_hash));
            Ok(CandidateAuthority {
                requirement: requirement_hash,
                grant: grant.map(|_| Id("compile/grant")),
                allowed: grant.is_some(),
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;
    let placement = PlacementCandidate {
        manifest,
        artifacts: arena.alloc_slice_copy(&artifacts),
        report,
        allocation: document.allocation.into(),
        capabilities: &[],
        resources: &[],
        topology: &[],
        authorities: arena.alloc_slice_copy(&authorities),
    };
    Ok(PreparedCandidate {
        document,
        manifest,
        report,
        placement,
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
    Ok(conduit_core::CapabilityReport {
        schema_version: document.schema_version,
        identity: parse_hash(&document.identity)?,
        id: id(&document.id)?,
        host: id(&document.host)?,
        reporter: pin(&document.reporter)?,
        trust: pin(&document.trust)?,
        membership: None,
        time_basis: id(&document.time_basis)?,
        observed_at_tick: document.observed_at_tick,
        valid_until_tick: document.valid_until_tick,
        available: document.available.into(),
        capabilities: &[],
        resources: &[],
        topology: &[],
        supported_executors: arena.alloc_slice_copy(&executors),
        supported_targets: arena.alloc_slice_copy(&targets),
        supported_abis: arena.alloc_slice_copy(&abis),
        minimum_plan_version: document.minimum_plan_version,
        maximum_plan_version: document.maximum_plan_version,
        current_constraints: &[],
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
    Ok(HostResolverPolicy {
        resolver: pin(&input.resolver)?,
        policy_hash: parse_hash(&input.resolver_policy_hash)?,
        time_basis: id(&input.time_basis)?,
        current_tick: input.current_tick,
        plan_version: EXECUTION_PLAN_SCHEMA_VERSION_V2,
        trusted_reporters: arena.alloc_slice_copy(&trusted_reporters),
        trusted_report_trust: arena.alloc_slice_copy(&report_trust),
        required_realm: None,
        trusted_entities: &[],
        trusted_status_reporters: &[],
        require_active_passport: false,
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
        required_effects: &[],
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
    prepared: &[PreparedCandidate<'_>],
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
    let mut nodes = plan
        .nodes
        .iter()
        .map(|node| PlanNodeDocument {
            instance: node.instance.as_str().to_owned(),
            contract: pin_document(node.contract),
            implementation: pin_document(node.implementation),
            lifecycle_policy: pin_document(node.lifecycle_policy),
            artifact: node.artifact.to_string(),
            host_observation: node.host_observation.to_string(),
            host: node.host.to_string(),
            allocation: node.allocation.into(),
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
    let _ = prepared;
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
        artifacts,
        nodes,
        cords,
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

fn pin_document(value: PinnedDescriptor<'_>) -> PinDocument {
    PinDocument {
        id: value.id.to_string(),
        schema_version: value.schema_version,
        semantic_hash: value.semantic_hash.to_string(),
    }
}

fn canonicalize_compile_input(input: &mut CompileInput) {
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
        candidate.implementation.required_authorities.sort();
        candidate.granted_authorities.sort();
    }
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

fn tie_policy(value: &str) -> Result<ResolverTiePolicy, CompileError> {
    match value {
        "reject-ambiguous" => Ok(ResolverTiePolicy::RejectAmbiguous),
        "lowest-canonical-identity" => Ok(ResolverTiePolicy::LowestCanonicalIdentity),
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
                minimum_plan_version: 1,
                maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION_V2,
                minimum_runtime_protocol: 1,
                maximum_runtime_protocol: 1,
                coexistence_memory_bytes: 0,
            },
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
                host: format!("fixture/host-{ordinal}"),
                reporter: pin_doc("fixture/reporter", 50),
                trust: pin_doc("fixture/report-trust", 51),
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
                supported_executors: vec!["native-in-process".to_owned()],
                supported_targets: Vec::new(),
                supported_abis: Vec::new(),
                minimum_plan_version: 1,
                maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION_V2,
            },
            allocation: BudgetDocument {
                memory_bytes: 32,
                cpu_units: 1,
                ..BudgetDocument::default()
            },
            lifecycle_policy: pin_doc("conduit/finite-lifecycle", 60),
            granted_authorities: Vec::new(),
        }
    }

    fn compile_input(panel: &conduit_panel::Panel) -> CompileInput {
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
            maximum_search_states: 128,
            tie_policy: "lowest-canonical-identity".to_owned(),
            implementation_preference: Vec::new(),
            candidates,
        };
        input.seal().unwrap();
        input
    }

    #[test]
    fn identical_explicit_inputs_emit_byte_identical_portable_plans() {
        let panel = parse(include_str!("../../../examples/hello.panel")).unwrap();
        let input = compile_input(&panel);
        let first = compile_panel(&panel, &input).unwrap();
        let second = compile_panel(&panel, &input).unwrap();
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
        assert!(first.unresolved_selectors.is_empty());
        first.validate().unwrap();
    }

    #[test]
    fn stale_reports_unresolved_contracts_and_budget_overruns_fail_closed() {
        let panel = parse(include_str!("../../../examples/hello.panel")).unwrap();

        let mut stale = compile_input(&panel);
        for candidate in &mut stale.candidates {
            candidate.host_report.valid_until_tick = 11;
        }
        stale.seal().unwrap();
        assert_eq!(
            compile_panel(&panel, &stale).unwrap_err().code(),
            "CND-CMP-006"
        );

        let mut unresolved = compile_input(&panel);
        unresolved.candidates.pop();
        unresolved.seal().unwrap();
        assert_eq!(
            compile_panel(&panel, &unresolved).unwrap_err().code(),
            "CND-CMP-005"
        );

        let mut over_budget = compile_input(&panel);
        over_budget.plan_budget.memory_bytes = 1;
        over_budget.seal().unwrap();
        assert_eq!(
            compile_panel(&panel, &over_budget).unwrap_err().code(),
            "CND-CMP-008"
        );
    }
}
