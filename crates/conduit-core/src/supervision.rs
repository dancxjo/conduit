//! Allocator-free terminal-observation and bounded supervision contracts.
//!
//! Expected domain outcomes remain ordinary typed values. Admission failures
//! remain diagnostics before a run starts. This module applies only after an
//! admitted subject reaches a runtime terminal state.

use core::fmt;

use crate::{Id, InstancePath, SemanticHash, StopPolicy, TerminalCauseCode, TerminalClass};

/// Version of the portable supervision observation and decision contract.
pub const SUPERVISION_CONTRACT_VERSION: u32 = 1;

/// The three mechanisms that must not be collapsed into a generic error port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailurePlane {
    /// Expected negative domain behavior, carried by an explicit typed value.
    DomainValue,
    /// Failure of an admitted running subject, carried by control/evidence.
    RuntimeTerminal,
    /// Parse, lower, resolve, authorize, reserve, or admission rejection.
    AdmissionDiagnostic,
}

/// Runtime phase in which the observed subject became terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalPhase {
    Prepare,
    Start,
    Step,
    HostOperation,
    Drain,
    Cleanup,
}

/// Whether retrying the same admitted operation is semantically declared.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryDeclaration {
    /// No retry contract exists. `retry-same` is forbidden.
    Undeclared,
    /// The owning contract declares the operation idempotent and retryable.
    Idempotent,
    /// A fresh instance attempt is allowed, but replaying the operation is not.
    RestartOnly,
}

/// Exact optional identities retained when policy permits their disclosure.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerminalContext<'a> {
    pub resource: Option<Id<'a>>,
    pub authority: Option<SemanticHash>,
    pub host: Option<Id<'a>>,
    pub implementation: Option<SemanticHash>,
    pub artifact: Option<Id<'a>>,
    pub transition: Option<SemanticHash>,
}

/// Redacted cursor into immutable execution evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidenceCursor<'a> {
    pub stream: Id<'a>,
    pub sequence: u64,
}

/// Result of reading bounded supervision evidence at an exact cursor.
///
/// A gap reports only the first retained sequence; it never fabricates the
/// evicted events or their outcomes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceCursorStatus {
    Available,
    Gap { resume_at: u64 },
    Future { next_sequence: u64 },
}

/// Classify a cursor against one finite retained evidence window.
pub const fn classify_evidence_cursor(
    requested: u64,
    retained_from: u64,
    next_sequence: u64,
) -> Result<EvidenceCursorStatus, SupervisionReason> {
    if retained_from > next_sequence {
        return Err(SupervisionReason::ObservationInvalid);
    }
    if requested < retained_from {
        Ok(EvidenceCursorStatus::Gap {
            resume_at: retained_from,
        })
    } else if requested >= next_sequence {
        Ok(EvidenceCursorStatus::Future { next_sequence })
    } else {
        Ok(EvidenceCursorStatus::Available)
    }
}

/// Finite recovery budget copied into every terminal observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryBudget {
    pub remaining_observations: u16,
    pub remaining_decisions: u16,
    pub remaining_attempts: u16,
    pub remaining_evidence_events: u16,
    pub now_tick: u64,
    pub deadline_tick: u64,
}

impl RecoveryBudget {
    /// Reject an exhausted or non-finite observation budget.
    pub const fn validate(self) -> Result<(), SupervisionReason> {
        if self.remaining_observations == 0 {
            return Err(SupervisionReason::ObservationBudgetExhausted);
        }
        if self.remaining_decisions == 0 {
            return Err(SupervisionReason::DecisionBudgetExhausted);
        }
        if self.remaining_evidence_events == 0 {
            return Err(SupervisionReason::EvidenceBudgetExhausted);
        }
        if self.deadline_tick <= self.now_tick {
            return Err(SupervisionReason::DeadlineExpired);
        }
        Ok(())
    }
}

/// One exact retained cause in a supervision chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupervisionCauseRef<'a> {
    pub code: TerminalCauseCode,
    pub subject: InstancePath<'a>,
    pub generation: u32,
    pub attempt: u16,
}

/// Structured observation delivered to an ordinary supervisor node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalObservation<'a> {
    pub semantic_subject: InstancePath<'a>,
    pub expanded_subject: InstancePath<'a>,
    pub run: Id<'a>,
    pub plan_identity: SemanticHash,
    pub plan_epoch: u64,
    pub generation: u32,
    pub attempt: u16,
    pub class: TerminalClass,
    pub code: TerminalCauseCode,
    pub phase: TerminalPhase,
    pub caused_by: &'a [SupervisionCauseRef<'a>],
    pub retry: RetryDeclaration,
    pub context: TerminalContext<'a>,
    pub evidence: EvidenceCursor<'a>,
    pub budget: RecoveryBudget,
}

/// Scope owned by one explicit supervisor binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisionScope {
    Child,
    NamedGroup,
    CompositeBoundary,
    ReplicatedChild,
}

