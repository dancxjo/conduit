use conduit_core::{
    AuthorityTime, ContainmentContext, Id, POLICY_BUDGET_SCHEMA_VERSION, PersistentBudgetLedger,
    PersistentBudgetPolicy, PinnedDescriptor, PolicyBudgetAnchor, PolicyBudgetAvailability,
    PolicyBudgetConsumer, PolicyBudgetLease, PolicyBudgetLimits, PolicyBudgetReason,
    PolicyBudgetRequest, PolicyBudgetTransition, PolicyLeaseRule, RollingLimit, SemanticHash,
    validate_offline_lease, validate_policy_budget_bindings, validate_policy_budget_replacement,
    validate_policy_budget_status,
};

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 1,
        semantic_hash: hash(byte),
    }
}

fn policy(
    limit: u64,
    evidence: u32,
    anchor: PolicyBudgetAnchor<'static>,
) -> PersistentBudgetPolicy<'static> {
    let mut policy = PersistentBudgetPolicy {
        schema_version: POLICY_BUDGET_SCHEMA_VERSION,
        identity: hash(0),
        descriptor: pin("budget.enrollment", 1),
        owner: pin("owner.site-operations", 2),
        subject: pin("subject.members", 3),
        anchor,
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
        lease: Some(PolicyLeaseRule {
            maximum_ticks: 20,
            renewal_authority: pin("authority.lease-renewal", 5),
            offline_allowed: true,
        }),
        audit_id: Id("audit.enrollment"),
        persistence_profile: pin("persistence.atomic-durable", 6),
        maximum_reservations: 16,
        maximum_evidence_events: evidence,
    };
    policy.identity = policy.computed_semantic_hash().unwrap();
    policy
}

fn consumer(
    realm: &'static str,
    plan: u8,
    epoch: u64,
    generation: u64,
) -> PolicyBudgetConsumer<'static> {
    PolicyBudgetConsumer {
        realm: Id(realm),
        plan: hash(plan),
        epoch,
        generation,
        run: Id("run.fixture"),
    }
}

fn lease(
    policy: PersistentBudgetPolicy<'static>,
    expires_at_tick: u64,
) -> PolicyBudgetLease<'static> {
    let mut lease = PolicyBudgetLease {
        schema_version: POLICY_BUDGET_SCHEMA_VERSION,
        identity: hash(0),
        policy_identity: policy.identity,
        holder: pin("holder.offline-host", 7),
        renewal_authority: policy.lease.unwrap().renewal_authority,
        time_basis: policy.time_basis,
        issued_at_tick: 1,
        expires_at_tick,
        offline: true,
    };
    lease.identity = lease.computed_semantic_hash().unwrap();
    lease
}

fn request(
    policy: PersistentBudgetPolicy<'static>,
    correlation: u8,
    consumer: PolicyBudgetConsumer<'static>,
    tick: u64,
) -> PolicyBudgetRequest<'static> {
    let mut request = PolicyBudgetRequest {
        identity: hash(0),
        correlation: hash(correlation),
        policy_identity: policy.identity,
        consumer,
        action: policy.action,
        units: 1,
        requested_at_tick: tick,
        lease: None,
    };
    request.identity = request.computed_semantic_hash().unwrap();
    request
}

fn now(tick: u64) -> AuthorityTime<'static> {
    AuthorityTime {
        basis: Id("clock.monotonic"),
        tick,
    }
}

fn committed(
    ledger: &mut PersistentBudgetLedger<'static, 16>,
    request: PolicyBudgetRequest<'static>,
    tick: u64,
) {
    let (reservation, _) = ledger.reserve(request, now(tick), true).unwrap();
    ledger.commit(reservation.identity, tick + 1).unwrap();
}

#[test]
fn lifetime_limit_survives_epoch_plan_run_and_reboot() {
    let policy = policy(10, 100, PolicyBudgetAnchor::Host(Id("host.alpha")));
    let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0).unwrap();
    for index in 0..10_u8 {
        committed(
            &mut ledger,
            request(
                policy,
                30 + index,
                consumer(
                    "realm.alpha",
                    40 + index,
                    u64::from(index),
                    u64::from(index),
                ),
                u64::from(index) * 2,
            ),
            u64::from(index) * 2,
        );
    }
    let checkpoint = ledger.checkpoint();
    let mut recovered = PersistentBudgetLedger::<16>::recover(policy, checkpoint).unwrap();
    let eleventh = request(policy, 70, consumer("realm.alpha", 80, 99, 99), 30);
    assert_eq!(
        recovered.reserve(eleventh, now(30), true).unwrap_err(),
        PolicyBudgetReason::CapacityExceeded
    );
}

