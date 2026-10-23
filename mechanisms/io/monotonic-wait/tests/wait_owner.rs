//! CPU timer, current identities and capability issuance are explicit fixtures.
extern crate alloc;
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_monotonic_wait::*;
use conduit_plan_lowering::selected_operation::SelectedOperationPlan;
use std::{
    cell::Cell,
    future::Future,
    rc::Rc,
    task::{Context, Poll, Waker},
};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "support/fixture.rs"]
mod fixture;
#[path = "../../../../targets/conduitos/src/machine_membrane/selection_fixture.rs"]
mod selection_fixture;
#[derive(Default)]
struct State {
    calls: Cell<u32>,
    duration: Cell<u64>,
    pending: Cell<bool>,
    cancelled: Cell<u32>,
    fail: Cell<bool>,
}
struct Provider(Rc<State>);
impl MonotonicWaitProvider for Provider {
    async fn wait(&mut self, duration: u64) -> Result<(), WaitProviderFailure> {
        self.0.calls.set(self.0.calls.get() + 1);
        self.0.duration.set(duration);
        struct Guard<'a>(&'a State, bool);
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                if !self.1 {
                    self.0.cancelled.set(self.0.cancelled.get() + 1);
                }
            }
        }
        let mut guard = Guard(&self.0, false);
        if self.0.pending.get() {
            core::future::pending::<()>().await;
        }
        guard.1 = true;
        if self.0.fail.get() {
            Err(WaitProviderFailure::Pressure)
        } else {
            Ok(())
        }
    }
}
fn immediate<T>(future: impl Future<Output = T>) -> T {
    let mut future = core::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("fixture unexpectedly pending"),
    }
}
#[test]
fn bounded_waits_are_exact_sequenced_and_allocation_free() {
    let state = Rc::new(State::default());
    let (mut owner, node) = fixture::owner(Provider(state.clone()));
    let ((), memory) = allocation_probe::observe(|| {
        assert_eq!(
            immediate(owner.invoke(
                NodeId(node.0 + 1),
                HostCallId(0),
                RequestId(0),
                &encode_monotonic_duration(1)
            )),
            Err(WaitCallRefusal::WrongBinding)
        );
        assert_eq!(
            immediate(owner.invoke(
                node,
                HostCallId(1),
                RequestId(0),
                &encode_monotonic_duration(1)
            )),
            Err(WaitCallRefusal::WrongBinding)
        );
        assert_eq!(
            immediate(owner.invoke(node, HostCallId(0), RequestId(0), &[0; 7])),
            Err(WaitCallRefusal::InvalidDuration)
        );
        assert_eq!(
            immediate(owner.invoke(
                node,
                HostCallId(0),
                RequestId(0),
                &encode_monotonic_duration(1001)
            )),
            Err(WaitCallRefusal::InvalidDuration)
        );
        assert_eq!(state.calls.get(), 0);
        assert_eq!(
            immediate(owner.invoke(
                node,
                HostCallId(0),
                RequestId(0),
                &encode_monotonic_duration(0)
            )),
            Ok(())
        );
        assert_eq!(state.duration.get(), 0);
        assert_eq!(
            immediate(owner.invoke(
                node,
                HostCallId(0),
                RequestId(0),
                &encode_monotonic_duration(1)
            )),
            Err(WaitCallRefusal::StaleRequest)
        );
        assert_eq!(
            immediate(owner.invoke(
                node,
                HostCallId(0),
                RequestId(1),
                &encode_monotonic_duration(1000)
            )),
            Ok(())
        );
        assert_eq!(state.duration.get(), 1000);
        assert_eq!(state.calls.get(), 2);
        state.fail.set(true);
        assert_eq!(
            immediate(owner.invoke(
                node,
                HostCallId(0),
                RequestId(2),
                &encode_monotonic_duration(1)
            )),
            Err(WaitCallRefusal::Provider(WaitProviderFailure::Pressure))
        );
        assert_eq!(owner.inspection().completed_operations, 3);
        owner.revoke(node, HostCallId(0)).unwrap();
        assert!(immediate(owner.invoke(
            node,
            HostCallId(0),
            RequestId(3),
            &encode_monotonic_duration(1)
        ))
        .is_err());
        assert_eq!(state.calls.get(), 3);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn dropping_pending_wait_quiesces_provider_then_revokes_unfinished_possession() {
    let state = Rc::new(State::default());
    state.pending.set(true);
    let (mut owner, node) = fixture::owner(Provider(state.clone()));
    let bytes = encode_monotonic_duration(1);
    let ((), memory) = allocation_probe::observe(|| {
        {
            let mut wait = core::pin::pin!(owner.invoke(node, HostCallId(0), RequestId(0), &bytes));
            assert!(wait
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending());
            assert_eq!(state.calls.get(), 1);
        }
        assert_eq!(state.cancelled.get(), 1);
        assert_eq!(owner.inspection().lifecycle, CapabilityLifecycle::Revoked);
        assert!(immediate(owner.invoke(node, HostCallId(0), RequestId(1), &bytes)).is_err());
        assert_eq!(state.calls.get(), 1);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn foreign_plan_and_play_cannot_bind_the_retained_native_slot() {
    let (fragment, lowered, mut active, placement) = fixture::selected();
    active.boot_id = "fixture/foreign".into();
    let state = Rc::new(State::default());
    assert!(fixture::bind(
        &fragment,
        &lowered,
        &active,
        &placement,
        Provider(state.clone())
    )
    .is_err());
    assert_eq!(state.calls.get(), 0);
}

#[test]
fn a_resealed_plan_cannot_replace_independently_retained_tick_configuration() {
    let (mut fragment, _, _, placement) = fixture::selected();
    fragment.placements[0].configuration[1].value = ConfigurationValue::U64(500);
    let identity = PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let fragment = seal_plan(identity, vec![fragment]).fragments.remove(0);
    assert!(verify_plan_fragment(&fragment));
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    let state = Rc::new(State::default());
    assert!(matches!(
        fixture::bind(
            &fragment,
            &lowered,
            &active,
            &placement,
            Provider(state.clone())
        ),
        Err(WaitCallRefusal::WrongBinding)
    ));
    assert_eq!(state.calls.get(), 0);
}

#[path = "support/installation.rs"]
mod installation;

#[path = "support/assigned.rs"]
mod assigned;
