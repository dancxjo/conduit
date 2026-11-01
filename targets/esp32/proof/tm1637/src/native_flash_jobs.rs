//! One Boot-local cooperative maintenance mailbox; no SDK or provider wiring.
//! Jobs retain authenticated custody; budgets require independent qualification.
//! Callbacks run outside RefCell borrows. The mailbox allocates nothing;
use crate::native_flash_window::{FlashPermit, Refusal as WindowRefusal, WaitGuard};
use core::{
    cell::RefCell,
    task::{Poll, Waker},
};
/// Actual read-only ordinal: no hardware work, mutation, or received labels.
pub(crate) trait Job {
    fn step(&self) -> u32;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    Busy,
    Identity,
    Bounds,
    Exhausted,
    AlreadySplit,
    StaleStep,
    Window(WindowRefusal),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Progress {
    AdvancedOne,
    Unchanged,
    Unexpected,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Idle,
    Queued(u64),
    Running(u64),
    Completed(u64),
}
pub(crate) struct Completion<J, R, E> {
    pub id: u64,
    pub step: u32,
    pub budget: u32,
    pub owner: J,
    pub result: Result<R, E>,
    pub progress: Progress,
}
struct Queued<J> {
    id: u64,
    step: u32,
    budget: u32,
    owner: J,
}
enum State<J, R, E> {
    Idle,
    Queued(Queued<J>),
    Running(u64),
    Completed(Completion<J, R, E>),
}
struct Inner<J, R, E> {
    state: State<J, R, E>,
    sequence: u64,
    split: bool,
    waker: Option<Waker>,
}
pub(crate) struct Mailbox<J, R, E> {
    inner: RefCell<Inner<J, R, E>>,
}
pub(crate) struct RootSubmitter<'a, J, R, E> {
    mailbox: &'a Mailbox<J, R, E>,
}
pub(crate) struct WaitExecutor<'a, J, R, E> {
    mailbox: &'a Mailbox<J, R, E>,
}
type Endpoints<'a, J, R, E> = (RootSubmitter<'a, J, R, E>, WaitExecutor<'a, J, R, E>);
impl<J, R, E> Mailbox<J, R, E> {
    pub(crate) const fn new() -> Self {
        Self {
            inner: RefCell::new(Inner {
                state: State::Idle,
                sequence: 0,
                split: false,
                waker: None,
            }),
        }
    }
    pub(crate) fn split(&mut self) -> Result<Endpoints<'_, J, R, E>, Refusal> {
        if self.inner.get_mut().split {
            return Err(Refusal::AlreadySplit);
        }
        self.inner.get_mut().split = true;
        Ok((
            RootSubmitter { mailbox: self },
            WaitExecutor { mailbox: self },
        ))
    }
    fn phase(&self) -> Phase {
        match &self.inner.borrow().state {
            State::Idle => Phase::Idle,
            State::Queued(q) => Phase::Queued(q.id),
            State::Running(id) => Phase::Running(*id),
            State::Completed(c) => Phase::Completed(c.id),
        }
    }
}
impl<J: Job, R, E> RootSubmitter<'_, J, R, E> {
    pub(crate) fn phase(&self) -> Phase {
        self.mailbox.phase()
    }
    /// Failure returns the actual owner; no ID or step is caller supplied.
    pub(crate) fn submit(&mut self, owner: J, budget: u32) -> Result<u64, (Refusal, J)> {
        if budget == 0 {
            return Err((Refusal::Bounds, owner));
        }
        let step = owner.step();
        if step == u32::MAX {
            return Err((Refusal::Exhausted, owner));
        }
        let (id, waker) = {
            let mut inner = self.mailbox.inner.borrow_mut();
            if !matches!(inner.state, State::Idle) {
                return Err((Refusal::Busy, owner));
            }
            let Some(id) = inner.sequence.checked_add(1) else {
                return Err((Refusal::Exhausted, owner));
            };
            inner.sequence = id;
            inner.state = State::Queued(Queued {
                id,
                step,
                budget,
                owner,
            });
            (id, inner.waker.take())
        };
        if let Some(waker) = waker {
            waker.wake()
        }
        Ok(id)
    }
    /// Observe only the actual retained owner of this exact submission. A
    /// pure kind predicate receives a short borrow, never ownership. Running
    /// work is unavailable; idle/stale IDs cannot establish an owner match.
    pub(crate) fn owner_matches(&self, id: u64, kind: fn(&J) -> bool) -> bool {
        let inner = self.mailbox.inner.borrow();
        match &inner.state {
            State::Queued(job) if job.id == id => kind(&job.owner),
            State::Completed(done) if done.id == id => kind(&done.owner),
            _ => false,
        }
    }
    pub(crate) fn take(&mut self, id: u64) -> Result<Completion<J, R, E>, Refusal> {
        let mut inner = self.mailbox.inner.borrow_mut();
        if !matches!(&inner.state,State::Completed(c) if c.id==id) {
            return Err(Refusal::Identity);
        }
        let State::Completed(done) = core::mem::replace(&mut inner.state, State::Idle) else {
            unreachable!()
        };
        Ok(done)
    }
    /// Cancels Queued only; Completed may hold affine R/E and requires take().
    pub(crate) fn cancel(&mut self, id: u64) -> Result<J, Refusal> {
        let mut inner = self.mailbox.inner.borrow_mut();
        match &inner.state {
            State::Queued(q) if q.id == id => {}
            State::Running(_) | State::Completed(_) => return Err(Refusal::Busy),
            _ => return Err(Refusal::Identity),
        }
        let State::Queued(job) = core::mem::replace(&mut inner.state, State::Idle) else {
            unreachable!()
        };
        Ok(job.owner)
    }
}
impl<J: Job, R, E> WaitExecutor<'_, J, R, E> {
    /// Takes an owned waker; replacement/drop/wake occur outside mailbox borrows.
    pub(crate) fn register(&mut self, waker: Waker) -> Poll<()> {
        let mut incoming = Some(waker);
        let (old, pending) = {
            let mut inner = self.mailbox.inner.borrow_mut();
            if matches!(inner.state, State::Queued(_)) {
                (None, false)
            } else {
                (core::mem::replace(&mut inner.waker, incoming.take()), true)
            }
        };
        drop(old);
        drop(incoming);
        if pending {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
    pub(crate) fn execute(
        &mut self,
        wait: &WaitGuard<'_>,
        now: u64,
        run: impl FnOnce(&mut J, &FlashPermit<'_, '_>) -> Result<R, E>,
    ) -> Result<Option<u64>, Refusal> {
        let mut job = {
            let mut inner = self.mailbox.inner.borrow_mut();
            if !matches!(inner.state, State::Queued(_)) {
                return Ok(None);
            }
            let State::Queued(job) = core::mem::replace(&mut inner.state, State::Running(0)) else {
                unreachable!()
            };
            inner.state = State::Running(job.id);
            job
        };
        if job.owner.step() != job.step {
            self.restore(job);
            return Err(Refusal::StaleStep);
        }
        let permit = match wait.try_flash(now, job.budget) {
            Ok(permit) => permit,
            Err(error) => {
                self.restore(job);
                return Err(Refusal::Window(error));
            }
        };
        let result = run(&mut job.owner, &permit);
        drop(permit);
        let after = job.owner.step();
        let progress = if Some(after) == job.step.checked_add(1) {
            Progress::AdvancedOne
        } else if after == job.step {
            Progress::Unchanged
        } else {
            Progress::Unexpected
        };
        let id = job.id;
        self.mailbox.inner.borrow_mut().state = State::Completed(Completion {
            id,
            step: job.step,
            budget: job.budget,
            owner: job.owner,
            result,
            progress,
        });
        Ok(Some(id))
    }
    fn restore(&self, job: Queued<J>) {
        self.mailbox.inner.borrow_mut().state = State::Queued(job)
    }
}
#[cfg(test)]
#[path = "native_flash_jobs_tests.rs"]
mod tests;
