//! Persistent policy budgets for cross-epoch administrative effects.
//!
//! A policy budget is governance state owned outside the plan that consumes
//! it. This module is allocator-free and executor-neutral: hosts provide the
//! durable atomic storage, clock, and administrative proof, while core
//! validates exact identities and bounded state transitions.

use core::convert::Infallible;

use crate::canonical::semantic_hash_with_hash_set;
use crate::{
    AdministrativeProof, AuthorityTime, CanonicalDescriptor, CanonicalError, CanonicalValue,
    ContainmentContext, FieldDisposition, Id, MapField, PinnedDescriptor, SemanticHash,
    validate_administrative_proof,
};

pub const POLICY_BUDGET_SCHEMA_VERSION: u32 = 0;
pub const MAX_POLICY_BUDGET_BINDINGS: usize = 8;

/// Durable scope that cannot be replaced by a workload epoch or realm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyBudgetAnchor<'a> {
    Realm(Id<'a>),
    Host(Id<'a>),
    Site(Id<'a>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RollingLimit {
    pub units: u64,
    pub window_ticks: u64,
}

/// At least one limit must be finite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyBudgetLimits {
    pub current_stock: Option<u64>,
    pub rolling: Option<RollingLimit>,
    pub lifetime: Option<u64>,
}

/// Optional finite lease policy. Renewal authority is domain-owned and pinned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyLeaseRule<'a> {
    pub maximum_ticks: u64,
    pub renewal_authority: PinnedDescriptor<'a>,
    pub offline_allowed: bool,
}

/// Exact durable policy. `owner` and `subject` are descriptors rather than
/// workload identities so a new plan, generation, run, or realm cannot replace
/// the governing state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistentBudgetPolicy<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub descriptor: PinnedDescriptor<'a>,
    pub owner: PinnedDescriptor<'a>,
    pub subject: PinnedDescriptor<'a>,
    pub anchor: PolicyBudgetAnchor<'a>,
    pub action: Id<'a>,
    pub resource_class: PinnedDescriptor<'a>,
    pub time_basis: Id<'a>,
    pub limits: PolicyBudgetLimits,
    pub reservation_ttl_ticks: u64,
    pub lease: Option<PolicyLeaseRule<'a>>,
    pub audit_id: Id<'a>,
    pub persistence_profile: PinnedDescriptor<'a>,
    pub maximum_reservations: u16,
    pub maximum_evidence_events: u32,
}

/// Exact plan/run provenance retained only as the consumer of a durable
/// budget. None of these fields participates in selecting the ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyBudgetConsumer<'a> {
    pub realm: Id<'a>,
    pub plan: SemanticHash,
    pub epoch: u64,
    pub generation: u64,
    pub run: Id<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyBudgetAvailability {
    Available,
    Unavailable,
    RetentionGap,
}

/// Fresh projection used during plan resolution. This is explicitly not the
/// authoritative ledger or a recovery source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyBudgetStatus<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub policy_identity: SemanticHash,
    pub ledger: PinnedDescriptor<'a>,
    pub checkpoint: SemanticHash,
    pub sequence: u64,
    pub current_stock: u64,
    pub rolling_window_start: u64,
    pub rolling_committed: u64,
    pub lifetime_committed: u64,
    pub reserved: u64,
    pub evidence_remaining: u32,
    pub availability: PolicyBudgetAvailability,
    pub time_basis: Id<'a>,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
}

/// Exact finite lease. An offline lease is useful only when the policy
/// explicitly permits it and never silently renews.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyBudgetLease<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub policy_identity: SemanticHash,
    pub holder: PinnedDescriptor<'a>,
    pub renewal_authority: PinnedDescriptor<'a>,
    pub time_basis: Id<'a>,
    pub issued_at_tick: u64,
    pub expires_at_tick: u64,
    pub offline: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyBudgetRequest<'a> {
    pub identity: SemanticHash,
    pub correlation: SemanticHash,
    pub policy_identity: SemanticHash,
    pub consumer: PolicyBudgetConsumer<'a>,
    pub action: Id<'a>,
    pub units: u64,
    pub requested_at_tick: u64,
    pub lease: Option<PolicyBudgetLease<'a>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyReservationState {
    Empty,
    Reserved,
    Committed,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyReservation {
    pub identity: SemanticHash,
    pub request_identity: SemanticHash,
    pub correlation: SemanticHash,
    pub units: u64,
    pub sequence: u64,
    pub expires_at_tick: u64,
    pub state: PolicyReservationState,
}

impl PolicyReservation {
    pub const EMPTY: Self = Self {
        identity: SemanticHash::from_bytes([0; 32]),
        request_identity: SemanticHash::from_bytes([0; 32]),
        correlation: SemanticHash::from_bytes([0; 32]),
        units: 0,
        sequence: 0,
        expires_at_tick: 0,
        state: PolicyReservationState::Empty,
    };
}

