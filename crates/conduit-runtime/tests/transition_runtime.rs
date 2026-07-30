use std::net::{IpAddr, Ipv4Addr};
use std::task::Poll;

use conduit_core::{
    AdministrativeApproval, AdministrativeApprovalStatus, AdministrativeApprover,
    AdministrativeCommit, AdministrativeExecution, AdministrativePrincipal, AdministrativeProof,
    AdministrativeProposal, AdministrativeSubject, ArtifactDigest, ArtifactManifest,
    ArtifactProvenance, AuthorityConstraintRef, AuthorityGrant, AuthorityScope, AuthorityTime,
    CAPABILITY_REPORT_SCHEMA_VERSION, CONTAINMENT_POLICY_SCHEMA_VERSION, CapabilityReport,
    ContainmentContext, ContainmentPolicy, DelegationPolicy, EffectClassBinding, EffectClassTraits,
    EffectRequirement, EventClass, EventPayloadRef, ExecutorKind, GrantStatus,
    HAZARD_CLOSURE_POLICY_SCHEMA_VERSION, HAZARDOUS_HOST_PROFILE_SCHEMA_VERSION,
    HazardClosureContext, HazardClosureLimits, HazardClosurePolicy, HazardousHostBinding,
    HazardousHostProfile, HostCapability, INHIBIT_OBSERVATION_SCHEMA_VERSION, Id,
    ImplementationConfinement, ImplementationManifest, InhibitLatchState, InhibitObservation,
    InstancePath, ManifestArtifactRef, ManifestEntrypoint, ObservedGrant, OperatingEnvelopeLimit,
    OptionalCharacteristicChange, PLAN_TRANSITION_SCHEMA_VERSION, POLICY_BUDGET_SCHEMA_VERSION,
    PersistentBudgetLedger, PersistentBudgetPolicy, PinnedDescriptor, PlanArtifact, PlanAuthority,
    PlanEpoch, PlanResourceBudget, PolicyBudgetAnchor, PolicyBudgetConsumer, PolicyBudgetLimits,
    PolicyBudgetRequest, PolicyLeaseRule, ReplacementSupport, ReplayGapPolicy,
    ReproducibilityClaim, ResonanceEnvelope, ResonanceRelations, ResourceRef, ResourceSelector,
    RollingLimit, SemanticHash, Sensitivity, StopPolicy, ToxicCombinationRule, ToxicEffectPattern,
    TraitRequirement, TransitionBudget, TransitionContract, TransitionEffectClosure,
    TransitionGuaranteeFloor, TransitionKind, TransitionLevel, TransitionPhase,
    TransitionRecoveryPolicy, TransitionReplayContract, TransitionStateContract, TransitionUsage,
    TypeContractRef, resolve_authority, transition_effect_closure_subject,
};
use conduit_http::{
    HTTP_IN_MEMORY_IMPLEMENTATION_ID, HttpExchangeEvent, HttpProtocol, HttpResponsePart,
    HttpSecurityMode, HttpServiceLimits, HttpServingAuthority, HttpServingCapabilities,
    InMemoryHttpTransitionGeneration, ResolvedHttpService,
};
use conduit_runtime::{
    CandidateAuthority, HostResolverPolicy, HostedDrainObservation, HostedGenerationBinding,
    HostedTransitionAdmission, HostedTransitionAdmissionError, HostedTransitionGeneration,
    HostedTransitionTransaction, PlacementCandidate, PlacementRequest, ResolvedReplacementSupport,
    ResolverTiePolicy, RetainedReplayItem, RetainedReplayProvider, RuntimeValueEnvelope,
    StableBoundaryRouter, admit_hosted_transition, resolve_host_placement,
};

const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const TRANSITION_FIXTURE: &str = include_str!("../../../conformance/c5/plan-transitions-v1.json");
const CONTRACT_PIN: PinnedDescriptor<'static> = pin("fixture/semantic-service", 20);
const PROFILE_PIN: PinnedDescriptor<'static> = pin("fixture/hosted-profile", 21);
const REPORTER: PinnedDescriptor<'static> = pin("fixture/reporter", 22);
const REPORT_TRUST: PinnedDescriptor<'static> = pin("fixture/report-trust", 23);
const RESOLVER: PinnedDescriptor<'static> = pin("fixture/resolver", 24);
const CANDIDATE_ARTIFACT: ArtifactDigest = ArtifactDigest::from_bytes([25; 32]);
const ARTIFACT_REF: ManifestArtifactRef<'static> = ManifestArtifactRef {
    id: Id("fixture/candidate-artifact"),
    digest: CANDIDATE_ARTIFACT,
    role: Id("executable"),
    required: true,
};

const fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

const fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 1,
        semantic_hash: hash(byte),
    }
}

const fn resources(memory: u64) -> PlanResourceBudget {
    PlanResourceBudget {
        memory_bytes: memory,
        storage_bytes: 8,
        cpu_units: 1,
        timers: 2,
        transports: 1,
        checkpoints: 1,
        evidence_bytes: 2048,
    }
}

fn artifact_manifest() -> ArtifactManifest<'static> {
    let mut value = ArtifactManifest {
        schema_version: 1,
        identity: ZERO,
        id: ARTIFACT_REF.id,
        digest: CANDIDATE_ARTIFACT,
        media_type: "application/octet-stream",
        byte_size: 64,
        target: None,
        abi: None,
        provenance: ArtifactProvenance {
            builder: Id("fixture/builder"),
            source_digest: ArtifactDigest::from_bytes([26; 32]),
            build_recipe_digest: ArtifactDigest::from_bytes([27; 32]),
            reproducible: true,
        },
        signatures: &[],
        license_expressions: &["Apache-2.0"],
        notices: &[],
        sbom: None,
        source: None,
        related_artifacts: &[],
        locations: &[],
    };
    value.identity = value.computed_semantic_hash(&mut [ZERO; 1]).unwrap();
    value
}

fn candidate_manifest() -> ImplementationManifest<'static> {
    let mut value = ImplementationManifest {
        schema_version: 1,
        identity: ZERO,
        id: Id("fixture/candidate"),
        implementation_version: "1",
        semantic_contract: CONTRACT_PIN,
        executor: ExecutorKind::NativeInProcess,
        entrypoint: ManifestEntrypoint {
            name: Id("run"),
            adapter: Id("conduit-step-v1"),
            abi: Id("fixture-abi-v1"),
            protocol_version: 1,
        },
        execution_profile: PROFILE_PIN,
        artifacts: core::slice::from_ref(&ARTIFACT_REF),
        required_interfaces: &[],
        provided_interfaces: &[],
        required_authorities: &[],
        required_effects: &[],
        minimum_plan_version: 1,
        maximum_plan_version: 32,
        minimum_runtime_protocol: 1,
        maximum_runtime_protocol: 1,
        replacement: ReplacementSupport::Stateful {
            state_contract: pin("fixture/state", 8),
            maximum_export_bytes: 32,
            maximum_import_bytes: 32,
            maximum_ticks: 100,
        },
        coexistence_memory_bytes: 128,
        reproducibility: Some(ReproducibilityClaim {
            source_digest: ArtifactDigest::from_bytes([26; 32]),
            build_recipe_digest: ArtifactDigest::from_bytes([27; 32]),
            expected_artifact_digest: CANDIDATE_ARTIFACT,
        }),
    };
    value.identity = value.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    value
}

fn quiescent_candidate_manifest(
    boundary: PinnedDescriptor<'static>,
) -> ImplementationManifest<'static> {
    let mut value = candidate_manifest();
    value.id = Id("conduit/http.in-memory");
    value.replacement = ReplacementSupport::Quiescent {
        boundary,
        maximum_ticks: 100,
    };
    value.identity = value.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    value
}

fn tongues_tts_candidate_manifest() -> ImplementationManifest<'static> {
    let mut value = candidate_manifest();
    value.id = Id("fixture/tongues-tts-candidate");
    value.replacement = ReplacementSupport::Quiescent {
        boundary: pin("tongues/utterance-boundary", 152),
        maximum_ticks: 100,
    };
    value.identity = value.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    value
}

