use conduit_core::{
    AppendCommit, AppendRecovery, ArtifactDigest, BlockingFairness, CancellationCheckpointPolicy,
    CheckpointCommit, CheckpointEnvelope, CheckpointError, CheckpointMigration,
    CheckpointProviderCapabilities, CheckpointRecovery, CheckpointStateKind, CheckpointStateRef,
    CheckpointStatus, DeliveryClaim, DuplicatePolicy, DurableCommit, EventClass, EventPayloadRef,
    EventProviderCapabilities, EventStreamContract, FlowCapacity, FlowPolicy, FlowWatermarks,
    JobAttemptMachine, JobContract, JobError, JobEvidenceKind, JobEvidenceRecord, JobIdentity,
    JobPhase, PinnedDescriptor, Pressure, RecoveryDecision, ReplayDelivery, ResonanceEnvelope,
    ResonanceRelations, RestartPolicy, ResultValidationDecision, ResultValidationPolicy,
    ResumeTarget, RetentionPolicy, SemanticHash, Sensitivity, StopPolicy, SubscriberCoupling,
    TypeContractRef, ValidationOutcome, WorkLease, validate_checkpoint_resume,
    validate_job_contract, validate_job_evidence_envelope, validate_result_decision,
};
use conduit_core::{Id, InstancePath};

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

fn contract() -> JobContract<'static> {
    JobContract {
        id: Id("job-contract/reference"),
        total_work_units: 10,
        maximum_attempts: 3,
        retry_backoff_ticks: 2,
        attempt_deadline_ticks: 50,
        maximum_checkpoints: 2,
        maximum_checkpoint_bytes: 128,
        maximum_checkpoint_state_refs: 4,
        maximum_checkpoint_operations: 2,
        lease_basis: Id("clock/lease"),
        maximum_lease_renewals: 2,
        delivery: DeliveryClaim::AtLeastOnce,
        duplicate_policy: DuplicatePolicy::ReturnCommitted,
        commit_boundary: pin("boundary/result-store", 8),
        transactional_boundary: None,
        checkpoint_provider: Some(pin("provider/checkpoints", 9)),
        evidence_stream: Id("stream/job-progress"),
        restart: RestartPolicy::ResumeRequired,
        cancellation_checkpoint: CancellationCheckpointPolicy::FinalCheckpoint { maximum_ticks: 4 },
        result_validation: Some(validation_policy()),
    }
}

fn identity(attempt: &'static str) -> JobIdentity<'static> {
    JobIdentity {
        job: Id("job/a"),
        run: Id("run/a"),
        attempt: Id(attempt),
        attempt_ordinal: if attempt == "attempt/a" { 1 } else { 2 },
        work_unit: Id("work/a"),
        idempotency: Id("idempotency/a"),
    }
}

fn lease(expires_at_tick: u64) -> WorkLease<'static> {
    WorkLease {
        id: Id("lease/a"),
        job: Id("job/a"),
        run: Id("run/a"),
        attempt: Id("attempt/a"),
        work_unit: Id("work/a"),
        time_basis: Id("clock/lease"),
        issued_at_tick: 0,
        expires_at_tick,
        renewal: 0,
    }
}

fn validation_policy() -> ResultValidationPolicy<'static> {
    ResultValidationPolicy {
        validator: pin("validator/domain", 50),
        equivalence: pin("equivalence/tolerant", 51),
        homogeneous_constraint: Some(pin("constraint/numerical-class", 52)),
        maximum_results: 3,
        quorum: 2,
        deadline_ticks: 80,
    }
}

fn flow() -> FlowPolicy<'static> {
    let capacity = FlowCapacity::new(2, 64, 128).unwrap();
    FlowPolicy::new(
        capacity,
        Pressure::Block(BlockingFairness::Fifo),
        FlowWatermarks::new(0, 2, capacity).unwrap(),
    )
    .unwrap()
}