/// Durable checkpoint. Hosts must atomically persist this complete structure
/// or an equivalent representation before acknowledging a transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyBudgetCheckpoint<const N: usize> {
    pub policy_identity: SemanticHash,
    pub predecessor_checkpoint: SemanticHash,
    pub checkpoint: SemanticHash,
    pub sequence: u64,
    pub retention_floor: u64,
    pub current_stock: u64,
    pub rolling_window_start: u64,
    pub rolling_committed: u64,
    pub lifetime_committed: u64,
    pub evidence_remaining: u32,
    pub reservations: [PolicyReservation; N],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyBudgetTransition {
    Applied,
    Idempotent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyBudgetReason {
    UnsupportedVersion,
    InvalidDescriptor,
    IdentityMismatch,
    PolicyMismatch,
    ActionMismatch,
    StaleStatus,
    LedgerUnavailable,
    CapacityExceeded,
    ReservationConflict,
    ReservationExpired,
    CorrelationConflict,
    TransitionInvalid,
    RecoveryGap,
    StorageExhausted,
    EvidenceExhausted,
    LeaseExpired,
    LeaseMismatch,
    ApprovalRequired,
    BindingSetInvalid,
}

impl PolicyBudgetReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-PBG-001",
            Self::InvalidDescriptor => "CND-PBG-002",
            Self::IdentityMismatch => "CND-PBG-003",
            Self::PolicyMismatch => "CND-PBG-004",
            Self::ActionMismatch => "CND-PBG-005",
            Self::StaleStatus => "CND-PBG-006",
            Self::LedgerUnavailable => "CND-PBG-007",
            Self::CapacityExceeded => "CND-PBG-008",
            Self::ReservationConflict => "CND-PBG-009",
            Self::ReservationExpired => "CND-PBG-010",
            Self::CorrelationConflict => "CND-PBG-011",
            Self::TransitionInvalid => "CND-PBG-012",
            Self::RecoveryGap => "CND-PBG-013",
            Self::StorageExhausted => "CND-PBG-014",
            Self::EvidenceExhausted => "CND-PBG-015",
            Self::LeaseExpired => "CND-PBG-016",
            Self::LeaseMismatch => "CND-PBG-017",
            Self::ApprovalRequired => "CND-PBG-018",
            Self::BindingSetInvalid => "CND-PBG-019",
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "unsupported-policy-budget-version",
            Self::InvalidDescriptor => "invalid-policy-budget-descriptor",
            Self::IdentityMismatch => "policy-budget-identity-mismatch",
            Self::PolicyMismatch => "persistent-policy-budget-mismatch",
            Self::ActionMismatch => "policy-budget-action-mismatch",
            Self::StaleStatus => "persistent-policy-budget-status-stale",
            Self::LedgerUnavailable => "persistent-policy-budget-ledger-unavailable",
            Self::CapacityExceeded => "persistent-policy-budget-denied",
            Self::ReservationConflict => "policy-budget-reservation-conflict",
            Self::ReservationExpired => "policy-budget-reservation-expired",
            Self::CorrelationConflict => "policy-budget-correlation-conflict",
            Self::TransitionInvalid => "policy-budget-transition-invalid",
            Self::RecoveryGap => "policy-budget-recovery-retention-gap",
            Self::StorageExhausted => "policy-budget-storage-exhausted",
            Self::EvidenceExhausted => "policy-budget-evidence-exhausted",
            Self::LeaseExpired => "policy-budget-lease-expired",
            Self::LeaseMismatch => "policy-budget-lease-mismatch",
            Self::ApprovalRequired => "policy-budget-increase-needs-independent-approval",
            Self::BindingSetInvalid => "policy-budget-binding-set-invalid",
        }
    }
}

/// Fixed-capacity authoritative state machine. Plan/run/epoch identities are
/// present only on requests, never on this ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistentBudgetLedger<'a, const N: usize> {
    policy: PersistentBudgetPolicy<'a>,
    checkpoint: PolicyBudgetCheckpoint<N>,
}

impl<'a, const N: usize> PersistentBudgetLedger<'a, N> {
    pub fn new(
        policy: PersistentBudgetPolicy<'a>,
        checkpoint_identity: SemanticHash,
        now_tick: u64,
    ) -> Result<Self, PolicyBudgetReason> {
        validate_policy(policy)?;
        if N == 0 || N > usize::from(policy.maximum_reservations) {
            return Err(PolicyBudgetReason::InvalidDescriptor);
        }
        Self {
            policy,
            checkpoint: PolicyBudgetCheckpoint {
                policy_identity: policy.identity,
                predecessor_checkpoint: checkpoint_identity,
                checkpoint: SemanticHash::from_bytes([0; 32]),
                sequence: 0,
                retention_floor: 0,
                current_stock: 0,
                rolling_window_start: now_tick,
                rolling_committed: 0,
                lifetime_committed: 0,
                evidence_remaining: policy.maximum_evidence_events,
                reservations: [PolicyReservation::EMPTY; N],
            },
        }
        .with_refreshed_checkpoint()
    }

    pub fn recover(
        policy: PersistentBudgetPolicy<'a>,
        checkpoint: PolicyBudgetCheckpoint<N>,
    ) -> Result<Self, PolicyBudgetReason> {
        validate_policy(policy)?;
        if checkpoint.policy_identity != policy.identity
            || checkpoint.checkpoint
                != checkpoint
                    .computed_semantic_hash()
                    .map_err(|_| PolicyBudgetReason::RecoveryGap)?
            || checkpoint.evidence_remaining > policy.maximum_evidence_events
            || checkpoint
                .reservations
                .iter()
                .filter(|slot| slot.state != PolicyReservationState::Empty)
                .count()
                > usize::from(policy.maximum_reservations)
        {
            return Err(PolicyBudgetReason::RecoveryGap);
        }
        let ledger = Self { policy, checkpoint };
        ledger.validate_counters()?;
        Ok(ledger)
    }

    #[must_use]
    pub const fn checkpoint(&self) -> PolicyBudgetCheckpoint<N> {
        self.checkpoint
    }

