//! Allocator-free replicated-composite pool admission and lifecycle contract.
//!
//! The controller owns no executor and performs no allocation. Callers provide
//! fixed storage through const generics and prove that every resource needed by
//! an instance was reserved before a slot changes from queued to live.

use core::convert::Infallible;
use core::fmt;

use crate::canonical::semantic_hash_with_hash_set;
use crate::{
    CanonicalError, CanonicalValue, FieldDisposition, Id, InstancePath, MapField,
    PlanResourceBudget, SemanticHash,
};

/// Current portable replicated-pool contract.
pub const POOL_CONTRACT_SCHEMA_VERSION: u32 = 0;

/// Source-authored admission behavior after exact lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolAdmissionPolicy {
    Reject,
    /// The caller retains the request and may offer it again. The pool creates
    /// no implicit queue entry.
    Block,
    QueueBounded,
    Fail,
}

/// Source-authored cleanup behavior after exact lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolCleanupPolicy {
    Drain,
    Abort,
}

/// Source-authored supervision behavior after exact lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolSupervisionPolicy<'a> {
    FailTogether,
    Isolate,
    RestartBounded {
        maximum_attempts: u16,
        backoff_ticks: u64,
    },
    Fallback {
        target: InstancePath<'a>,
    },
    Escalate,
}

/// Every reservation category charged atomically for one live instance.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PoolReservationProfile {
    pub resources: PlanResourceBudget,
    pub child_nodes: u16,
    pub child_cords: u16,
    pub state_bytes: u64,
    pub scheduler_slots: u16,
    pub host_operations: u16,
    pub cancellation_scopes: u16,
}

impl PoolReservationProfile {
    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            resources: resource_checked_add(self.resources, other.resources)?,
            child_nodes: self.child_nodes.checked_add(other.child_nodes)?,
            child_cords: self.child_cords.checked_add(other.child_cords)?,
            state_bytes: self.state_bytes.checked_add(other.state_bytes)?,
            scheduler_slots: self.scheduler_slots.checked_add(other.scheduler_slots)?,
            host_operations: self.host_operations.checked_add(other.host_operations)?,
            cancellation_scopes: self
                .cancellation_scopes
                .checked_add(other.cancellation_scopes)?,
        })
    }

    #[must_use]
    pub fn checked_mul(self, count: u16) -> Option<Self> {
        Some(Self {
            resources: resource_checked_mul(self.resources, count)?,
            child_nodes: self.child_nodes.checked_mul(count)?,
            child_cords: self.child_cords.checked_mul(count)?,
            state_bytes: self.state_bytes.checked_mul(count as u64)?,
            scheduler_slots: self.scheduler_slots.checked_mul(count)?,
            host_operations: self.host_operations.checked_mul(count)?,
            cancellation_scopes: self.cancellation_scopes.checked_mul(count)?,
        })
    }

    #[must_use]
    pub const fn fits_within(self, available: Self) -> bool {
        resource_fits(self.resources, available.resources)
            && self.child_nodes <= available.child_nodes
            && self.child_cords <= available.child_cords
            && self.state_bytes <= available.state_bytes
            && self.scheduler_slots <= available.scheduler_slots
            && self.host_operations <= available.host_operations
            && self.cancellation_scopes <= available.cancellation_scopes
    }

    #[must_use]
    pub const fn is_nonzero(self) -> bool {
        self.child_nodes > 0
            && self.child_cords > 0
            && self.scheduler_slots > 0
            && self.cancellation_scopes > 0
            && self.resources.memory_bytes > 0
            && self.resources.timers > 0
            && self.resources.evidence_bytes > 0
    }
}

/// Complete exact runtime contract for one finite replicated pool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolContract<'a> {
    pub pool: InstancePath<'a>,
    pub template_hash: SemanticHash,
    pub implementation_set_hash: SemanticHash,
    pub maximum_live: u16,
    pub maximum_queued: u16,
    pub admission: PoolAdmissionPolicy,
    pub supervision: PoolSupervisionPolicy<'a>,
    pub cleanup: PoolCleanupPolicy,
    pub deadline_ticks: u64,
    pub idle_timeout_ticks: u64,
    pub cleanup_ticks: u64,
    pub reservation: PoolReservationProfile,
    /// Includes every live slot, bounded queue slot, evidence structure, and
    /// one old/new/rollback overlap reserve selected by the plan.
    pub total_reservation: PoolReservationProfile,
    pub maximum_evidence_events: u16,
}

impl PoolContract<'_> {
    pub fn validate(self) -> Result<(), PoolError> {
        if self.maximum_live == 0
            || self.deadline_ticks == 0
            || self.idle_timeout_ticks == 0
            || self.cleanup_ticks == 0
            || self.maximum_evidence_events == 0
            || !self.reservation.is_nonzero()
        {
            return Err(PoolError::InvalidContract);
        }
        match self.admission {
            PoolAdmissionPolicy::QueueBounded if self.maximum_queued == 0 => {
                return Err(PoolError::InvalidContract);
            }
            PoolAdmissionPolicy::Reject
            | PoolAdmissionPolicy::Block
            | PoolAdmissionPolicy::Fail
                if self.maximum_queued != 0 =>
            {
                return Err(PoolError::InvalidContract);
            }
            _ => {}
        }
        if let PoolSupervisionPolicy::RestartBounded {
            maximum_attempts,
            backoff_ticks,
        } = self.supervision
        {
            if maximum_attempts == 0 || backoff_ticks == 0 {
                return Err(PoolError::InvalidContract);
            }
        }
        let live = match self.reservation.checked_mul(self.maximum_live) {
            Some(value) => value,
            None => return Err(PoolError::ReservationOverflow),
        };
        if !live.fits_within(self.total_reservation) {
            return Err(PoolError::ReservationExceeded);
        }
        Ok(())
    }
}

/// Stable population generation. It is selected by plan identity and epoch,
/// never by discovery or scheduler order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolGeneration {
    pub plan: SemanticHash,
    pub epoch: u64,
    pub generation: u32,
    pub template_hash: SemanticHash,
}