fn contract(candidate_manifest: &ImplementationManifest<'static>) -> TransitionContract<'static> {
    const FLOOR: TransitionGuaranteeFloor = TransitionGuaranteeFloor {
        semantic_contract: CONTRACT_PIN.semantic_hash,
        authority: hash(31),
        sensitivity: hash(32),
        delivery: hash(33),
        memory: hash(34),
        security: hash(35),
        committedness: hash(36),
    };
    static OPTIONAL: [OptionalCharacteristicChange<'static>; 1] = [OptionalCharacteristicChange {
        characteristic: pin("fixture/latency-class", 37),
        old_value: hash(38),
        new_value: hash(39),
        weakened: false,
    }];
    let old = resources(64);
    let candidate = resources(80);
    let rollback = resources(48);
    let overlap_reserved = PlanResourceBudget {
        memory_bytes: 192,
        storage_bytes: 24,
        cpu_units: 3,
        timers: 6,
        transports: 3,
        checkpoints: 3,
        evidence_bytes: 6144,
    };
    let mut value = TransitionContract {
        schema_version: PLAN_TRANSITION_SCHEMA_VERSION,
        identity: ZERO,
        old: PlanEpoch {
            plan: hash(1),
            epoch: 4,
        },
        candidate: PlanEpoch {
            plan: hash(2),
            epoch: 5,
        },
        stable_subject: InstancePath::new("root/service").unwrap(),
        old_implementation: pin("fixture/old", 3),
        candidate_implementation: PinnedDescriptor {
            id: candidate_manifest.id,
            schema_version: candidate_manifest.schema_version,
            semantic_hash: candidate_manifest.identity,
        },
        old_artifact: ArtifactDigest::from_bytes([4; 32]),
        candidate_artifact: CANDIDATE_ARTIFACT,
        kind: TransitionKind::ImplementationReplacement,
        level: TransitionLevel::Stateful,
        boundary: pin("fixture/segment-boundary", 7),
        state: Some(TransitionStateContract {
            descriptor: pin("fixture/state", 8),
            maximum_export_bytes: 32,
            maximum_import_bytes: 32,
            sensitivity: pin("fixture/restricted", 9),
            authority: pin("fixture/state-authority", 10),
        }),
        replay: Some(TransitionReplayContract {
            stream: pin("fixture/retained-input", 11),
            stream_epoch: 3,
            first_cursor: 9,
            maximum_items: 4,
            maximum_bytes: 64,
            duplicates_permitted: true,
            gap_policy: ReplayGapPolicy::Rollback,
        }),
        discontinuity_permitted: false,
        required_floor: FLOOR,
        candidate_floor: FLOOR,
        optional_changes: &OPTIONAL,
        mode_decision: None,
        budget: TransitionBudget {
            old,
            candidate,
            rollback,
            overlap_reserved,
            maximum_in_flight_values: 4,
            maximum_pending_operations: 2,
            maximum_replay_items: 4,
            maximum_replay_bytes: 64,
            maximum_state_bytes: 64,
            maximum_evidence_records: 32,
            maximum_ticks: 100,
        },
        recovery: TransitionRecoveryPolicy {
            maximum_attempts: 2,
            cooldown_ticks: 10,
            hysteresis_ticks: 5,
        },
    };
    value.identity = value.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    value
}

fn quiescent_contract(
    candidate_manifest: &ImplementationManifest<'static>,
) -> TransitionContract<'static> {
    let mut value = contract(candidate_manifest);
    value.old_implementation = pin("conduit/http.in-memory", 3);
    value.level = TransitionLevel::Quiescent;
    value.boundary = pin("conduit.http/request-boundary", 7);
    value.state = None;
    value.replay = None;
    value.identity = value.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    value
}

fn tongues_tts_contract(
    candidate_manifest: &ImplementationManifest<'static>,
) -> TransitionContract<'static> {
    let mut value = contract(candidate_manifest);
    value.level = TransitionLevel::Quiescent;
    value.boundary = pin("tongues/utterance-boundary", 152);
    value.state = None;
    value.replay = None;
    value.stable_subject = InstancePath::new("root/tongues-tts").unwrap();
    value.identity = value.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    value
}

fn resolution(
    contract: TransitionContract<'static>,
    implementation: &ImplementationManifest<'_>,
) -> conduit_runtime::ResolvedPlacement {
    let artifact = artifact_manifest();
    let executors = [ExecutorKind::NativeInProcess];
    let mut report = CapabilityReport {
        schema_version: CAPABILITY_REPORT_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("fixture/report"),
        host: Id("host.effect"),
        reporter: REPORTER,
        trust: REPORT_TRUST,
        membership: None,
        time_basis: Id("clock.monotonic"),
        observed_at_tick: 10,
        valid_until_tick: 40,
        available: resources(1024),
        capabilities: &[],
        resources: &[],
        topology: &[],
        supported_executors: &executors,
        supported_targets: &[],
        supported_abis: &[],
        minimum_plan_version: 1,
        maximum_plan_version: 32,
        current_constraints: &[],
    };
    report.identity = report.computed_semantic_hash(&mut [ZERO; 8]).unwrap();
    let artifacts = [&artifact];
    let candidate = PlacementCandidate {
        manifest: implementation,
        artifacts: &artifacts,
        report: &report,
        allocation: contract.budget.candidate,
        capabilities: &[],
        resources: &[],
        topology: &[],
        authorities: &[CandidateAuthority {
            requirement: hash(60),
            grant: Some(Id("grant.transition")),
            allowed: true,
        }],
    };
    let candidates = [candidate];
    let request = PlacementRequest {
        instance: contract.stable_subject,
        semantic_contract: CONTRACT_PIN,
        candidates: &candidates,
    };
    let mut policy = HostResolverPolicy {
        resolver: RESOLVER,
        policy_hash: ZERO,
        time_basis: Id("clock.monotonic"),
        current_tick: 20,
        plan_version: 1,
        trusted_reporters: &[REPORTER],
        trusted_report_trust: &[REPORT_TRUST.semantic_hash],
        required_realm: None,
        trusted_entities: &[],
        trusted_status_reporters: &[],
        require_active_passport: false,
        allowed_implementations: &[implementation.id],
        implementation_preference: &[implementation.id],
        tie_policy: ResolverTiePolicy::RejectAmbiguous,
        maximum_search_states: 8,
    };
    policy.policy_hash = policy.computed_semantic_hash().unwrap();
    resolve_host_placement(&[request], policy).unwrap()
}

fn principal(
    entity: &'static str,
    key: &'static str,
    plan: u8,
) -> AdministrativePrincipal<'static> {
    AdministrativePrincipal {
        realm: Id("realm.alpha"),
        entity: Id(entity),
        key: Id(key),
        profile: pin("profile.member", 70),
        source_plan: hash(plan),
        source_epoch: 1,
    }
}

fn authorization(
    contract: TransitionContract<'static>,
    effect_class: PinnedDescriptor<'static>,
    subject: AdministrativeSubject<'static>,
    self_approve: bool,
) -> AdministrativeProof<'static> {
    let requester = principal("requester", "key.requester", 71);
    let approver = if self_approve {
        requester
    } else {
        principal("approver", "key.approver", 72)
    };
    let committer = principal("committer", "key.committer", 73);
    let executor = principal("executor", "key.executor", 74);
    let failure = pin("failure.independent", 75);
    let approvers: &'static [AdministrativeApprover<'static>] = Box::leak(
        vec![AdministrativeApprover {
            realm: approver.realm,
            entity: approver.entity,
            key: approver.key,
            profile: approver.profile,
            failure_domain: failure,
        }]
        .into_boxed_slice(),
    );
    let mut policy = ContainmentPolicy {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("policy.transition", 76),
        effect_class,
        approvers,
        committer: AdministrativeApprover {
            realm: committer.realm,
            entity: committer.entity,
            key: committer.key,
            profile: committer.profile,
            failure_domain: pin("failure.committer", 77),
        },
        executor: AdministrativeApprover {
            realm: executor.realm,
            entity: executor.entity,
            key: executor.key,
            profile: executor.profile,
            failure_domain: pin("failure.executor", 78),
        },
        minimum_approvals: 1,
        minimum_failure_domains: 1,
        requester_independence: true,
        beneficiary_independence: true,
        successor_independence: true,
        delegation_ceiling: None,
        ceremony: None,
    };
    policy.identity = policy.computed_semantic_hash().unwrap();
    let beneficiaries: &'static [AdministrativeSubject<'static>] =
        Box::leak(vec![subject].into_boxed_slice());
    let mut proposal = AdministrativeProposal {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("proposal.transition"),
        effect_class,
        operation: pin("operation.activate-successor", 79),
        requester,
        subject,
        beneficiaries,
        predecessor_plan: Some(contract.old.plan),
        delegation: None,
        protected_handle: None,
        ceremony: None,
        time_basis: Id("clock.monotonic"),
        created_at_tick: 10,
        expires_at_tick: 35,
    };
    proposal.identity = proposal.computed_semantic_hash().unwrap();
    let mut approval = AdministrativeApproval {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("approval.transition"),
        proposal_identity: proposal.identity,
        policy_identity: policy.identity,
        approver,
        failure_domain: failure,
        time_basis: Id("clock.monotonic"),
        issued_at_tick: 12,
        expires_at_tick: 35,
        status: AdministrativeApprovalStatus::Current,
    };
    approval.identity = approval.computed_semantic_hash().unwrap();
    let approvals: &'static [AdministrativeApproval<'static>] =
        Box::leak(vec![approval].into_boxed_slice());
    let approval_hashes: &'static [SemanticHash] =
        Box::leak(vec![approval.identity].into_boxed_slice());
    let mut commit = AdministrativeCommit {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("commit.transition"),
        proposal_identity: proposal.identity,
        policy_identity: policy.identity,
        approvals: approval_hashes,
        committed_by: committer,
        committed_at_tick: 15,
    };
    commit.identity = commit.computed_semantic_hash().unwrap();
    let mut execution = AdministrativeExecution {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("execution.transition"),
        proposal_identity: proposal.identity,
        commit_identity: commit.identity,
        executor,
        time_basis: Id("clock.monotonic"),
        not_before_tick: 15,
        expires_at_tick: 35,
    };
    execution.identity = execution.computed_semantic_hash().unwrap();
    AdministrativeProof {
        proposal,
        policy,
        approvals,
        commit,
        execution,
    }
}

