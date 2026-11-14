//! One cumulative work bound; waking never replenishes the polling allowance.
use core::{
    future::Future,
    num::NonZeroU32,
    pin::pin,
    task::{Context, Poll, Waker},
};

pub(crate) fn run<F: Future>(future: F, maximum_polls: NonZeroU32) -> Option<F::Output> {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    for _ in 0..maximum_polls.get() {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return Some(value),
            Poll::Pending => core::hint::spin_loop(),
        }
    }
    // The future is dropped before the native owner quarantines its controller.
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::{cell::Cell, pin::Pin};

    struct Transfer<'a> {
        polls: &'a Cell<u32>,
        dropped: &'a Cell<bool>,
        ready_at: Option<u32>,
        wake: bool,
        result: Result<usize, u8>,
    }
    impl Future for Transfer<'_> {
        type Output = Result<usize, u8>;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let count = self.polls.get() + 1;
            self.polls.set(count);
            if self.ready_at == Some(count) {
                return Poll::Ready(self.result);
            }
            if self.wake {
                cx.waker().wake_by_ref();
            }
            Poll::Pending
        }
    }
    impl Drop for Transfer<'_> {
        fn drop(&mut self) {
            self.dropped.set(true);
        }
    }
    #[test]
    fn stalled_and_repeatedly_waking_transfers_spend_the_same_finite_allowance() {
        for wake in [false, true] {
            for limit in 1..=64 {
                let polls = Cell::new(0);
                let dropped = Cell::new(false);
                let result = run(
                    Transfer {
                        polls: &polls,
                        dropped: &dropped,
                        ready_at: None,
                        wake,
                        result: Ok(32),
                    },
                    NonZeroU32::new(limit).unwrap(),
                );
                assert_eq!(result, None);
                assert_eq!(polls.get(), limit);
                assert!(
                    dropped.get(),
                    "borrowed storage must be released before quarantine"
                );
            }
        }
    }
    #[test]
    fn completion_at_the_last_poll_preserves_success_and_failure_without_retry() {
        for result in [Ok(32), Err(7)] {
            let polls = Cell::new(0);
            let dropped = Cell::new(false);
            let actual = run(
                Transfer {
                    polls: &polls,
                    dropped: &dropped,
                    ready_at: Some(4),
                    wake: true,
                    result,
                },
                NonZeroU32::new(4).unwrap(),
            );
            assert_eq!(actual, Some(result));
            assert_eq!(polls.get(), 4);
            assert!(dropped.get());
        }
    }
}
