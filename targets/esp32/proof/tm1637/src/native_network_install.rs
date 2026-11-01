//! Scheduling fence only. Actual owner selection stays in NativePlay::install_held.
//! Neither this phase nor release establishes Body or heap admission.
pub(crate) fn release(phase: &mut &'static str, complete: bool) -> Result<(), ()> {
    if *phase != "installed-held" || !complete {
        return Err(());
    }
    *phase = "installed";
    Ok(())
}
/// Must precede even borrowing the kernel for Step/Host Call dispatch.
pub(crate) fn begin_run(phase: &mut &'static str, complete: bool) -> bool {
    if *phase != "installed" || !complete {
        return false;
    }
    *phase = "running";
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn held_never_enters_kernel_and_remains_selected_on_capacity_refusal() {
        let mut phase = "installed-held";
        let mut effects = 0;
        for _ in 0..3 {
            if begin_run(&mut phase, true) {
                effects += 1;
            }
            assert_eq!(phase, "installed-held");
        }
        assert_eq!(effects, 0);
        release(&mut phase, true).unwrap();
        assert!(begin_run(&mut phase, true));
        effects += 1;
        assert_eq!(effects, 1);
        assert!(!begin_run(&mut phase, true));
        assert!(release(&mut phase, true).is_err());
    }
    #[test]
    fn polling_held_dispatch_future_never_polls_effect_future() {
        use core::{
            future::Future,
            pin::pin,
            task::{Context, Poll, Waker},
        };
        use std::cell::Cell;
        async fn invocation(phase: &mut &'static str, polls: &Cell<u32>) {
            if !begin_run(phase, true) {
                return;
            }
            core::future::poll_fn(|_| {
                polls.set(polls.get() + 1);
                Poll::Ready(())
            })
            .await;
        }
        let polls = Cell::new(0);
        let mut phase = "installed-held";
        let mut context = Context::from_waker(Waker::noop());
        {
            let mut held = pin!(invocation(&mut phase, &polls));
            assert!(held.as_mut().poll(&mut context).is_ready());
        }
        assert_eq!(polls.get(), 0);
        assert_eq!(phase, "installed-held");
        release(&mut phase, true).unwrap();
        let mut released = pin!(invocation(&mut phase, &polls));
        assert!(released.as_mut().poll(&mut context).is_ready());
        assert_eq!(polls.get(), 1);
    }
    #[test]
    fn partial_installation_and_other_phases_cannot_release_or_run() {
        for initial in ["uninstalled", "installation-refused", "running", "attached"] {
            let mut phase = initial;
            assert!(release(&mut phase, true).is_err());
            assert!(!begin_run(&mut phase, true));
            assert_eq!(phase, initial);
        }
        let mut phase = "installed-held";
        assert!(release(&mut phase, false).is_err());
        assert_eq!(phase, "installed-held");
        phase = "installed";
        assert!(!begin_run(&mut phase, false));
        assert_eq!(phase, "installed");
    }
}
