//! Bounded correlated request/reply and cancellable-action contracts.
//!
//! This module is intentionally above `conduit-core`. It composes the core's
//! typed descriptors, identities, bounded work, and evidence vocabulary into
//! reusable interaction shapes without adding a callback runtime, transport,
//! RPC privilege path, or universal source-language `Action<T>` value.

use conduit_core::{DescriptorRef, Id, SemanticHash, TypeContractRef};

/// Current pre-release descriptor form.
pub const CONTROL_CONTRACT_SCHEMA_VERSION: u32 = 0;

/// Portable ceilings for the allocation-free reference composites.
pub const MAXIMUM_REFERENCE_EXCHANGES: usize = 16;
pub const MAXIMUM_REFERENCE_GOALS: usize = 32;
pub const MAXIMUM_REFERENCE_FEEDBACK_ITEMS_PER_GOAL: usize = 32;
pub const MAXIMUM_REFERENCE_EVIDENCE: usize = 256;

/// Semantic shape published by the bounded-control composite catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlCompositeKind {
    RequestReply,
    CancellableAction,
}

/// One independently specialized domain type at a composite boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlTypeParameter {
    pub id: Id<'static>,
    pub role: Id<'static>,
}

/// Classification of one exact-plan-visible control field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlPlanFieldKind {
    Type,
    Descriptor,
    Limit,
    Policy,
}

/// One field which a resolver must specialize and retain in the exact plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlPlanField {
    pub id: Id<'static>,
    pub kind: ControlPlanFieldKind,
}

/// Language-neutral semantic-composite definition. This is catalog meaning,
/// not an implementation, provider observation, source-language future, or
/// transport protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlCatalogEntry {
    pub id: Id<'static>,
    pub schema_version: u32,
    pub kind: ControlCompositeKind,
    pub type_parameters: &'static [ControlTypeParameter],
    pub plan_fields: &'static [ControlPlanField],
}

const REQUEST_REPLY_TYPE_PARAMETERS: &[ControlTypeParameter] = &[
    ControlTypeParameter {
        id: Id("request"),
        role: Id("domain-request"),
    },
    ControlTypeParameter {
        id: Id("reply"),
        role: Id("domain-reply"),
    },
    ControlTypeParameter {
        id: Id("domain-error"),
        role: Id("domain-failure"),
    },
];

const ACTION_TYPE_PARAMETERS: &[ControlTypeParameter] = &[
    ControlTypeParameter {
        id: Id("goal"),
        role: Id("domain-goal"),
    },
    ControlTypeParameter {
        id: Id("feedback"),
        role: Id("non-authoritative-feedback"),
    },
    ControlTypeParameter {
        id: Id("result"),
        role: Id("domain-result"),
    },
    ControlTypeParameter {
        id: Id("domain-failure"),
        role: Id("domain-failure"),
    },
];

macro_rules! control_field {
    ($id:literal, $kind:ident) => {
        ControlPlanField {
            id: Id($id),
            kind: ControlPlanFieldKind::$kind,
        }
    };
}

const REQUEST_REPLY_PLAN_FIELDS: &[ControlPlanField] = &[
    control_field!("request-type", Type),
    control_field!("reply-type", Type),
    control_field!("domain-error-type", Type),
    control_field!("clock", Descriptor),
    control_field!("correlation", Descriptor),
    control_field!("cancellation", Descriptor),
    control_field!("idempotency", Descriptor),
    control_field!("maximum-in-flight", Limit),
    control_field!("maximum-request-bytes", Limit),
    control_field!("maximum-reply-bytes", Limit),
    control_field!("maximum-domain-error-bytes", Limit),
    control_field!("maximum-deadline-ticks", Limit),
    control_field!("maximum-retries", Limit),
    control_field!("maximum-replay-outcomes", Limit),
    control_field!("maximum-timers", Limit),
    control_field!("maximum-evidence-events", Limit),
    control_field!("maximum-work-per-step", Limit),
];

const ACTION_PLAN_FIELDS: &[ControlPlanField] = &[
    control_field!("goal-type", Type),
    control_field!("feedback-type", Type),
    control_field!("result-type", Type),
    control_field!("domain-failure-type", Type),
    control_field!("clock", Descriptor),
    control_field!("correlation", Descriptor),
    control_field!("idempotency", Descriptor),
    control_field!("cancellation", Descriptor),
    control_field!("admission-authority", Descriptor),
    control_field!("workload-admission", Descriptor),
    control_field!("placement", Descriptor),
    control_field!("resource-commit-cleanup", Descriptor),
    control_field!("transition", Descriptor),
    control_field!("inhibit", Descriptor),
    control_field!("checkpoint", Descriptor),
    control_field!("feedback-pressure", Policy),
    control_field!("transition-policy", Policy),
    control_field!("maximum-concurrent-goals", Limit),
    control_field!("maximum-queued-admissions", Limit),
    control_field!("maximum-goal-bytes", Limit),
    control_field!("maximum-result-bytes", Limit),
    control_field!("maximum-domain-failure-bytes", Limit),
    control_field!("maximum-feedback-items-per-goal", Limit),
    control_field!("maximum-feedback-bytes-per-goal", Limit),
    control_field!("maximum-replay-outcomes", Limit),
    control_field!("maximum-deadline-ticks", Limit),
    control_field!("maximum-retries-per-goal", Limit),
    control_field!("maximum-cancellations", Limit),
    control_field!("maximum-timers", Limit),
    control_field!("maximum-evidence-events", Limit),
    control_field!("maximum-work-per-step", Limit),
];

/// The single current pre-release bounded-control composite catalog.
pub static STANDARD_CONTROL_CATALOG: &[ControlCatalogEntry] = &[
    ControlCatalogEntry {
        id: Id("conduit.std/control/request-reply"),
        schema_version: CONTROL_CONTRACT_SCHEMA_VERSION,
        kind: ControlCompositeKind::RequestReply,
        type_parameters: REQUEST_REPLY_TYPE_PARAMETERS,
        plan_fields: REQUEST_REPLY_PLAN_FIELDS,
    },
    ControlCatalogEntry {
        id: Id("conduit.std/control/cancellable-action"),
        schema_version: CONTROL_CONTRACT_SCHEMA_VERSION,
        kind: ControlCompositeKind::CancellableAction,
        type_parameters: ACTION_TYPE_PARAMETERS,
        plan_fields: ACTION_PLAN_FIELDS,
    },
];

/// Looks up one exact bounded-control semantic composite without allocation.
#[must_use]
pub fn control_composite_contract(id: &str) -> Option<&'static ControlCatalogEntry> {
    STANDARD_CONTROL_CATALOG
        .iter()
        .find(|entry| entry.id.as_str() == id)
}