#[test]
fn recovery_rejects_counter_tampering_under_an_old_checkpoint_identity() {
    let policy = policy(2, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
    let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0).unwrap();
    committed(
        &mut ledger,
        request(policy, 30, consumer("realm.alpha", 40, 1, 1), 1),
        1,
    );
    let mut tampered = ledger.checkpoint();
    tampered.current_stock = 0;
    tampered.lifetime_committed = 0;
    assert_eq!(
        PersistentBudgetLedger::<16>::recover(policy, tampered).unwrap_err(),
        PolicyBudgetReason::RecoveryGap
    );
}

#[test]
fn duplicate_reservation_and_commit_are_idempotent() {
    let policy = policy(2, 20, PolicyBudgetAnchor::Site(Id("site.alpha")));
    let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0).unwrap();
    let request = request(policy, 30, consumer("realm.alpha", 40, 1, 1), 1);
    let (first, transition) = ledger.reserve(request, now(1), true).unwrap();
    assert_eq!(transition, PolicyBudgetTransition::Applied);
    let (duplicate, transition) = ledger.reserve(request, now(1), true).unwrap();
    assert_eq!(duplicate.identity, first.identity);
    assert_eq!(transition, PolicyBudgetTransition::Idempotent);
    assert_eq!(
        ledger.commit(first.identity, 2).unwrap(),
        PolicyBudgetTransition::Applied
    );
    assert_eq!(
        ledger.commit(first.identity, 2).unwrap(),
        PolicyBudgetTransition::Idempotent
    );
}

#[test]
fn last_unit_race_has_one_winner() {
    let policy = policy(1, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
    let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0).unwrap();
    let one = request(policy, 30, consumer("realm.alpha", 40, 1, 1), 1);
    let two = request(policy, 31, consumer("realm.beta", 41, 8, 3), 1);
    let (winner, _) = ledger.reserve(one, now(1), true).unwrap();
    assert_eq!(
        ledger.reserve(two, now(1), true).unwrap_err(),
        PolicyBudgetReason::CapacityExceeded
    );
    ledger.commit(winner.identity, 2).unwrap();
}

#[test]
fn expiry_releases_only_uncommitted_capacity() {
    let policy = policy(1, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
    let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0).unwrap();
    let first = request(policy, 30, consumer("realm.alpha", 40, 1, 1), 1);
    ledger.reserve(first, now(1), true).unwrap();
    assert_eq!(ledger.expire(11).unwrap(), 1);
    let second = request(policy, 31, consumer("realm.alpha", 41, 2, 2), 11);
    committed(&mut ledger, second, 11);
}

#[test]
fn generation_and_realm_changes_do_not_select_new_ledgers() {
    let policy = policy(1, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
    let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0).unwrap();
    committed(
        &mut ledger,
        request(policy, 30, consumer("realm.alpha", 40, 1, 1), 1),
        1,
    );
    for replacement in [
        request(policy, 31, consumer("realm.alpha", 41, 2, 2), 3),
        request(policy, 32, consumer("realm.new", 42, 1, 1), 3),
    ] {
        assert_eq!(
            ledger.reserve(replacement, now(3), true).unwrap_err(),
            PolicyBudgetReason::CapacityExceeded
        );
    }
}

#[test]
fn partition_stale_status_and_expired_offline_lease_fail_closed() {
    let policy = policy(2, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
    let ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0).unwrap();
    let mut status = ledger.status(pin("ledger.primary", 8), 1, 5).unwrap();
    status.availability = PolicyBudgetAvailability::Unavailable;
    status.identity = status.computed_semantic_hash().unwrap();
    assert_eq!(
        validate_policy_budget_status(policy, status, now(2), 1).unwrap_err(),
        PolicyBudgetReason::LedgerUnavailable
    );
    let fresh = ledger.status(pin("ledger.primary", 8), 1, 5).unwrap();
    assert_eq!(
        validate_policy_budget_status(policy, fresh, now(5), 1).unwrap_err(),
        PolicyBudgetReason::StaleStatus
    );
    assert_eq!(
        validate_offline_lease(policy, lease(policy, 4), now(4)).unwrap_err(),
        PolicyBudgetReason::LeaseExpired
    );
}