fn evidence_stream() -> (EventStreamContract<'static>, EventProviderCapabilities) {
    (
        EventStreamContract {
            id: Id("stream/job-progress"),
            event_class: EventClass::NormativeEvidence,
            payload_type: TypeContractRef {
                contract_id: Id("conduit/job-evidence"),
                schema_version: 0,
                semantic_hash: hash(53),
            },
            retention: RetentionPolicy::DurableAppend {
                maximum_events: 64,
                maximum_bytes: 4096,
                flush_ticks: 2,
            },
            subscriber_coupling: SubscriberCoupling::Isolated(flow()),
            delivery: ReplayDelivery::AtLeastOnce,
            maximum_publishers: 1,
            maximum_subscribers: 2,
            maximum_pending_operations: 2,
            maximum_projection_bytes: 128,
            provider: pin("provider/evidence", 54),
            recording_authority: None,
            sensitivity: Sensitivity::Public,
            terminal_evidence_required: true,
        },
        EventProviderCapabilities {
            ephemeral: false,
            retained: true,
            durable: true,
            checkpoint_cursor: true,
            integrity: true,
            redaction: true,
            maximum_events: 64,
            maximum_bytes: 4096,
            maximum_subscribers: 2,
            maximum_pending_operations: 2,
        },
    )
}

fn checkpoint_capabilities() -> CheckpointProviderCapabilities {
    CheckpointProviderCapabilities {
        durable: true,
        integrity: true,
        migration: true,
        maximum_checkpoints: 2,
        maximum_checkpoint_bytes: 128,
        maximum_state_references: 4,
        maximum_pending_operations: 2,
    }
}

fn states() -> [CheckpointStateRef<'static>; 2] {
    [
        CheckpointStateRef {
            id: Id("state/source-offset"),
            owner: InstancePath::new("root/source").unwrap(),
            kind: CheckpointStateKind::SourceOffset,
            state_contract: pin("checkpoint/source-offset", 10),
            content_digest: ArtifactDigest::from_bytes([11; 32]),
            bytes: 8,
        },
        CheckpointStateRef {
            id: Id("state/queued-values"),
            owner: InstancePath::new("root/cord.values").unwrap(),
            kind: CheckpointStateKind::Cord,
            state_contract: pin("checkpoint/cord-queue", 12),
            content_digest: ArtifactDigest::from_bytes([13; 32]),
            bytes: 32,
        },
    ]
}

fn envelope<'a>(state: &'a [CheckpointStateRef<'a>]) -> CheckpointEnvelope<'a> {
    let mut value = CheckpointEnvelope {
        schema_version: 0,
        checkpoint: Id("checkpoint/a"),
        status: CheckpointStatus::Complete,
        job: Id("job/a"),
        run: Id("run/a"),
        work_unit: Id("work/a"),
        source_attempt: Id("attempt/a"),
        source_lease: Id("lease/a"),
        sequence: 4,
        provider: pin("provider/checkpoints", 9),
        evidence_stream: Id("stream/job-progress"),
        stream_epoch: hash(14),
        event_cursor: 17,
        plan_identity: hash(1),
        implementation_hash: hash(2),
        artifact_hash: hash(3),
        configuration_hash: hash(4),
        type_contracts_hash: hash(5),
        template_hash: hash(6),
        correlation_hash: hash(7),
        migration_version: 1,
        state,
        integrity: ZERO,
    };
    let mut scratch = [ZERO; 2];
    value.integrity = value
        .computed_integrity(&mut scratch)
        .expect("fixture integrity");
    value
}

fn target() -> ResumeTarget<'static> {
    ResumeTarget {
        checkpoint: Id("checkpoint/a"),
        job: Id("job/a"),
        run: Id("run/a"),
        work_unit: Id("work/a"),
        source_lease: Id("lease/a"),
        new_attempt: Id("attempt/b"),
        new_lease: Id("lease/b"),
        evidence_stream: Id("stream/job-progress"),
        stream_epoch: hash(14),
        event_cursor: 17,
        checkpoint_provider: pin("provider/checkpoints", 9),
        plan_identity: hash(1),
        implementation_hash: hash(2),
        artifact_hash: hash(3),
        configuration_hash: hash(4),
        type_contracts_hash: hash(5),
        template_hash: hash(6),
        correlation_hash: hash(7),
        maximum_checkpoint_bytes: 128,
        maximum_state_references: 4,
        migration_supported: true,
    }
}