/// One end-to-end identity family. Retries replace only `attempt`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlIdentity {
    pub subject: u64,
    pub attempt: u32,
    pub correlation: u64,
    pub idempotency: u64,
}

impl ControlIdentity {
    fn valid(self) -> bool {
        self.subject != 0 && self.attempt != 0 && self.correlation != 0 && self.idempotency != 0
    }
}

/// Every retained request/reply resource, including work and correlation
/// state, is finite and therefore suitable for exact-plan projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestReplyLimits {
    pub maximum_in_flight: u16,
    pub maximum_request_bytes: u32,
    pub maximum_reply_bytes: u32,
    pub maximum_domain_error_bytes: u32,
    pub maximum_deadline_ticks: u64,
    pub maximum_retries: u16,
    pub maximum_replay_outcomes: u16,
    pub maximum_timers: u16,
    pub maximum_evidence_events: u16,
    pub maximum_work_per_step: u16,
}

/// A concrete domain-typed request/reply specialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestReplyContract<'a> {
    pub id: Id<'a>,
    pub schema_version: u32,
    pub request: TypeContractRef<'a>,
    pub reply: TypeContractRef<'a>,
    pub domain_error: TypeContractRef<'a>,
    pub limits: RequestReplyLimits,
    pub clock: DescriptorRef<'a>,
    pub correlation: DescriptorRef<'a>,
    pub cancellation: DescriptorRef<'a>,
    pub idempotency: DescriptorRef<'a>,
}

/// Explicit pressure policy for non-authoritative feedback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeedbackPressurePolicy {
    BlockProducer,
    DropOldest,
    CoalesceLatest,
}

/// Exact action behavior when its provider or active plan changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionPolicy {
    TerminalFailure,
    ExplicitDiscontinuity,
    CompatibleCheckpointHandoff,
}

/// Every action queue, timer, retry, cancellation, retained feedback item,
/// replay outcome, evidence event, and unit of step work is finite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionLimits {
    pub maximum_concurrent_goals: u16,
    pub maximum_queued_admissions: u16,
    pub maximum_goal_bytes: u32,
    pub maximum_result_bytes: u32,
    pub maximum_domain_failure_bytes: u32,
    pub maximum_feedback_items_per_goal: u16,
    pub maximum_feedback_bytes_per_goal: u32,
    pub maximum_replay_outcomes: u16,
    pub maximum_deadline_ticks: u64,
    pub maximum_retries_per_goal: u16,
    pub maximum_cancellations: u16,
    pub maximum_timers: u16,
    pub maximum_evidence_events: u16,
    pub maximum_work_per_step: u16,
}

/// A concrete domain-typed action shape.
///
/// The descriptor references admission, placement, resource/commit/cleanup,
/// transition, and optional inhibit/checkpoint contracts. A goal submission
/// carries none of those authorities by itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionContract<'a> {
    pub id: Id<'a>,
    pub schema_version: u32,
    pub goal: TypeContractRef<'a>,
    pub feedback: TypeContractRef<'a>,
    pub result: TypeContractRef<'a>,
    pub domain_failure: TypeContractRef<'a>,
    pub limits: ActionLimits,
    pub feedback_pressure: FeedbackPressurePolicy,
    pub transition_policy: TransitionPolicy,
    pub clock: DescriptorRef<'a>,
    pub correlation: DescriptorRef<'a>,
    pub idempotency: DescriptorRef<'a>,
    pub cancellation: DescriptorRef<'a>,
    pub admission_authority: DescriptorRef<'a>,
    pub workload_admission: DescriptorRef<'a>,
    pub placement: DescriptorRef<'a>,
    pub resource_commit_cleanup: DescriptorRef<'a>,
    pub transition: DescriptorRef<'a>,
    pub inhibit: Option<DescriptorRef<'a>>,
    pub checkpoint: Option<DescriptorRef<'a>>,
}

/// Contract validation or state-transition failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlError {
    UnsupportedSchema,
    InvalidIdentity,
    InvalidDescriptor,
    Unbounded,
    ReferenceCapacityExceeded,
    CorrelationConflict,
    IllegalState,
    RequestTooLarge,
    ReplyTooLarge,
    FeedbackTooLarge,
    DeadlineInvalid,
    RetryExhausted,
    CancellationExhausted,
    EvidenceExhausted,
    HandoffUnsupported,
    ResultTooLarge,
    DomainFailureTooLarge,
}

impl ControlError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedSchema => "CND-CTL-001",
            Self::InvalidIdentity => "CND-CTL-002",
            Self::InvalidDescriptor => "CND-CTL-003",
            Self::Unbounded => "CND-CTL-004",
            Self::ReferenceCapacityExceeded => "CND-CTL-005",
            Self::CorrelationConflict => "CND-CTL-006",
            Self::IllegalState => "CND-CTL-007",
            Self::RequestTooLarge => "CND-CTL-008",
            Self::ReplyTooLarge => "CND-CTL-009",
            Self::FeedbackTooLarge => "CND-CTL-010",
            Self::DeadlineInvalid => "CND-CTL-011",
            Self::RetryExhausted => "CND-CTL-012",
            Self::CancellationExhausted => "CND-CTL-013",
            Self::EvidenceExhausted => "CND-CTL-014",
            Self::HandoffUnsupported => "CND-CTL-015",
            Self::ResultTooLarge => "CND-CTL-016",
            Self::DomainFailureTooLarge => "CND-CTL-017",
        }
    }
}

fn valid_descriptor(value: DescriptorRef<'_>) -> bool {
    !value.kind.as_str().is_empty() && value.semantic_hash != SemanticHash::from_bytes([0; 32])
}

fn valid_type(value: TypeContractRef<'_>) -> bool {
    !value.contract_id.as_str().is_empty()
        && value.semantic_hash != SemanticHash::from_bytes([0; 32])
}

