//! Finite resource leases, domain commit points, and cleanup disposition.

use core::convert::Infallible;
use core::fmt;

use crate::{
    AuthorityTime, CanonicalDescriptor, CanonicalError, CanonicalValue, EffectRequirement,
    ExecutionProfile, FieldDisposition, HostCapability, HostOperationContext, HostOperationRequest,
    Id, ImplementationError, InstancePath, MapField, ObservedGrant, PinnedDescriptor,
    PlanResourceBudget, ResolvedAuthorityBinding, SemanticHash, validate_authority_at_use,
    validate_host_operation,
};

pub const RESOURCE_LEASE_SCHEMA_VERSION: u32 = 0;
pub const EFFECT_COMMIT_PROFILE_SCHEMA_VERSION: u32 = 0;

/// Whether and how one exact resource reservation may be shared.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceSharingMode {
    Exclusive,
    SharedRead,
    SharedBounded { maximum_holders: u16 },
}

impl ResourceSharingMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Exclusive => "exclusive",
            Self::SharedRead => "shared-read",
            Self::SharedBounded { .. } => "shared-bounded",
        }
    }

    const fn maximum_holders(self) -> u16 {
        match self {
            Self::Exclusive => 1,
            Self::SharedRead => u16::MAX,
            Self::SharedBounded { maximum_holders } => maximum_holders,
        }
    }
}

/// Truthful classification of bytes retained outside executor ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForeignRetention {
    None,
    Bounded {
        maximum_bytes: u64,
        release_ticks: u64,
    },
    ObservedOnly,
    Unsupported,
}

impl ForeignRetention {
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bounded { .. } => "bounded",
            Self::ObservedOnly => "observed-only",
            Self::Unsupported => "unsupported",
        }
    }
}

/// One exact pre-effect resource lease. It is authority to use a reservation,
/// not proof that an effect committed or that retained data is durable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceLeaseContract<'a> {
    pub schema_version: u32,
    pub id: Id<'a>,
    pub resource_binding: Id<'a>,
    pub holder: InstancePath<'a>,
    pub run: Id<'a>,
    pub epoch: u64,
    pub scope: Id<'a>,
    pub sharing: ResourceSharingMode,
    pub reservation: PlanResourceBudget,
    pub time_basis: Id<'a>,
    pub issued_at_tick: u64,
    pub expires_at_tick: u64,
    pub revocation_grace_ticks: u64,
    pub cleanup_ticks: u64,
    pub maximum_operations: u32,
    pub maximum_evidence_events: u32,
    pub cleanup_escalation: PinnedDescriptor<'a>,
    pub foreign_retention: ForeignRetention,
}

/// Relationship between retries and a domain-owned commit boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectIdempotency {
    None,
    SameKeySameEffect,
    ReconcileBeforeRetry,
}

impl EffectIdempotency {
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SameKeySameEffect => "same-key-same-effect",
            Self::ReconcileBeforeRetry => "reconcile-before-retry",
        }
    }
}

/// What a provider may do after it cannot determine whether commit occurred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownCommitPolicy {
    Fail,
    Reconcile,
    RetrySameIdempotencyKey,
}

impl UnknownCommitPolicy {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Fail => "fail",
            Self::Reconcile => "reconcile",
            Self::RetrySameIdempotencyKey => "retry-same-idempotency-key",
        }
    }
}

/// Outcome when the selected provider/host disappears mid-operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectDiscontinuity {
    FailedBeforeCommit,
    CommitUnknown,
    ReconcileRequired,
}

impl EffectDiscontinuity {
    const fn as_str(self) -> &'static str {
        match self {
            Self::FailedBeforeCommit => "failed-before-commit",
            Self::CommitUnknown => "commit-unknown",
            Self::ReconcileRequired => "reconcile-required",
        }
    }
}

