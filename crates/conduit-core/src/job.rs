//! Durable finite-job, checkpoint, and resume contracts.

use core::convert::Infallible;
use core::fmt;

use crate::canonical::semantic_hash_with_hash_set;
use crate::{
    ArtifactDigest, CanonicalDescriptor, CanonicalError, CanonicalValue, EventClass,
    EventProviderCapabilities, EventStreamContract, FieldDisposition, Id, InstancePath, MapField,
    PinnedDescriptor, ReplayDelivery, ResonanceEnvelope, RetentionPolicy, SemanticHash, StopPolicy,
    validate_envelope, validate_stream_contract,
};

pub const CHECKPOINT_SCHEMA_VERSION: u32 = 1;
pub const JOB_CONTRACT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryClaim {
    AtMostOnce,
    AtLeastOnce,
    TransactionalExactlyOnce,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DuplicatePolicy {
    Reject,
    ReturnCommitted,
    RetryWithSameKey,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationCheckpointPolicy {
    None,
    FinalCheckpoint { maximum_ticks: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestartPolicy {
    ResumeRequired,
    RestartFromBeginning { maximum_lost_work_units: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResultValidationPolicy<'a> {
    pub validator: PinnedDescriptor<'a>,
    pub equivalence: PinnedDescriptor<'a>,
    pub homogeneous_constraint: Option<PinnedDescriptor<'a>>,
    pub maximum_results: u16,
    pub quorum: u16,
    pub deadline_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobContract<'a> {
    pub id: Id<'a>,
    pub total_work_units: u64,
    pub maximum_attempts: u16,
    pub retry_backoff_ticks: u64,
    pub attempt_deadline_ticks: u64,
    pub maximum_checkpoints: u16,
    pub maximum_checkpoint_bytes: u64,
    pub maximum_checkpoint_state_refs: u16,
    pub maximum_checkpoint_operations: u16,
    pub lease_basis: Id<'a>,
    pub maximum_lease_renewals: u16,
    pub delivery: DeliveryClaim,
    pub duplicate_policy: DuplicatePolicy,
    pub commit_boundary: PinnedDescriptor<'a>,
    pub transactional_boundary: Option<PinnedDescriptor<'a>>,
    pub checkpoint_provider: Option<PinnedDescriptor<'a>>,
    pub evidence_stream: Id<'a>,
    pub restart: RestartPolicy,
    pub cancellation_checkpoint: CancellationCheckpointPolicy,
    pub result_validation: Option<ResultValidationPolicy<'a>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobIdentity<'a> {
    pub job: Id<'a>,
    pub run: Id<'a>,
    pub attempt: Id<'a>,
    pub attempt_ordinal: u16,
    pub work_unit: Id<'a>,
    pub idempotency: Id<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkLease<'a> {
    pub id: Id<'a>,
    pub job: Id<'a>,
    pub run: Id<'a>,
    pub attempt: Id<'a>,
    pub work_unit: Id<'a>,
    pub time_basis: Id<'a>,
    pub issued_at_tick: u64,
    pub expires_at_tick: u64,
    pub renewal: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckpointProviderCapabilities {
    pub durable: bool,
    pub integrity: bool,
    pub migration: bool,
    pub maximum_checkpoints: u16,
    pub maximum_checkpoint_bytes: u64,
    pub maximum_state_references: u16,
    pub maximum_pending_operations: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckpointStateKind {
    Node,
    Cord,
    SourceOffset,
    CommittedResult,
}

impl CheckpointStateKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Cord => "cord",
            Self::SourceOffset => "source-offset",
            Self::CommittedResult => "committed-result",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckpointStateRef<'a> {
    pub id: Id<'a>,
    pub owner: InstancePath<'a>,
    pub kind: CheckpointStateKind,
    pub state_contract: PinnedDescriptor<'a>,
    pub content_digest: ArtifactDigest,
    pub bytes: u64,
}

impl CheckpointStateRef<'_> {
    pub fn semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        hash_state_ref(*self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckpointStatus {
    Complete,
    Partial,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckpointRecovery {
    DiscardPartial,
    SelectCommitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckpointCommit {
    prepared: bool,
    committed: bool,
}

impl CheckpointCommit {
    pub const fn prepare() -> Self {
        Self {
            prepared: true,
            committed: false,
        }
    }

    pub fn commit(&mut self) {
        self.committed = self.prepared;
    }

    pub const fn recover(self) -> CheckpointRecovery {
        if self.committed {
            CheckpointRecovery::SelectCommitted
        } else {
            CheckpointRecovery::DiscardPartial
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckpointEnvelope<'a> {
    pub schema_version: u32,
    pub checkpoint: Id<'a>,
    pub status: CheckpointStatus,
    pub job: Id<'a>,
    pub run: Id<'a>,
    pub work_unit: Id<'a>,
    pub source_attempt: Id<'a>,
    pub source_lease: Id<'a>,
    pub sequence: u64,
    pub provider: PinnedDescriptor<'a>,
    pub evidence_stream: Id<'a>,
    pub stream_epoch: SemanticHash,
    pub event_cursor: u64,
    pub plan_identity: SemanticHash,
    pub implementation_hash: SemanticHash,
    pub artifact_hash: SemanticHash,
    pub configuration_hash: SemanticHash,
    pub type_contracts_hash: SemanticHash,
    pub template_hash: SemanticHash,
    pub correlation_hash: SemanticHash,
    pub migration_version: u32,
    pub state: &'a [CheckpointStateRef<'a>],
    pub integrity: SemanticHash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResumeTarget<'a> {
    pub checkpoint: Id<'a>,
    pub job: Id<'a>,
    pub run: Id<'a>,
    pub work_unit: Id<'a>,
    pub source_lease: Id<'a>,
    pub new_attempt: Id<'a>,
    pub new_lease: Id<'a>,
    pub evidence_stream: Id<'a>,
    pub stream_epoch: SemanticHash,
    pub event_cursor: u64,
    pub checkpoint_provider: PinnedDescriptor<'a>,
    pub plan_identity: SemanticHash,
    pub implementation_hash: SemanticHash,
    pub artifact_hash: SemanticHash,
    pub configuration_hash: SemanticHash,
    pub type_contracts_hash: SemanticHash,
    pub template_hash: SemanticHash,
    pub correlation_hash: SemanticHash,
    pub maximum_checkpoint_bytes: u64,
    pub maximum_state_references: u16,
    pub migration_supported: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckpointMigration<'a> {
    pub source_compatibility: SemanticHash,
    pub target_compatibility: SemanticHash,
    pub from_version: u32,
    pub to_version: u32,
    pub contract: PinnedDescriptor<'a>,
}

impl CheckpointEnvelope<'_> {
    pub fn compatibility_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        compatibility_hash(
            self.plan_identity,
            self.implementation_hash,
            self.artifact_hash,
            self.configuration_hash,
            self.type_contracts_hash,
            self.template_hash,
            self.correlation_hash,
        )
    }

    pub fn computed_integrity(
        &self,
        scratch: &mut [SemanticHash],
    ) -> Result<SemanticHash, CheckpointError> {
        if scratch.len() < self.state.len() {
            return Err(CheckpointError::ScratchTooSmall);
        }
        for (index, state) in self.state.iter().enumerate() {
            scratch[index] = state
                .semantic_hash()
                .map_err(|_| CheckpointError::InvalidEnvelope)?;
        }
        let fields = checkpoint_fields(self);
        semantic_hash_with_hash_set(
            Id("conduit/checkpoint-envelope"),
            CHECKPOINT_SCHEMA_VERSION,
            &fields,
            Id("state"),
            &scratch[..self.state.len()],
        )
        .map_err(|_| CheckpointError::InvalidEnvelope)
    }
}

impl ResumeTarget<'_> {
    pub fn compatibility_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        compatibility_hash(
            self.plan_identity,
            self.implementation_hash,
            self.artifact_hash,
            self.configuration_hash,
            self.type_contracts_hash,
            self.template_hash,
            self.correlation_hash,
        )
    }
}

pub fn validate_checkpoint_resume(
    checkpoint: &CheckpointEnvelope<'_>,
    target: ResumeTarget<'_>,
    migration: Option<CheckpointMigration<'_>>,
    scratch: &mut [SemanticHash],
) -> Result<(), CheckpointError> {
    if checkpoint.schema_version != CHECKPOINT_SCHEMA_VERSION
        || !valid_id(checkpoint.checkpoint)
        || checkpoint.status != CheckpointStatus::Complete
        || !valid_id(checkpoint.job)
        || !valid_id(checkpoint.run)
        || !valid_id(checkpoint.work_unit)
        || !valid_id(checkpoint.source_attempt)
        || !valid_id(checkpoint.source_lease)
        || !valid_pin(checkpoint.provider)
        || !valid_id(checkpoint.evidence_stream)
        || !all_distinct_ids(&[
            checkpoint.checkpoint,
            checkpoint.job,
            checkpoint.run,
            checkpoint.work_unit,
            checkpoint.source_attempt,
            checkpoint.source_lease,
            checkpoint.evidence_stream,
        ])
        || checkpoint.sequence == 0
        || checkpoint.state.is_empty()
        || checkpoint.checkpoint != target.checkpoint
        || checkpoint.job != target.job
        || checkpoint.run != target.run
        || checkpoint.work_unit != target.work_unit
        || checkpoint.source_lease != target.source_lease
        || checkpoint.source_attempt == target.new_attempt
        || !valid_id(target.new_attempt)
        || checkpoint.source_lease == target.new_lease
        || !valid_id(target.new_lease)
        || target.new_lease == target.new_attempt
        || target.new_lease == target.job
        || target.new_lease == target.run
        || target.new_lease == target.work_unit
        || [target.checkpoint, target.job, target.run, target.work_unit]
            .contains(&target.new_attempt)
        || checkpoint.provider != target.checkpoint_provider
        || checkpoint.evidence_stream != target.evidence_stream
        || checkpoint.stream_epoch != target.stream_epoch
        || checkpoint.event_cursor != target.event_cursor
        || checkpoint.state.len() > usize::from(target.maximum_state_references)
    {
        return Err(CheckpointError::InvalidEnvelope);
    }
    let mut total = 0_u64;
    for (index, state) in checkpoint.state.iter().enumerate() {
        if !valid_state_ref(*state)
            || checkpoint.state[..index]
                .iter()
                .any(|prior| prior.id == state.id)
        {
            return Err(CheckpointError::InvalidEnvelope);
        }
        total = total
            .checked_add(state.bytes)
            .ok_or(CheckpointError::CheckpointTooLarge)?;
    }
    if total == 0 || total > target.maximum_checkpoint_bytes {
        return Err(CheckpointError::CheckpointTooLarge);
    }
    if checkpoint.computed_integrity(scratch)? != checkpoint.integrity {
        return Err(CheckpointError::IntegrityMismatch);
    }
    let source = checkpoint
        .compatibility_hash()
        .map_err(|_| CheckpointError::InvalidEnvelope)?;
    let target_hash = target
        .compatibility_hash()
        .map_err(|_| CheckpointError::InvalidEnvelope)?;
    if source == target_hash {
        if migration.is_some() {
            return Err(CheckpointError::MigrationInvalid);
        }
        return Ok(());
    }
    let Some(migration) = migration else {
        return Err(CheckpointError::Incompatible);
    };
    if !target.migration_supported
        || migration.source_compatibility != source
        || migration.target_compatibility != target_hash
        || migration.from_version != checkpoint.migration_version
        || migration.to_version <= migration.from_version
        || !valid_pin(migration.contract)
    {
        return Err(CheckpointError::MigrationInvalid);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationOutcome {
    Accepted,
    Rejected,
    Inconclusive,
    Conflicting,
    Late,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResultValidationDecision<'a> {
    pub id: Id<'a>,
    pub work_unit: Id<'a>,
    pub output: Id<'a>,
    pub output_digest: ArtifactDigest,
    pub validator: PinnedDescriptor<'a>,
    pub equivalence: PinnedDescriptor<'a>,
    pub homogeneous_constraint: Option<PinnedDescriptor<'a>>,
    pub compared_attempts: &'a [Id<'a>],
    pub decided_at_tick: u64,
    pub outcome: ValidationOutcome,
    pub canonical_result: Option<Id<'a>>,
}

pub fn validate_result_decision(
    policy: ResultValidationPolicy<'_>,
    decision: ResultValidationDecision<'_>,
) -> Result<(), JobError> {
    let compared = decision.compared_attempts.len();
    let identities_valid = all_distinct_ids(&[decision.id, decision.work_unit, decision.output])
        && valid_id(decision.id)
        && valid_id(decision.work_unit)
        && valid_id(decision.output)
        && decision
            .compared_attempts
            .iter()
            .enumerate()
            .all(|(index, attempt)| {
                valid_id(*attempt)
                    && ![decision.id, decision.work_unit, decision.output].contains(attempt)
                    && !decision.compared_attempts[..index].contains(attempt)
            });
    let accepted = decision.outcome == ValidationOutcome::Accepted;
    if !valid_validation_policy(policy)
        || !identities_valid
        || decision.validator != policy.validator
        || decision.equivalence != policy.equivalence
        || decision.homogeneous_constraint != policy.homogeneous_constraint
        || compared == 0
        || compared > usize::from(policy.maximum_results)
        || (accepted && compared < usize::from(policy.quorum))
        || (accepted != decision.canonical_result.is_some())
        || decision.canonical_result.is_some_and(|id| {
            !valid_id(id)
                || [decision.id, decision.work_unit, decision.output].contains(&id)
                || decision.compared_attempts.contains(&id)
        })
        || (decision.decided_at_tick > policy.deadline_ticks
            && decision.outcome != ValidationOutcome::Late)
    {
        return Err(JobError::ValidationInvalid);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobEvidenceKind {
    Progress,
    CheckpointCommitted,
    AttemptExecuted,
    Validation,
    AcceptedResult,
    Cancelled,
    Failed,
}

impl JobEvidenceKind {
    pub const fn permits_committed_total(self) -> bool {
        matches!(self, Self::AttemptExecuted | Self::AcceptedResult)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobEvidenceRecord<'a> {
    pub event: Id<'a>,
    pub job: Id<'a>,
    pub run: Id<'a>,
    pub attempt: Id<'a>,
    pub work_unit: Id<'a>,
    pub sequence: u64,
    pub progress_units: u64,
    pub kind: JobEvidenceKind,
}

/// Validate the job-specific facts carried by a typed Resonance payload.
///
/// The immutable job record remains distinct from its stream envelope. This
/// check binds the two without treating a mutable progress projection as
/// authoritative.
pub fn validate_job_evidence_envelope(
    contract: JobContract<'_>,
    identity: JobIdentity<'_>,
    record: JobEvidenceRecord<'_>,
    envelope: &ResonanceEnvelope<'_>,
) -> Result<(), JobError> {
    if validate_envelope(envelope).is_err()
        || !all_distinct_ids(&[
            record.event,
            record.job,
            record.run,
            record.attempt,
            record.work_unit,
        ])
        || record.event != envelope.event
        || record.job != identity.job
        || record.run != identity.run
        || record.attempt != identity.attempt
        || record.work_unit != identity.work_unit
        || record.sequence == 0
        || record.sequence != envelope.sequence
        || record.progress_units > contract.total_work_units
        || (record.progress_units == contract.total_work_units
            && !record.kind.permits_committed_total())
        || envelope.stream != contract.evidence_stream
        || envelope.run != identity.run
        || envelope.class != EventClass::NormativeEvidence
        || envelope.idempotency != Some(identity.idempotency)
    {
        return Err(JobError::EvidenceInvalid);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobPhase {
    Pending,
    Running,
    Checkpointing,
    Committing,
    Executed,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DurableCommit<'a> {
    pub idempotency: Id<'a>,
    pub result: Id<'a>,
    pub result_digest: ArtifactDigest,
    pub boundary: PinnedDescriptor<'a>,
    pub commit_evidence: Id<'a>,
    pub acknowledgement_evidence: Option<Id<'a>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryDecision {
    Retry {
        next_attempt_ordinal: u16,
        not_before_tick: u64,
    },
    ReturnCommitted,
    RejectDuplicate,
    AttemptsExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobAttemptMachine<'a> {
    contract: JobContract<'a>,
    identity: JobIdentity<'a>,
    lease: WorkLease<'a>,
    phase: JobPhase,
    progress_units: u64,
    evidence_sequence: u64,
    checkpoints: u16,
    cancellation_deadline: Option<u64>,
    cancellation_checkpoint_pending: bool,
    durable_commit: Option<DurableCommit<'a>>,
}

impl<'a> JobAttemptMachine<'a> {
    pub fn new(
        contract: JobContract<'a>,
        identity: JobIdentity<'a>,
        lease: WorkLease<'a>,
    ) -> Result<Self, JobError> {
        if !valid_contract(contract)
            || !valid_identity(identity)
            || identity.attempt_ordinal == 0
            || identity.attempt_ordinal > contract.maximum_attempts
            || !valid_lease(lease, identity, contract)
        {
            return Err(JobError::InvalidContract);
        }
        Ok(Self {
            contract,
            identity,
            lease,
            phase: JobPhase::Pending,
            progress_units: 0,
            evidence_sequence: 0,
            checkpoints: 0,
            cancellation_deadline: None,
            cancellation_checkpoint_pending: false,
            durable_commit: None,
        })
    }

    pub const fn phase(&self) -> JobPhase {
        self.phase
    }

    pub const fn progress_units(&self) -> u64 {
        self.progress_units
    }

    pub const fn evidence_sequence(&self) -> u64 {
        self.evidence_sequence
    }

    pub fn start(&mut self, now: u64) -> Result<(), JobError> {
        self.require_live_attempt(now)?;
        if self.phase != JobPhase::Pending {
            return Err(JobError::IllegalTransition);
        }
        self.phase = JobPhase::Running;
        self.bump_evidence()
    }

    pub fn record_progress(&mut self, units: u64, now: u64) -> Result<(), JobError> {
        self.require_live_attempt(now)?;
        if self.phase != JobPhase::Running || units == 0 {
            return Err(JobError::IllegalTransition);
        }
        let next = self
            .progress_units
            .checked_add(units)
            .ok_or(JobError::ProgressOutOfBounds)?;
        if next >= self.contract.total_work_units {
            return Err(JobError::ProgressOutOfBounds);
        }
        self.progress_units = next;
        self.bump_evidence()
    }

    pub fn begin_checkpoint(&mut self, now: u64) -> Result<(), JobError> {
        self.require_live_attempt(now)?;
        if self.phase != JobPhase::Running || self.checkpoints == self.contract.maximum_checkpoints
        {
            return Err(JobError::CheckpointUnavailable);
        }
        self.phase = JobPhase::Checkpointing;
        self.bump_evidence()
    }

    pub fn finish_checkpoint(&mut self, complete: bool, now: u64) -> Result<(), JobError> {
        if self.phase != JobPhase::Checkpointing {
            return Err(JobError::IllegalTransition);
        }
        if self
            .cancellation_deadline
            .is_some_and(|deadline| now >= deadline)
        {
            self.phase = JobPhase::Cancelled;
            self.cancellation_checkpoint_pending = false;
            return self.bump_evidence();
        }
        self.require_live_attempt(now)?;
        self.phase = if complete {
            self.checkpoints += 1;
            if self.cancellation_checkpoint_pending {
                self.cancellation_checkpoint_pending = false;
                self.cancellation_deadline = None;
                JobPhase::Cancelled
            } else {
                JobPhase::Running
            }
        } else if self.cancellation_checkpoint_pending {
            self.cancellation_checkpoint_pending = false;
            self.cancellation_deadline = None;
            JobPhase::Cancelled
        } else {
            JobPhase::Failed
        };
        self.bump_evidence()
    }

    pub fn begin_commit(&mut self, now: u64) -> Result<(), JobError> {
        self.require_live_attempt(now)?;
        if self.phase != JobPhase::Running {
            return Err(JobError::IllegalTransition);
        }
        self.phase = JobPhase::Committing;
        self.bump_evidence()
    }

    pub fn record_durable_commit(
        &mut self,
        commit: DurableCommit<'a>,
        now: u64,
    ) -> Result<(), JobError> {
        self.require_live_attempt(now)?;
        if self.phase != JobPhase::Committing
            || commit.idempotency != self.identity.idempotency
            || !valid_id(commit.idempotency)
            || !valid_id(commit.result)
            || [
                self.identity.job,
                self.identity.run,
                self.identity.attempt,
                self.identity.work_unit,
                self.identity.idempotency,
            ]
            .contains(&commit.result)
            || commit.boundary != self.contract.commit_boundary
            || !valid_id(commit.commit_evidence)
            || commit.commit_evidence == commit.result
            || commit.acknowledgement_evidence.is_some_and(|id| {
                !valid_id(id) || id == commit.result || id == commit.commit_evidence
            })
            || (self.contract.delivery == DeliveryClaim::TransactionalExactlyOnce
                && commit.acknowledgement_evidence.is_none())
        {
            return Err(JobError::CommitMismatch);
        }
        self.durable_commit = Some(commit);
        self.bump_evidence()
    }

    pub fn complete(&mut self) -> Result<(), JobError> {
        if self.phase != JobPhase::Committing || self.durable_commit.is_none() {
            return Err(JobError::CompletionNotCommitted);
        }
        self.progress_units = self.contract.total_work_units;
        self.phase = JobPhase::Executed;
        self.bump_evidence()
    }

    pub fn cancel(&mut self, stop: StopPolicy, now: u64) -> Result<(), JobError> {
        if matches!(
            self.phase,
            JobPhase::Executed | JobPhase::Cancelled | JobPhase::Failed
        ) {
            return Err(JobError::IllegalTransition);
        }
        match (stop, self.contract.cancellation_checkpoint) {
            (
                StopPolicy::Drain,
                CancellationCheckpointPolicy::FinalCheckpoint { maximum_ticks },
            ) if matches!(self.phase, JobPhase::Running | JobPhase::Checkpointing)
                && self.contract.maximum_checkpoints > self.checkpoints =>
            {
                let deadline = now
                    .checked_add(maximum_ticks)
                    .ok_or(JobError::CancellationBoundExceeded)?;
                self.cancellation_deadline = Some(deadline);
                self.cancellation_checkpoint_pending = true;
                self.phase = JobPhase::Checkpointing;
            }
            _ => {
                self.cancellation_checkpoint_pending = false;
                self.phase = JobPhase::Cancelled;
            }
        }
        self.bump_evidence()
    }

    pub fn poll_cancellation(&mut self, now: u64) -> Result<(), JobError> {
        let Some(deadline) = self.cancellation_deadline else {
            return Err(JobError::IllegalTransition);
        };
        if now < deadline || self.phase != JobPhase::Checkpointing {
            return Err(JobError::CancellationPending);
        }
        self.cancellation_checkpoint_pending = false;
        self.cancellation_deadline = None;
        self.phase = JobPhase::Cancelled;
        self.bump_evidence()
    }

    pub fn renew_lease(&mut self, replacement: WorkLease<'a>, now: u64) -> Result<(), JobError> {
        if !valid_lease(replacement, self.identity, self.contract)
            || now < self.lease.issued_at_tick
            || now >= self.lease.expires_at_tick
            || replacement.issued_at_tick != now
            || replacement.id != self.lease.id
            || self.lease.renewal.checked_add(1) != Some(replacement.renewal)
            || replacement.renewal > self.contract.maximum_lease_renewals
            || replacement.issued_at_tick < self.lease.issued_at_tick
            || replacement.expires_at_tick <= self.lease.expires_at_tick
        {
            return Err(JobError::LeaseInvalid);
        }
        self.lease = replacement;
        self.bump_evidence()
    }

    pub fn recover_after_crash(&self, now: u64) -> RecoveryDecision {
        if self.durable_commit.is_some() {
            match self.contract.duplicate_policy {
                DuplicatePolicy::Reject => RecoveryDecision::RejectDuplicate,
                DuplicatePolicy::ReturnCommitted | DuplicatePolicy::RetryWithSameKey => {
                    RecoveryDecision::ReturnCommitted
                }
            }
        } else if self.identity.attempt_ordinal >= self.contract.maximum_attempts {
            RecoveryDecision::AttemptsExhausted
        } else {
            now.checked_add(self.contract.retry_backoff_ticks).map_or(
                RecoveryDecision::AttemptsExhausted,
                |not_before_tick| RecoveryDecision::Retry {
                    next_attempt_ordinal: self.identity.attempt_ordinal + 1,
                    not_before_tick,
                },
            )
        }
    }

    fn require_live_attempt(&self, now: u64) -> Result<(), JobError> {
        if now < self.lease.issued_at_tick || now >= self.lease.expires_at_tick {
            Err(JobError::LeaseExpired)
        } else if self
            .lease
            .issued_at_tick
            .checked_add(self.contract.attempt_deadline_ticks)
            .is_none_or(|deadline| now >= deadline)
        {
            Err(JobError::AttemptDeadlineExceeded)
        } else {
            Ok(())
        }
    }

    fn bump_evidence(&mut self) -> Result<(), JobError> {
        self.evidence_sequence = self
            .evidence_sequence
            .checked_add(1)
            .ok_or(JobError::EvidenceOverflow)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckpointError {
    InvalidEnvelope,
    ScratchTooSmall,
    CheckpointTooLarge,
    IntegrityMismatch,
    Incompatible,
    MigrationInvalid,
}

impl CheckpointError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidEnvelope => "CND-JOB-001",
            Self::ScratchTooSmall | Self::CheckpointTooLarge => "CND-JOB-002",
            Self::IntegrityMismatch => "CND-JOB-003",
            Self::Incompatible => "CND-JOB-004",
            Self::MigrationInvalid => "CND-JOB-005",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobError {
    InvalidContract,
    IllegalTransition,
    ProgressOutOfBounds,
    CheckpointUnavailable,
    CommitMismatch,
    CompletionNotCommitted,
    LeaseExpired,
    EvidenceOverflow,
    AttemptDeadlineExceeded,
    CancellationPending,
    CancellationBoundExceeded,
    LeaseInvalid,
    EvidenceInvalid,
    ValidationInvalid,
    ProviderIncapable,
}

impl JobError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidContract => "CND-JOB-006",
            Self::IllegalTransition | Self::CheckpointUnavailable => "CND-JOB-007",
            Self::ProgressOutOfBounds | Self::CompletionNotCommitted => "CND-JOB-008",
            Self::CommitMismatch => "CND-JOB-009",
            Self::LeaseExpired => "CND-JOB-010",
            Self::EvidenceOverflow => "CND-JOB-011",
            Self::AttemptDeadlineExceeded
            | Self::CancellationPending
            | Self::CancellationBoundExceeded => "CND-JOB-012",
            Self::LeaseInvalid => "CND-JOB-013",
            Self::EvidenceInvalid => "CND-JOB-014",
            Self::ValidationInvalid => "CND-JOB-015",
            Self::ProviderIncapable => "CND-JOB-016",
        }
    }
}

impl fmt::Display for CheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for JobError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

pub fn validate_job_contract(
    contract: JobContract<'_>,
    checkpoint_provider: Option<CheckpointProviderCapabilities>,
    evidence_stream: EventStreamContract<'_>,
    event_provider: EventProviderCapabilities,
) -> Result<(), JobError> {
    if !valid_contract(contract) {
        return Err(JobError::InvalidContract);
    }
    if evidence_stream.id != contract.evidence_stream
        || evidence_stream.event_class != EventClass::NormativeEvidence
        || !matches!(
            evidence_stream.retention,
            RetentionPolicy::DurableAppend { .. }
        )
        || evidence_stream.delivery != ReplayDelivery::AtLeastOnce
        || !evidence_stream.terminal_evidence_required
        || validate_stream_contract(evidence_stream, event_provider).is_err()
    {
        return Err(JobError::ProviderIncapable);
    }
    match (contract.checkpoint_provider, checkpoint_provider) {
        (None, None) if contract.maximum_checkpoints == 0 => Ok(()),
        (Some(provider), Some(capabilities))
            if valid_pin(provider)
                && capabilities.durable
                && capabilities.integrity
                && capabilities.maximum_checkpoints >= contract.maximum_checkpoints
                && capabilities.maximum_checkpoint_bytes >= contract.maximum_checkpoint_bytes
                && capabilities.maximum_state_references
                    >= contract.maximum_checkpoint_state_refs
                && capabilities.maximum_pending_operations
                    >= contract.maximum_checkpoint_operations =>
        {
            Ok(())
        }
        _ => Err(JobError::ProviderIncapable),
    }
}

impl JobContract<'_> {
    pub fn semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let (checkpoint_provider_id, checkpoint_provider_version, checkpoint_provider_hash) =
            self.checkpoint_provider.as_ref().map_or(
                (
                    CanonicalValue::Null,
                    CanonicalValue::Null,
                    CanonicalValue::Null,
                ),
                |provider| {
                    (
                        CanonicalValue::Identifier(provider.id),
                        CanonicalValue::Integer(i128::from(provider.schema_version)),
                        CanonicalValue::Bytes(provider.semantic_hash.as_bytes()),
                    )
                },
            );
        let (restart, maximum_lost_work_units) = match self.restart {
            RestartPolicy::ResumeRequired => ("resume-required", 0),
            RestartPolicy::RestartFromBeginning {
                maximum_lost_work_units,
            } => ("restart-from-beginning", maximum_lost_work_units),
        };
        let (cancel, cancel_ticks) = match self.cancellation_checkpoint {
            CancellationCheckpointPolicy::None => ("none", 0),
            CancellationCheckpointPolicy::FinalCheckpoint { maximum_ticks } => {
                ("final-checkpoint", maximum_ticks)
            }
        };
        let delivery = match self.delivery {
            DeliveryClaim::AtMostOnce => "at-most-once",
            DeliveryClaim::AtLeastOnce => "at-least-once",
            DeliveryClaim::TransactionalExactlyOnce => "transactional-exactly-once",
        };
        let duplicate = match self.duplicate_policy {
            DuplicatePolicy::Reject => "reject",
            DuplicatePolicy::ReturnCommitted => "return-committed",
            DuplicatePolicy::RetryWithSameKey => "retry-with-same-key",
        };
        let validation_hash = self
            .result_validation
            .map(hash_validation_policy)
            .transpose()?;
        let (
            transactional_boundary_id,
            transactional_boundary_version,
            transactional_boundary_hash,
        ) = self.transactional_boundary.as_ref().map_or(
            (
                CanonicalValue::Null,
                CanonicalValue::Null,
                CanonicalValue::Null,
            ),
            |boundary| {
                (
                    CanonicalValue::Identifier(boundary.id),
                    CanonicalValue::Integer(i128::from(boundary.schema_version)),
                    CanonicalValue::Bytes(boundary.semantic_hash.as_bytes()),
                )
            },
        );
        CanonicalDescriptor {
            kind: Id("conduit/job-contract"),
            schema_version: JOB_CONTRACT_VERSION,
            body: CanonicalValue::Map(&[
                semantic("id", CanonicalValue::Identifier(self.id)),
                semantic(
                    "total_work_units",
                    CanonicalValue::Integer(i128::from(self.total_work_units)),
                ),
                semantic(
                    "maximum_attempts",
                    CanonicalValue::Integer(i128::from(self.maximum_attempts)),
                ),
                semantic(
                    "retry_backoff_ticks",
                    CanonicalValue::Integer(i128::from(self.retry_backoff_ticks)),
                ),
                semantic(
                    "attempt_deadline_ticks",
                    CanonicalValue::Integer(i128::from(self.attempt_deadline_ticks)),
                ),
                semantic(
                    "maximum_checkpoints",
                    CanonicalValue::Integer(i128::from(self.maximum_checkpoints)),
                ),
                semantic(
                    "maximum_checkpoint_bytes",
                    CanonicalValue::Integer(i128::from(self.maximum_checkpoint_bytes)),
                ),
                semantic(
                    "maximum_checkpoint_state_refs",
                    CanonicalValue::Integer(i128::from(self.maximum_checkpoint_state_refs)),
                ),
                semantic(
                    "maximum_checkpoint_operations",
                    CanonicalValue::Integer(i128::from(self.maximum_checkpoint_operations)),
                ),
                semantic("lease_basis", CanonicalValue::Identifier(self.lease_basis)),
                semantic(
                    "maximum_lease_renewals",
                    CanonicalValue::Integer(i128::from(self.maximum_lease_renewals)),
                ),
                semantic("delivery", CanonicalValue::Identifier(Id(delivery))),
                semantic(
                    "duplicate_policy",
                    CanonicalValue::Identifier(Id(duplicate)),
                ),
                semantic(
                    "commit_boundary_id",
                    CanonicalValue::Identifier(self.commit_boundary.id),
                ),
                semantic(
                    "commit_boundary_version",
                    CanonicalValue::Integer(i128::from(self.commit_boundary.schema_version)),
                ),
                semantic(
                    "commit_boundary_hash",
                    CanonicalValue::Bytes(self.commit_boundary.semantic_hash.as_bytes()),
                ),
                semantic("transactional_boundary_id", transactional_boundary_id),
                semantic(
                    "transactional_boundary_version",
                    transactional_boundary_version,
                ),
                semantic("transactional_boundary_hash", transactional_boundary_hash),
                semantic("checkpoint_provider_id", checkpoint_provider_id),
                semantic("checkpoint_provider_version", checkpoint_provider_version),
                semantic("checkpoint_provider_hash", checkpoint_provider_hash),
                semantic(
                    "evidence_stream",
                    CanonicalValue::Identifier(self.evidence_stream),
                ),
                semantic("restart", CanonicalValue::Identifier(Id(restart))),
                semantic(
                    "maximum_lost_work_units",
                    CanonicalValue::Integer(i128::from(maximum_lost_work_units)),
                ),
                semantic(
                    "cancellation_checkpoint",
                    CanonicalValue::Identifier(Id(cancel)),
                ),
                semantic(
                    "cancellation_checkpoint_ticks",
                    CanonicalValue::Integer(i128::from(cancel_ticks)),
                ),
                semantic(
                    "result_validation_hash",
                    validation_hash
                        .as_ref()
                        .map_or(CanonicalValue::Null, |hash| {
                            CanonicalValue::Bytes(hash.as_bytes())
                        }),
                ),
            ]),
        }
        .semantic_hash()
    }
}

fn valid_contract(contract: JobContract<'_>) -> bool {
    valid_id(contract.id)
        && contract.total_work_units > 0
        && contract.maximum_attempts > 0
        && contract.retry_backoff_ticks > 0
        && contract.attempt_deadline_ticks > 0
        && valid_id(contract.lease_basis)
        && valid_pin(contract.commit_boundary)
        && match contract.delivery {
            DeliveryClaim::TransactionalExactlyOnce => {
                contract.transactional_boundary.is_some_and(valid_pin)
            }
            DeliveryClaim::AtMostOnce | DeliveryClaim::AtLeastOnce => {
                contract.transactional_boundary.is_none()
            }
        }
        && valid_id(contract.evidence_stream)
        && match contract.checkpoint_provider {
            Some(provider) => {
                valid_pin(provider)
                    && contract.maximum_checkpoints > 0
                    && contract.maximum_checkpoint_bytes > 0
                    && contract.maximum_checkpoint_state_refs > 0
                    && contract.maximum_checkpoint_operations > 0
            }
            None => {
                contract.maximum_checkpoints == 0
                    && contract.maximum_checkpoint_bytes == 0
                    && contract.maximum_checkpoint_state_refs == 0
                    && contract.maximum_checkpoint_operations == 0
            }
        }
        && match contract.restart {
            RestartPolicy::ResumeRequired => contract.checkpoint_provider.is_some(),
            RestartPolicy::RestartFromBeginning {
                maximum_lost_work_units,
            } => maximum_lost_work_units > 0,
        }
        && match contract.cancellation_checkpoint {
            CancellationCheckpointPolicy::None => true,
            CancellationCheckpointPolicy::FinalCheckpoint { maximum_ticks } => {
                maximum_ticks > 0 && contract.checkpoint_provider.is_some()
            }
        }
        && contract
            .result_validation
            .is_none_or(valid_validation_policy)
}

fn valid_identity(identity: JobIdentity<'_>) -> bool {
    let identities = [
        identity.job,
        identity.run,
        identity.attempt,
        identity.work_unit,
        identity.idempotency,
    ];
    identity.attempt_ordinal > 0
        && identities
            .iter()
            .enumerate()
            .all(|(index, id)| valid_id(*id) && !identities[..index].contains(id))
}

fn valid_lease(lease: WorkLease<'_>, identity: JobIdentity<'_>, contract: JobContract<'_>) -> bool {
    valid_id(lease.id)
        && ![
            identity.job,
            identity.run,
            identity.attempt,
            identity.work_unit,
            identity.idempotency,
        ]
        .contains(&lease.id)
        && lease.job == identity.job
        && lease.run == identity.run
        && lease.attempt == identity.attempt
        && lease.work_unit == identity.work_unit
        && lease.time_basis == contract.lease_basis
        && lease.expires_at_tick > lease.issued_at_tick
        && lease.renewal <= contract.maximum_lease_renewals
        && lease
            .issued_at_tick
            .checked_add(contract.attempt_deadline_ticks)
            .is_some()
}

fn valid_validation_policy(policy: ResultValidationPolicy<'_>) -> bool {
    valid_pin(policy.validator)
        && valid_pin(policy.equivalence)
        && policy.homogeneous_constraint.is_none_or(valid_pin)
        && policy.maximum_results > 0
        && policy.quorum > 0
        && policy.quorum <= policy.maximum_results
        && policy.deadline_ticks > 0
}

fn valid_state_ref(state: CheckpointStateRef<'_>) -> bool {
    valid_id(state.id)
        && InstancePath::new(state.owner.as_str()).is_ok()
        && valid_pin(state.state_contract)
        && state.bytes > 0
}

fn valid_pin(pin: PinnedDescriptor<'_>) -> bool {
    valid_id(pin.id) && pin.schema_version > 0
}

fn valid_id(id: Id<'_>) -> bool {
    Id::new(id.as_str()).is_ok()
}

fn all_distinct_ids(values: &[Id<'_>]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(index, value)| !values[..index].contains(value))
}

fn hash_state_ref(
    state: CheckpointStateRef<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    CanonicalDescriptor {
        kind: Id("conduit/checkpoint-state-ref"),
        schema_version: 1,
        body: CanonicalValue::Map(&[
            semantic("bytes", CanonicalValue::Integer(i128::from(state.bytes))),
            semantic(
                "content_digest",
                CanonicalValue::Bytes(state.content_digest.as_bytes()),
            ),
            semantic("id", CanonicalValue::Identifier(state.id)),
            semantic("kind", CanonicalValue::Text(state.kind.as_str())),
            semantic("owner", CanonicalValue::Text(state.owner.as_str())),
            semantic(
                "state_contract_hash",
                CanonicalValue::Bytes(state.state_contract.semantic_hash.as_bytes()),
            ),
            semantic(
                "state_contract_id",
                CanonicalValue::Identifier(state.state_contract.id),
            ),
            semantic(
                "state_contract_version",
                CanonicalValue::Integer(i128::from(state.state_contract.schema_version)),
            ),
        ]),
    }
    .semantic_hash()
}

fn compatibility_hash(
    plan: SemanticHash,
    implementation: SemanticHash,
    artifact: SemanticHash,
    configuration: SemanticHash,
    types: SemanticHash,
    template: SemanticHash,
    correlation: SemanticHash,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    CanonicalDescriptor {
        kind: Id("conduit/checkpoint-compatibility"),
        schema_version: 1,
        body: CanonicalValue::Map(&[
            semantic("artifact", CanonicalValue::Bytes(artifact.as_bytes())),
            semantic(
                "configuration",
                CanonicalValue::Bytes(configuration.as_bytes()),
            ),
            semantic("correlation", CanonicalValue::Bytes(correlation.as_bytes())),
            semantic(
                "implementation",
                CanonicalValue::Bytes(implementation.as_bytes()),
            ),
            semantic("plan", CanonicalValue::Bytes(plan.as_bytes())),
            semantic("template", CanonicalValue::Bytes(template.as_bytes())),
            semantic("types", CanonicalValue::Bytes(types.as_bytes())),
        ]),
    }
    .semantic_hash()
}

fn hash_validation_policy(
    policy: ResultValidationPolicy<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    CanonicalDescriptor {
        kind: Id("conduit/result-validation-policy"),
        schema_version: 1,
        body: CanonicalValue::Map(&[
            semantic(
                "validator_id",
                CanonicalValue::Identifier(policy.validator.id),
            ),
            semantic(
                "validator_version",
                CanonicalValue::Integer(i128::from(policy.validator.schema_version)),
            ),
            semantic(
                "validator_hash",
                CanonicalValue::Bytes(policy.validator.semantic_hash.as_bytes()),
            ),
            semantic(
                "equivalence_id",
                CanonicalValue::Identifier(policy.equivalence.id),
            ),
            semantic(
                "equivalence_version",
                CanonicalValue::Integer(i128::from(policy.equivalence.schema_version)),
            ),
            semantic(
                "equivalence_hash",
                CanonicalValue::Bytes(policy.equivalence.semantic_hash.as_bytes()),
            ),
            semantic(
                "homogeneous_constraint_id",
                policy
                    .homogeneous_constraint
                    .as_ref()
                    .map_or(CanonicalValue::Null, |pin| {
                        CanonicalValue::Identifier(pin.id)
                    }),
            ),
            semantic(
                "homogeneous_constraint_version",
                policy
                    .homogeneous_constraint
                    .as_ref()
                    .map_or(CanonicalValue::Null, |pin| {
                        CanonicalValue::Integer(i128::from(pin.schema_version))
                    }),
            ),
            semantic(
                "homogeneous_constraint_hash",
                policy
                    .homogeneous_constraint
                    .as_ref()
                    .map_or(CanonicalValue::Null, |pin| {
                        CanonicalValue::Bytes(pin.semantic_hash.as_bytes())
                    }),
            ),
            semantic(
                "maximum_results",
                CanonicalValue::Integer(i128::from(policy.maximum_results)),
            ),
            semantic("quorum", CanonicalValue::Integer(i128::from(policy.quorum))),
            semantic(
                "deadline_ticks",
                CanonicalValue::Integer(i128::from(policy.deadline_ticks)),
            ),
        ]),
    }
    .semantic_hash()
}

fn checkpoint_fields<'a>(value: &'a CheckpointEnvelope<'a>) -> [MapField<'a>; 23] {
    let status = match value.status {
        CheckpointStatus::Complete => "complete",
        CheckpointStatus::Partial => "partial",
        CheckpointStatus::Failed => "failed",
    };
    [
        semantic(
            "artifact_hash",
            CanonicalValue::Bytes(value.artifact_hash.as_bytes()),
        ),
        semantic("checkpoint", CanonicalValue::Identifier(value.checkpoint)),
        semantic(
            "configuration_hash",
            CanonicalValue::Bytes(value.configuration_hash.as_bytes()),
        ),
        semantic(
            "correlation_hash",
            CanonicalValue::Bytes(value.correlation_hash.as_bytes()),
        ),
        semantic(
            "event_cursor",
            CanonicalValue::Integer(i128::from(value.event_cursor)),
        ),
        semantic(
            "evidence_stream",
            CanonicalValue::Identifier(value.evidence_stream),
        ),
        semantic(
            "implementation_hash",
            CanonicalValue::Bytes(value.implementation_hash.as_bytes()),
        ),
        semantic("job", CanonicalValue::Identifier(value.job)),
        semantic(
            "migration_version",
            CanonicalValue::Integer(i128::from(value.migration_version)),
        ),
        semantic(
            "plan_identity",
            CanonicalValue::Bytes(value.plan_identity.as_bytes()),
        ),
        semantic(
            "provider_hash",
            CanonicalValue::Bytes(value.provider.semantic_hash.as_bytes()),
        ),
        semantic("provider_id", CanonicalValue::Identifier(value.provider.id)),
        semantic(
            "provider_version",
            CanonicalValue::Integer(i128::from(value.provider.schema_version)),
        ),
        semantic("run", CanonicalValue::Identifier(value.run)),
        semantic(
            "schema_version",
            CanonicalValue::Integer(i128::from(value.schema_version)),
        ),
        semantic(
            "sequence",
            CanonicalValue::Integer(i128::from(value.sequence)),
        ),
        semantic(
            "source_attempt",
            CanonicalValue::Identifier(value.source_attempt),
        ),
        semantic(
            "source_lease",
            CanonicalValue::Identifier(value.source_lease),
        ),
        semantic("status", CanonicalValue::Identifier(Id(status))),
        semantic(
            "stream_epoch",
            CanonicalValue::Bytes(value.stream_epoch.as_bytes()),
        ),
        semantic(
            "template_hash",
            CanonicalValue::Bytes(value.template_hash.as_bytes()),
        ),
        semantic(
            "type_contracts_hash",
            CanonicalValue::Bytes(value.type_contracts_hash.as_bytes()),
        ),
        semantic("work_unit", CanonicalValue::Identifier(value.work_unit)),
    ]
}

const fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}