/// Whether one member terminal stops the whole admitted scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisionFailureMode {
    FailTogether,
    IsolatedOptional,
}

/// Finite action kinds available to a supervisor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisionActionKind {
    Propagate,
    StopScope,
    RestartSame,
    RetrySame,
    ActivateDeclaredFallback,
    ContinueDeclaredDegradedMode,
    RequestOperatorAction,
}

/// One action already admitted in an immutable exact plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedSupervisionAction<'a> {
    pub kind: SupervisionActionKind,
    /// Exact admitted fallback, degraded mode, or operator request. Other
    /// actions require `None`.
    pub target: Option<Id<'a>>,
    /// Maximum decisions selecting this action in one supervisor generation.
    pub maximum_uses: u16,
    /// Retry-same requires this to be true as well as an idempotent contract.
    pub permits_effect_replay: bool,
    /// A degraded choice may be selected only when this is true.
    pub preserves_required_guarantees: bool,
    /// True only when this action would require a candidate #57 plan epoch.
    /// Such an action is plan-visible for reporting but cannot execute in
    /// place.
    pub requires_new_epoch: bool,
}

/// Finite decision emitted as an ordinary typed supervisor value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupervisionDecision<'a> {
    pub kind: SupervisionActionKind,
    pub target: Option<Id<'a>>,
}

/// Exact resources reserved for one supervisor binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupervisionLimits {
    pub maximum_observations: u16,
    pub maximum_decisions: u16,
    pub maximum_in_flight: u16,
    pub maximum_cause_depth: u16,
    pub maximum_nested_depth: u8,
    pub maximum_handler_ticks: u64,
    pub maximum_recovery_ticks: u64,
    pub restart_window_ticks: u64,
    pub backoff_ticks: u64,
    pub cooldown_ticks: u64,
    pub operator_wait_ticks: u64,
    pub maximum_evidence_events: u16,
    pub observation_bytes: u32,
    pub decision_bytes: u32,
    pub scratch_bytes: u32,
}

impl SupervisionLimits {
    pub const fn validate(self) -> Result<(), SupervisionReason> {
        if self.maximum_observations == 0
            || self.maximum_decisions == 0
            || self.maximum_in_flight == 0
            || self.maximum_cause_depth == 0
            || self.maximum_nested_depth == 0
            || self.maximum_handler_ticks == 0
            || self.maximum_recovery_ticks == 0
            || self.restart_window_ticks == 0
            || self.backoff_ticks == 0
            || self.cooldown_ticks == 0
            || self.operator_wait_ticks == 0
            || self.maximum_evidence_events == 0
            || self.observation_bytes == 0
            || self.decision_bytes == 0
            || self.scratch_bytes == 0
            || self.maximum_in_flight > self.maximum_observations
            || self.restart_window_ticks > self.maximum_recovery_ticks
            || self.backoff_ticks > self.maximum_recovery_ticks
            || self.cooldown_ticks > self.maximum_recovery_ticks
            || self.operator_wait_ticks > self.maximum_recovery_ticks
        {
            return Err(SupervisionReason::UnboundedContract);
        }
        Ok(())
    }
}

/// Exact, host-neutral binding between a subject and an ordinary handler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupervisionContract<'a> {
    pub schema_version: u32,
    pub id: Id<'a>,
    pub scope: SupervisionScope,
    pub subject: InstancePath<'a>,
    pub handler: InstancePath<'a>,
    /// Exact group members. Non-group scopes require an empty slice.
    pub members: &'a [InstancePath<'a>],
    pub failure_mode: SupervisionFailureMode,
    pub outer: Option<Id<'a>>,
    pub actions: &'a [AdmittedSupervisionAction<'a>],
    pub limits: SupervisionLimits,
    pub cleanup: StopPolicy,
    /// Required behavior forbids a degraded-mode decision whose admitted
    /// action does not preserve required guarantees.
    pub required_behavior: bool,
}

impl SupervisionContract<'_> {
    pub fn validate(self) -> Result<(), SupervisionReason> {
        if self.schema_version != SUPERVISION_CONTRACT_VERSION
            || self.id.as_str().is_empty()
            || self.subject.as_str().is_empty()
            || self.handler.as_str().is_empty()
            || self.subject == self.handler
            || self.actions.is_empty()
        {
            return Err(SupervisionReason::InvalidContract);
        }
        let group_shape_valid = match self.scope {
            SupervisionScope::NamedGroup => {
                self.members.len() >= 2
                    && self.members.contains(&self.subject)
                    && !self.members.contains(&self.handler)
            }
            SupervisionScope::Child
            | SupervisionScope::CompositeBoundary
            | SupervisionScope::ReplicatedChild => self.members.is_empty(),
        };
        if !group_shape_valid
            || self
                .members
                .iter()
                .enumerate()
                .any(|(index, member)| self.members[..index].contains(member))
            || (self.failure_mode == SupervisionFailureMode::IsolatedOptional
                && self.scope != SupervisionScope::NamedGroup)
        {
            return Err(SupervisionReason::InvalidContract);
        }
        self.limits.validate()?;
        for (index, action) in self.actions.iter().enumerate() {
            if action.maximum_uses == 0
                || action_shape_invalid(*action)
                || self.actions[..index]
                    .iter()
                    .any(|prior| prior.kind == action.kind && prior.target == action.target)
            {
                return Err(SupervisionReason::InvalidContract);
            }
        }
        Ok(())
    }
}

