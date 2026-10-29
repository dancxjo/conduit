//! CPU providers only; no physical radio, initialized advertisement or Body.
extern crate alloc;
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_net::*;
use conduit_network_join_call::installation::*;
use conduit_network_join_call::scan::*;
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
    cleanups: Cell<u32>,
    now: Cell<u64>,
    pending: Cell<bool>,
    cleanup_failed: Cell<bool>,
    late: Cell<bool>,
}
struct Provider(Rc<State>);
impl NetworkScanProvider for Provider {
    fn now_ms(&self) -> u64 {
        self.0.now.get()
    }
    async fn scan(
        &mut self,
        r: NetworkScanRequest,
        out: &mut [ScanObservation; 16],
    ) -> Result<ScanCompletion, ScanFailure> {
        self.0.effects.set(self.0.effects.get() + 1);
        if self.0.pending.get() {
            core::future::pending::<()>().await;
        }
        if self.0.late.get() {
            self.0.now.set(r.scan_deadline_ms);
        }
        out[0] = ScanObservation::new(
            b"actual-observed",
            ObservedNetworkSecurity::Unknown,
            -52,
            self.0.now.get(),
        )
        .unwrap();
        Ok(ScanCompletion {
            generation: r.generation,
            completed_ms: self.0.now.get(),
            count: 1,
            total: 17,
        })
    }
    async fn cleanup(&mut self, _: u64) -> Result<(), ScanFailure> {
        self.0.cleanups.set(self.0.cleanups.get() + 1);
        if self.0.cleanup_failed.get() {
            Err(ScanFailure::Unavailable)
        } else {
            Ok(())
        }
    }
}
fn request() -> NetworkScanRequest {
    NetworkScanRequest {
        generation: 9,
        scan_deadline_ms: 100,
        cleanup_deadline_ms: 200,
    }
}
fn ready(state: Rc<State>) -> (NetworkScanHostCall<Provider>, NodeId) {
    prepare_owner(state, 0).unwrap()
}
fn prepare_owner(
    state: Rc<State>,
    substitution: u8,
) -> Result<(NetworkScanHostCall<Provider>, NodeId), ScanCallRefusal> {
    prepare_owner_mode(state, substitution, false)
}
fn prepare_owner_mode(
    state: Rc<State>,
    substitution: u8,
    assigned: bool,
) -> Result<(NetworkScanHostCall<Provider>, NodeId), ScanCallRefusal> {
    let contract = PreparedScanContract::prepare();
    let mut scope = possession::scope(NETWORK_SCAN_SUBJECT);
    scope.authority_contract_id = NETWORK_SCAN_AUTHORITY.into();
    scope.operation_contract_id = NETWORK_SCAN_HOST_CALL.into();
    scope.maximum_parameter_bytes = SCAN_OPERATION_PARAMETER_BYTES;
    scope.maximum_result_bytes = MAXIMUM_SCAN_OUTPUT_BYTES;
    scope.capability_id = SCAN_CAPABILITY.into();
    let identity = NetworkNativeIdentity {
        host: scope.host_id.clone(),
        boot: scope.boot_id.clone(),
        base: "base/scan".into(),
        instance: scope.base_instance_id.clone(),
        provider_generation: scope.base_provider_generation,
        resource: scope.resource_pool_id.clone(),
        resource_generation: scope.resource_generation_id.clone(),
        envelope: scope.envelope_id.clone(),
        artifact: "artifact/scan/fixture".into(),
        implementation: scope.implementation_id.clone(),
        base_implementation: "base/scan/fixture@1".into(),
    };
    // SAFETY: CPU-only inert provider; this establishes no physical availability.
    let ready = match unsafe { ReadyNetworkScanBase::new(identity, request(), Provider(state)) } {
        Ok(r) => r,
        Err(_) => panic!("valid fixture"),
    };
    if substitution == 2 {
        scope.resource_generation_id = ResourceGenerationId("generation/other".into());
    }

    let (mut fragment, _, _, placement) = fixture::selected(
        &scope,
        contract.kind(),
        NETWORK_SCAN_HOST_CALL,
        MAXIMUM_SCAN_INPUT_BYTES,
        "radio",
        "scan",
    );
    fragment.placements[0].host_calls[0].maximum_output_bytes = MAXIMUM_SCAN_OUTPUT_BYTES;
    fragment.placements[0].host_calls[0].target_kind = Some(NETWORK_SCAN_SUBJECT.into());
    fragment.placements[0].resources[0].class_id = WIFI_STATION_RESOURCE_CLASS.into();
    fragment.placements[0].execution_profile_id = NETWORK_SCAN_EXECUTION_PROFILE.into();
    fragment.placements[0]
        .base
        .as_mut()
        .unwrap()
        .mechanism_family = SCAN_BASE_KIND.into();
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
    if substitution == 6 {
        scope.subject_kind = "authority/foreign".into();
    }
    let (table, handle, claim) = possession::possession(scope.clone());
    let node = lowered.nodes[0].node;
    let mut retained = Some(ready);
    let result = if assigned {
        use conduit_plan_lowering::selected_assigned_operation::{
            AssignedOperationInput, SelectedAssignedOperationPlan,
        };
        let image = conduit_embedded_build::generate_embedded_plan(
            &fragment,
            &lowered,
            conduit_embedded_build::EmbeddedImageBounds::HOST_TOOLING,
        )
        .unwrap();
        let maxima = AssignedPlanMaxima {
            encoded_bytes: 65535,
            runtime_state_bytes: 32768,
            counts: [32; ASSIGNED_PLAN_COUNT_KINDS],
        };
        let bytes = conduit_embedded_build::encode_assigned_native_plan(&image, maxima).unwrap();
        let calls = lowered
            .host_calls
            .iter()
            .map(|c| AssignedIdentity::from_text(c.contract_id.as_str()))
            .collect::<Vec<_>>();
        let resources = lowered
            .resources
            .iter()
            .map(|r| r.binding.resource.0)
            .collect::<Vec<_>>();
        let computations =
            conduit_embedded_build::assigned_computation_requirements(&image).unwrap();
        ReadyNetworkScanBase::bind_assigned_retained(
            &mut retained,
            table,
            handle,
            claim,
            &contract,
            SelectedAssignedOperationPlan {
                input: AssignedOperationInput::Contiguous(&bytes),
                maxima,
                requirements: AssignedPlanRequirements {
                    host: AssignedIdentity::from_text(fragment.host_id.as_str()),
                    boot: AssignedIdentity::from_text(fragment.boot_id.as_str()),
                    host_calls: &calls,
                    resources: &resources,
                    remote_bindings: &[],
                },
                computations: AssignedComputationProfile {
                    maximum_text_bytes: u16::MAX,
                    maximum_total_text_bytes: u32::MAX,
                    requirements: &computations,
                },
                fragment_id: &fragment.fragment_id,
                active: &active,
                node,
            },
        )
    } else {
        ReadyNetworkScanBase::bind_selected_retained(
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
        )
    };
    match result {
        Ok(owner) => {
            assert!(retained.is_none());
            let (table, handle, claim) = possession::possession(scope);
            assert!(matches!(
                ReadyNetworkScanBase::bind_selected_retained(
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
                Err(ScanCallRefusal::Unavailable)
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
fn selected_scan_preserves_unknown_truncation_and_requires_actual_cleanup() {
    let state = Rc::new(State::default());
    let (mut owner, node) = ready(state.clone());
    assert!(PreparedScanContract::prepare().kind().inputs.is_empty());
    assert!(run(owner.invoke(
        node,
        HostCallId(0),
        RequestId(0),
        b"not-an-authored-request"
    ))
    .is_err());
    assert_eq!(state.effects.get(), 0);
    let bytes = run(owner.invoke(node, HostCallId(0), RequestId(0), &[])).unwrap();
    let original_bytes = bytes.to_vec();
    let mut records = [ScanObservation::EMPTY; 16];
    let completion = ScanCompletion::decode(bytes, &mut records).unwrap();
    assert!(completion.truncated());
    assert_eq!(completion.total, 17);
    assert_eq!(records[0].security, ObservedNetworkSecurity::Unknown);
    assert_eq!(
        owner.completed_observations().unwrap()[0].ssid(),
        b"actual-observed"
    );
    assert_eq!(state.effects.get(), 1);
    assert!(run(owner.invoke(node, HostCallId(0), RequestId(0), &[])).is_err());
    owner.revoke(node, HostCallId(0)).unwrap();
    let mut slot = Some(owner);
    assert!(NetworkScanHostCall::retire_retained(&mut slot).is_err());
    assert!(slot.is_some());
    state.cleanup_failed.set(true);
    assert!(run(slot.as_mut().unwrap().cleanup()).is_err());
    assert!(slot.as_ref().unwrap().cleanup_required());
    state.cleanup_failed.set(false);
    run(slot.as_mut().unwrap().cleanup()).unwrap();
    let (provider, evidence) =
        NetworkScanHostCall::retire_with_evidence_retained(&mut slot).unwrap();
    assert!(Rc::ptr_eq(&provider.0, &state));
    assert_eq!(evidence.completed_output().unwrap(), original_bytes);
    assert_eq!(evidence.request(), request());
    assert_eq!(
        evidence.inspection().lifecycle,
        CapabilityLifecycle::Revoked
    );
    assert_eq!(evidence.inspection().completed_operations, 1);
    assert_eq!(evidence.inspection().in_flight_operations, 0);
    let retained = evidence.completed_observations().unwrap();
    assert_eq!(retained[0].ssid(), b"actual-observed");
    assert_eq!(retained[0].security, ObservedNetworkSecurity::Unknown);
    assert_eq!(state.effects.get(), 1);
    assert!(slot.is_none());
}
#[test]
fn cancellation_and_deadline_never_return_a_still_owned_provider() {
    let state = Rc::new(State::default());
    state.pending.set(true);
    let (mut owner, node) = ready(state.clone());
    let bytes = [];
    {
        let mut future = core::pin::pin!(owner.invoke(node, HostCallId(0), RequestId(0), &bytes));
        assert!(future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending());
    }
    assert!(owner.cleanup_required());
    assert_eq!(owner.inspection().lifecycle, CapabilityLifecycle::Revoked);
    assert!(owner.completed_output().is_none());
    assert!(owner.completed_observations().is_none());
    state.now.set(200);
    assert_eq!(run(owner.cleanup()), Err(ScanCallRefusal::Deadline));
    let mut slot = Some(owner);
    assert!(NetworkScanHostCall::retire_retained(&mut slot).is_err());
    assert!(slot.is_some());
    assert_eq!(state.cleanups.get(), 0);
}
#[test]
fn late_completion_and_foreign_lowering_do_not_publish_or_transfer() {
    for substitution in [1, 2, 3, 4, 5, 6] {
        assert!(prepare_owner(Rc::new(State::default()), substitution).is_err());
    }
    let state = Rc::new(State::default());
    state.late.set(true);
    let (mut owner, node) = ready(state);
    assert!(run(owner.invoke(node, HostCallId(0), RequestId(0), &[])).is_err());
    assert!(owner.completed_output().is_none());
    assert!(owner.completed_observations().is_none());
    assert!(owner.cleanup_required());
}

#[test]
fn selected_dispatch_cleanup_and_wire_validation_need_no_heap() {
    let state = Rc::new(State::default());
    let (mut owner, node) = ready(state);
    let request = [];
    let ((), usage) = allocation_probe::observe(|| {
        let bytes = run(owner.invoke(node, HostCallId(0), RequestId(0), &request)).unwrap();
        let mut output = [ScanObservation::EMPTY; 16];
        let completion = ScanCompletion::decode(bytes, &mut output).unwrap();
        assert_eq!(completion.count, 1);
        run(owner.cleanup()).unwrap();
        owner.revoke(node, HostCallId(0)).unwrap();
    });
    assert_eq!(usage.allocations, 0);
    println!(
        "scan owner bytes={} observation={} output={}",
        core::mem::size_of_val(&owner),
        core::mem::size_of::<ScanObservation>(),
        MAXIMUM_SCAN_OUTPUT_BYTES
    );
}
#[test]
fn malformed_result_never_partially_replaces_caller_observations() {
    let original =
        ScanObservation::new(b"retained", ObservedNetworkSecurity::Unsupported, -80, 5).unwrap();
    let valid = ScanObservation::new(b"actual", ObservedNetworkSecurity::Unknown, -42, 9).unwrap();
    let completion = ScanCompletion {
        generation: 1,
        completed_ms: 10,
        count: 1,
        total: 1,
    };
    let mut bytes = [0; MAXIMUM_SCAN_OUTPUT_BYTES as usize];
    let length = completion.encode(&[valid], &mut bytes).unwrap();
    let mut output = [original; 16];
    bytes[24 + 35] = 255;
    assert_eq!(
        ScanCompletion::decode(&bytes[..length], &mut output),
        Err(ScanFailure::Malformed)
    );
    assert_eq!(output, [original; 16]);
    assert!(NetworkScanRequest {
        generation: 1,
        scan_deadline_ms: 10,
        cleanup_deadline_ms: 10
    }
    .encode_context()
    .is_err());
}

#[test]
fn canonical_compact_assignment_preserves_selected_scope_and_owner_refusals() {
    let state = Rc::new(State::default());
    let (mut owner, node) = prepare_owner_mode(state.clone(), 0, true).unwrap();
    run(owner.invoke(node, HostCallId(0), RequestId(0), &[])).unwrap();
    assert_eq!(state.effects.get(), 1);
    run(owner.cleanup()).unwrap();
    owner.revoke(node, HostCallId(0)).unwrap();
    let mut slot = Some(owner);
    assert!(NetworkScanHostCall::retire_retained(&mut slot).is_ok());
    // These are consistent sealed assignments with foreign native pins, not only
    // corrupt bytes. The retained provider must survive every pre-transfer refusal.
    for substitution in [1, 2, 3, 4, 6] {
        assert!(prepare_owner_mode(Rc::new(State::default()), substitution, true).is_err());
    }
}