/// Validates a request/reply descriptor without allocation or discovery.
pub fn validate_request_reply_contract(
    contract: &RequestReplyContract<'_>,
) -> Result<(), ControlError> {
    if contract.schema_version != CONTROL_CONTRACT_SCHEMA_VERSION {
        return Err(ControlError::UnsupportedSchema);
    }
    if Id::new(contract.id.as_str()).is_err() {
        return Err(ControlError::InvalidIdentity);
    }
    if !valid_type(contract.request)
        || !valid_type(contract.reply)
        || !valid_type(contract.domain_error)
        || !valid_descriptor(contract.clock)
        || !valid_descriptor(contract.correlation)
        || !valid_descriptor(contract.cancellation)
        || !valid_descriptor(contract.idempotency)
    {
        return Err(ControlError::InvalidDescriptor);
    }
    let limits = contract.limits;
    if limits.maximum_in_flight == 0
        || limits.maximum_request_bytes == 0
        || limits.maximum_reply_bytes == 0
        || limits.maximum_domain_error_bytes == 0
        || limits.maximum_deadline_ticks == 0
        || limits.maximum_replay_outcomes == 0
        || limits.maximum_timers == 0
        || limits.maximum_evidence_events == 0
        || limits.maximum_work_per_step == 0
    {
        return Err(ControlError::Unbounded);
    }
    if usize::from(limits.maximum_in_flight) + usize::from(limits.maximum_replay_outcomes)
        > MAXIMUM_REFERENCE_EXCHANGES
        || usize::from(limits.maximum_evidence_events) > MAXIMUM_REFERENCE_EVIDENCE
    {
        return Err(ControlError::ReferenceCapacityExceeded);
    }
    if limits.maximum_timers < limits.maximum_in_flight {
        return Err(ControlError::Unbounded);
    }
    Ok(())
}