    pub fn status(
        &self,
        ledger: PinnedDescriptor<'a>,
        now_tick: u64,
        valid_until_tick: u64,
    ) -> Result<PolicyBudgetStatus<'a>, PolicyBudgetReason> {
        if valid_until_tick <= now_tick {
            return Err(PolicyBudgetReason::StaleStatus);
        }
        let mut status = PolicyBudgetStatus {
            schema_version: POLICY_BUDGET_SCHEMA_VERSION,
            identity: SemanticHash::from_bytes([0; 32]),
            policy_identity: self.policy.identity,
            ledger,
            checkpoint: self.checkpoint.checkpoint,
            sequence: self.checkpoint.sequence,
            current_stock: self.checkpoint.current_stock,
            rolling_window_start: self.checkpoint.rolling_window_start,
            rolling_committed: self.checkpoint.rolling_committed,
            lifetime_committed: self.checkpoint.lifetime_committed,
            reserved: self.reserved_units(),
            evidence_remaining: self.checkpoint.evidence_remaining,
            availability: PolicyBudgetAvailability::Available,
            time_basis: self.policy.time_basis,
            observed_at_tick: now_tick,
            valid_until_tick,
        };
        status.identity = status
            .computed_semantic_hash()
            .map_err(|_| PolicyBudgetReason::InvalidDescriptor)?;
        Ok(status)
    }

    pub fn reserve(
        &mut self,
        request: PolicyBudgetRequest<'a>,
        now: AuthorityTime<'a>,
        ledger_available: bool,
    ) -> Result<(PolicyReservation, PolicyBudgetTransition), PolicyBudgetReason> {
        validate_request(self.policy, request, now, ledger_available)?;
        if self.advance_window(now.tick)? {
            self.refresh_checkpoint()?;
        }
        if request.requested_at_tick < self.checkpoint.retention_floor {
            return Err(PolicyBudgetReason::RecoveryGap);
        }
        if let Some(slot) = self.checkpoint.reservations.iter().find(|slot| {
            slot.correlation == request.correlation && slot.state != PolicyReservationState::Empty
        }) {
            if slot.request_identity == request.identity && slot.units == request.units {
                return Ok((*slot, PolicyBudgetTransition::Idempotent));
            }
            return Err(PolicyBudgetReason::CorrelationConflict);
        }
        self.require_evidence()?;
        self.require_capacity(request.units)?;
        let slot_index = self
            .checkpoint
            .reservations
            .iter()
            .position(|slot| slot.state == PolicyReservationState::Empty)
            .ok_or(PolicyBudgetReason::StorageExhausted)?;
        let sequence = self
            .checkpoint
            .sequence
            .checked_add(1)
            .ok_or(PolicyBudgetReason::StorageExhausted)?;
        let expires_at_tick = now
            .tick
            .checked_add(self.policy.reservation_ttl_ticks)
            .ok_or(PolicyBudgetReason::InvalidDescriptor)?;
        let identity = reservation_hash(request.identity, sequence, expires_at_tick)
            .map_err(|_| PolicyBudgetReason::InvalidDescriptor)?;
        self.checkpoint.reservations[slot_index] = PolicyReservation {
            identity,
            request_identity: request.identity,
            correlation: request.correlation,
            units: request.units,
            sequence,
            expires_at_tick,
            state: PolicyReservationState::Reserved,
        };
        self.checkpoint.sequence = sequence;
        self.checkpoint.evidence_remaining -= 1;
        self.refresh_checkpoint()?;
        Ok((
            self.checkpoint.reservations[slot_index],
            PolicyBudgetTransition::Applied,
        ))
    }

    pub fn commit(
        &mut self,
        reservation_identity: SemanticHash,
        now_tick: u64,
    ) -> Result<PolicyBudgetTransition, PolicyBudgetReason> {
        let index = self.find_reservation(reservation_identity)?;
        match self.checkpoint.reservations[index].state {
            PolicyReservationState::Committed => return Ok(PolicyBudgetTransition::Idempotent),
            PolicyReservationState::Reserved => {}
            PolicyReservationState::Released | PolicyReservationState::Empty => {
                return Err(PolicyBudgetReason::TransitionInvalid);
            }
        }
        if now_tick >= self.checkpoint.reservations[index].expires_at_tick {
            self.release_index(index)?;
            return Err(PolicyBudgetReason::ReservationExpired);
        }
        if self.advance_window(now_tick)? {
            self.refresh_checkpoint()?;
        }
        self.require_evidence()?;
        let units = self.checkpoint.reservations[index].units;
        self.checkpoint.current_stock = self
            .checkpoint
            .current_stock
            .checked_add(units)
            .ok_or(PolicyBudgetReason::CapacityExceeded)?;
        self.checkpoint.rolling_committed = self
            .checkpoint
            .rolling_committed
            .checked_add(units)
            .ok_or(PolicyBudgetReason::CapacityExceeded)?;
        self.checkpoint.lifetime_committed = self
            .checkpoint
            .lifetime_committed
            .checked_add(units)
            .ok_or(PolicyBudgetReason::CapacityExceeded)?;
        self.checkpoint.sequence += 1;
        self.checkpoint.evidence_remaining -= 1;
        self.checkpoint.reservations[index].state = PolicyReservationState::Committed;
        self.refresh_checkpoint()?;
        Ok(PolicyBudgetTransition::Applied)
    }

    /// Release an uncommitted reservation or reduce current stock after a
    /// committed subject is removed. Rolling and lifetime consumption remain.
    pub fn release(
        &mut self,
        reservation_identity: SemanticHash,
    ) -> Result<PolicyBudgetTransition, PolicyBudgetReason> {
        let index = self.find_reservation(reservation_identity)?;
        if self.checkpoint.reservations[index].state == PolicyReservationState::Released {
            return Ok(PolicyBudgetTransition::Idempotent);
        }
        self.release_index(index)?;
        Ok(PolicyBudgetTransition::Applied)
    }

    pub fn expire(&mut self, now_tick: u64) -> Result<u16, PolicyBudgetReason> {
        let mut expired = 0_u16;
        for index in 0..N {
            if self.checkpoint.reservations[index].state == PolicyReservationState::Reserved
                && now_tick >= self.checkpoint.reservations[index].expires_at_tick
            {
                self.release_index(index)?;
                expired = expired
                    .checked_add(1)
                    .ok_or(PolicyBudgetReason::StorageExhausted)?;
            }
        }
        Ok(expired)
    }

    /// Bounded compaction may discard terminal correlations only by advancing
    /// an explicit retention floor. Replays before that floor fail closed.
    pub fn compact(&mut self, through_sequence: u64) -> Result<(), PolicyBudgetReason> {
        self.require_evidence()?;
        for slot in &mut self.checkpoint.reservations {
            if slot.sequence <= through_sequence && slot.state == PolicyReservationState::Released {
                *slot = PolicyReservation::EMPTY;
            }
        }
        self.checkpoint.retention_floor = self.checkpoint.retention_floor.max(through_sequence);
        self.checkpoint.sequence += 1;
        self.checkpoint.evidence_remaining -= 1;
        self.refresh_checkpoint()?;
        Ok(())
    }

    fn require_capacity(&self, units: u64) -> Result<(), PolicyBudgetReason> {
        let reserved = self.reserved_units();
        if self
            .policy
            .limits
            .current_stock
            .is_some_and(|limit| exceeds(self.checkpoint.current_stock, reserved, units, limit))
            || self.policy.limits.rolling.is_some_and(|limit| {
                exceeds(
                    self.checkpoint.rolling_committed,
                    reserved,
                    units,
                    limit.units,
                )
            })
            || self.policy.limits.lifetime.is_some_and(|limit| {
                exceeds(self.checkpoint.lifetime_committed, reserved, units, limit)
            })
        {
            return Err(PolicyBudgetReason::CapacityExceeded);
        }
        Ok(())
    }

    fn require_evidence(&self) -> Result<(), PolicyBudgetReason> {
        if self.checkpoint.evidence_remaining == 0 {
            Err(PolicyBudgetReason::EvidenceExhausted)
        } else {
            Ok(())
        }
    }

    fn advance_window(&mut self, now_tick: u64) -> Result<bool, PolicyBudgetReason> {
        if let Some(limit) = self.policy.limits.rolling {
            let end = self
                .checkpoint
                .rolling_window_start
                .checked_add(limit.window_ticks)
                .ok_or(PolicyBudgetReason::InvalidDescriptor)?;
            if now_tick >= end {
                let elapsed = now_tick - self.checkpoint.rolling_window_start;
                let windows = elapsed / limit.window_ticks;
                self.checkpoint.rolling_window_start = self
                    .checkpoint
                    .rolling_window_start
                    .checked_add(windows * limit.window_ticks)
                    .ok_or(PolicyBudgetReason::InvalidDescriptor)?;
                self.checkpoint.rolling_committed = 0;
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn reserved_units(&self) -> u64 {
        self.checkpoint
            .reservations
            .iter()
            .filter(|slot| slot.state == PolicyReservationState::Reserved)
            .fold(0_u64, |total, slot| total.saturating_add(slot.units))
    }

    fn find_reservation(&self, identity: SemanticHash) -> Result<usize, PolicyBudgetReason> {
        self.checkpoint
            .reservations
            .iter()
            .position(|slot| {
                slot.identity == identity && slot.state != PolicyReservationState::Empty
            })
            .ok_or(PolicyBudgetReason::ReservationConflict)
    }

    fn release_index(&mut self, index: usize) -> Result<(), PolicyBudgetReason> {
        self.require_evidence()?;
        if self.checkpoint.reservations[index].state == PolicyReservationState::Committed {
            self.checkpoint.current_stock = self
                .checkpoint
                .current_stock
                .checked_sub(self.checkpoint.reservations[index].units)
                .ok_or(PolicyBudgetReason::TransitionInvalid)?;
        }
        self.checkpoint.reservations[index].state = PolicyReservationState::Released;
        self.checkpoint.sequence += 1;
        self.checkpoint.evidence_remaining -= 1;
        self.refresh_checkpoint()?;
        Ok(())
    }

    fn validate_counters(&self) -> Result<(), PolicyBudgetReason> {
        let mut committed_stock = 0_u64;
        for (index, slot) in self.checkpoint.reservations.iter().enumerate() {
            if slot.state == PolicyReservationState::Empty {
                continue;
            }
            if slot.units == 0
                || slot.sequence == 0
                || slot.sequence > self.checkpoint.sequence
                || self.checkpoint.reservations[..index].iter().any(|prior| {
                    prior.state != PolicyReservationState::Empty
                        && (prior.identity == slot.identity
                            || prior.request_identity == slot.request_identity
                            || prior.correlation == slot.correlation)
                })
            {
                return Err(PolicyBudgetReason::RecoveryGap);
            }
            if slot.state == PolicyReservationState::Committed {
                committed_stock = committed_stock
                    .checked_add(slot.units)
                    .ok_or(PolicyBudgetReason::RecoveryGap)?;
            }
        }
        if self
            .policy
            .limits
            .current_stock
            .is_some_and(|limit| self.checkpoint.current_stock > limit)
            || self
                .policy
                .limits
                .rolling
                .is_some_and(|limit| self.checkpoint.rolling_committed > limit.units)
            || self
                .policy
                .limits
                .lifetime
                .is_some_and(|limit| self.checkpoint.lifetime_committed > limit)
            || committed_stock != self.checkpoint.current_stock
        {
            return Err(PolicyBudgetReason::RecoveryGap);
        }
        Ok(())
    }

    fn with_refreshed_checkpoint(mut self) -> Result<Self, PolicyBudgetReason> {
        self.checkpoint.checkpoint = self
            .checkpoint
            .computed_semantic_hash()
            .map_err(|_| PolicyBudgetReason::InvalidDescriptor)?;
        Ok(self)
    }

    fn refresh_checkpoint(&mut self) -> Result<(), PolicyBudgetReason> {
        self.checkpoint.predecessor_checkpoint = self.checkpoint.checkpoint;
        self.checkpoint.checkpoint = self
            .checkpoint
            .computed_semantic_hash()
            .map_err(|_| PolicyBudgetReason::InvalidDescriptor)?;
        Ok(())
    }
}

impl<const N: usize> PolicyBudgetCheckpoint<N> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let mut reservation_hashes = [SemanticHash::from_bytes([0; 32]); N];
        let mut count = 0;
        for reservation in self
            .reservations
            .iter()
            .filter(|reservation| reservation.state != PolicyReservationState::Empty)
        {
            reservation_hashes[count] = hash_reservation_state(*reservation)?;
            count += 1;
        }
        let fields = [
            semantic(
                "policy_identity",
                CanonicalValue::Bytes(self.policy_identity.as_bytes()),
            ),
            semantic(
                "predecessor_checkpoint",
                CanonicalValue::Bytes(self.predecessor_checkpoint.as_bytes()),
            ),
            semantic(
                "sequence",
                CanonicalValue::Integer(i128::from(self.sequence)),
            ),
            semantic(
                "retention_floor",
                CanonicalValue::Integer(i128::from(self.retention_floor)),
            ),
            semantic(
                "current_stock",
                CanonicalValue::Integer(i128::from(self.current_stock)),
            ),
            semantic(
                "rolling_window_start",
                CanonicalValue::Integer(i128::from(self.rolling_window_start)),
            ),
            semantic(
                "rolling_committed",
                CanonicalValue::Integer(i128::from(self.rolling_committed)),
            ),
            semantic(
                "lifetime_committed",
                CanonicalValue::Integer(i128::from(self.lifetime_committed)),
            ),
            semantic(
                "evidence_remaining",
                CanonicalValue::Integer(i128::from(self.evidence_remaining)),
            ),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/policy-budget-checkpoint"),
            POLICY_BUDGET_SCHEMA_VERSION,
            &fields,
            Id("reservations"),
            &reservation_hashes[..count],
        )
    }
}

