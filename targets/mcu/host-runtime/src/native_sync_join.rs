//! Fair cooperative grouping of existing actors; no scheduler or authority.
use core::{
    future::{poll_fn, Future},
    pin::pin,
    task::Poll,
};
pub(super) async fn join<A, B, E>(a: A, b: B) -> Result<(), E>
where
    A: Future<Output = Result<(), E>>,
    B: Future<Output = Result<(), E>>,
{
    let (mut a, mut b) = (pin!(a), pin!(b));
    let mut done = [false; 2];
    poll_fn(|cx| {
        if !done[0] {
            if let Poll::Ready(result) = a.as_mut().poll(cx) {
                result?;
                done[0] = true;
            }
        }
        if !done[1] {
            if let Poll::Ready(result) = b.as_mut().poll(cx) {
                result?;
                done[1] = true;
            }
        }
        if done.iter().all(|done| *done) {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    })
    .await
}
