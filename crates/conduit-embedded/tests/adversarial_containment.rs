use conduit_core::{
    AdministrativeSupportEdge, AuthorityTime, HAZARDOUS_HOST_PROFILE_SCHEMA_VERSION,
    HazardControlPhase, HazardControlState, HazardousCommand, HazardousHostProfile, Id,
    InhibitCause, POLICY_BUDGET_SCHEMA_VERSION, PersistentBudgetLedger, PersistentBudgetPolicy,
    PinnedDescriptor, PolicyBudgetAnchor, PolicyBudgetConsumer, PolicyBudgetLimits,
    PolicyBudgetRequest, RollingLimit, SemanticHash, accept_hazardous_command,
    inhibit_hazardous_host, validate_support_graph,
};

const FIXTURE: &str = include_str!("../../../conformance/c5/adversarial-containment.json");
const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);

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

fn policy() -> PersistentBudgetPolicy<'static> {
    let mut policy = PersistentBudgetPolicy {
        schema_version: POLICY_BUDGET_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("budget.constrained", 1),
        owner: pin("owner.independent", 2),
        subject: pin("subject.population", 3),
        anchor: PolicyBudgetAnchor::Host(Id("host.constrained")),
        action: Id("action.enroll"),
        resource_class: pin("resource.member", 4),
        time_basis: Id("clock.monotonic"),
        limits: PolicyBudgetLimits {
            current_stock: Some(1),
            rolling: Some(RollingLimit {
                units: 1,
                window_ticks: 100,
            }),
            lifetime: Some(1),
        },
        reservation_ttl_ticks: 10,
        lease: None,
        audit_id: Id("audit.constrained"),
        persistence_profile: pin("persistence.atomic", 5),
        maximum_reservations: 2,
        maximum_evidence_events: 8,
    };
    policy.identity = policy.computed_semantic_hash().unwrap();
    policy
}

fn request(
    policy: PersistentBudgetPolicy<'static>,
    correlation: u8,
    epoch: u64,
) -> PolicyBudgetRequest<'static> {
    let mut request = PolicyBudgetRequest {
        identity: ZERO,
        correlation: hash(correlation),
        policy_identity: policy.identity,
        consumer: PolicyBudgetConsumer {
            realm: Id("realm.fixture"),
            plan: hash(correlation),
            epoch,
            generation: epoch,
            run: Id("run.constrained"),
        },
        action: policy.action,
        units: 1,
        requested_at_tick: epoch,
        lease: None,
    };
    request.identity = request.computed_semantic_hash().unwrap();
    request
}

#[test]
fn constrained_subset_executes_allocator_free_production_paths() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        fixture["profiles"]["constrained"],
        serde_json::json!("normalized-subset")
    );

    assert_eq!(
        validate_support_graph(
            &[AdministrativeSupportEdge {
                supporter: hash(10),
                beneficiary: hash(10),
            }],
            &mut [false; 2],
        )
        .unwrap_err()
        .code(),
        "CND-CTN-014"
    );

    let policy = policy();
    let mut ledger = PersistentBudgetLedger::<2>::new(policy, hash(20), 0).unwrap();
    let (reservation, _) = ledger
        .reserve(
            request(policy, 21, 1),
            AuthorityTime {
                basis: Id("clock.monotonic"),
                tick: 1,
            },
            true,
        )
        .unwrap();
    ledger.commit(reservation.identity, 2).unwrap();
    let checkpoint = ledger.checkpoint();
    let mut recovered = PersistentBudgetLedger::<2>::recover(policy, checkpoint).unwrap();
    assert_eq!(
        recovered
            .reserve(
                request(policy, 22, 3),
                AuthorityTime {
                    basis: Id("clock.monotonic"),
                    tick: 3,
                },
                true,
            )
            .unwrap_err()
            .code(),
        "CND-PBG-008"
    );
    assert_eq!(recovered.checkpoint().lifetime_committed, 1);

    let inhibited = inhibit_hazardous_host(
        HazardControlState::safe_disarmed(1, hash(30)),
        hash(31),
        InhibitCause::Partition,
    );
    assert_eq!(inhibited.phase, HazardControlPhase::Inhibited);
    assert_eq!(inhibited.plan, ZERO);
    assert_eq!(inhibited.command_authority, ZERO);
    let profile = HazardousHostProfile {
        schema_version: HAZARDOUS_HOST_PROFILE_SCHEMA_VERSION,
        identity: hash(32),
        descriptor: pin("profile.hazardous", 33),
        safe_state: pin("safe.state", 34),
        inhibit_boundary: pin("inhibit.boundary", 35),
        watchdog: pin("watchdog.boundary", 36),
        effect_boundary: pin("effect.boundary", 37),
        command_effect_class: pin("effect.command", 38),
        clear_effect_class: pin("effect.clear", 39),
        clear_operation: pin("operation.clear", 40),
        clear_ceremony: pin("ceremony.clear", 41),
        time_basis: Id("clock.monotonic"),
        maximum_command_horizon_ticks: 1,
        maximum_observation_age_ticks: 1,
        maximum_evidence_records: 1,
        require_physical_presence_to_clear: true,
        require_isolated_implementation: true,
        envelope: &[],
    };
    assert_eq!(
        accept_hazardous_command(
            profile,
            inhibited,
            HazardousCommand {
                plan: hash(42),
                epoch: 1,
                authority: hash(43),
                sequence: 1,
                time_basis: Id("clock.monotonic"),
                issued_at_tick: 1,
                expires_at_tick: 2,
                values: &[],
            },
            1,
        )
        .unwrap_err()
        .code(),
        "CND-INH-012"
    );
}
