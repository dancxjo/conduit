#![cfg(feature = "host-call")]
extern crate alloc;
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::selected_operation::SelectedOperationPlan;
use conduit_pulse_capture::{owner::*, *};
use std::{
    cell::Cell,
    future::Future,
    rc::Rc,
    task::{Context, Poll, Waker},
};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "support/owner_fixture.rs"]
mod fixture;
#[path = "../../../../targets/conduitos/src/machine_membrane/selection_fixture.rs"]
mod selection_fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[derive(Default)]
struct State {
    effects: Cell<usize>,
    cancelled: Cell<usize>,
    pending: Cell<bool>,
    malformed: Cell<bool>,
}
#[derive(Default)]
struct Provider(Rc<State>);
impl PulseReadProvider for Provider {
    async fn read(&mut self, _: CheckedPulseRead) -> Result<PulseReadCapture, PulseReadRefusal> {
        self.0.effects.set(self.0.effects.get() + 1);
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
        let mut capture = PulseReadCapture::from_complete_octets(
            PulseReadDisposition::Complete,
            &[56, 0, 19, 8, 83],
        )
        .unwrap();
        if self.0.malformed.get() {
            capture.wire[7] = 1;
        }
        Ok(capture)
    }
}
fn limits() -> PulseReadLimits {
    PulseReadLimits {
        maximum_hold_low_us: 50_000,
        maximum_skipped_pulses: 4,
        minimum_sample_offset_us: 10,
        maximum_sample_offset_us: 200,
        maximum_octets: 8,
        maximum_capture_timeout_us: 50_000,
    }
}
fn request(octets: u8) -> Vec<u8> {
    use conduit_plot::*;
    let source = format!(
        "{}\n{}",
        include_str!("../../../../plots/device-protocols/pulse-types.conduit"),
        include_str!("../../../../plots/device-protocols/dht11-capture.conduit")
            .replace("octets: 5", &format!("octets: {octets}"))
    );
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "dht11-capture-request",
        &ProfileCatalog::new(),
    )
    .unwrap();
    let ConfigurationValue::Text(program) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("Source expression")
    };
    PortableExpressionProgram::from_canonical_hex(program)
        .unwrap()
        .evaluate(&[])
        .unwrap()
}
fn ready<T>(future: impl core::future::Future<Output = T>) -> T {
    let mut future = core::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("immediate fixture"),
    }
}
fn is_tag(bytes: &[u8], tag: &str) -> bool {
    validate_canonical_structured_value(bytes)
        .unwrap()
        .variant_payload(tag)
        .unwrap()
        .is_some()
}

#[test]
fn exact_binding_and_replay_are_checked_before_any_effect() {
    let provider = Provider::default();
    let state = provider.0.clone();
    let (mut owner, node) = fixture::owner(provider);
    let input = request(5);
    assert_eq!(
        ready(owner.invoke(node, HostCallId(1), RequestId(0), &input)).err(),
        Some(PulseCallRefusal::WrongBinding)
    );
    assert_eq!(
        ready(owner.invoke(node, HostCallId(0), RequestId(1), &input)).err(),
        Some(PulseCallRefusal::StaleRequest)
    );
    assert!(
        ready(owner.invoke(node, HostCallId(0), RequestId(0), &input[..input.len() - 1])).is_err()
    );
    assert_eq!(state.effects.get(), 0);
    let (_, measured) = allocation_probe::observe(|| {
        assert!(is_tag(
            ready(owner.invoke(node, HostCallId(0), RequestId(0), &input)).unwrap(),
            "completed"
        ));
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
    assert_eq!(
        ready(owner.invoke(node, HostCallId(0), RequestId(0), &input)).err(),
        Some(PulseCallRefusal::StaleRequest)
    );
    assert_eq!(state.effects.get(), 1);
    assert_eq!(owner.inspection().completed_operations, 1);
}

#[test]
fn refused_geometry_and_malformed_completion_consume_once_and_release_leases() {
    let provider = Provider::default();
    let state = provider.0.clone();
    let (mut owner, node) = fixture::owner(provider);
    let invalid = request(9);
    assert!(is_tag(
        ready(owner.invoke(node, HostCallId(0), RequestId(0), &invalid)).unwrap(),
        "geometry-refused"
    ));
    assert_eq!(state.effects.get(), 0);
    let input = request(5);
    state.malformed.set(true);
    assert_eq!(
        ready(owner.invoke(node, HostCallId(0), RequestId(1), &input)).err(),
        Some(PulseCallRefusal::Codec(PulseCodecRefusal::InvalidCapture))
    );
    state.malformed.set(false);
    assert!(ready(owner.invoke(node, HostCallId(0), RequestId(2), &input)).is_ok());
    assert_eq!(owner.inspection().in_flight_operations, 0);
    assert_eq!(owner.inspection().completed_operations, 3);
    owner.revoke(node, HostCallId(0)).unwrap();
    assert!(matches!(
        ready(owner.invoke(node, HostCallId(0), RequestId(3), &input)),
        Err(PulseCallRefusal::Capability(BaseCapabilityRefusal::Revoked))
    ));
    assert_eq!(state.effects.get(), 2);
}

#[test]
fn dropping_a_pending_dispatch_quiesces_provider_and_revokes_unfinished_possession() {
    let provider = Provider::default();
    let state = provider.0.clone();
    state.pending.set(true);
    let (mut owner, node) = fixture::owner(provider);
    let input = request(5);
    let (_, measured) = allocation_probe::observe(|| {
        let mut future = core::pin::pin!(owner.invoke(node, HostCallId(0), RequestId(0), &input));
        assert!(future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending());
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
    assert_eq!(state.cancelled.get(), 1);
    assert_eq!(owner.inspection().in_flight_operations, 0);
    assert_eq!(owner.inspection().lifecycle, CapabilityLifecycle::Revoked);
    assert!(matches!(
        ready(owner.invoke(node, HostCallId(0), RequestId(1), &input)),
        Err(PulseCallRefusal::Capability(BaseCapabilityRefusal::Revoked))
    ));
    assert_eq!(state.effects.get(), 1);
}

#[test]
fn matching_identifiers_cannot_substitute_lowering_or_active_play() {
    let (fragment, mut lowered, active, placement) = fixture::selected();
    lowered.host_calls[0].binding.maximum_output_bytes += 1;
    assert_eq!(
        fixture::bind(
            &fragment,
            &lowered,
            &active,
            &placement,
            Provider::default()
        )
        .err(),
        Some(PulseCallRefusal::WrongBinding)
    );
    let (fragment, lowered, mut active, placement) = fixture::selected();
    active.active_play_id = "play/stale".into();
    assert_eq!(
        fixture::bind(
            &fragment,
            &lowered,
            &active,
            &placement,
            Provider::default()
        )
        .err(),
        Some(PulseCallRefusal::WrongBinding)
    );
}
