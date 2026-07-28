//! Allocator-free lifecycle, cancellation, and terminal resolution contracts.

use core::cmp::Ordering;
use core::fmt;

use crate::Id;

/// Lifecycle subjects that share the managed-node transition vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagedSubject {
    /// One primitive node instance.
    Node,
    /// One exported composite instance.
    Composite,
    /// One execution run.
    Run,
}

/// Exact lifecycle state for nodes, composites, and runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleState {
    Created,
    Preparing,
    Ready,
    Running,
    Draining,
    Succeeded,
    Cancelled,
    Failed,
}

impl LifecycleState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Cancelled | Self::Failed)
    }
}

/// Exact cord lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CordState {
    Created,
    Prepared,
    Open,
    Draining,
    Completed,
    Cancelled,
    Failed,
    Disconnected,
}

impl CordState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Cancelled | Self::Failed | Self::Disconnected
        )
    }
}

/// A lifecycle state encoded without erasing the subject kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubjectState {
    Managed(LifecycleState),
    Cord(CordState),
}

/// Stable terminal and cancellation cause codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalCauseCode {
    NaturalCompletion,
    TransportDisconnected,
    CancellationRequested,
    ParentCancelled,
    DeadlineExpired,
    AuthorityRevoked,
    NodeFailed,
}

impl TerminalCauseCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NaturalCompletion => "natural-completion",
            Self::TransportDisconnected => "transport-disconnected",
            Self::CancellationRequested => "cancellation-requested",
            Self::ParentCancelled => "parent-cancelled",
            Self::DeadlineExpired => "deadline-expired",
            Self::AuthorityRevoked => "authority-revoked",
            Self::NodeFailed => "node-failed",
        }
    }

    const fn precedence(self) -> u8 {
        match self {
            Self::NaturalCompletion => 0,
            Self::TransportDisconnected => 1,
            Self::CancellationRequested | Self::ParentCancelled => 2,
            Self::DeadlineExpired => 3,
            Self::AuthorityRevoked => 4,
            Self::NodeFailed => 5,
        }
    }
}

/// Whether accepted queued values drain or are explicitly discarded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopPolicy {
    Drain,
    Abort,
}

/// Stable reference to another retained cause.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CauseRef<'a> {
    pub code: TerminalCauseCode,
    pub subject: Id<'a>,
}

/// One immutable terminal cause with its semantic subject and causal parent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalCause<'a> {
    pub code: TerminalCauseCode,
    pub subject: Id<'a>,
    pub caused_by: Option<CauseRef<'a>>,
    pub stop: StopPolicy,
}

/// Deterministic terminal classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalClass {
    Succeeded,
    Disconnected,
    Cancelled,
    Failed,
}

/// Result of resolving simultaneous terminal causes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalResolution<'a> {
    pub class: TerminalClass,
    pub primary: TerminalCause<'a>,
    pub queue: StopPolicy,
    pub retained_causes: usize,
}

/// Resolve a race and copy the complete cause set into caller-owned storage.
///
/// Causes are retained in precedence-descending order, then by stable code and
/// subject. Input order therefore cannot affect the result.
pub fn resolve_terminal<'a>(
    causes: &[TerminalCause<'a>],
    retained: &mut [Option<TerminalCause<'a>>],
) -> Result<TerminalResolution<'a>, LifecycleError> {
    if causes.is_empty() {
        return Err(LifecycleError::NoTerminalCause);
    }
    if retained.len() < causes.len() {
        return Err(LifecycleError::EvidenceStorageTooSmall);
    }
    for slot in &mut retained[..causes.len()] {
        *slot = None;
    }
    for (count, cause) in causes.iter().copied().enumerate() {
        let mut insertion = count;
        while insertion > 0 {
            let prior = retained[insertion - 1].expect("sorted prefix");
            if cause_order(cause, prior) != Ordering::Less {
                break;
            }
            retained[insertion] = Some(prior);
            insertion -= 1;
        }
        retained[insertion] = Some(cause);
    }
    let primary = retained[0].expect("non-empty cause set");
    let class = match primary.code {
        TerminalCauseCode::NaturalCompletion => TerminalClass::Succeeded,
        TerminalCauseCode::TransportDisconnected => TerminalClass::Disconnected,
        TerminalCauseCode::CancellationRequested
        | TerminalCauseCode::ParentCancelled
        | TerminalCauseCode::DeadlineExpired
        | TerminalCauseCode::AuthorityRevoked => TerminalClass::Cancelled,
        TerminalCauseCode::NodeFailed => TerminalClass::Failed,
    };
    let queue = if primary.code == TerminalCauseCode::NaturalCompletion {
        StopPolicy::Drain
    } else {
        primary.stop
    };
    Ok(TerminalResolution {
        class,
        primary,
        queue,
        retained_causes: causes.len(),
    })
}

