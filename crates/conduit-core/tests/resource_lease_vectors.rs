use conduit_core::{
    AuthorityTime, CancellationDisposition, EffectAttemptPhase, EffectAttemptState,
    EffectCommitProfile, EffectDiscontinuity, EffectIdempotency, ForeignRetention, Id,
    InstancePath, PinnedDescriptor, PlanResourceBudget, ResourceLeaseContract, ResourceLeasePhase,
    ResourceLeaseReason, ResourceLeaseState, ResourceSharingMode, SemanticHash,
    UnknownCommitPolicy, validate_effect_commit_profile, validate_resource_lease,
};

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

fn lease() -> ResourceLeaseContract<'static> {
    ResourceLeaseContract {
        schema_version: 0,
        id: Id("lease/file-output"),
        resource_binding: Id("resource/file-output"),
        holder: InstancePath::new("writer").unwrap(),
        run: Id("run/value-17"),
        epoch: 4,
        scope: Id("scope/write"),
        sharing: ResourceSharingMode::Exclusive,
        reservation: PlanResourceBudget {
            memory_bytes: 4096,
            storage_bytes: 8192,
            cpu_units: 1,
            timers: 1,
            transports: 0,
            checkpoints: 0,
            evidence_bytes: 2048,
        },
        time_basis: Id("clock/monotonic"),
        issued_at_tick: 10,
        expires_at_tick: 100,
        revocation_grace_ticks: 5,
        cleanup_ticks: 10,
        maximum_operations: 2,
        maximum_evidence_events: 8,
        cleanup_escalation: pin("cleanup/force-close", 1),
        foreign_retention: ForeignRetention::Bounded {
            maximum_bytes: 1024,
            release_ticks: 10,
        },
    }
}

fn profile() -> EffectCommitProfile<'static> {
    EffectCommitProfile {
        schema_version: 0,
        id: Id("effect/file-replace"),
        operation: Id("file/replace"),
        resource_lease: Id("lease/file-output"),
        commit_boundary: pin("commit/rename", 2),
        idempotency: EffectIdempotency::SameKeySameEffect,
        unknown_commit: UnknownCommitPolicy::RetrySameIdempotencyKey,
        discontinuity: EffectDiscontinuity::CommitUnknown,
        cleanup: pin("cleanup/unlink-temp", 3),
        maximum_attempts: 2,
        evidence_events_per_attempt: 4,
    }
}

fn now(tick: u64) -> AuthorityTime<'static> {
    AuthorityTime {
        basis: Id("clock/monotonic"),
        tick,
    }
}

#[test]
fn exact_lease_and_domain_commit_profile_are_canonical_and_valid() {
    let lease = lease();
    let profile = profile();
    validate_resource_lease(lease).unwrap();
    validate_effect_commit_profile(profile, lease).unwrap();
    assert_ne!(
        lease.semantic_hash().unwrap(),
        profile.semantic_hash().unwrap()
    );

    let mut invalid = profile;
    invalid.unknown_commit = UnknownCommitPolicy::RetrySameIdempotencyKey;
    invalid.idempotency = EffectIdempotency::None;
    assert_eq!(
        validate_effect_commit_profile(invalid, lease),
        Err(ResourceLeaseReason::InvalidContract)
    );
}

#[test]
fn use_checks_holder_run_epoch_resource_and_expiry() {
    let state = ResourceLeaseState::new(lease()).unwrap();
    assert_eq!(
        state.check_use(
            Id("resource/file-output"),
            InstancePath::new("writer").unwrap(),
            Id("run/value-17"),
            4,
            now(20),
        ),
        Ok(())
    );
    assert_eq!(
        state.check_use(
            Id("resource/file-output"),
            InstancePath::new("other").unwrap(),
            Id("run/value-17"),
            4,
            now(20),
        ),
        Err(ResourceLeaseReason::WrongHolder)
    );
    assert_eq!(
        state.check_use(
            Id("resource/file-output"),
            InstancePath::new("writer").unwrap(),
            Id("run/value-18"),
            4,
            now(20),
        ),
        Err(ResourceLeaseReason::WrongRun)
    );
    assert_eq!(
        state.check_use(
            Id("resource/file-output"),
            InstancePath::new("writer").unwrap(),
            Id("run/value-17"),
            5,
            now(20),
        ),
        Err(ResourceLeaseReason::WrongEpoch)
    );
    assert_eq!(
        state.check_use(
            Id("resource/other"),
            InstancePath::new("writer").unwrap(),
            Id("run/value-17"),
            4,
            now(20),
        ),
        Err(ResourceLeaseReason::WrongResource)
    );
    assert_eq!(
        state.check_use(
            Id("resource/file-output"),
            InstancePath::new("writer").unwrap(),
            Id("run/value-17"),
            4,
            now(100),
        ),
        Err(ResourceLeaseReason::Expired)
    );
}

#[test]
fn operation_and_evidence_admission_fail_before_mutation() {
    let mut state = ResourceLeaseState::new(lease()).unwrap();
    for expected in 1..=2 {
        assert_eq!(
            state
                .begin_operation(
                    Id("resource/file-output"),
                    InstancePath::new("writer").unwrap(),
                    Id("run/value-17"),
                    4,
                    now(20),
                )
                .unwrap(),
            expected
        );
    }
    assert_eq!(
        state.begin_operation(
            Id("resource/file-output"),
            InstancePath::new("writer").unwrap(),
            Id("run/value-17"),
            4,
            now(20),
        ),
        Err(ResourceLeaseReason::OperationLimit)
    );
    for _ in 0..8 {
        state.record_required_evidence().unwrap();
    }
    assert_eq!(
        state.record_required_evidence(),
        Err(ResourceLeaseReason::EvidenceExhausted)
    );
}

