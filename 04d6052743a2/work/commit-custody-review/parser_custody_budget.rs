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
impl Usage {
    fn fits(self, cap: Self) -> bool {
        self.origins <= cap.origins
            && self.facts <= cap.facts
            && self.rebases <= cap.rebases
            && self.snapshots <= cap.snapshots
            && self.commits <= cap.commits
            && self.bytes <= cap.bytes
    }
    fn retains_counts(self, prior: Self) -> bool {
        Self { bytes: 0, ..prior }.fits(Self { bytes: 0, ..self })
    }
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
/// Reserved before Source execution; actual full custody charges are checked
/// after it. A consumed refusal must poison the owning ingress separately.
pub(crate) struct EnvelopeReservation<'a> {
    reservation: Reservation<'a>,
    candidate_bytes: u64,
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
        if !next.fits(self.limits.retained) {
            return Err(Refusal::Pressure);
        }
        Ok(())
    }
    /// Reserve declared complete custody and candidate ceilings before invoking
    /// Source. This bounds retained evidence admission, not executor allocation.
    pub fn reserve_envelope(
        &mut self,
        maximum_next: Usage,
        maximum_candidate_bytes: u64,
    ) -> Result<EnvelopeReservation<'_>, Refusal> {
        Ok(EnvelopeReservation {
            reservation: self.reserve(maximum_next, maximum_candidate_bytes)?,
            candidate_bytes: maximum_candidate_bytes,
        })
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
        if !next.retains_counts(self.used) {
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

impl EnvelopeReservation<'_> {
    /// The caller checks this immediately before consuming its Source ingress.
    pub fn check_before_consumption(&self) -> Result<(), Refusal> {
        if self.reservation.budget.cancelled.get() {
            Err(Refusal::Cancelled)
        } else {
            Ok(())
        }
    }
    /// Only the concrete owner derives actual charges from complete admitted
    /// receipts. All fallible work must finish before subsequent custody moves.
    pub fn publish_exact(
        mut self,
        actual: Usage,
        actual_candidate_bytes: u64,
    ) -> Result<(), Refusal> {
        self.check_before_consumption()?;
        if !actual.fits(self.reservation.next)
            || !actual.retains_counts(self.reservation.budget.used)
            || actual_candidate_bytes > self.candidate_bytes
        {
            return Err(Refusal::Pressure);
        }
        self.reservation.next = actual;
        self.reservation.publish()
    }
}