fn cause_order(left: TerminalCause<'_>, right: TerminalCause<'_>) -> Ordering {
    right
        .code
        .precedence()
        .cmp(&left.code.precedence())
        .then_with(|| left.code.as_str().cmp(right.code.as_str()))
        .then_with(|| left.subject.as_str().cmp(right.subject.as_str()))
}

/// Immutable lifecycle transition evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleEvent<'a> {
    pub sequence: u64,
    pub subject: Id<'a>,
    pub from: SubjectState,
    pub to: SubjectState,
    pub cause: Option<TerminalCauseCode>,
}

/// One primitive/composite/run state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleMachine<'a> {
    subject: Id<'a>,
    kind: ManagedSubject,
    state: LifecycleState,
    next_sequence: u64,
}

impl<'a> LifecycleMachine<'a> {
    #[must_use]
    pub const fn new(subject: Id<'a>, kind: ManagedSubject) -> Self {
        Self {
            subject,
            kind,
            state: LifecycleState::Created,
            next_sequence: 0,
        }
    }

    #[must_use]
    pub const fn state(self) -> LifecycleState {
        self.state
    }

    pub fn transition(
        &mut self,
        to: LifecycleState,
        cause: Option<TerminalCauseCode>,
    ) -> Result<LifecycleEvent<'a>, LifecycleError> {
        let next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(LifecycleError::SequenceExhausted)?;
        if !managed_transition_allowed(self.state, to) {
            return Err(LifecycleError::IllegalTransition);
        }
        if !managed_cause_matches(to, cause) {
            return Err(LifecycleError::InvalidTerminalCause);
        }
        let event = LifecycleEvent {
            sequence: self.next_sequence,
            subject: self.subject,
            from: SubjectState::Managed(self.state),
            to: SubjectState::Managed(to),
            cause,
        };
        self.state = to;
        self.next_sequence = next_sequence;
        Ok(event)
    }

    #[must_use]
    pub const fn kind(self) -> ManagedSubject {
        self.kind
    }
}

/// One cord state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CordLifecycle<'a> {
    subject: Id<'a>,
    state: CordState,
    next_sequence: u64,
}

impl<'a> CordLifecycle<'a> {
    #[must_use]
    pub const fn new(subject: Id<'a>) -> Self {
        Self {
            subject,
            state: CordState::Created,
            next_sequence: 0,
        }
    }

    #[must_use]
    pub const fn state(self) -> CordState {
        self.state
    }

    pub fn transition(
        &mut self,
        to: CordState,
        cause: Option<TerminalCauseCode>,
    ) -> Result<LifecycleEvent<'a>, LifecycleError> {
        let next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(LifecycleError::SequenceExhausted)?;
        if !cord_transition_allowed(self.state, to) {
            return Err(LifecycleError::IllegalTransition);
        }
        if !cord_cause_matches(to, cause) {
            return Err(LifecycleError::InvalidTerminalCause);
        }
        let event = LifecycleEvent {
            sequence: self.next_sequence,
            subject: self.subject,
            from: SubjectState::Cord(self.state),
            to: SubjectState::Cord(to),
            cause,
        };
        self.state = to;
        self.next_sequence = next_sequence;
        Ok(event)
    }
}