/// Domain-owned effect semantics pinned beside one exact authority binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectCommitProfile<'a> {
    pub schema_version: u32,
    pub id: Id<'a>,
    pub operation: Id<'a>,
    pub resource_lease: Id<'a>,
    pub commit_boundary: PinnedDescriptor<'a>,
    pub idempotency: EffectIdempotency,
    pub unknown_commit: UnknownCommitPolicy,
    pub discontinuity: EffectDiscontinuity,
    pub cleanup: PinnedDescriptor<'a>,
    pub maximum_attempts: u16,
    pub evidence_events_per_attempt: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceLeaseReason {
    InvalidContract,
    IdentityMismatch,
    WrongHolder,
    WrongRun,
    WrongEpoch,
    WrongResource,
    TimeBasisMismatch,
    NotYetValid,
    Expired,
    Revoked,
    ExclusiveConflict,
    HolderLimit,
    OperationLimit,
    StaleRelease,
    CleanupRequired,
    CleanupTimeout,
    EvidenceExhausted,
    CommitUnknown,
    RetryForbidden,
    HostLost,
    IllegalTransition,
}

impl ResourceLeaseReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidContract => "CND-LSE-001",
            Self::IdentityMismatch => "CND-LSE-002",
            Self::WrongHolder => "CND-LSE-003",
            Self::WrongRun => "CND-LSE-004",
            Self::WrongEpoch => "CND-LSE-005",
            Self::WrongResource => "CND-LSE-006",
            Self::TimeBasisMismatch => "CND-LSE-007",
            Self::NotYetValid => "CND-LSE-008",
            Self::Expired => "CND-LSE-009",
            Self::Revoked => "CND-LSE-010",
            Self::ExclusiveConflict => "CND-LSE-011",
            Self::HolderLimit => "CND-LSE-012",
            Self::OperationLimit => "CND-LSE-013",
            Self::StaleRelease => "CND-LSE-014",
            Self::CleanupRequired => "CND-LSE-015",
            Self::CleanupTimeout => "CND-LSE-016",
            Self::EvidenceExhausted => "CND-LSE-017",
            Self::CommitUnknown => "CND-LSE-018",
            Self::RetryForbidden => "CND-LSE-019",
            Self::HostLost => "CND-LSE-020",
            Self::IllegalTransition => "CND-LSE-021",
        }
    }
}

impl fmt::Display for ResourceLeaseReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidContract => "resource lease or effect commit profile is invalid",
            Self::IdentityMismatch => "resource lease identity does not match",
            Self::WrongHolder => "resource lease holder does not match",
            Self::WrongRun => "resource lease run does not match",
            Self::WrongEpoch => "resource lease epoch does not match",
            Self::WrongResource => "resource lease does not cover the operation resource",
            Self::TimeBasisMismatch => "resource lease time basis does not match",
            Self::NotYetValid => "resource lease is not yet valid",
            Self::Expired => "resource lease expired",
            Self::Revoked => "resource lease was revoked",
            Self::ExclusiveConflict => "exclusive resource lease conflicts with another holder",
            Self::HolderLimit => "resource lease holder limit was exceeded",
            Self::OperationLimit => "resource lease operation limit was exceeded",
            Self::StaleRelease => "resource lease release sequence is stale",
            Self::CleanupRequired => "resource lease still requires cleanup disposition",
            Self::CleanupTimeout => "resource lease cleanup deadline expired",
            Self::EvidenceExhausted => "required lease evidence was exhausted before mutation",
            Self::CommitUnknown => "effect commit disposition is unknown",
            Self::RetryForbidden => "effect retry is forbidden by the commit profile",
            Self::HostLost => "effect provider disappeared before disposition was known",
            Self::IllegalTransition => "resource lease lifecycle transition is illegal",
        })
    }
}

pub fn validate_resource_lease(
    lease: ResourceLeaseContract<'_>,
) -> Result<(), ResourceLeaseReason> {
    let reservation_nonzero = lease.reservation != PlanResourceBudget::ZERO;
    let foreign_valid = match lease.foreign_retention {
        ForeignRetention::None | ForeignRetention::ObservedOnly | ForeignRetention::Unsupported => {
            true
        }
        ForeignRetention::Bounded {
            maximum_bytes,
            release_ticks,
        } => maximum_bytes > 0 && release_ticks > 0 && release_ticks <= lease.cleanup_ticks,
    };
    if lease.schema_version != RESOURCE_LEASE_SCHEMA_VERSION
        || Id::new(lease.id.as_str()).is_err()
        || Id::new(lease.resource_binding.as_str()).is_err()
        || InstancePath::new(lease.holder.as_str()).is_err()
        || Id::new(lease.run.as_str()).is_err()
        || Id::new(lease.scope.as_str()).is_err()
        || Id::new(lease.time_basis.as_str()).is_err()
        || lease.sharing.maximum_holders() == 0
        || !reservation_nonzero
        || lease.expires_at_tick <= lease.issued_at_tick
        || lease.revocation_grace_ticks == 0
        || lease.cleanup_ticks == 0
        || lease.maximum_operations == 0
        || lease.maximum_evidence_events < 2
        || !valid_pin(lease.cleanup_escalation)
        || !foreign_valid
    {
        return Err(ResourceLeaseReason::InvalidContract);
    }
    Ok(())
}