impl PoolGeneration {
    pub fn identity(
        self,
        pool: InstancePath<'_>,
    ) -> Result<SemanticHash, CanonicalError<Infallible>> {
        hash_identity(
            Id("conduit/pool-generation"),
            &[
                semantic(
                    "plan",
                    CanonicalValue::Bytes(self.plan.as_bytes().as_slice()),
                ),
                semantic("pool", CanonicalValue::Text(pool.as_str())),
                semantic("epoch", CanonicalValue::Integer(i128::from(self.epoch))),
                semantic(
                    "generation",
                    CanonicalValue::Integer(i128::from(self.generation)),
                ),
                semantic(
                    "template",
                    CanonicalValue::Bytes(self.template_hash.as_bytes().as_slice()),
                ),
            ],
        )
    }
}

/// Caller-supplied stable work identity. `request` must be derived from
/// semantic input, not arrival order or wall-clock time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolWorkIdentity {
    pub request: SemanticHash,
    pub work_unit: SemanticHash,
    pub correlation: SemanticHash,
}

/// Exact identity of one concrete pool attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolAttemptIdentity {
    pub generation: SemanticHash,
    pub instance: SemanticHash,
    pub work_unit: SemanticHash,
    pub attempt: u16,
    pub correlation: SemanticHash,
}

impl PoolAttemptIdentity {
    pub fn derive(
        generation: SemanticHash,
        work: PoolWorkIdentity,
        attempt: u16,
    ) -> Result<Self, PoolError> {
        if attempt == 0 {
            return Err(PoolError::InvalidIdentity);
        }
        let instance = hash_identity(
            Id("conduit/pool-instance"),
            &[
                semantic(
                    "generation",
                    CanonicalValue::Bytes(generation.as_bytes().as_slice()),
                ),
                semantic(
                    "request",
                    CanonicalValue::Bytes(work.request.as_bytes().as_slice()),
                ),
            ],
        )
        .map_err(|_| PoolError::InvalidIdentity)?;
        let correlation = hash_identity(
            Id("conduit/pool-correlation"),
            &[
                semantic(
                    "generation",
                    CanonicalValue::Bytes(generation.as_bytes().as_slice()),
                ),
                semantic(
                    "request",
                    CanonicalValue::Bytes(work.request.as_bytes().as_slice()),
                ),
                semantic(
                    "work_unit",
                    CanonicalValue::Bytes(work.work_unit.as_bytes().as_slice()),
                ),
                semantic("attempt", CanonicalValue::Integer(i128::from(attempt))),
                semantic(
                    "caller_correlation",
                    CanonicalValue::Bytes(work.correlation.as_bytes().as_slice()),
                ),
            ],
        )
        .map_err(|_| PoolError::InvalidIdentity)?;
        Ok(Self {
            generation,
            instance,
            work_unit: work.work_unit,
            attempt,
            correlation,
        })
    }
}

/// Plan-visible state of one reserved slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolSlotState {
    Empty,
    Queued,
    Reserved,
    Running,
    Checkpointing,
    RestartBackoff,
    Draining,
    Cleanup,
    Succeeded,
    Cancelled,
    Failed,
}

impl PoolSlotState {
    #[must_use]
    pub const fn is_live(self) -> bool {
        matches!(
            self,
            Self::Reserved
                | Self::Running
                | Self::Checkpointing
                | Self::RestartBackoff
                | Self::Draining
                | Self::Cleanup
        )
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Cancelled | Self::Failed)
    }

    #[must_use]
    pub const fn can_begin_drain(self) -> bool {
        matches!(
            self,
            Self::Reserved | Self::Running | Self::Checkpointing | Self::RestartBackoff
        )
    }

    #[must_use]
    pub const fn can_begin_cleanup(self) -> bool {
        matches!(
            self,
            Self::Queued
                | Self::Reserved
                | Self::Running
                | Self::Checkpointing
                | Self::RestartBackoff
                | Self::Draining
        )
    }
}

/// One fixed-storage slot. Timestamps use the plan's monotonic time basis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolSlot {
    pub state: PoolSlotState,
    pub identity: PoolAttemptIdentity,
    pub work: PoolWorkIdentity,
    pub request: SemanticHash,
    pub admitted_at_tick: u64,
    pub last_progress_tick: u64,
    pub deadline_tick: u64,
    pub wake_tick: u64,
    pub cleanup_deadline_tick: u64,
    pub cause: Option<SemanticHash>,
}

impl PoolSlot {
    const EMPTY: Self = Self {
        state: PoolSlotState::Empty,
        identity: PoolAttemptIdentity {
            generation: SemanticHash::from_bytes([0; 32]),
            instance: SemanticHash::from_bytes([0; 32]),
            work_unit: SemanticHash::from_bytes([0; 32]),
            attempt: 0,
            correlation: SemanticHash::from_bytes([0; 32]),
        },
        work: PoolWorkIdentity {
            request: SemanticHash::from_bytes([0; 32]),
            work_unit: SemanticHash::from_bytes([0; 32]),
            correlation: SemanticHash::from_bytes([0; 32]),
        },
        request: SemanticHash::from_bytes([0; 32]),
        admitted_at_tick: 0,
        last_progress_tick: 0,
        deadline_tick: 0,
        wake_tick: 0,
        cleanup_deadline_tick: 0,
        cause: None,
    };
}

/// Resource and policy facts checked before any partial activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolAdmissionFacts {
    pub authority_granted: bool,
    pub sensitivity_allowed: bool,
    pub template_hash: SemanticHash,
    pub implementation_set_hash: SemanticHash,
    pub available: PoolReservationProfile,
}

/// Explainable result of offering one work item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolAdmissionDisposition {
    Started { slot: u16 },
    Queued { slot: u16 },
    Blocked,
    Rejected(PoolReason),
    Failed(PoolReason),
}

/// Supervision action emitted by a failed instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolFailureDisposition<'a> {
    FailPool,
    Isolated,
    RestartAt { tick: u64, attempt: u16 },
    Fallback(InstancePath<'a>),
    Escalate,
    RestartExhausted,
}

/// Exact reason vocabulary for evidence and stable diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolReason {
    Admitted,
    Started,
    Queued,
    Capacity,
    QueueFull,
    CallerBlocked,
    AdmissionFailed,
    AuthorityDenied,
    SensitivityDenied,
    ImplementationMismatch,
    ReservationUnavailable,
    Progress,
    Pressure,
    Loss,
    Completed,
    Cancelled,
    DeadlineExpired,
    IdleExpired,
    RestartScheduled,
    Restarted,
    RestartExhausted,
    Fallback,
    Escalated,
    CleanupDrain,
    CleanupAbort,
    CleanupExpired,
    CheckpointCompatible,
    CheckpointIncompatible,
    GenerationDraining,
    GenerationRollback,
    GenerationRetired,
    ForeignProfileExceeded,
}