/// Strict execution profile supported by a host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisionHostProfile {
    /// Full portable action vocabulary; exact actions still come from the plan.
    Hosted,
    /// Browser worker/main-thread implementations using the same typed values.
    Browser,
    /// Deterministic reference implementation.
    Deterministic,
    /// Constrained profile: propagate, stop, and bounded restart-same only.
    Constrained,
}

/// Caller-owned usage counter for one admitted action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionUsage<'a> {
    pub kind: SupervisionActionKind,
    pub target: Option<Id<'a>>,
    pub uses: u16,
}

/// Mutable bounded state for one admitted supervisor generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupervisionState {
    pub observations: u16,
    pub decisions: u16,
    pub in_flight: u16,
    pub evidence_events: u16,
    pub next_sequence: u64,
    pub cancelled: bool,
    pub terminal: bool,
}

impl SupervisionState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            observations: 0,
            decisions: 0,
            in_flight: 0,
            evidence_events: 0,
            next_sequence: 0,
            cancelled: false,
            terminal: false,
        }
    }

    pub fn admit_observation(
        &mut self,
        contract: SupervisionContract<'_>,
        observation: TerminalObservation<'_>,
    ) -> Result<SupervisionAdmissionEvidence, SupervisionReason> {
        contract.validate()?;
        observation.budget.validate()?;
        if observation.budget.remaining_evidence_events < 2 {
            return Err(SupervisionReason::EvidenceBudgetExhausted);
        }
        if self.cancelled || self.terminal {
            return Err(SupervisionReason::SupervisorTerminal);
        }
        if observation.semantic_subject != contract.subject
            && !contract.members.contains(&observation.semantic_subject)
        {
            return Err(SupervisionReason::ObservationInvalid);
        }
        if observation.caused_by.len() > usize::from(contract.limits.maximum_cause_depth)
            || observation.budget.deadline_tick
                > observation
                    .budget
                    .now_tick
                    .checked_add(contract.limits.maximum_recovery_ticks)
                    .ok_or(SupervisionReason::DeadlineExpired)?
        {
            return Err(SupervisionReason::ObservationInvalid);
        }
        if self.observations >= contract.limits.maximum_observations {
            return Err(SupervisionReason::ObservationBudgetExhausted);
        }
        if self.in_flight >= contract.limits.maximum_in_flight {
            return Err(SupervisionReason::InFlightLimitReached);
        }
        self.ensure_evidence_slots(contract, 2)?;
        self.observations = self
            .observations
            .checked_add(1)
            .ok_or(SupervisionReason::ObservationBudgetExhausted)?;
        self.in_flight = self
            .in_flight
            .checked_add(1)
            .ok_or(SupervisionReason::InFlightLimitReached)?;
        Ok(SupervisionAdmissionEvidence {
            observed: self.emit(SupervisionEvidenceKind::TerminalObserved)?,
            admitted: self.emit(SupervisionEvidenceKind::ObservationAdmitted)?,
        })
    }

    pub fn apply_decision<'a>(
        &mut self,
        contract: SupervisionContract<'a>,
        profile: SupervisionHostProfile,
        observation: TerminalObservation<'a>,
        decision: SupervisionDecision<'a>,
        usages: &mut [ActionUsage<'a>],
    ) -> Result<DecisionOutcome<'a>, SupervisionReason> {
        contract.validate()?;
        if self.cancelled || self.terminal {
            return Err(SupervisionReason::SupervisorTerminal);
        }
        if self.in_flight == 0
            || (observation.semantic_subject != contract.subject
                && !contract.members.contains(&observation.semantic_subject))
        {
            return Err(SupervisionReason::ObservationInvalid);
        }
        if self.decisions >= contract.limits.maximum_decisions
            || observation.budget.remaining_decisions == 0
        {
            return Err(SupervisionReason::DecisionBudgetExhausted);
        }
        if observation.budget.now_tick >= observation.budget.deadline_tick {
            return Err(SupervisionReason::DeadlineExpired);
        }
        if observation.budget.remaining_evidence_events < 2 {
            return Err(SupervisionReason::EvidenceBudgetExhausted);
        }
        if !profile_supports(profile, decision.kind) {
            return Err(SupervisionReason::UnsupportedProfile);
        }
        let (action_index, admitted) = contract
            .actions
            .iter()
            .copied()
            .enumerate()
            .find(|(_, action)| action.kind == decision.kind && action.target == decision.target)
            .ok_or(SupervisionReason::ActionNotAdmitted)?;
        let action_index =
            u16::try_from(action_index).map_err(|_| SupervisionReason::InvalidContract)?;
        if admitted.requires_new_epoch {
            return Err(SupervisionReason::CandidateEpochRequired);
        }
        if decision.kind == SupervisionActionKind::RetrySame
            && (observation.retry != RetryDeclaration::Idempotent
                || !admitted.permits_effect_replay)
        {
            return Err(SupervisionReason::RetryNotDeclaredIdempotent);
        }
        if matches!(
            decision.kind,
            SupervisionActionKind::RestartSame | SupervisionActionKind::RetrySame
        ) && observation.budget.remaining_attempts == 0
        {
            return Err(SupervisionReason::AttemptBudgetExhausted);
        }
        if decision.kind == SupervisionActionKind::ContinueDeclaredDegradedMode
            && contract.required_behavior
            && !admitted.preserves_required_guarantees
        {
            return Err(SupervisionReason::RequiredGuaranteeWouldWeaken);
        }
        if usages
            .iter()
            .filter(|usage| usage.kind == decision.kind && usage.target == decision.target)
            .count()
            != 1
        {
            return Err(SupervisionReason::UsageStorageMissing);
        }
        let usage = usages
            .iter_mut()
            .find(|usage| usage.kind == decision.kind && usage.target == decision.target)
            .ok_or(SupervisionReason::UsageStorageMissing)?;
        if usage.uses >= admitted.maximum_uses {
            return Err(SupervisionReason::ActionUseBudgetExhausted);
        }
        let next_uses = usage
            .uses
            .checked_add(1)
            .ok_or(SupervisionReason::ActionUseBudgetExhausted)?;
        let next_decisions = self
            .decisions
            .checked_add(1)
            .ok_or(SupervisionReason::DecisionBudgetExhausted)?;
        let next_in_flight = self
            .in_flight
            .checked_sub(1)
            .ok_or(SupervisionReason::ObservationInvalid)?;
        let next_attempt = if matches!(
            decision.kind,
            SupervisionActionKind::RestartSame | SupervisionActionKind::RetrySame
        ) {
            Some(
                observation
                    .attempt
                    .checked_add(1)
                    .ok_or(SupervisionReason::AttemptBudgetExhausted)?,
            )
        } else {
            None
        };
        let timing = decision_timing(contract.limits, observation, decision.kind)?;
        self.ensure_evidence_slots(contract, 2)?;
        // All checks complete before mutable state changes.
        usage.uses = next_uses;
        self.decisions = next_decisions;
        self.in_flight = next_in_flight;
        let accepted =
            self.emit_decision(SupervisionEvidenceKind::DecisionAccepted, action_index)?;
        let consequence =
            self.emit_decision(decision_evidence_kind(decision.kind), action_index)?;
        let affected_scope = match decision.kind {
            SupervisionActionKind::Propagate => SupervisionAffectedScope::Outward,
            SupervisionActionKind::StopScope
                if contract.failure_mode == SupervisionFailureMode::IsolatedOptional =>
            {
                SupervisionAffectedScope::ObservedSubject
            }
            SupervisionActionKind::StopScope
            | SupervisionActionKind::ActivateDeclaredFallback
            | SupervisionActionKind::ContinueDeclaredDegradedMode
            | SupervisionActionKind::RequestOperatorAction => SupervisionAffectedScope::BoundScope,
            SupervisionActionKind::RestartSame | SupervisionActionKind::RetrySame => {
                SupervisionAffectedScope::ObservedSubject
            }
        };
        if decision.kind == SupervisionActionKind::Propagate
            || (decision.kind == SupervisionActionKind::StopScope
                && contract.failure_mode == SupervisionFailureMode::FailTogether)
        {
            self.terminal = true;
        }
        Ok(DecisionOutcome {
            decision,
            next_attempt,
            timing,
            affected_scope,
            accepted,
            consequence,
        })
    }

    pub fn cancel(
        &mut self,
        contract: SupervisionContract<'_>,
    ) -> Result<SupervisionEvidence, SupervisionReason> {
        contract.validate()?;
        if self.terminal {
            return Err(SupervisionReason::SupervisorTerminal);
        }
        self.ensure_evidence_slots(contract, 1)?;
        self.cancelled = true;
        self.in_flight = 0;
        self.emit(SupervisionEvidenceKind::Cancelled)
    }

    pub fn handler_failed(
        &mut self,
        contract: SupervisionContract<'_>,
    ) -> Result<SupervisionEvidence, SupervisionReason> {
        self.finish_handler(contract, SupervisionEvidenceKind::HandlerFailed)
    }

    pub fn handler_timed_out(
        &mut self,
        contract: SupervisionContract<'_>,
        observation: TerminalObservation<'_>,
        now_tick: u64,
    ) -> Result<SupervisionEvidence, SupervisionReason> {
        if (observation.semantic_subject != contract.subject
            && !contract.members.contains(&observation.semantic_subject))
            || now_tick < handler_deadline_tick(contract, observation)?
        {
            return Err(SupervisionReason::ObservationInvalid);
        }
        self.finish_handler(contract, SupervisionEvidenceKind::Exhausted)
    }

    pub fn cleanup_failed(
        &mut self,
        contract: SupervisionContract<'_>,
    ) -> Result<SupervisionEvidence, SupervisionReason> {
        self.finish_handler(contract, SupervisionEvidenceKind::CleanupFailed)
    }

    fn finish_handler(
        &mut self,
        contract: SupervisionContract<'_>,
        kind: SupervisionEvidenceKind,
    ) -> Result<SupervisionEvidence, SupervisionReason> {
        contract.validate()?;
        if self.terminal {
            return Err(SupervisionReason::SupervisorTerminal);
        }
        self.ensure_evidence_slots(contract, 1)?;
        self.terminal = true;
        self.in_flight = 0;
        self.emit(kind)
    }

    fn emit(
        &mut self,
        kind: SupervisionEvidenceKind,
    ) -> Result<SupervisionEvidence, SupervisionReason> {
        self.emit_detail(kind, None, None)
    }

    fn emit_decision(
        &mut self,
        kind: SupervisionEvidenceKind,
        action_index: u16,
    ) -> Result<SupervisionEvidence, SupervisionReason> {
        self.emit_detail(kind, Some(action_index), None)
    }

    fn emit_detail(
        &mut self,
        kind: SupervisionEvidenceKind,
        action_index: Option<u16>,
        reason: Option<SupervisionReason>,
    ) -> Result<SupervisionEvidence, SupervisionReason> {
        let sequence = self.next_sequence;
        let next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(SupervisionReason::EvidenceBudgetExhausted)?;
        let next_events = self
            .evidence_events
            .checked_add(1)
            .ok_or(SupervisionReason::EvidenceBudgetExhausted)?;
        self.next_sequence = next_sequence;
        self.evidence_events = next_events;
        Ok(SupervisionEvidence {
            sequence,
            kind,
            action_index,
            reason,
        })
    }

    /// Retain an additional required control/evidence-plane event.
    pub fn record_evidence(
        &mut self,
        contract: SupervisionContract<'_>,
        kind: SupervisionEvidenceKind,
    ) -> Result<SupervisionEvidence, SupervisionReason> {
        self.ensure_evidence_slots(contract, 1)?;
        self.emit(kind)
    }

    /// Retain a rejected decision and its stable reason before returning it.
    pub fn record_rejection(
        &mut self,
        contract: SupervisionContract<'_>,
        decision: Option<SupervisionDecision<'_>>,
        reason: SupervisionReason,
    ) -> Result<SupervisionEvidence, SupervisionReason> {
        self.ensure_evidence_slots(contract, 1)?;
        let action_index = decision
            .and_then(|decision| {
                contract.actions.iter().position(|action| {
                    action.kind == decision.kind && action.target == decision.target
                })
            })
            .and_then(|index| u16::try_from(index).ok());
        self.emit_detail(
            SupervisionEvidenceKind::DecisionRejected,
            action_index,
            Some(reason),
        )
    }

    fn ensure_evidence_slots(
        self,
        contract: SupervisionContract<'_>,
        needed: u16,
    ) -> Result<(), SupervisionReason> {
        if self
            .evidence_events
            .checked_add(needed)
            .is_none_or(|total| total > contract.limits.maximum_evidence_events)
        {
            Err(SupervisionReason::EvidenceBudgetExhausted)
        } else {
            Ok(())
        }
    }
}

