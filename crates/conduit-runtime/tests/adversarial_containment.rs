use conduit_core::{
    AcknowledgementMode, AdministrativeApproval, AdministrativeApprovalStatus,
    AdministrativeApprover, AdministrativeCommit, AdministrativeExecution, AdministrativePrincipal,
    AdministrativeProof, AdministrativeProposal, AdministrativeSubject, AdministrativeSupportEdge,
    ArtifactDigest, ArtifactLocation, ArtifactLocationKind, ArtifactManifest, ArtifactProvenance,
    ArtifactSignature, ArtifactTrustPolicy, AuthorityGrant, AuthorityScope, AuthorityTime,
    CONTAINMENT_POLICY_SCHEMA_VERSION, ConfigContract, ContainmentContext, ContainmentPolicy,
    CredentialVerification, CredentialVerificationOutcome, DelegationEnvelope, DelegationPolicy,
    DisconnectPolicy, DistributedCordBudget, DistributedDelivery, DistributedOrdering,
    DistributedPeerRequirement, DistributedSessionMachine, EffectRequirement, EntityPassport,
    EnvelopeValue, FlowCapacity, FlowPolicy, FlowWatermarks, GrantStatus,
    HAZARDOUS_HOST_PROFILE_SCHEMA_VERSION, HazardControlPhase, HazardControlState,
    HazardousCommand, HazardousHostBinding, HazardousHostProfile, HostCapability,
    INHIBIT_OBSERVATION_SCHEMA_VERSION, Id, ImplementationConfinement, InhibitCause,
    InhibitClearRequest, InhibitLatchState, InhibitObservation, InstancePath, KeyProtection,
    MemberDisposition, MemberSecurityState, MembershipCredential, NodeContract, ObservedGrant,
    OperatingEnvelopeLimit, POLICY_BUDGET_SCHEMA_VERSION, PassportStatus,
    PassportStatusObservation, PendingControl, PersistentBudgetLedger, PersistentBudgetPolicy,
    PinnedDescriptor, PlanDistributedCord, PlanGraph, PlanNode, PlanResourceBudget,
    PolicyBudgetAnchor, PolicyBudgetConsumer, PolicyBudgetLimits, PolicyBudgetRequest, Pressure,
    PublicKeyRef, REALM_SCHEMA_VERSION, RUNTIME_EVIDENCE_POLICY_VERSION, RealmDescriptor,
    ReceiveDisposition, ReconnectMode, ResourceRef, ResourceSelector, RollingLimit,
    RuntimeEvidenceBudget, RuntimeEvidenceMode, RuntimeEvidencePolicy, SemanticHash, Sensitivity,
    SignatureVerification, StopPolicy, resolve_authority, validate_administrative_proof,
    validate_federation, validate_hazardous_host_binding, validate_passport_at,
    validate_passport_status, validate_plan_graph, validate_policy_budget_status,
    validate_quarantined_member, validate_realm, validate_recovery_narrowing,
    validate_support_graph, verify_artifact_candidate,
};
use serde::Deserialize;
use serde_json::{Value, json};

const FIXTURE: &str = include_str!("../../../conformance/c5/adversarial-containment.json");
const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);

#[derive(Deserialize)]
struct Fixture {
    seed: u64,
    maximum_steps_per_trace: usize,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    initial: String,
    attacker_capabilities: Vec<String>,
    operations: Vec<String>,
    expected_rejection_step: String,
    expected: Value,
    final_state: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProtectedSnapshot {
    authority: u8,
    population: u8,
    installed: u8,
    federations: u8,
    host_resources: u8,
    budget: conduit_core::PolicyBudgetCheckpoint<4>,
    hazard: HazardControlState,
}

struct FakeSystem {
    authority: u8,
    population: u8,
    installed: u8,
    federations: u8,
    host_resources: u8,
    authority_ceiling: u8,
    population_ceiling: u8,
    installed_ceiling: u8,
    budget: PersistentBudgetLedger<'static, 4>,
    hazard: HazardControlState,
    evidence: Vec<String>,
    maximum_steps: usize,
}

impl FakeSystem {
    fn new(maximum_steps: usize, evidence_slots: u32) -> Self {
        let policy = budget_policy(1, evidence_slots);
        Self {
            authority: 1,
            population: 1,
            installed: 0,
            federations: 0,
            host_resources: 0,
            authority_ceiling: 1,
            population_ceiling: 2,
            installed_ceiling: 0,
            budget: PersistentBudgetLedger::new(policy, hash(90), 0).unwrap(),
            hazard: inhibited_state(),
            evidence: Vec::new(),
            maximum_steps,
        }
    }

    fn snapshot(&self) -> ProtectedSnapshot {
        ProtectedSnapshot {
            authority: self.authority,
            population: self.population,
            installed: self.installed,
            federations: self.federations,
            host_resources: self.host_resources,
            budget: self.budget.checkpoint(),
            hazard: self.hazard,
        }
    }

    fn accepted_step<T>(&mut self, label: &str, operation: impl FnOnce(&mut Self) -> T) -> T {
        assert!(self.evidence.len() < self.maximum_steps);
        let result = operation(self);
        self.evidence
            .push(format!("{:02}:{label}:accepted", self.evidence.len() + 1));
        self.assert_global_properties();
        result
    }