impl PoolReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::Started => "started",
            Self::Queued => "queued",
            Self::Capacity => "capacity",
            Self::QueueFull => "queue-full",
            Self::CallerBlocked => "caller-blocked",
            Self::AdmissionFailed => "admission-failed",
            Self::AuthorityDenied => "authority-denied",
            Self::SensitivityDenied => "sensitivity-denied",
            Self::ImplementationMismatch => "implementation-mismatch",
            Self::ReservationUnavailable => "reservation-unavailable",
            Self::Progress => "progress",
            Self::Pressure => "pressure",
            Self::Loss => "loss",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::DeadlineExpired => "deadline-expired",
            Self::IdleExpired => "idle-expired",
            Self::RestartScheduled => "restart-scheduled",
            Self::Restarted => "restarted",
            Self::RestartExhausted => "restart-exhausted",
            Self::Fallback => "fallback",
            Self::Escalated => "escalated",
            Self::CleanupDrain => "cleanup-drain",
            Self::CleanupAbort => "cleanup-abort",
            Self::CleanupExpired => "cleanup-expired",
            Self::CheckpointCompatible => "checkpoint-compatible",
            Self::CheckpointIncompatible => "checkpoint-incompatible",
            Self::GenerationDraining => "generation-draining",
            Self::GenerationRollback => "generation-rollback",
            Self::GenerationRetired => "generation-retired",
            Self::ForeignProfileExceeded => "foreign-profile-exceeded",
        }
    }
}

/// One normative, bounded pool event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolEvidence {
    pub sequence: u64,
    pub tick: u64,
    pub identity: PoolAttemptIdentity,
    pub from: PoolSlotState,
    pub to: PoolSlotState,
    pub reason: PoolReason,
    pub cause: Option<SemanticHash>,
}

impl PoolEvidence {
    const EMPTY: Self = Self {
        sequence: 0,
        tick: 0,
        identity: PoolSlot::EMPTY.identity,
        from: PoolSlotState::Empty,
        to: PoolSlotState::Empty,
        reason: PoolReason::AdmissionFailed,
        cause: None,
    };
}

/// Population snapshot derived from slots, never from an independent counter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PoolPopulationSnapshot {
    pub queued: u16,
    pub live: u16,
    pub restarting: u16,
    pub retiring: u16,
    pub cleanup: u16,
    pub terminal: u16,
}

/// Fixed-storage portable pool controller.
///
/// `SLOTS` must cover `maximum_live + maximum_queued`; `EVIDENCE` must cover
/// the plan's normative evidence reservation. Mutations fail before changing
/// state when evidence storage is exhausted.
pub struct PoolController<'a, const SLOTS: usize, const EVIDENCE: usize> {
    contract: PoolContract<'a>,
    generation: PoolGeneration,
    generation_identity: SemanticHash,
    slot_limit: usize,
    slots: [PoolSlot; SLOTS],
    evidence: [PoolEvidence; EVIDENCE],
    evidence_len: usize,
    next_sequence: u64,
    fairness_cursor: usize,
    accepting: bool,
}

impl<'a, const SLOTS: usize, const EVIDENCE: usize> PoolController<'a, SLOTS, EVIDENCE> {
    pub fn new(contract: PoolContract<'a>, generation: PoolGeneration) -> Result<Self, PoolError> {
        contract.validate()?;
        let needed = usize::from(contract.maximum_live)
            .checked_add(usize::from(contract.maximum_queued))
            .ok_or(PoolError::ReservationOverflow)?;
        if needed > usize::from(u16::MAX) {
            return Err(PoolError::ReservationOverflow);
        }
        if SLOTS < needed || EVIDENCE < usize::from(contract.maximum_evidence_events) {
            return Err(PoolError::StorageTooSmall);
        }
        if generation.template_hash != contract.template_hash {
            return Err(PoolError::InvalidGeneration);
        }
        let generation_identity = generation
            .identity(contract.pool)
            .map_err(|_| PoolError::InvalidIdentity)?;
        Ok(Self {
            contract,
            generation,
            generation_identity,
            slot_limit: needed,
            slots: [PoolSlot::EMPTY; SLOTS],
            evidence: [PoolEvidence::EMPTY; EVIDENCE],
            evidence_len: 0,
            next_sequence: 1,
            fairness_cursor: 0,
            accepting: true,
        })
    }