#[must_use]
pub const fn managed_transition_allowed(from: LifecycleState, to: LifecycleState) -> bool {
    matches!(
        (from, to),
        (LifecycleState::Created, LifecycleState::Preparing)
            | (LifecycleState::Preparing, LifecycleState::Ready)
            | (LifecycleState::Ready, LifecycleState::Running)
            | (LifecycleState::Running, LifecycleState::Draining)
            | (LifecycleState::Draining, LifecycleState::Succeeded)
    ) || (!from.is_terminal() && matches!(to, LifecycleState::Cancelled | LifecycleState::Failed))
}

#[must_use]
pub const fn cord_transition_allowed(from: CordState, to: CordState) -> bool {
    matches!(
        (from, to),
        (CordState::Created, CordState::Prepared)
            | (CordState::Prepared, CordState::Open)
            | (CordState::Open, CordState::Draining)
            | (CordState::Draining, CordState::Completed)
    ) || (!from.is_terminal()
        && matches!(
            to,
            CordState::Cancelled | CordState::Failed | CordState::Disconnected
        ))
}

const fn managed_cause_matches(state: LifecycleState, cause: Option<TerminalCauseCode>) -> bool {
    match state {
        LifecycleState::Cancelled => matches!(
            cause,
            Some(
                TerminalCauseCode::CancellationRequested
                    | TerminalCauseCode::ParentCancelled
                    | TerminalCauseCode::DeadlineExpired
                    | TerminalCauseCode::AuthorityRevoked
            )
        ),
        LifecycleState::Failed => matches!(cause, Some(TerminalCauseCode::NodeFailed)),
        _ => cause.is_none(),
    }
}

const fn cord_cause_matches(state: CordState, cause: Option<TerminalCauseCode>) -> bool {
    match state {
        CordState::Cancelled => matches!(
            cause,
            Some(
                TerminalCauseCode::CancellationRequested
                    | TerminalCauseCode::ParentCancelled
                    | TerminalCauseCode::DeadlineExpired
                    | TerminalCauseCode::AuthorityRevoked
            )
        ),
        CordState::Failed => matches!(cause, Some(TerminalCauseCode::NodeFailed)),
        CordState::Disconnected => {
            matches!(cause, Some(TerminalCauseCode::TransportDisconnected))
        }
        _ => cause.is_none(),
    }
}

/// Derive a composite state from its children and boundary cords.
///
/// Failure wins cancellation; cancellation wins activity; successful
/// completion requires every child and boundary cord to complete.
#[must_use]
pub fn derive_composite(children: &[LifecycleState], boundaries: &[CordState]) -> LifecycleState {
    if children.is_empty() && boundaries.is_empty() {
        return LifecycleState::Created;
    }
    if children.contains(&LifecycleState::Failed) || boundaries.contains(&CordState::Failed) {
        return LifecycleState::Failed;
    }
    if children.contains(&LifecycleState::Cancelled)
        || boundaries.contains(&CordState::Cancelled)
        || boundaries.contains(&CordState::Disconnected)
    {
        return LifecycleState::Cancelled;
    }
    if children
        .iter()
        .all(|state| *state == LifecycleState::Succeeded)
        && boundaries
            .iter()
            .all(|state| *state == CordState::Completed)
    {
        return LifecycleState::Succeeded;
    }
    if children.contains(&LifecycleState::Draining) || boundaries.contains(&CordState::Draining) {
        return LifecycleState::Draining;
    }
    if children.contains(&LifecycleState::Running) || boundaries.contains(&CordState::Open) {
        return LifecycleState::Running;
    }
    if children.iter().all(|state| *state == LifecycleState::Ready)
        && boundaries.iter().all(|state| *state == CordState::Prepared)
    {
        return LifecycleState::Ready;
    }
    if children.contains(&LifecycleState::Preparing) {
        LifecycleState::Preparing
    } else {
        LifecycleState::Created
    }
}