#[test]
fn checkpoint_resume_restores_exact_source_and_queue_state_only() {
    let state = states();
    assert_ne!(
        state[0].semantic_hash().unwrap(),
        state[1].semantic_hash().unwrap()
    );
    let checkpoint = envelope(&state);
    assert_eq!(
        validate_checkpoint_resume(&checkpoint, target(), None, &mut [ZERO; 2]),
        Ok(())
    );
    assert_eq!(checkpoint.state[0].kind, CheckpointStateKind::SourceOffset);
    assert_eq!(checkpoint.state[1].kind, CheckpointStateKind::Cord);

    for incompatible in [
        ResumeTarget {
            plan_identity: hash(20),
            ..target()
        },
        ResumeTarget {
            artifact_hash: hash(21),
            ..target()
        },
        ResumeTarget {
            configuration_hash: hash(22),
            ..target()
        },
    ] {
        assert_eq!(
            validate_checkpoint_resume(&checkpoint, incompatible, None, &mut [ZERO; 2]),
            Err(CheckpointError::Incompatible)
        );
    }

    let corrupted = CheckpointEnvelope {
        integrity: hash(99),
        ..checkpoint
    };
    assert_eq!(
        validate_checkpoint_resume(&corrupted, target(), None, &mut [ZERO; 2]),
        Err(CheckpointError::IntegrityMismatch)
    );
    let partial = CheckpointEnvelope {
        status: CheckpointStatus::Partial,
        ..checkpoint
    };
    assert_eq!(
        validate_checkpoint_resume(&partial, target(), None, &mut [ZERO; 2]),
        Err(CheckpointError::InvalidEnvelope)
    );
    assert_eq!(
        validate_checkpoint_resume(
            &checkpoint,
            ResumeTarget {
                event_cursor: 18,
                ..target()
            },
            None,
            &mut [ZERO; 2]
        ),
        Err(CheckpointError::InvalidEnvelope)
    );

    let duplicate_state = [state[0], state[0]];
    let duplicate = CheckpointEnvelope {
        state: &duplicate_state,
        ..checkpoint
    };
    assert_eq!(
        validate_checkpoint_resume(&duplicate, target(), None, &mut [ZERO; 2]),
        Err(CheckpointError::InvalidEnvelope)
    );
}

#[test]
fn explicit_migration_names_both_compatibility_boundaries() {
    let state = states();
    let checkpoint = envelope(&state);
    let changed = ResumeTarget {
        implementation_hash: hash(30),
        ..target()
    };
    let migration = CheckpointMigration {
        source_compatibility: checkpoint.compatibility_hash().unwrap(),
        target_compatibility: changed.compatibility_hash().unwrap(),
        from_version: 1,
        to_version: 2,
        contract: pin("migration/checkpoint-v1-v2", 31),
    };
    assert_eq!(
        validate_checkpoint_resume(&checkpoint, changed, Some(migration), &mut [ZERO; 2]),
        Ok(())
    );
    assert_eq!(
        validate_checkpoint_resume(
            &checkpoint,
            changed,
            Some(CheckpointMigration {
                target_compatibility: hash(90),
                ..migration
            }),
            &mut [ZERO; 2]
        ),
        Err(CheckpointError::MigrationInvalid)
    );
}

#[test]
fn event_append_and_checkpoint_commits_recover_independently() {
    assert_eq!(
        AppendCommit::prepare().recover(),
        AppendRecovery::DiscardPartial
    );
    let mut appended = AppendCommit::prepare();
    appended.commit();
    assert_eq!(appended.recover(), AppendRecovery::ReplayCommitted);

    assert_eq!(
        CheckpointCommit::prepare().recover(),
        CheckpointRecovery::DiscardPartial
    );
    let mut checkpoint = CheckpointCommit::prepare();
    checkpoint.commit();
    assert_eq!(checkpoint.recover(), CheckpointRecovery::SelectCommitted);
}