    #[must_use]
    pub const fn contract(&self) -> PoolContract<'a> {
        self.contract
    }

    #[must_use]
    pub const fn generation(&self) -> PoolGeneration {
        self.generation
    }

    #[must_use]
    pub const fn generation_identity(&self) -> SemanticHash {
        self.generation_identity
    }

    #[must_use]
    pub fn slots(&self) -> &[PoolSlot] {
        &self.slots[..self.slot_limit]
    }

    #[must_use]
    pub fn evidence(&self) -> &[PoolEvidence] {
        &self.evidence[..self.evidence_len]
    }

    #[must_use]
    pub fn population(&self) -> PoolPopulationSnapshot {
        let mut result = PoolPopulationSnapshot::default();
        for slot in self.slots() {
            match slot.state {
                PoolSlotState::Empty => {}
                PoolSlotState::Queued => result.queued = result.queued.saturating_add(1),
                PoolSlotState::RestartBackoff => {
                    result.live = result.live.saturating_add(1);
                    result.restarting = result.restarting.saturating_add(1);
                }
                PoolSlotState::Draining => {
                    result.live = result.live.saturating_add(1);
                    result.retiring = result.retiring.saturating_add(1);
                }
                PoolSlotState::Cleanup => {
                    result.live = result.live.saturating_add(1);
                    result.cleanup = result.cleanup.saturating_add(1);
                }
                state if state.is_live() => result.live = result.live.saturating_add(1),
                state if state.is_terminal() => {
                    result.terminal = result.terminal.saturating_add(1);
                }
                _ => {}
            }
        }
        result
    }

    pub fn offer(
        &mut self,
        work: PoolWorkIdentity,
        facts: PoolAdmissionFacts,
        now_tick: u64,
    ) -> Result<PoolAdmissionDisposition, PoolError> {
        if !self.accepting {
            let identity = PoolAttemptIdentity::derive(self.generation_identity, work, 1)?;
            self.record(
                identity,
                PoolSlotState::Empty,
                PoolSlotState::Empty,
                PoolReason::GenerationDraining,
                None,
                now_tick,
            )?;
            return Ok(PoolAdmissionDisposition::Rejected(
                PoolReason::GenerationDraining,
            ));
        }
        let denial = self.admission_denial(facts);
        if let Some(reason) = denial {
            return self.non_admission(work, now_tick, reason);
        }
        if self
            .slots()
            .iter()
            .any(|slot| slot.state != PoolSlotState::Empty && slot.request == work.request)
        {
            return Err(PoolError::DuplicateRequest);
        }
        if self.population().live < self.contract.maximum_live {
            return self.start_in_empty_slot(work, now_tick);
        }
        match self.contract.admission {
            PoolAdmissionPolicy::Reject => self.non_admission(work, now_tick, PoolReason::Capacity),
            PoolAdmissionPolicy::Block => {
                self.non_admission(work, now_tick, PoolReason::CallerBlocked)
            }
            PoolAdmissionPolicy::Fail => {
                self.non_admission(work, now_tick, PoolReason::AdmissionFailed)
            }
            PoolAdmissionPolicy::QueueBounded => self.enqueue(work, now_tick),
        }
    }

    pub fn mark_running(&mut self, slot: u16, now_tick: u64) -> Result<(), PoolError> {
        self.transition(slot, PoolSlotState::Running, PoolReason::Started, now_tick)
    }

    pub fn progress(&mut self, slot: u16, now_tick: u64) -> Result<(), PoolError> {
        let index = self.slot_index(slot)?;
        if self.slots[index].state != PoolSlotState::Running {
            return Err(PoolError::IllegalTransition);
        }
        let prior = self.slots[index];
        self.record(
            prior.identity,
            prior.state,
            prior.state,
            PoolReason::Progress,
            prior.cause,
            now_tick,
        )?;
        self.slots[index].last_progress_tick = now_tick;
        Ok(())
    }

    /// Record bounded pressure or loss inside one instance without coupling
    /// another instance or manufacturing a pool-wide capacity decision.
    pub fn observe_pressure(
        &mut self,
        slot: u16,
        loss: bool,
        cause: Option<SemanticHash>,
        now_tick: u64,
    ) -> Result<(), PoolError> {
        let index = self.slot_index(slot)?;
        let prior = self.slots[index];
        if prior.state != PoolSlotState::Running {
            return Err(PoolError::IllegalTransition);
        }
        self.record(
            prior.identity,
            prior.state,
            prior.state,
            if loss {
                PoolReason::Loss
            } else {
                PoolReason::Pressure
            },
            cause,
            now_tick,
        )
    }

    /// Reconcile foreign/native implementation usage against the reservation
    /// charged before activation. An excess becomes terminal evidence; it
    /// never expands the reservation.
    pub fn observe_usage(
        &mut self,
        slot: u16,
        usage: PoolReservationProfile,
        now_tick: u64,
    ) -> Result<bool, PoolError> {
        let index = self.slot_index(slot)?;
        if !self.slots[index].state.can_begin_cleanup()
            || self.slots[index].state == PoolSlotState::Queued
        {
            return Err(PoolError::IllegalTransition);
        }
        if usage.fits_within(self.contract.reservation) {
            return Ok(true);
        }
        self.contain_foreign(slot, None, now_tick)?;
        Ok(false)
    }

    /// Contain a foreign/native implementation contract violation observed by
    /// the executor. No implementation-supplied callback can enlarge or
    /// override this transition.
    pub fn contain_foreign(
        &mut self,
        slot: u16,
        cause: Option<SemanticHash>,
        now_tick: u64,
    ) -> Result<(), PoolError> {
        self.begin_cleanup(slot, PoolReason::ForeignProfileExceeded, cause, now_tick)
    }

    pub fn checkpoint(
        &mut self,
        slot: u16,
        checkpoint_template: SemanticHash,
        now_tick: u64,
    ) -> Result<bool, PoolError> {
        let index = self.slot_index(slot)?;
        if self.slots[index].state != PoolSlotState::Running {
            return Err(PoolError::IllegalTransition);
        }
        if checkpoint_template != self.contract.template_hash {
            let prior = self.slots[index];
            self.record(
                prior.identity,
                prior.state,
                prior.state,
                PoolReason::CheckpointIncompatible,
                prior.cause,
                now_tick,
            )?;
            return Ok(false);
        }
        self.transition(
            slot,
            PoolSlotState::Checkpointing,
            PoolReason::CheckpointCompatible,
            now_tick,
        )?;
        Ok(true)
    }

    pub fn resume(&mut self, slot: u16, now_tick: u64) -> Result<(), PoolError> {
        self.transition(slot, PoolSlotState::Running, PoolReason::Progress, now_tick)
    }

    pub fn complete(&mut self, slot: u16, now_tick: u64) -> Result<(), PoolError> {
        self.begin_cleanup(slot, PoolReason::Completed, None, now_tick)
    }

    pub fn cancel(
        &mut self,
        slot: u16,
        cause: SemanticHash,
        now_tick: u64,
    ) -> Result<(), PoolError> {
        let index = self.slot_index(slot)?;
        if self.slots[index].state == PoolSlotState::Queued {
            return self.transition_with_cause(
                slot,
                PoolSlotState::Cancelled,
                PoolReason::Cancelled,
                Some(cause),
                now_tick,
            );
        }
        self.begin_cleanup(slot, PoolReason::Cancelled, Some(cause), now_tick)
    }

    pub fn fail(
        &mut self,
        slot: u16,
        cause: SemanticHash,
        now_tick: u64,
    ) -> Result<PoolFailureDisposition<'a>, PoolError> {
        let index = self.slot_index(slot)?;
        match self.contract.supervision {
            PoolSupervisionPolicy::FailTogether => {
                self.fail_all(cause, now_tick)?;
                Ok(PoolFailureDisposition::FailPool)
            }
            PoolSupervisionPolicy::Isolate => {
                self.begin_cleanup(slot, PoolReason::AdmissionFailed, Some(cause), now_tick)?;
                Ok(PoolFailureDisposition::Isolated)
            }
            PoolSupervisionPolicy::RestartBounded {
                maximum_attempts,
                backoff_ticks,
            } => {
                let attempt = self.slots[index].identity.attempt;
                if attempt >= maximum_attempts {
                    self.begin_cleanup(slot, PoolReason::RestartExhausted, Some(cause), now_tick)?;
                    return Ok(PoolFailureDisposition::RestartExhausted);
                }
                let wake = now_tick
                    .checked_add(backoff_ticks)
                    .ok_or(PoolError::DeadlineOverflow)?;
                self.transition_with_cause(
                    slot,
                    PoolSlotState::RestartBackoff,
                    PoolReason::RestartScheduled,
                    Some(cause),
                    now_tick,
                )?;
                self.slots[index].wake_tick = wake;
                Ok(PoolFailureDisposition::RestartAt {
                    tick: wake,
                    attempt: attempt + 1,
                })
            }
            PoolSupervisionPolicy::Fallback { target } => {
                self.begin_cleanup(slot, PoolReason::Fallback, Some(cause), now_tick)?;
                Ok(PoolFailureDisposition::Fallback(target))
            }
            PoolSupervisionPolicy::Escalate => {
                self.begin_cleanup(slot, PoolReason::Escalated, Some(cause), now_tick)?;
                Ok(PoolFailureDisposition::Escalate)
            }
        }
    }

    /// Advances exact timers and starts at most one queued item. Starting one
    /// item per tick makes the bounded fairness cursor observable.
    pub fn tick(&mut self, now_tick: u64) -> Result<Option<u16>, PoolError> {
        let mut index = 0;
        while index < self.slot_limit {
            let slot = self.slots[index];
            match slot.state {
                PoolSlotState::Running
                    if now_tick >= slot.deadline_tick
                        || now_tick.saturating_sub(slot.last_progress_tick)
                            >= self.contract.idle_timeout_ticks =>
                {
                    let reason = if now_tick >= slot.deadline_tick {
                        PoolReason::DeadlineExpired
                    } else {
                        PoolReason::IdleExpired
                    };
                    self.begin_cleanup(index as u16, reason, slot.cause, now_tick)?;
                }
                PoolSlotState::RestartBackoff if now_tick >= slot.wake_tick => {
                    let deadline_tick = now_tick
                        .checked_add(self.contract.deadline_ticks)
                        .ok_or(PoolError::DeadlineOverflow)?;
                    let identity = PoolAttemptIdentity::derive(
                        self.generation_identity,
                        slot.work,
                        slot.identity.attempt + 1,
                    )?;
                    self.record(
                        identity,
                        slot.state,
                        PoolSlotState::Reserved,
                        PoolReason::Restarted,
                        slot.cause,
                        now_tick,
                    )?;
                    self.slots[index].identity = identity;
                    self.slots[index].state = PoolSlotState::Reserved;
                    self.slots[index].admitted_at_tick = now_tick;
                    self.slots[index].last_progress_tick = now_tick;
                    self.slots[index].deadline_tick = deadline_tick;
                }
                PoolSlotState::Cleanup if now_tick >= slot.cleanup_deadline_tick => {
                    let terminal = match self.contract.cleanup {
                        PoolCleanupPolicy::Drain
                            if matches!(
                                self.evidence_for(slot.identity).map(|event| event.reason),
                                Some(PoolReason::Completed)
                            ) =>
                        {
                            PoolSlotState::Succeeded
                        }
                        _ if matches!(
                            self.evidence_for(slot.identity).map(|event| event.reason),
                            Some(PoolReason::Cancelled)
                        ) =>
                        {
                            PoolSlotState::Cancelled
                        }
                        _ => PoolSlotState::Failed,
                    };
                    self.transition(index as u16, terminal, PoolReason::CleanupExpired, now_tick)?;
                }
                _ => {}
            }
            index += 1;
        }
        if self.population().live >= self.contract.maximum_live {
            return Ok(None);
        }
        let queued = self.next_queued_slot();
        if let Some(slot) = queued {
            let index = usize::from(slot);
            let prior = self.slots[index];
            let deadline_tick = now_tick
                .checked_add(self.contract.deadline_ticks)
                .ok_or(PoolError::DeadlineOverflow)?;
            self.record(
                prior.identity,
                PoolSlotState::Queued,
                PoolSlotState::Reserved,
                PoolReason::Admitted,
                prior.cause,
                now_tick,
            )?;
            self.slots[index].state = PoolSlotState::Reserved;
            self.slots[index].admitted_at_tick = now_tick;
            self.slots[index].last_progress_tick = now_tick;
            self.slots[index].deadline_tick = deadline_tick;
            self.fairness_cursor = (index + 1) % self.slot_limit;
            return Ok(Some(slot));
        }
        Ok(None)
    }

    /// Stop new admission and mark every live slot for generation retirement.
    pub fn begin_generation_drain(
        &mut self,
        cause: SemanticHash,
        now_tick: u64,
    ) -> Result<(), PoolError> {
        let affected = self
            .slots()
            .iter()
            .filter(|slot| slot.state == PoolSlotState::Queued || slot.state.can_begin_drain())
            .count();
        self.ensure_evidence_capacity(affected)?;
        self.accepting = false;
        for index in 0..self.slot_limit {
            if self.slots[index].state == PoolSlotState::Queued {
                self.transition_with_cause(
                    index as u16,
                    PoolSlotState::Cancelled,
                    PoolReason::GenerationDraining,
                    Some(cause),
                    now_tick,
                )?;
            } else if self.slots[index].state.can_begin_drain() {
                self.transition_with_cause(
                    index as u16,
                    PoolSlotState::Draining,
                    PoolReason::GenerationDraining,
                    Some(cause),
                    now_tick,
                )?;
            }
        }
        Ok(())
    }

    pub fn retire_drained(&mut self, now_tick: u64) -> Result<(), PoolError> {
        let needed = self
            .slots()
            .iter()
            .filter(|slot| slot.state == PoolSlotState::Draining)
            .count()
            .checked_mul(2)
            .ok_or(PoolError::EvidenceExhausted)?;
        self.ensure_evidence_capacity(needed)?;
        for index in 0..self.slot_limit {
            if self.slots[index].state == PoolSlotState::Draining {
                self.begin_cleanup(
                    index as u16,
                    PoolReason::GenerationRetired,
                    self.slots[index].cause,
                    now_tick,
                )?;
            }
        }
        Ok(())
    }

    pub fn rollback_generation(
        &mut self,
        cause: SemanticHash,
        now_tick: u64,
    ) -> Result<(), PoolError> {
        let needed = self
            .slots()
            .iter()
            .map(|slot| {
                if slot.state == PoolSlotState::Queued {
                    1
                } else if slot.state.can_begin_cleanup() {
                    2
                } else {
                    0
                }
            })
            .sum();
        self.ensure_evidence_capacity(needed)?;
        self.accepting = false;
        for index in 0..self.slot_limit {
            if self.slots[index].state.can_begin_cleanup() {
                self.begin_cleanup(
                    index as u16,
                    PoolReason::GenerationRollback,
                    Some(cause),
                    now_tick,
                )?;
            }
        }
        Ok(())
    }

    pub fn reclaim_terminal(&mut self, slot: u16) -> Result<(), PoolError> {
        let index = self.slot_index(slot)?;
        if !self.slots[index].state.is_terminal() {
            return Err(PoolError::IllegalTransition);
        }
        self.slots[index] = PoolSlot::EMPTY;
        Ok(())
    }

    fn admission_denial(&self, facts: PoolAdmissionFacts) -> Option<PoolReason> {
        if facts.template_hash != self.contract.template_hash
            || facts.implementation_set_hash != self.contract.implementation_set_hash
        {
            Some(PoolReason::ImplementationMismatch)
        } else if !facts.authority_granted {
            Some(PoolReason::AuthorityDenied)
        } else if !facts.sensitivity_allowed {
            Some(PoolReason::SensitivityDenied)
        } else if !self.contract.reservation.fits_within(facts.available) {
            Some(PoolReason::ReservationUnavailable)
        } else {
            None
        }
    }

    fn non_admission(
        &mut self,
        work: PoolWorkIdentity,
        now_tick: u64,
        reason: PoolReason,
    ) -> Result<PoolAdmissionDisposition, PoolError> {
        let identity = PoolAttemptIdentity::derive(self.generation_identity, work, 1)?;
        self.record(
            identity,
            PoolSlotState::Empty,
            PoolSlotState::Empty,
            reason,
            None,
            now_tick,
        )?;
        Ok(match self.contract.admission {
            PoolAdmissionPolicy::Fail => PoolAdmissionDisposition::Failed(reason),
            PoolAdmissionPolicy::Block => PoolAdmissionDisposition::Blocked,
            PoolAdmissionPolicy::Reject | PoolAdmissionPolicy::QueueBounded => {
                PoolAdmissionDisposition::Rejected(reason)
            }
        })
    }

    fn start_in_empty_slot(
        &mut self,
        work: PoolWorkIdentity,
        now_tick: u64,
    ) -> Result<PoolAdmissionDisposition, PoolError> {
        let index = self.slots[..self.slot_limit]
            .iter()
            .position(|slot| slot.state == PoolSlotState::Empty)
            .ok_or(PoolError::ReservationDrift)?;
        let identity = PoolAttemptIdentity::derive(self.generation_identity, work, 1)?;
        let deadline_tick = now_tick
            .checked_add(self.contract.deadline_ticks)
            .ok_or(PoolError::DeadlineOverflow)?;
        self.record(
            identity,
            PoolSlotState::Empty,
            PoolSlotState::Reserved,
            PoolReason::Admitted,
            None,
            now_tick,
        )?;
        self.slots[index] = PoolSlot {
            state: PoolSlotState::Reserved,
            identity,
            work,
            request: work.request,
            admitted_at_tick: now_tick,
            last_progress_tick: now_tick,
            deadline_tick,
            wake_tick: 0,
            cleanup_deadline_tick: 0,
            cause: None,
        };
        Ok(PoolAdmissionDisposition::Started { slot: index as u16 })
    }

    fn enqueue(
        &mut self,
        work: PoolWorkIdentity,
        now_tick: u64,
    ) -> Result<PoolAdmissionDisposition, PoolError> {
        if self.population().queued >= self.contract.maximum_queued {
            return self.non_admission(work, now_tick, PoolReason::QueueFull);
        }
        let index = self.slots[..self.slot_limit]
            .iter()
            .position(|slot| slot.state == PoolSlotState::Empty)
            .ok_or(PoolError::ReservationDrift)?;
        let identity = PoolAttemptIdentity::derive(self.generation_identity, work, 1)?;
        self.record(
            identity,
            PoolSlotState::Empty,
            PoolSlotState::Queued,
            PoolReason::Queued,
            None,
            now_tick,
        )?;
        self.slots[index] = PoolSlot {
            state: PoolSlotState::Queued,
            identity,
            work,
            request: work.request,
            admitted_at_tick: 0,
            last_progress_tick: 0,
            deadline_tick: 0,
            wake_tick: 0,
            cleanup_deadline_tick: 0,
            cause: None,
        };
        Ok(PoolAdmissionDisposition::Queued { slot: index as u16 })
    }

    fn begin_cleanup(
        &mut self,
        slot: u16,
        reason: PoolReason,
        cause: Option<SemanticHash>,
        now_tick: u64,
    ) -> Result<(), PoolError> {
        let index = self.slot_index(slot)?;
        let prior = self.slots[index];
        if prior.state == PoolSlotState::Queued {
            return self.transition_with_cause(
                slot,
                PoolSlotState::Cancelled,
                reason,
                cause,
                now_tick,
            );
        }
        if !prior.state.can_begin_cleanup() {
            return Err(PoolError::IllegalTransition);
        }
        let cleanup_deadline_tick = now_tick
            .checked_add(self.contract.cleanup_ticks)
            .ok_or(PoolError::DeadlineOverflow)?;
        self.ensure_evidence_capacity(2)?;
        let cleanup_reason = match self.contract.cleanup {
            PoolCleanupPolicy::Drain => PoolReason::CleanupDrain,
            PoolCleanupPolicy::Abort => PoolReason::CleanupAbort,
        };
        self.record(
            prior.identity,
            prior.state,
            PoolSlotState::Cleanup,
            reason,
            cause,
            now_tick,
        )?;
        self.record(
            prior.identity,
            PoolSlotState::Cleanup,
            PoolSlotState::Cleanup,
            cleanup_reason,
            cause,
            now_tick,
        )?;
        self.slots[index].state = PoolSlotState::Cleanup;
        self.slots[index].cause = cause;
        self.slots[index].cleanup_deadline_tick = cleanup_deadline_tick;
        Ok(())
    }

    fn fail_all(&mut self, cause: SemanticHash, now_tick: u64) -> Result<(), PoolError> {
        let needed = self
            .slots()
            .iter()
            .map(|slot| {
                if slot.state == PoolSlotState::Queued {
                    1
                } else if slot.state.can_begin_cleanup() {
                    2
                } else {
                    0
                }
            })
            .sum();
        self.ensure_evidence_capacity(needed)?;
        for index in 0..self.slot_limit {
            if self.slots[index].state.can_begin_cleanup() {
                self.begin_cleanup(
                    index as u16,
                    PoolReason::AdmissionFailed,
                    Some(cause),
                    now_tick,
                )?;
            }
        }
        Ok(())
    }

    fn transition(
        &mut self,
        slot: u16,
        to: PoolSlotState,
        reason: PoolReason,
        now_tick: u64,
    ) -> Result<(), PoolError> {
        let index = self.slot_index(slot)?;
        self.transition_with_cause(slot, to, reason, self.slots[index].cause, now_tick)
    }

    fn transition_with_cause(
        &mut self,
        slot: u16,
        to: PoolSlotState,
        reason: PoolReason,
        cause: Option<SemanticHash>,
        now_tick: u64,
    ) -> Result<(), PoolError> {
        let index = self.slot_index(slot)?;
        let prior = self.slots[index];
        if !pool_transition_allowed(prior.state, to) {
            return Err(PoolError::IllegalTransition);
        }
        self.record(prior.identity, prior.state, to, reason, cause, now_tick)?;
        self.slots[index].state = to;
        self.slots[index].cause = cause;
        if to == PoolSlotState::Running {
            self.slots[index].last_progress_tick = now_tick;
        }
        Ok(())
    }

    fn record(
        &mut self,
        identity: PoolAttemptIdentity,
        from: PoolSlotState,
        to: PoolSlotState,
        reason: PoolReason,
        cause: Option<SemanticHash>,
        tick: u64,
    ) -> Result<(), PoolError> {
        self.ensure_evidence_capacity(1)?;
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(PoolError::SequenceExhausted)?;
        self.evidence[self.evidence_len] = PoolEvidence {
            sequence,
            tick,
            identity,
            from,
            to,
            reason,
            cause,
        };
        self.evidence_len += 1;
        Ok(())
    }

    fn ensure_evidence_capacity(&self, additional: usize) -> Result<(), PoolError> {
        let needed = self
            .evidence_len
            .checked_add(additional)
            .ok_or(PoolError::EvidenceExhausted)?;
        if needed > EVIDENCE || needed > usize::from(self.contract.maximum_evidence_events) {
            Err(PoolError::EvidenceExhausted)
        } else {
            Ok(())
        }
    }

    fn evidence_for(&self, identity: PoolAttemptIdentity) -> Option<&PoolEvidence> {
        self.evidence()
            .iter()
            .rev()
            .find(|event| event.identity == identity && event.from != event.to)
    }

    fn slot_index(&self, slot: u16) -> Result<usize, PoolError> {
        let index = usize::from(slot);
        if index >= self.slot_limit || self.slots[index].state == PoolSlotState::Empty {
            Err(PoolError::UnknownSlot)
        } else {
            Ok(index)
        }
    }

    fn next_queued_slot(&self) -> Option<u16> {
        if self.slot_limit == 0 {
            return None;
        }
        let mut offset = 0;
        while offset < self.slot_limit {
            let index = (self.fairness_cursor + offset) % self.slot_limit;
            if self.slots[index].state == PoolSlotState::Queued {
                return Some(index as u16);
            }
            offset += 1;
        }
        None
    }
}