impl Default for SupervisionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Exact deadline at which the ordinary handler has exhausted its reserved
/// execution window.
pub fn handler_deadline_tick(
    contract: SupervisionContract<'_>,
    observation: TerminalObservation<'_>,
) -> Result<u64, SupervisionReason> {
    contract.validate()?;
    observation.budget.validate()?;
    observation
        .budget
        .now_tick
        .checked_add(contract.limits.maximum_handler_ticks)
        .map(|deadline| deadline.min(observation.budget.deadline_tick))
        .ok_or(SupervisionReason::DeadlineExpired)
}

/// Accepted decision plus its deterministic new attempt identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecisionOutcome<'a> {
    pub decision: SupervisionDecision<'a>,
    pub next_attempt: Option<u16>,
    pub timing: DecisionTiming,
    pub affected_scope: SupervisionAffectedScope,
    pub accepted: SupervisionEvidence,
    pub consequence: SupervisionEvidence,
}

/// Exact scope affected by an accepted decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisionAffectedScope {
    ObservedSubject,
    BoundScope,
    Outward,
}

/// Exact timer consequences selected with one accepted decision.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DecisionTiming {
    pub attempt_not_before_tick: Option<u64>,
    pub restart_window_deadline_tick: Option<u64>,
    pub cooldown_until_tick: Option<u64>,
    pub operator_deadline_tick: Option<u64>,
}