pub fn validate_effect_commit_profile(
    profile: EffectCommitProfile<'_>,
    lease: ResourceLeaseContract<'_>,
) -> Result<(), ResourceLeaseReason> {
    if profile.schema_version != EFFECT_COMMIT_PROFILE_SCHEMA_VERSION
        || Id::new(profile.id.as_str()).is_err()
        || Id::new(profile.operation.as_str()).is_err()
        || profile.resource_lease != lease.id
        || !valid_pin(profile.commit_boundary)
        || !valid_pin(profile.cleanup)
        || profile.maximum_attempts == 0
        || profile.evidence_events_per_attempt < 2
        || (profile.unknown_commit == UnknownCommitPolicy::RetrySameIdempotencyKey
            && profile.idempotency != EffectIdempotency::SameKeySameEffect)
    {
        return Err(ResourceLeaseReason::InvalidContract);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceLeasePhase {
    Active,
    Revoked,
    Expired,
    Cleaning,
    Released,
    Failed,
}

/// Allocation-free runtime state for one already planned lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceLeaseState<'a> {
    contract: ResourceLeaseContract<'a>,
    phase: ResourceLeasePhase,
    operations: u32,
    evidence_events: u32,
    cleanup_deadline_tick: Option<u64>,
    release_sequence: u64,
}

impl<'a> ResourceLeaseState<'a> {
    pub fn new(contract: ResourceLeaseContract<'a>) -> Result<Self, ResourceLeaseReason> {
        validate_resource_lease(contract)?;
        Ok(Self {
            contract,
            phase: ResourceLeasePhase::Active,
            operations: 0,
            evidence_events: 0,
            cleanup_deadline_tick: None,
            release_sequence: 0,
        })
    }

    #[must_use]
    pub const fn contract(&self) -> ResourceLeaseContract<'a> {
        self.contract
    }

    #[must_use]
    pub const fn phase(&self) -> ResourceLeasePhase {
        self.phase
    }

    pub fn record_required_evidence(&mut self) -> Result<u32, ResourceLeaseReason> {
        self.reserve_required_evidence(1)
    }

    /// Reserve a complete bounded evidence allowance before an effect can
    /// mutate its provider. A failed reservation changes no lease state.
    pub fn reserve_required_evidence(&mut self, count: u32) -> Result<u32, ResourceLeaseReason> {
        let next = self
            .evidence_events
            .checked_add(count)
            .ok_or(ResourceLeaseReason::EvidenceExhausted)?;
        if next > self.contract.maximum_evidence_events {
            return Err(ResourceLeaseReason::EvidenceExhausted);
        }
        self.evidence_events = next;
        Ok(next)
    }

    pub fn check_use(
        &self,
        resource_binding: Id<'_>,
        holder: InstancePath<'_>,
        run: Id<'_>,
        epoch: u64,
        now: AuthorityTime<'_>,
    ) -> Result<(), ResourceLeaseReason> {
        if resource_binding != self.contract.resource_binding {
            return Err(ResourceLeaseReason::WrongResource);
        }
        if holder != self.contract.holder {
            return Err(ResourceLeaseReason::WrongHolder);
        }
        if run != self.contract.run {
            return Err(ResourceLeaseReason::WrongRun);
        }
        if epoch != self.contract.epoch {
            return Err(ResourceLeaseReason::WrongEpoch);
        }
        if now.basis != self.contract.time_basis {
            return Err(ResourceLeaseReason::TimeBasisMismatch);
        }
        if now.tick < self.contract.issued_at_tick {
            return Err(ResourceLeaseReason::NotYetValid);
        }
        if now.tick >= self.contract.expires_at_tick || self.phase == ResourceLeasePhase::Expired {
            return Err(ResourceLeaseReason::Expired);
        }
        if self.phase == ResourceLeasePhase::Revoked {
            return Err(ResourceLeaseReason::Revoked);
        }
        if self.phase != ResourceLeasePhase::Active {
            return Err(ResourceLeaseReason::CleanupRequired);
        }
        Ok(())
    }

    pub fn begin_operation(
        &mut self,
        resource_binding: Id<'_>,
        holder: InstancePath<'_>,
        run: Id<'_>,
        epoch: u64,
        now: AuthorityTime<'_>,
    ) -> Result<u32, ResourceLeaseReason> {
        self.check_use(resource_binding, holder, run, epoch, now)?;
        let next = self
            .operations
            .checked_add(1)
            .ok_or(ResourceLeaseReason::OperationLimit)?;
        if next > self.contract.maximum_operations {
            return Err(ResourceLeaseReason::OperationLimit);
        }
        self.operations = next;
        Ok(next)
    }

    pub fn revoke(
        &mut self,
        now: AuthorityTime<'_>,
    ) -> Result<AuthorityTime<'a>, ResourceLeaseReason> {
        if self.phase != ResourceLeasePhase::Active || now.basis != self.contract.time_basis {
            return Err(ResourceLeaseReason::IllegalTransition);
        }
        let deadline = now
            .tick
            .checked_add(self.contract.revocation_grace_ticks)
            .ok_or(ResourceLeaseReason::InvalidContract)?;
        self.phase = ResourceLeasePhase::Revoked;
        Ok(AuthorityTime {
            basis: self.contract.time_basis,
            tick: deadline,
        })
    }

    pub fn expire(&mut self, now: AuthorityTime<'_>) -> Result<(), ResourceLeaseReason> {
        if self.phase != ResourceLeasePhase::Active
            || now.basis != self.contract.time_basis
            || now.tick < self.contract.expires_at_tick
        {
            return Err(ResourceLeaseReason::IllegalTransition);
        }
        self.phase = ResourceLeasePhase::Expired;
        Ok(())
    }

    pub fn begin_cleanup(
        &mut self,
        now: AuthorityTime<'_>,
    ) -> Result<AuthorityTime<'a>, ResourceLeaseReason> {
        if !matches!(
            self.phase,
            ResourceLeasePhase::Active
                | ResourceLeasePhase::Revoked
                | ResourceLeasePhase::Expired
                | ResourceLeasePhase::Failed
        ) || now.basis != self.contract.time_basis
        {
            return Err(ResourceLeaseReason::IllegalTransition);
        }
        let deadline = now
            .tick
            .checked_add(self.contract.cleanup_ticks)
            .ok_or(ResourceLeaseReason::InvalidContract)?;
        self.phase = ResourceLeasePhase::Cleaning;
        self.cleanup_deadline_tick = Some(deadline);
        Ok(AuthorityTime {
            basis: self.contract.time_basis,
            tick: deadline,
        })
    }

    pub fn complete_cleanup(&mut self, release_sequence: u64) -> Result<(), ResourceLeaseReason> {
        if release_sequence <= self.release_sequence {
            return Err(ResourceLeaseReason::StaleRelease);
        }
        if self.phase != ResourceLeasePhase::Cleaning {
            return Err(ResourceLeaseReason::CleanupRequired);
        }
        self.release_sequence = release_sequence;
        self.cleanup_deadline_tick = None;
        self.phase = ResourceLeasePhase::Released;
        Ok(())
    }

    pub fn enforce_cleanup_deadline(
        &mut self,
        now: AuthorityTime<'_>,
    ) -> Result<(), ResourceLeaseReason> {
        if now.basis != self.contract.time_basis {
            return Err(ResourceLeaseReason::TimeBasisMismatch);
        }
        let Some(deadline) = self.cleanup_deadline_tick else {
            return Err(ResourceLeaseReason::CleanupRequired);
        };
        if now.tick < deadline {
            return Ok(());
        }
        self.phase = ResourceLeasePhase::Failed;
        Err(ResourceLeaseReason::CleanupTimeout)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectAttemptPhase {
    Prepared,
    Running,
    Committed,
    CommitUnknown,
    Acknowledged,
    Cleaning,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationDisposition {
    CancelledBeforeCommit,
    CleanupRequired,
    ReconcileRequired,
}

/// Allocation-free attempt state. Completion is impossible until commit and
/// cleanup/acknowledgement disposition are both explicit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectAttemptState<'a> {
    profile: EffectCommitProfile<'a>,
    phase: EffectAttemptPhase,
    attempt: u16,
    idempotency_key: Option<Id<'a>>,
}

impl<'a> EffectAttemptState<'a> {
    pub fn new(
        profile: EffectCommitProfile<'a>,
        lease: ResourceLeaseContract<'a>,
        attempt: u16,
        idempotency_key: Option<Id<'a>>,
    ) -> Result<Self, ResourceLeaseReason> {
        validate_effect_commit_profile(profile, lease)?;
        if attempt == 0
            || attempt > profile.maximum_attempts
            || idempotency_key.is_some_and(|value| Id::new(value.as_str()).is_err())
            || (profile.idempotency == EffectIdempotency::SameKeySameEffect
                && idempotency_key.is_none())
        {
            return Err(ResourceLeaseReason::InvalidContract);
        }
        Ok(Self {
            profile,
            phase: EffectAttemptPhase::Prepared,
            attempt,
            idempotency_key,
        })
    }

    #[must_use]
    pub const fn phase(&self) -> EffectAttemptPhase {
        self.phase
    }

    #[must_use]
    pub const fn profile(&self) -> EffectCommitProfile<'a> {
        self.profile
    }

    pub fn start(&mut self) -> Result<(), ResourceLeaseReason> {
        if self.phase != EffectAttemptPhase::Prepared {
            return Err(ResourceLeaseReason::IllegalTransition);
        }
        self.phase = EffectAttemptPhase::Running;
        Ok(())
    }

    pub fn committed(&mut self) -> Result<(), ResourceLeaseReason> {
        if self.phase != EffectAttemptPhase::Running {
            return Err(ResourceLeaseReason::IllegalTransition);
        }
        self.phase = EffectAttemptPhase::Committed;
        Ok(())
    }

    /// Record a failure known to precede the provider's commit boundary.
    pub fn fail_before_commit(&mut self) -> Result<(), ResourceLeaseReason> {
        if !matches!(
            self.phase,
            EffectAttemptPhase::Prepared | EffectAttemptPhase::Running
        ) {
            return Err(ResourceLeaseReason::IllegalTransition);
        }
        self.phase = EffectAttemptPhase::Failed;
        Ok(())
    }

    pub fn acknowledge(&mut self) -> Result<(), ResourceLeaseReason> {
        if self.phase != EffectAttemptPhase::Committed {
            return Err(ResourceLeaseReason::IllegalTransition);
        }
        self.phase = EffectAttemptPhase::Acknowledged;
        Ok(())
    }

    pub fn lose_host(&mut self) -> Result<(), ResourceLeaseReason> {
        match (self.phase, self.profile.discontinuity) {
            (EffectAttemptPhase::Prepared, _)
            | (EffectAttemptPhase::Running, EffectDiscontinuity::FailedBeforeCommit) => {
                self.phase = EffectAttemptPhase::Failed;
                Err(ResourceLeaseReason::HostLost)
            }
            (
                EffectAttemptPhase::Running | EffectAttemptPhase::Committed,
                EffectDiscontinuity::CommitUnknown,
            ) => {
                self.phase = EffectAttemptPhase::CommitUnknown;
                Err(ResourceLeaseReason::CommitUnknown)
            }
            (
                EffectAttemptPhase::Running | EffectAttemptPhase::Committed,
                EffectDiscontinuity::ReconcileRequired,
            ) => {
                self.phase = EffectAttemptPhase::CommitUnknown;
                Err(ResourceLeaseReason::CommitUnknown)
            }
            _ => Err(ResourceLeaseReason::IllegalTransition),
        }
    }

    pub fn retry(&mut self, idempotency_key: Option<Id<'a>>) -> Result<u16, ResourceLeaseReason> {
        if self.phase != EffectAttemptPhase::CommitUnknown {
            return Err(ResourceLeaseReason::IllegalTransition);
        }
        match self.profile.unknown_commit {
            UnknownCommitPolicy::Fail | UnknownCommitPolicy::Reconcile => {
                return Err(ResourceLeaseReason::RetryForbidden);
            }
            UnknownCommitPolicy::RetrySameIdempotencyKey
                if idempotency_key != self.idempotency_key =>
            {
                return Err(ResourceLeaseReason::RetryForbidden);
            }
            UnknownCommitPolicy::RetrySameIdempotencyKey => {}
        }
        let next = self
            .attempt
            .checked_add(1)
            .ok_or(ResourceLeaseReason::RetryForbidden)?;
        if next > self.profile.maximum_attempts {
            return Err(ResourceLeaseReason::RetryForbidden);
        }
        self.attempt = next;
        self.phase = EffectAttemptPhase::Prepared;
        Ok(next)
    }

    pub fn cancel(&mut self) -> Result<CancellationDisposition, ResourceLeaseReason> {
        match self.phase {
            EffectAttemptPhase::Prepared => {
                self.phase = EffectAttemptPhase::Cancelled;
                Ok(CancellationDisposition::CancelledBeforeCommit)
            }
            EffectAttemptPhase::Running | EffectAttemptPhase::Committed => {
                self.phase = EffectAttemptPhase::Cleaning;
                Ok(CancellationDisposition::CleanupRequired)
            }
            EffectAttemptPhase::CommitUnknown => Ok(CancellationDisposition::ReconcileRequired),
            _ => Err(ResourceLeaseReason::IllegalTransition),
        }
    }

    pub fn cleanup_complete(&mut self) -> Result<(), ResourceLeaseReason> {
        if self.phase != EffectAttemptPhase::Cleaning {
            return Err(ResourceLeaseReason::CleanupRequired);
        }
        self.phase = EffectAttemptPhase::Cancelled;
        Ok(())
    }

    #[must_use]
    pub fn may_report_success(&self) -> bool {
        self.phase == EffectAttemptPhase::Acknowledged
    }
}

