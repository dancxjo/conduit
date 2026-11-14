use super::super::super::CooperativeYield;
use super::*;

struct Yield {
    calls: Cell<usize>,
}
impl Yield {
    fn new() -> Self {
        Self {
            calls: Cell::new(0),
        }
    }
}
impl CooperativeYield for Yield {
    async fn pause(&self) {
        self.calls.set(self.calls.get() + 1);
        let mut pending = true;
        core::future::poll_fn(|cx| {
            if pending {
                pending = false;
                cx.waker().wake_by_ref();
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await;
    }
}
struct Work<'a> {
    activity: &'a NativeActivity,
    polls: &'a Cell<usize>,
    pending: bool,
    result: Result<u32, &'static str>,
}
impl Future for Work<'_> {
    type Output = Result<u32, &'static str>;
    fn poll(mut self: core::pin::Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        assert!(self.activity.try_timing().is_none());
        assert!(self.activity.try_blocking().is_none());
        self.polls.set(self.polls.get() + 1);
        if self.pending {
            self.pending = false;
            Poll::Pending
        } else {
            Poll::Ready(self.result)
        }
    }
}
fn poll<F: Future>(future: core::pin::Pin<&mut F>) -> Poll<F::Output> {
    let (result, cost) = crate::member_return_tests::return_allocation_probe::observe(|| {
        future.poll(&mut Context::from_waker(Waker::noop()))
    });
    assert_eq!(
        cost.allocation_attempts, 0,
        "prepared poll attempted allocation"
    );
    result
}
#[test]
fn timing_contention_defers_original_future_and_yields_without_holding_window() {
    let activity = NativeActivity::new();
    let yield_to_host = Yield::new();
    let polls = Cell::new(0);
    let timing = activity.try_timing().unwrap();
    let mut future = pin!(activity.with_blocking_polls(
        Work {
            activity: &activity,
            polls: &polls,
            pending: false,
            result: Ok(7)
        },
        &yield_to_host
    ));
    assert_eq!(poll(future.as_mut()), Poll::Pending);
    assert_eq!(polls.get(), 0);
    assert_eq!(yield_to_host.calls.get(), 1);
    // Waiting for the caller's yield never retains a blocking guard.
    let concurrent = activity.try_timing().unwrap();
    drop(concurrent);
    drop(timing);
    assert_eq!(poll(future.as_mut()), Poll::Ready(Ok(7)));
    assert_eq!(polls.get(), 1);
    assert!(activity.try_timing().is_some());
    assert!(activity.try_blocking().is_some());
}
#[test]
fn pending_work_releases_window_and_rechecks_timing_before_next_poll() {
    let activity = NativeActivity::new();
    let yield_to_host = Yield::new();
    let polls = Cell::new(0);
    let mut future = pin!(activity.with_blocking_polls(
        Work {
            activity: &activity,
            polls: &polls,
            pending: true,
            result: Ok(8)
        },
        &yield_to_host
    ));
    assert_eq!(poll(future.as_mut()), Poll::Pending);
    assert_eq!(polls.get(), 1);
    let timing = activity.try_timing().unwrap();
    assert_eq!(poll(future.as_mut()), Poll::Pending);
    assert_eq!(polls.get(), 1);
    assert_eq!(yield_to_host.calls.get(), 1);
    drop(timing);
    assert_eq!(poll(future.as_mut()), Poll::Ready(Ok(8)));
    assert_eq!(polls.get(), 2);
    assert!(activity.try_blocking().is_some());
}
#[test]
fn cancelling_waiting_work_or_yield_leaves_no_blocking_window() {
    for contention in [false, true] {
        let activity = NativeActivity::new();
        let yield_to_host = Yield::new();
        let polls = Cell::new(0);
        let timing = contention.then(|| activity.try_timing().unwrap());
        {
            let mut future = pin!(activity.with_blocking_polls(
                Work {
                    activity: &activity,
                    polls: &polls,
                    pending: true,
                    result: Ok(0)
                },
                &yield_to_host
            ));
            assert_eq!(poll(future.as_mut()), Poll::Pending);
        }
        drop(timing);
        assert!(activity.try_blocking().is_some());
        assert!(activity.try_timing().is_some());
        assert_eq!(polls.get(), usize::from(!contention));
    }
}
#[test]
fn success_and_refusal_outputs_are_returned_unchanged_without_allocation() {
    for result in [Ok(123), Err("provider refused")] {
        let activity = NativeActivity::new();
        let yield_to_host = Yield::new();
        let polls = Cell::new(0);
        let mut future = pin!(activity.with_blocking_polls(
            Work {
                activity: &activity,
                polls: &polls,
                pending: false,
                result
            },
            &yield_to_host
        ));
        assert_eq!(poll(future.as_mut()), Poll::Ready(result));
        assert_eq!(polls.get(), 1);
        assert_eq!(yield_to_host.calls.get(), 0);
        assert!(activity.try_timing().is_some());
    }
}