/// Atomic evidence for terminal observation and handler admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupervisionAdmissionEvidence {
    pub observed: SupervisionEvidence,
    pub admitted: SupervisionEvidence,
}

/// Required immutable evidence vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisionEvidenceKind {
    TerminalObserved,
    ObservationAdmitted,
    DecisionAccepted,
    DecisionRejected,
    AttemptStarted,
    FallbackSelected,
    DegradedSelected,
    OperatorActionRequested,
    Exhausted,
    Propagated,
    CleanupStarted,
    CleanupFailed,
    Cancelled,
    HandlerFailed,
    FinalOutcome,
}

/// One compact evidence item; payload identity is carried by the observation
/// and exact plan binding rather than copied into an unbounded log.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupervisionEvidence {
    pub sequence: u64,
    pub kind: SupervisionEvidenceKind,
    /// Index into the exact plan binding's canonical admitted action set.
    pub action_index: Option<u16>,
    pub reason: Option<SupervisionReason>,
}

/// Stable portable supervision rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisionReason {
    InvalidContract,
    UnboundedContract,
    ObservationInvalid,
    ObservationBudgetExhausted,
    DecisionBudgetExhausted,
    InFlightLimitReached,
    EvidenceBudgetExhausted,
    DeadlineExpired,
    ActionNotAdmitted,
    ActionUseBudgetExhausted,
    RetryNotDeclaredIdempotent,
    AttemptBudgetExhausted,
    RequiredGuaranteeWouldWeaken,
    CandidateEpochRequired,
    AuthorityExpansionForbidden,
    UnsupportedProfile,
    UsageStorageMissing,
    SupervisorTerminal,
    HandlerTimeout,
    CleanupFailed,
}