/// Exact capacity reserved for simultaneous old, candidate, and rollback
/// generations. #57 owns transition orchestration; this contract makes its
/// overlap finite before either generation starts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolGenerationReservation {
    pub old_maximum_live: u16,
    pub candidate_maximum_live: u16,
    pub rollback_maximum_live: u16,
    pub reserved_slots: u16,
    pub per_instance: PoolReservationProfile,
    pub reserved_resources: PoolReservationProfile,
}

impl PoolGenerationReservation {
    pub fn validate(self) -> Result<(), PoolError> {
        let old_and_candidate = match self
            .old_maximum_live
            .checked_add(self.candidate_maximum_live)
        {
            Some(value) => value,
            None => return Err(PoolError::ReservationOverflow),
        };
        let needed = match old_and_candidate.checked_add(self.rollback_maximum_live) {
            Some(value) => value,
            None => return Err(PoolError::ReservationOverflow),
        };
        if needed == 0 || needed != self.reserved_slots {
            return Err(PoolError::GenerationOverlapExceeded);
        }
        let profile = match self.per_instance.checked_mul(needed) {
            Some(value) => value,
            None => return Err(PoolError::ReservationOverflow),
        };
        if profile != self.reserved_resources {
            return Err(PoolError::GenerationOverlapExceeded);
        }
        Ok(())
    }
}