/// Authority material that must be rechecked with the lease at operation use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostOperationAuthorityContext<'a> {
    pub binding: ResolvedAuthorityBinding<'a>,
    pub effect: EffectRequirement<'a>,
    pub capability: HostCapability<'a>,
    pub grant: ObservedGrant<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceLeaseUseContext<'a> {
    pub holder: InstancePath<'a>,
    pub run: Id<'a>,
    pub epoch: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeasedHostOperationError {
    Authority,
    Lease(ResourceLeaseReason),
    Operation(ImplementationError),
}

/// Validate fresh authority, the complete host-operation binding, and the
/// exact run-scoped lease before any provider mutation is invoked.
pub fn validate_leased_host_operation(
    request: HostOperationRequest<'_>,
    context: HostOperationContext<'_>,
    authority: HostOperationAuthorityContext<'_>,
    lease: &ResourceLeaseState<'_>,
    lease_use: ResourceLeaseUseContext<'_>,
    profile: &ExecutionProfile<'_>,
) -> Result<(), LeasedHostOperationError> {
    validate_authority_at_use(
        authority.binding,
        authority.effect,
        context.now,
        authority.capability,
        authority.grant,
    )
    .map_err(|_| LeasedHostOperationError::Authority)?;
    validate_host_operation(request, context, profile)
        .map_err(LeasedHostOperationError::Operation)?;
    lease
        .check_use(
            request.resource_binding,
            lease_use.holder,
            lease_use.run,
            lease_use.epoch,
            context.now,
        )
        .map_err(LeasedHostOperationError::Lease)
}