fn budget_policy() -> PersistentBudgetPolicy<'static> {
    let mut value = PersistentBudgetPolicy {
        schema_version: POLICY_BUDGET_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("budget.transitions", 80),
        owner: pin("owner.site", 81),
        subject: pin("subject.plan-epochs", 82),
        anchor: PolicyBudgetAnchor::Host(Id("host.effect")),
        action: Id("action.activate-successor"),
        resource_class: pin("resource.plan-epoch", 83),
        time_basis: Id("clock.monotonic"),
        limits: PolicyBudgetLimits {
            current_stock: Some(4),
            rolling: Some(RollingLimit {
                units: 4,
                window_ticks: 100,
            }),
            lifetime: Some(4),
        },
        reservation_ttl_ticks: 20,
        lease: Some(PolicyLeaseRule {
            maximum_ticks: 20,
            renewal_authority: pin("authority.budget-renewal", 84),
            offline_allowed: false,
        }),
        audit_id: Id("audit.transitions"),
        persistence_profile: pin("persistence.atomic", 85),
        maximum_reservations: 8,
        maximum_evidence_events: 64,
    };
    value.identity = value.computed_semantic_hash().unwrap();
    value
}

fn budget_request(
    policy: PersistentBudgetPolicy<'static>,
    contract: TransitionContract<'static>,
    correlation: SemanticHash,
) -> PolicyBudgetRequest<'static> {
    let mut value = PolicyBudgetRequest {
        identity: ZERO,
        correlation,
        policy_identity: policy.identity,
        consumer: PolicyBudgetConsumer {
            realm: Id("realm.alpha"),
            plan: contract.candidate.plan,
            epoch: contract.candidate.epoch,
            generation: 1,
            run: Id("run.transition"),
        },
        action: policy.action,
        units: 1,
        requested_at_tick: 20,
        lease: None,
    };
    value.identity = value.computed_semantic_hash().unwrap();
    value
}

fn hazard_authority(
    node: &'static str,
    effect_id: &'static str,
    class: EffectClassBinding<'static>,
) -> PlanAuthority<'static> {
    let constraints: &'static [AuthorityConstraintRef<'static>] =
        Box::leak(vec![class.constraint].into_boxed_slice());
    let effect = EffectRequirement {
        id: Id(effect_id),
        administrative_class: None,
        policy_budget_class: None,
        action: Id("action.use"),
        resource: ResourceSelector::Exact(ResourceRef {
            kind: Id("resource.fixture"),
            id: Id(effect_id),
        }),
        requester: InstancePath::new(node).unwrap(),
        audience: Id("runtime"),
        constraints,
        check_at_use: true,
    };
    let capability = HostCapability {
        id: Id(effect_id),
        action: effect.action,
        resource: match effect.resource {
            ResourceSelector::Exact(resource) => resource,
            ResourceSelector::Kind(_) => unreachable!(),
        },
        host: Id("host.effect"),
        time_basis: Id("clock.monotonic"),
        observed_at_tick: 1,
        valid_until_tick: 40,
    };
    let grant = AuthorityGrant {
        id: Id(effect_id),
        action: effect.action,
        resource: capability.resource,
        scope: AuthorityScope {
            root: effect.requester,
            descendants: false,
        },
        audience: effect.audience,
        constraints,
        time_basis: Id("clock.monotonic"),
        not_before_tick: 1,
        expires_at_tick: 40,
        issued_for_host: Id("host.effect"),
        delegation: DelegationPolicy::None,
        audit_id: Id("audit.fixture"),
        terminal_policy: StopPolicy::Abort,
    };
    let binding = resolve_authority(
        effect,
        Id("host.effect"),
        AuthorityTime {
            basis: Id("clock.monotonic"),
            tick: 20,
        },
        &[capability],
        &[ObservedGrant {
            grant,
            status: GrantStatus::Active,
        }],
    )
    .unwrap();
    PlanAuthority {
        node: effect.requester,
        effect_hash: effect.semantic_hash().unwrap(),
        grant_hash: grant.semantic_hash().unwrap(),
        effect,
        capability,
        grant,
        binding,
        administrative_subject: None,
        containment: None,
        policy_budgets: &[],
    }
}

fn hazard_facts() -> (
    HazardClosurePolicy<'static>,
    &'static [PlanAuthority<'static>],
    &'static [PlanAuthority<'static>],
) {
    let descriptor = pin("effect.fixture", 90);
    let constraint = AuthorityConstraintRef {
        id: descriptor.id,
        semantic_hash: descriptor.semantic_hash,
    };
    let mut class = EffectClassBinding {
        identity: ZERO,
        descriptor,
        constraint,
        traits: EffectClassTraits::default(),
    };
    class.identity = class.computed_semantic_hash().unwrap();
    let pattern = ToxicEffectPattern {
        id: Id("pattern.never-matches"),
        class: descriptor,
        resource: None,
        audience: None,
        host: Some(Id("host.not-selected")),
        realm: None,
        budget: None,
        persistence: TraitRequirement::Any,
        delegation: TraitRequirement::Any,
        distributed: TraitRequirement::Any,
        administrative: TraitRequirement::Any,
    };
    let patterns: &'static [ToxicEffectPattern<'static>] =
        Box::leak(vec![pattern].into_boxed_slice());
    let mut rule = ToxicCombinationRule {
        identity: ZERO,
        descriptor: pin("rule.forbidden-combination", 91),
        patterns,
        flows: &[],
    };
    rule.identity = rule.computed_semantic_hash().unwrap();
    let classes: &'static [EffectClassBinding<'static>] = Box::leak(vec![class].into_boxed_slice());
    let rules: &'static [ToxicCombinationRule<'static>] = Box::leak(vec![rule].into_boxed_slice());
    let mut policy = HazardClosurePolicy {
        schema_version: HAZARD_CLOSURE_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("policy.hazard", 92),
        permit_class: pin("effect.hazard-permit", 93),
        classes,
        rules,
        limits: HazardClosureLimits {
            maximum_effects: 8,
            maximum_classes: 4,
            maximum_rules: 4,
            maximum_patterns_per_rule: 4,
            maximum_flows: 4,
            maximum_permits: 2,
            maximum_proof_nodes: 8,
            maximum_search_steps: 32,
        },
    };
    policy.identity = policy.computed_semantic_hash().unwrap();
    let old: &'static [PlanAuthority<'static>] =
        Box::leak(vec![hazard_authority("root/old", "effect.old", class)].into_boxed_slice());
    let candidate: &'static [PlanAuthority<'static>] = Box::leak(
        vec![hazard_authority(
            "root/candidate",
            "effect.candidate",
            class,
        )]
        .into_boxed_slice(),
    );
    (policy, old, candidate)
}