/// One cancellation scope's finite propagation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CancellationScope<'a> {
    pub id: Id<'a>,
    pub parent: Option<Id<'a>>,
    pub deadline_ticks: u64,
    pub stop: StopPolicy,
}

/// One registered owned resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CancellationRegistration<'a> {
    pub resource: Id<'a>,
    pub scope: CancellationScope<'a>,
    cancelled: bool,
}

impl<'a> CancellationRegistration<'a> {
    #[must_use]
    pub const fn new(resource: Id<'a>, scope: CancellationScope<'a>) -> Self {
        Self {
            resource,
            scope,
            cancelled: false,
        }
    }
}

/// Exact delivery produced by hierarchical cancellation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CancellationDelivery<'a> {
    pub sequence: u16,
    pub resource: Id<'a>,
    pub scope: Id<'a>,
    pub caused_by_scope: Id<'a>,
    pub reason: TerminalCauseCode,
    pub caused_by_reason: Option<TerminalCauseCode>,
    pub deadline_tick: u64,
    pub stop: StopPolicy,
}

/// Idempotent cancellation result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationOutcome {
    Delivered(usize),
    Repeated,
}

/// Propagate cancellation to a scope and every descendant in registration order.
pub fn cancel_scope<'a>(
    registrations: &mut [CancellationRegistration<'a>],
    scope: Id<'a>,
    reason: TerminalCauseCode,
    now_tick: u64,
    deliveries: &mut [Option<CancellationDelivery<'a>>],
) -> Result<CancellationOutcome, LifecycleError> {
    if !matches!(
        reason,
        TerminalCauseCode::CancellationRequested
            | TerminalCauseCode::DeadlineExpired
            | TerminalCauseCode::AuthorityRevoked
            | TerminalCauseCode::NodeFailed
    ) {
        return Err(LifecycleError::InvalidCancellationReason);
    }
    if !registrations.iter().any(|entry| entry.scope.id == scope) {
        return Err(LifecycleError::UnknownCancellationScope);
    }
    validate_scope_tree(registrations)?;

    let needed = registrations
        .iter()
        .filter(|entry| !entry.cancelled && is_descendant(entry.scope.id, scope, registrations))
        .count();
    if needed == 0 {
        return Ok(CancellationOutcome::Repeated);
    }
    if deliveries.len() < needed {
        return Err(LifecycleError::EvidenceStorageTooSmall);
    }
    if needed > usize::from(u16::MAX) {
        return Err(LifecycleError::EvidenceStorageTooSmall);
    }
    for entry in registrations
        .iter()
        .filter(|entry| !entry.cancelled && is_descendant(entry.scope.id, scope, registrations))
    {
        now_tick
            .checked_add(entry.scope.deadline_ticks)
            .ok_or(LifecycleError::DeadlineOverflow)?;
    }
    for slot in &mut deliveries[..needed] {
        *slot = None;
    }
    let mut written = 0;
    for index in 0..registrations.len() {
        let entry = registrations[index];
        if entry.cancelled || !is_descendant(entry.scope.id, scope, registrations) {
            continue;
        }
        let deadline_tick = now_tick
            .checked_add(entry.scope.deadline_ticks)
            .ok_or(LifecycleError::DeadlineOverflow)?;
        deliveries[written] = Some(CancellationDelivery {
            sequence: written as u16,
            resource: entry.resource,
            scope: entry.scope.id,
            caused_by_scope: scope,
            reason: if entry.scope.id == scope {
                reason
            } else {
                TerminalCauseCode::ParentCancelled
            },
            caused_by_reason: if entry.scope.id == scope {
                None
            } else {
                Some(reason)
            },
            deadline_tick,
            stop: entry.scope.stop,
        });
        registrations[index].cancelled = true;
        written += 1;
    }
    Ok(CancellationOutcome::Delivered(written))
}

fn validate_scope_tree(
    registrations: &[CancellationRegistration<'_>],
) -> Result<(), LifecycleError> {
    for entry in registrations {
        if entry.scope.deadline_ticks == 0 {
            return Err(LifecycleError::UnboundedCancellation);
        }
        if let Some(parent) = entry.scope.parent {
            if !registrations
                .iter()
                .any(|candidate| candidate.scope.id == parent)
            {
                return Err(LifecycleError::UnknownCancellationParent);
            }
            let mut cursor = Some(parent);
            for _ in 0..registrations.len() {
                let Some(id) = cursor else { break };
                if id == entry.scope.id {
                    return Err(LifecycleError::CancellationCycle);
                }
                cursor = registrations
                    .iter()
                    .find(|candidate| candidate.scope.id == id)
                    .and_then(|candidate| candidate.scope.parent);
            }
        }
        if registrations.iter().any(|candidate| {
            candidate.scope.id == entry.scope.id
                && (candidate.scope.parent != entry.scope.parent
                    || candidate.scope.deadline_ticks != entry.scope.deadline_ticks
                    || candidate.scope.stop != entry.scope.stop)
        }) {
            return Err(LifecycleError::ConflictingCancellationScope);
        }
        if registrations
            .iter()
            .filter(|candidate| candidate.resource == entry.resource)
            .count()
            != 1
        {
            return Err(LifecycleError::DuplicateCancellationResource);
        }
    }
    Ok(())
}

fn is_descendant(
    candidate: Id<'_>,
    ancestor: Id<'_>,
    registrations: &[CancellationRegistration<'_>],
) -> bool {
    let mut cursor = Some(candidate);
    for _ in 0..=registrations.len() {
        let Some(id) = cursor else { return false };
        if id == ancestor {
            return true;
        }
        cursor = registrations
            .iter()
            .find(|entry| entry.scope.id == id)
            .and_then(|entry| entry.scope.parent);
    }
    false
}

/// Replicated child attempt lifecycle required by bounded composite pools.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplicaState {
    Template,
    QueuedAdmission,
    AdmittedInstance,
    Attempt,
    Draining,
    Cleanup,
    Succeeded,
    Cancelled,
    Failed,
}

#[must_use]
pub const fn replica_transition_allowed(from: ReplicaState, to: ReplicaState) -> bool {
    matches!(
        (from, to),
        (ReplicaState::Template, ReplicaState::QueuedAdmission)
            | (
                ReplicaState::QueuedAdmission,
                ReplicaState::AdmittedInstance
            )
            | (ReplicaState::AdmittedInstance, ReplicaState::Attempt)
            | (ReplicaState::Attempt, ReplicaState::Draining)
            | (ReplicaState::Draining, ReplicaState::Cleanup)
            | (ReplicaState::Cleanup, ReplicaState::Succeeded)
            | (ReplicaState::Cleanup, ReplicaState::Attempt)
    ) || (!matches!(
        from,
        ReplicaState::Succeeded | ReplicaState::Cancelled | ReplicaState::Failed
    ) && matches!(to, ReplicaState::Cancelled | ReplicaState::Failed))
}

/// Stable identity for one replicated child attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplicaIdentity<'a> {
    pub template: Id<'a>,
    pub instance: u16,
    pub attempt: u16,
}

impl<'a> ReplicaIdentity<'a> {
    /// Creates a one-based attempt identity.
    pub const fn new(
        template: Id<'a>,
        instance: u16,
        attempt: u16,
    ) -> Result<Self, LifecycleError> {
        if attempt == 0 {
            return Err(LifecycleError::InvalidReplicaIdentity);
        }
        Ok(Self {
            template,
            instance,
            attempt,
        })
    }

    /// Creates the next attempt without exceeding the exact restart budget.
    pub const fn restart(self, pool: ReplicaPoolContract<'a>) -> Result<Self, LifecycleError> {
        let SupervisionPolicy::Restart { max_attempts, .. } = pool.supervision else {
            return Err(LifecycleError::RestartNotAllowed);
        };
        if self.attempt >= max_attempts {
            return Err(LifecycleError::RestartBudgetExhausted);
        }
        Ok(Self {
            template: self.template,
            instance: self.instance,
            attempt: self.attempt + 1,
        })
    }
}

/// Exact bounded supervision strategy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisionPolicy<'a> {
    FailTogether,
    Isolate,
    Restart {
        max_attempts: u16,
        backoff_ticks: u64,
    },
    Fallback {
        node: Id<'a>,
    },
    Drain,
    Abort,
    Escalate,
}

