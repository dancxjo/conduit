//! Bounded transitions between immutable execution-plan epochs.
//!
//! This module owns the host-neutral transaction and evidence contract. It
//! never resolves a host, loads an artifact, provisions a service, or mutates
//! an [`ExecutionPlan`](crate::ExecutionPlan).

use core::convert::Infallible;

use crate::canonical::semantic_hash_with_hash_set;
use crate::{
    ArtifactDigest, CanonicalDescriptor, CanonicalError, CanonicalValue, FieldDisposition, Id,
    InstancePath, MapField, PinnedDescriptor, PlanResourceBudget, ReplacementSupport, SemanticHash,
};

pub const PLAN_TRANSITION_SCHEMA_VERSION: u32 = 0;
pub const MAX_TRANSITION_OPTIONAL_CHANGES: usize = 16;

const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanEpoch {
    pub plan: SemanticHash,
    pub epoch: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionLevel {
    Cold,
    Quiescent,
    Stateful,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionKind {
    ImplementationReplacement,
    PlanModeTransition,
    TerminalFallback,
}

impl TransitionKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ImplementationReplacement => "implementation-replacement",
            Self::PlanModeTransition => "plan-mode-transition",
            Self::TerminalFallback => "terminal-fallback",
        }
    }
}

/// Opaque domain-owned mode policy facts. Conduit pins the exact decision but
/// does not interpret quality, speech, session, or other domain taxonomies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionModeDecision<'a> {
    pub policy: PinnedDescriptor<'a>,
    pub selected_mode: PinnedDescriptor<'a>,
    pub minimum_mode: PinnedDescriptor<'a>,
    pub trigger: PinnedDescriptor<'a>,
    pub authorization: SemanticHash,
}

impl TransitionLevel {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Quiescent => "quiescent",
            Self::Stateful => "stateful",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayGapPolicy {
    Reject,
    Rollback,
    Discontinuity,
}

impl ReplayGapPolicy {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Reject => "reject",
            Self::Rollback => "rollback",
            Self::Discontinuity => "discontinuity",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionStateContract<'a> {
    pub descriptor: PinnedDescriptor<'a>,
    pub maximum_export_bytes: u64,
    pub maximum_import_bytes: u64,
    pub sensitivity: PinnedDescriptor<'a>,
    pub authority: PinnedDescriptor<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionReplayContract<'a> {
    pub stream: PinnedDescriptor<'a>,
    pub stream_epoch: u64,
    pub first_cursor: u64,
    pub maximum_items: u32,
    pub maximum_bytes: u64,
    pub duplicates_permitted: bool,
    pub gap_policy: ReplayGapPolicy,
}

/// Required guarantees are exact identities. Quality/capacity preferences do
/// not belong in this floor and cannot conceal a changed semantic guarantee.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionGuaranteeFloor {
    pub semantic_contract: SemanticHash,
    pub authority: SemanticHash,
    pub sensitivity: SemanticHash,
    pub delivery: SemanticHash,
    pub memory: SemanticHash,
    pub security: SemanticHash,
    pub committedness: SemanticHash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OptionalCharacteristicChange<'a> {
    pub characteristic: PinnedDescriptor<'a>,
    pub old_value: SemanticHash,
    pub new_value: SemanticHash,
    pub weakened: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionBudget {
    pub old: PlanResourceBudget,
    pub candidate: PlanResourceBudget,
    pub rollback: PlanResourceBudget,
    /// Must equal the checked sum of old, candidate, and rollback.
    pub overlap_reserved: PlanResourceBudget,
    pub maximum_in_flight_values: u32,
    pub maximum_pending_operations: u32,
    pub maximum_replay_items: u32,
    pub maximum_replay_bytes: u64,
    pub maximum_state_bytes: u64,
    pub maximum_evidence_records: u16,
    pub maximum_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionRecoveryPolicy {
    pub maximum_attempts: u16,
    pub cooldown_ticks: u64,
    pub hysteresis_ticks: u64,
}

/// Exact transition plan. This identity is distinct from source, either
/// execution plan, run evidence, and presentation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionContract<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub old: PlanEpoch,
    pub candidate: PlanEpoch,
    pub stable_subject: InstancePath<'a>,
    pub old_implementation: PinnedDescriptor<'a>,
    pub candidate_implementation: PinnedDescriptor<'a>,
    pub old_artifact: ArtifactDigest,
    pub candidate_artifact: ArtifactDigest,
    pub kind: TransitionKind,
    pub level: TransitionLevel,
    pub boundary: PinnedDescriptor<'a>,
    pub state: Option<TransitionStateContract<'a>>,
    pub replay: Option<TransitionReplayContract<'a>>,
    pub discontinuity_permitted: bool,
    pub required_floor: TransitionGuaranteeFloor,
    pub candidate_floor: TransitionGuaranteeFloor,
    pub optional_changes: &'a [OptionalCharacteristicChange<'a>],
    pub mode_decision: Option<TransitionModeDecision<'a>>,
    pub budget: TransitionBudget,
    pub recovery: TransitionRecoveryPolicy,
}