impl ResourceLeaseContract<'_> {
    pub fn semantic_hash(self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let reservation = budget_fields(self.reservation);
        let (foreign_bytes, foreign_ticks) = match self.foreign_retention {
            ForeignRetention::Bounded {
                maximum_bytes,
                release_ticks,
            } => (maximum_bytes, release_ticks),
            _ => (0, 0),
        };
        CanonicalDescriptor {
            kind: Id("conduit/resource-lease"),
            schema_version: self.schema_version,
            body: CanonicalValue::Map(&[
                field("id", CanonicalValue::Identifier(self.id)),
                field(
                    "resource_binding",
                    CanonicalValue::Identifier(self.resource_binding),
                ),
                field("holder", CanonicalValue::Text(self.holder.as_str())),
                field("run", CanonicalValue::Identifier(self.run)),
                field("epoch", CanonicalValue::Integer(i128::from(self.epoch))),
                field("scope", CanonicalValue::Identifier(self.scope)),
                field("sharing", CanonicalValue::Text(self.sharing.as_str())),
                field(
                    "maximum_holders",
                    CanonicalValue::Integer(i128::from(self.sharing.maximum_holders())),
                ),
                field("reservation", CanonicalValue::Map(&reservation)),
                field("time_basis", CanonicalValue::Identifier(self.time_basis)),
                field(
                    "issued_at_tick",
                    CanonicalValue::Integer(i128::from(self.issued_at_tick)),
                ),
                field(
                    "expires_at_tick",
                    CanonicalValue::Integer(i128::from(self.expires_at_tick)),
                ),
                field(
                    "revocation_grace_ticks",
                    CanonicalValue::Integer(i128::from(self.revocation_grace_ticks)),
                ),
                field(
                    "cleanup_ticks",
                    CanonicalValue::Integer(i128::from(self.cleanup_ticks)),
                ),
                field(
                    "maximum_operations",
                    CanonicalValue::Integer(i128::from(self.maximum_operations)),
                ),
                field(
                    "maximum_evidence_events",
                    CanonicalValue::Integer(i128::from(self.maximum_evidence_events)),
                ),
                field(
                    "cleanup_escalation",
                    CanonicalValue::Bytes(self.cleanup_escalation.semantic_hash.as_bytes()),
                ),
                field(
                    "foreign_retention",
                    CanonicalValue::Text(self.foreign_retention.as_str()),
                ),
                field(
                    "foreign_maximum_bytes",
                    CanonicalValue::Integer(i128::from(foreign_bytes)),
                ),
                field(
                    "foreign_release_ticks",
                    CanonicalValue::Integer(i128::from(foreign_ticks)),
                ),
            ]),
        }
        .semantic_hash()
    }
}