    fn rejected_step(
        &mut self,
        label: &str,
        operation: impl FnOnce(&mut Self) -> &'static str,
    ) -> Value {
        assert!(self.evidence.len() < self.maximum_steps);
        let before = self.snapshot();
        let reason = operation(self);
        assert_eq!(
            self.snapshot(),
            before,
            "rejected consequential step mutated protected state"
        );
        self.evidence.push(format!(
            "{:02}:{label}:rejected:{reason}",
            self.evidence.len() + 1
        ));
        self.assert_global_properties();
        json!({"accepted": false, "reason": reason, "mutation": false})
    }

    fn assert_global_properties(&self) {
        let checkpoint = self.budget.checkpoint();
        assert!(self.authority <= self.authority_ceiling);
        assert!(self.population <= self.population_ceiling);
        assert!(self.installed <= self.installed_ceiling);
        assert_eq!(self.federations, 0);
        assert_eq!(self.host_resources, 0);
        assert!(checkpoint.current_stock <= 1);
        assert!(checkpoint.rolling_committed <= 1);
        assert!(checkpoint.lifetime_committed <= 1);
        assert_eq!(self.hazard.phase, HazardControlPhase::Inhibited);
        assert_eq!(self.hazard.plan, ZERO);
        assert_eq!(self.hazard.command_authority, ZERO);
    }

    fn final_state(&self) -> Value {
        let checkpoint = self.budget.checkpoint();
        let reserved = checkpoint
            .reservations
            .iter()
            .filter(|reservation| {
                reservation.state == conduit_core::PolicyReservationState::Reserved
            })
            .map(|reservation| reservation.units)
            .sum::<u64>();
        json!({
            "authority": self.authority,
            "population": self.population,
            "installed": self.installed,
            "federations": self.federations,
            "budget_lifetime": checkpoint.lifetime_committed,
            "budget_reserved": reserved,
            "inhibit": "inhibited"
        })
    }
}

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 0,
        semantic_hash: hash(byte),
    }
}

fn validate_active_passport() {
    let root = PublicKeyRef {
        id: Id("key.root"),
        algorithm: Id("ed25519"),
        public_key_digest: hash(101),
    };
    let member_key = PublicKeyRef {
        id: Id("key.member"),
        algorithm: Id("ed25519"),
        public_key_digest: hash(102),
    };
    let roots = [root];
    let keys = [member_key];
    let mut realm = RealmDescriptor {
        schema_version: REALM_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("realm.alpha"),
        genesis_root: root,
        accepted_roots: &roots,
        root_epoch: 1,
        policy: pin("policy.realm", 103),
        membership_profile: pin("profile.membership", 104),
        revocation_profile: pin("profile.revocation", 105),
        event_integrity_profile: pin("profile.integrity", 106),
        federation_profile: pin("profile.federation", 107),
        successions: &[],
    };
    realm.identity = realm.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    validate_realm(&realm, &mut [ZERO; 2]).unwrap();

    let mut passport = EntityPassport {
        schema_version: REALM_SCHEMA_VERSION,
        identity: ZERO,
        entity: Id("member.compromised"),
        profile: pin("profile.member", 108),
        realm: realm.id,
        credential: MembershipCredential {
            id: Id("credential.member"),
            realm: realm.id,
            entity: Id("member.compromised"),
            key: member_key.id,
            issuer_key: root.id,
            issued_at_tick: 1,
            expires_at_tick: 20,
            time_basis: Id("clock.monotonic"),
            receipt: hash(109),
        },
        keys: &keys,
        roles: &[],
        key_protection: KeyProtection::ExportableSoftware,
        sensitivity: Sensitivity::Restricted,
        extensions: &[],
    };
    passport.identity = passport.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    let mut verification = CredentialVerification {
        identity: ZERO,
        credential: passport.credential.id,
        passport: passport.identity,
        verifier: pin("verifier.credential", 110),
        challenge: Id("challenge.member"),
        time_basis: Id("clock.monotonic"),
        observed_at_tick: 2,
        valid_until_tick: 10,
        outcome: CredentialVerificationOutcome::Verified,
        receipt: hash(111),
    };
    verification.identity = verification.computed_semantic_hash().unwrap();
    validate_passport_at(
        &passport,
        &realm,
        verification,
        Id("clock.monotonic"),
        3,
        &mut [ZERO; 2],
    )
    .unwrap();
    validate_passport_status(
        PassportStatusObservation {
            passport: passport.identity,
            realm: realm.id,
            entity: passport.entity,
            reporter: pin("reporter.status", 112),
            time_basis: Id("clock.monotonic"),
            observed_at_tick: 2,
            valid_until_tick: 10,
            status: PassportStatus::Active,
        },
        passport.identity,
        realm.id,
        passport.entity,
        Id("clock.monotonic"),
        3,
    )
    .unwrap();
}

fn budget_policy(limit: u64, evidence: u32) -> PersistentBudgetPolicy<'static> {
    let mut policy = PersistentBudgetPolicy {
        schema_version: POLICY_BUDGET_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("budget.containment", 1),
        owner: pin("owner.independent", 2),
        subject: pin("subject.population", 3),
        anchor: PolicyBudgetAnchor::Host(Id("host.alpha")),
        action: Id("action.enroll"),
        resource_class: pin("resource.member", 4),
        time_basis: Id("clock.monotonic"),
        limits: PolicyBudgetLimits {
            current_stock: Some(limit),
            rolling: Some(RollingLimit {
                units: limit,
                window_ticks: 100,
            }),
            lifetime: Some(limit),
        },
        reservation_ttl_ticks: 10,
        lease: None,
        audit_id: Id("audit.containment"),
        persistence_profile: pin("persistence.atomic", 5),
        maximum_reservations: 4,
        maximum_evidence_events: evidence,
    };
    policy.identity = policy.computed_semantic_hash().unwrap();
    policy
}