fn inhibit_binding() -> HazardousHostBinding<'static> {
    let envelope: &'static [OperatingEnvelopeLimit<'static>] = Box::leak(
        vec![OperatingEnvelopeLimit {
            dimension: pin("fixture/hazard-dimension", 130),
            minimum: 0,
            maximum: 10,
        }]
        .into_boxed_slice(),
    );
    let mut profile = HazardousHostProfile {
        schema_version: HAZARDOUS_HOST_PROFILE_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("fixture/hazardous-profile", 131),
        safe_state: pin("fixture/safe-state", 132),
        inhibit_boundary: pin("fixture/inhibit-boundary", 133),
        watchdog: pin("fixture/watchdog", 134),
        effect_boundary: pin("fixture/effect-boundary", 135),
        command_effect_class: pin("fixture/command-effect", 136),
        clear_effect_class: pin("fixture/clear-effect", 137),
        clear_operation: pin("fixture/clear-operation", 138),
        clear_ceremony: pin("fixture/clear-ceremony", 139),
        time_basis: Id("clock.monotonic"),
        maximum_command_horizon_ticks: 10,
        maximum_observation_age_ticks: 20,
        maximum_evidence_records: 16,
        require_physical_presence_to_clear: true,
        require_isolated_implementation: true,
        envelope,
    };
    profile.identity = profile.computed_semantic_hash(&mut [ZERO; 4]).unwrap();
    let mut observation = InhibitObservation {
        schema_version: INHIBIT_OBSERVATION_SCHEMA_VERSION,
        identity: ZERO,
        profile_identity: profile.identity,
        host: Id("host.effect"),
        safe_state: profile.safe_state,
        inhibit_boundary: profile.inhibit_boundary,
        watchdog: profile.watchdog,
        effect_boundary: profile.effect_boundary,
        time_basis: profile.time_basis,
        observed_at_tick: 10,
        valid_until_tick: 35,
        latch_generation: 1,
        latch_state: InhibitLatchState::SafeDisarmed,
        independent_from_plan: true,
        local_safe_path: true,
        survives_executor_loss: true,
        survives_partition: true,
        graph_cannot_replace: true,
        confinement: ImplementationConfinement::EffectBoundaryEnforced,
    };
    observation.identity = observation.computed_semantic_hash().unwrap();
    HazardousHostBinding {
        host: observation.host,
        profile,
        observation,
    }
}

fn toxic_overlap_policy(baseline: HazardClosurePolicy<'static>) -> HazardClosurePolicy<'static> {
    let class = baseline.classes[0];
    let patterns: &'static [ToxicEffectPattern<'static>] = Box::leak(
        vec![
            ToxicEffectPattern {
                id: Id("pattern.old-generation"),
                class: class.descriptor,
                resource: None,
                audience: None,
                host: Some(Id("host.effect")),
                realm: None,
                budget: None,
                persistence: TraitRequirement::Any,
                delegation: TraitRequirement::Any,
                distributed: TraitRequirement::Any,
                administrative: TraitRequirement::Any,
            },
            ToxicEffectPattern {
                id: Id("pattern.candidate-generation"),
                class: class.descriptor,
                resource: None,
                audience: None,
                host: Some(Id("host.effect")),
                realm: None,
                budget: None,
                persistence: TraitRequirement::Any,
                delegation: TraitRequirement::Any,
                distributed: TraitRequirement::Any,
                administrative: TraitRequirement::Any,
            },
        ]
        .into_boxed_slice(),
    );
    let mut rule = ToxicCombinationRule {
        identity: ZERO,
        descriptor: pin("rule.overlap-toxic", 140),
        patterns,
        flows: &[],
    };
    rule.identity = rule.computed_semantic_hash().unwrap();
    let rules: &'static [ToxicCombinationRule<'static>] = Box::leak(vec![rule].into_boxed_slice());
    let mut policy = HazardClosurePolicy { rules, ..baseline };
    policy.identity = policy.computed_semantic_hash().unwrap();
    policy
}

fn control(
    event: &'static str,
    contract: TransitionContract<'static>,
    integrity: u8,
    caused_by: Option<Id<'static>>,
) -> ResonanceEnvelope<'static> {
    ResonanceEnvelope {
        event: Id(event),
        stream: Id("stream.transition-control"),
        run: Id("run.transition"),
        plan_epoch: contract.old.plan,
        producer: InstancePath::new(if caused_by.is_none() {
            "operator/control"
        } else {
            "authority/control"
        })
        .unwrap(),
        subject: contract.stable_subject,
        class: EventClass::Control,
        sequence: u64::from(integrity),
        observer: Id("observer.control"),
        observer_sequence: u64::from(integrity),
        domain_time: Some((Id("clock.monotonic"), 20)),
        correlation: Some(Id("correlation.transition")),
        idempotency: Some(Id(event)),
        payload_type: TypeContractRef {
            contract_id: Id("conduit/plan-transition-control"),
            schema_version: 1,
            semantic_hash: hash(94),
        },
        payload: EventPayloadRef::ContentAddressed {
            digest: ArtifactDigest::from_bytes([integrity; 32]),
            bytes: 32,
        },
        relations: ResonanceRelations {
            caused_by,
            derived_from: &[],
            supersedes: None,
            corrects: None,
            retracts: None,
        },
        provenance: Id("provenance.control"),
        recording_authority: Some(Id("authority.transition-control")),
        sensitivity: Sensitivity::Restricted,
        integrity: hash(integrity),
    }
}

struct FixtureGeneration {
    binding: HostedGenerationBinding<'static>,
    state: Vec<u8>,
    replayed: Vec<(u64, Vec<u8>, Option<RuntimeValueEnvelope>)>,
    prepared: bool,
    admission_stopped: bool,
    retired: bool,
    fail_prepare: bool,
    fail_drain: bool,
}

impl HostedTransitionGeneration for FixtureGeneration {
    fn binding(&self) -> HostedGenerationBinding<'_> {
        self.binding
    }

    fn prepare(&mut self) -> Result<(), Id<'static>> {
        if self.fail_prepare {
            return Err(Id("fixture/prepare-failed"));
        }
        self.prepared = true;
        Ok(())
    }

    fn stop_admission(&mut self, _: PinnedDescriptor<'_>) -> Result<(), Id<'static>> {
        self.admission_stopped = true;
        Ok(())
    }

    fn drain(&mut self, _: PinnedDescriptor<'_>) -> Result<HostedDrainObservation, Id<'static>> {
        if self.fail_drain {
            return Err(Id("fixture/drain-failed"));
        }
        Ok(HostedDrainObservation {
            remaining_values: 0,
            remaining_operations: 0,
            drained_values: 0,
            rejected_values: 0,
            lost_values: 0,
            completed_operations: 0,
            cancelled_operations: 0,
        })
    }

    fn export_state(
        &mut self,
        _: TransitionStateContract<'_>,
        output: &mut [u8],
    ) -> Result<usize, Id<'static>> {
        output[..self.state.len()].copy_from_slice(&self.state);
        Ok(self.state.len())
    }

    fn import_state(
        &mut self,
        _: TransitionStateContract<'_>,
        input: &[u8],
    ) -> Result<usize, Id<'static>> {
        self.state = input.to_vec();
        Ok(input.len())
    }

    fn accept_replayed_value(
        &mut self,
        cursor: u64,
        value: &[u8],
        envelope: Option<RuntimeValueEnvelope>,
        _: bool,
    ) -> Result<(), Id<'static>> {
        self.replayed.push((cursor, value.to_vec(), envelope));
        Ok(())
    }

    fn retire(&mut self) -> Result<(), Id<'static>> {
        self.retired = true;
        Ok(())
    }

    fn abort_candidate(&mut self) -> Result<(), Id<'static>> {
        self.prepared = false;
        Ok(())
    }

    fn restore_old(&mut self) -> Result<(), Id<'static>> {
        self.admission_stopped = false;
        Ok(())
    }
}

struct FixtureRouter {
    active: PlanEpoch,
    admissions: PlanEpoch,
}

impl StableBoundaryRouter for FixtureRouter {
    fn begin_handoff(
        &mut self,
        _: &str,
        _: PinnedDescriptor<'_>,
        old: PlanEpoch,
        candidate: PlanEpoch,
    ) -> Result<(), Id<'static>> {
        if self.active != old || self.admissions != old {
            return Err(Id("fixture/router-stale"));
        }
        self.admissions = candidate;
        Ok(())
    }

    fn rebind(
        &mut self,
        _: &str,
        _: PinnedDescriptor<'_>,
        old: PlanEpoch,
        candidate: PlanEpoch,
    ) -> Result<(), Id<'static>> {
        if self.active != old {
            return Err(Id("fixture/router-stale"));
        }
        if self.admissions != candidate {
            return Err(Id("fixture/admission-handoff-missing"));
        }
        self.active = candidate;
        Ok(())
    }

    fn restore(
        &mut self,
        _: &str,
        _: PinnedDescriptor<'_>,
        old: PlanEpoch,
        _: PlanEpoch,
    ) -> Result<(), Id<'static>> {
        self.active = old;
        self.admissions = old;
        Ok(())
    }
}