impl EffectCommitProfile<'_> {
    pub fn semantic_hash(self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        CanonicalDescriptor {
            kind: Id("conduit/effect-commit-profile"),
            schema_version: self.schema_version,
            body: CanonicalValue::Map(&[
                field("id", CanonicalValue::Identifier(self.id)),
                field("operation", CanonicalValue::Identifier(self.operation)),
                field(
                    "resource_lease",
                    CanonicalValue::Identifier(self.resource_lease),
                ),
                field(
                    "commit_boundary",
                    CanonicalValue::Bytes(self.commit_boundary.semantic_hash.as_bytes()),
                ),
                field(
                    "idempotency",
                    CanonicalValue::Text(self.idempotency.as_str()),
                ),
                field(
                    "unknown_commit",
                    CanonicalValue::Text(self.unknown_commit.as_str()),
                ),
                field(
                    "discontinuity",
                    CanonicalValue::Text(self.discontinuity.as_str()),
                ),
                field(
                    "cleanup",
                    CanonicalValue::Bytes(self.cleanup.semantic_hash.as_bytes()),
                ),
                field(
                    "maximum_attempts",
                    CanonicalValue::Integer(i128::from(self.maximum_attempts)),
                ),
                field(
                    "evidence_events_per_attempt",
                    CanonicalValue::Integer(i128::from(self.evidence_events_per_attempt)),
                ),
            ]),
        }
        .semantic_hash()
    }
}

fn field<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        disposition: FieldDisposition::Semantic,
        value,
    }
}

fn budget_fields(value: PlanResourceBudget) -> [MapField<'static>; 7] {
    [
        field(
            "memory_bytes",
            CanonicalValue::Integer(i128::from(value.memory_bytes)),
        ),
        field(
            "storage_bytes",
            CanonicalValue::Integer(i128::from(value.storage_bytes)),
        ),
        field(
            "cpu_units",
            CanonicalValue::Integer(i128::from(value.cpu_units)),
        ),
        field("timers", CanonicalValue::Integer(i128::from(value.timers))),
        field(
            "transports",
            CanonicalValue::Integer(i128::from(value.transports)),
        ),
        field(
            "checkpoints",
            CanonicalValue::Integer(i128::from(value.checkpoints)),
        ),
        field(
            "evidence_bytes",
            CanonicalValue::Integer(i128::from(value.evidence_bytes)),
        ),
    ]
}

fn valid_pin(value: PinnedDescriptor<'_>) -> bool {
    Id::new(value.id.as_str()).is_ok() && value.schema_version == 0
}
