//! A poll boundary keeps successive asynchronous phases from being flattened
//! into one poll function's stack frame. Storage remains in the original
//! future; this wrapper allocates nothing and changes no execution authority.
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pub struct PollBoundary<F>(F);

pub fn separate<F: Future>(future: F) -> PollBoundary<F> {
    PollBoundary(future)
}

impl<F: Future> Future for PollBoundary<F> {
    type Output = F::Output;

    #[inline(never)]
    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: the wrapped future is never moved after the wrapper is pinned.
        unsafe { self.map_unchecked_mut(|this| &mut this.0) }.poll(context)
    }
}