/// Finite replicated-composite pool contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplicaPoolContract<'a> {
    pub max_queued: u16,
    pub max_active: u16,
    pub supervision: SupervisionPolicy<'a>,
}

impl ReplicaPoolContract<'_> {
    pub const fn validate(self) -> Result<(), LifecycleError> {
        if self.max_queued == 0 || self.max_active == 0 {
            return Err(LifecycleError::UnboundedReplicaPool);
        }
        if let SupervisionPolicy::Restart {
            max_attempts,
            backoff_ticks,
        } = self.supervision
        {
            if max_attempts == 0 || backoff_ticks == 0 {
                return Err(LifecycleError::UnboundedRestart);
            }
        }
        Ok(())
    }
}

/// Stable lifecycle rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleError {
    IllegalTransition,
    InvalidTerminalCause,
    NoTerminalCause,
    EvidenceStorageTooSmall,
    SequenceExhausted,
    InvalidCancellationReason,
    UnknownCancellationScope,
    UnknownCancellationParent,
    CancellationCycle,
    ConflictingCancellationScope,
    DuplicateCancellationResource,
    UnboundedCancellation,
    DeadlineOverflow,
    UnboundedReplicaPool,
    UnboundedRestart,
    InvalidReplicaIdentity,
    RestartNotAllowed,
    RestartBudgetExhausted,
}