/// Validates an action descriptor without allocating, resolving a provider,
/// or interpreting a goal as authority.
pub fn validate_action_contract(contract: &ActionContract<'_>) -> Result<(), ControlError> {
    if contract.schema_version != CONTROL_CONTRACT_SCHEMA_VERSION {
        return Err(ControlError::UnsupportedSchema);
    }
    if Id::new(contract.id.as_str()).is_err() {
        return Err(ControlError::InvalidIdentity);
    }
    if !valid_type(contract.goal)
        || !valid_type(contract.feedback)
        || !valid_type(contract.result)
        || !valid_type(contract.domain_failure)
        || !valid_descriptor(contract.clock)
        || !valid_descriptor(contract.correlation)
        || !valid_descriptor(contract.idempotency)
        || !valid_descriptor(contract.cancellation)
        || !valid_descriptor(contract.admission_authority)
        || !valid_descriptor(contract.workload_admission)
        || !valid_descriptor(contract.placement)
        || !valid_descriptor(contract.resource_commit_cleanup)
        || !valid_descriptor(contract.transition)
        || contract
            .inhibit
            .is_some_and(|value| !valid_descriptor(value))
        || contract
            .checkpoint
            .is_some_and(|value| !valid_descriptor(value))
    {
        return Err(ControlError::InvalidDescriptor);
    }
    let limits = contract.limits;
    if limits.maximum_concurrent_goals == 0
        || limits.maximum_queued_admissions == 0
        || limits.maximum_goal_bytes == 0
        || limits.maximum_result_bytes == 0
        || limits.maximum_domain_failure_bytes == 0
        || limits.maximum_feedback_items_per_goal == 0
        || limits.maximum_feedback_bytes_per_goal == 0
        || limits.maximum_replay_outcomes == 0
        || limits.maximum_deadline_ticks == 0
        || limits.maximum_cancellations == 0
        || limits.maximum_timers == 0
        || limits.maximum_evidence_events == 0
        || limits.maximum_work_per_step == 0
    {
        return Err(ControlError::Unbounded);
    }
    if usize::from(limits.maximum_concurrent_goals)
        + usize::from(limits.maximum_queued_admissions)
        + usize::from(limits.maximum_replay_outcomes)
        > MAXIMUM_REFERENCE_GOALS
        || usize::from(limits.maximum_feedback_items_per_goal)
            > MAXIMUM_REFERENCE_FEEDBACK_ITEMS_PER_GOAL
        || usize::from(limits.maximum_evidence_events) > MAXIMUM_REFERENCE_EVIDENCE
    {
        return Err(ControlError::ReferenceCapacityExceeded);
    }
    if u32::from(limits.maximum_timers)
        < u32::from(limits.maximum_concurrent_goals) + u32::from(limits.maximum_queued_admissions)
    {
        return Err(ControlError::Unbounded);
    }
    if contract.transition_policy == TransitionPolicy::CompatibleCheckpointHandoff
        && contract.checkpoint.is_none()
    {
        return Err(ControlError::HandoffUnsupported);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestReplyOutcome {
    Reply,
    DomainError,
    TimedOut,
    Cancelled,
    Exhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestReplyState {
    Empty,
    InFlight,
    Terminal(RequestReplyOutcome),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestReplySubmission {
    Admitted,
    Duplicate(RequestReplyState),
    Exhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestReplyEvidenceKind {
    Requested,
    Replied,
    DomainError,
    TimedOut,
    Cancelled,
    DuplicateReplayed,
    Exhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestReplyEvidence {
    pub sequence: u32,
    pub identity: ControlIdentity,
    pub kind: RequestReplyEvidenceKind,
    pub outcome: Option<RequestReplyOutcome>,
    pub payload_bytes: u32,
}

/// Rebuildable state for one request/reply subject. Payload bytes remain in
/// their owning domain boundary rather than entering presentation state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestReplySnapshot {
    pub identity: ControlIdentity,
    pub state: RequestReplyState,
    pub deadline_tick: u64,
    pub retries: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExchangeSlot {
    identity: ControlIdentity,
    state: RequestReplyState,
    deadline_tick: u64,
    retries: u16,
    terminal_sequence: u32,
}

const EMPTY_IDENTITY: ControlIdentity = ControlIdentity {
    subject: 0,
    attempt: 0,
    correlation: 0,
    idempotency: 0,
};
const EMPTY_EXCHANGE: ExchangeSlot = ExchangeSlot {
    identity: EMPTY_IDENTITY,
    state: RequestReplyState::Empty,
    deadline_tick: 0,
    retries: 0,
    terminal_sequence: 0,
};

/// Allocation-free deterministic request/reply reference composite.
pub struct ReferenceRequestReply<'a> {
    contract: RequestReplyContract<'a>,
    slots: [ExchangeSlot; MAXIMUM_REFERENCE_EXCHANGES],
    evidence: [Option<RequestReplyEvidence>; MAXIMUM_REFERENCE_EVIDENCE],
    evidence_len: usize,
}

impl<'a> ReferenceRequestReply<'a> {
    pub fn new(contract: RequestReplyContract<'a>) -> Result<Self, ControlError> {
        validate_request_reply_contract(&contract)?;
        Ok(Self {
            contract,
            slots: [EMPTY_EXCHANGE; MAXIMUM_REFERENCE_EXCHANGES],
            evidence: [None; MAXIMUM_REFERENCE_EVIDENCE],
            evidence_len: 0,
        })
    }

    #[must_use]
    pub fn evidence(&self) -> &[Option<RequestReplyEvidence>] {
        &self.evidence[..self.evidence_len]
    }

    #[must_use]
    pub fn state(&self, subject: u64) -> Option<RequestReplyState> {
        self.slots
            .iter()
            .find(|slot| slot.state != RequestReplyState::Empty && slot.identity.subject == subject)
            .map(|slot| slot.state)
    }

    #[must_use]
    pub fn snapshot(&self, subject: u64) -> Option<RequestReplySnapshot> {
        self.slots
            .iter()
            .find(|slot| slot.state != RequestReplyState::Empty && slot.identity.subject == subject)
            .map(|slot| RequestReplySnapshot {
                identity: slot.identity,
                state: slot.state,
                deadline_tick: slot.deadline_tick,
                retries: slot.retries,
            })
    }

    fn reserve_evidence(&self, additional: usize) -> Result<(), ControlError> {
        if self.evidence_len + additional
            > usize::from(self.contract.limits.maximum_evidence_events)
        {
            Err(ControlError::EvidenceExhausted)
        } else {
            Ok(())
        }
    }

    fn record(
        &mut self,
        identity: ControlIdentity,
        kind: RequestReplyEvidenceKind,
        outcome: Option<RequestReplyOutcome>,
        payload_bytes: u32,
    ) {
        self.evidence[self.evidence_len] = Some(RequestReplyEvidence {
            sequence: self.evidence_len as u32,
            identity,
            kind,
            outcome,
            payload_bytes,
        });
        self.evidence_len += 1;
    }

    fn matching(&self, identity: ControlIdentity) -> Result<Option<usize>, ControlError> {
        for (index, slot) in self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.state != RequestReplyState::Empty)
        {
            if slot.identity.idempotency == identity.idempotency {
                return if slot.identity.subject == identity.subject
                    && slot.identity.correlation == identity.correlation
                {
                    Ok(Some(index))
                } else {
                    Err(ControlError::CorrelationConflict)
                };
            }
            if slot.identity.subject == identity.subject
                || slot.identity.correlation == identity.correlation
            {
                return Err(ControlError::CorrelationConflict);
            }
        }
        Ok(None)
    }

    fn compact_oldest_terminal(&mut self) {
        let retained = self
            .slots
            .iter()
            .filter(|slot| matches!(slot.state, RequestReplyState::Terminal(_)))
            .count();
        if retained < usize::from(self.contract.limits.maximum_replay_outcomes) {
            return;
        }
        if let Some((index, _)) = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| matches!(slot.state, RequestReplyState::Terminal(_)))
            .min_by_key(|(_, slot)| slot.terminal_sequence)
        {
            self.slots[index] = EMPTY_EXCHANGE;
        }
    }

    pub fn request(
        &mut self,
        identity: ControlIdentity,
        request_bytes: u32,
        now_tick: u64,
        deadline_tick: u64,
    ) -> Result<RequestReplySubmission, ControlError> {
        self.reserve_evidence(1)?;
        if !identity.valid() {
            return Err(ControlError::InvalidIdentity);
        }
        if request_bytes == 0 || request_bytes > self.contract.limits.maximum_request_bytes {
            return Err(ControlError::RequestTooLarge);
        }
        if deadline_tick <= now_tick
            || deadline_tick - now_tick > self.contract.limits.maximum_deadline_ticks
        {
            return Err(ControlError::DeadlineInvalid);
        }
        if let Some(index) = self.matching(identity)? {
            let state = self.slots[index].state;
            self.record(
                identity,
                RequestReplyEvidenceKind::DuplicateReplayed,
                match state {
                    RequestReplyState::Terminal(outcome) => Some(outcome),
                    _ => None,
                },
                0,
            );
            return Ok(RequestReplySubmission::Duplicate(state));
        }
        if self
            .slots
            .iter()
            .filter(|slot| slot.state == RequestReplyState::InFlight)
            .count()
            >= usize::from(self.contract.limits.maximum_in_flight)
        {
            self.compact_oldest_terminal();
            let slot = self
                .slots
                .iter_mut()
                .find(|slot| slot.state == RequestReplyState::Empty)
                .ok_or(ControlError::ReferenceCapacityExceeded)?;
            *slot = ExchangeSlot {
                identity,
                state: RequestReplyState::Terminal(RequestReplyOutcome::Exhausted),
                deadline_tick,
                retries: 0,
                terminal_sequence: self.evidence_len as u32,
            };
            self.record(
                identity,
                RequestReplyEvidenceKind::Exhausted,
                Some(RequestReplyOutcome::Exhausted),
                0,
            );
            return Ok(RequestReplySubmission::Exhausted);
        }
        self.compact_oldest_terminal();
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.state == RequestReplyState::Empty)
            .ok_or(ControlError::ReferenceCapacityExceeded)?;
        *slot = ExchangeSlot {
            identity,
            state: RequestReplyState::InFlight,
            deadline_tick,
            retries: 0,
            terminal_sequence: 0,
        };
        self.record(
            identity,
            RequestReplyEvidenceKind::Requested,
            None,
            request_bytes,
        );
        Ok(RequestReplySubmission::Admitted)
    }

    fn terminal(
        &mut self,
        subject: u64,
        outcome: RequestReplyOutcome,
        kind: RequestReplyEvidenceKind,
        payload_bytes: u32,
        maximum_payload_bytes: u32,
        oversize: ControlError,
    ) -> Result<RequestReplyState, ControlError> {
        self.reserve_evidence(1)?;
        if payload_bytes > maximum_payload_bytes {
            return Err(oversize);
        }
        let index = self
            .slots
            .iter()
            .position(|slot| {
                slot.state == RequestReplyState::InFlight && slot.identity.subject == subject
            })
            .ok_or(ControlError::IllegalState)?;
        let state = RequestReplyState::Terminal(outcome);
        self.slots[index].state = state;
        self.slots[index].terminal_sequence = self.evidence_len as u32;
        let identity = self.slots[index].identity;
        self.record(identity, kind, Some(outcome), payload_bytes);
        Ok(state)
    }

    pub fn reply(&mut self, subject: u64, bytes: u32) -> Result<RequestReplyState, ControlError> {
        self.terminal(
            subject,
            RequestReplyOutcome::Reply,
            RequestReplyEvidenceKind::Replied,
            bytes,
            self.contract.limits.maximum_reply_bytes,
            ControlError::ReplyTooLarge,
        )
    }

    pub fn domain_error(
        &mut self,
        subject: u64,
        bytes: u32,
    ) -> Result<RequestReplyState, ControlError> {
        self.terminal(
            subject,
            RequestReplyOutcome::DomainError,
            RequestReplyEvidenceKind::DomainError,
            bytes,
            self.contract.limits.maximum_domain_error_bytes,
            ControlError::DomainFailureTooLarge,
        )
    }

    pub fn cancel(&mut self, subject: u64) -> Result<RequestReplyState, ControlError> {
        self.terminal(
            subject,
            RequestReplyOutcome::Cancelled,
            RequestReplyEvidenceKind::Cancelled,
            0,
            self.contract.limits.maximum_reply_bytes,
            ControlError::ReplyTooLarge,
        )
    }

    pub fn retry(
        &mut self,
        subject: u64,
        now_tick: u64,
        deadline_tick: u64,
    ) -> Result<ControlIdentity, ControlError> {
        self.reserve_evidence(1)?;
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| {
                slot.state == RequestReplyState::InFlight && slot.identity.subject == subject
            })
            .ok_or(ControlError::IllegalState)?;
        if slot.retries >= self.contract.limits.maximum_retries {
            return Err(ControlError::RetryExhausted);
        }
        if deadline_tick <= now_tick
            || deadline_tick - now_tick > self.contract.limits.maximum_deadline_ticks
        {
            return Err(ControlError::DeadlineInvalid);
        }
        slot.retries += 1;
        slot.identity.attempt = slot
            .identity
            .attempt
            .checked_add(1)
            .ok_or(ControlError::RetryExhausted)?;
        slot.deadline_tick = deadline_tick;
        let identity = slot.identity;
        self.record(identity, RequestReplyEvidenceKind::Requested, None, 0);
        Ok(identity)
    }

    pub fn advance(&mut self, now_tick: u64) -> Result<u16, ControlError> {
        let work = usize::from(self.contract.limits.maximum_work_per_step);
        let expired = self
            .slots
            .iter()
            .filter(|slot| {
                slot.state == RequestReplyState::InFlight && now_tick >= slot.deadline_tick
            })
            .take(work)
            .count();
        self.reserve_evidence(expired)?;
        let mut count = 0_u16;
        for index in 0..self.slots.len() {
            if usize::from(count) == work {
                break;
            }
            if self.slots[index].state == RequestReplyState::InFlight
                && now_tick >= self.slots[index].deadline_tick
            {
                self.slots[index].state =
                    RequestReplyState::Terminal(RequestReplyOutcome::TimedOut);
                self.slots[index].terminal_sequence = self.evidence_len as u32;
                let identity = self.slots[index].identity;
                self.record(
                    identity,
                    RequestReplyEvidenceKind::TimedOut,
                    Some(RequestReplyOutcome::TimedOut),
                    0,
                );
                count += 1;
            }
        }
        Ok(count)
    }
}

/// Why an unadmitted goal was rejected. These are never running failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RejectionReason {
    DomainPolicy,
    Authority,
    WorkloadAdmission,
    Placement,
    ResourceCommitCleanup,
    Inhibited,
    ConcurrentGoalLimit,
}