pub fn validate_policy_budget_status(
    policy: PersistentBudgetPolicy<'_>,
    status: PolicyBudgetStatus<'_>,
    now: AuthorityTime<'_>,
    required_units: u64,
) -> Result<(), PolicyBudgetReason> {
    validate_policy(policy)?;
    if status.schema_version != POLICY_BUDGET_SCHEMA_VERSION {
        return Err(PolicyBudgetReason::UnsupportedVersion);
    }
    if status.identity
        != status
            .computed_semantic_hash()
            .map_err(|_| PolicyBudgetReason::InvalidDescriptor)?
    {
        return Err(PolicyBudgetReason::IdentityMismatch);
    }
    if status.policy_identity != policy.identity {
        return Err(PolicyBudgetReason::PolicyMismatch);
    }
    if status.availability == PolicyBudgetAvailability::RetentionGap {
        return Err(PolicyBudgetReason::RecoveryGap);
    }
    if status.availability != PolicyBudgetAvailability::Available {
        return Err(PolicyBudgetReason::LedgerUnavailable);
    }
    if status.time_basis != now.basis
        || now.tick < status.observed_at_tick
        || now.tick >= status.valid_until_tick
    {
        return Err(PolicyBudgetReason::StaleStatus);
    }
    if status.evidence_remaining == 0 {
        return Err(PolicyBudgetReason::EvidenceExhausted);
    }
    if policy
        .limits
        .current_stock
        .is_some_and(|limit| exceeds(status.current_stock, status.reserved, required_units, limit))
        || policy.limits.rolling.is_some_and(|limit| {
            exceeds(
                status.rolling_committed,
                status.reserved,
                required_units,
                limit.units,
            )
        })
        || policy.limits.lifetime.is_some_and(|limit| {
            exceeds(
                status.lifetime_committed,
                status.reserved,
                required_units,
                limit,
            )
        })
    {
        return Err(PolicyBudgetReason::CapacityExceeded);
    }
    Ok(())
}

