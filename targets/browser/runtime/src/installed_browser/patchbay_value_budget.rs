//! One prepared Body's value/scratch pool; leases retain real companion backing.
use conduit_core::{ActivePlayId, PlanId};
use std::{cell::RefCell, rc::Rc};

const PUBLICATIONS: usize = 2; // Current and prepared replacement may overlap.

struct Ledger {
    owner: PlanId,
    play: ActivePlayId,
    baseline: u64,
    limit: u64,
    live: bool,
    publications: [Option<u64>; PUBLICATIONS],
}

/// Constructed only from the prepared parent scheduler's measured value backing
/// and admitted scratch. This is storage accounting, not owner-route authority.
pub struct PatchbayValueStorageBudget {
    ledger: Rc<RefCell<Ledger>>,
}

pub(crate) struct PublicationValueLease {
    ledger: Rc<RefCell<Ledger>>,
    slot: usize,
}

impl core::fmt::Debug for PatchbayValueStorageBudget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let ledger = self.ledger.borrow();
        f.debug_struct("PatchbayValueStorageBudget")
            .field("owner", &ledger.owner)
            .field("play", &ledger.play)
            .field("baseline", &ledger.baseline)
            .field("limit", &ledger.limit)
            .field("live", &ledger.live)
            .field("publications", &ledger.publications)
            .finish()
    }
}

impl PatchbayValueStorageBudget {
    pub(crate) fn for_prepared_owner(
        owner: PlanId,
        play: ActivePlayId,
        backing: usize,
        scratch: u64,
    ) -> Result<Self, String> {
        let baseline = (backing as u64)
            .checked_add(scratch)
            .ok_or("owner value budget overflow")?;
        let limit = u64::from(super::super::measurement_limits::MEMORY_POOL_BYTES);
        if baseline > limit {
            return Err("owner exceeds browser value/scratch pool".into());
        }
        Ok(Self {
            ledger: Rc::new(RefCell::new(Ledger {
                owner,
                play,
                baseline,
                limit,
                live: true,
                publications: [None; PUBLICATIONS],
            })),
        })
    }

    pub(crate) fn admit(
        &self,
        owner: &PlanId,
        play: &ActivePlayId,
        backing: usize,
    ) -> Result<PublicationValueLease, String> {
        let mut ledger = self.ledger.borrow_mut();
        if !ledger.live || &ledger.owner != owner || &ledger.play != play {
            return Err("publication value budget has a retired or different owner Plan".into());
        }
        let slot = ledger
            .publications
            .iter()
            .position(Option::is_none)
            .ok_or("publication value lease capacity exhausted")?;
        let used = ledger
            .publications
            .iter()
            .flatten()
            .try_fold(ledger.baseline, |sum, bytes| sum.checked_add(*bytes))
            .ok_or("publication value budget overflow")?;
        if used
            .checked_add(backing as u64)
            .is_none_or(|total| total > ledger.limit)
        {
            return Err("publication exceeds parent browser value/scratch pool".into());
        }
        ledger.publications[slot] = Some(backing as u64);
        Ok(PublicationValueLease {
            ledger: Rc::clone(&self.ledger),
            slot,
        })
    }

    pub(crate) fn retire(&self) {
        self.ledger.borrow_mut().live = false;
    }
}

impl Drop for PatchbayValueStorageBudget {
    fn drop(&mut self) {
        self.retire();
    }
}

impl PublicationValueLease {
    pub(crate) fn is_live(&self) -> bool {
        let ledger = self.ledger.borrow();
        ledger.live && ledger.publications[self.slot].is_some()
    }
}

impl Drop for PublicationValueLease {
    fn drop(&mut self) {
        self.ledger.borrow_mut().publications[self.slot] = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_value_leases_bound_overlap_and_retirement_without_early_release() {
        let budget =
            PatchbayValueStorageBudget::for_prepared_owner("owner".into(), "play".into(), 0, 0)
                .unwrap();
        assert!(budget.admit(&"foreign".into(), &"play".into(), 1).is_err());
        assert!(budget
            .admit(&"owner".into(), &"foreign-play".into(), 1)
            .is_err());
        let first = budget.admit(&"owner".into(), &"play".into(), 1024).unwrap();
        let second = budget.admit(&"owner".into(), &"play".into(), 1024).unwrap();
        assert!(budget.admit(&"owner".into(), &"play".into(), 1).is_err());
        drop(first);
        assert!(budget
            .admit(&"owner".into(), &"play".into(), usize::MAX)
            .is_err());
        let next = budget.admit(&"owner".into(), &"play".into(), 1024).unwrap();
        budget.retire();
        assert!(!second.is_live());
        assert!(!next.is_live());
        assert!(budget.admit(&"owner".into(), &"play".into(), 1).is_err());
    }
}