/// Exact externally supplied admission proofs. The reference composite checks
/// their identities; it cannot create any proof itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionAdmission {
    pub authority: Option<SemanticHash>,
    pub workload: Option<SemanticHash>,
    pub placement: Option<SemanticHash>,
    pub resource_commit_cleanup: Option<SemanticHash>,
    pub inhibit: Option<SemanticHash>,
    pub domain_policy_allows: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionOutcome {
    Rejected(RejectionReason),
    Cancelled,
    Failed,
    Result,
    DeadlineExhausted,
    Discontinued,
    WithdrawnBeforeAdmission,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionState {
    Empty,
    Queued,
    Accepted,
    Terminal(ActionOutcome),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionSubmission {
    Queued,
    Duplicate(ActionState),
    Exhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeedbackDisposition {
    Retained,
    Blocked,
    DroppedOldest,
    CoalescedLatest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionInterruption {
    ProviderLoss,
    PlanTransition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionEvidenceKind {
    GoalQueued,
    GoalAccepted,
    GoalRejected,
    DuplicateReplayed,
    AdmissionExhausted,
    FeedbackObserved,
    FeedbackPressured,
    CancelRequested,
    GoalCancelled,
    GoalWithdrawn,
    AttemptRetried,
    Result,
    Failed,
    DeadlineExhausted,
    ProviderLost,
    TransitionDiscontinuity,
    TransitionHandoff,
}

/// Immutable causal evidence. Feedback payloads are deliberately absent;
/// only bounded observation/pressure facts are retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionEvidence {
    pub sequence: u32,
    pub identity: ControlIdentity,
    pub kind: ActionEvidenceKind,
    pub state: ActionState,
    pub feedback_bytes: u32,
    pub feedback_disposition: Option<FeedbackDisposition>,
    pub feedback_items_affected: u16,
    pub terminal_bytes: u32,
    pub causal_sequence: Option<u32>,
}

/// Rebuildable state for one action goal. Feedback payloads remain absent;
/// only finite retained counts and byte totals cross into presentation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionSnapshot {
    pub identity: ControlIdentity,
    pub state: ActionState,
    pub deadline_tick: u64,
    pub retries: u16,
    pub feedback_items: u16,
    pub feedback_bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActionSlot {
    identity: ControlIdentity,
    state: ActionState,
    deadline_tick: u64,
    retries: u16,
    feedback_items: u16,
    feedback_bytes: u32,
    feedback_head: u16,
    feedback_item_bytes: [u32; MAXIMUM_REFERENCE_FEEDBACK_ITEMS_PER_GOAL],
    terminal_sequence: u32,
}

const EMPTY_ACTION: ActionSlot = ActionSlot {
    identity: EMPTY_IDENTITY,
    state: ActionState::Empty,
    deadline_tick: 0,
    retries: 0,
    feedback_items: 0,
    feedback_bytes: 0,
    feedback_head: 0,
    feedback_item_bytes: [0; MAXIMUM_REFERENCE_FEEDBACK_ITEMS_PER_GOAL],
    terminal_sequence: 0,
};

/// Allocation-free deterministic action reference composite.
pub struct ReferenceAction<'a> {
    contract: ActionContract<'a>,
    slots: [ActionSlot; MAXIMUM_REFERENCE_GOALS],
    evidence: [Option<ActionEvidence>; MAXIMUM_REFERENCE_EVIDENCE],
    evidence_len: usize,
    cancellations: u16,
}

impl<'a> ReferenceAction<'a> {
    pub fn new(contract: ActionContract<'a>) -> Result<Self, ControlError> {
        validate_action_contract(&contract)?;
        Ok(Self {
            contract,
            slots: [EMPTY_ACTION; MAXIMUM_REFERENCE_GOALS],
            evidence: [None; MAXIMUM_REFERENCE_EVIDENCE],
            evidence_len: 0,
            cancellations: 0,
        })
    }

    #[must_use]
    pub fn evidence(&self) -> &[Option<ActionEvidence>] {
        &self.evidence[..self.evidence_len]
    }

    #[must_use]
    pub fn state(&self, goal: u64) -> Option<ActionState> {
        self.slots
            .iter()
            .find(|slot| slot.state != ActionState::Empty && slot.identity.subject == goal)
            .map(|slot| slot.state)
    }

    #[must_use]
    pub fn snapshot(&self, goal: u64) -> Option<ActionSnapshot> {
        self.slots
            .iter()
            .find(|slot| slot.state != ActionState::Empty && slot.identity.subject == goal)
            .map(|slot| ActionSnapshot {
                identity: slot.identity,
                state: slot.state,
                deadline_tick: slot.deadline_tick,
                retries: slot.retries,
                feedback_items: slot.feedback_items,
                feedback_bytes: slot.feedback_bytes,
            })
    }

    #[must_use]
    pub const fn cancellations(&self) -> u16 {
        self.cancellations
    }

    fn reserve_evidence(&self, additional: usize) -> Result<(), ControlError> {
        if self.evidence_len + additional
            > usize::from(self.contract.limits.maximum_evidence_events)
        {
            Err(ControlError::EvidenceExhausted)
        } else {
            Ok(())
        }
    }

    fn record(
        &mut self,
        identity: ControlIdentity,
        kind: ActionEvidenceKind,
        state: ActionState,
        feedback_bytes: u32,
        causal_sequence: Option<u32>,
    ) {
        self.evidence[self.evidence_len] = Some(ActionEvidence {
            sequence: self.evidence_len as u32,
            identity,
            kind,
            state,
            feedback_bytes,
            feedback_disposition: None,
            feedback_items_affected: 0,
            terminal_bytes: 0,
            causal_sequence,
        });
        self.evidence_len += 1;
    }

    fn matching(&self, identity: ControlIdentity) -> Result<Option<usize>, ControlError> {
        for (index, slot) in self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.state != ActionState::Empty)
        {
            if slot.identity.idempotency == identity.idempotency {
                return if slot.identity.subject == identity.subject
                    && slot.identity.correlation == identity.correlation
                {
                    Ok(Some(index))
                } else {
                    Err(ControlError::CorrelationConflict)
                };
            }
            if slot.identity.subject == identity.subject
                || slot.identity.correlation == identity.correlation
            {
                return Err(ControlError::CorrelationConflict);
            }
        }
        Ok(None)
    }

    fn compact_oldest_terminal(&mut self) {
        let retained = self
            .slots
            .iter()
            .filter(|slot| matches!(slot.state, ActionState::Terminal(_)))
            .count();
        if retained < usize::from(self.contract.limits.maximum_replay_outcomes) {
            return;
        }
        if let Some((index, _)) = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| matches!(slot.state, ActionState::Terminal(_)))
            .min_by_key(|(_, slot)| slot.terminal_sequence)
        {
            self.slots[index] = EMPTY_ACTION;
        }
    }

    pub fn submit(
        &mut self,
        identity: ControlIdentity,
        goal_bytes: u32,
        now_tick: u64,
        deadline_tick: u64,
    ) -> Result<ActionSubmission, ControlError> {
        self.reserve_evidence(1)?;
        if !identity.valid() {
            return Err(ControlError::InvalidIdentity);
        }
        if goal_bytes == 0 || goal_bytes > self.contract.limits.maximum_goal_bytes {
            return Err(ControlError::RequestTooLarge);
        }
        if deadline_tick <= now_tick
            || deadline_tick - now_tick > self.contract.limits.maximum_deadline_ticks
        {
            return Err(ControlError::DeadlineInvalid);
        }
        if let Some(index) = self.matching(identity)? {
            let state = self.slots[index].state;
            self.record(
                identity,
                ActionEvidenceKind::DuplicateReplayed,
                state,
                0,
                None,
            );
            return Ok(ActionSubmission::Duplicate(state));
        }
        if self
            .slots
            .iter()
            .filter(|slot| slot.state == ActionState::Queued)
            .count()
            >= usize::from(self.contract.limits.maximum_queued_admissions)
        {
            self.compact_oldest_terminal();
            let state =
                ActionState::Terminal(ActionOutcome::Rejected(RejectionReason::WorkloadAdmission));
            let slot = self
                .slots
                .iter_mut()
                .find(|slot| slot.state == ActionState::Empty)
                .ok_or(ControlError::ReferenceCapacityExceeded)?;
            *slot = ActionSlot {
                identity,
                state,
                deadline_tick,
                retries: 0,
                feedback_items: 0,
                feedback_bytes: 0,
                feedback_head: 0,
                feedback_item_bytes: [0; MAXIMUM_REFERENCE_FEEDBACK_ITEMS_PER_GOAL],
                terminal_sequence: self.evidence_len as u32,
            };
            self.record(
                identity,
                ActionEvidenceKind::AdmissionExhausted,
                state,
                0,
                None,
            );
            return Ok(ActionSubmission::Exhausted);
        }
        self.compact_oldest_terminal();
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.state == ActionState::Empty)
            .ok_or(ControlError::ReferenceCapacityExceeded)?;
        *slot = ActionSlot {
            identity,
            state: ActionState::Queued,
            deadline_tick,
            retries: 0,
            feedback_items: 0,
            feedback_bytes: 0,
            feedback_head: 0,
            feedback_item_bytes: [0; MAXIMUM_REFERENCE_FEEDBACK_ITEMS_PER_GOAL],
            terminal_sequence: 0,
        };
        self.record(
            identity,
            ActionEvidenceKind::GoalQueued,
            ActionState::Queued,
            0,
            None,
        );
        Ok(ActionSubmission::Queued)
    }

    fn admission_denial(&self, admission: ActionAdmission) -> Option<RejectionReason> {
        if !admission.domain_policy_allows {
            Some(RejectionReason::DomainPolicy)
        } else if admission.authority != Some(self.contract.admission_authority.semantic_hash) {
            Some(RejectionReason::Authority)
        } else if admission.workload != Some(self.contract.workload_admission.semantic_hash) {
            Some(RejectionReason::WorkloadAdmission)
        } else if admission.placement != Some(self.contract.placement.semantic_hash) {
            Some(RejectionReason::Placement)
        } else if admission.resource_commit_cleanup
            != Some(self.contract.resource_commit_cleanup.semantic_hash)
        {
            Some(RejectionReason::ResourceCommitCleanup)
        } else if self
            .contract
            .inhibit
            .is_some_and(|required| admission.inhibit != Some(required.semantic_hash))
        {
            Some(RejectionReason::Inhibited)
        } else if self
            .slots
            .iter()
            .filter(|slot| slot.state == ActionState::Accepted)
            .count()
            >= usize::from(self.contract.limits.maximum_concurrent_goals)
        {
            Some(RejectionReason::ConcurrentGoalLimit)
        } else {
            None
        }
    }

    pub fn admit(
        &mut self,
        goal: u64,
        admission: ActionAdmission,
    ) -> Result<ActionState, ControlError> {
        self.reserve_evidence(1)?;
        let index = self
            .slots
            .iter()
            .position(|slot| slot.state == ActionState::Queued && slot.identity.subject == goal)
            .ok_or(ControlError::IllegalState)?;
        let identity = self.slots[index].identity;
        if let Some(reason) = self.admission_denial(admission) {
            let state = ActionState::Terminal(ActionOutcome::Rejected(reason));
            self.slots[index].state = state;
            self.slots[index].terminal_sequence = self.evidence_len as u32;
            self.record(identity, ActionEvidenceKind::GoalRejected, state, 0, None);
            return Ok(state);
        }
        self.slots[index].state = ActionState::Accepted;
        self.record(
            identity,
            ActionEvidenceKind::GoalAccepted,
            ActionState::Accepted,
            0,
            None,
        );
        Ok(ActionState::Accepted)
    }

    pub fn feedback(&mut self, goal: u64, bytes: u32) -> Result<FeedbackDisposition, ControlError> {
        self.reserve_evidence(1)?;
        if bytes == 0 || bytes > self.contract.limits.maximum_feedback_bytes_per_goal {
            return Err(ControlError::FeedbackTooLarge);
        }
        let index = self
            .slots
            .iter()
            .position(|slot| slot.state == ActionState::Accepted && slot.identity.subject == goal)
            .ok_or(ControlError::IllegalState)?;
        let maximum_items = self.contract.limits.maximum_feedback_items_per_goal;
        let maximum_bytes = self.contract.limits.maximum_feedback_bytes_per_goal;
        let slot = &mut self.slots[index];
        let full = slot.feedback_items >= maximum_items
            || slot
                .feedback_bytes
                .checked_add(bytes)
                .is_none_or(|total| total > maximum_bytes);
        let mut affected_items = 0_u16;
        let disposition = if !full {
            let tail = (usize::from(slot.feedback_head) + usize::from(slot.feedback_items))
                % MAXIMUM_REFERENCE_FEEDBACK_ITEMS_PER_GOAL;
            slot.feedback_item_bytes[tail] = bytes;
            slot.feedback_items += 1;
            slot.feedback_bytes += bytes;
            FeedbackDisposition::Retained
        } else {
            match self.contract.feedback_pressure {
                FeedbackPressurePolicy::BlockProducer => FeedbackDisposition::Blocked,
                FeedbackPressurePolicy::DropOldest => {
                    while slot.feedback_items >= maximum_items
                        || slot
                            .feedback_bytes
                            .checked_add(bytes)
                            .is_none_or(|total| total > maximum_bytes)
                    {
                        let head = usize::from(slot.feedback_head);
                        slot.feedback_bytes -= slot.feedback_item_bytes[head];
                        slot.feedback_item_bytes[head] = 0;
                        slot.feedback_items -= 1;
                        slot.feedback_head =
                            ((head + 1) % MAXIMUM_REFERENCE_FEEDBACK_ITEMS_PER_GOAL) as u16;
                        affected_items += 1;
                    }
                    let tail = (usize::from(slot.feedback_head) + usize::from(slot.feedback_items))
                        % MAXIMUM_REFERENCE_FEEDBACK_ITEMS_PER_GOAL;
                    slot.feedback_item_bytes[tail] = bytes;
                    slot.feedback_items += 1;
                    slot.feedback_bytes += bytes;
                    FeedbackDisposition::DroppedOldest
                }
                FeedbackPressurePolicy::CoalesceLatest => {
                    affected_items = slot.feedback_items;
                    slot.feedback_item_bytes = [0; MAXIMUM_REFERENCE_FEEDBACK_ITEMS_PER_GOAL];
                    slot.feedback_item_bytes[0] = bytes;
                    slot.feedback_head = 0;
                    slot.feedback_items = 1;
                    slot.feedback_bytes = bytes;
                    FeedbackDisposition::CoalescedLatest
                }
            }
        };
        let identity = self.slots[index].identity;
        self.record(
            identity,
            if disposition == FeedbackDisposition::Retained {
                ActionEvidenceKind::FeedbackObserved
            } else {
                ActionEvidenceKind::FeedbackPressured
            },
            ActionState::Accepted,
            bytes,
            None,
        );
        if let Some(event) = self.evidence[self.evidence_len - 1].as_mut() {
            event.feedback_disposition = Some(disposition);
            event.feedback_items_affected = affected_items;
        }
        Ok(disposition)
    }

    pub fn consume_feedback(&mut self, goal: u64, bytes: u32) -> Result<(), ControlError> {
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.state == ActionState::Accepted && slot.identity.subject == goal)
            .ok_or(ControlError::IllegalState)?;
        if slot.feedback_items == 0 {
            return Err(ControlError::IllegalState);
        }
        let head = usize::from(slot.feedback_head);
        if slot.feedback_item_bytes[head] != bytes {
            return Err(ControlError::IllegalState);
        }
        slot.feedback_item_bytes[head] = 0;
        slot.feedback_items -= 1;
        slot.feedback_bytes -= bytes;
        slot.feedback_head = if slot.feedback_items == 0 {
            0
        } else {
            ((head + 1) % MAXIMUM_REFERENCE_FEEDBACK_ITEMS_PER_GOAL) as u16
        };
        Ok(())
    }

    pub fn cancel(&mut self, goal: u64) -> Result<ActionState, ControlError> {
        self.reserve_evidence(2)?;
        if self.cancellations >= self.contract.limits.maximum_cancellations {
            return Err(ControlError::CancellationExhausted);
        }
        let index = self
            .slots
            .iter()
            .position(|slot| {
                matches!(slot.state, ActionState::Queued | ActionState::Accepted)
                    && slot.identity.subject == goal
            })
            .ok_or(ControlError::IllegalState)?;
        let identity = self.slots[index].identity;
        let causal_sequence = self.evidence_len as u32;
        self.record(
            identity,
            ActionEvidenceKind::CancelRequested,
            self.slots[index].state,
            0,
            None,
        );
        let (state, kind) = if self.slots[index].state == ActionState::Queued {
            (
                ActionState::Terminal(ActionOutcome::WithdrawnBeforeAdmission),
                ActionEvidenceKind::GoalWithdrawn,
            )
        } else {
            (
                ActionState::Terminal(ActionOutcome::Cancelled),
                ActionEvidenceKind::GoalCancelled,
            )
        };
        self.slots[index].state = state;
        self.slots[index].feedback_items = 0;
        self.slots[index].feedback_bytes = 0;
        self.slots[index].feedback_head = 0;
        self.slots[index].feedback_item_bytes = [0; MAXIMUM_REFERENCE_FEEDBACK_ITEMS_PER_GOAL];
        self.slots[index].terminal_sequence = self.evidence_len as u32;
        self.cancellations += 1;
        self.record(identity, kind, state, 0, Some(causal_sequence));
        Ok(state)
    }

    pub fn retry_attempt(&mut self, goal: u64) -> Result<ControlIdentity, ControlError> {
        self.reserve_evidence(1)?;
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.state == ActionState::Accepted && slot.identity.subject == goal)
            .ok_or(ControlError::IllegalState)?;
        if slot.retries >= self.contract.limits.maximum_retries_per_goal {
            return Err(ControlError::RetryExhausted);
        }
        slot.retries += 1;
        slot.identity.attempt = slot
            .identity
            .attempt
            .checked_add(1)
            .ok_or(ControlError::RetryExhausted)?;
        let identity = slot.identity;
        self.record(
            identity,
            ActionEvidenceKind::AttemptRetried,
            ActionState::Accepted,
            0,
            None,
        );
        Ok(identity)
    }

    fn finish(
        &mut self,
        goal: u64,
        outcome: ActionOutcome,
        kind: ActionEvidenceKind,
        terminal_bytes: u32,
        maximum_terminal_bytes: u32,
        oversize: ControlError,
    ) -> Result<ActionState, ControlError> {
        self.reserve_evidence(1)?;
        if terminal_bytes > maximum_terminal_bytes {
            return Err(oversize);
        }
        let index = self
            .slots
            .iter()
            .position(|slot| slot.state == ActionState::Accepted && slot.identity.subject == goal)
            .ok_or(ControlError::IllegalState)?;
        let state = ActionState::Terminal(outcome);
        self.slots[index].state = state;
        self.slots[index].feedback_items = 0;
        self.slots[index].feedback_bytes = 0;
        self.slots[index].feedback_head = 0;
        self.slots[index].feedback_item_bytes = [0; MAXIMUM_REFERENCE_FEEDBACK_ITEMS_PER_GOAL];
        self.slots[index].terminal_sequence = self.evidence_len as u32;
        let identity = self.slots[index].identity;
        self.record(identity, kind, state, 0, None);
        if let Some(event) = self.evidence[self.evidence_len - 1].as_mut() {
            event.terminal_bytes = terminal_bytes;
        }
        Ok(state)
    }

    pub fn result(&mut self, goal: u64, bytes: u32) -> Result<ActionState, ControlError> {
        self.finish(
            goal,
            ActionOutcome::Result,
            ActionEvidenceKind::Result,
            bytes,
            self.contract.limits.maximum_result_bytes,
            ControlError::ResultTooLarge,
        )
    }

    pub fn fail(&mut self, goal: u64, bytes: u32) -> Result<ActionState, ControlError> {
        self.finish(
            goal,
            ActionOutcome::Failed,
            ActionEvidenceKind::Failed,
            bytes,
            self.contract.limits.maximum_domain_failure_bytes,
            ControlError::DomainFailureTooLarge,
        )
    }

    pub fn advance(&mut self, now_tick: u64) -> Result<u16, ControlError> {
        let work = usize::from(self.contract.limits.maximum_work_per_step);
        let expired = self
            .slots
            .iter()
            .filter(|slot| {
                matches!(slot.state, ActionState::Queued | ActionState::Accepted)
                    && now_tick >= slot.deadline_tick
            })
            .take(work)
            .count();
        self.reserve_evidence(expired)?;
        let mut count = 0_u16;
        for index in 0..self.slots.len() {
            if usize::from(count) == work {
                break;
            }
            if matches!(
                self.slots[index].state,
                ActionState::Queued | ActionState::Accepted
            ) && now_tick >= self.slots[index].deadline_tick
            {
                let state = ActionState::Terminal(ActionOutcome::DeadlineExhausted);
                self.slots[index].state = state;
                self.slots[index].feedback_items = 0;
                self.slots[index].feedback_bytes = 0;
                self.slots[index].feedback_head = 0;
                self.slots[index].feedback_item_bytes =
                    [0; MAXIMUM_REFERENCE_FEEDBACK_ITEMS_PER_GOAL];
                self.slots[index].terminal_sequence = self.evidence_len as u32;
                let identity = self.slots[index].identity;
                self.record(
                    identity,
                    ActionEvidenceKind::DeadlineExhausted,
                    state,
                    0,
                    None,
                );
                count += 1;
            }
        }
        Ok(count)
    }

    pub fn interrupt(
        &mut self,
        goal: u64,
        interruption: ActionInterruption,
        transition_proof: Option<SemanticHash>,
        checkpoint_proof: Option<SemanticHash>,
    ) -> Result<ActionState, ControlError> {
        self.reserve_evidence(1)?;
        let index = self
            .slots
            .iter()
            .position(|slot| slot.state == ActionState::Accepted && slot.identity.subject == goal)
            .ok_or(ControlError::IllegalState)?;
        if transition_proof != Some(self.contract.transition.semantic_hash) {
            return Err(ControlError::HandoffUnsupported);
        }
        let identity = self.slots[index].identity;
        match self.contract.transition_policy {
            TransitionPolicy::CompatibleCheckpointHandoff => {
                let required = self
                    .contract
                    .checkpoint
                    .ok_or(ControlError::HandoffUnsupported)?;
                if checkpoint_proof != Some(required.semantic_hash) {
                    return Err(ControlError::HandoffUnsupported);
                }
                self.record(
                    identity,
                    ActionEvidenceKind::TransitionHandoff,
                    ActionState::Accepted,
                    0,
                    None,
                );
                Ok(ActionState::Accepted)
            }
            TransitionPolicy::ExplicitDiscontinuity => self.finish(
                goal,
                ActionOutcome::Discontinued,
                ActionEvidenceKind::TransitionDiscontinuity,
                0,
                self.contract.limits.maximum_domain_failure_bytes,
                ControlError::DomainFailureTooLarge,
            ),
            TransitionPolicy::TerminalFailure => self.finish(
                goal,
                ActionOutcome::Failed,
                match interruption {
                    ActionInterruption::ProviderLoss => ActionEvidenceKind::ProviderLost,
                    ActionInterruption::PlanTransition => ActionEvidenceKind::Failed,
                },
                0,
                self.contract.limits.maximum_domain_failure_bytes,
                ControlError::DomainFailureTooLarge,
            ),
        }
    }
}