impl SupervisionReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidContract => "CND-SUP-001",
            Self::UnboundedContract => "CND-SUP-002",
            Self::ObservationInvalid => "CND-SUP-003",
            Self::ObservationBudgetExhausted => "CND-SUP-004",
            Self::DecisionBudgetExhausted => "CND-SUP-005",
            Self::InFlightLimitReached => "CND-SUP-006",
            Self::EvidenceBudgetExhausted => "CND-SUP-007",
            Self::DeadlineExpired | Self::HandlerTimeout => "CND-SUP-008",
            Self::ActionNotAdmitted | Self::ActionUseBudgetExhausted => "CND-SUP-009",
            Self::RetryNotDeclaredIdempotent => "CND-SUP-010",
            Self::AttemptBudgetExhausted => "CND-SUP-011",
            Self::RequiredGuaranteeWouldWeaken => "CND-SUP-012",
            Self::CandidateEpochRequired => "CND-SUP-013",
            Self::AuthorityExpansionForbidden => "CND-SUP-014",
            Self::UnsupportedProfile => "CND-SUP-015",
            Self::UsageStorageMissing => "CND-SUP-016",
            Self::SupervisorTerminal | Self::CleanupFailed => "CND-SUP-017",
        }
    }
}

impl fmt::Display for SupervisionReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidContract => "supervision contract is invalid",
            Self::UnboundedContract => "supervision resources must be positive and finite",
            Self::ObservationInvalid => "terminal observation does not match its binding",
            Self::ObservationBudgetExhausted => "terminal observation budget is exhausted",
            Self::DecisionBudgetExhausted => "supervision decision budget is exhausted",
            Self::InFlightLimitReached => "supervision in-flight limit is reached",
            Self::EvidenceBudgetExhausted => "reserved supervision evidence is exhausted",
            Self::DeadlineExpired => "recovery deadline has expired",
            Self::ActionNotAdmitted => "decision does not select an admitted action",
            Self::ActionUseBudgetExhausted => "admitted action use budget is exhausted",
            Self::RetryNotDeclaredIdempotent => "retry-same requires an idempotent declared effect",
            Self::AttemptBudgetExhausted => "restart or retry attempt budget is exhausted",
            Self::RequiredGuaranteeWouldWeaken => "degraded mode would weaken a required guarantee",
            Self::CandidateEpochRequired => "decision requires a separately admitted plan epoch",
            Self::AuthorityExpansionForbidden => "supervision cannot acquire broader authority",
            Self::UnsupportedProfile => "host profile does not support this supervision action",
            Self::UsageStorageMissing => "caller-owned action usage storage is incomplete",
            Self::SupervisorTerminal => "supervisor is already terminal",
            Self::HandlerTimeout => "supervisor handler timed out",
            Self::CleanupFailed => "recovery cleanup failed",
        })
    }
}