#[test]
fn crash_before_and_after_commit_never_fabricate_completion() {
    let mut before =
        JobAttemptMachine::new(contract(), identity("attempt/a"), lease(100)).expect("valid job");
    before.start(1).unwrap();
    before.record_progress(9, 2).unwrap();
    assert_eq!(
        before.record_progress(1, 3),
        Err(JobError::ProgressOutOfBounds)
    );
    before.begin_commit(3).unwrap();
    assert_eq!(before.complete(), Err(JobError::CompletionNotCommitted));
    assert_eq!(
        before.recover_after_crash(4),
        RecoveryDecision::Retry {
            next_attempt_ordinal: 2,
            not_before_tick: 6
        }
    );

    let mut after =
        JobAttemptMachine::new(contract(), identity("attempt/a"), lease(100)).expect("valid job");
    after.start(1).unwrap();
    after.begin_commit(2).unwrap();
    after
        .record_durable_commit(
            DurableCommit {
                idempotency: Id("idempotency/a"),
                result: Id("result/attempt-a"),
                result_digest: ArtifactDigest::from_bytes([40; 32]),
                boundary: pin("boundary/result-store", 8),
                commit_evidence: Id("event/commit"),
                acknowledgement_evidence: Some(Id("event/ack")),
            },
            3,
        )
        .unwrap();
    assert_eq!(
        after.recover_after_crash(4),
        RecoveryDecision::ReturnCommitted
    );
    after.complete().unwrap();
    assert_eq!(after.phase(), JobPhase::Executed);
    assert_eq!(after.progress_units(), 10);

    let mut lost_ack =
        JobAttemptMachine::new(contract(), identity("attempt/a"), lease(100)).unwrap();
    lost_ack.start(1).unwrap();
    lost_ack.begin_commit(2).unwrap();
    lost_ack
        .record_durable_commit(
            DurableCommit {
                idempotency: Id("idempotency/a"),
                result: Id("result/attempt-a"),
                result_digest: ArtifactDigest::from_bytes([42; 32]),
                boundary: pin("boundary/result-store", 8),
                commit_evidence: Id("event/commit-with-lost-ack"),
                acknowledgement_evidence: None,
            },
            3,
        )
        .unwrap();
    assert_eq!(
        lost_ack.recover_after_crash(4),
        RecoveryDecision::ReturnCommitted
    );

    let final_identity = JobIdentity {
        attempt_ordinal: 3,
        ..identity("attempt/a")
    };
    let exhausted = JobAttemptMachine::new(contract(), final_identity, lease(100)).unwrap();
    assert_eq!(
        exhausted.recover_after_crash(4),
        RecoveryDecision::AttemptsExhausted
    );
}

