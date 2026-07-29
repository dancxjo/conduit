use std::collections::{BTreeMap, BTreeSet};

use conduit_core::{
    ArtifactManifest, CanonicalDescriptor, CanonicalError, CanonicalValue, CapabilityReport,
    CompatibilityOutcome, FieldDisposition, HostReportReason, Id, ImplementationManifest,
    InstancePath, ManifestReason, MapField, PinnedDescriptor, PlanResourceBudget,
    ReplacementSupport, ReportCapability, ReportResource, ReportTopology, SatisfactionProof,
    SatisfactionReason, SatisfactionRole, SemanticHash, validate_artifact_manifest,
    validate_capability_report, validate_implementation_manifest, validate_satisfaction_proof,
};
use core::convert::Infallible;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityPredicate<'a> {
    pub interface: PinnedDescriptor<'a>,
    pub mode: Id<'a>,
    pub subject: Option<Id<'a>>,
    pub details: Option<SemanticHash>,
    pub minimum_capacity: PlanResourceBudget,
    /// Required only when offered and required capability descriptors are not
    /// nominally exact.
    pub satisfaction_proof: Option<&'a SatisfactionProof<'a>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourcePredicate<'a> {
    pub kind: Id<'a>,
    /// Exact resource selected by candidate enumeration. `None` selects the
    /// canonical lowest matching pool.
    pub id: Option<Id<'a>>,
    pub descriptor: Option<PinnedDescriptor<'a>>,
    pub minimum_capacity: PlanResourceBudget,
    pub require_exclusive: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TopologyPredicate<'a> {
    pub contract: PinnedDescriptor<'a>,
    pub from: Id<'a>,
    pub to: Id<'a>,
    pub minimum_transfer_unit: u32,
    pub minimum_sessions: u32,
    pub details: Option<SemanticHash>,
}

/// An explicit result from the authority resolver. Capability never implies
/// this permission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateAuthority<'a> {
    pub requirement: SemanticHash,
    pub grant: Option<Id<'a>>,
    pub allowed: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct PlacementCandidate<'a> {
    pub manifest: &'a ImplementationManifest<'a>,
    pub artifacts: &'a [&'a ArtifactManifest<'a>],
    pub report: &'a CapabilityReport<'a>,
    pub allocation: PlanResourceBudget,
    pub capabilities: &'a [CapabilityPredicate<'a>],
    pub resources: &'a [ResourcePredicate<'a>],
    pub topology: &'a [TopologyPredicate<'a>],
    pub authorities: &'a [CandidateAuthority<'a>],
}

#[derive(Clone, Copy, Debug)]
pub struct PlacementRequest<'a> {
    pub instance: InstancePath<'a>,
    pub semantic_contract: PinnedDescriptor<'a>,
    pub candidates: &'a [PlacementCandidate<'a>],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolverTiePolicy {
    RejectAmbiguous,
    LowestCanonicalIdentity,
}

#[derive(Clone, Copy, Debug)]
pub struct HostResolverPolicy<'a> {
    pub resolver: PinnedDescriptor<'a>,
    pub policy_hash: SemanticHash,
    pub time_basis: Id<'a>,
    pub current_tick: u64,
    pub plan_version: u32,
    /// Exact reporter pins admitted by policy. Reporter IDs, schema versions,
    /// and semantic identities remain inseparable.
    pub trusted_reporters: &'a [PinnedDescriptor<'a>],
    pub trusted_report_trust: &'a [SemanticHash],
    /// Optional exact realm required for authenticated host reports.
    pub required_realm: Option<Id<'a>>,
    /// Empty accepts any entity whose active status otherwise satisfies policy.
    pub trusted_entities: &'a [Id<'a>],
    /// Empty accepts any structurally valid status reporter.
    pub trusted_status_reporters: &'a [SemanticHash],
    /// When true, candidates without a fresh active passport binding fail.
    pub require_active_passport: bool,
    pub allowed_implementations: &'a [Id<'a>],
    /// Earlier entries are preferred. Omitted implementations share the last
    /// rank and are never ordered by discovery order.
    pub implementation_preference: &'a [Id<'a>],
    pub tie_policy: ResolverTiePolicy,
    pub maximum_search_states: usize,
}

