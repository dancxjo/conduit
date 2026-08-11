//! Finite host effect adapter for exact monotonic millisecond deadlines.
//!
//! This module owns clock reads, waiting, arm/cancel bookkeeping, and wake
//! ordering only. Semantic timeout or debounce policy remains in kernel
//! operations.

use conduit_kernel::scheduler::{HostOperationCancellation, HostOperationRequest};
use conduit_kernel::{HostOperationId, NodeId, RequestId};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeadlineKey {
    pub node: NodeId,
    pub request: RequestId,
    pub operation: HostOperationId,
}

impl From<HostOperationRequest> for DeadlineKey {
    fn from(request: HostOperationRequest) -> Self {
        Self {
            node: request.node,
            request: request.request,
            operation: request.operation,
        }
    }
}

impl From<HostOperationCancellation> for DeadlineKey {
    fn from(cancellation: HostOperationCancellation) -> Self {
        Self {
            node: cancellation.node,
            request: cancellation.request,
            operation: cancellation.operation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ArmedDeadline {
    key: DeadlineKey,
    deadline_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineWake {
    Fired(DeadlineKey),
    Pending { deadline_ms: u64 },
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineReactorError {
    Full,
    Duplicate,
    Stale,
    DeadlineOverflow,
}

pub struct DeadlineReactor<const SLOTS: usize> {
    slots: [Option<ArmedDeadline>; SLOTS],
}

impl<const SLOTS: usize> DeadlineReactor<SLOTS> {
    pub const fn new() -> Self {
        Self {
            slots: [None; SLOTS],
        }
    }

    pub fn arm(
        &mut self,
        key: DeadlineKey,
        duration_ms: u64,
        now_ms: u64,
    ) -> Result<(), DeadlineReactorError> {
        if self.slots.iter().flatten().any(|armed| armed.key == key) {
            return Err(DeadlineReactorError::Duplicate);
        }
        let deadline_ms = now_ms
            .checked_add(duration_ms)
            .ok_or(DeadlineReactorError::DeadlineOverflow)?;
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or(DeadlineReactorError::Full)?;
        *slot = Some(ArmedDeadline { key, deadline_ms });
        Ok(())
    }

    pub fn cancel(&mut self, key: DeadlineKey) -> Result<(), DeadlineReactorError> {
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.is_some_and(|armed| armed.key == key))
            .ok_or(DeadlineReactorError::Stale)?;
        *slot = None;
        Ok(())
    }

    pub fn poll(&mut self, now_ms: u64) -> DeadlineWake {
        let Some((index, armed)) = self.next() else {
            return DeadlineWake::Empty;
        };
        if armed.deadline_ms > now_ms {
            return DeadlineWake::Pending {
                deadline_ms: armed.deadline_ms,
            };
        }
        self.slots[index] = None;
        DeadlineWake::Fired(armed.key)
    }

    pub fn len(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    pub const fn capacity(&self) -> usize {
        SLOTS
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn next(&self) -> Option<(usize, ArmedDeadline)> {
        self.slots
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(index, armed)| armed.map(|armed| (index, armed)))
            .min_by_key(|(_, armed)| (armed.deadline_ms, armed.key))
    }
}

impl<const SLOTS: usize> Default for DeadlineReactor<SLOTS> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineClockError {
    Unavailable,
    Regressed,
    Overflow,
}

pub trait DeadlineClock {
    fn now_ms(&mut self) -> Result<u64, DeadlineClockError>;
    fn wait_until_ms(&mut self, deadline_ms: u64) -> Result<(), DeadlineClockError>;
}

pub struct ThreadMonotonicClock {
    epoch: Instant,
}

impl ThreadMonotonicClock {
    pub fn new() -> Self {
        Self {
            epoch: Instant::now(),
        }
    }
}

impl Default for ThreadMonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

impl DeadlineClock for ThreadMonotonicClock {
    fn now_ms(&mut self) -> Result<u64, DeadlineClockError> {
        u64::try_from(self.epoch.elapsed().as_millis()).map_err(|_| DeadlineClockError::Overflow)
    }

    fn wait_until_ms(&mut self, deadline_ms: u64) -> Result<(), DeadlineClockError> {
        let now = self.now_ms()?;
        if deadline_ms > now {
            thread::sleep(Duration::from_millis(deadline_ms - now));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineHostError {
    Clock(DeadlineClockError),
    Reactor(DeadlineReactorError),
}

pub struct DeadlineHostAdapter<C, const SLOTS: usize> {
    clock: C,
    reactor: DeadlineReactor<SLOTS>,
    last_now_ms: Option<u64>,
}

impl<C: DeadlineClock, const SLOTS: usize> DeadlineHostAdapter<C, SLOTS> {
    pub const fn new(clock: C) -> Self {
        Self {
            clock,
            reactor: DeadlineReactor::new(),
            last_now_ms: None,
        }
    }

    pub fn arm(
        &mut self,
        request: HostOperationRequest,
        duration_ms: u64,
    ) -> Result<(), DeadlineHostError> {
        let now = self.now()?;
        self.reactor
            .arm(request.into(), duration_ms, now)
            .map_err(DeadlineHostError::Reactor)
    }

    pub fn cancel(
        &mut self,
        cancellation: HostOperationCancellation,
    ) -> Result<(), DeadlineHostError> {
        self.reactor
            .cancel(cancellation.into())
            .map_err(DeadlineHostError::Reactor)
    }

    pub fn poll(&mut self) -> Result<DeadlineWake, DeadlineHostError> {
        let now = self.now()?;
        Ok(self.reactor.poll(now))
    }

    pub fn wait_next(&mut self) -> Result<DeadlineWake, DeadlineHostError> {
        let now = self.now()?;
        match self.reactor.poll(now) {
            DeadlineWake::Pending { deadline_ms } => {
                self.clock
                    .wait_until_ms(deadline_ms)
                    .map_err(DeadlineHostError::Clock)?;
                let after_wait = self.now()?;
                Ok(self.reactor.poll(after_wait))
            }
            wake => Ok(wake),
        }
    }

    pub fn len(&self) -> usize {
        self.reactor.len()
    }

    pub const fn capacity(&self) -> usize {
        self.reactor.capacity()
    }

    pub fn is_empty(&self) -> bool {
        self.reactor.is_empty()
    }

    pub fn into_clock(self) -> C {
        self.clock
    }

    fn now(&mut self) -> Result<u64, DeadlineHostError> {
        let now = self.clock.now_ms().map_err(DeadlineHostError::Clock)?;
        if self.last_now_ms.is_some_and(|last| now < last) {
            return Err(DeadlineHostError::Clock(DeadlineClockError::Regressed));
        }
        self.last_now_ms = Some(now);
        Ok(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{BoundedValueRef, ValueRef};

    #[derive(Debug, Clone, Copy)]
    struct VirtualClock {
        now_ms: u64,
        available: bool,
    }

    impl DeadlineClock for VirtualClock {
        fn now_ms(&mut self) -> Result<u64, DeadlineClockError> {
            self.available
                .then_some(self.now_ms)
                .ok_or(DeadlineClockError::Unavailable)
        }

        fn wait_until_ms(&mut self, deadline_ms: u64) -> Result<(), DeadlineClockError> {
            if !self.available {
                return Err(DeadlineClockError::Unavailable);
            }
            self.now_ms = self.now_ms.max(deadline_ms);
            Ok(())
        }
    }

    fn request(node: u16, request: u32) -> HostOperationRequest {
        HostOperationRequest {
            node: NodeId(node),
            request: RequestId(request),
            operation: HostOperationId(0),
            input: BoundedValueRef::new(
                ValueRef {
                    slot: request as u16,
                    generation: 1,
                    byte_len: 8,
                },
                8,
            )
            .unwrap(),
        }
    }

    #[test]
    fn equal_deadlines_are_key_ordered_and_cancel_is_exact() {
        let mut reactor = DeadlineReactor::<3>::new();
        reactor.arm(request(2, 3).into(), 5, 10).unwrap();
        reactor.arm(request(1, 9).into(), 5, 10).unwrap();
        reactor.arm(request(1, 4).into(), 5, 10).unwrap();
        assert_eq!(reactor.poll(14), DeadlineWake::Pending { deadline_ms: 15 });
        assert_eq!(reactor.poll(15), DeadlineWake::Fired(request(1, 4).into()));
        reactor.cancel(request(2, 3).into()).unwrap();
        assert_eq!(reactor.poll(15), DeadlineWake::Fired(request(1, 9).into()));
        assert_eq!(reactor.poll(15), DeadlineWake::Empty);
        assert_eq!(
            reactor.cancel(request(2, 3).into()),
            Err(DeadlineReactorError::Stale)
        );
    }

    #[test]
    fn full_duplicate_overflow_and_clock_failure_are_distinct() {
        let mut reactor = DeadlineReactor::<1>::new();
        reactor.arm(request(0, 1).into(), 1, 0).unwrap();
        assert_eq!(
            reactor.arm(request(0, 1).into(), 1, 0),
            Err(DeadlineReactorError::Duplicate)
        );
        assert_eq!(
            reactor.arm(request(0, 2).into(), 1, 0),
            Err(DeadlineReactorError::Full)
        );
        reactor.cancel(request(0, 1).into()).unwrap();
        assert_eq!(
            reactor.arm(request(0, 2).into(), 1, u64::MAX),
            Err(DeadlineReactorError::DeadlineOverflow)
        );

        let mut adapter = DeadlineHostAdapter::<_, 1>::new(VirtualClock {
            now_ms: 0,
            available: false,
        });
        assert_eq!(
            adapter.arm(request(0, 1), 1),
            Err(DeadlineHostError::Clock(DeadlineClockError::Unavailable))
        );
    }

    #[test]
    fn virtual_clock_uses_the_same_arm_wait_and_wake_contract_without_allocation() {
        let mut adapter = DeadlineHostAdapter::<_, 2>::new(VirtualClock {
            now_ms: 7,
            available: true,
        });
        let probe = crate::allocation_probe::begin();
        adapter.arm(request(0, 1), 3).unwrap();
        assert_eq!(
            adapter.poll().unwrap(),
            DeadlineWake::Pending { deadline_ms: 10 }
        );
        assert_eq!(
            adapter.wait_next().unwrap(),
            DeadlineWake::Fired(request(0, 1).into())
        );
        assert_eq!(probe.finish(), 0);
        assert_eq!(adapter.len(), 0);
        assert_eq!(adapter.capacity(), 2);
    }

    #[test]
    fn hosted_monotonic_clock_uses_the_same_zero_duration_contract() {
        let mut adapter = DeadlineHostAdapter::<_, 1>::new(ThreadMonotonicClock::new());
        adapter.arm(request(0, 1), 0).unwrap();
        assert_eq!(
            adapter.wait_next().unwrap(),
            DeadlineWake::Fired(request(0, 1).into())
        );
        assert!(adapter.is_empty());
    }
}