pub fn validate_policy_budget_bindings(
    policies: &[PersistentBudgetPolicy<'_>],
    statuses: &[PolicyBudgetStatus<'_>],
    now: AuthorityTime<'_>,
    required_units: u64,
) -> Result<(), PolicyBudgetReason> {
    if policies.is_empty()
        || policies.len() != statuses.len()
        || policies.len() > MAX_POLICY_BUDGET_BINDINGS
    {
        return Err(PolicyBudgetReason::BindingSetInvalid);
    }
    for index in 0..policies.len() {
        if policies[..index]
            .iter()
            .any(|prior| prior.identity == policies[index].identity)
        {
            return Err(PolicyBudgetReason::BindingSetInvalid);
        }
        validate_policy_budget_status(policies[index], statuses[index], now, required_units)?;
    }
    Ok(())
}

pub fn validate_offline_lease(
    policy: PersistentBudgetPolicy<'_>,
    lease: PolicyBudgetLease<'_>,
    now: AuthorityTime<'_>,
) -> Result<(), PolicyBudgetReason> {
    let rule = policy.lease.ok_or(PolicyBudgetReason::LeaseMismatch)?;
    if !rule.offline_allowed || !lease.offline {
        return Err(PolicyBudgetReason::LedgerUnavailable);
    }
    validate_lease(policy, lease, now)
}

/// Increasing or replacing a governing budget requires the independent
/// administrative proof introduced by the containment contract. Decreases
/// preserving the exact owner, subject, anchor, action, and resource class are
/// monotonic and need no expansion proof.
pub fn validate_policy_budget_replacement(
    old: PersistentBudgetPolicy<'_>,
    new: PersistentBudgetPolicy<'_>,
    proof: Option<AdministrativeProof<'_>>,
    context: ContainmentContext<'_>,
) -> Result<(), PolicyBudgetReason> {
    validate_policy(old)?;
    validate_policy(new)?;
    let same_boundary = old.owner == new.owner
        && old.subject == new.subject
        && old.anchor == new.anchor
        && old.action == new.action
        && old.resource_class == new.resource_class
        && old.time_basis == new.time_basis
        && old.audit_id == new.audit_id
        && old.persistence_profile == new.persistence_profile;
    let no_increase = limit_not_increased(old.limits.current_stock, new.limits.current_stock)
        && rolling_not_increased(old.limits.rolling, new.limits.rolling)
        && limit_not_increased(old.limits.lifetime, new.limits.lifetime)
        && new.reservation_ttl_ticks <= old.reservation_ttl_ticks
        && lease_not_increased(old.lease, new.lease)
        && new.maximum_reservations <= old.maximum_reservations
        && new.maximum_evidence_events <= old.maximum_evidence_events;
    if same_boundary && no_increase {
        return Ok(());
    }
    let proof = proof.ok_or(PolicyBudgetReason::ApprovalRequired)?;
    if proof.proposal.subject.budget != Some(new.descriptor)
        || context.subject.budget != Some(new.descriptor)
    {
        return Err(PolicyBudgetReason::PolicyMismatch);
    }
    validate_administrative_proof(proof, context).map_err(|_| PolicyBudgetReason::ApprovalRequired)
}

impl PersistentBudgetPolicy<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let descriptor = hash_pin(self.descriptor)?;
        let owner = hash_pin(self.owner)?;
        let subject = hash_pin(self.subject)?;
        let resource_class = hash_pin(self.resource_class)?;
        let persistence = hash_pin(self.persistence_profile)?;
        let lease = self.lease.map(hash_lease_rule).transpose()?;
        let (anchor_kind, anchor_id) = match self.anchor {
            PolicyBudgetAnchor::Realm(id) => ("realm", id),
            PolicyBudgetAnchor::Host(id) => ("host", id),
            PolicyBudgetAnchor::Site(id) => ("site", id),
        };
        let fields = [
            semantic("descriptor", CanonicalValue::Bytes(descriptor.as_bytes())),
            semantic("owner", CanonicalValue::Bytes(owner.as_bytes())),
            semantic("subject", CanonicalValue::Bytes(subject.as_bytes())),
            semantic("anchor_kind", CanonicalValue::Identifier(Id(anchor_kind))),
            semantic("anchor_id", CanonicalValue::Identifier(anchor_id)),
            semantic("action", CanonicalValue::Identifier(self.action)),
            semantic(
                "resource_class",
                CanonicalValue::Bytes(resource_class.as_bytes()),
            ),
            semantic("time_basis", CanonicalValue::Identifier(self.time_basis)),
            semantic(
                "current_stock",
                optional_u64_value(self.limits.current_stock),
            ),
            semantic(
                "rolling_units",
                optional_u64_value(self.limits.rolling.map(|value| value.units)),
            ),
            semantic(
                "rolling_window_ticks",
                optional_u64_value(self.limits.rolling.map(|value| value.window_ticks)),
            ),
            semantic("lifetime", optional_u64_value(self.limits.lifetime)),
            semantic(
                "reservation_ttl_ticks",
                CanonicalValue::Integer(i128::from(self.reservation_ttl_ticks)),
            ),
            semantic(
                "lease",
                lease.as_ref().map_or(CanonicalValue::Null, |value| {
                    CanonicalValue::Bytes(value.as_bytes())
                }),
            ),
            semantic("audit_id", CanonicalValue::Identifier(self.audit_id)),
            semantic(
                "persistence_profile",
                CanonicalValue::Bytes(persistence.as_bytes()),
            ),
            semantic(
                "maximum_reservations",
                CanonicalValue::Integer(i128::from(self.maximum_reservations)),
            ),
            semantic(
                "maximum_evidence_events",
                CanonicalValue::Integer(i128::from(self.maximum_evidence_events)),
            ),
        ];
        descriptor_hash(
            "conduit/persistent-policy-budget",
            self.schema_version,
            &fields,
        )
    }
}