impl TransitionContract<'_> {
    #[must_use]
    pub const fn identity_fact_count(&self) -> usize {
        self.optional_changes.len()
    }

    pub fn computed_semantic_hash(
        &self,
        scratch: &mut [SemanticHash],
    ) -> Result<SemanticHash, TransitionIdentityError> {
        if scratch.len() < self.optional_changes.len() {
            return Err(TransitionIdentityError::ScratchTooSmall);
        }
        for (index, change) in self.optional_changes.iter().enumerate() {
            scratch[index] =
                hash_optional_change(*change).map_err(TransitionIdentityError::Canonical)?;
        }
        let old_implementation =
            hash_pin(self.old_implementation).map_err(TransitionIdentityError::Canonical)?;
        let candidate_implementation =
            hash_pin(self.candidate_implementation).map_err(TransitionIdentityError::Canonical)?;
        let boundary = hash_pin(self.boundary).map_err(TransitionIdentityError::Canonical)?;
        let mode_decision = self
            .mode_decision
            .map(hash_mode_decision)
            .transpose()
            .map_err(TransitionIdentityError::Canonical)?;
        let state = self
            .state
            .map(hash_state)
            .transpose()
            .map_err(TransitionIdentityError::Canonical)?;
        let replay = self
            .replay
            .map(hash_replay)
            .transpose()
            .map_err(TransitionIdentityError::Canonical)?;
        let required_floor =
            hash_floor(self.required_floor).map_err(TransitionIdentityError::Canonical)?;
        let candidate_floor =
            hash_floor(self.candidate_floor).map_err(TransitionIdentityError::Canonical)?;
        let budget = hash_budget(self.budget).map_err(TransitionIdentityError::Canonical)?;
        let recovery = hash_recovery(self.recovery).map_err(TransitionIdentityError::Canonical)?;
        let state_value = state.as_ref().map_or(CanonicalValue::Null, |identity| {
            CanonicalValue::Bytes(identity.as_bytes())
        });
        let replay_value = replay.as_ref().map_or(CanonicalValue::Null, |identity| {
            CanonicalValue::Bytes(identity.as_bytes())
        });
        let mode_decision_value = mode_decision
            .as_ref()
            .map_or(CanonicalValue::Null, |identity| {
                CanonicalValue::Bytes(identity.as_bytes())
            });
        let fields = [
            semantic("old_plan", CanonicalValue::Bytes(self.old.plan.as_bytes())),
            semantic(
                "old_epoch",
                CanonicalValue::Integer(i128::from(self.old.epoch)),
            ),
            semantic(
                "candidate_plan",
                CanonicalValue::Bytes(self.candidate.plan.as_bytes()),
            ),
            semantic(
                "candidate_epoch",
                CanonicalValue::Integer(i128::from(self.candidate.epoch)),
            ),
            semantic(
                "stable_subject",
                CanonicalValue::Text(self.stable_subject.as_str()),
            ),
            semantic(
                "old_implementation",
                CanonicalValue::Bytes(old_implementation.as_bytes()),
            ),
            semantic(
                "candidate_implementation",
                CanonicalValue::Bytes(candidate_implementation.as_bytes()),
            ),
            semantic(
                "old_artifact",
                CanonicalValue::Bytes(self.old_artifact.as_bytes()),
            ),
            semantic(
                "candidate_artifact",
                CanonicalValue::Bytes(self.candidate_artifact.as_bytes()),
            ),
            semantic("kind", CanonicalValue::Identifier(Id(self.kind.as_str()))),
            semantic("level", CanonicalValue::Identifier(Id(self.level.as_str()))),
            semantic("boundary", CanonicalValue::Bytes(boundary.as_bytes())),
            semantic("state", state_value),
            semantic("replay", replay_value),
            semantic(
                "discontinuity_permitted",
                CanonicalValue::Boolean(self.discontinuity_permitted),
            ),
            semantic(
                "required_floor",
                CanonicalValue::Bytes(required_floor.as_bytes()),
            ),
            semantic(
                "candidate_floor",
                CanonicalValue::Bytes(candidate_floor.as_bytes()),
            ),
            semantic("mode_decision", mode_decision_value),
            semantic("budget", CanonicalValue::Bytes(budget.as_bytes())),
            semantic("recovery", CanonicalValue::Bytes(recovery.as_bytes())),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/plan-transition"),
            self.schema_version,
            &fields,
            Id("optional_changes"),
            &scratch[..self.optional_changes.len()],
        )
        .map_err(TransitionIdentityError::Canonical)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionIdentityError {
    ScratchTooSmall,
    Canonical(CanonicalError<Infallible>),
}

/// Proof identities are produced by the hosted admission boundary only after
/// the owning validators succeed. They remain separate so one signature,
/// membership, or capability fact cannot stand in for another.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionAdmissionProofs {
    pub request: SemanticHash,
    pub decision: SemanticHash,
    pub authorization: SemanticHash,
    pub candidate_resolution: SemanticHash,
    pub persistent_budget_status: SemanticHash,
    pub hazard_closure: SemanticHash,
    pub inhibit_decision: SemanticHash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionUsage {
    pub overlap: PlanResourceBudget,
    /// Work live when the admission barrier was reserved.
    pub in_flight_values: u32,
    pub pending_operations: u32,
    /// Cumulative, mutually exclusive disposition counts since the barrier.
    pub drained_values: u32,
    pub rejected_values: u32,
    pub lost_values: u32,
    pub completed_operations: u32,
    pub cancelled_operations: u32,
    pub replay_items: u32,
    pub replay_bytes: u64,
    pub duplicate_replay_items: u32,
    pub state_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionDrainObservation {
    pub remaining_values: u32,
    pub remaining_operations: u32,
    pub drained_values: u32,
    pub rejected_values: u32,
    pub lost_values: u32,
    pub completed_operations: u32,
    pub cancelled_operations: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionReplayObservation<'a> {
    pub stream: PinnedDescriptor<'a>,
    pub stream_epoch: u64,
    pub first_cursor: u64,
    pub items: u32,
    pub bytes: u64,
    pub duplicate_items: u32,
    pub gap: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionPhase {
    Requested,
    Reserved,
    Prepared,
    Barrier,
    Draining,
    Transferring,
    Rebinding,
    Committed,
    Retiring,
    Completed,
    RollingBack,
    RolledBack,
    Discontinuous,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionEvidenceKind {
    Requested,
    ResourcesReserved,
    CandidatePrepared,
    AdmissionBarrier,
    WorkDrained,
    StateTransferred,
    InputReplayed,
    DiscontinuityDeclared,
    EndpointRebound,
    Committed,
    OldGenerationRetired,
    Completed,
    RollbackStarted,
    RolledBack,
    RecoverySuppressed,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionEvidence<'a> {
    pub sequence: u32,
    pub tick: u64,
    pub transition: SemanticHash,
    pub old: PlanEpoch,
    pub candidate: PlanEpoch,
    pub active: PlanEpoch,
    pub subject: InstancePath<'a>,
    pub phase: TransitionPhase,
    pub kind: TransitionEvidenceKind,
    pub cause: Option<SemanticHash>,
    pub boundary: Option<PinnedDescriptor<'a>>,
    pub usage: TransitionUsage,
    pub proofs: Option<TransitionAdmissionProofs>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionReason {
    UnsupportedVersion,
    IdentityMismatch,
    InvalidContract,
    ImmutableEpochRequired,
    GuaranteeWeakened,
    ReplacementUnsupported,
    StateContractMismatch,
    ReplayContractMismatch,
    OverlapExceeded,
    EvidenceExhausted,
    IllegalPhase,
    BoundaryMismatch,
    StaleEpoch,
    AdmissionProofMissing,
    AttemptLimit,
    CooldownActive,
    DeadlineExceeded,
    StateTooLarge,
    ReplayGap,
}

impl TransitionReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-TRN-001",
            Self::IdentityMismatch => "CND-TRN-002",
            Self::InvalidContract => "CND-TRN-003",
            Self::ImmutableEpochRequired => "CND-TRN-004",
            Self::GuaranteeWeakened => "CND-TRN-005",
            Self::ReplacementUnsupported => "CND-TRN-006",
            Self::StateContractMismatch => "CND-TRN-007",
            Self::ReplayContractMismatch => "CND-TRN-008",
            Self::OverlapExceeded => "CND-TRN-009",
            Self::EvidenceExhausted => "CND-TRN-010",
            Self::IllegalPhase => "CND-TRN-011",
            Self::BoundaryMismatch => "CND-TRN-012",
            Self::StaleEpoch => "CND-TRN-013",
            Self::AdmissionProofMissing => "CND-TRN-014",
            Self::AttemptLimit => "CND-TRN-015",
            Self::CooldownActive => "CND-TRN-016",
            Self::DeadlineExceeded => "CND-TRN-017",
            Self::StateTooLarge => "CND-TRN-018",
            Self::ReplayGap => "CND-TRN-019",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionController<'a, const EVIDENCE: usize> {
    contract: TransitionContract<'a>,
    phase: TransitionPhase,
    active: PlanEpoch,
    usage: TransitionUsage,
    proofs: Option<TransitionAdmissionProofs>,
    evidence: [Option<TransitionEvidence<'a>>; EVIDENCE],
    evidence_len: usize,
    started_at_tick: u64,
    last_attempt_tick: Option<u64>,
    attempts: u16,
}

impl<'a, const EVIDENCE: usize> TransitionController<'a, EVIDENCE> {
    pub fn new(
        contract: TransitionContract<'a>,
        current: PlanEpoch,
        tick: u64,
        scratch: &mut [SemanticHash],
    ) -> Result<Self, TransitionReason> {
        validate_transition_contract(&contract, scratch)?;
        if current != contract.old {
            return Err(TransitionReason::StaleEpoch);
        }
        if usize::from(contract.budget.maximum_evidence_records) > EVIDENCE {
            return Err(TransitionReason::EvidenceExhausted);
        }
        let mut controller = Self {
            contract,
            phase: TransitionPhase::Requested,
            active: current,
            usage: TransitionUsage {
                overlap: PlanResourceBudget::ZERO,
                in_flight_values: 0,
                pending_operations: 0,
                drained_values: 0,
                rejected_values: 0,
                lost_values: 0,
                completed_operations: 0,
                cancelled_operations: 0,
                replay_items: 0,
                replay_bytes: 0,
                duplicate_replay_items: 0,
                state_bytes: 0,
            },
            proofs: None,
            evidence: [None; EVIDENCE],
            evidence_len: 0,
            started_at_tick: tick,
            last_attempt_tick: None,
            attempts: 0,
        };
        controller.emit(
            tick,
            TransitionPhase::Requested,
            TransitionEvidenceKind::Requested,
            None,
            None,
        )?;
        Ok(controller)
    }

    #[must_use]
    pub const fn contract(&self) -> TransitionContract<'a> {
        self.contract
    }

    #[must_use]
    pub const fn phase(&self) -> TransitionPhase {
        self.phase
    }

    #[must_use]
    pub const fn active_epoch(&self) -> PlanEpoch {
        self.active
    }

    #[must_use]
    pub fn evidence(&self) -> &[Option<TransitionEvidence<'a>>] {
        &self.evidence[..self.evidence_len]
    }

    pub fn reserve(
        &mut self,
        proofs: TransitionAdmissionProofs,
        usage: TransitionUsage,
        tick: u64,
    ) -> Result<(), TransitionReason> {
        self.require_phase(TransitionPhase::Requested)?;
        self.check_time(tick)?;
        if [
            proofs.request,
            proofs.decision,
            proofs.authorization,
            proofs.candidate_resolution,
            proofs.persistent_budget_status,
            proofs.hazard_closure,
            proofs.inhibit_decision,
        ]
        .contains(&ZERO)
        {
            return Err(TransitionReason::AdmissionProofMissing);
        }
        self.check_usage(usage)?;
        if self.attempts >= self.contract.recovery.maximum_attempts {
            return Err(TransitionReason::AttemptLimit);
        }
        if self
            .last_attempt_tick
            .is_some_and(|last| tick < last.saturating_add(self.contract.recovery.cooldown_ticks))
        {
            return Err(TransitionReason::CooldownActive);
        }
        self.ensure_evidence(1)?;
        self.attempts = self.attempts.saturating_add(1);
        self.last_attempt_tick = Some(tick);
        self.proofs = Some(proofs);
        self.usage = usage;
        self.phase = TransitionPhase::Reserved;
        self.emit(
            tick,
            TransitionPhase::Reserved,
            TransitionEvidenceKind::ResourcesReserved,
            None,
            None,
        )
    }

    pub fn prepared(&mut self, tick: u64) -> Result<(), TransitionReason> {
        self.preflight_prepared(tick)?;
        self.advance(
            TransitionPhase::Reserved,
            TransitionPhase::Prepared,
            TransitionEvidenceKind::CandidatePrepared,
            tick,
            None,
            None,
        )
    }

    pub fn preflight_prepared(&self, tick: u64) -> Result<(), TransitionReason> {
        self.require_phase(TransitionPhase::Reserved)?;
        self.check_time(tick)?;
        self.ensure_evidence(1)
    }

    pub fn barrier(
        &mut self,
        boundary: PinnedDescriptor<'a>,
        tick: u64,
    ) -> Result<(), TransitionReason> {
        self.preflight_barrier(boundary, tick)?;
        self.advance(
            TransitionPhase::Prepared,
            TransitionPhase::Barrier,
            TransitionEvidenceKind::AdmissionBarrier,
            tick,
            None,
            Some(boundary),
        )
    }

    pub fn preflight_barrier(
        &self,
        boundary: PinnedDescriptor<'a>,
        tick: u64,
    ) -> Result<(), TransitionReason> {
        if boundary != self.contract.boundary {
            return Err(TransitionReason::BoundaryMismatch);
        }
        self.require_phase(TransitionPhase::Prepared)?;
        self.check_time(tick)?;
        self.ensure_evidence(1)
    }

    pub fn drained(
        &mut self,
        observation: TransitionDrainObservation,
        tick: u64,
    ) -> Result<(), TransitionReason> {
        self.preflight_drained(observation, tick)?;
        self.ensure_evidence(1)?;
        self.usage.drained_values = observation.drained_values;
        self.usage.rejected_values = observation.rejected_values;
        self.usage.lost_values = observation.lost_values;
        self.usage.completed_operations = observation.completed_operations;
        self.usage.cancelled_operations = observation.cancelled_operations;
        self.phase = TransitionPhase::Draining;
        self.emit(
            tick,
            TransitionPhase::Draining,
            TransitionEvidenceKind::WorkDrained,
            None,
            Some(self.contract.boundary),
        )
    }

    pub fn preflight_drained(
        &self,
        observation: TransitionDrainObservation,
        tick: u64,
    ) -> Result<(), TransitionReason> {
        self.require_phase_any(&[TransitionPhase::Barrier, TransitionPhase::Draining])?;
        self.check_time(tick)?;
        let accounted_values = observation
            .remaining_values
            .checked_add(observation.drained_values)
            .and_then(|value| value.checked_add(observation.rejected_values))
            .and_then(|value| value.checked_add(observation.lost_values))
            .ok_or(TransitionReason::OverlapExceeded)?;
        let accounted_operations = observation
            .remaining_operations
            .checked_add(observation.completed_operations)
            .and_then(|value| value.checked_add(observation.cancelled_operations))
            .ok_or(TransitionReason::OverlapExceeded)?;
        if accounted_values != self.usage.in_flight_values
            || accounted_operations != self.usage.pending_operations
            || observation.remaining_values > self.contract.budget.maximum_in_flight_values
            || observation.remaining_operations > self.contract.budget.maximum_pending_operations
        {
            return Err(TransitionReason::OverlapExceeded);
        }
        self.ensure_evidence(1)
    }

    /// Preflight a hosted drain call before it changes backend queues or
    /// completes operations. The resulting exact disposition is validated by
    /// [`Self::drained`] after the call.
    pub fn preflight_drain(&self, tick: u64) -> Result<(), TransitionReason> {
        self.require_phase_any(&[TransitionPhase::Barrier, TransitionPhase::Draining])?;
        self.check_time(tick)?;
        self.ensure_evidence(1)
    }

    pub fn transfer_state(
        &mut self,
        state_contract: PinnedDescriptor<'a>,
        exported_bytes: u64,
        imported_bytes: u64,
        tick: u64,
    ) -> Result<(), TransitionReason> {
        self.preflight_transfer_state(state_contract, exported_bytes, imported_bytes, tick)?;
        let total = exported_bytes
            .checked_add(imported_bytes)
            .ok_or(TransitionReason::StateTooLarge)?;
        self.ensure_evidence(1)?;
        self.usage.state_bytes = total;
        self.phase = TransitionPhase::Transferring;
        self.emit(
            tick,
            TransitionPhase::Transferring,
            TransitionEvidenceKind::StateTransferred,
            None,
            Some(self.contract.boundary),
        )
    }

    pub fn preflight_transfer_state(
        &self,
        state_contract: PinnedDescriptor<'a>,
        exported_bytes: u64,
        imported_bytes: u64,
        tick: u64,
    ) -> Result<(), TransitionReason> {
        self.require_phase(TransitionPhase::Draining)?;
        let state = self
            .contract
            .state
            .ok_or(TransitionReason::StateContractMismatch)?;
        if self.contract.level != TransitionLevel::Stateful
            || state.descriptor != state_contract
            || exported_bytes > state.maximum_export_bytes
            || imported_bytes > state.maximum_import_bytes
        {
            return Err(TransitionReason::StateContractMismatch);
        }
        let total = exported_bytes
            .checked_add(imported_bytes)
            .ok_or(TransitionReason::StateTooLarge)?;
        if total > self.contract.budget.maximum_state_bytes {
            return Err(TransitionReason::StateTooLarge);
        }
        self.check_time(tick)?;
        self.ensure_evidence(1)
    }

    pub fn replayed(
        &mut self,
        observation: TransitionReplayObservation<'a>,
        tick: u64,
    ) -> Result<(), TransitionReason> {
        self.preflight_replayed(observation, tick)?;
        self.ensure_evidence(1)?;
        self.usage.replay_items = observation.items;
        self.usage.replay_bytes = observation.bytes;
        self.usage.duplicate_replay_items = observation.duplicate_items;
        self.phase = if observation.gap {
            TransitionPhase::Discontinuous
        } else {
            TransitionPhase::Transferring
        };
        self.emit(
            tick,
            self.phase,
            if observation.gap {
                TransitionEvidenceKind::DiscontinuityDeclared
            } else {
                TransitionEvidenceKind::InputReplayed
            },
            None,
            Some(self.contract.boundary),
        )
    }

    pub fn preflight_replayed(
        &self,
        observation: TransitionReplayObservation<'a>,
        tick: u64,
    ) -> Result<(), TransitionReason> {
        self.require_phase_any(&[TransitionPhase::Draining, TransitionPhase::Transferring])?;
        let replay = self
            .contract
            .replay
            .ok_or(TransitionReason::ReplayContractMismatch)?;
        if replay.stream != observation.stream
            || replay.stream_epoch != observation.stream_epoch
            || replay.first_cursor != observation.first_cursor
            || observation.items > replay.maximum_items
            || observation.bytes > replay.maximum_bytes
            || observation.duplicate_items > observation.items
            || (!replay.duplicates_permitted && observation.duplicate_items != 0)
            || observation.items > self.contract.budget.maximum_replay_items
            || observation.bytes > self.contract.budget.maximum_replay_bytes
        {
            return Err(TransitionReason::ReplayContractMismatch);
        }
        if observation.gap && replay.gap_policy != ReplayGapPolicy::Discontinuity {
            return Err(TransitionReason::ReplayGap);
        }
        if observation.gap && !self.contract.discontinuity_permitted {
            return Err(TransitionReason::ReplayGap);
        }
        self.check_time(tick)?;
        self.ensure_evidence(1)
    }

    pub fn declare_discontinuity(
        &mut self,
        cause: SemanticHash,
        tick: u64,
    ) -> Result<(), TransitionReason> {
        self.require_phase_any(&[TransitionPhase::Draining, TransitionPhase::Transferring])?;
        if cause == ZERO || !self.contract.discontinuity_permitted {
            return Err(TransitionReason::InvalidContract);
        }
        self.check_time(tick)?;
        self.ensure_evidence(1)?;
        self.phase = TransitionPhase::Discontinuous;
        self.emit(
            tick,
            TransitionPhase::Discontinuous,
            TransitionEvidenceKind::DiscontinuityDeclared,
            Some(cause),
            Some(self.contract.boundary),
        )
    }

    pub fn rebind(&mut self, tick: u64) -> Result<(), TransitionReason> {
        self.preflight_rebind(tick)?;
        self.advance_any(
            TransitionPhase::Rebinding,
            TransitionEvidenceKind::EndpointRebound,
            tick,
            None,
            Some(self.contract.boundary),
        )
    }

    /// Read-only preflight for a host's atomic stable-boundary switch.
    ///
    /// Hosted adapters call this before touching the endpoint router, then
    /// call [`Self::rebind`] at the same tick after the router succeeds.
    pub fn preflight_rebind(&self, tick: u64) -> Result<(), TransitionReason> {
        self.require_phase_any(&[
            TransitionPhase::Draining,
            TransitionPhase::Transferring,
            TransitionPhase::Discontinuous,
        ])?;
        if self.phase != TransitionPhase::Discontinuous
            && (self.usage.in_flight_values
                != self
                    .usage
                    .drained_values
                    .saturating_add(self.usage.rejected_values)
                    .saturating_add(self.usage.lost_values)
                || self.usage.pending_operations
                    != self
                        .usage
                        .completed_operations
                        .saturating_add(self.usage.cancelled_operations))
        {
            return Err(TransitionReason::IllegalPhase);
        }
        if self.contract.level == TransitionLevel::Stateful
            && self.contract.state.is_some()
            && self.usage.state_bytes == 0
            && self.contract.replay.is_none()
            && self.phase != TransitionPhase::Discontinuous
        {
            return Err(TransitionReason::StateContractMismatch);
        }
        self.check_time(tick)?;
        self.ensure_evidence(1)
    }

    /// Atomic authority switch. The active epoch changes only after evidence
    /// capacity and every phase precondition have passed.
    pub fn commit(&mut self, tick: u64) -> Result<(), TransitionReason> {
        self.preflight_commit(tick)?;
        self.active = self.contract.candidate;
        self.phase = TransitionPhase::Committed;
        self.emit(
            tick,
            TransitionPhase::Committed,
            TransitionEvidenceKind::Committed,
            None,
            Some(self.contract.boundary),
        )
    }

    /// Read-only preflight before a hosted boundary commits persistent
    /// transition budget and flips the active epoch.
    pub fn preflight_commit(&self, tick: u64) -> Result<(), TransitionReason> {
        self.require_phase(TransitionPhase::Rebinding)?;
        self.check_time(tick)?;
        self.ensure_evidence(1)
    }

    pub fn retire_old(&mut self, tick: u64) -> Result<(), TransitionReason> {
        self.preflight_retire_old(tick)?;
        self.advance(
            TransitionPhase::Committed,
            TransitionPhase::Retiring,
            TransitionEvidenceKind::OldGenerationRetired,
            tick,
            None,
            None,
        )
    }

    /// Read-only preflight before a host irreversibly retires the old
    /// generation.
    pub fn preflight_retire_old(&self, tick: u64) -> Result<(), TransitionReason> {
        self.require_phase(TransitionPhase::Committed)?;
        self.check_time(tick)?;
        self.ensure_evidence(1)
    }

    pub fn complete(&mut self, tick: u64) -> Result<(), TransitionReason> {
        self.require_phase(TransitionPhase::Retiring)?;
        self.check_time(tick)?;
        self.ensure_evidence(1)?;
        self.phase = TransitionPhase::Completed;
        self.emit(
            tick,
            TransitionPhase::Completed,
            TransitionEvidenceKind::Completed,
            None,
            None,
        )
    }

    /// Deterministic rollback keeps or restores the old epoch. A failed
    /// pre-commit transition never makes the candidate authoritative.
    pub fn rollback(&mut self, cause: SemanticHash, tick: u64) -> Result<(), TransitionReason> {
        self.preflight_rollback(cause, tick)?;
        self.phase = TransitionPhase::RollingBack;
        self.emit(
            tick,
            TransitionPhase::RollingBack,
            TransitionEvidenceKind::RollbackStarted,
            Some(cause),
            None,
        )?;
        self.active = self.contract.old;
        self.phase = TransitionPhase::RolledBack;
        self.emit(
            tick,
            TransitionPhase::RolledBack,
            TransitionEvidenceKind::RolledBack,
            Some(cause),
            None,
        )
    }

    /// Read-only preflight before host participants abort/restore a
    /// generation. Retirement is intentionally irreversible: after old
    /// retirement starts, failure has a terminal policy outcome rather than a
    /// fabricated rollback.
    pub fn preflight_rollback(
        &self,
        cause: SemanticHash,
        tick: u64,
    ) -> Result<(), TransitionReason> {
        if matches!(
            self.phase,
            TransitionPhase::Requested
                | TransitionPhase::Retiring
                | TransitionPhase::Completed
                | TransitionPhase::RolledBack
                | TransitionPhase::Terminal
        ) || cause == ZERO
        {
            return Err(TransitionReason::IllegalPhase);
        }
        self.check_time(tick)?;
        self.ensure_evidence(2)
    }

    pub fn terminal(&mut self, cause: SemanticHash, tick: u64) -> Result<(), TransitionReason> {
        if matches!(
            self.phase,
            TransitionPhase::Completed | TransitionPhase::RolledBack | TransitionPhase::Terminal
        ) || cause == ZERO
        {
            return Err(TransitionReason::IllegalPhase);
        }
        self.check_time(tick)?;
        self.ensure_evidence(1)?;
        self.phase = TransitionPhase::Terminal;
        self.emit(
            tick,
            TransitionPhase::Terminal,
            TransitionEvidenceKind::Terminal,
            Some(cause),
            None,
        )
    }

    /// Begin another bounded attempt without resetting the persistent attempt
    /// counter or admission proofs. The new attempt still requires a fresh
    /// `reserve` call with current proof identities.
    pub fn retry(&mut self, tick: u64) -> Result<(), TransitionReason> {
        self.require_phase(TransitionPhase::RolledBack)?;
        if self.attempts >= self.contract.recovery.maximum_attempts {
            return Err(TransitionReason::AttemptLimit);
        }
        let separation =
            if self.contract.recovery.cooldown_ticks > self.contract.recovery.hysteresis_ticks {
                self.contract.recovery.cooldown_ticks
            } else {
                self.contract.recovery.hysteresis_ticks
            };
        if self
            .last_attempt_tick
            .is_some_and(|last| tick < last.saturating_add(separation))
        {
            return Err(TransitionReason::CooldownActive);
        }
        self.ensure_evidence(1)?;
        self.phase = TransitionPhase::Requested;
        self.active = self.contract.old;
        self.usage = TransitionUsage {
            overlap: PlanResourceBudget::ZERO,
            in_flight_values: 0,
            pending_operations: 0,
            drained_values: 0,
            rejected_values: 0,
            lost_values: 0,
            completed_operations: 0,
            cancelled_operations: 0,
            replay_items: 0,
            replay_bytes: 0,
            duplicate_replay_items: 0,
            state_bytes: 0,
        };
        self.proofs = None;
        self.started_at_tick = tick;
        self.emit(
            tick,
            TransitionPhase::Requested,
            TransitionEvidenceKind::Requested,
            None,
            None,
        )
    }

    fn check_usage(&self, usage: TransitionUsage) -> Result<(), TransitionReason> {
        if !budget_fits(usage.overlap, self.contract.budget.overlap_reserved)
            || usage.in_flight_values > self.contract.budget.maximum_in_flight_values
            || usage.pending_operations > self.contract.budget.maximum_pending_operations
            || usage.replay_items > self.contract.budget.maximum_replay_items
            || usage.replay_bytes > self.contract.budget.maximum_replay_bytes
            || usage.drained_values != 0
            || usage.rejected_values != 0
            || usage.lost_values != 0
            || usage.completed_operations != 0
            || usage.cancelled_operations != 0
            || usage.duplicate_replay_items != 0
            || usage.state_bytes > self.contract.budget.maximum_state_bytes
        {
            return Err(TransitionReason::OverlapExceeded);
        }
        Ok(())
    }

    fn check_time(&self, tick: u64) -> Result<(), TransitionReason> {
        if tick < self.started_at_tick
            || tick.saturating_sub(self.started_at_tick) > self.contract.budget.maximum_ticks
        {
            Err(TransitionReason::DeadlineExceeded)
        } else {
            Ok(())
        }
    }

    fn require_phase(&self, phase: TransitionPhase) -> Result<(), TransitionReason> {
        if self.phase == phase {
            Ok(())
        } else {
            Err(TransitionReason::IllegalPhase)
        }
    }

    fn require_phase_any(&self, phases: &[TransitionPhase]) -> Result<(), TransitionReason> {
        if phases.contains(&self.phase) {
            Ok(())
        } else {
            Err(TransitionReason::IllegalPhase)
        }
    }

    fn advance(
        &mut self,
        from: TransitionPhase,
        to: TransitionPhase,
        kind: TransitionEvidenceKind,
        tick: u64,
        cause: Option<SemanticHash>,
        boundary: Option<PinnedDescriptor<'a>>,
    ) -> Result<(), TransitionReason> {
        self.require_phase(from)?;
        self.advance_any(to, kind, tick, cause, boundary)
    }

    fn advance_any(
        &mut self,
        to: TransitionPhase,
        kind: TransitionEvidenceKind,
        tick: u64,
        cause: Option<SemanticHash>,
        boundary: Option<PinnedDescriptor<'a>>,
    ) -> Result<(), TransitionReason> {
        self.check_time(tick)?;
        self.ensure_evidence(1)?;
        self.phase = to;
        self.emit(tick, to, kind, cause, boundary)
    }

    fn ensure_evidence(&self, count: usize) -> Result<(), TransitionReason> {
        let limit = usize::from(self.contract.budget.maximum_evidence_records);
        if self
            .evidence_len
            .checked_add(count)
            .is_none_or(|needed| needed > EVIDENCE || needed > limit)
        {
            Err(TransitionReason::EvidenceExhausted)
        } else {
            Ok(())
        }
    }

    fn emit(
        &mut self,
        tick: u64,
        phase: TransitionPhase,
        kind: TransitionEvidenceKind,
        cause: Option<SemanticHash>,
        boundary: Option<PinnedDescriptor<'a>>,
    ) -> Result<(), TransitionReason> {
        self.ensure_evidence(1)?;
        let sequence =
            u32::try_from(self.evidence_len).map_err(|_| TransitionReason::EvidenceExhausted)?;
        self.evidence[self.evidence_len] = Some(TransitionEvidence {
            sequence,
            tick,
            transition: self.contract.identity,
            old: self.contract.old,
            candidate: self.contract.candidate,
            active: self.active,
            subject: self.contract.stable_subject,
            phase,
            kind,
            cause,
            boundary,
            usage: self.usage,
            proofs: self.proofs,
        });
        self.evidence_len += 1;
        Ok(())
    }
}

pub fn validate_transition_contract(
    contract: &TransitionContract<'_>,
    scratch: &mut [SemanticHash],
) -> Result<(), TransitionReason> {
    if contract.schema_version != PLAN_TRANSITION_SCHEMA_VERSION {
        return Err(TransitionReason::UnsupportedVersion);
    }
    if contract.identity == ZERO
        || contract.old.plan == ZERO
        || contract.candidate.plan == ZERO
        || contract.old.plan == contract.candidate.plan
        || contract.candidate.epoch <= contract.old.epoch
        || contract.stable_subject.as_str().is_empty()
        || !valid_pin(contract.old_implementation)
        || !valid_pin(contract.candidate_implementation)
        || contract.old_artifact.as_bytes() == &[0; 32]
        || contract.candidate_artifact.as_bytes() == &[0; 32]
        || !valid_pin(contract.boundary)
        || contract.optional_changes.len() > MAX_TRANSITION_OPTIONAL_CHANGES
        || contract.recovery.maximum_attempts == 0
        || contract.budget.maximum_evidence_records == 0
        || contract.budget.maximum_ticks == 0
    {
        return Err(TransitionReason::InvalidContract);
    }
    if contract.required_floor != contract.candidate_floor {
        return Err(TransitionReason::GuaranteeWeakened);
    }
    match (contract.kind, contract.mode_decision) {
        (TransitionKind::ImplementationReplacement, None) => {}
        (TransitionKind::PlanModeTransition, Some(decision))
            if contract
                .optional_changes
                .iter()
                .any(|change| change.weakened)
                && valid_mode_decision(decision) => {}
        (TransitionKind::TerminalFallback, Some(decision)) if valid_mode_decision(decision) => {}
        _ => return Err(TransitionReason::InvalidContract),
    }
    if contract.optional_changes.iter().any(|change| {
        !valid_pin(change.characteristic)
            || change.old_value == ZERO
            || change.new_value == ZERO
            || change.old_value == change.new_value
    }) {
        return Err(TransitionReason::InvalidContract);
    }
    let exact_overlap = checked_budget_add(
        checked_budget_add(contract.budget.old, contract.budget.candidate)
            .ok_or(TransitionReason::OverlapExceeded)?,
        contract.budget.rollback,
    )
    .ok_or(TransitionReason::OverlapExceeded)?;
    if exact_overlap != contract.budget.overlap_reserved {
        return Err(TransitionReason::OverlapExceeded);
    }
    match contract.level {
        TransitionLevel::Cold | TransitionLevel::Quiescent if contract.state.is_some() => {
            return Err(TransitionReason::StateContractMismatch);
        }
        TransitionLevel::Stateful if contract.state.is_none() => {
            return Err(TransitionReason::StateContractMismatch);
        }
        _ => {}
    }
    if let Some(state) = contract.state {
        if !valid_pin(state.descriptor)
            || !valid_pin(state.sensitivity)
            || !valid_pin(state.authority)
            || state.maximum_export_bytes == 0
            || state.maximum_import_bytes == 0
            || state
                .maximum_export_bytes
                .checked_add(state.maximum_import_bytes)
                .is_none_or(|total| total > contract.budget.maximum_state_bytes)
        {
            return Err(TransitionReason::StateContractMismatch);
        }
    }
    if let Some(replay) = contract.replay {
        if !valid_pin(replay.stream)
            || replay.maximum_items == 0
            || replay.maximum_bytes == 0
            || replay.maximum_items > contract.budget.maximum_replay_items
            || replay.maximum_bytes > contract.budget.maximum_replay_bytes
        {
            return Err(TransitionReason::ReplayContractMismatch);
        }
        if replay.gap_policy == ReplayGapPolicy::Discontinuity && !contract.discontinuity_permitted
        {
            return Err(TransitionReason::ReplayContractMismatch);
        }
    }
    let identity = contract
        .computed_semantic_hash(scratch)
        .map_err(|_| TransitionReason::InvalidContract)?;
    if identity != contract.identity {
        return Err(TransitionReason::IdentityMismatch);
    }
    Ok(())
}

/// Validate a manifest's honest replacement capability against one exact
/// transition contract. A stronger-sounding nominal mode is never inferred
/// from matching implementation or port identities.
pub fn validate_replacement_support(
    support: ReplacementSupport<'_>,
    contract: TransitionContract<'_>,
) -> Result<(), TransitionReason> {
    match (contract.level, support) {
        (TransitionLevel::Cold, _) => Ok(()),
        (
            TransitionLevel::Quiescent,
            ReplacementSupport::Quiescent {
                boundary,
                maximum_ticks,
            },
        ) if boundary == contract.boundary && maximum_ticks >= contract.budget.maximum_ticks => {
            Ok(())
        }
        (
            TransitionLevel::Stateful,
            ReplacementSupport::Stateful {
                state_contract,
                maximum_export_bytes,
                maximum_import_bytes,
                maximum_ticks,
            },
        ) => {
            let state = contract
                .state
                .ok_or(TransitionReason::StateContractMismatch)?;
            if state_contract == state.descriptor
                && maximum_export_bytes >= state.maximum_export_bytes
                && maximum_import_bytes >= state.maximum_import_bytes
                && maximum_ticks >= contract.budget.maximum_ticks
            {
                Ok(())
            } else {
                Err(TransitionReason::StateContractMismatch)
            }
        }
        (TransitionLevel::Stateful, _) => Err(TransitionReason::StateContractMismatch),
        _ => Err(TransitionReason::ReplacementUnsupported),
    }
}

fn checked_budget_add(
    left: PlanResourceBudget,
    right: PlanResourceBudget,
) -> Option<PlanResourceBudget> {
    Some(PlanResourceBudget {
        memory_bytes: left.memory_bytes.checked_add(right.memory_bytes)?,
        storage_bytes: left.storage_bytes.checked_add(right.storage_bytes)?,
        cpu_units: left.cpu_units.checked_add(right.cpu_units)?,
        timers: left.timers.checked_add(right.timers)?,
        transports: left.transports.checked_add(right.transports)?,
        checkpoints: left.checkpoints.checked_add(right.checkpoints)?,
        evidence_bytes: left.evidence_bytes.checked_add(right.evidence_bytes)?,
    })
}

fn budget_fits(value: PlanResourceBudget, limit: PlanResourceBudget) -> bool {
    value.memory_bytes <= limit.memory_bytes
        && value.storage_bytes <= limit.storage_bytes
        && value.cpu_units <= limit.cpu_units
        && value.timers <= limit.timers
        && value.transports <= limit.transports
        && value.checkpoints <= limit.checkpoints
        && value.evidence_bytes <= limit.evidence_bytes
}

fn hash_optional_change(
    value: OptionalCharacteristicChange<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let characteristic = hash_pin(value.characteristic)?;
    let fields = [
        semantic(
            "characteristic",
            CanonicalValue::Bytes(characteristic.as_bytes()),
        ),
        semantic(
            "old_value",
            CanonicalValue::Bytes(value.old_value.as_bytes()),
        ),
        semantic(
            "new_value",
            CanonicalValue::Bytes(value.new_value.as_bytes()),
        ),
        semantic("weakened", CanonicalValue::Boolean(value.weakened)),
    ];
    descriptor_hash(Id("conduit/optional-transition-change"), &fields)
}

fn hash_mode_decision(
    value: TransitionModeDecision<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let policy = hash_pin(value.policy)?;
    let selected_mode = hash_pin(value.selected_mode)?;
    let minimum_mode = hash_pin(value.minimum_mode)?;
    let trigger = hash_pin(value.trigger)?;
    let fields = [
        semantic("policy", CanonicalValue::Bytes(policy.as_bytes())),
        semantic(
            "selected_mode",
            CanonicalValue::Bytes(selected_mode.as_bytes()),
        ),
        semantic(
            "minimum_mode",
            CanonicalValue::Bytes(minimum_mode.as_bytes()),
        ),
        semantic("trigger", CanonicalValue::Bytes(trigger.as_bytes())),
        semantic(
            "authorization",
            CanonicalValue::Bytes(value.authorization.as_bytes()),
        ),
    ];
    descriptor_hash(Id("conduit/transition-mode-decision"), &fields)
}

fn valid_mode_decision(value: TransitionModeDecision<'_>) -> bool {
    valid_pin(value.policy)
        && valid_pin(value.selected_mode)
        && valid_pin(value.minimum_mode)
        && valid_pin(value.trigger)
        && value.authorization != ZERO
}

fn hash_state(
    value: TransitionStateContract<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let descriptor = hash_pin(value.descriptor)?;
    let sensitivity = hash_pin(value.sensitivity)?;
    let authority = hash_pin(value.authority)?;
    let fields = [
        semantic("descriptor", CanonicalValue::Bytes(descriptor.as_bytes())),
        semantic(
            "maximum_export_bytes",
            CanonicalValue::Integer(i128::from(value.maximum_export_bytes)),
        ),
        semantic(
            "maximum_import_bytes",
            CanonicalValue::Integer(i128::from(value.maximum_import_bytes)),
        ),
        semantic("sensitivity", CanonicalValue::Bytes(sensitivity.as_bytes())),
        semantic("authority", CanonicalValue::Bytes(authority.as_bytes())),
    ];
    descriptor_hash(Id("conduit/transition-state-contract"), &fields)
}

fn hash_replay(
    value: TransitionReplayContract<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let stream = hash_pin(value.stream)?;
    let fields = [
        semantic("stream", CanonicalValue::Bytes(stream.as_bytes())),
        semantic(
            "stream_epoch",
            CanonicalValue::Integer(i128::from(value.stream_epoch)),
        ),
        semantic(
            "first_cursor",
            CanonicalValue::Integer(i128::from(value.first_cursor)),
        ),
        semantic(
            "maximum_items",
            CanonicalValue::Integer(i128::from(value.maximum_items)),
        ),
        semantic(
            "maximum_bytes",
            CanonicalValue::Integer(i128::from(value.maximum_bytes)),
        ),
        semantic(
            "duplicates_permitted",
            CanonicalValue::Boolean(value.duplicates_permitted),
        ),
        semantic(
            "gap_policy",
            CanonicalValue::Identifier(Id(value.gap_policy.as_str())),
        ),
    ];
    descriptor_hash(Id("conduit/transition-replay-contract"), &fields)
}

fn hash_floor(value: TransitionGuaranteeFloor) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let fields = [
        semantic(
            "semantic_contract",
            CanonicalValue::Bytes(value.semantic_contract.as_bytes()),
        ),
        semantic(
            "authority",
            CanonicalValue::Bytes(value.authority.as_bytes()),
        ),
        semantic(
            "sensitivity",
            CanonicalValue::Bytes(value.sensitivity.as_bytes()),
        ),
        semantic("delivery", CanonicalValue::Bytes(value.delivery.as_bytes())),
        semantic("memory", CanonicalValue::Bytes(value.memory.as_bytes())),
        semantic("security", CanonicalValue::Bytes(value.security.as_bytes())),
        semantic(
            "committedness",
            CanonicalValue::Bytes(value.committedness.as_bytes()),
        ),
    ];
    descriptor_hash(Id("conduit/transition-guarantee-floor"), &fields)
}

fn hash_budget(value: TransitionBudget) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let old = hash_resource_budget(value.old)?;
    let candidate = hash_resource_budget(value.candidate)?;
    let rollback = hash_resource_budget(value.rollback)?;
    let overlap = hash_resource_budget(value.overlap_reserved)?;
    let fields = [
        semantic("old", CanonicalValue::Bytes(old.as_bytes())),
        semantic("candidate", CanonicalValue::Bytes(candidate.as_bytes())),
        semantic("rollback", CanonicalValue::Bytes(rollback.as_bytes())),
        semantic(
            "overlap_reserved",
            CanonicalValue::Bytes(overlap.as_bytes()),
        ),
        semantic(
            "maximum_in_flight_values",
            CanonicalValue::Integer(i128::from(value.maximum_in_flight_values)),
        ),
        semantic(
            "maximum_pending_operations",
            CanonicalValue::Integer(i128::from(value.maximum_pending_operations)),
        ),
        semantic(
            "maximum_replay_items",
            CanonicalValue::Integer(i128::from(value.maximum_replay_items)),
        ),
        semantic(
            "maximum_replay_bytes",
            CanonicalValue::Integer(i128::from(value.maximum_replay_bytes)),
        ),
        semantic(
            "maximum_state_bytes",
            CanonicalValue::Integer(i128::from(value.maximum_state_bytes)),
        ),
        semantic(
            "maximum_evidence_records",
            CanonicalValue::Integer(i128::from(value.maximum_evidence_records)),
        ),
        semantic(
            "maximum_ticks",
            CanonicalValue::Integer(i128::from(value.maximum_ticks)),
        ),
    ];
    descriptor_hash(Id("conduit/transition-budget"), &fields)
}

fn hash_recovery(
    value: TransitionRecoveryPolicy,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let fields = [
        semantic(
            "maximum_attempts",
            CanonicalValue::Integer(i128::from(value.maximum_attempts)),
        ),
        semantic(
            "cooldown_ticks",
            CanonicalValue::Integer(i128::from(value.cooldown_ticks)),
        ),
        semantic(
            "hysteresis_ticks",
            CanonicalValue::Integer(i128::from(value.hysteresis_ticks)),
        ),
    ];
    descriptor_hash(Id("conduit/transition-recovery-policy"), &fields)
}

fn hash_resource_budget(
    value: PlanResourceBudget,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let fields = [
        semantic(
            "memory_bytes",
            CanonicalValue::Integer(i128::from(value.memory_bytes)),
        ),
        semantic(
            "storage_bytes",
            CanonicalValue::Integer(i128::from(value.storage_bytes)),
        ),
        semantic(
            "cpu_units",
            CanonicalValue::Integer(i128::from(value.cpu_units)),
        ),
        semantic("timers", CanonicalValue::Integer(i128::from(value.timers))),
        semantic(
            "transports",
            CanonicalValue::Integer(i128::from(value.transports)),
        ),
        semantic(
            "checkpoints",
            CanonicalValue::Integer(i128::from(value.checkpoints)),
        ),
        semantic(
            "evidence_bytes",
            CanonicalValue::Integer(i128::from(value.evidence_bytes)),
        ),
    ];
    descriptor_hash(Id("conduit/transition-resource-budget"), &fields)
}

fn hash_pin(value: PinnedDescriptor<'_>) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let fields = [
        semantic("id", CanonicalValue::Identifier(value.id)),
        semantic(
            "version",
            CanonicalValue::Integer(i128::from(value.schema_version)),
        ),
        semantic(
            "hash",
            CanonicalValue::Bytes(value.semantic_hash.as_bytes()),
        ),
    ];
    descriptor_hash(Id("conduit/pinned-descriptor"), &fields)
}

fn descriptor_hash(
    kind: Id<'_>,
    fields: &[MapField<'_>],
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    CanonicalDescriptor {
        kind,
        schema_version: 0,
        body: CanonicalValue::Map(fields),
    }
    .semantic_hash()
}

fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        disposition: FieldDisposition::Semantic,
        value,
    }
}

fn valid_pin(value: PinnedDescriptor<'_>) -> bool {
    !value.id.as_str().is_empty() && value.schema_version == 0 && value.semantic_hash != ZERO
}