impl HostResolverPolicy<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let trusted_hashes = self
            .trusted_reporters
            .iter()
            .map(|reporter| {
                let fields = [
                    policy_field("id", CanonicalValue::Identifier(reporter.id)),
                    policy_field(
                        "version",
                        CanonicalValue::Integer(i128::from(reporter.schema_version)),
                    ),
                    policy_field(
                        "hash",
                        CanonicalValue::Bytes(reporter.semantic_hash.as_bytes()),
                    ),
                ];
                CanonicalDescriptor {
                    kind: Id("conduit/trusted-host-reporter"),
                    schema_version: 1,
                    body: CanonicalValue::Map(&fields),
                }
                .semantic_hash()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let trusted = trusted_hashes
            .iter()
            .map(|identity| CanonicalValue::Bytes(identity.as_bytes()))
            .collect::<Vec<_>>();
        let report_trust = self
            .trusted_report_trust
            .iter()
            .map(|identity| CanonicalValue::Bytes(identity.as_bytes()))
            .collect::<Vec<_>>();
        let trusted_entities = self
            .trusted_entities
            .iter()
            .map(|id| CanonicalValue::Identifier(*id))
            .collect::<Vec<_>>();
        let status_reporters = self
            .trusted_status_reporters
            .iter()
            .map(|identity| CanonicalValue::Bytes(identity.as_bytes()))
            .collect::<Vec<_>>();
        let required_realm = self
            .required_realm
            .map_or(CanonicalValue::Null, CanonicalValue::Identifier);
        let allowed = self
            .allowed_implementations
            .iter()
            .map(|id| CanonicalValue::Identifier(*id))
            .collect::<Vec<_>>();
        let preference = self
            .implementation_preference
            .iter()
            .map(|id| CanonicalValue::Identifier(*id))
            .collect::<Vec<_>>();
        let fields = [
            policy_field("resolver_id", CanonicalValue::Identifier(self.resolver.id)),
            policy_field(
                "resolver_version",
                CanonicalValue::Integer(i128::from(self.resolver.schema_version)),
            ),
            policy_field(
                "resolver_hash",
                CanonicalValue::Bytes(self.resolver.semantic_hash.as_bytes()),
            ),
            policy_field(
                "plan_version",
                CanonicalValue::Integer(i128::from(self.plan_version)),
            ),
            policy_field("trusted_reporters", CanonicalValue::Set(trusted.as_slice())),
            policy_field(
                "trusted_report_trust",
                CanonicalValue::Set(report_trust.as_slice()),
            ),
            policy_field("required_realm", required_realm),
            policy_field(
                "trusted_entities",
                CanonicalValue::Set(trusted_entities.as_slice()),
            ),
            policy_field(
                "trusted_status_reporters",
                CanonicalValue::Set(status_reporters.as_slice()),
            ),
            policy_field(
                "require_active_passport",
                CanonicalValue::Boolean(self.require_active_passport),
            ),
            policy_field(
                "allowed_implementations",
                CanonicalValue::Set(allowed.as_slice()),
            ),
            policy_field(
                "implementation_preference",
                CanonicalValue::List(preference.as_slice()),
            ),
            policy_field(
                "tie_policy",
                CanonicalValue::Identifier(Id(match self.tie_policy {
                    ResolverTiePolicy::RejectAmbiguous => "reject-ambiguous",
                    ResolverTiePolicy::LowestCanonicalIdentity => "lowest-canonical-identity",
                })),
            ),
            policy_field(
                "maximum_search_states",
                CanonicalValue::Integer(self.maximum_search_states as i128),
            ),
        ];
        CanonicalDescriptor {
            kind: Id("conduit/host-resolver-policy"),
            schema_version: 1,
            body: CanonicalValue::Map(&fields),
        }
        .semantic_hash()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum CandidateRejectionReason {
    InvalidImplementationManifest,
    ContractMismatch,
    StaleReport,
    InvalidReport,
    ReportTrustRejected,
    RealmMismatch,
    EntityRejected,
    PassportStatusRejected,
    UnsupportedPlanVersion,
    ExecutorMismatch,
    MissingArtifact,
    InvalidArtifactManifest,
    WrongTarget,
    UnsupportedAbi,
    CapabilityMissing,
    InsufficientCapacity,
    ResourceMissing,
    AuthorityDenied,
    TopologyConflict,
    PolicyRejected,
    Ambiguous,
    SearchLimit,
}

impl CandidateRejectionReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidImplementationManifest => "CND-RES-001",
            Self::ContractMismatch => "CND-RES-002",
            Self::StaleReport => "CND-RES-003",
            Self::InvalidReport => "CND-RES-004",
            Self::ReportTrustRejected => "CND-RES-005",
            Self::UnsupportedPlanVersion => "CND-RES-006",
            Self::ExecutorMismatch => "CND-RES-007",
            Self::MissingArtifact => "CND-RES-008",
            Self::InvalidArtifactManifest => "CND-RES-009",
            Self::WrongTarget => "CND-RES-010",
            Self::UnsupportedAbi => "CND-RES-011",
            Self::CapabilityMissing => "CND-RES-012",
            Self::InsufficientCapacity => "CND-RES-013",
            Self::ResourceMissing => "CND-RES-014",
            Self::AuthorityDenied => "CND-RES-015",
            Self::TopologyConflict => "CND-RES-016",
            Self::PolicyRejected => "CND-RES-017",
            Self::Ambiguous => "CND-RES-018",
            Self::SearchLimit => "CND-RES-019",
            Self::RealmMismatch => "CND-RES-027",
            Self::EntityRejected => "CND-RES-028",
            Self::PassportStatusRejected => "CND-RES-029",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateRejection {
    pub implementation: String,
    pub host: String,
    pub report: String,
    pub reasons: Vec<CandidateRejectionReason>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionFailure {
    pub requests: Vec<String>,
    pub candidates: Vec<CandidateRejection>,
    pub global_reasons: Vec<CandidateRejectionReason>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPlacementBinding {
    pub instance: String,
    pub semantic_contract: SemanticHash,
    pub implementation_id: String,
    pub implementation_identity: SemanticHash,
    pub replacement: ResolvedReplacementSupport,
    pub host: String,
    pub report_id: String,
    pub report_identity: SemanticHash,
    pub report_time_basis: String,
    pub report_observed_at_tick: u64,
    pub report_valid_until_tick: u64,
    pub allocation: PlanResourceBudget,
    pub artifacts: Vec<(String, conduit_core::ArtifactDigest)>,
    pub capability_subjects: Vec<String>,
    pub capability_proofs: Vec<SemanticHash>,
    pub resource_ids: Vec<String>,
    pub authority_grants: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedReplacementSupport {
    Cold,
    Quiescent {
        boundary_id: String,
        boundary_schema_version: u32,
        boundary_identity: SemanticHash,
        maximum_ticks: u64,
    },
    Stateful {
        state_contract_id: String,
        state_contract_schema_version: u32,
        state_contract_identity: SemanticHash,
        maximum_export_bytes: u64,
        maximum_import_bytes: u64,
        maximum_ticks: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPlacement {
    pub resolver_id: String,
    pub resolver_schema_version: u32,
    pub resolver_identity: SemanticHash,
    pub policy_hash: SemanticHash,
    pub bindings: Vec<ResolvedPlacementBinding>,
    pub search_states: usize,
}

impl ResolvedPlacement {
    /// Exact identity of the resolver decision consumed by plan-transition
    /// admission. The decision remains distinct from the candidate plan and
    /// from every host report it references.
    #[must_use]
    pub fn computed_identity(&self) -> SemanticHash {
        let mut digest = Sha256::new();
        hash_field(&mut digest, b"kind", b"conduit/resolved-placement-v1");
        hash_field(&mut digest, b"resolver-id", self.resolver_id.as_bytes());
        hash_field(
            &mut digest,
            b"resolver-version",
            &self.resolver_schema_version.to_be_bytes(),
        );
        hash_field(
            &mut digest,
            b"resolver-identity",
            self.resolver_identity.as_bytes(),
        );
        hash_field(&mut digest, b"policy", self.policy_hash.as_bytes());
        hash_field(
            &mut digest,
            b"binding-count",
            &u64::try_from(self.bindings.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        hash_field(
            &mut digest,
            b"search-states",
            &u64::try_from(self.search_states)
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        for binding in &self.bindings {
            hash_field(&mut digest, b"instance", binding.instance.as_bytes());
            hash_field(
                &mut digest,
                b"semantic-contract",
                binding.semantic_contract.as_bytes(),
            );
            hash_field(
                &mut digest,
                b"implementation-id",
                binding.implementation_id.as_bytes(),
            );
            hash_field(
                &mut digest,
                b"implementation-identity",
                binding.implementation_identity.as_bytes(),
            );
            hash_replacement_support(&mut digest, &binding.replacement);
            hash_field(&mut digest, b"host", binding.host.as_bytes());
            hash_field(&mut digest, b"report-id", binding.report_id.as_bytes());
            hash_field(
                &mut digest,
                b"report-identity",
                binding.report_identity.as_bytes(),
            );
            hash_field(
                &mut digest,
                b"report-time-basis",
                binding.report_time_basis.as_bytes(),
            );
            hash_field(
                &mut digest,
                b"report-observed",
                &binding.report_observed_at_tick.to_be_bytes(),
            );
            hash_field(
                &mut digest,
                b"report-valid-until",
                &binding.report_valid_until_tick.to_be_bytes(),
            );
            hash_resource_budget(&mut digest, binding.allocation);
            hash_collection_len(&mut digest, b"artifacts", binding.artifacts.len());
            for (id, artifact) in &binding.artifacts {
                hash_field(&mut digest, b"artifact-id", id.as_bytes());
                hash_field(&mut digest, b"artifact-digest", artifact.as_bytes());
            }
            hash_collection_len(
                &mut digest,
                b"capability-subjects",
                binding.capability_subjects.len(),
            );
            for subject in &binding.capability_subjects {
                hash_field(&mut digest, b"capability-subject", subject.as_bytes());
            }
            hash_collection_len(
                &mut digest,
                b"capability-proofs",
                binding.capability_proofs.len(),
            );
            for proof in &binding.capability_proofs {
                hash_field(&mut digest, b"capability-proof", proof.as_bytes());
            }
            hash_collection_len(&mut digest, b"resources", binding.resource_ids.len());
            for resource in &binding.resource_ids {
                hash_field(&mut digest, b"resource", resource.as_bytes());
            }
            hash_collection_len(
                &mut digest,
                b"authority-grants",
                binding.authority_grants.len(),
            );
            for grant in &binding.authority_grants {
                hash_field(&mut digest, b"authority-grant", grant.as_bytes());
            }
        }
        SemanticHash::from_bytes(digest.finalize().into())
    }
}

fn hash_field(digest: &mut Sha256, name: &[u8], value: &[u8]) {
    hash_part(digest, name);
    hash_part(digest, value);
}

fn hash_collection_len(digest: &mut Sha256, name: &[u8], len: usize) {
    hash_field(
        digest,
        name,
        &u64::try_from(len).unwrap_or(u64::MAX).to_be_bytes(),
    );
}

fn hash_replacement_support(digest: &mut Sha256, support: &ResolvedReplacementSupport) {
    match support {
        ResolvedReplacementSupport::Cold => {
            hash_field(digest, b"replacement-mode", b"cold");
        }
        ResolvedReplacementSupport::Quiescent {
            boundary_id,
            boundary_schema_version,
            boundary_identity,
            maximum_ticks,
        } => {
            hash_field(digest, b"replacement-mode", b"quiescent");
            hash_field(digest, b"boundary-id", boundary_id.as_bytes());
            hash_field(
                digest,
                b"boundary-version",
                &boundary_schema_version.to_be_bytes(),
            );
            hash_field(digest, b"boundary-identity", boundary_identity.as_bytes());
            hash_field(digest, b"replacement-ticks", &maximum_ticks.to_be_bytes());
        }
        ResolvedReplacementSupport::Stateful {
            state_contract_id,
            state_contract_schema_version,
            state_contract_identity,
            maximum_export_bytes,
            maximum_import_bytes,
            maximum_ticks,
        } => {
            hash_field(digest, b"replacement-mode", b"stateful");
            hash_field(digest, b"state-contract-id", state_contract_id.as_bytes());
            hash_field(
                digest,
                b"state-contract-version",
                &state_contract_schema_version.to_be_bytes(),
            );
            hash_field(
                digest,
                b"state-contract-identity",
                state_contract_identity.as_bytes(),
            );
            hash_field(
                digest,
                b"maximum-export-bytes",
                &maximum_export_bytes.to_be_bytes(),
            );
            hash_field(
                digest,
                b"maximum-import-bytes",
                &maximum_import_bytes.to_be_bytes(),
            );
            hash_field(digest, b"replacement-ticks", &maximum_ticks.to_be_bytes());
        }
    }
}

fn owned_replacement_support(support: ReplacementSupport<'_>) -> ResolvedReplacementSupport {
    match support {
        ReplacementSupport::Cold => ResolvedReplacementSupport::Cold,
        ReplacementSupport::Quiescent {
            boundary,
            maximum_ticks,
        } => ResolvedReplacementSupport::Quiescent {
            boundary_id: boundary.id.to_string(),
            boundary_schema_version: boundary.schema_version,
            boundary_identity: boundary.semantic_hash,
            maximum_ticks,
        },
        ReplacementSupport::Stateful {
            state_contract,
            maximum_export_bytes,
            maximum_import_bytes,
            maximum_ticks,
        } => ResolvedReplacementSupport::Stateful {
            state_contract_id: state_contract.id.to_string(),
            state_contract_schema_version: state_contract.schema_version,
            state_contract_identity: state_contract.semantic_hash,
            maximum_export_bytes,
            maximum_import_bytes,
            maximum_ticks,
        },
    }
}

fn hash_part(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    digest.update(bytes);
}

fn hash_resource_budget(digest: &mut Sha256, budget: PlanResourceBudget) {
    hash_part(digest, &budget.memory_bytes.to_be_bytes());
    hash_part(digest, &budget.storage_bytes.to_be_bytes());
    hash_part(digest, &budget.cpu_units.to_be_bytes());
    hash_part(digest, &budget.timers.to_be_bytes());
    hash_part(digest, &budget.transports.to_be_bytes());
    hash_part(digest, &budget.checkpoints.to_be_bytes());
    hash_part(digest, &budget.evidence_bytes.to_be_bytes());
}

#[derive(Clone, Debug)]
struct EvaluatedCandidate {
    request_index: usize,
    candidate_index: usize,
    preference_rank: usize,
    canonical_key: String,
}

#[derive(Clone, Debug)]
struct Solution {
    choices: Vec<EvaluatedCandidate>,
    ranks: Vec<usize>,
}

/// Resolves every request as one immutable placement set. The function reads
/// only its explicit inputs; it performs no discovery, provisioning, artifact
/// fetch, grant acquisition, login, network operation, or host mutation.
pub fn resolve_host_placement(
    requests: &[PlacementRequest<'_>],
    policy: HostResolverPolicy<'_>,
) -> Result<ResolvedPlacement, ResolutionFailure> {
    if policy.plan_version == 0
        || policy.resolver.schema_version == 0
        || Id::new(policy.resolver.id.as_str()).is_err()
        || policy.computed_semantic_hash().ok() != Some(policy.policy_hash)
    {
        return Err(failure(
            requests,
            Vec::new(),
            vec![CandidateRejectionReason::PolicyRejected],
        ));
    }
    let mut request_order = (0..requests.len()).collect::<Vec<_>>();
    request_order.sort_by_key(|index| requests[*index].instance.as_str());

    let mut rejections = Vec::new();
    let mut viable = Vec::with_capacity(requests.len());
    for request_index in request_order {
        let request = &requests[request_index];
        let mut request_viable = Vec::new();
        for (candidate_index, candidate) in request.candidates.iter().enumerate() {
            let mut reasons = evaluate_candidate(request, candidate, policy);
            reasons.sort_unstable();
            reasons.dedup();
            if reasons.is_empty() {
                request_viable.push(EvaluatedCandidate {
                    request_index,
                    candidate_index,
                    preference_rank: preference_rank(
                        candidate.manifest.id,
                        policy.implementation_preference,
                    ),
                    canonical_key: canonical_candidate_key(candidate),
                });
            } else {
                rejections.push(rejection(candidate, reasons));
            }
        }
        request_viable.sort_by(|left, right| {
            left.preference_rank
                .cmp(&right.preference_rank)
                .then_with(|| left.canonical_key.cmp(&right.canonical_key))
        });
        request_viable.dedup_by(|left, right| left.canonical_key == right.canonical_key);
        if request_viable.is_empty() {
            return Err(failure(requests, rejections, Vec::new()));
        }
        viable.push(request_viable);
    }

    if policy.maximum_search_states == 0 {
        return Err(failure(
            requests,
            rejections,
            vec![CandidateRejectionReason::SearchLimit],
        ));
    }
    let mut search = Search {
        requests,
        viable: &viable,
        policy,
        states: 0,
        current: Vec::new(),
        usage: BTreeMap::new(),
        fact_usage: BTreeMap::new(),
        exclusive_resources: BTreeSet::new(),
        solutions: Vec::new(),
        exhausted: false,
    };
    search.visit(0);
    if search.exhausted {
        return Err(failure(
            requests,
            rejections,
            vec![CandidateRejectionReason::SearchLimit],
        ));
    }
    let Some(first) = search.solutions.first() else {
        return Err(failure(
            requests,
            rejections,
            vec![CandidateRejectionReason::InsufficientCapacity],
        ));
    };
    if policy.tie_policy == ResolverTiePolicy::RejectAmbiguous
        && search
            .solutions
            .get(1)
            .is_some_and(|second| second.ranks == first.ranks)
    {
        return Err(failure(
            requests,
            rejections,
            vec![CandidateRejectionReason::Ambiguous],
        ));
    }

    let bindings = first
        .choices
        .iter()
        .map(|choice| {
            let request = &requests[choice.request_index];
            let candidate = &request.candidates[choice.candidate_index];
            let mut artifacts = candidate
                .manifest
                .artifacts
                .iter()
                .filter(|artifact| artifact.required)
                .map(|artifact| (artifact.id.to_string(), artifact.digest))
                .collect::<Vec<_>>();
            artifacts.sort_by(|left, right| left.0.cmp(&right.0));
            let mut capability_subjects = candidate
                .capabilities
                .iter()
                .filter_map(|required| {
                    matching_capability(candidate.report, required, policy.policy_hash)
                        .map(|capability| capability.subject.to_string())
                })
                .collect::<Vec<_>>();
            capability_subjects.sort();
            let mut capability_proofs = candidate
                .capabilities
                .iter()
                .filter_map(|required| {
                    matching_capability(candidate.report, required, policy.policy_hash)
                        .filter(|observed| observed.interface != required.interface)
                        .and_then(|_| required.satisfaction_proof.map(|proof| proof.identity))
                })
                .collect::<Vec<_>>();
            capability_proofs.sort_by_key(SemanticHash::to_string);
            let mut resource_ids = candidate
                .resources
                .iter()
                .filter_map(|required| {
                    matching_resource(candidate.report, required)
                        .map(|resource| resource.resource.id.to_string())
                })
                .collect::<Vec<_>>();
            resource_ids.sort();
            let mut authority_grants = candidate
                .authorities
                .iter()
                .filter(|authority| authority.allowed)
                .filter_map(|authority| authority.grant.map(|grant| grant.to_string()))
                .collect::<Vec<_>>();
            authority_grants.sort();
            ResolvedPlacementBinding {
                instance: request.instance.as_str().to_owned(),
                semantic_contract: request.semantic_contract.semantic_hash,
                implementation_id: candidate.manifest.id.to_string(),
                implementation_identity: candidate.manifest.identity,
                replacement: owned_replacement_support(candidate.manifest.replacement),
                host: candidate.report.host.to_string(),
                report_id: candidate.report.id.to_string(),
                report_identity: candidate.report.identity,
                report_time_basis: candidate.report.time_basis.to_string(),
                report_observed_at_tick: candidate.report.observed_at_tick,
                report_valid_until_tick: candidate.report.valid_until_tick,
                allocation: candidate.allocation,
                artifacts,
                capability_subjects,
                capability_proofs,
                resource_ids,
                authority_grants,
            }
        })
        .collect();
    Ok(ResolvedPlacement {
        resolver_id: policy.resolver.id.to_string(),
        resolver_schema_version: policy.resolver.schema_version,
        resolver_identity: policy.resolver.semantic_hash,
        policy_hash: policy.policy_hash,
        bindings,
        search_states: search.states,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanSealingReason {
    ResolverMismatch,
    PolicyMismatch,
    BindingMissing,
    BindingMismatch,
    ArtifactMissing,
    HostObservationMissing,
    PortablePlanInvalid,
}

impl PlanSealingReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ResolverMismatch => "CND-RES-020",
            Self::PolicyMismatch => "CND-RES-021",
            Self::BindingMissing => "CND-RES-022",
            Self::BindingMismatch => "CND-RES-023",
            Self::ArtifactMissing => "CND-RES-024",
            Self::HostObservationMissing => "CND-RES-025",
            Self::PortablePlanInvalid => "CND-RES-026",
        }
    }
}

/// Proves that a caller-assembled exact `ExecutionPlan` pins every resolver
/// decision before the plan is admitted. Source topology remains owned by the
/// ordinary planner; this function does not create a second plan model.
pub fn seal_resolved_execution_plan(
    resolution: &ResolvedPlacement,
    plan: &conduit_core::ExecutionPlan<'_>,
    context: conduit_core::PlanValidationContext<'_>,
) -> Result<(), PlanSealingReason> {
    if plan.resolver.id.as_str() != resolution.resolver_id
        || plan.resolver.schema_version != resolution.resolver_schema_version
        || plan.resolver.semantic_hash != resolution.resolver_identity
    {
        return Err(PlanSealingReason::ResolverMismatch);
    }
    if plan.resolver_policy_hash != resolution.policy_hash {
        return Err(PlanSealingReason::PolicyMismatch);
    }
    if plan.nodes.len() != resolution.bindings.len() {
        return Err(PlanSealingReason::BindingMissing);
    }
    for binding in &resolution.bindings {
        let node = plan
            .nodes
            .iter()
            .find(|node| node.instance.as_str() == binding.instance)
            .ok_or(PlanSealingReason::BindingMissing)?;
        if node.contract.semantic_hash != binding.semantic_contract
            || node.implementation.id.as_str() != binding.implementation_id
            || node.implementation.semantic_hash != binding.implementation_identity
            || node.host.as_str() != binding.host
            || node.host_observation.as_str() != binding.report_id
            || node.allocation != binding.allocation
        {
            return Err(PlanSealingReason::BindingMismatch);
        }
        if !binding.artifacts.iter().any(|(id, digest)| {
            node.artifact.as_str() == id
                && plan
                    .artifacts
                    .iter()
                    .any(|artifact| artifact.id.as_str() == id && artifact.digest == *digest)
        }) || binding.artifacts.iter().any(|(id, digest)| {
            !plan
                .artifacts
                .iter()
                .any(|artifact| artifact.id.as_str() == id && artifact.digest == *digest)
        }) {
            return Err(PlanSealingReason::ArtifactMissing);
        }
        if binding.resource_ids.iter().any(|resource_id| {
            !plan.resources.iter().any(|resource| {
                resource.id.as_str() == resource_id
                    && resource.node.as_str() == binding.instance
                    && resource.host_observation.as_str() == binding.report_id
            })
        }) || binding.authority_grants.iter().any(|grant_id| {
            !plan.authorities.iter().any(|authority| {
                authority.node.as_str() == binding.instance
                    && authority.grant.id.as_str() == grant_id
            })
        }) || binding.capability_proofs.iter().any(|proof_identity| {
            !plan.satisfaction_proofs.iter().any(|proof| {
                proof.proof.identity == *proof_identity
                    && matches!(
                        proof.subject,
                        conduit_core::PlanSatisfactionSubject::HostCapability {
                            node,
                            host_observation
                        } if node.as_str() == binding.instance
                            && host_observation.as_str() == binding.report_id
                    )
            })
        }) {
            return Err(PlanSealingReason::BindingMismatch);
        }
        if !plan.host_observations.iter().any(|observation| {
            observation.id.as_str() == binding.report_id
                && observation.host.as_str() == binding.host
                && observation.semantic_hash == binding.report_identity
                && observation.time_basis.as_str() == binding.report_time_basis
                && observation.observed_at_tick == binding.report_observed_at_tick
                && observation.valid_until_tick == binding.report_valid_until_tick
        }) {
            return Err(PlanSealingReason::HostObservationMissing);
        }
    }
    crate::validate_hosted_execution_plan(plan, context)
        .map_err(|_| PlanSealingReason::PortablePlanInvalid)
}

struct Search<'a, 'b> {
    requests: &'a [PlacementRequest<'a>],
    viable: &'b [Vec<EvaluatedCandidate>],
    policy: HostResolverPolicy<'a>,
    states: usize,
    current: Vec<EvaluatedCandidate>,
    usage: BTreeMap<String, PlanResourceBudget>,
    fact_usage: BTreeMap<String, PlanResourceBudget>,
    exclusive_resources: BTreeSet<String>,
    solutions: Vec<Solution>,
    exhausted: bool,
}

impl Search<'_, '_> {
    fn visit(&mut self, depth: usize) {
        if self.solutions.len() >= 2 || self.exhausted {
            return;
        }
        if self.states >= self.policy.maximum_search_states {
            self.exhausted = true;
            return;
        }
        self.states += 1;
        if depth == self.viable.len() {
            self.solutions.push(Solution {
                choices: self.current.clone(),
                ranks: self
                    .current
                    .iter()
                    .map(|candidate| candidate.preference_rank)
                    .collect(),
            });
            return;
        }
        for evaluated in &self.viable[depth] {
            let candidate =
                &self.requests[evaluated.request_index].candidates[evaluated.candidate_index];
            let report_key = candidate.report.identity.to_string();
            let before = self
                .usage
                .get(&report_key)
                .copied()
                .unwrap_or(PlanResourceBudget::ZERO);
            let Some(after) = checked_add(before, candidate.allocation) else {
                continue;
            };
            if !fits(after, candidate.report.available) {
                continue;
            }
            let fact_usage_before = self.fact_usage.clone();
            let exclusive_before = self.exclusive_resources.clone();
            if !reserve_candidate_facts(
                candidate,
                self.policy.policy_hash,
                &mut self.fact_usage,
                &mut self.exclusive_resources,
            ) {
                self.fact_usage = fact_usage_before;
                self.exclusive_resources = exclusive_before;
                continue;
            }
            self.usage.insert(report_key.clone(), after);
            self.current.push(evaluated.clone());
            self.visit(depth + 1);
            self.current.pop();
            if before == PlanResourceBudget::ZERO {
                self.usage.remove(&report_key);
            } else {
                self.usage.insert(report_key, before);
            }
            self.fact_usage = fact_usage_before;
            self.exclusive_resources = exclusive_before;
        }
    }
}

fn evaluate_candidate(
    request: &PlacementRequest<'_>,
    candidate: &PlacementCandidate<'_>,
    policy: HostResolverPolicy<'_>,
) -> Vec<CandidateRejectionReason> {
    let mut reasons = Vec::new();
    let mut scratch =
        vec![SemanticHash::from_bytes([0; 32]); candidate.manifest.identity_fact_count()];
    if let Err(reason) = validate_implementation_manifest(candidate.manifest, &mut scratch) {
        reasons.push(match reason {
            ManifestReason::UnsupportedVersion => CandidateRejectionReason::UnsupportedPlanVersion,
            _ => CandidateRejectionReason::InvalidImplementationManifest,
        });
    }
    if candidate.manifest.semantic_contract != request.semantic_contract {
        reasons.push(CandidateRejectionReason::ContractMismatch);
    }
    if policy.plan_version < candidate.manifest.minimum_plan_version
        || policy.plan_version > candidate.manifest.maximum_plan_version
    {
        reasons.push(CandidateRejectionReason::UnsupportedPlanVersion);
    }
    let mut report_scratch =
        vec![SemanticHash::from_bytes([0; 32]); candidate.report.identity_fact_count()];
    if let Err(reason) = validate_capability_report(
        candidate.report,
        policy.time_basis,
        policy.current_tick,
        policy.plan_version,
        &mut report_scratch,
    ) {
        reasons.push(match reason {
            HostReportReason::Stale | HostReportReason::NotYetObserved => {
                CandidateRejectionReason::StaleReport
            }
            HostReportReason::UnsupportedPlanVersion => {
                CandidateRejectionReason::UnsupportedPlanVersion
            }
            HostReportReason::MembershipInvalid => CandidateRejectionReason::PassportStatusRejected,
            _ => CandidateRejectionReason::InvalidReport,
        });
    }
    if !policy.trusted_reporters.is_empty()
        && !policy
            .trusted_reporters
            .contains(&candidate.report.reporter)
    {
        reasons.push(CandidateRejectionReason::ReportTrustRejected);
    }
    if !policy.trusted_report_trust.is_empty()
        && !policy
            .trusted_report_trust
            .contains(&candidate.report.trust.semantic_hash)
    {
        reasons.push(CandidateRejectionReason::ReportTrustRejected);
    }
    match candidate.report.membership {
        Some(membership) => {
            if policy
                .required_realm
                .is_some_and(|realm| realm != membership.realm)
            {
                reasons.push(CandidateRejectionReason::RealmMismatch);
            }
            if !policy.trusted_entities.is_empty()
                && !policy.trusted_entities.contains(&membership.entity)
            {
                reasons.push(CandidateRejectionReason::EntityRejected);
            }
            if !policy.trusted_status_reporters.is_empty()
                && !policy
                    .trusted_status_reporters
                    .contains(&membership.status.reporter.semantic_hash)
            {
                reasons.push(CandidateRejectionReason::PassportStatusRejected);
            }
        }
        None if policy.require_active_passport
            || policy.required_realm.is_some()
            || !policy.trusted_entities.is_empty()
            || !policy.trusted_status_reporters.is_empty() =>
        {
            reasons.push(CandidateRejectionReason::PassportStatusRejected);
        }
        None => {}
    }
    if !policy.allowed_implementations.is_empty()
        && !policy
            .allowed_implementations
            .contains(&candidate.manifest.id)
    {
        reasons.push(CandidateRejectionReason::PolicyRejected);
    }
    if !candidate
        .report
        .supported_executors
        .contains(&candidate.manifest.executor)
    {
        reasons.push(CandidateRejectionReason::ExecutorMismatch);
    }
    if !fits(candidate.allocation, candidate.report.available) {
        reasons.push(CandidateRejectionReason::InsufficientCapacity);
    }
    for reference in candidate
        .manifest
        .artifacts
        .iter()
        .filter(|reference| reference.required)
    {
        let Some(artifact) =
            candidate.artifacts.iter().copied().find(|artifact| {
                artifact.id == reference.id && artifact.digest == reference.digest
            })
        else {
            reasons.push(CandidateRejectionReason::MissingArtifact);
            continue;
        };
        let mut artifact_scratch =
            vec![SemanticHash::from_bytes([0; 32]); artifact.identity_fact_count()];
        if validate_artifact_manifest(artifact, &mut artifact_scratch).is_err() {
            reasons.push(CandidateRejectionReason::InvalidArtifactManifest);
        }
        if artifact
            .target
            .is_some_and(|target| !candidate.report.supported_targets.contains(&target))
        {
            reasons.push(CandidateRejectionReason::WrongTarget);
        }
        if artifact
            .abi
            .is_some_and(|abi| !candidate.report.supported_abis.contains(&abi))
        {
            reasons.push(CandidateRejectionReason::UnsupportedAbi);
        }
    }
    for required in candidate.capabilities {
        let Some(observed) = matching_capability(candidate.report, required, policy.policy_hash)
        else {
            reasons.push(CandidateRejectionReason::CapabilityMissing);
            continue;
        };
        if !fits(required.minimum_capacity, observed.capacity) {
            reasons.push(CandidateRejectionReason::InsufficientCapacity);
        }
    }
    for required in candidate.resources {
        let Some(observed) = matching_resource(candidate.report, required) else {
            reasons.push(CandidateRejectionReason::ResourceMissing);
            continue;
        };
        if !fits(required.minimum_capacity, observed.capacity) {
            reasons.push(CandidateRejectionReason::InsufficientCapacity);
        }
    }
    for required in candidate.topology {
        if !candidate
            .report
            .topology
            .iter()
            .any(|observed| topology_matches(observed, required))
        {
            reasons.push(CandidateRejectionReason::TopologyConflict);
        }
    }
    for required in candidate.manifest.required_authorities {
        if !candidate.authorities.iter().any(|authority| {
            authority.requirement == *required && authority.allowed && authority.grant.is_some()
        }) {
            reasons.push(CandidateRejectionReason::AuthorityDenied);
        }
    }
    reasons
}

fn matching_capability<'a>(
    report: &'a CapabilityReport<'a>,
    required: &CapabilityPredicate<'_>,
    policy_hash: SemanticHash,
) -> Option<&'a ReportCapability<'a>> {
    report
        .capabilities
        .iter()
        .filter(|observed| {
            capability_contract_matches(observed, required, policy_hash)
                && observed.mode == required.mode
                && required
                    .subject
                    .is_none_or(|subject| subject == observed.subject)
                && required
                    .details
                    .is_none_or(|details| details == observed.details)
        })
        .min_by_key(|observed| observed.subject.as_str())
}

fn capability_contract_matches(
    observed: &ReportCapability<'_>,
    required: &CapabilityPredicate<'_>,
    policy_hash: SemanticHash,
) -> bool {
    if observed.interface == required.interface {
        return true;
    }
    let Some(proof) = required.satisfaction_proof else {
        return false;
    };
    if proof.role != SatisfactionRole::HostCapability
        || proof.outcome != CompatibilityOutcome::Compatible
        || proof.reason != SatisfactionReason::Satisfied
        || proof.required.kind != required.interface.id
        || proof.required.schema_version != required.interface.schema_version
        || proof.required.semantic_hash != required.interface.semantic_hash
        || proof.offered.kind != observed.interface.id
        || proof.offered.schema_version != observed.interface.schema_version
        || proof.offered.semantic_hash != observed.interface.semantic_hash
        || proof
            .policy
            .is_none_or(|policy| policy.descriptor.semantic_hash != policy_hash)
    {
        return false;
    }
    let mut scratch = vec![SemanticHash::from_bytes([0; 32]); proof.identity_fact_count()];
    validate_satisfaction_proof(proof, &mut scratch).is_ok()
}

fn matching_resource<'a>(
    report: &'a CapabilityReport<'a>,
    required: &ResourcePredicate<'_>,
) -> Option<&'a ReportResource<'a>> {
    report
        .resources
        .iter()
        .filter(|observed| {
            observed.resource.kind == required.kind
                && required.id.is_none_or(|id| id == observed.resource.id)
                && required
                    .descriptor
                    .is_none_or(|descriptor| descriptor == observed.descriptor)
                && (!required.require_exclusive || observed.exclusive)
        })
        .min_by_key(|observed| observed.resource.id.as_str())
}

fn topology_matches(observed: &ReportTopology<'_>, required: &TopologyPredicate<'_>) -> bool {
    observed.contract == required.contract
        && observed.from == required.from
        && observed.to == required.to
        && observed.reachable
        && observed.maximum_transfer_unit >= required.minimum_transfer_unit
        && observed.maximum_sessions >= required.minimum_sessions
        && required
            .details
            .is_none_or(|details| details == observed.details)
}

fn rejection(
    candidate: &PlacementCandidate<'_>,
    reasons: Vec<CandidateRejectionReason>,
) -> CandidateRejection {
    CandidateRejection {
        implementation: candidate.manifest.id.to_string(),
        host: candidate.report.host.to_string(),
        report: candidate.report.id.to_string(),
        reasons,
    }
}

fn failure(
    requests: &[PlacementRequest<'_>],
    mut candidates: Vec<CandidateRejection>,
    mut global_reasons: Vec<CandidateRejectionReason>,
) -> ResolutionFailure {
    candidates.sort_by(|left, right| {
        left.implementation
            .cmp(&right.implementation)
            .then_with(|| left.host.cmp(&right.host))
            .then_with(|| left.report.cmp(&right.report))
    });
    global_reasons.sort_unstable();
    global_reasons.dedup();
    let mut request_names = requests
        .iter()
        .map(|request| request.instance.as_str().to_owned())
        .collect::<Vec<_>>();
    request_names.sort();
    ResolutionFailure {
        requests: request_names,
        candidates,
        global_reasons,
    }
}

fn preference_rank(value: Id<'_>, preference: &[Id<'_>]) -> usize {
    preference
        .iter()
        .position(|candidate| *candidate == value)
        .unwrap_or(preference.len())
}

fn canonical_candidate_key(candidate: &PlacementCandidate<'_>) -> String {
    let mut facts = candidate
        .capabilities
        .iter()
        .map(|required| {
            format!(
                "cap:{}@{}:{}:{}:{}:{}:{}",
                required.interface.id,
                required.interface.semantic_hash,
                required.mode,
                required.subject.map_or("*", Id::as_str),
                required
                    .details
                    .map_or("*".to_owned(), |value| value.to_string()),
                budget_key(required.minimum_capacity),
                required
                    .satisfaction_proof
                    .map_or("*".to_owned(), |proof| proof.identity.to_string())
            )
        })
        .chain(candidate.resources.iter().map(|required| {
            format!(
                "res:{}:{}:{}:{}:{}",
                required.kind,
                required.id.map_or("*", Id::as_str),
                required
                    .descriptor
                    .map_or("*".to_owned(), |value| value.semantic_hash.to_string()),
                budget_key(required.minimum_capacity),
                required.require_exclusive
            )
        }))
        .chain(candidate.topology.iter().map(|required| {
            format!(
                "top:{}:{}:{}:{}:{}:{}",
                required.contract.semantic_hash,
                required.from,
                required.to,
                required.minimum_transfer_unit,
                required.minimum_sessions,
                required
                    .details
                    .map_or("*".to_owned(), |value| value.to_string())
            )
        }))
        .chain(candidate.authorities.iter().map(|authority| {
            format!(
                "auth:{}:{}:{}",
                authority.requirement,
                authority.grant.map_or("*", Id::as_str),
                authority.allowed
            )
        }))
        .chain(
            candidate
                .artifacts
                .iter()
                .map(|artifact| format!("artifact:{}", artifact.identity)),
        )
        .collect::<Vec<_>>();
    facts.sort();
    format!(
        "{}@{}:{}@{}:{}:{}",
        candidate.manifest.id,
        candidate.manifest.identity,
        candidate.report.id,
        candidate.report.identity,
        budget_key(candidate.allocation),
        facts.join("|")
    )
}

fn checked_add(left: PlanResourceBudget, right: PlanResourceBudget) -> Option<PlanResourceBudget> {
    Some(PlanResourceBudget {
        memory_bytes: left.memory_bytes.checked_add(right.memory_bytes)?,
        storage_bytes: left.storage_bytes.checked_add(right.storage_bytes)?,
        cpu_units: left.cpu_units.checked_add(right.cpu_units)?,
        timers: left.timers.checked_add(right.timers)?,
        transports: left.transports.checked_add(right.transports)?,
        checkpoints: left.checkpoints.checked_add(right.checkpoints)?,
        evidence_bytes: left.evidence_bytes.checked_add(right.evidence_bytes)?,
    })
}

fn fits(value: PlanResourceBudget, ceiling: PlanResourceBudget) -> bool {
    value.memory_bytes <= ceiling.memory_bytes
        && value.storage_bytes <= ceiling.storage_bytes
        && value.cpu_units <= ceiling.cpu_units
        && value.timers <= ceiling.timers
        && value.transports <= ceiling.transports
        && value.checkpoints <= ceiling.checkpoints
        && value.evidence_bytes <= ceiling.evidence_bytes
}

fn policy_field<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

fn reserve_candidate_facts(
    candidate: &PlacementCandidate<'_>,
    policy_hash: SemanticHash,
    usage: &mut BTreeMap<String, PlanResourceBudget>,
    exclusive_resources: &mut BTreeSet<String>,
) -> bool {
    for required in candidate.capabilities {
        let Some(observed) = matching_capability(candidate.report, required, policy_hash) else {
            return false;
        };
        let key = format!(
            "{}:cap:{}:{}:{}",
            candidate.report.identity,
            observed.interface.semantic_hash,
            observed.mode,
            observed.subject
        );
        let before = usage.get(&key).copied().unwrap_or(PlanResourceBudget::ZERO);
        let Some(after) = checked_add(before, required.minimum_capacity) else {
            return false;
        };
        if !fits(after, observed.capacity) {
            return false;
        }
        usage.insert(key, after);
    }
    for required in candidate.resources {
        let Some(observed) = matching_resource(candidate.report, required) else {
            return false;
        };
        let key = format!(
            "{}:res:{}:{}",
            candidate.report.identity, observed.resource.kind, observed.resource.id
        );
        if observed.exclusive && !exclusive_resources.insert(key.clone()) {
            return false;
        }
        let before = usage.get(&key).copied().unwrap_or(PlanResourceBudget::ZERO);
        let Some(after) = checked_add(before, required.minimum_capacity) else {
            return false;
        };
        if !fits(after, observed.capacity) {
            return false;
        }
        usage.insert(key, after);
    }
    true
}

fn budget_key(value: PlanResourceBudget) -> String {
    format!(
        "{},{},{},{},{},{},{}",
        value.memory_bytes,
        value.storage_bytes,
        value.cpu_units,
        value.timers,
        value.transports,
        value.checkpoints,
        value.evidence_bytes
    )
}
