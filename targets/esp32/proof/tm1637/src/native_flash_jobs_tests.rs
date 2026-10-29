use super::*;
use crate::native_flash_window::Gate;
use std::{
    cell::Cell,
    rc::Rc,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::Wake,
};
struct Owner {
    identity: u64,
    step: u32,
    shared: Option<Rc<Cell<u32>>>,
}
impl Job for Owner {
    fn step(&self) -> u32 {
        self.shared.as_ref().map_or(self.step, |s| s.get())
    }
}
fn owner() -> Owner {
    Owner {
        identity: 77,
        step: 3,
        shared: None,
    }
}
fn gate() -> Gate {
    Gate::new(100, 20, 10).unwrap()
}
#[test]
fn kind_observation_uses_exact_retained_owner_without_moving_it() {
    let mut m = Mailbox::<Owner, (), ()>::new();
    let (mut root, mut worker) = m.split().unwrap();
    assert!(!root.owner_matches(0, |_| true));
    let id = root.submit(owner(), 5).ok().unwrap();
    assert!(root.owner_matches(id, |o| o.identity == 77));
    assert!(!root.owner_matches(id + 1, |_| true));
    assert!(!root.owner_matches(id, |o| o.identity == 88));
    let actual = root.cancel(id).unwrap();
    assert_eq!(actual.identity, 77);
    assert!(!root.owner_matches(id, |_| true));
    let next = root.submit(actual, 5).ok().unwrap();
    assert!(!root.owner_matches(id, |_| true));
    let g = gate();
    let wait = g.begin_wait(1, 50).unwrap();
    worker
        .execute(&wait, 2, |_, _| {
            assert!(!root.owner_matches(next, |_| true));
            Ok(())
        })
        .unwrap();
    assert!(root.owner_matches(next, |o| o.identity == 77));
    assert!(!root.owner_matches(next, |o| o.identity == 88));
    assert!(matches!(root.cancel(next), Err(Refusal::Busy)));
    assert_eq!(root.take(next).unwrap().owner.identity, 77);
    assert!(!root.owner_matches(next, |_| true));
}
#[test]
fn actual_owner_survives_queued_cancellation_and_completed_take() {
    let mut m = Mailbox::<Owner, (), ()>::new();
    let (mut root, mut worker) = m.split().unwrap();
    let id = root.submit(owner(), 5).ok().unwrap();
    assert_eq!(root.cancel(id).unwrap().identity, 77);
    let id = root.submit(owner(), 5).ok().unwrap();
    let g = gate();
    let wait = g.begin_wait(1, 50).unwrap();
    worker
        .execute(&wait, 2, |j, _permit| {
            j.step += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(root.phase(), Phase::Completed(id));
    assert!(matches!(root.cancel(id), Err(Refusal::Busy)));
    assert_eq!(root.take(id).unwrap().owner.step, 4);
    assert_eq!(root.phase(), Phase::Idle);
}
#[test]
fn deadline_and_pulse_refusal_do_not_run_or_lose_queued_job() {
    let mut m = Mailbox::<Owner, (), ()>::new();
    let (mut root, mut worker) = m.split().unwrap();
    let id = root.submit(owner(), 5).ok().unwrap();
    let g = gate();
    let wait = g.begin_wait(1, 10).unwrap();
    let called = Cell::new(false);
    assert_eq!(
        worker.execute(&wait, 6, |_, _permit| {
            called.set(true);
            Ok(())
        }),
        Err(Refusal::Window(WindowRefusal::Deadline))
    );
    let pulse = g.begin_pulse(2, 8).unwrap();
    assert_eq!(
        worker.execute(&wait, 3, |_, _permit| {
            called.set(true);
            Ok(())
        }),
        Err(Refusal::Window(WindowRefusal::Overlap))
    );
    drop(pulse);
    assert!(!called.get());
    assert_eq!(root.phase(), Phase::Queued(id));
    assert_eq!(root.cancel(id).unwrap().identity, 77);
}
#[test]
fn running_and_lost_completion_fence_resubmission_without_reentrant_borrow() {
    let mut m = Mailbox::<Owner, u8, &str>::new();
    let (mut root, mut worker) = m.split().unwrap();
    let id = root.submit(owner(), 5).ok().unwrap();
    let g = gate();
    let wait = g.begin_wait(1, 50).unwrap();
    worker
        .execute(&wait, 2, |j, _permit| {
            assert_eq!(root.phase(), Phase::Running(id));
            assert!(matches!(root.submit(owner(), 5), Err((Refusal::Busy, _))));
            assert!(matches!(root.cancel(id), Err(Refusal::Busy)));
            j.step += 1;
            Ok(9)
        })
        .unwrap();
    assert!(matches!(root.submit(owner(), 5), Err((Refusal::Busy, _))));
    assert!(matches!(root.take(id + 1), Err(Refusal::Identity)));
    let done = root.take(id).unwrap();
    assert_eq!(
        (
            done.id,
            done.step,
            done.budget,
            done.owner.identity,
            done.result,
            done.progress
        ),
        (id, 3, 5, 77, Ok(9), Progress::AdvancedOne)
    );
}
#[test]
fn unchanged_or_failed_step_is_not_invented_successful_progress() {
    let mut m = Mailbox::<Owner, (), &str>::new();
    let (mut root, mut worker) = m.split().unwrap();
    let g = gate();
    let wait = g.begin_wait(1, 50).unwrap();
    let id = root.submit(owner(), 5).ok().unwrap();
    worker.execute(&wait, 2, |_, _permit| Ok(())).unwrap();
    assert_eq!(root.take(id).unwrap().progress, Progress::Unchanged);
    let id = root.submit(owner(), 5).ok().unwrap();
    worker
        .execute(&wait, 3, |_, _permit| Err("storage"))
        .unwrap();
    let done = root.take(id).unwrap();
    assert_eq!(done.result, Err("storage"));
    assert_eq!(done.owner.step, 3);
    let id = root.submit(owner(), 5).ok().unwrap();
    worker
        .execute(&wait, 4, |j, _permit| {
            j.step += 2;
            Ok(())
        })
        .unwrap();
    assert_eq!(root.take(id).unwrap().progress, Progress::Unexpected);
}
#[test]
fn stale_actual_step_refuses_before_permit_or_callback() {
    let shared = Rc::new(Cell::new(3));
    let mut j = owner();
    j.shared = Some(shared.clone());
    let mut m = Mailbox::<Owner, (), ()>::new();
    let (mut root, mut worker) = m.split().unwrap();
    let id = root.submit(j, 5).ok().unwrap();
    shared.set(4);
    let g = gate();
    let wait = g.begin_wait(1, 50).unwrap();
    assert_eq!(
        worker.execute(&wait, 2, |_, _permit| panic!("must not run")),
        Err(Refusal::StaleStep)
    );
    assert_eq!(root.cancel(id).unwrap().identity, 77);
}
struct Counter(AtomicUsize);
impl Wake for Counter {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn registering_then_queueing_wakes_once_and_ready_registration_is_not_lost() {
    let mut m = Mailbox::<Owner, (), ()>::new();
    let (mut root, mut worker) = m.split().unwrap();
    let counter = Arc::new(Counter(AtomicUsize::new(0)));
    let waker = Waker::from(counter.clone());
    assert!(worker.register(waker).is_pending());
    let id = root.submit(owner(), 1).ok().unwrap();
    assert_eq!(counter.0.load(Ordering::SeqCst), 1);
    assert!(worker.register(Waker::noop().clone()).is_ready());
    drop(root.cancel(id).unwrap());
}
#[test]
fn endpoints_and_ids_never_reset_or_wrap() {
    let mut m = Mailbox::<Owner, (), ()>::new();
    {
        let _ = m.split().unwrap();
    }
    assert!(matches!(m.split(), Err(Refusal::AlreadySplit)));
    let mut m = Mailbox::<Owner, (), ()>::new();
    m.inner.borrow_mut().sequence = u64::MAX;
    let (mut root, _) = m.split().unwrap();
    assert!(matches!(
        root.submit(owner(), 1),
        Err((Refusal::Exhausted, _))
    ));
    assert!(matches!(root.submit(owner(), 0), Err((Refusal::Bounds, _))));
    assert_eq!(root.phase(), Phase::Idle);
}
#[test]
fn panicked_callback_fences_ambiguous_running_state() {
    let mut m = Mailbox::<Owner, (), ()>::new();
    let (mut root, mut worker) = m.split().unwrap();
    let id = root.submit(owner(), 1).ok().unwrap();
    let g = gate();
    let wait = g.begin_wait(1, 50).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = worker.execute(&wait, 2, |_, _permit| panic!("ambiguous callback"));
    }));
    assert!(result.is_err());
    assert_eq!(root.phase(), Phase::Running(id));
    assert!(matches!(root.submit(owner(), 1), Err((Refusal::Busy, _))));
    assert!(matches!(root.cancel(id), Err(Refusal::Busy)));
    assert!(
        wait.try_flash(3, 1).is_ok(),
        "permit dropped while mailbox remains fenced"
    );
}
#[test]
fn empty_mailbox_and_exhausted_step_never_run_a_callback() {
    let mut m = Mailbox::<Owner, (), ()>::new();
    let (mut root, mut worker) = m.split().unwrap();
    let g = gate();
    let wait = g.begin_wait(1, 50).unwrap();
    assert_eq!(
        worker.execute(&wait, 2, |_, _permit| panic!("no queued owner")),
        Ok(None)
    );
    let mut exhausted = owner();
    exhausted.step = u32::MAX;
    assert!(matches!(
        root.submit(exhausted, 1),
        Err((Refusal::Exhausted, _))
    ));
    assert_eq!(root.phase(), Phase::Idle);
}
#[test]
fn completed_cancellation_cannot_discard_affine_success_or_error_custody() {
    struct Custody(Rc<Cell<u32>>);
    impl Drop for Custody {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1)
        }
    }
    for success in [true, false] {
        let drops = Rc::new(Cell::new(0));
        let mut m = Mailbox::<Owner, Custody, Custody>::new();
        let (mut root, mut worker) = m.split().unwrap();
        let id = root.submit(owner(), 1).ok().unwrap();
        let g = gate();
        let wait = g.begin_wait(1, 50).unwrap();
        worker
            .execute(&wait, 2, |j, _permit| {
                j.step += 1;
                let custody = Custody(drops.clone());
                if success {
                    Ok(custody)
                } else {
                    Err(custody)
                }
            })
            .unwrap();
        assert!(matches!(root.cancel(id), Err(Refusal::Busy)));
        assert_eq!(drops.get(), 0);
        let completion = root.take(id).unwrap();
        assert_eq!(drops.get(), 0);
        assert_eq!(completion.owner.identity, 77);
        assert_eq!(completion.result.is_ok(), success);
        drop(completion);
        assert_eq!(drops.get(), 1);
    }
}