/// Select the next ready pool without using map or registry iteration order.
#[must_use]
pub fn select_fair_pool(ready: &[bool], cursor: usize) -> Option<usize> {
    if ready.is_empty() {
        return None;
    }
    let start = cursor % ready.len();
    let mut offset = 0;
    while offset < ready.len() {
        let index = (start + offset) % ready.len();
        if ready[index] {
            return Some(index);
        }
        offset += 1;
    }
    None
}

/// Portable pool-contract failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolError {
    InvalidContract,
    StorageTooSmall,
    InvalidGeneration,
    InvalidIdentity,
    DuplicateRequest,
    UnknownSlot,
    IllegalTransition,
    DeadlineOverflow,
    ReservationOverflow,
    ReservationExceeded,
    ReservationDrift,
    EvidenceExhausted,
    SequenceExhausted,
    GenerationOverlapExceeded,
}

impl PoolError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidContract | Self::StorageTooSmall => "CND-POL-001",
            Self::InvalidGeneration | Self::InvalidIdentity | Self::DuplicateRequest => {
                "CND-POL-002"
            }
            Self::UnknownSlot | Self::IllegalTransition => "CND-POL-003",
            Self::DeadlineOverflow => "CND-POL-004",
            Self::ReservationOverflow
            | Self::ReservationExceeded
            | Self::ReservationDrift
            | Self::GenerationOverlapExceeded => "CND-POL-005",
            Self::EvidenceExhausted | Self::SequenceExhausted => "CND-POL-006",
        }
    }
}