struct FixtureReplay {
    index: usize,
}

impl RetainedReplayProvider for FixtureReplay {
    fn stream(&self) -> PinnedDescriptor<'_> {
        pin("fixture/retained-input", 11)
    }

    fn stream_epoch(&self) -> u64 {
        3
    }

    fn first_cursor(&self) -> u64 {
        9
    }

    fn next(&mut self, output: &mut [u8]) -> Result<Option<RetainedReplayItem>, Id<'static>> {
        let values: [&[u8]; 2] = [b"segment-a", b"segment-b"];
        let Some(value) = values.get(self.index) else {
            return Ok(None);
        };
        output[..value.len()].copy_from_slice(value);
        let item = RetainedReplayItem {
            cursor: 9 + self.index as u64,
            bytes: value.len(),
            redelivered: false,
            gap: false,
            value_envelope: Some(RuntimeValueEnvelope {
                representation: hash(80),
                envelope_bytes: 32,
                fragment_count: 1,
                fragment_bytes: value.len() as u32,
                identity: Some(hash(81)),
                correlation: Some(hash(82)),
                causation: None,
                provenance: Some(hash(83)),
                timestamp_count: 0,
                timestamps: [conduit_runtime::RuntimeTimestamp::default();
                    conduit_core::MAX_VALUE_CLOCK_DOMAINS],
                sensitivity: Sensitivity::Restricted,
            }),
        };
        self.index += 1;
        Ok(Some(item))
    }
}

struct AdmissionFixture {
    contract: TransitionContract<'static>,
    request: ResonanceEnvelope<'static>,
    decision: ResonanceEnvelope<'static>,
    authorization: AdministrativeProof<'static>,
    resolution: conduit_runtime::ResolvedPlacement,
    budget_policy: PersistentBudgetPolicy<'static>,
    hazard_policy: HazardClosurePolicy<'static>,
    old_authorities: &'static [PlanAuthority<'static>],
    candidate_authorities: &'static [PlanAuthority<'static>],
}

fn admission_fixture() -> AdmissionFixture {
    let manifest = candidate_manifest();
    let contract = contract(&manifest);
    admission_fixture_from(&manifest, contract)
}

fn admission_fixture_from(
    manifest: &ImplementationManifest<'static>,
    contract: TransitionContract<'static>,
) -> AdmissionFixture {
    let request = control("event.transition-request", contract, 100, None);
    let decision = control(
        "event.transition-decision",
        contract,
        101,
        Some(request.event),
    );
    let effect_class = pin("effect.activate-successor", 102);
    let budget_policy = budget_policy();
    let subject = AdministrativeSubject {
        realm: Id("realm.alpha"),
        entity: Id("service.owner"),
        plan: contract.candidate.plan,
        epoch: contract.candidate.epoch,
        artifact: Some(contract.candidate_artifact),
        budget: None,
    };
    let authorization = authorization(contract, effect_class, subject, false);
    let resolution = resolution(contract, manifest);
    let (hazard_policy, old_authorities, candidate_authorities) = hazard_facts();
    AdmissionFixture {
        contract,
        request,
        decision,
        authorization,
        resolution,
        budget_policy,
        hazard_policy,
        old_authorities,
        candidate_authorities,
    }
}

fn admit(
    fixture: &AdmissionFixture,
    ledger: &mut PersistentBudgetLedger<'static, 8>,
) -> Result<conduit_runtime::HostedTransitionReservation, HostedTransitionAdmissionError> {
    admit_with(fixture, ledger, None, false, false)
}

fn admit_with(
    fixture: &AdmissionFixture,
    ledger: &mut PersistentBudgetLedger<'static, 8>,
    inhibit: Option<conduit_core::HazardousHostBinding<'static>>,
    inhibit_required: bool,
    stale_budget_status: bool,
) -> Result<conduit_runtime::HostedTransitionReservation, HostedTransitionAdmissionError> {
    let mut status = ledger
        .status(pin("ledger.transitions", 103), 20, 35)
        .unwrap();
    if stale_budget_status {
        status.valid_until_tick = 20;
        status.identity = status.computed_semantic_hash().unwrap();
    }
    let subject = fixture.authorization.proposal.subject;
    let request = budget_request(
        fixture.budget_policy,
        fixture.contract,
        fixture.request.integrity,
    );
    admit_hosted_transition(
        HostedTransitionAdmission {
            contract: fixture.contract,
            request: fixture.request,
            decision: fixture.decision,
            effect_class: fixture.authorization.proposal.effect_class,
            authorization: fixture.authorization,
            containment: ContainmentContext {
                subject,
                time_basis: Id("clock.monotonic"),
                now_tick: 20,
            },
            resolution: &fixture.resolution,
            budget_policy: fixture.budget_policy,
            budget_status: status,
            budget_request: request,
            budget_ledger_available: true,
            hazard_policy: fixture.hazard_policy,
            effect_closure: TransitionEffectClosure {
                old_authorities: fixture.old_authorities,
                new_and_rollback_authorities: fixture.candidate_authorities,
                old_flows: &[],
                new_and_rollback_flows: &[],
            },
            hazard_permits: &[],
            hazard_context: HazardClosureContext {
                plan_subject: transition_effect_closure_subject(
                    fixture.old_authorities,
                    fixture.candidate_authorities,
                    &[],
                    &[],
                    fixture.contract.candidate.epoch,
                    Id("clock.monotonic"),
                )
                .unwrap(),
                epoch: fixture.contract.candidate.epoch,
                time: AuthorityTime {
                    basis: Id("clock.monotonic"),
                    tick: 20,
                },
            },
            inhibit,
            inhibit_required,
            now: AuthorityTime {
                basis: Id("clock.monotonic"),
                tick: 20,
            },
        },
        ledger,
        &mut [None; 8],
        &mut [ZERO; 4],
    )
}

fn generation(binding: HostedGenerationBinding<'static>, state: &[u8]) -> FixtureGeneration {
    FixtureGeneration {
        binding,
        state: state.to_vec(),
        replayed: Vec::new(),
        prepared: false,
        admission_stopped: false,
        retired: false,
        fail_prepare: false,
        fail_drain: false,
    }
}

fn stateful_binding(
    contract: TransitionContract<'static>,
    candidate: bool,
) -> HostedGenerationBinding<'static> {
    HostedGenerationBinding {
        epoch: if candidate {
            contract.candidate
        } else {
            contract.old
        },
        implementation: if candidate {
            contract.candidate_implementation
        } else {
            contract.old_implementation
        },
        artifact: if candidate {
            contract.candidate_artifact
        } else {
            contract.old_artifact
        },
        replacement: ReplacementSupport::Stateful {
            state_contract: contract.state.unwrap().descriptor,
            maximum_export_bytes: 32,
            maximum_import_bytes: 32,
            maximum_ticks: 100,
        },
    }
}

fn empty_usage(contract: TransitionContract<'static>) -> TransitionUsage {
    TransitionUsage {
        overlap: contract.budget.overlap_reserved,
        in_flight_values: 0,
        pending_operations: 0,
        drained_values: 0,
        rejected_values: 0,
        lost_values: 0,
        completed_operations: 0,
        cancelled_operations: 0,
        replay_items: 0,
        replay_bytes: 0,
        duplicate_replay_items: 0,
        state_bytes: 0,
    }
}