fn request(
    policy: PersistentBudgetPolicy<'static>,
    correlation: u8,
    plan: u8,
    epoch: u64,
    realm: &'static str,
    tick: u64,
) -> PolicyBudgetRequest<'static> {
    let mut request = PolicyBudgetRequest {
        identity: ZERO,
        correlation: hash(correlation),
        policy_identity: policy.identity,
        consumer: PolicyBudgetConsumer {
            realm: Id(realm),
            plan: hash(plan),
            epoch,
            generation: epoch,
            run: Id("run.adversarial"),
        },
        action: policy.action,
        units: 1,
        requested_at_tick: tick,
        lease: None,
    };
    request.identity = request.computed_semantic_hash().unwrap();
    request
}

fn commit_population(system: &mut FakeSystem, correlation: u8, plan: u8, epoch: u64) {
    let policy = budget_policy(1, 16);
    let request = request(policy, correlation, plan, epoch, "realm.alpha", 1);
    let (reservation, _) = system
        .budget
        .reserve(
            request,
            AuthorityTime {
                basis: Id("clock.monotonic"),
                tick: 1,
            },
            true,
        )
        .unwrap();
    system.budget.commit(reservation.identity, 2).unwrap();
    system.population += 1;
}

fn envelope(depth: u8) -> DelegationEnvelope<'static> {
    DelegationEnvelope {
        action: Id("action.install"),
        resource: ResourceSelector::Exact(ResourceRef {
            kind: Id("artifact"),
            id: Id("artifact.fixture"),
        }),
        audience: Id("runtime"),
        time_basis: Id("clock.monotonic"),
        not_before_tick: 1,
        expires_at_tick: 20,
        remaining_depth: depth,
    }
}

fn validate_minimal_plan() {
    let contract = NodeContract {
        id: Id("fixture/contained-node"),
        config: ConfigContract { fields: &[] },
        inputs: &[],
        outputs: &[],
    };
    let nodes = [PlanNode {
        id: Id("contained"),
        contract: &contract,
    }];
    validate_plan_graph(&PlanGraph {
        nodes: &nodes,
        cords: &[],
    })
    .unwrap();
}

fn validate_exact_ordinary_authority() {
    let resource = ResourceRef {
        kind: Id("artifact"),
        id: Id("artifact.fixture"),
    };
    let requester = InstancePath::new("root/installer").unwrap();
    let effect = EffectRequirement {
        id: Id("install"),
        administrative_class: None,
        policy_budget_class: None,
        action: Id("artifact.read"),
        resource: ResourceSelector::Exact(resource),
        requester,
        audience: Id("runtime"),
        constraints: &[],
        check_at_use: true,
    };
    let capability = HostCapability {
        id: Id("capability.artifact-read"),
        action: effect.action,
        resource,
        host: Id("host.alpha"),
        time_basis: Id("clock.monotonic"),
        observed_at_tick: 1,
        valid_until_tick: 20,
    };
    let grant = AuthorityGrant {
        id: Id("grant.artifact-read"),
        action: effect.action,
        resource,
        scope: AuthorityScope {
            root: requester,
            descendants: false,
        },
        audience: effect.audience,
        constraints: &[],
        time_basis: Id("clock.monotonic"),
        not_before_tick: 1,
        expires_at_tick: 20,
        issued_for_host: Id("host.alpha"),
        delegation: DelegationPolicy::None,
        audit_id: Id("audit.artifact-read"),
        terminal_policy: StopPolicy::Abort,
    };
    resolve_authority(
        effect,
        Id("host.alpha"),
        AuthorityTime {
            basis: Id("clock.monotonic"),
            tick: 2,
        },
        &[capability],
        &[ObservedGrant {
            grant,
            status: GrantStatus::Active,
        }],
    )
    .unwrap();
}

fn distributed_peer(
    host: &'static str,
    realm: &'static str,
    entity: &'static str,
    byte: u8,
) -> DistributedPeerRequirement<'static> {
    DistributedPeerRequirement {
        node: InstancePath::new(if byte == 1 {
            "root/writer"
        } else {
            "root/reader"
        })
        .unwrap(),
        host_observation: Id(host),
        realm: Id(realm),
        realm_identity: hash(byte + 10),
        entity: Id(entity),
        passport: hash(byte + 20),
        passport_schema_version: 0,
        credential: Id(if byte == 1 {
            "credential.writer"
        } else {
            "credential.reader"
        }),
        credential_epoch: 1,
        key: Id(if byte == 1 {
            "key.writer"
        } else {
            "key.reader"
        }),
        key_epoch: 1,
        status_reporter: pin("provider.status", 87),
        credential_verifier: pin("provider.credential", 88),
        audience: Id("runtime"),
        grant_hash: hash(byte + 30),
    }
}