impl LifecycleError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::IllegalTransition => "CND-LIF-001",
            Self::InvalidTerminalCause | Self::NoTerminalCause => "CND-LIF-002",
            Self::EvidenceStorageTooSmall | Self::SequenceExhausted => "CND-LIF-003",
            Self::InvalidCancellationReason
            | Self::UnknownCancellationScope
            | Self::UnknownCancellationParent
            | Self::CancellationCycle
            | Self::ConflictingCancellationScope
            | Self::DuplicateCancellationResource
            | Self::UnboundedCancellation
            | Self::DeadlineOverflow => "CND-CAN-001",
            Self::UnboundedReplicaPool
            | Self::UnboundedRestart
            | Self::InvalidReplicaIdentity
            | Self::RestartNotAllowed
            | Self::RestartBudgetExhausted => "CND-LIF-004",
        }
    }
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::IllegalTransition => "illegal lifecycle transition",
            Self::InvalidTerminalCause => "terminal cause does not match target state",
            Self::NoTerminalCause => "terminal resolution requires at least one cause",
            Self::EvidenceStorageTooSmall => "caller evidence storage is too small",
            Self::SequenceExhausted => "lifecycle evidence sequence is exhausted",
            Self::InvalidCancellationReason => "cause cannot initiate cancellation",
            Self::UnknownCancellationScope => "cancellation scope is not registered",
            Self::UnknownCancellationParent => "cancellation parent is not registered",
            Self::CancellationCycle => "cancellation scope hierarchy contains a cycle",
            Self::ConflictingCancellationScope => {
                "registrations disagree about one cancellation scope"
            }
            Self::DuplicateCancellationResource => "an owned resource is registered more than once",
            Self::UnboundedCancellation => "cancellation deadline must be positive and finite",
            Self::DeadlineOverflow => "cancellation deadline overflowed the deterministic clock",
            Self::UnboundedReplicaPool => "replica admission and activity bounds must be positive",
            Self::UnboundedRestart => "restart attempts and backoff must be positive and finite",
            Self::InvalidReplicaIdentity => "replica attempt identities are one-based",
            Self::RestartNotAllowed => "replica supervision does not permit restart",
            Self::RestartBudgetExhausted => "replica restart budget is exhausted",
        })
    }
}