impl PolicyBudgetStatus<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let ledger = hash_pin(self.ledger)?;
        let fields = [
            semantic(
                "policy_identity",
                CanonicalValue::Bytes(self.policy_identity.as_bytes()),
            ),
            semantic("ledger", CanonicalValue::Bytes(ledger.as_bytes())),
            semantic(
                "checkpoint",
                CanonicalValue::Bytes(self.checkpoint.as_bytes()),
            ),
            semantic(
                "sequence",
                CanonicalValue::Integer(i128::from(self.sequence)),
            ),
            semantic(
                "current_stock",
                CanonicalValue::Integer(i128::from(self.current_stock)),
            ),
            semantic(
                "rolling_window_start",
                CanonicalValue::Integer(i128::from(self.rolling_window_start)),
            ),
            semantic(
                "rolling_committed",
                CanonicalValue::Integer(i128::from(self.rolling_committed)),
            ),
            semantic(
                "lifetime_committed",
                CanonicalValue::Integer(i128::from(self.lifetime_committed)),
            ),
            semantic(
                "reserved",
                CanonicalValue::Integer(i128::from(self.reserved)),
            ),
            semantic(
                "evidence_remaining",
                CanonicalValue::Integer(i128::from(self.evidence_remaining)),
            ),
            semantic(
                "availability",
                CanonicalValue::Identifier(Id(match self.availability {
                    PolicyBudgetAvailability::Available => "available",
                    PolicyBudgetAvailability::Unavailable => "unavailable",
                    PolicyBudgetAvailability::RetentionGap => "retention-gap",
                })),
            ),
            semantic("time_basis", CanonicalValue::Identifier(self.time_basis)),
            semantic(
                "observed_at_tick",
                CanonicalValue::Integer(i128::from(self.observed_at_tick)),
            ),
            semantic(
                "valid_until_tick",
                CanonicalValue::Integer(i128::from(self.valid_until_tick)),
            ),
        ];
        descriptor_hash("conduit/policy-budget-status", self.schema_version, &fields)
    }
}

