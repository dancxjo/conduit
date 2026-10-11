//! Bounded preparation-side FIFO waiting, delegating occupation to the kernel.
//! Entries contain custody and admitted extent, never request payloads. Expiry
//! ends waiting; it does not predict or promise provider completion.
use super::*;
use alloc::string::String;
use conduit_core::MonotonicInstant;

pub const MAXIMUM_POOL_WAITING_OPERATIONS: usize = 32;
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PoolWaitingLimits {
    pub operations: usize,
    pub per_requester: usize,
    pub admitted_bytes: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedPoolOperation {
    pub plan: PlanId,
    pub pool: SharedPoolId,
    pub operation: PoolOperationId,
    pub requester: String,
    pub key: MemberKey,
    pub demand: [u8; 32],
    /// Extent admitted by the request owner; this queue holds no payload.
    pub admitted_bytes: u32,
    pub waiting_expires: MonotonicInstant,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolWaitingError {
    InvalidLimits,
    AlreadyEnabled,
    NotEnabled,
    InvalidIdentity,
    Correlation,
    Duplicate,
    Full,
    RequesterFull,
    ByteLimit,
    ClockChanged,
    ClockRegressed,
    Expired,
    Missing,
    SequenceExhausted,
    Admission(SharedPoolRuntimeError),
}
struct Entry {
    sequence: u64,
    operation: QueuedPoolOperation,
}
pub(super) struct BoundedPoolWaiting {
    limits: PoolWaitingLimits,
    entries: [Option<Entry>; MAXIMUM_POOL_WAITING_OPERATIONS],
    bytes: u32,
    next_sequence: u64,
    last_now: MonotonicInstant,
}
impl BoundedPoolWaiting {
    fn check_time(&self, now: &MonotonicInstant) -> Result<(), PoolWaitingError> {
        if now.validate().is_err() || now.clock() != self.last_now.clock() {
            return Err(PoolWaitingError::ClockChanged);
        }
        if now.ticks() < self.last_now.ticks() {
            return Err(PoolWaitingError::ClockRegressed);
        }
        Ok(())
    }
    fn first(&self) -> Option<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.as_ref().map(|e| (i, e.sequence)))
            .min_by_key(|(_, sequence)| *sequence)
            .map(|(i, _)| i)
    }
    fn remove(&mut self, index: usize) -> QueuedPoolOperation {
        let entry = self.entries[index]
            .take()
            .expect("checked occupied waiting slot");
        self.bytes -= entry.operation.admitted_bytes;
        entry.operation
    }
}
impl<const M: usize, const S: usize, const O: usize> HostedSharedPool<M, S, O> {
    /// Admit fixed preparation storage explicitly. No allocation occurs in the
    /// kernel selector, and enabling waiting changes no sealed realization.
    pub fn enable_waiting(
        &mut self,
        limits: PoolWaitingLimits,
        now: MonotonicInstant,
    ) -> Result<(), PoolWaitingError> {
        if self.waiting.is_some() {
            return Err(PoolWaitingError::AlreadyEnabled);
        }
        if limits.operations == 0
            || limits.operations > MAXIMUM_POOL_WAITING_OPERATIONS
            || limits.per_requester == 0
            || limits.per_requester > limits.operations
            || limits.admitted_bytes == 0
            || limits.admitted_bytes > 64 * 1024
            || now.validate().is_err()
        {
            return Err(PoolWaitingError::InvalidLimits);
        }
        self.waiting = Some(BoundedPoolWaiting {
            limits,
            entries: core::array::from_fn(|_| None),
            bytes: 0,
            next_sequence: 0,
            last_now: now,
        });
        Ok(())
    }
    pub fn waiting_population(&self) -> usize {
        self.waiting
            .as_ref()
            .map_or(0, |q| q.entries.iter().flatten().count())
    }
    pub fn queue_operation(
        &mut self,
        operation: QueuedPoolOperation,
        now: &MonotonicInstant,
    ) -> Result<(), PoolWaitingError> {
        if operation.plan != self.plan.plan_id || operation.pool != self.pool_id {
            return Err(PoolWaitingError::Correlation);
        }
        if [operation.operation.as_str(), operation.requester.as_str()]
            .iter()
            .any(|s| s.is_empty() || s.len() > 256)
        {
            return Err(PoolWaitingError::InvalidIdentity);
        }
        if self.kernel.member_for_key(operation.key).is_ok() {
            return Err(PoolWaitingError::Duplicate);
        }
        let queue = self.waiting.as_mut().ok_or(PoolWaitingError::NotEnabled)?;
        queue.check_time(now)?;
        if operation.waiting_expires.validate().is_err()
            || operation.waiting_expires.clock() != now.clock()
        {
            return Err(PoolWaitingError::ClockChanged);
        }
        if operation.waiting_expires.ticks() <= now.ticks() {
            return Err(PoolWaitingError::Expired);
        }
        if queue.entries.iter().flatten().any(|e| {
            e.operation.key == operation.key || e.operation.operation == operation.operation
        }) {
            return Err(PoolWaitingError::Duplicate);
        }
        if queue.entries.iter().flatten().count() >= queue.limits.operations {
            return Err(PoolWaitingError::Full);
        }
        if queue
            .entries
            .iter()
            .flatten()
            .filter(|e| e.operation.requester == operation.requester)
            .count()
            >= queue.limits.per_requester
        {
            return Err(PoolWaitingError::RequesterFull);
        }
        let bytes = queue
            .bytes
            .checked_add(operation.admitted_bytes)
            .filter(|n| *n <= queue.limits.admitted_bytes)
            .ok_or(PoolWaitingError::ByteLimit)?;
        let next = queue
            .next_sequence
            .checked_add(1)
            .ok_or(PoolWaitingError::SequenceExhausted)?;
        let slot = queue
            .entries
            .iter_mut()
            .find(|e| e.is_none())
            .expect("bounded waiting slot available");
        *slot = Some(Entry {
            sequence: queue.next_sequence,
            operation,
        });
        queue.next_sequence = next;
        queue.bytes = bytes;
        queue.last_now = now.clone();
        Ok(())
    }
    /// Borrow the FIFO request so the owner can prepare fresh operation-specific
    /// costs. Expired requests require explicit retirement, never silent loss.
    pub fn next_waiting_operation(
        &self,
        now: &MonotonicInstant,
    ) -> Result<Option<&QueuedPoolOperation>, PoolWaitingError> {
        let q = self.waiting.as_ref().ok_or(PoolWaitingError::NotEnabled)?;
        q.check_time(now)?;
        let Some(i) = q.first() else {
            return Ok(None);
        };
        let operation = &q.entries[i].as_ref().unwrap().operation;
        if operation.waiting_expires.ticks() <= now.ticks() {
            return Err(PoolWaitingError::Expired);
        }
        Ok(Some(operation))
    }
    /// Cancellation returns the exact waiting custody. An admitted member is
    /// absent here; this method cannot release, migrate or replay its effects.
    pub fn cancel_waiting_operation(
        &mut self,
        operation: &PoolOperationId,
    ) -> Result<QueuedPoolOperation, PoolWaitingError> {
        let q = self.waiting.as_mut().ok_or(PoolWaitingError::NotEnabled)?;
        let i = q
            .entries
            .iter()
            .position(|e| {
                e.as_ref()
                    .is_some_and(|e| &e.operation.operation == operation)
            })
            .ok_or(PoolWaitingError::Missing)?;
        Ok(q.remove(i))
    }
    /// Retire one expired request with custody; at most 32 entries are scanned.
    pub fn retire_expired_waiting(
        &mut self,
        now: &MonotonicInstant,
    ) -> Result<Option<QueuedPoolOperation>, PoolWaitingError> {
        let q = self.waiting.as_mut().ok_or(PoolWaitingError::NotEnabled)?;
        q.check_time(now)?;
        let i = q
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                e.as_ref()
                    .filter(|e| e.operation.waiting_expires.ticks() <= now.ticks())
                    .map(|e| (i, e.sequence))
            })
            .min_by_key(|(_, s)| *s)
            .map(|(i, _)| i);
        q.last_now = now.clone();
        Ok(i.map(|i| q.remove(i)))
    }
    #[allow(clippy::too_many_arguments)]
    pub fn admit_next_waiting_with_costs(
        &mut self,
        now: &MonotonicInstant,
        current: &[PoolRealizationObservation],
        sign: SignId,
        prepared: &[conduit_core::PreparedPoolCost<'_>],
        evaluations: &[PoolCostEvaluation<'_>],
    ) -> Result<Option<QueuedCostAdmission>, PoolWaitingError> {
        let Some(operation) = self.next_waiting_operation(now)? else {
            return Ok(None);
        };
        let id = operation.operation.clone();
        let key = operation.key;
        let demand = operation.demand;
        let outcome = self
            .admit_new_operation_with_costs(
                id.clone(),
                key,
                current,
                sign,
                demand,
                prepared,
                evaluations,
            )
            .map_err(PoolWaitingError::Admission)?;
        let retained = matches!(&outcome.admission,SharedPoolAdmissionOutcome::Refused(e) if e.disposition==PoolSelectionDisposition::CapacityRefused);
        let retired = if retained {
            None
        } else {
            Some(self.cancel_waiting_operation(&id)?)
        };
        self.waiting.as_mut().unwrap().last_now = now.clone();
        Ok(Some(QueuedCostAdmission { outcome, retired }))
    }
}
/// Selected/envelope-exhausted requests leave waiting and return their exact
/// metadata. Capacity refusal retains the request. Errors retain it as well.
#[derive(Debug)]
pub struct QueuedCostAdmission {
    pub outcome: CostAwareSharedPoolAdmissionOutcome,
    pub retired: Option<QueuedPoolOperation>,
}
