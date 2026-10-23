//! CPU providers only; no physical radio, initialized advertisement or Body.
extern crate alloc;
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_net::*;
use conduit_network_join_call::installation::*;
use conduit_network_join_call::*;
use conduit_plan_lowering::{
    lowering::lower_plan_fragment, selected_operation::SelectedOperationPlan,
};
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
#[path = "../../../../targets/conduitos/src/machine_membrane/selection_fixture.rs"]
mod fixture;
#[path = "../../../../architecture/plan-lowering/tests/common/native_possession.rs"]
mod possession;

#[derive(Default)]
struct State {
    effects: Cell<u32>,
    cancelled: Cell<u32>,
    cleanups: Cell<u32>,
    pending: Cell<bool>,
    failure: Cell<Option<StationFailure>>,
    foreign: Cell<u8>,
    cleanup_failed: Cell<bool>,
    cleanup_pressure: Cell<bool>,
    correct_request: Cell<bool>,
}
struct Provider(Rc<State>);
impl NetworkJoinProvider for Provider {
    async fn join(
        &mut self,
        request: NetworkJoinRequest<'_>,
    ) -> Result<AttachmentRecord, StationAttemptFailure> {
        self.0.effects.set(self.0.effects.get() + 1);
        self.0
            .correct_request
            .set(request.ssid == b"fixture-net" && request.credential == b"fixture-password");
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
        if let Some(reason) = self.0.failure.get() {
            return Err(StationAttemptFailure {
                phase: StationPhase::Associate,
                reason,
            });
        }
        Ok(AttachmentRecord::encode(NetworkAttachmentInfo {
            attachment_id: "attachment/actual-result",
            host_id: if self.0.foreign.get() == 1 {
                "host/foreign"
            } else {
                "host/network"
            },
            boot_id: if self.0.foreign.get() == 2 {
                "boot/old"
            } else {
                "boot/current"
            },
            resource_pool_id: if self.0.foreign.get() == 3 {
                "pool/other"
            } else {
                "pool/radio"
            },
            generation: if self.0.foreign.get() == 4 { 8 } else { 9 },
        })
        .unwrap())
    }
    async fn cleanup(&mut self) -> Result<(), StationFailure> {
        self.0.cleanups.set(self.0.cleanups.get() + 1);
        if self.0.cleanup_pressure.get() {
            Err(StationFailure::Pressure)
        } else if self.0.cleanup_failed.get() {
            Err(StationFailure::ProviderUnavailable)
        } else {
            Ok(())
        }
    }
}
fn ready(state: Rc<State>) -> (NetworkJoinHostCall<Provider>, NodeId) {
    prepare_owner(state, 0).unwrap()
}
fn prepare_owner(
    state: Rc<State>,
    substitution: u8,
) -> Result<(NetworkJoinHostCall<Provider>, NodeId), JoinCallRefusal> {
    let contract = PreparedJoinContract::prepare();
    let mut scope = possession::scope(NETWORK_CONFIG_SUBJECT);
    scope.authority_contract_id = NETWORK_CONFIG_AUTHORITY.into();
    scope.operation_contract_id = NETWORK_JOIN_HOST_CALL.into();
    scope.maximum_parameter_bytes = MAXIMUM_JOIN_INPUT_BYTES;
    scope.maximum_result_bytes = MAXIMUM_JOIN_OUTPUT_BYTES;
    scope.capability_id = JOIN_CAPABILITY.into();
    let identity = NetworkNativeIdentity {
        host: scope.host_id.clone(),
        boot: scope.boot_id.clone(),
        base: "base/join".into(),
        instance: scope.base_instance_id.clone(),
        provider_generation: scope.base_provider_generation,
        resource: scope.resource_pool_id.clone(),
        resource_generation: scope.resource_generation_id.clone(),
        envelope: scope.envelope_id.clone(),
        artifact: "artifact/join/fixture".into(),
        implementation: scope.implementation_id.clone(),
        base_implementation: "base/join/fixture@1".into(),
    };
    // SAFETY: CPU-only inert provider; this establishes no physical availability.
    let ready = unsafe { ReadyNetworkJoinBase::new(identity, 9, Provider(state)) }.unwrap();
    if substitution == 2 {
        scope.resource_generation_id = ResourceGenerationId("generation/other".into());
    }

    let (mut fragment, _, _, placement) = fixture::selected(
        &scope,
        contract.kind(),
        NETWORK_JOIN_HOST_CALL,
        MAXIMUM_JOIN_INPUT_BYTES,
        "radio",
        "join",
    );
    fragment.placements[0].host_calls[0].target_kind = Some(NETWORK_CONFIG_SUBJECT.into());
    fragment.placements[0].resources[0].class_id = WIFI_STATION_RESOURCE_CLASS.into();
    fragment.placements[0].execution_profile_id = NETWORK_JOIN_EXECUTION_PROFILE.into();
    fragment.placements[0]
        .base
        .as_mut()
        .unwrap()
        .mechanism_family = JOIN_BASE_KIND.into();
    if substitution == 1 {
        fragment.placements[0].artifact_id = "artifact/other".into();
    }
    if substitution == 3 {
        fragment.placements[0].execution_profile_id = "profile/other".into();
    }
    if substitution == 4 {
        fragment.placements[0]
            .base
            .as_mut()
            .unwrap()
            .implementation_id = "base/other".into();
    }

    let identity = PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let fragment = seal_plan(identity, vec![fragment]).fragments.remove(0);
    let mut lowered = lower_plan_fragment(&fragment).unwrap();
    if substitution == 5 {
        lowered.node_specs[0].maximum_step_fuel += 1;
    }
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 3);
    scope.plan_id = fragment.plan_id.clone();
    scope.active_play_id = active.active_play_id.clone();
    let (table, handle, claim) = possession::possession(scope.clone());
    let node = lowered.nodes[0].node;
    let mut retained = Some(ready);
    let result = ReadyNetworkJoinBase::bind_selected_retained(
        &mut retained,
        table,
        handle,
        claim,
        &contract,
        SelectedOperationPlan {
            fragment: &fragment,
            lowered: &lowered,
            active: &active,
            placement_id: &placement,
        },
    );
    match result {
        Ok(owner) => {
            assert!(retained.is_none());
            let (table, handle, claim) = possession::possession(scope);
            assert!(matches!(
                ReadyNetworkJoinBase::bind_selected_retained(
                    &mut retained,
                    table,
                    handle,
                    claim,
                    &contract,
                    SelectedOperationPlan {
                        fragment: &fragment,
                        lowered: &lowered,
                        active: &active,
                        placement_id: &placement,
                    }
                ),
                Err(JoinCallRefusal::OwnerUnavailable)
            ));
            Ok((owner, node))
        }
        Err(error) => {
            assert!(
                retained.is_some(),
                "refusal must retain the actual fixture provider"
            );
            assert_eq!(
                retained.as_ref().unwrap().native_identity().boot.as_str(),
                "boot/current"
            );
            Err(error)
        }
    }
}
fn request() -> ([u8; MAXIMUM_JOIN_INPUT_BYTES as usize], usize) {
    let mut bytes = [0; MAXIMUM_JOIN_INPUT_BYTES as usize];
    let length = encode_network_join_request(
        NetworkJoinRequest {
            ssid: b"fixture-net",
            credential: b"fixture-password",
        },
        &mut bytes,
    )
    .unwrap();
    (bytes, length)
}
fn run<T>(future: impl Future<Output = T>) -> T {
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
fn only_exact_request_dispatches_and_result_and_cleanup_allocate_nothing() {
    let state = Rc::new(State::default());
    let (mut owner, node) = ready(state.clone());
    let (bytes, length) = request();
    assert!(matches!(
        run(owner.invoke(node, HostCallId(1), RequestId(0), &bytes[..length])),
        Err(JoinCallRefusal::WrongBinding)
    ));
    assert!(matches!(
        run(owner.invoke(node, HostCallId(0), RequestId(1), &bytes[..length])),
        Err(JoinCallRefusal::StaleRequest)
    ));
    assert!(matches!(
        run(owner.invoke(node, HostCallId(0), RequestId(0), b"bad")),
        Err(JoinCallRefusal::Request(_))
    ));
    assert_eq!(state.effects.get(), 0);
    let (generation, allocation) = allocation_probe::observe(|| {
        decode_network_attachment(
            run(owner.invoke(node, HostCallId(0), RequestId(0), &bytes[..length])).unwrap(),
        )
        .unwrap()
        .generation
    });
    assert_eq!(generation, 9);
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert!(state.correct_request.get());
    assert!(owner.cleanup_required());
    assert_eq!(owner.inspection().completed_operations, 1);
    assert!(matches!(
        run(owner.invoke(node, HostCallId(0), RequestId(0), &bytes[..length])),
        Err(JoinCallRefusal::StaleRequest)
    ));
    let ((), allocation) = allocation_probe::observe(|| {
        run(owner.cleanup()).unwrap();
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert!(!owner.cleanup_required());
    run(owner.cleanup()).unwrap();
    assert_eq!(state.cleanups.get(), 1);
}
#[test]
fn cancellation_revokes_unfinished_possession_and_retains_bounded_cleanup_obligation() {
    let state = Rc::new(State::default());
    state.pending.set(true);
    let (mut owner, node) = ready(state.clone());
    let (bytes, length) = request();
    {
        let mut call =
            core::pin::pin!(owner.invoke(node, HostCallId(0), RequestId(0), &bytes[..length]));
        assert!(call
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending());
    }
    assert_eq!(state.cancelled.get(), 1);
    assert!(owner.cleanup_required());
    assert_eq!(owner.inspection().lifecycle, CapabilityLifecycle::Revoked);
    state.cleanup_failed.set(true);
    assert_eq!(
        run(owner.cleanup()),
        Err(StationFailure::ProviderUnavailable)
    );
    assert!(owner.cleanup_required());
    state.cleanup_failed.set(false);
    run(owner.cleanup()).unwrap();
    assert!(!owner.cleanup_required());
    assert_eq!(state.cleanups.get(), 2);
    assert!(matches!(
        run(owner.invoke(node, HostCallId(0), RequestId(0), &bytes[..length])),
        Err(JoinCallRefusal::StaleRequest)
    ));
}
#[test]
fn foreign_attachment_and_provider_failure_never_publish_success() {
    for foreign in 0..=4 {
        let state = Rc::new(State::default());
        state.foreign.set(foreign);
        state
            .failure
            .set((foreign == 0).then_some(StationFailure::Rejected));
        let (mut owner, node) = ready(state.clone());
        let (bytes, length) = request();
        let result = run(owner.invoke(node, HostCallId(0), RequestId(0), &bytes[..length]));
        if foreign != 0 {
            assert!(matches!(result, Err(JoinCallRefusal::InvalidAttachment)));
        } else {
            assert!(matches!(
                result,
                Err(JoinCallRefusal::Provider(StationAttemptFailure {
                    phase: StationPhase::Associate,
                    reason: StationFailure::Rejected
                }))
            ));
        }
        assert_eq!(owner.inspection().completed_operations, 1);
        assert!(owner.cleanup_required());
        run(owner.cleanup()).unwrap();
        assert_eq!(state.cleanups.get(), 1);
    }
}
#[test]
fn revoked_capability_prevents_provider_effects() {
    let state = Rc::new(State::default());
    let (mut owner, node) = ready(state.clone());
    let (bytes, length) = request();
    owner.revoke(node, HostCallId(0)).unwrap();
    assert!(matches!(
        run(owner.invoke(node, HostCallId(0), RequestId(0), &bytes[..length])),
        Err(JoinCallRefusal::Capability(_))
    ));
    assert_eq!(state.effects.get(), 0);
    assert!(!owner.cleanup_required());
}

#[test]
fn retained_native_identity_refuses_consistent_but_foreign_planned_metadata() {
    for substitution in 1..=4 {
        let state = Rc::new(State::default());
        assert!(matches!(
            prepare_owner(state.clone(), substitution),
            Err(JoinCallRefusal::WrongBinding)
        ));
        assert_eq!(state.effects.get(), 0);
    }
}

#[test]
fn lowering_refusal_keeps_the_initialized_owner_before_transfer() {
    let state = Rc::new(State::default());
    assert!(matches!(
        prepare_owner(state.clone(), 5),
        Err(JoinCallRefusal::Binding(_))
    ));
    assert_eq!(state.effects.get(), 0);
}

#[test]
fn native_storage_pressure_remains_distinct_and_preserves_cleanup_without_retry() {
    let state = Rc::new(State::default());
    state.failure.set(Some(StationFailure::Pressure));
    state.cleanup_pressure.set(true);
    let (mut owner, node) = ready(state.clone());
    let (bytes, length) = request();
    let ((), allocations) = allocation_probe::observe(|| {
        assert!(matches!(
            run(owner.invoke(node, HostCallId(0), RequestId(0), &bytes[..length])),
            Err(JoinCallRefusal::Provider(StationAttemptFailure {
                phase: StationPhase::Associate,
                reason: StationFailure::Pressure
            }))
        ));
        assert!(owner.cleanup_required());
        assert_eq!(run(owner.cleanup()), Err(StationFailure::Pressure));
        assert!(owner.cleanup_required());
        state.cleanup_pressure.set(false);
        run(owner.cleanup()).unwrap();
        assert!(!owner.cleanup_required());
        assert!(matches!(
            run(owner.invoke(node, HostCallId(0), RequestId(0), &bytes[..length])),
            Err(JoinCallRefusal::StaleRequest)
        ));
    });
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    assert_eq!(state.effects.get(), 1);
    assert_eq!(state.cleanups.get(), 2);
    assert_eq!(owner.inspection().completed_operations, 1);
}