/// Select one racing terminal observation independently of input order.
///
/// Runtime cause precedence matches lifecycle resolution. Stable subject,
/// generation, attempt, and phase break ties.
pub fn select_terminal_observation(
    observations: &[TerminalObservation<'_>],
) -> Result<usize, SupervisionReason> {
    let mut best = observations
        .first()
        .map(|_| 0)
        .ok_or(SupervisionReason::ObservationInvalid)?;
    for index in 1..observations.len() {
        if observation_precedes(observations[index], observations[best]) {
            best = index;
        }
    }
    Ok(best)
}

/// Compare the complete decision-correlation identity retained in
/// caller-owned pending-observation storage.
#[must_use]
pub fn terminal_observations_correlate(
    expected: TerminalObservation<'_>,
    supplied: TerminalObservation<'_>,
) -> bool {
    expected.run == supplied.run
        && expected.plan_identity == supplied.plan_identity
        && expected.plan_epoch == supplied.plan_epoch
        && expected.generation == supplied.generation
        && expected.attempt == supplied.attempt
        && expected.semantic_subject == supplied.semantic_subject
        && expected.expanded_subject == supplied.expanded_subject
}

/// Build the outward terminal observation when a handler itself terminates.
///
/// The caller supplies exact fixed cause storage. The original observation
/// and its complete retained chain are copied before the handler becomes the
/// new subject; no cause or context value is invented.
pub fn outward_handler_observation<'a>(
    observed: TerminalObservation<'a>,
    contract: SupervisionContract<'a>,
    handler_code: TerminalCauseCode,
    handler_phase: TerminalPhase,
    causes: &'a mut [SupervisionCauseRef<'a>],
) -> Result<TerminalObservation<'a>, SupervisionReason> {
    contract.validate()?;
    let needed = observed
        .caused_by
        .len()
        .checked_add(1)
        .ok_or(SupervisionReason::ObservationInvalid)?;
    if needed > causes.len() || needed > usize::from(contract.limits.maximum_cause_depth) {
        return Err(SupervisionReason::ObservationInvalid);
    }
    causes[..observed.caused_by.len()].copy_from_slice(observed.caused_by);
    causes[observed.caused_by.len()] = SupervisionCauseRef {
        code: observed.code,
        subject: observed.semantic_subject,
        generation: observed.generation,
        attempt: observed.attempt,
    };
    Ok(TerminalObservation {
        semantic_subject: contract.handler,
        expanded_subject: contract.handler,
        class: terminal_class(handler_code),
        code: handler_code,
        phase: handler_phase,
        caused_by: &causes[..needed],
        retry: RetryDeclaration::Undeclared,
        context: TerminalContext::default(),
        ..observed
    })
}

/// Pick the nearest explicit boundary from a deterministic inner-to-outer
/// chain. A handler is never allowed to select itself.
pub fn nearest_supervision_boundary<'a>(
    subject: InstancePath<'a>,
    boundary_ids: &'a [Id<'a>],
    contracts: &'a [SupervisionContract<'a>],
) -> Result<Option<&'a SupervisionContract<'a>>, SupervisionReason> {
    for boundary in boundary_ids {
        let contract = contracts
            .iter()
            .find(|candidate| candidate.id == *boundary)
            .ok_or(SupervisionReason::InvalidContract)?;
        contract.validate()?;
        if contract.handler == subject {
            continue;
        }
        return Ok(Some(contract));
    }
    Ok(None)
}