fn distributed_binding() -> PlanDistributedCord<'static> {
    let capacity = FlowCapacity::new(2, 64, 128).unwrap();
    let mut binding = PlanDistributedCord {
        schema_version: 0,
        identity: ZERO,
        cord: Id("cord.adversarial"),
        writer_port_contract_hash: hash(90),
        reader_port_contract_hash: hash(91),
        flow: FlowPolicy::new(
            capacity,
            Pressure::Reject,
            FlowWatermarks::new(0, 2, capacity).unwrap(),
        )
        .unwrap(),
        session: Id("session.adversarial"),
        initial_session_epoch: 1,
        backend: pin("backend.deterministic", 92),
        backend_artifact: None,
        backend_profile: None,
        carrier_security: pin("carrier.mtls", 93),
        carrier_security_mode: None,
        carrier_endpoint: None,
        carrier_binding: Id("carrier.binding"),
        delivery: DistributedDelivery::AtLeastOnce,
        acknowledgement: AcknowledgementMode::Cumulative,
        ordering: DistributedOrdering::InOrder,
        reconnect: ReconnectMode::ResumeSameEpoch,
        disconnect: DisconnectPolicy::AwaitReconnect,
        writer: distributed_peer("report.writer", "realm.a", "writer", 1),
        reader: distributed_peer("report.reader", "realm.b", "reader", 2),
        federation_policy: Some(pin("federation.a-to-b", 94)),
        budget: DistributedCordBudget {
            send_items: 2,
            send_bytes: 128,
            receive_items: 2,
            receive_bytes: 128,
            retry_items: 2,
            retry_bytes: 128,
            reorder_items: 2,
            reorder_bytes: 128,
            dedup_items: 2,
            maximum_payload_bytes: 64,
            maximum_frame_bytes: 80,
            maximum_unacknowledged: 2,
            maximum_retries: 2,
            maximum_reconnect_attempts: 2,
            heartbeat_interval_ticks: 2,
            liveness_timeout_ticks: 5,
            reconnect_deadline_ticks: 10,
            maximum_evidence_events: 16,
            allocated_memory_bytes: 512,
        },
        allocation: PlanResourceBudget {
            memory_bytes: 512,
            storage_bytes: 0,
            cpu_units: 1,
            timers: 2,
            transports: 1,
            checkpoints: 0,
            evidence_bytes: 16,
        },
    };
    binding.identity = binding.semantic_hash().unwrap();
    binding
}

fn exercise_transport_replay_partition_and_transition() {
    let binding = distributed_binding();
    let mut session = DistributedSessionMachine::new(1);
    session.establish(1).unwrap();
    assert_eq!(
        session.receive(&binding, 0, 8).unwrap(),
        ReceiveDisposition::Accepted
    );
    assert_eq!(
        session.receive(&binding, 0, 8).unwrap(),
        ReceiveDisposition::DuplicateSuppressed
    );
    assert_eq!(
        session.observe_liveness(&binding, 6).unwrap_err().code(),
        "CND-DST-021"
    );
    assert_eq!(session.session_epoch, 1);
    assert_eq!(session.next_receive_sequence, 1);
    session.request_cancel().unwrap();
    session
        .acknowledge_control(PendingControl::Cancellation)
        .unwrap();
}

fn exercise_runtime_evidence_capacity() {
    let policy = RuntimeEvidencePolicy {
        schema_version: RUNTIME_EVIDENCE_POLICY_VERSION,
        mode: RuntimeEvidenceMode::Record,
        stream: Some(Id("evidence.adversarial")),
        maximum_events: 2,
        maximum_bytes: 128,
        required_reserve_events: 1,
        required_reserve_bytes: 32,
        telemetry_period: 1,
        telemetry_offset: 0,
        gap_summary_bytes: 16,
    };
    let mut budget = RuntimeEvidenceBudget::new(policy);
    budget.record_required(32, false).unwrap();
    assert_eq!(
        budget.record_required(32, false).unwrap_err().code(),
        "CND-RTE-006"
    );
    budget.record_required(32, true).unwrap();
    budget.finish().unwrap();
}

fn inhibited_state() -> HazardControlState {
    conduit_core::inhibit_hazardous_host(
        HazardControlState::safe_disarmed(1, hash(40)),
        hash(41),
        InhibitCause::StopRequest,
    )
}

fn hazardous_profile() -> HazardousHostProfile<'static> {
    static ENVELOPE: [OperatingEnvelopeLimit<'static>; 1] = [OperatingEnvelopeLimit {
        dimension: PinnedDescriptor {
            id: Id("domain.limit"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([50; 32]),
        },
        minimum: 0,
        maximum: 10,
    }];
    let mut profile = HazardousHostProfile {
        schema_version: HAZARDOUS_HOST_PROFILE_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("profile.hazardous", 51),
        safe_state: pin("domain.safe", 52),
        inhibit_boundary: pin("host.inhibit", 53),
        watchdog: pin("host.watchdog", 54),
        effect_boundary: pin("host.effect", 55),
        command_effect_class: pin("effect.command", 56),
        clear_effect_class: pin("effect.clear", 57),
        clear_operation: pin("operation.clear", 58),
        clear_ceremony: pin("ceremony.clear", 59),
        time_basis: Id("clock.monotonic"),
        maximum_command_horizon_ticks: 10,
        maximum_observation_age_ticks: 20,
        maximum_evidence_records: 16,
        require_physical_presence_to_clear: true,
        require_isolated_implementation: true,
        envelope: &ENVELOPE,
    };
    profile.identity = profile.computed_semantic_hash(&mut [ZERO; 4]).unwrap();
    profile
}

fn hazardous_binding(
    profile: HazardousHostProfile<'static>,
    confinement: ImplementationConfinement,
) -> HazardousHostBinding<'static> {
    let mut observation = InhibitObservation {
        schema_version: INHIBIT_OBSERVATION_SCHEMA_VERSION,
        identity: ZERO,
        profile_identity: profile.identity,
        host: Id("host.hazard"),
        safe_state: profile.safe_state,
        inhibit_boundary: profile.inhibit_boundary,
        watchdog: profile.watchdog,
        effect_boundary: profile.effect_boundary,
        time_basis: profile.time_basis,
        observed_at_tick: 1,
        valid_until_tick: 20,
        latch_generation: 2,
        latch_state: InhibitLatchState::Inhibited,
        independent_from_plan: true,
        local_safe_path: true,
        survives_executor_loss: true,
        survives_partition: true,
        graph_cannot_replace: true,
        confinement,
    };
    observation.identity = observation.computed_semantic_hash().unwrap();
    HazardousHostBinding {
        host: observation.host,
        profile,
        observation,
    }
}