#[test]
fn tongues_asr_segment_transition_uses_opaque_state_retained_replay_and_real_admission() {
    let fixture = admission_fixture();
    let mut ledger = PersistentBudgetLedger::<8>::new(fixture.budget_policy, hash(110), 0).unwrap();
    let reservation = admit(&fixture, &mut ledger).unwrap();
    let old = generation(
        HostedGenerationBinding {
            epoch: fixture.contract.old,
            implementation: fixture.contract.old_implementation,
            artifact: fixture.contract.old_artifact,
            replacement: ReplacementSupport::Stateful {
                state_contract: fixture.contract.state.unwrap().descriptor,
                maximum_export_bytes: 32,
                maximum_import_bytes: 32,
                maximum_ticks: 100,
            },
        },
        b"bounded-state",
    );
    let candidate = generation(
        HostedGenerationBinding {
            epoch: fixture.contract.candidate,
            implementation: fixture.contract.candidate_implementation,
            artifact: fixture.contract.candidate_artifact,
            replacement: ReplacementSupport::Stateful {
                state_contract: fixture.contract.state.unwrap().descriptor,
                maximum_export_bytes: 32,
                maximum_import_bytes: 32,
                maximum_ticks: 100,
            },
        },
        b"",
    );
    let mut transaction = HostedTransitionTransaction::<_, _, _, 64>::new(
        fixture.contract,
        fixture.contract.old,
        old,
        candidate,
        FixtureRouter {
            active: fixture.contract.old,
            admissions: fixture.contract.old,
        },
        20,
        &mut [ZERO; 2],
    )
    .unwrap();
    transaction
        .reserve(
            reservation,
            TransitionUsage {
                overlap: fixture.contract.budget.overlap_reserved,
                in_flight_values: 0,
                pending_operations: 0,
                drained_values: 0,
                rejected_values: 0,
                lost_values: 0,
                completed_operations: 0,
                cancelled_operations: 0,
                replay_items: 0,
                replay_bytes: 0,
                duplicate_replay_items: 0,
                state_bytes: 0,
            },
            21,
        )
        .unwrap();
    transaction.prepare(22).unwrap();
    transaction.barrier(23).unwrap();
    transaction.drain(24).unwrap();
    transaction.transfer_state(&mut [0_u8; 32], 25).unwrap();
    transaction
        .replay(&mut FixtureReplay { index: 0 }, &mut [0_u8; 16], 26)
        .unwrap();
    transaction.rebind(27).unwrap();
    assert_eq!(transaction.active_epoch(), fixture.contract.old);
    transaction.commit(&mut ledger, 28).unwrap();
    assert_eq!(transaction.active_epoch(), fixture.contract.candidate);
    transaction.retire_old(29).unwrap();
    transaction.complete(30).unwrap();
    assert_eq!(transaction.phase(), TransitionPhase::Completed);
    let checkpoint = ledger.checkpoint();
    assert_eq!(checkpoint.current_stock, 1);
    assert_eq!(checkpoint.lifetime_committed, 1);
    let (old, candidate, router) = transaction.into_parts();
    assert!(old.retired);
    assert_eq!(candidate.state, b"bounded-state");
    assert_eq!(candidate.replayed.len(), 2);
    assert_eq!(
        candidate.replayed[0].2,
        Some(RuntimeValueEnvelope {
            representation: hash(80),
            envelope_bytes: 32,
            fragment_count: 1,
            fragment_bytes: 9,
            identity: Some(hash(81)),
            correlation: Some(hash(82)),
            causation: None,
            provenance: Some(hash(83)),
            timestamp_count: 0,
            timestamps: [conduit_runtime::RuntimeTimestamp::default();
                conduit_core::MAX_VALUE_CLOCK_DOMAINS],
            sensitivity: Sensitivity::Restricted,
        })
    );
    assert_eq!(router.active, fixture.contract.candidate);
}

#[test]
fn tongues_tts_transition_is_quiescent_at_the_opaque_utterance_boundary() {
    let manifest = tongues_tts_candidate_manifest();
    let contract = tongues_tts_contract(&manifest);
    let fixture = admission_fixture_from(&manifest, contract);
    let mut ledger = PersistentBudgetLedger::<8>::new(fixture.budget_policy, hash(110), 0).unwrap();
    let reservation = admit(&fixture, &mut ledger).unwrap();
    let replacement = ReplacementSupport::Quiescent {
        boundary: contract.boundary,
        maximum_ticks: 100,
    };
    let old = generation(
        HostedGenerationBinding {
            epoch: contract.old,
            implementation: contract.old_implementation,
            artifact: contract.old_artifact,
            replacement,
        },
        b"",
    );
    let candidate = generation(
        HostedGenerationBinding {
            epoch: contract.candidate,
            implementation: contract.candidate_implementation,
            artifact: contract.candidate_artifact,
            replacement,
        },
        b"",
    );
    let mut transaction = HostedTransitionTransaction::<_, _, _, 64>::new(
        contract,
        contract.old,
        old,
        candidate,
        FixtureRouter {
            active: contract.old,
            admissions: contract.old,
        },
        20,
        &mut [ZERO; 2],
    )
    .unwrap();
    transaction
        .reserve(
            reservation,
            TransitionUsage {
                overlap: contract.budget.overlap_reserved,
                in_flight_values: 0,
                pending_operations: 0,
                drained_values: 0,
                rejected_values: 0,
                lost_values: 0,
                completed_operations: 0,
                cancelled_operations: 0,
                replay_items: 0,
                replay_bytes: 0,
                duplicate_replay_items: 0,
                state_bytes: 0,
            },
            21,
        )
        .unwrap();
    transaction.prepare(22).unwrap();
    transaction.barrier(23).unwrap();
    transaction.drain(24).unwrap();
    transaction.rebind(25).unwrap();
    transaction.commit(&mut ledger, 26).unwrap();
    transaction.retire_old(27).unwrap();
    transaction.complete(28).unwrap();
    assert_eq!(transaction.phase(), TransitionPhase::Completed);
    assert!(transaction.evidence().iter().flatten().any(|event| {
        event.kind == conduit_core::TransitionEvidenceKind::AdmissionBarrier
            && event.boundary == Some(contract.boundary)
    }));
}

#[test]
fn stale_control_or_authorization_fails_before_budget_reservation() {
    let mut fixture = admission_fixture();
    fixture.decision.plan_epoch = hash(199);
    let mut ledger = PersistentBudgetLedger::<8>::new(fixture.budget_policy, hash(110), 0).unwrap();
    let before = ledger.checkpoint();
    assert_eq!(
        admit(&fixture, &mut ledger),
        Err(HostedTransitionAdmissionError::ControlMismatch)
    );
    assert_eq!(ledger.checkpoint(), before);
}

#[test]
fn self_approved_successor_fails_before_budget_reservation() {
    let mut fixture = admission_fixture();
    let subject = fixture.authorization.proposal.subject;
    fixture.authorization = authorization(
        fixture.contract,
        fixture.authorization.proposal.effect_class,
        subject,
        true,
    );
    let mut ledger = PersistentBudgetLedger::<8>::new(fixture.budget_policy, hash(110), 0).unwrap();
    let before = ledger.checkpoint();
    assert!(matches!(
        admit(&fixture, &mut ledger),
        Err(HostedTransitionAdmissionError::Containment(
            conduit_core::ContainmentReason::SelfSupporting
                | conduit_core::ContainmentReason::SuccessorSelfAuthorized
        ))
    ));
    assert_eq!(ledger.checkpoint(), before);
}

#[test]
fn stale_resolution_and_budget_status_fail_closed_without_ledger_mutation() {
    let mut fixture = admission_fixture();
    fixture.resolution.bindings[0].report_valid_until_tick = 20;
    let mut ledger = PersistentBudgetLedger::<8>::new(fixture.budget_policy, hash(110), 0).unwrap();
    let before = ledger.checkpoint();
    assert_eq!(
        admit(&fixture, &mut ledger),
        Err(HostedTransitionAdmissionError::Resolution)
    );
    assert_eq!(ledger.checkpoint(), before);

    let fixture = admission_fixture();
    assert!(matches!(
        admit_with(&fixture, &mut ledger, None, false, true),
        Err(HostedTransitionAdmissionError::Budget(
            conduit_core::PolicyBudgetReason::StaleStatus
        ))
    ));
    assert_eq!(ledger.checkpoint(), before);
}

#[test]
fn resolved_candidate_cannot_overstate_its_replacement_level() {
    let mut fixture = admission_fixture();
    fixture.resolution.bindings[0].replacement = ResolvedReplacementSupport::Cold;
    let mut ledger = PersistentBudgetLedger::<8>::new(fixture.budget_policy, hash(110), 0).unwrap();
    let before = ledger.checkpoint();
    assert_eq!(
        admit(&fixture, &mut ledger),
        Err(HostedTransitionAdmissionError::Replacement(
            conduit_core::TransitionReason::StateContractMismatch
        ))
    );
    assert_eq!(ledger.checkpoint(), before);
}

#[test]
fn combined_old_candidate_effect_closure_is_checked_before_prepare() {
    let mut fixture = admission_fixture();
    fixture.hazard_policy = toxic_overlap_policy(fixture.hazard_policy);
    let mut ledger = PersistentBudgetLedger::<8>::new(fixture.budget_policy, hash(110), 0).unwrap();
    let before = ledger.checkpoint();
    assert!(matches!(
        admit(&fixture, &mut ledger),
        Err(HostedTransitionAdmissionError::Hazard(
            conduit_core::HazardClosureReason::ToxicCombination
                | conduit_core::HazardClosureReason::PermitMissing
        ))
    ));
    assert_eq!(ledger.checkpoint(), before);
}