#[test]
fn increases_require_independent_containment_proof() {
    let old = policy(1, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
    let mut increased = old;
    increased.limits.current_stock = Some(2);
    increased.limits.lifetime = Some(2);
    increased.identity = increased.computed_semantic_hash().unwrap();
    let subject = conduit_core::AdministrativeSubject {
        realm: Id("realm.alpha"),
        entity: Id("entity.target"),
        plan: hash(90),
        epoch: 1,
        artifact: None,
        budget: Some(increased.descriptor),
    };
    assert_eq!(
        validate_policy_budget_replacement(
            old,
            increased,
            None,
            ContainmentContext {
                subject,
                time_basis: Id("clock.monotonic"),
                now_tick: 2,
            },
        )
        .unwrap_err(),
        PolicyBudgetReason::ApprovalRequired
    );
}

#[test]
fn evidence_and_storage_exhaust_before_effect() {
    let policy = policy(2, 1, PolicyBudgetAnchor::Host(Id("host.alpha")));
    let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0).unwrap();
    let request = request(policy, 30, consumer("realm.alpha", 40, 1, 1), 1);
    let (reservation, _) = ledger.reserve(request, now(1), true).unwrap();
    assert_eq!(
        ledger.commit(reservation.identity, 2).unwrap_err(),
        PolicyBudgetReason::EvidenceExhausted
    );
    assert_eq!(ledger.checkpoint().current_stock, 0);
    assert_eq!(ledger.checkpoint().lifetime_committed, 0);
}

#[test]
fn stock_release_does_not_refund_lifetime() {
    let policy = policy(1, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
    let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0).unwrap();
    let first = request(policy, 30, consumer("realm.alpha", 40, 1, 1), 1);
    let (reservation, _) = ledger.reserve(first, now(1), true).unwrap();
    ledger.commit(reservation.identity, 2).unwrap();
    ledger.release(reservation.identity).unwrap();
    assert_eq!(ledger.checkpoint().current_stock, 0);
    assert_eq!(ledger.checkpoint().lifetime_committed, 1);
    let second = request(policy, 31, consumer("realm.alpha", 41, 2, 2), 4);
    assert_eq!(
        ledger.reserve(second, now(4), true).unwrap_err(),
        PolicyBudgetReason::CapacityExceeded
    );
}

#[test]
fn bounded_compaction_declares_replay_gap() {
    let policy = policy(2, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
    let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0).unwrap();
    let first = request(policy, 30, consumer("realm.alpha", 40, 1, 1), 1);
    let (reservation, _) = ledger.reserve(first, now(1), true).unwrap();
    ledger.release(reservation.identity).unwrap();
    ledger.compact(2).unwrap();
    assert_eq!(
        ledger.reserve(first, now(4), true).unwrap_err(),
        PolicyBudgetReason::RecoveryGap
    );
}