fn principal(entity: &'static str, plan: u8) -> AdministrativePrincipal<'static> {
    AdministrativePrincipal {
        realm: Id("realm.alpha"),
        entity: Id(entity),
        key: Id("key.fixture"),
        profile: pin("profile.member", 60),
        source_plan: hash(plan),
        source_epoch: 1,
    }
}

fn successor_self_approval_reason() -> &'static str {
    let requester = principal("requester", 2);
    let approver = principal("predecessor-approver", 1);
    let committer = principal("committer", 3);
    let executor = principal("executor", 4);
    let failure = pin("failure.independent", 64);
    let policy_approvers = [AdministrativeApprover {
        realm: approver.realm,
        entity: approver.entity,
        key: approver.key,
        profile: approver.profile,
        failure_domain: failure,
    }];
    let effect_class = pin("effect.activate-successor", 65);
    let mut policy = ContainmentPolicy {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("policy.successor", 66),
        effect_class,
        approvers: &policy_approvers,
        committer: AdministrativeApprover {
            realm: committer.realm,
            entity: committer.entity,
            key: committer.key,
            profile: committer.profile,
            failure_domain: pin("failure.committer", 67),
        },
        executor: AdministrativeApprover {
            realm: executor.realm,
            entity: executor.entity,
            key: executor.key,
            profile: executor.profile,
            failure_domain: pin("failure.executor", 68),
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
    let subject = AdministrativeSubject {
        realm: Id("realm.alpha"),
        entity: Id("successor"),
        plan: hash(5),
        epoch: 2,
        artifact: None,
        budget: None,
    };
    let beneficiaries = [subject];
    let mut proposal = AdministrativeProposal {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("proposal.successor"),
        effect_class,
        operation: pin("operation.activate", 69),
        requester,
        subject,
        beneficiaries: &beneficiaries,
        predecessor_plan: Some(hash(1)),
        delegation: None,
        protected_handle: None,
        ceremony: None,
        time_basis: Id("clock.monotonic"),
        created_at_tick: 1,
        expires_at_tick: 20,
    };
    proposal.identity = proposal.computed_semantic_hash().unwrap();
    let mut approval = AdministrativeApproval {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("approval.predecessor"),
        proposal_identity: proposal.identity,
        policy_identity: policy.identity,
        approver,
        failure_domain: failure,
        time_basis: Id("clock.monotonic"),
        issued_at_tick: 2,
        expires_at_tick: 15,
        status: AdministrativeApprovalStatus::Current,
    };
    approval.identity = approval.computed_semantic_hash().unwrap();
    let approvals = [approval];
    let approval_hashes = [approval.identity];
    let mut commit = AdministrativeCommit {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("commit.successor"),
        proposal_identity: proposal.identity,
        policy_identity: policy.identity,
        approvals: &approval_hashes,
        committed_by: committer,
        committed_at_tick: 3,
    };
    commit.identity = commit.computed_semantic_hash().unwrap();
    let mut execution = AdministrativeExecution {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("execution.successor"),
        proposal_identity: proposal.identity,
        commit_identity: commit.identity,
        executor,
        time_basis: Id("clock.monotonic"),
        not_before_tick: 3,
        expires_at_tick: 15,
    };
    execution.identity = execution.computed_semantic_hash().unwrap();
    validate_administrative_proof(
        AdministrativeProof {
            proposal,
            policy,
            approvals: &approvals,
            commit,
            execution,
        },
        ContainmentContext {
            subject,
            time_basis: Id("clock.monotonic"),
            now_tick: 5,
        },
    )
    .unwrap_err()
    .code()
}

fn clear_subject() -> AdministrativeSubject<'static> {
    AdministrativeSubject {
        realm: Id("realm.alpha"),
        entity: Id("host.hazard"),
        plan: hash(61),
        epoch: 1,
        artifact: None,
        budget: None,
    }
}

fn invalid_self_clear_proof(
    profile: HazardousHostProfile<'static>,
) -> AdministrativeProof<'static> {
    static EMPTY_APPROVERS: [AdministrativeApprover<'static>; 0] = [];
    static EMPTY_APPROVALS: [AdministrativeApproval<'static>; 0] = [];
    static EMPTY_HASHES: [SemanticHash; 0] = [];
    static BENEFICIARIES: [AdministrativeSubject<'static>; 1] = [AdministrativeSubject {
        realm: Id("realm.alpha"),
        entity: Id("host.hazard"),
        plan: SemanticHash::from_bytes([61; 32]),
        epoch: 1,
        artifact: None,
        budget: None,
    }];
    let requester = principal("host.hazard", 61);
    let subject = clear_subject();
    AdministrativeProof {
        proposal: AdministrativeProposal {
            schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
            identity: ZERO,
            id: Id("proposal.self-clear"),
            effect_class: profile.clear_effect_class,
            operation: profile.clear_operation,
            requester,
            subject,
            beneficiaries: &BENEFICIARIES,
            predecessor_plan: Some(hash(61)),
            delegation: None,
            protected_handle: Some(profile.inhibit_boundary),
            ceremony: Some(profile.clear_ceremony),
            time_basis: profile.time_basis,
            created_at_tick: 1,
            expires_at_tick: 20,
        },
        policy: ContainmentPolicy {
            schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
            identity: ZERO,
            descriptor: pin("policy.clear", 62),
            effect_class: profile.clear_effect_class,
            approvers: &EMPTY_APPROVERS,
            committer: AdministrativeApprover {
                realm: Id("realm.alpha"),
                entity: Id("host.hazard"),
                key: Id("key.fixture"),
                profile: pin("profile.member", 60),
                failure_domain: pin("failure.same", 63),
            },
            executor: AdministrativeApprover {
                realm: Id("realm.alpha"),
                entity: Id("host.hazard"),
                key: Id("key.fixture"),
                profile: pin("profile.member", 60),
                failure_domain: pin("failure.same", 63),
            },
            minimum_approvals: 1,
            minimum_failure_domains: 1,
            requester_independence: true,
            beneficiary_independence: true,
            successor_independence: true,
            delegation_ceiling: None,
            ceremony: Some(profile.clear_ceremony),
        },
        approvals: &EMPTY_APPROVALS,
        commit: AdministrativeCommit {
            schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
            identity: ZERO,
            id: Id("commit.self-clear"),
            proposal_identity: ZERO,
            policy_identity: ZERO,
            approvals: &EMPTY_HASHES,
            committed_by: requester,
            committed_at_tick: 2,
        },
        execution: AdministrativeExecution {
            schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
            identity: ZERO,
            id: Id("execution.self-clear"),
            proposal_identity: ZERO,
            commit_identity: ZERO,
            executor: requester,
            time_basis: profile.time_basis,
            not_before_tick: 2,
            expires_at_tick: 20,
        },
    }
}

fn signed_artifact() -> (
    ArtifactManifest<'static>,
    ArtifactTrustPolicy<'static>,
    [SignatureVerification<'static>; 1],
) {
    static SIGNATURES: [ArtifactSignature<'static>; 1] = [ArtifactSignature {
        scheme: Id("ed25519"),
        signer: Id("signer.online"),
        signature_artifact: ArtifactDigest::from_bytes([70; 32]),
        provenance_evidence: Some(ArtifactDigest::from_bytes([71; 32])),
    }];
    static LOCATIONS: [ArtifactLocation<'static>; 1] = [ArtifactLocation {
        kind: ArtifactLocationKind::BundlePath,
        locator: "bin/adversarial",
    }];
    static TRUSTED: [Id<'static>; 1] = [Id("signer.online")];
    let mut manifest = ArtifactManifest {
        schema_version: 0,
        identity: ZERO,
        id: Id("artifact.fixture"),
        digest: ArtifactDigest::from_bytes([72; 32]),
        media_type: "application/wasm",
        byte_size: 12,
        target: Some(Id("wasm32-wasip2")),
        abi: Some(Id("component-v1")),
        provenance: ArtifactProvenance {
            builder: Id("builder.compromised"),
            source_digest: ArtifactDigest::from_bytes([73; 32]),
            build_recipe_digest: ArtifactDigest::from_bytes([74; 32]),
            reproducible: true,
        },
        signatures: &SIGNATURES,
        license_expressions: &["Apache-2.0"],
        notices: &[],
        sbom: Some(conduit_core::ManifestArtifactRef {
            id: Id("sbom.fixture"),
            digest: ArtifactDigest::from_bytes([75; 32]),
            role: Id("spdx"),
            required: true,
        }),
        source: None,
        related_artifacts: &[],
        locations: &LOCATIONS,
    };
    manifest.identity = manifest.computed_semantic_hash(&mut [ZERO; 8]).unwrap();
    (
        manifest,
        ArtifactTrustPolicy {
            require_signature: true,
            require_provenance_evidence: true,
            require_known_license: true,
            require_sbom: true,
            trusted_signers: &TRUSTED,
        },
        [SignatureVerification {
            signer: Id("signer.online"),
            scheme: Id("ed25519"),
            verified: true,
            verifier: Id("verifier.fixture"),
            evidence_digest: ArtifactDigest::from_bytes([71; 32]),
        }],
    )
}

fn verify_signed_artifact() {
    let (manifest, policy, verification) = signed_artifact();
    verify_artifact_candidate(
        &manifest,
        manifest.digest,
        manifest.byte_size,
        manifest.target,
        manifest.abi,
        policy,
        &verification,
    )
    .unwrap();
}

fn run_case(case: &Case, maximum_steps: usize) -> (Value, Vec<String>, Value) {
    let evidence_slots = if case.id == "evidence-exhaustion-before-effect" {
        1
    } else {
        16
    };
    let mut system = FakeSystem::new(maximum_steps, evidence_slots);
    let outcome = match case.id.as_str() {
        "self-grant-and-successor" => {
            system.rejected_step("self-grant-approval", |_| {
                validate_support_graph(
                    &[AdministrativeSupportEdge {
                        supporter: hash(1),
                        beneficiary: hash(1),
                    }],
                    &mut [false; 2],
                )
                .unwrap_err()
                .code()
            });
            system.rejected_step("successor-activation", |_| successor_self_approval_reason())
        }
        "cyclic-cross-plan-member-approval" => system.rejected_step("support-closure", |_| {
            validate_support_graph(
                &[
                    AdministrativeSupportEdge {
                        supporter: hash(1),
                        beneficiary: hash(2),
                    },
                    AdministrativeSupportEdge {
                        supporter: hash(2),
                        beneficiary: hash(1),
                    },
                ],
                &mut [false; 4],
            )
            .unwrap_err()
            .code()
        }),
        "recursive-enrollment-sybil-churn" => {
            system.accepted_step("first-enrollment", |system| {
                commit_population(system, 10, 10, 1)
            });
            system.rejected_step("second-budget-reservation", |system| {
                let policy = budget_policy(1, 16);
                system
                    .budget
                    .reserve(
                        request(policy, 11, 11, 2, "realm.clone", 3),
                        AuthorityTime {
                            basis: Id("clock.monotonic"),
                            tick: 3,
                        },
                        true,
                    )
                    .unwrap_err()
                    .code()
            })
        }
        "discover-enroll-install-execute-redelegate" => {
            system.accepted_step("plan-validation-and-discovery", |_| validate_minimal_plan());
            system.accepted_step("enrollment", |system| commit_population(system, 12, 12, 1));
            system.accepted_step("artifact-verification", |_| verify_signed_artifact());
            system.rejected_step("install-authority", |_| {
                conduit_core::require_realm_operation_authority(None)
                    .unwrap_err()
                    .code()
            })
        }
        "budget-reset-across-lifecycle" => {
            system.accepted_step("consume-lifetime-unit", |system| {
                commit_population(system, 13, 13, 1)
            });
            system.accepted_step("recover-checkpoint", |system| {
                system.budget = PersistentBudgetLedger::recover(
                    budget_policy(1, 16),
                    system.budget.checkpoint(),
                )
                .unwrap();
            });
            system.rejected_step("post-recovery-reservation", |system| {
                let policy = budget_policy(1, 16);
                system
                    .budget
                    .reserve(
                        request(policy, 14, 14, 99, "realm.new", 3),
                        AuthorityTime {
                            basis: Id("clock.monotonic"),
                            tick: 3,
                        },
                        true,
                    )
                    .unwrap_err()
                    .code()
            })
        }
        "concurrent-replay-double-spend" => {
            system.accepted_step("transport-replay-and-partition", |_| {
                exercise_transport_replay_partition_and_transition()
            });
            system.accepted_step("first-reservation", |system| {
                let policy = budget_policy(1, 16);
                system
                    .budget
                    .reserve(
                        request(policy, 15, 15, 1, "realm.alpha", 1),
                        AuthorityTime {
                            basis: Id("clock.monotonic"),
                            tick: 1,
                        },
                        true,
                    )
                    .unwrap();
            });
            system.rejected_step("competing-reservation", |system| {
                let policy = budget_policy(1, 16);
                system
                    .budget
                    .reserve(
                        request(policy, 16, 16, 2, "realm.beta", 1),
                        AuthorityTime {
                            basis: Id("clock.monotonic"),
                            tick: 1,
                        },
                        true,
                    )
                    .unwrap_err()
                    .code()
            })
        }
        "authenticated-but-wrong-authority" => {
            system.accepted_step("passport-and-ordinary-authority-verified", |_| {
                validate_active_passport();
                validate_exact_ordinary_authority();
            });
            system.rejected_step("authority-at-use", |_| {
                conduit_core::require_realm_operation_authority(None)
                    .unwrap_err()
                    .code()
            })
        }
        "compromised-member-and-online-signer" => {
            system.accepted_step("passport-and-signature-verification", |_| {
                validate_active_passport();
                verify_signed_artifact();
            });
            system.rejected_step("install-authority", |_| {
                conduit_core::require_realm_operation_authority(None)
                    .unwrap_err()
                    .code()
            })
        }
        "federation-laundering" => system.rejected_step("federation-direction", |_| {
            let stream = pin("stream.allowed", 80);
            validate_federation(
                conduit_core::FederationPolicy {
                    id: Id("federation.a-to-b"),
                    local_realm: Id("realm.a"),
                    remote_realm: Id("realm.b"),
                    local_root_epoch: 1,
                    remote_root_epoch: 1,
                    time_basis: Id("clock.monotonic"),
                    expires_at_tick: 20,
                    allow_identity: true,
                    allow_event_verification: true,
                    allow_transport_admission: false,
                    allow_grant_delegation: false,
                    allowed_streams: &[stream],
                    receipt: hash(85),
                },
                Id("realm.b"),
                Id("realm.a"),
                stream,
                Id("clock.monotonic"),
                2,
                true,
            )
            .unwrap_err()
            .code()
        }),
        "public-browser-administration-escalation" => {
            system.rejected_step("quarantine-validation", |_| {
                validate_quarantined_member(MemberSecurityState {
                    entity: Id("browser.member"),
                    passport: hash(86),
                    disposition: MemberDisposition::Quarantined,
                    roles: &[],
                    grants: &[hash(81)],
                    delegations: &[],
                    federations: 0,
                    installed_providers: 0,
                    protected_subscriptions: 0,
                    remote_plan_activations: 0,
                    administrative_effects: 1,
                    actuating_effects: 0,
                })
                .unwrap_err()
                .code()
            })
        }
        "evidence-exhaustion-before-effect" => {
            system.accepted_step("runtime-evidence-capacity", |_| {
                exercise_runtime_evidence_capacity()
            });
            let reservation = system.accepted_step("reservation-evidence", |system| {
                let policy = budget_policy(1, 1);
                system
                    .budget
                    .reserve(
                        request(policy, 17, 17, 1, "realm.alpha", 1),
                        AuthorityTime {
                            basis: Id("clock.monotonic"),
                            tick: 1,
                        },
                        true,
                    )
                    .unwrap()
                    .0
            });
            system.rejected_step("effect-commit", |system| {
                system
                    .budget
                    .commit(reservation.identity, 2)
                    .unwrap_err()
                    .code()
            })
        }
        "recovery-flapping" => system.rejected_step("recovery-scope", |_| {
            validate_recovery_narrowing(envelope(1), envelope(2))
                .unwrap_err()
                .code()
        }),
        "stale-partition-clock-confusion" => {
            system.rejected_step("freshness-validation", |system| {
                let policy = budget_policy(1, 16);
                let status = system
                    .budget
                    .status(pin("ledger.primary", 82), 1, 3)
                    .unwrap();
                validate_policy_budget_status(
                    policy,
                    status,
                    AuthorityTime {
                        basis: Id("clock.monotonic"),
                        tick: 3,
                    },
                    1,
                )
                .unwrap_err()
                .code()
            })
        }
        "implementation-and-graph-downgrade" => {
            system.rejected_step("host-binding-validation", |_| {
                let profile = hazardous_profile();
                validate_hazardous_host_binding(
                    hazardous_binding(profile, ImplementationConfinement::UnconfinedNative),
                    Id("clock.monotonic"),
                    2,
                    &mut [ZERO; 4],
                )
                .unwrap_err()
                .code()
            })
        }
        "malicious-implementation-exceeds-profile" => {
            system.rejected_step("confinement-observation", |_| {
                let profile = hazardous_profile();
                validate_hazardous_host_binding(
                    hazardous_binding(profile, ImplementationConfinement::UnconfinedNative),
                    Id("clock.monotonic"),
                    2,
                    &mut [ZERO; 4],
                )
                .unwrap_err()
                .code()
            })
        }
        "old-hazardous-command-after-transition" => {
            system.accepted_step("plan-transition", |system| {
                system.hazard = conduit_core::inhibit_hazardous_host(
                    system.hazard,
                    hash(95),
                    InhibitCause::PlanTransition,
                );
            });
            system.rejected_step("effect-boundary", |system| {
                let profile = hazardous_profile();
                conduit_core::accept_hazardous_command(
                    profile,
                    system.hazard,
                    HazardousCommand {
                        plan: hash(83),
                        epoch: 1,
                        authority: hash(84),
                        sequence: 1,
                        time_basis: profile.time_basis,
                        issued_at_tick: 1,
                        expires_at_tick: 3,
                        values: &[EnvelopeValue {
                            dimension: profile.envelope[0].dimension,
                            value: 1,
                        }],
                    },
                    2,
                )
                .unwrap_err()
                .code()
            })
        }
        "self-clear-by-inhibiting-plan" => system.rejected_step("clear-ceremony", |system| {
            let profile = hazardous_profile();
            conduit_core::clear_inhibit(
                profile,
                HazardControlState {
                    profile_identity: profile.identity,
                    safe_state_identity: profile.safe_state.semantic_hash,
                    ..system.hazard
                },
                InhibitClearRequest {
                    profile_identity: profile.identity,
                    host: Id("host.hazard"),
                    latch_identity: system.hazard.latch_identity,
                    latch_generation: system.hazard.latch_generation,
                    subject: clear_subject(),
                    proof: invalid_self_clear_proof(profile),
                    physical_presence_receipt: None,
                },
                3,
            )
            .unwrap_err()
            .code()
        }),
        id => panic!("unimplemented adversarial case {id}"),
    };
    assert!(!system.evidence.is_empty());
    let final_state = system.final_state();
    (outcome, system.evidence, final_state)
}

#[test]
fn every_adversarial_trace_executes_and_replays_exactly() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(fixture.cases.len(), 17);
    for case in &fixture.cases {
        assert!(!case.initial.is_empty());
        assert!(!case.attacker_capabilities.is_empty());
        assert!(!case.operations.is_empty());
        assert!(!case.expected_rejection_step.is_empty());
        let first = run_case(case, fixture.maximum_steps_per_trace);
        let replay = run_case(case, fixture.maximum_steps_per_trace);
        assert_eq!(first, replay, "trace replay drifted for {}", case.id);
        assert_eq!(first.0, case.expected, "trace outcome for {}", case.id);
        assert_eq!(
            first.2, case.final_state,
            "final protected state for {}",
            case.id
        );
        assert!(
            first
                .1
                .last()
                .is_some_and(|record| record.contains(&case.expected_rejection_step)),
            "rejection commit point for {}",
            case.id
        );
    }
}

#[test]
fn fast_seeded_campaign_checks_global_properties_after_every_step() {
    run_seeded_campaign(1380991557, 64);
}

#[test]
#[ignore = "scheduled reproducible containment campaign"]
fn scheduled_seeded_campaign() {
    let seed = std::env::var("CONDUIT_ADVERSARIAL_SEED")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1380991557);
    let traces = std::env::var("CONDUIT_ADVERSARIAL_TRACES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(4096);
    run_seeded_campaign(seed, traces);
}

fn run_seeded_campaign(seed: u64, traces: usize) {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(seed, fixture.seed);
    let mut state = seed;
    for trace_index in 0..traces {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let case_index = usize::try_from(state % fixture.cases.len() as u64).unwrap();
        let case = &fixture.cases[case_index];
        let result = std::panic::catch_unwind(|| run_case(case, fixture.maximum_steps_per_trace));
        if let Err(payload) = result {
            panic!(
                "adversarial campaign failed: seed={seed} trace={trace_index} minimal_trace={} operations={:?} payload={payload:?}",
                case.id, case.operations,
            );
        }
    }
}