#[test]
fn required_inhibit_is_fresh_exact_and_independent() {
    let fixture = admission_fixture();
    let mut ledger = PersistentBudgetLedger::<8>::new(fixture.budget_policy, hash(110), 0).unwrap();
    let before = ledger.checkpoint();
    assert!(matches!(
        admit_with(&fixture, &mut ledger, None, true, false),
        Err(HostedTransitionAdmissionError::Inhibit(
            conduit_core::InhibitReason::ObservationAbsentOrStale
        ))
    ));
    assert_eq!(ledger.checkpoint(), before);

    let reservation =
        admit_with(&fixture, &mut ledger, Some(inhibit_binding()), true, false).unwrap();
    assert_ne!(reservation.proofs().inhibit_decision, ZERO);

    let mut stale = inhibit_binding();
    stale.observation.valid_until_tick = 20;
    stale.observation.identity = stale.observation.computed_semantic_hash().unwrap();
    let checkpoint = ledger.checkpoint();
    assert!(matches!(
        admit_with(&fixture, &mut ledger, Some(stale), true, false),
        Err(HostedTransitionAdmissionError::Inhibit(
            conduit_core::InhibitReason::ObservationAbsentOrStale
        ))
    ));
    assert_eq!(ledger.checkpoint(), checkpoint);
}

#[test]
fn rollback_releases_stock_without_replenishing_lifetime_or_attempt_state() {
    let fixture = admission_fixture();
    let mut ledger = PersistentBudgetLedger::<8>::new(fixture.budget_policy, hash(110), 0).unwrap();
    let reservation = admit(&fixture, &mut ledger).unwrap();
    let old = generation(
        HostedGenerationBinding {
            epoch: fixture.contract.old,
            implementation: fixture.contract.old_implementation,
            artifact: fixture.contract.old_artifact,
            replacement: ReplacementSupport::Stateful {
                state_contract: fixture.contract.state.unwrap().descriptor,
                maximum_export_bytes: 32,
                maximum_import_bytes: 32,
                maximum_ticks: 100,
            },
        },
        b"old",
    );
    let candidate = generation(
        HostedGenerationBinding {
            epoch: fixture.contract.candidate,
            implementation: fixture.contract.candidate_implementation,
            artifact: fixture.contract.candidate_artifact,
            replacement: ReplacementSupport::Stateful {
                state_contract: fixture.contract.state.unwrap().descriptor,
                maximum_export_bytes: 32,
                maximum_import_bytes: 32,
                maximum_ticks: 100,
            },
        },
        b"",
    );
    let mut transaction = HostedTransitionTransaction::<_, _, _, 64>::new(
        fixture.contract,
        fixture.contract.old,
        old,
        candidate,
        FixtureRouter {
            active: fixture.contract.old,
            admissions: fixture.contract.old,
        },
        20,
        &mut [ZERO; 2],
    )
    .unwrap();
    transaction
        .reserve(
            reservation,
            TransitionUsage {
                overlap: fixture.contract.budget.overlap_reserved,
                in_flight_values: 0,
                pending_operations: 0,
                drained_values: 0,
                rejected_values: 0,
                lost_values: 0,
                completed_operations: 0,
                cancelled_operations: 0,
                replay_items: 0,
                replay_bytes: 0,
                duplicate_replay_items: 0,
                state_bytes: 0,
            },
            21,
        )
        .unwrap();
    transaction.prepare(22).unwrap();
    transaction.rollback(&mut ledger, hash(120), 23).unwrap();
    assert_eq!(transaction.active_epoch(), fixture.contract.old);
    assert_eq!(transaction.phase(), TransitionPhase::RolledBack);
    assert_eq!(ledger.checkpoint().current_stock, 0);
    assert_eq!(ledger.checkpoint().lifetime_committed, 0);
    assert_eq!(ledger.checkpoint().reservations[0].units, 1);
}

#[test]
fn candidate_prepare_failure_rolls_back_to_the_unchanged_old_generation() {
    let fixture = admission_fixture();
    let mut ledger = PersistentBudgetLedger::<8>::new(fixture.budget_policy, hash(110), 0).unwrap();
    let reservation = admit(&fixture, &mut ledger).unwrap();
    let old = generation(stateful_binding(fixture.contract, false), b"old");
    let mut candidate = generation(stateful_binding(fixture.contract, true), b"");
    candidate.fail_prepare = true;
    let mut transaction = HostedTransitionTransaction::<_, _, _, 64>::new(
        fixture.contract,
        fixture.contract.old,
        old,
        candidate,
        FixtureRouter {
            active: fixture.contract.old,
            admissions: fixture.contract.old,
        },
        20,
        &mut [ZERO; 2],
    )
    .unwrap();
    transaction
        .reserve(reservation, empty_usage(fixture.contract), 21)
        .unwrap();
    assert!(matches!(
        transaction.prepare(22),
        Err(conduit_runtime::HostedTransitionError::Generation(_))
    ));
    transaction.rollback(&mut ledger, hash(160), 23).unwrap();
    assert_eq!(transaction.active_epoch(), fixture.contract.old);
    assert_eq!(ledger.checkpoint().current_stock, 0);
}

#[test]
fn old_generation_drain_failure_has_a_deterministic_precommit_rollback() {
    let fixture = admission_fixture();
    let mut ledger = PersistentBudgetLedger::<8>::new(fixture.budget_policy, hash(110), 0).unwrap();
    let reservation = admit(&fixture, &mut ledger).unwrap();
    let mut old = generation(stateful_binding(fixture.contract, false), b"old");
    old.fail_drain = true;
    let candidate = generation(stateful_binding(fixture.contract, true), b"");
    let mut transaction = HostedTransitionTransaction::<_, _, _, 64>::new(
        fixture.contract,
        fixture.contract.old,
        old,
        candidate,
        FixtureRouter {
            active: fixture.contract.old,
            admissions: fixture.contract.old,
        },
        20,
        &mut [ZERO; 2],
    )
    .unwrap();
    transaction
        .reserve(reservation, empty_usage(fixture.contract), 21)
        .unwrap();
    transaction.prepare(22).unwrap();
    transaction.barrier(23).unwrap();
    assert!(matches!(
        transaction.drain(24),
        Err(conduit_runtime::HostedTransitionError::Generation(_))
    ));
    transaction.rollback(&mut ledger, hash(161), 25).unwrap();
    assert_eq!(transaction.active_epoch(), fixture.contract.old);
}

#[test]
fn admission_freshness_loss_before_a_phase_fails_closed_and_can_roll_back() {
    let fixture = admission_fixture();
    let mut ledger = PersistentBudgetLedger::<8>::new(fixture.budget_policy, hash(110), 0).unwrap();
    let reservation = admit(&fixture, &mut ledger).unwrap();
    let old = generation(stateful_binding(fixture.contract, false), b"old");
    let candidate = generation(stateful_binding(fixture.contract, true), b"");
    let mut transaction = HostedTransitionTransaction::<_, _, _, 64>::new(
        fixture.contract,
        fixture.contract.old,
        old,
        candidate,
        FixtureRouter {
            active: fixture.contract.old,
            admissions: fixture.contract.old,
        },
        20,
        &mut [ZERO; 2],
    )
    .unwrap();
    transaction
        .reserve(reservation, empty_usage(fixture.contract), 21)
        .unwrap();
    transaction.prepare(22).unwrap();
    assert_eq!(
        transaction.barrier(35),
        Err(conduit_runtime::HostedTransitionError::AdmissionExpired)
    );
    transaction.rollback(&mut ledger, hash(162), 35).unwrap();
    assert_eq!(transaction.active_epoch(), fixture.contract.old);
}

fn http_capabilities() -> HttpServingCapabilities {
    HttpServingCapabilities {
        profile_version: 1,
        plaintext: true,
        direct_tls: false,
        trusted_proxy_tls: false,
        http11: true,
        http2: false,
        websocket: false,
        sse: false,
        maximum_request_head_bytes: 1024,
        maximum_request_body_bytes: 64,
        maximum_response_bytes: 64,
        maximum_connections: 4,
        maximum_sessions: 0,
        adapter_buffer_bytes: 1024,
        backend_buffer_bytes: 1024,
        kernel_buffer_bytes: 1024,
        complete_stack_hard_bounded: true,
    }
}