impl PolicyBudgetLease<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let holder = hash_pin(self.holder)?;
        let renewal = hash_pin(self.renewal_authority)?;
        let fields = [
            semantic(
                "policy_identity",
                CanonicalValue::Bytes(self.policy_identity.as_bytes()),
            ),
            semantic("holder", CanonicalValue::Bytes(holder.as_bytes())),
            semantic(
                "renewal_authority",
                CanonicalValue::Bytes(renewal.as_bytes()),
            ),
            semantic("time_basis", CanonicalValue::Identifier(self.time_basis)),
            semantic(
                "issued_at_tick",
                CanonicalValue::Integer(i128::from(self.issued_at_tick)),
            ),
            semantic(
                "expires_at_tick",
                CanonicalValue::Integer(i128::from(self.expires_at_tick)),
            ),
            semantic("offline", CanonicalValue::Boolean(self.offline)),
        ];
        descriptor_hash("conduit/policy-budget-lease", self.schema_version, &fields)
    }
}

impl PolicyBudgetRequest<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let consumer = hash_consumer(self.consumer)?;
        let lease = self
            .lease
            .map(|value| value.computed_semantic_hash())
            .transpose()?;
        let fields = [
            semantic(
                "correlation",
                CanonicalValue::Bytes(self.correlation.as_bytes()),
            ),
            semantic(
                "policy_identity",
                CanonicalValue::Bytes(self.policy_identity.as_bytes()),
            ),
            semantic("consumer", CanonicalValue::Bytes(consumer.as_bytes())),
            semantic("action", CanonicalValue::Identifier(self.action)),
            semantic("units", CanonicalValue::Integer(i128::from(self.units))),
            semantic(
                "requested_at_tick",
                CanonicalValue::Integer(i128::from(self.requested_at_tick)),
            ),
            semantic(
                "lease",
                lease.as_ref().map_or(CanonicalValue::Null, |value| {
                    CanonicalValue::Bytes(value.as_bytes())
                }),
            ),
        ];
        descriptor_hash("conduit/policy-budget-request", 1, &fields)
    }
}

fn validate_policy(policy: PersistentBudgetPolicy<'_>) -> Result<(), PolicyBudgetReason> {
    if policy.schema_version != POLICY_BUDGET_SCHEMA_VERSION {
        return Err(PolicyBudgetReason::UnsupportedVersion);
    }
    if policy.identity
        != policy
            .computed_semantic_hash()
            .map_err(|_| PolicyBudgetReason::InvalidDescriptor)?
    {
        return Err(PolicyBudgetReason::IdentityMismatch);
    }
    let has_limit = policy.limits.current_stock.is_some()
        || policy.limits.rolling.is_some()
        || policy.limits.lifetime.is_some();
    if !has_limit
        || policy.reservation_ttl_ticks == 0
        || policy.maximum_reservations == 0
        || policy.maximum_evidence_events == 0
        || policy
            .limits
            .rolling
            .is_some_and(|value| value.units == 0 || value.window_ticks == 0)
        || !valid_pin(policy.descriptor)
        || !valid_pin(policy.owner)
        || !valid_pin(policy.subject)
        || !valid_pin(policy.resource_class)
        || !valid_pin(policy.persistence_profile)
        || Id::new(policy.action.as_str()).is_err()
        || Id::new(policy.time_basis.as_str()).is_err()
        || Id::new(policy.audit_id.as_str()).is_err()
        || match policy.anchor {
            PolicyBudgetAnchor::Realm(id)
            | PolicyBudgetAnchor::Host(id)
            | PolicyBudgetAnchor::Site(id) => Id::new(id.as_str()).is_err(),
        }
        || policy
            .lease
            .is_some_and(|lease| lease.maximum_ticks == 0 || !valid_pin(lease.renewal_authority))
    {
        return Err(PolicyBudgetReason::InvalidDescriptor);
    }
    Ok(())
}

fn validate_request(
    policy: PersistentBudgetPolicy<'_>,
    request: PolicyBudgetRequest<'_>,
    now: AuthorityTime<'_>,
    ledger_available: bool,
) -> Result<(), PolicyBudgetReason> {
    validate_policy(policy)?;
    if request.identity
        != request
            .computed_semantic_hash()
            .map_err(|_| PolicyBudgetReason::InvalidDescriptor)?
    {
        return Err(PolicyBudgetReason::IdentityMismatch);
    }
    if request.policy_identity != policy.identity {
        return Err(PolicyBudgetReason::PolicyMismatch);
    }
    if request.action != policy.action {
        return Err(PolicyBudgetReason::ActionMismatch);
    }
    if request.units == 0
        || request.requested_at_tick > now.tick
        || Id::new(request.consumer.realm.as_str()).is_err()
        || Id::new(request.consumer.run.as_str()).is_err()
        || now.basis != policy.time_basis
    {
        return Err(PolicyBudgetReason::InvalidDescriptor);
    }
    if ledger_available {
        if let Some(lease) = request.lease {
            validate_lease(policy, lease, now)?;
        }
        return Ok(());
    }
    let lease = request.lease.ok_or(PolicyBudgetReason::LedgerUnavailable)?;
    validate_offline_lease(policy, lease, now)
}