impl fmt::Display for PoolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidContract => "replicated-pool contract is invalid or unbounded",
            Self::StorageTooSmall => "fixed pool storage is smaller than the exact plan",
            Self::InvalidGeneration => "pool generation does not match the planned template",
            Self::InvalidIdentity => "pool identity derivation failed",
            Self::DuplicateRequest => "request identity is already present in this generation",
            Self::UnknownSlot => "pool slot is absent",
            Self::IllegalTransition => "pool lifecycle transition is illegal",
            Self::DeadlineOverflow => "pool deadline cannot be represented",
            Self::ReservationOverflow => "pool reservation arithmetic overflowed",
            Self::ReservationExceeded => "pool reservation exceeds the exact plan",
            Self::ReservationDrift => "pool slots do not reconcile with the exact population",
            Self::EvidenceExhausted => "normative pool evidence storage is exhausted",
            Self::SequenceExhausted => "pool evidence sequence is exhausted",
            Self::GenerationOverlapExceeded => {
                "old, candidate, and rollback generations exceed overlap reserve"
            }
        })
    }
}

#[must_use]
pub const fn pool_transition_allowed(from: PoolSlotState, to: PoolSlotState) -> bool {
    matches!(
        (from, to),
        (PoolSlotState::Reserved, PoolSlotState::Running)
            | (PoolSlotState::Running, PoolSlotState::Checkpointing)
            | (PoolSlotState::Checkpointing, PoolSlotState::Running)
            | (PoolSlotState::RestartBackoff, PoolSlotState::Reserved)
            | (PoolSlotState::Queued, PoolSlotState::Reserved)
            | (PoolSlotState::Running, PoolSlotState::RestartBackoff)
            | (PoolSlotState::Reserved, PoolSlotState::RestartBackoff)
            | (PoolSlotState::Running, PoolSlotState::Draining)
            | (PoolSlotState::Reserved, PoolSlotState::Draining)
            | (PoolSlotState::Checkpointing, PoolSlotState::Draining)
            | (PoolSlotState::RestartBackoff, PoolSlotState::Draining)
            | (PoolSlotState::Draining, PoolSlotState::Cleanup)
            | (PoolSlotState::Reserved, PoolSlotState::Cleanup)
            | (PoolSlotState::Running, PoolSlotState::Cleanup)
            | (PoolSlotState::Checkpointing, PoolSlotState::Cleanup)
            | (PoolSlotState::RestartBackoff, PoolSlotState::Cleanup)
            | (PoolSlotState::Queued, PoolSlotState::Cancelled)
            | (PoolSlotState::Cleanup, PoolSlotState::Succeeded)
            | (PoolSlotState::Cleanup, PoolSlotState::Cancelled)
            | (PoolSlotState::Cleanup, PoolSlotState::Failed)
    )
}