fn http_service(
    implementation: PinnedDescriptor<'static>,
    artifact: ArtifactDigest,
) -> ResolvedHttpService<'static> {
    assert_eq!(implementation.id.as_str(), HTTP_IN_MEMORY_IMPLEMENTATION_ID);
    let mut value = ResolvedHttpService {
        identity: ZERO,
        service: CONTRACT_PIN,
        backend: implementation,
        artifact: PlanArtifact {
            id: Id("fixture/http-artifact"),
            digest: artifact,
        },
        execution_profile: PROFILE_PIN,
        listen: "memory://transition",
        protocol: HttpProtocol::Http11,
        security: pin("fixture/http-plaintext", 151),
        security_mode: HttpSecurityMode::Plaintext,
        certificate_identity: None,
        trusted_proxy: None,
        grant: Id("grant.transition"),
        secret_scope: None,
        require_complete_stack_hard_bound: true,
        limits: HttpServiceLimits {
            maximum_request_head_bytes: 256,
            maximum_request_body_bytes: 32,
            maximum_response_bytes: 32,
            maximum_header_count: 8,
            maximum_header_bytes: 128,
            maximum_connections: 4,
            maximum_queued_admissions: 4,
            maximum_live_handlers: 4,
            maximum_sessions: 0,
            maximum_session_queue_items: 0,
            maximum_session_queue_bytes: 0,
            maximum_evidence_events: 32,
            header_deadline_ticks: 10,
            body_deadline_ticks: 10,
            handler_deadline_ticks: 10,
            drain_deadline_ticks: 10,
            reserved_memory_bytes: 4096,
        },
    };
    value.identity = value.computed_identity();
    value
}

fn http_authority() -> HttpServingAuthority<'static> {
    HttpServingAuthority {
        grant: Id("grant.transition"),
        allowed: true,
        current_tick: 20,
        valid_until_tick: 40,
    }
}

fn http_request(path: &str) -> Vec<u8> {
    format!("GET {path} HTTP/1.1\r\nContent-Length: 0\r\n\r\n").into_bytes()
}

#[test]
fn http_generation_handoff_routes_new_admissions_while_old_request_drains() {
    let boundary = pin("conduit.http/request-boundary", 7);
    let manifest = quiescent_candidate_manifest(boundary);
    let contract = quiescent_contract(&manifest);
    let fixture = admission_fixture_from(&manifest, contract);
    let mut ledger = PersistentBudgetLedger::<8>::new(fixture.budget_policy, hash(110), 0).unwrap();
    let reservation = admit(&fixture, &mut ledger).unwrap();
    let old_binding = HostedGenerationBinding {
        epoch: contract.old,
        implementation: contract.old_implementation,
        artifact: contract.old_artifact,
        replacement: ReplacementSupport::Quiescent {
            boundary,
            maximum_ticks: 100,
        },
    };
    let candidate_binding = HostedGenerationBinding {
        epoch: contract.candidate,
        implementation: contract.candidate_implementation,
        artifact: contract.candidate_artifact,
        replacement: ReplacementSupport::Quiescent {
            boundary,
            maximum_ticks: 100,
        },
    };
    let (old, old_handle) = InMemoryHttpTransitionGeneration::active(
        old_binding,
        boundary,
        http_capabilities(),
        http_service(contract.old_implementation, contract.old_artifact),
        http_authority(),
    )
    .unwrap();
    let (candidate, candidate_handle) = InMemoryHttpTransitionGeneration::candidate(
        candidate_binding,
        boundary,
        http_capabilities(),
        http_service(
            contract.candidate_implementation,
            contract.candidate_artifact,
        ),
        http_authority(),
    )
    .unwrap();
    let old_exchange = old_handle
        .admit(IpAddr::V4(Ipv4Addr::LOCALHOST), &http_request("/old"))
        .unwrap();
    let mut transaction = HostedTransitionTransaction::<_, _, _, 64>::new(
        contract,
        contract.old,
        old,
        candidate,
        FixtureRouter {
            active: contract.old,
            admissions: contract.old,
        },
        20,
        &mut [ZERO; 2],
    )
    .unwrap();
    transaction
        .reserve(
            reservation,
            TransitionUsage {
                overlap: contract.budget.overlap_reserved,
                in_flight_values: 1,
                pending_operations: 1,
                drained_values: 0,
                rejected_values: 0,
                lost_values: 0,
                completed_operations: 0,
                cancelled_operations: 0,
                replay_items: 0,
                replay_bytes: 0,
                duplicate_replay_items: 0,
                state_bytes: 0,
            },
            21,
        )
        .unwrap();
    transaction.prepare(22).unwrap();
    transaction.barrier(23).unwrap();
    assert_eq!(
        old_handle.admit(IpAddr::V4(Ipv4Addr::LOCALHOST), &http_request("/late-old")),
        Err(conduit_http::HttpReason::Closed)
    );
    let candidate_exchange = candidate_handle
        .admit(IpAddr::V4(Ipv4Addr::LOCALHOST), &http_request("/candidate"))
        .unwrap();

    let old_connection = match old_handle.poll_accept() {
        Poll::Ready(Ok(connection)) => connection,
        other => panic!("old request was not admitted: {other:?}"),
    };
    let old_request = match old_handle.poll_exchange(old_connection) {
        Poll::Ready(Ok(HttpExchangeEvent::Request(request))) => request,
        other => panic!("old request did not drain: {other:?}"),
    };
    assert_eq!(old_request.exchange, old_exchange);
    assert_eq!(
        old_handle.poll_send(
            old_connection,
            &HttpResponsePart {
                exchange: old_request.exchange,
                status: 200,
                headers: vec![],
                body: b"old".to_vec(),
                terminal: true,
            }
        ),
        Poll::Ready(Ok(()))
    );
    old_handle.close(old_connection).unwrap();
    assert_eq!(transaction.drain(24).unwrap().drained_values, 1);
    transaction.rebind(25).unwrap();
    transaction.commit(&mut ledger, 26).unwrap();
    transaction.retire_old(27).unwrap();
    transaction.complete(28).unwrap();

    assert_eq!(candidate_handle.connection_count(), 1);
    let candidate_connection = match candidate_handle.poll_accept() {
        Poll::Ready(Ok(connection)) => connection,
        other => panic!("candidate request was not admitted: {other:?}"),
    };
    let candidate_request = match candidate_handle.poll_exchange(candidate_connection) {
        Poll::Ready(Ok(HttpExchangeEvent::Request(request))) => request,
        other => panic!("candidate request did not survive commit: {other:?}"),
    };
    assert_eq!(candidate_request.exchange, candidate_exchange);
    let (_, _, router) = transaction.into_parts();
    assert_eq!(router.active, contract.candidate);
    assert_eq!(router.admissions, contract.candidate);
}

#[test]
fn every_hosted_transition_fixture_case_executes_independently() {
    let fixture: serde_json::Value = serde_json::from_str(TRANSITION_FIXTURE).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        if case["runner"] != "transition-hosted" {
            continue;
        }
        match case["id"].as_str().unwrap() {
            "tongues-tts-utterance-boundary" => {
                tongues_tts_transition_is_quiescent_at_the_opaque_utterance_boundary();
            }
            "tongues-asr-segment-state-replay" => {
                tongues_asr_segment_transition_uses_opaque_state_retained_replay_and_real_admission(
                );
            }
            "http-request-generation-drain" => {
                http_generation_handoff_routes_new_admissions_while_old_request_drains();
            }
            "candidate-prepare-failure-rollback" => {
                candidate_prepare_failure_rolls_back_to_the_unchanged_old_generation();
            }
            "old-generation-failure-rollback" => {
                old_generation_drain_failure_has_a_deterministic_precommit_rollback();
            }
            "host-report-freshness-loss" => {
                admission_freshness_loss_before_a_phase_fails_closed_and_can_roll_back();
            }
            "stale-control-epoch-rejected" => {
                stale_control_or_authorization_fails_before_budget_reservation();
            }
            "self-authorized-successor-rejected" => {
                self_approved_successor_fails_before_budget_reservation();
            }
            "persistent-budget-status-stale" => {
                stale_resolution_and_budget_status_fail_closed_without_ledger_mutation();
            }
            "toxic-generation-overlap-rejected" => {
                combined_old_candidate_effect_closure_is_checked_before_prepare();
            }
            "inhibit-missing-or-stale-rejected" => {
                required_inhibit_is_fresh_exact_and_independent();
            }
            "rollback-does-not-reset-durable-budget" => {
                rollback_releases_stock_without_replenishing_lifetime_or_attempt_state();
            }
            "replacement-capability-overstatement-rejected" => {
                resolved_candidate_cannot_overstate_its_replacement_level();
            }
            other => panic!("unhandled hosted transition fixture {other}"),
        }
    }
}