#[test]
fn cancellation_checkpoint_and_expired_lease_are_bounded() {
    let mut job = JobAttemptMachine::new(contract(), identity("attempt/a"), lease(10)).unwrap();
    job.start(1).unwrap();
    job.cancel(StopPolicy::Drain, 2).unwrap();
    assert_eq!(job.phase(), JobPhase::Checkpointing);
    assert_eq!(job.poll_cancellation(5), Err(JobError::CancellationPending));
    job.poll_cancellation(6).unwrap();
    assert_eq!(job.phase(), JobPhase::Cancelled);

    let mut completed =
        JobAttemptMachine::new(contract(), identity("attempt/a"), lease(10)).unwrap();
    completed.start(1).unwrap();
    completed.cancel(StopPolicy::Drain, 2).unwrap();
    completed.finish_checkpoint(true, 3).unwrap();
    assert_eq!(completed.phase(), JobPhase::Cancelled);

    let mut failed_checkpoint =
        JobAttemptMachine::new(contract(), identity("attempt/a"), lease(10)).unwrap();
    failed_checkpoint.start(1).unwrap();
    failed_checkpoint.begin_checkpoint(2).unwrap();
    failed_checkpoint.finish_checkpoint(false, 3).unwrap();
    assert_eq!(failed_checkpoint.phase(), JobPhase::Failed);

    let mut expired = JobAttemptMachine::new(contract(), identity("attempt/a"), lease(10)).unwrap();
    assert_eq!(expired.start(10), Err(JobError::LeaseExpired));

    let mut renewed = JobAttemptMachine::new(contract(), identity("attempt/a"), lease(10)).unwrap();
    renewed
        .renew_lease(
            WorkLease {
                issued_at_tick: 5,
                expires_at_tick: 20,
                renewal: 1,
                ..lease(10)
            },
            5,
        )
        .unwrap();
    renewed.start(6).unwrap();

    let exactly_once = JobContract {
        delivery: DeliveryClaim::TransactionalExactlyOnce,
        transactional_boundary: Some(pin("boundary/result-transaction", 43)),
        ..contract()
    };
    let mut exact = JobAttemptMachine::new(exactly_once, identity("attempt/a"), lease(10)).unwrap();
    exact.start(1).unwrap();
    exact.begin_commit(2).unwrap();
    assert_eq!(
        exact.record_durable_commit(
            DurableCommit {
                idempotency: Id("idempotency/a"),
                result: Id("result/attempt-a"),
                result_digest: ArtifactDigest::from_bytes([41; 32]),
                boundary: pin("boundary/result-store", 8),
                commit_evidence: Id("event/commit"),
                acknowledgement_evidence: None,
            },
            3
        ),
        Err(JobError::CommitMismatch)
    );

    let unscoped_exactly_once = JobContract {
        delivery: DeliveryClaim::TransactionalExactlyOnce,
        ..contract()
    };
    assert_eq!(
        JobAttemptMachine::new(unscoped_exactly_once, identity("attempt/a"), lease(10)),
        Err(JobError::InvalidContract)
    );
}

#[test]
fn providers_and_non_checkpointable_restart_are_explicit() {
    let (stream, provider) = evidence_stream();
    assert_eq!(
        validate_job_contract(
            contract(),
            Some(checkpoint_capabilities()),
            stream,
            provider
        ),
        Ok(())
    );
    assert_eq!(
        validate_job_contract(
            contract(),
            Some(CheckpointProviderCapabilities {
                durable: false,
                ..checkpoint_capabilities()
            }),
            stream,
            provider
        ),
        Err(JobError::ProviderIncapable)
    );

    let restart_only = JobContract {
        id: Id("job-contract/restart-only"),
        maximum_checkpoints: 0,
        maximum_checkpoint_bytes: 0,
        maximum_checkpoint_state_refs: 0,
        maximum_checkpoint_operations: 0,
        checkpoint_provider: None,
        restart: RestartPolicy::RestartFromBeginning {
            maximum_lost_work_units: 10,
        },
        cancellation_checkpoint: CancellationCheckpointPolicy::None,
        ..contract()
    };
    assert_eq!(
        validate_job_contract(restart_only, None, stream, provider),
        Ok(())
    );
}

#[test]
fn execution_validation_and_accepted_result_are_distinct() {
    let policy = validation_policy();
    let accepted = ResultValidationDecision {
        id: Id("validation/a"),
        work_unit: Id("work/a"),
        output: Id("result/attempt-a"),
        output_digest: ArtifactDigest::from_bytes([60; 32]),
        validator: policy.validator,
        equivalence: policy.equivalence,
        homogeneous_constraint: policy.homogeneous_constraint,
        compared_attempts: &[Id("attempt/a"), Id("attempt/b")],
        decided_at_tick: 70,
        outcome: ValidationOutcome::Accepted,
        canonical_result: Some(Id("result/canonical-a")),
    };
    assert_eq!(validate_result_decision(policy, accepted), Ok(()));
    assert_eq!(
        validate_result_decision(
            policy,
            ResultValidationDecision {
                compared_attempts: &[Id("attempt/a")],
                ..accepted
            }
        ),
        Err(JobError::ValidationInvalid)
    );
    assert_eq!(
        validate_result_decision(
            policy,
            ResultValidationDecision {
                homogeneous_constraint: None,
                ..accepted
            }
        ),
        Err(JobError::ValidationInvalid)
    );
    assert_eq!(
        validate_result_decision(
            policy,
            ResultValidationDecision {
                decided_at_tick: 81,
                outcome: ValidationOutcome::Late,
                canonical_result: None,
                ..accepted
            }
        ),
        Ok(())
    );
}