#[test]
fn revocation_expiry_and_cleanup_have_finite_disposition() {
    let mut state = ResourceLeaseState::new(lease()).unwrap();
    assert_eq!(state.revoke(now(30)).unwrap().tick, 35);
    assert_eq!(state.phase(), ResourceLeasePhase::Revoked);
    assert_eq!(
        state.check_use(
            Id("resource/file-output"),
            InstancePath::new("writer").unwrap(),
            Id("run/value-17"),
            4,
            now(31),
        ),
        Err(ResourceLeaseReason::Revoked)
    );
    assert_eq!(state.begin_cleanup(now(31)).unwrap().tick, 41);
    assert_eq!(state.complete_cleanup(1), Ok(()));
    assert_eq!(state.phase(), ResourceLeasePhase::Released);

    let mut expired = ResourceLeaseState::new(lease()).unwrap();
    expired.expire(now(100)).unwrap();
    expired.begin_cleanup(now(100)).unwrap();
    assert_eq!(
        expired.enforce_cleanup_deadline(now(110)),
        Err(ResourceLeaseReason::CleanupTimeout)
    );
    assert_eq!(expired.phase(), ResourceLeasePhase::Failed);
}

#[test]
fn stale_release_never_frees_a_newer_lease_generation() {
    let mut state = ResourceLeaseState::new(lease()).unwrap();
    state.begin_cleanup(now(30)).unwrap();
    assert_eq!(state.complete_cleanup(1), Ok(()));
    assert_eq!(
        state.complete_cleanup(1),
        Err(ResourceLeaseReason::StaleRelease)
    );
    assert_eq!(
        state.complete_cleanup(2),
        Err(ResourceLeaseReason::CleanupRequired)
    );
}

#[test]
fn lost_acknowledgement_preserves_unknown_commit_and_retry_identity() {
    let lease = lease();
    let mut attempt =
        EffectAttemptState::new(profile(), lease, 1, Some(Id("idempotency/write-17"))).unwrap();
    attempt.start().unwrap();
    attempt.committed().unwrap();
    assert_eq!(attempt.lose_host(), Err(ResourceLeaseReason::CommitUnknown));
    assert_eq!(attempt.phase(), EffectAttemptPhase::CommitUnknown);
    assert!(!attempt.may_report_success());
    assert_eq!(
        attempt.retry(Some(Id("idempotency/other"))),
        Err(ResourceLeaseReason::RetryForbidden)
    );
    assert_eq!(attempt.retry(Some(Id("idempotency/write-17"))), Ok(2));
}

#[test]
fn cancellation_waits_for_commit_and_cleanup_disposition() {
    let lease = lease();
    let mut before =
        EffectAttemptState::new(profile(), lease, 1, Some(Id("idempotency/before"))).unwrap();
    assert_eq!(
        before.cancel(),
        Ok(CancellationDisposition::CancelledBeforeCommit)
    );

    let mut running =
        EffectAttemptState::new(profile(), lease, 1, Some(Id("idempotency/running"))).unwrap();
    running.start().unwrap();
    assert_eq!(
        running.cancel(),
        Ok(CancellationDisposition::CleanupRequired)
    );
    assert_eq!(running.phase(), EffectAttemptPhase::Cleaning);
    assert!(!running.may_report_success());
    running.cleanup_complete().unwrap();
    assert_eq!(running.phase(), EffectAttemptPhase::Cancelled);
}

#[test]
fn success_requires_commit_acknowledgement() {
    let lease = lease();
    let mut attempt =
        EffectAttemptState::new(profile(), lease, 1, Some(Id("idempotency/success"))).unwrap();
    attempt.start().unwrap();
    attempt.committed().unwrap();
    assert!(!attempt.may_report_success());
    attempt.acknowledge().unwrap();
    assert!(attempt.may_report_success());
}

#[test]
fn foreign_retention_must_be_bounded_or_truthfully_classified() {
    let mut invalid = lease();
    invalid.foreign_retention = ForeignRetention::Bounded {
        maximum_bytes: 1,
        release_ticks: invalid.cleanup_ticks + 1,
    };
    assert_eq!(
        validate_resource_lease(invalid),
        Err(ResourceLeaseReason::InvalidContract)
    );

    let mut observed = lease();
    observed.foreign_retention = ForeignRetention::ObservedOnly;
    validate_resource_lease(observed).unwrap();
    let mut unsupported = lease();
    unsupported.foreign_retention = ForeignRetention::Unsupported;
    validate_resource_lease(unsupported).unwrap();
}

#[test]
fn retry_after_unknown_commit_is_never_global_exactly_once() {
    let lease = lease();
    for (idempotency, policy, expected) in [
        (
            EffectIdempotency::None,
            UnknownCommitPolicy::Fail,
            Err(ResourceLeaseReason::RetryForbidden),
        ),
        (
            EffectIdempotency::ReconcileBeforeRetry,
            UnknownCommitPolicy::Reconcile,
            Err(ResourceLeaseReason::RetryForbidden),
        ),
    ] {
        let mut profile = profile();
        profile.idempotency = idempotency;
        profile.unknown_commit = policy;
        let mut attempt = EffectAttemptState::new(profile, lease, 1, None).unwrap();
        attempt.start().unwrap();
        assert_eq!(attempt.lose_host(), Err(ResourceLeaseReason::CommitUnknown));
        assert_eq!(attempt.retry(None), expected);
    }
}
