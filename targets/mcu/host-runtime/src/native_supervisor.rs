//! Fair, allocation-free polling of four admitted service actors. It creates no
//! Play or authority. A caller supplies already authenticated kernel/services.
use core::{
    future::{poll_fn, Future},
    pin::pin,
    task::Poll,
};

/// Poll each active actor once per wake. On refusal all owned actor futures are
/// dropped before this future returns. The caller may only then cancel kernels:
/// provider pending-lease guards have quiesced before request/storage retirement.
/// Transport loss must not be supplied as an actor refusal for local Plays.
pub async fn supervise<A, B, C, D, E>(a: A, b: B, c: C, d: D) -> Result<(), E>
where
    A: Future<Output = Result<(), E>>,
    B: Future<Output = Result<(), E>>,
    C: Future<Output = Result<(), E>>,
    D: Future<Output = Result<(), E>>,
{
    let (mut a, mut b, mut c, mut d) = (pin!(a), pin!(b), pin!(c), pin!(d));
    let mut done = [false; 4];
    poll_fn(|cx| {
        macro_rules! actor {
            ($index:expr, $future:ident) => {
                if !done[$index] {
                    if let Poll::Ready(result) = $future.as_mut().poll(cx) {
                        done[$index] = true;
                        if let Err(error) = result {
                            return Poll::Ready(Err(error));
                        }
                    }
                }
            };
        }
        actor!(0, a);
        actor!(1, b);
        actor!(2, c);
        actor!(3, d);
        if done.iter().all(|done| *done) {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::{
        cell::Cell,
        task::{Context, Waker},
    };
    struct PendingProvider<'a>(&'a Cell<u8>);
    impl Future for PendingProvider<'_> {
        type Output = Result<(), ()>;
        fn poll(self: core::pin::Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            self.0.set(self.0.get() | 1);
            Poll::Pending
        }
    }
    impl Drop for PendingProvider<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() | 2);
        }
    }
    #[test]
    fn refusal_quiesces_provider_before_supervisor_returns() {
        let state = Cell::new(0);
        let mut future = pin!(supervise(
            PendingProvider(&state),
            async { Err(()) },
            async { Ok(()) },
            async { Ok(()) }
        ));
        let mut cx = Context::from_waker(Waker::noop());
        assert_eq!(future.as_mut().poll(&mut cx), Poll::Ready(Err(())));
        assert_eq!(
            state.get(),
            3,
            "provider was polled fairly and dropped before return"
        );
    }
}