#[test]
fn immutable_job_records_bind_to_resonance_without_becoming_projections() {
    let identity = identity("attempt/a");
    let record = JobEvidenceRecord {
        event: Id("event/progress"),
        job: identity.job,
        run: identity.run,
        attempt: identity.attempt,
        work_unit: identity.work_unit,
        sequence: 3,
        progress_units: 4,
        kind: JobEvidenceKind::Progress,
    };
    let envelope = ResonanceEnvelope {
        event: record.event,
        stream: Id("stream/job-progress"),
        run: identity.run,
        plan_epoch: hash(1),
        producer: InstancePath::new("root/worker").unwrap(),
        subject: InstancePath::new("root/worker").unwrap(),
        class: EventClass::NormativeEvidence,
        sequence: record.sequence,
        observer: Id("host/a"),
        observer_sequence: record.sequence,
        domain_time: None,
        correlation: Some(Id("correlation/a")),
        idempotency: Some(identity.idempotency),
        payload_type: evidence_stream().0.payload_type,
        payload: EventPayloadRef::ContentAddressed {
            digest: ArtifactDigest::from_bytes([61; 32]),
            bytes: 64,
        },
        relations: ResonanceRelations {
            caused_by: None,
            derived_from: &[],
            supersedes: None,
            corrects: None,
            retracts: None,
        },
        provenance: Id("provider/job"),
        recording_authority: None,
        sensitivity: Sensitivity::Public,
        integrity: hash(62),
    };
    assert_eq!(
        validate_job_evidence_envelope(contract(), identity, record, &envelope),
        Ok(())
    );
    assert_eq!(
        validate_job_evidence_envelope(
            contract(),
            identity,
            JobEvidenceRecord {
                progress_units: 10,
                ..record
            },
            &envelope
        ),
        Err(JobError::EvidenceInvalid)
    );
}

#[test]
fn normative_job_fixture_inventory_is_owned_here() {
    let fixture = include_str!("../../../conformance/c4/durable-job.json");
    for id in [
        "distinct-durable-identities",
        "distinct-work-lease",
        "at-most-once-boundary",
        "at-least-once-boundary",
        "exactly-once-without-transaction-rejected",
        "transactional-claim-requires-acknowledgement",
        "crash-before-commit",
        "crash-after-commit",
        "crash-before-event-append",
        "crash-after-event-append",
        "crash-before-checkpoint-commit",
        "crash-after-checkpoint-commit",
        "lost-acknowledgement-after-commit",
        "duplicate-retry",
        "finite-retry-backoff-deadline",
        "checkpoint-integrity-failure",
        "duplicate-checkpoint-state-rejected",
        "partial-checkpoint-rejected",
        "incompatible-plan",
        "incompatible-artifact",
        "incompatible-config",
        "incompatible-type-contract",
        "incompatible-stream-epoch",
        "incompatible-event-cursor",
        "explicit-migration",
        "source-offset-restoration",
        "queued-value-restoration",
        "cancellation-during-checkpoint",
        "cancellation-checkpoint-deadline",
        "expired-work-lease",
        "terminal-evidence-replay",
        "progress-resonance-binding",
        "progress-never-reaches-total-before-commit",
        "checkpoint-state-replacement-boundary",
        "executor-success-not-domain-acceptance",
        "domain-validation-quorum",
        "late-validation-remains-evidence",
        "non-checkpointable-restart-declared",
        "incapable-checkpoint-provider-rejected",
        "plan-job-identity",
    ] {
        assert!(fixture.contains(&format!("\"id\":\"{id}\"")), "{id}");
    }
}
