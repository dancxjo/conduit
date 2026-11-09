use super::*;
use core::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

#[test]
fn timing_set_excludes_blocking_until_last_drop() {
    let activity = NativeActivity::new();
    let first = activity.try_timing().unwrap();
    let second = activity.try_timing().unwrap();
    assert!(activity.try_blocking().is_none());
    drop(first);
    assert!(activity.try_blocking().is_none());
    drop(second);
    let blocking = activity.try_blocking().unwrap();
    assert!(activity.try_timing().is_none());
    assert!(activity.try_blocking().is_none());
    drop(blocking);
    assert!(activity.try_timing().is_some());
}

#[test]
fn cancelling_pending_provider_releases_timing() {
    let activity = NativeActivity::new();
    {
        let future = async {
            let _guard = activity.try_timing().unwrap();
            core::future::pending::<()>().await;
        };
        let mut future = pin!(future);
        let mut cx = Context::from_waker(Waker::noop());
        assert_eq!(future.as_mut().poll(&mut cx), Poll::Pending);
        assert!(activity.try_blocking().is_none());
    }
    assert!(activity.try_blocking().is_some());
}

#[test]
fn overflow_refuses_without_losing_existing_timing() {
    let activity = NativeActivity::new();
    activity.timing.set(usize::MAX);
    assert!(activity.try_timing().is_none());
    assert!(activity.try_blocking().is_none());
    assert_eq!(activity.timing.get(), usize::MAX);
}
