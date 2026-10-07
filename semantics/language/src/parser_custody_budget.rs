//! Mechanical reservation for the concrete parser custody owner.
//! This is not a fact, lineage, or commitment admission. Only the concrete owner
//! may derive charges from its complete Source-admitted Native receipts.
use alloc::rc::Rc;
use core::cell::Cell;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Usage {
    pub origins: u8,
    pub facts: u32,
    pub rebases: u32,
    pub snapshots: u32,
    pub commits: u32,
    pub bytes: u64,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    pub retained: Usage,
    /// Complete old custody plus the single prepared candidate.
    pub peak_bytes: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    Limits,
    Pressure,
    Cancelled,
}
#[derive(Clone)]
pub(crate) struct Cancellation(Rc<Cell<bool>>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.set(true);
    }
}
pub(crate) struct Budget {
    limits: Limits,
    used: Usage,
    cancelled: Rc<Cell<bool>>,
}
pub(crate) struct Reservation<'a> {
    budget: &'a mut Budget,
    next: Usage,
}
impl Budget {
    pub fn new(limits: Limits, initial: Usage) -> Result<Self, Refusal> {
        if limits.retained.origins > 4
            || limits.retained.bytes == 0
            || limits.peak_bytes < limits.retained.bytes
        {
            return Err(Refusal::Limits);
        }
        let budget = Self {
            limits,
            used: initial,
            cancelled: Rc::new(Cell::new(false)),
        };
        budget.check(initial)?;
        Ok(budget)
    }
    pub fn used(&self) -> Usage {
        self.used
    }
    pub fn cancellation(&self) -> Cancellation {
        Cancellation(self.cancelled.clone())
    }
    fn check(&self, next: Usage) -> Result<(), Refusal> {
        let cap = self.limits.retained;
        if next.origins > cap.origins
            || next.facts > cap.facts
            || next.rebases > cap.rebases
            || next.snapshots > cap.snapshots
            || next.commits > cap.commits
            || next.bytes > cap.bytes
        {
            return Err(Refusal::Pressure);
        }
        Ok(())
    }
    /// One mutable borrow permits one candidate and prevents cross-owner publish.
    /// `next` is calculated by the concrete owner, including every retained full
    /// receipt; `candidate_bytes` accounts for complete temporary custody.
    pub fn reserve(
        &mut self,
        next: Usage,
        candidate_bytes: u64,
    ) -> Result<Reservation<'_>, Refusal> {
        if self.cancelled.get() {
            return Err(Refusal::Cancelled);
        }
        self.check(next)?;
        if next.origins < self.used.origins
            || next.facts < self.used.facts
            || next.rebases < self.used.rebases
            || next.snapshots < self.used.snapshots
            || next.commits < self.used.commits
        {
            return Err(Refusal::Pressure);
        }
        let peak = self
            .used
            .bytes
            .checked_add(candidate_bytes)
            .ok_or(Refusal::Pressure)?;
        if peak > self.limits.peak_bytes {
            return Err(Refusal::Pressure);
        }
        Ok(Reservation { budget: self, next })
    }
}
impl Reservation<'_> {
    /// Recheck the independent cancellation latch immediately before publication.
    /// The concrete owner must perform only infallible custody swaps afterward.
    pub fn publish(self) -> Result<(), Refusal> {
        if self.budget.cancelled.get() {
            return Err(Refusal::Cancelled);
        }
        self.budget.used = self.next;
        Ok(())
    }
}