/// Validate explicit inner-to-outer supervision references without allocation.
pub fn validate_supervision_nesting(
    contracts: &[SupervisionContract<'_>],
) -> Result<(), SupervisionReason> {
    for contract in contracts {
        contract.validate()?;
        let mut cursor = contract.outer;
        let mut depth = 0_usize;
        while let Some(outer) = cursor {
            if outer == contract.id
                || depth >= contracts.len()
                || depth >= usize::from(contract.limits.maximum_nested_depth)
            {
                return Err(SupervisionReason::InvalidContract);
            }
            let parent = contracts
                .iter()
                .find(|candidate| candidate.id == outer)
                .ok_or(SupervisionReason::InvalidContract)?;
            cursor = parent.outer;
            depth += 1;
        }
    }
    Ok(())
}

const fn profile_supports(profile: SupervisionHostProfile, action: SupervisionActionKind) -> bool {
    match profile {
        SupervisionHostProfile::Hosted
        | SupervisionHostProfile::Browser
        | SupervisionHostProfile::Deterministic => true,
        SupervisionHostProfile::Constrained => matches!(
            action,
            SupervisionActionKind::Propagate
                | SupervisionActionKind::StopScope
                | SupervisionActionKind::RestartSame
        ),
    }
}

const fn action_shape_invalid(action: AdmittedSupervisionAction<'_>) -> bool {
    let needs_target = matches!(
        action.kind,
        SupervisionActionKind::ActivateDeclaredFallback
            | SupervisionActionKind::ContinueDeclaredDegradedMode
            | SupervisionActionKind::RequestOperatorAction
    );
    needs_target != action.target.is_some()
}

fn observation_precedes(left: TerminalObservation<'_>, right: TerminalObservation<'_>) -> bool {
    cause_rank(left.code)
        .cmp(&cause_rank(right.code))
        .then_with(|| {
            right
                .semantic_subject
                .as_str()
                .cmp(left.semantic_subject.as_str())
        })
        .then_with(|| left.generation.cmp(&right.generation))
        .then_with(|| left.attempt.cmp(&right.attempt))
        .then_with(|| phase_rank(left.phase).cmp(&phase_rank(right.phase)))
        .is_gt()
}

const fn cause_rank(code: TerminalCauseCode) -> u8 {
    match code {
        TerminalCauseCode::NaturalCompletion => 0,
        TerminalCauseCode::TransportDisconnected => 1,
        TerminalCauseCode::CancellationRequested | TerminalCauseCode::ParentCancelled => 2,
        TerminalCauseCode::DeadlineExpired => 3,
        TerminalCauseCode::AuthorityRevoked => 4,
        TerminalCauseCode::NodeFailed => 5,
    }
}

const fn terminal_class(code: TerminalCauseCode) -> TerminalClass {
    match code {
        TerminalCauseCode::NaturalCompletion => TerminalClass::Succeeded,
        TerminalCauseCode::TransportDisconnected => TerminalClass::Disconnected,
        TerminalCauseCode::CancellationRequested | TerminalCauseCode::ParentCancelled => {
            TerminalClass::Cancelled
        }
        TerminalCauseCode::DeadlineExpired
        | TerminalCauseCode::AuthorityRevoked
        | TerminalCauseCode::NodeFailed => TerminalClass::Failed,
    }
}

const fn phase_rank(phase: TerminalPhase) -> u8 {
    match phase {
        TerminalPhase::Prepare => 0,
        TerminalPhase::Start => 1,
        TerminalPhase::Step => 2,
        TerminalPhase::HostOperation => 3,
        TerminalPhase::Drain => 4,
        TerminalPhase::Cleanup => 5,
    }
}

const fn decision_evidence_kind(kind: SupervisionActionKind) -> SupervisionEvidenceKind {
    match kind {
        SupervisionActionKind::Propagate => SupervisionEvidenceKind::Propagated,
        SupervisionActionKind::StopScope => SupervisionEvidenceKind::FinalOutcome,
        SupervisionActionKind::RestartSame | SupervisionActionKind::RetrySame => {
            SupervisionEvidenceKind::AttemptStarted
        }
        SupervisionActionKind::ActivateDeclaredFallback => {
            SupervisionEvidenceKind::FallbackSelected
        }
        SupervisionActionKind::ContinueDeclaredDegradedMode => {
            SupervisionEvidenceKind::DegradedSelected
        }
        SupervisionActionKind::RequestOperatorAction => {
            SupervisionEvidenceKind::OperatorActionRequested
        }
    }
}

fn decision_timing(
    limits: SupervisionLimits,
    observation: TerminalObservation<'_>,
    kind: SupervisionActionKind,
) -> Result<DecisionTiming, SupervisionReason> {
    let bounded_deadline = |delta: u64| {
        observation
            .budget
            .now_tick
            .checked_add(delta)
            .map(|deadline| deadline.min(observation.budget.deadline_tick))
            .ok_or(SupervisionReason::DeadlineExpired)
    };
    match kind {
        SupervisionActionKind::RestartSame | SupervisionActionKind::RetrySame => {
            let attempt_not_before_tick = observation
                .budget
                .now_tick
                .checked_add(limits.backoff_ticks)
                .ok_or(SupervisionReason::DeadlineExpired)?;
            if attempt_not_before_tick >= observation.budget.deadline_tick {
                return Err(SupervisionReason::DeadlineExpired);
            }
            Ok(DecisionTiming {
                attempt_not_before_tick: Some(attempt_not_before_tick),
                restart_window_deadline_tick: Some(bounded_deadline(limits.restart_window_ticks)?),
                ..DecisionTiming::default()
            })
        }
        SupervisionActionKind::ActivateDeclaredFallback
        | SupervisionActionKind::ContinueDeclaredDegradedMode => Ok(DecisionTiming {
            cooldown_until_tick: Some(bounded_deadline(limits.cooldown_ticks)?),
            ..DecisionTiming::default()
        }),
        SupervisionActionKind::RequestOperatorAction => Ok(DecisionTiming {
            operator_deadline_tick: Some(bounded_deadline(limits.operator_wait_ticks)?),
            ..DecisionTiming::default()
        }),
        SupervisionActionKind::Propagate | SupervisionActionKind::StopScope => {
            Ok(DecisionTiming::default())
        }
    }
}