#[test]
fn realm_and_host_scopes_are_all_required() {
    let realm_policy = policy(2, 20, PolicyBudgetAnchor::Realm(Id("realm.alpha")));
    let mut host_policy = policy(1, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
    host_policy.descriptor = pin("budget.host-enrollment", 9);
    host_policy.identity = host_policy.computed_semantic_hash().unwrap();
    let realm = PersistentBudgetLedger::<16>::new(realm_policy, hash(20), 0).unwrap();
    let host = PersistentBudgetLedger::<16>::new(host_policy, hash(21), 0).unwrap();
    let policies = [realm_policy, host_policy];
    let statuses = [
        realm.status(pin("ledger.realm", 10), 1, 5).unwrap(),
        host.status(pin("ledger.host", 11), 1, 5).unwrap(),
    ];
    validate_policy_budget_bindings(&policies, &statuses, now(2), 1).unwrap();
}

fn run_fixture(scenario: &str) -> Result<&'static str, PolicyBudgetReason> {
    match scenario {
        "lifetime-cross-epoch" => {
            let policy = policy(10, 100, PolicyBudgetAnchor::Host(Id("host.alpha")));
            let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0)?;
            for index in 0..10_u8 {
                let request = request(
                    policy,
                    30 + index,
                    consumer(
                        "realm.alpha",
                        40 + index,
                        u64::from(index),
                        u64::from(index),
                    ),
                    u64::from(index) * 2,
                );
                let (reservation, _) = ledger.reserve(request, now(u64::from(index) * 2), true)?;
                ledger.commit(reservation.identity, u64::from(index) * 2 + 1)?;
            }
            let eleventh = request(policy, 70, consumer("realm.alpha", 80, 99, 99), 30);
            ledger.reserve(eleventh, now(30), true)?;
            Ok("accepted")
        }
        "recovery-preserves" => {
            let policy = policy(1, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
            let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0)?;
            let first = request(policy, 30, consumer("realm.alpha", 40, 1, 1), 1);
            let (reservation, _) = ledger.reserve(first, now(1), true)?;
            ledger.commit(reservation.identity, 2)?;
            let mut recovered = PersistentBudgetLedger::<16>::recover(policy, ledger.checkpoint())?;
            let replacement = request(policy, 31, consumer("realm.alpha", 41, 9, 9), 3);
            recovered.reserve(replacement, now(3), true)?;
            Ok("accepted")
        }
        "idempotent" => {
            let policy = policy(2, 20, PolicyBudgetAnchor::Site(Id("site.alpha")));
            let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0)?;
            let request = request(policy, 30, consumer("realm.alpha", 40, 1, 1), 1);
            let (first, first_transition) = ledger.reserve(request, now(1), true)?;
            let (duplicate, reserve_transition) = ledger.reserve(request, now(1), true)?;
            let commit_transition = ledger.commit(first.identity, 2)?;
            let duplicate_commit = ledger.commit(first.identity, 2)?;
            if duplicate.identity == first.identity
                && first_transition == PolicyBudgetTransition::Applied
                && reserve_transition == PolicyBudgetTransition::Idempotent
                && commit_transition == PolicyBudgetTransition::Applied
                && duplicate_commit == PolicyBudgetTransition::Idempotent
            {
                Ok("accepted")
            } else {
                Err(PolicyBudgetReason::TransitionInvalid)
            }
        }
        "last-unit-race" => {
            let policy = policy(1, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
            let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0)?;
            let one = request(policy, 30, consumer("realm.alpha", 40, 1, 1), 1);
            let two = request(policy, 31, consumer("realm.beta", 41, 8, 3), 1);
            let (winner, _) = ledger.reserve(one, now(1), true)?;
            let loser = ledger.reserve(two, now(1), true);
            ledger.commit(winner.identity, 2)?;
            if loser == Err(PolicyBudgetReason::CapacityExceeded) {
                Ok("one-commit")
            } else {
                Err(PolicyBudgetReason::ReservationConflict)
            }
        }
        "expiry-release" => {
            let policy = policy(1, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
            let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0)?;
            let first = request(policy, 30, consumer("realm.alpha", 40, 1, 1), 1);
            ledger.reserve(first, now(1), true)?;
            if ledger.expire(11)? != 1 {
                return Err(PolicyBudgetReason::TransitionInvalid);
            }
            let second = request(policy, 31, consumer("realm.alpha", 41, 2, 2), 11);
            let (reservation, _) = ledger.reserve(second, now(11), true)?;
            ledger.commit(reservation.identity, 12)?;
            Ok("accepted")
        }
        "generation-share" | "realm-evasion" => {
            let policy = policy(1, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
            let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0)?;
            let first = request(policy, 30, consumer("realm.alpha", 40, 1, 1), 1);
            let (reservation, _) = ledger.reserve(first, now(1), true)?;
            ledger.commit(reservation.identity, 2)?;
            let replacement = if scenario == "generation-share" {
                request(policy, 31, consumer("realm.alpha", 41, 2, 2), 3)
            } else {
                request(policy, 31, consumer("realm.new", 41, 1, 1), 3)
            };
            ledger.reserve(replacement, now(3), true)?;
            Ok("accepted")
        }
        "partition" | "stale" => {
            let policy = policy(2, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
            let ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0)?;
            let mut status = ledger.status(pin("ledger.primary", 8), 1, 5)?;
            if scenario == "partition" {
                status.availability = PolicyBudgetAvailability::Unavailable;
                status.identity = status
                    .computed_semantic_hash()
                    .map_err(|_| PolicyBudgetReason::InvalidDescriptor)?;
                validate_policy_budget_status(policy, status, now(2), 1)?;
            } else {
                validate_policy_budget_status(policy, status, now(5), 1)?;
            }
            Ok("accepted")
        }
        "offline-expiry" => {
            let policy = policy(2, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
            validate_offline_lease(policy, lease(policy, 4), now(4))?;
            Ok("accepted")
        }
        "increase-approval" => {
            let old = policy(1, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
            let mut increased = old;
            increased.limits.current_stock = Some(2);
            increased.identity = increased
                .computed_semantic_hash()
                .map_err(|_| PolicyBudgetReason::InvalidDescriptor)?;
            let subject = conduit_core::AdministrativeSubject {
                realm: Id("realm.alpha"),
                entity: Id("entity.target"),
                plan: hash(90),
                epoch: 1,
                artifact: None,
                budget: Some(increased.descriptor),
            };
            validate_policy_budget_replacement(
                old,
                increased,
                None,
                ContainmentContext {
                    subject,
                    time_basis: Id("clock.monotonic"),
                    now_tick: 2,
                },
            )?;
            Ok("accepted")
        }
        "evidence-first" => {
            let policy = policy(2, 1, PolicyBudgetAnchor::Host(Id("host.alpha")));
            let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0)?;
            let request = request(policy, 30, consumer("realm.alpha", 40, 1, 1), 1);
            let (reservation, _) = ledger.reserve(request, now(1), true)?;
            ledger.commit(reservation.identity, 2)?;
            Ok("accepted")
        }
        "stock-lifetime" => {
            let policy = policy(1, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
            let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0)?;
            let first = request(policy, 30, consumer("realm.alpha", 40, 1, 1), 1);
            let (reservation, _) = ledger.reserve(first, now(1), true)?;
            ledger.commit(reservation.identity, 2)?;
            ledger.release(reservation.identity)?;
            let second = request(policy, 31, consumer("realm.alpha", 41, 2, 2), 4);
            ledger.reserve(second, now(4), true)?;
            Ok("accepted")
        }
        "retention-gap" => {
            let policy = policy(2, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
            let mut ledger = PersistentBudgetLedger::<16>::new(policy, hash(20), 0)?;
            let first = request(policy, 30, consumer("realm.alpha", 40, 1, 1), 1);
            let (reservation, _) = ledger.reserve(first, now(1), true)?;
            ledger.release(reservation.identity)?;
            ledger.compact(2)?;
            ledger.reserve(first, now(4), true)?;
            Ok("accepted")
        }
        "coexisting-anchors" => {
            let realm_policy = policy(2, 20, PolicyBudgetAnchor::Realm(Id("realm.alpha")));
            let mut host_policy = policy(1, 20, PolicyBudgetAnchor::Host(Id("host.alpha")));
            host_policy.descriptor = pin("budget.host-enrollment", 9);
            host_policy.identity = host_policy
                .computed_semantic_hash()
                .map_err(|_| PolicyBudgetReason::InvalidDescriptor)?;
            let realm = PersistentBudgetLedger::<16>::new(realm_policy, hash(20), 0)?;
            let host = PersistentBudgetLedger::<16>::new(host_policy, hash(21), 0)?;
            validate_policy_budget_bindings(
                &[realm_policy, host_policy],
                &[
                    realm.status(pin("ledger.realm", 10), 1, 5)?,
                    host.status(pin("ledger.host", 11), 1, 5)?,
                ],
                now(2),
                1,
            )?;
            Ok("accepted")
        }
        _ => panic!("unregistered persistent-budget fixture scenario {scenario}"),
    }
}

#[test]
fn every_persistent_budget_fixture_executes_independently() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/c2/persistent-budget-v1.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let scenario = case["scenario"].as_str().unwrap();
        let expected = case["expected"].as_str().unwrap();
        let actual = match run_fixture(scenario) {
            Ok(value) => value,
            Err(reason) => reason.code(),
        };
        assert_eq!(actual, expected, "fixture scenario {scenario}");
    }
}
