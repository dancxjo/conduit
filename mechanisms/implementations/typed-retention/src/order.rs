//! One preparation-owned, finite order shared by local outcome endpoints.
use alloc::rc::Rc;
use core::cell::Cell;

#[derive(Clone)]
pub struct CommitOrder(Rc<State>);
struct State {
    maximum: u64,
    committed: Cell<u64>,
    reserved: Cell<u64>,
}
impl CommitOrder {
    /// Allocates during preparation. This is local ordering, not a cross-Boot
    /// identity, clock or durable acknowledgement. A zero budget admits no event.
    pub fn prepare(maximum_events: u64) -> Self {
        Self(Rc::new(State {
            maximum: maximum_events,
            committed: Cell::new(0),
            reserved: Cell::new(0),
        }))
    }
    pub fn committed(&self) -> u64 {
        self.0.committed.get()
    }
    pub(super) fn reserve(&self) -> bool {
        let Some(total) = self.committed().checked_add(self.0.reserved.get()) else {
            return false;
        };
        if total >= self.0.maximum {
            return false;
        }
        self.0.reserved.set(self.0.reserved.get() + 1);
        true
    }
    pub(super) fn release(&self) {
        self.0.reserved.set(self.0.reserved.get() - 1);
    }
    pub(super) fn commit(&self) -> u64 {
        // A held reservation proves both finite capacity and arithmetic headroom.
        let next = self.committed() + 1;
        self.0.committed.set(next);
        self.release();
        next
    }
}