fn validate_lease(
    policy: PersistentBudgetPolicy<'_>,
    lease: PolicyBudgetLease<'_>,
    now: AuthorityTime<'_>,
) -> Result<(), PolicyBudgetReason> {
    let rule = policy.lease.ok_or(PolicyBudgetReason::LeaseMismatch)?;
    if lease.schema_version != POLICY_BUDGET_SCHEMA_VERSION
        || lease.identity
            != lease
                .computed_semantic_hash()
                .map_err(|_| PolicyBudgetReason::InvalidDescriptor)?
        || lease.policy_identity != policy.identity
        || lease.renewal_authority != rule.renewal_authority
        || lease.time_basis != now.basis
        || lease.expires_at_tick <= lease.issued_at_tick
        || lease.expires_at_tick - lease.issued_at_tick > rule.maximum_ticks
    {
        return Err(PolicyBudgetReason::LeaseMismatch);
    }
    if now.tick < lease.issued_at_tick || now.tick >= lease.expires_at_tick {
        return Err(PolicyBudgetReason::LeaseExpired);
    }
    Ok(())
}

fn exceeds(committed: u64, reserved: u64, requested: u64, limit: u64) -> bool {
    committed
        .checked_add(reserved)
        .and_then(|value| value.checked_add(requested))
        .is_none_or(|total| total > limit)
}

fn limit_not_increased(old: Option<u64>, new: Option<u64>) -> bool {
    match (old, new) {
        (Some(old), Some(new)) => new <= old,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

fn rolling_not_increased(old: Option<RollingLimit>, new: Option<RollingLimit>) -> bool {
    match (old, new) {
        (Some(old), Some(new)) => new.units <= old.units && new.window_ticks >= old.window_ticks,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

fn lease_not_increased(old: Option<PolicyLeaseRule<'_>>, new: Option<PolicyLeaseRule<'_>>) -> bool {
    match (old, new) {
        (Some(old), Some(new)) => {
            new.maximum_ticks <= old.maximum_ticks
                && new.renewal_authority == old.renewal_authority
                && (!new.offline_allowed || old.offline_allowed)
        }
        (Some(_), None) | (None, None) => true,
        (None, Some(_)) => false,
    }
}

fn reservation_hash(
    request: SemanticHash,
    sequence: u64,
    expires_at_tick: u64,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let fields = [
        semantic("request", CanonicalValue::Bytes(request.as_bytes())),
        semantic("sequence", CanonicalValue::Integer(i128::from(sequence))),
        semantic(
            "expires_at_tick",
            CanonicalValue::Integer(i128::from(expires_at_tick)),
        ),
    ];
    descriptor_hash("conduit/policy-budget-reservation", 1, &fields)
}

fn hash_reservation_state(
    reservation: PolicyReservation,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let fields = [
        semantic(
            "identity",
            CanonicalValue::Bytes(reservation.identity.as_bytes()),
        ),
        semantic(
            "request_identity",
            CanonicalValue::Bytes(reservation.request_identity.as_bytes()),
        ),
        semantic(
            "correlation",
            CanonicalValue::Bytes(reservation.correlation.as_bytes()),
        ),
        semantic(
            "units",
            CanonicalValue::Integer(i128::from(reservation.units)),
        ),
        semantic(
            "sequence",
            CanonicalValue::Integer(i128::from(reservation.sequence)),
        ),
        semantic(
            "expires_at_tick",
            CanonicalValue::Integer(i128::from(reservation.expires_at_tick)),
        ),
        semantic(
            "state",
            CanonicalValue::Identifier(Id(match reservation.state {
                PolicyReservationState::Empty => "empty",
                PolicyReservationState::Reserved => "reserved",
                PolicyReservationState::Committed => "committed",
                PolicyReservationState::Released => "released",
            })),
        ),
    ];
    descriptor_hash("conduit/policy-budget-reservation-state", 1, &fields)
}

fn hash_consumer(
    consumer: PolicyBudgetConsumer<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let fields = [
        semantic("realm", CanonicalValue::Identifier(consumer.realm)),
        semantic("plan", CanonicalValue::Bytes(consumer.plan.as_bytes())),
        semantic("epoch", CanonicalValue::Integer(i128::from(consumer.epoch))),
        semantic(
            "generation",
            CanonicalValue::Integer(i128::from(consumer.generation)),
        ),
        semantic("run", CanonicalValue::Identifier(consumer.run)),
    ];
    descriptor_hash("conduit/policy-budget-consumer", 1, &fields)
}

fn hash_lease_rule(lease: PolicyLeaseRule<'_>) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let renewal = hash_pin(lease.renewal_authority)?;
    let fields = [
        semantic(
            "maximum_ticks",
            CanonicalValue::Integer(i128::from(lease.maximum_ticks)),
        ),
        semantic(
            "renewal_authority",
            CanonicalValue::Bytes(renewal.as_bytes()),
        ),
        semantic(
            "offline_allowed",
            CanonicalValue::Boolean(lease.offline_allowed),
        ),
    ];
    descriptor_hash("conduit/policy-budget-lease-rule", 1, &fields)
}

fn hash_pin(pin: PinnedDescriptor<'_>) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let fields = [
        semantic("id", CanonicalValue::Identifier(pin.id)),
        semantic(
            "schema_version",
            CanonicalValue::Integer(i128::from(pin.schema_version)),
        ),
        semantic(
            "semantic_hash",
            CanonicalValue::Bytes(pin.semantic_hash.as_bytes()),
        ),
    ];
    descriptor_hash("conduit/pinned-descriptor", 1, &fields)
}

fn descriptor_hash(
    kind: &'static str,
    schema_version: u32,
    fields: &[MapField<'_>],
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    CanonicalDescriptor {
        kind: Id(kind),
        schema_version,
        body: CanonicalValue::Map(fields),
    }
    .semantic_hash()
}

fn optional_u64_value(value: Option<u64>) -> CanonicalValue<'static> {
    value.map_or(CanonicalValue::Null, |value| {
        CanonicalValue::Integer(i128::from(value))
    })
}

fn valid_pin(pin: PinnedDescriptor<'_>) -> bool {
    pin.schema_version == 0 && Id::new(pin.id.as_str()).is_ok()
}

const fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}