fn hash_identity(
    kind: Id<'_>,
    fields: &[MapField<'_>],
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    semantic_hash_with_hash_set(
        kind,
        POOL_CONTRACT_SCHEMA_VERSION,
        fields,
        Id("hashes"),
        &[],
    )
}

const fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

const fn resource_checked_add(
    left: PlanResourceBudget,
    right: PlanResourceBudget,
) -> Option<PlanResourceBudget> {
    Some(PlanResourceBudget {
        memory_bytes: match left.memory_bytes.checked_add(right.memory_bytes) {
            Some(value) => value,
            None => return None,
        },
        storage_bytes: match left.storage_bytes.checked_add(right.storage_bytes) {
            Some(value) => value,
            None => return None,
        },
        cpu_units: match left.cpu_units.checked_add(right.cpu_units) {
            Some(value) => value,
            None => return None,
        },
        timers: match left.timers.checked_add(right.timers) {
            Some(value) => value,
            None => return None,
        },
        transports: match left.transports.checked_add(right.transports) {
            Some(value) => value,
            None => return None,
        },
        checkpoints: match left.checkpoints.checked_add(right.checkpoints) {
            Some(value) => value,
            None => return None,
        },
        evidence_bytes: match left.evidence_bytes.checked_add(right.evidence_bytes) {
            Some(value) => value,
            None => return None,
        },
    })
}

const fn resource_checked_mul(value: PlanResourceBudget, count: u16) -> Option<PlanResourceBudget> {
    Some(PlanResourceBudget {
        memory_bytes: match value.memory_bytes.checked_mul(count as u64) {
            Some(value) => value,
            None => return None,
        },
        storage_bytes: match value.storage_bytes.checked_mul(count as u64) {
            Some(value) => value,
            None => return None,
        },
        cpu_units: match value.cpu_units.checked_mul(count as u32) {
            Some(value) => value,
            None => return None,
        },
        timers: match value.timers.checked_mul(count) {
            Some(value) => value,
            None => return None,
        },
        transports: match value.transports.checked_mul(count) {
            Some(value) => value,
            None => return None,
        },
        checkpoints: match value.checkpoints.checked_mul(count) {
            Some(value) => value,
            None => return None,
        },
        evidence_bytes: match value.evidence_bytes.checked_mul(count as u64) {
            Some(value) => value,
            None => return None,
        },
    })
}

const fn resource_fits(value: PlanResourceBudget, available: PlanResourceBudget) -> bool {
    value.memory_bytes <= available.memory_bytes
        && value.storage_bytes <= available.storage_bytes
        && value.cpu_units <= available.cpu_units
        && value.timers <= available.timers
        && value.transports <= available.transports
        && value.checkpoints <= available.checkpoints
        && value.evidence_bytes <= available.evidence_bytes
}
