//! CPU fixture: the actual network Kind through ordinary generation and admission.
//! This issues no hardware offer and performs no network operation.
extern crate alloc;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_embedded_build as e;
use conduit_plan_lowering::{
    lowering::lower_plan_fragment, selected_assigned_operation::*, selected_operation::*,
};
#[path = "../../../../targets/conduitos/src/machine_membrane/selection_fixture.rs"]
mod fixture;
#[path = "../../../../architecture/plan-lowering/tests/common/native_possession.rs"]
mod possession;

#[test]
fn network_assignment_preserves_its_independent_subject_and_native_possession() {
    network_assignment_control(false);
}
#[test]
fn borrowed_network_assignment_preserves_subject_and_native_possession() {
    network_assignment_control(true);
}
fn network_assignment_control(borrowed: bool) {
    let kind = conduit_net::network_join_contract();
    let subject = c::KindId::from(conduit_net::NETWORK_CONFIG_SUBJECT);
    let mut scope = possession::scope(subject.as_str());
    scope.authority_contract_id = conduit_net::NETWORK_CONFIG_AUTHORITY.into();
    scope.operation_contract_id = conduit_net::NETWORK_JOIN_HOST_CALL.into();
    scope.maximum_parameter_bytes = conduit_net::MAXIMUM_JOIN_INPUT_BYTES;
    scope.maximum_result_bytes = conduit_net::MAXIMUM_JOIN_OUTPUT_BYTES;
    scope.capability_id = conduit_network_join_call::installation::JOIN_CAPABILITY.into();
    let effects = core::cell::Cell::new(0);
    // SAFETY: CPU fixture only; no hardware or initialized radio is claimed.
    let ready_base = unsafe {
        conduit_network_join_call::installation::ReadyNetworkJoinBase::new(
            conduit_network_join_call::installation::NetworkNativeIdentity {
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
            },
            9,
            Provider(&effects),
        )
    }
    .unwrap();
    let (mut fragment, _, _, placement) = fixture::selected(
        &scope,
        &kind,
        conduit_net::NETWORK_JOIN_HOST_CALL,
        conduit_net::MAXIMUM_JOIN_INPUT_BYTES,
        "radio",
        "join",
    );
    let gear = &mut fragment.placements[0];
    gear.host_calls[0].target_kind = Some(subject.clone());
    gear.resources[0].class_id = conduit_net::WIFI_STATION_RESOURCE_CLASS.into();
    gear.execution_profile_id = conduit_net::NETWORK_JOIN_EXECUTION_PROFILE.into();
    gear.base.as_mut().unwrap().mechanism_family =
        conduit_network_join_call::installation::JOIN_BASE_KIND.into();
    let identity = c::PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let fragment = c::seal_plan(identity, vec![fragment]).fragments.remove(0);
    let lowered = lower_plan_fragment(&fragment).unwrap();
    let active = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 3);
    scope.plan_id = fragment.plan_id.clone();
    scope.active_play_id = active.active_play_id.clone();
    let (table, handle, claim) = possession::possession(scope);
    let image = e::generate_embedded_plan(
        &fragment,
        &lowered,
        e::EmbeddedImageBounds {
            maximum_nodes: 1,
            maximum_cords: 0,
            maximum_routes: 1,
            maximum_route_targets: 1,
            maximum_host_calls: 1,
            maximum_resources: 1,
            maximum_sign_expectations: 16,
            maximum_configuration_entries: 0,
            maximum_ports_per_node: 8,
            maximum_remote_endpoints: 0,
            maximum_cord_value_slots: 1,
            maximum_cord_value_bytes: 1024,
            maximum_sign_items: 64,
            maximum_sign_bytes: 8192,
        },
    )
    .unwrap();
    let maxima = c::AssignedPlanMaxima {
        encoded_bytes: 8192,
        runtime_state_bytes: 8192,
        counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
    };
    let bytes = e::encode_assigned_native_plan(&image, maxima).unwrap();
    let calls = [c::AssignedIdentity::from_text(
        conduit_net::NETWORK_JOIN_HOST_CALL,
    )];
    let resources: Vec<_> = image.resources.iter().map(|r| r.resource).collect();
    let definition = || SelectedOperationContract {
        kind: &kind,
        call: conduit_net::NETWORK_JOIN_HOST_CALL,
        input_bytes: conduit_net::MAXIMUM_JOIN_INPUT_BYTES,
        output_bytes: conduit_net::MAXIMUM_JOIN_OUTPUT_BYTES,
        resource_bytes: 1,
    };
    let native = || AssignedNativeIdentity {
        capability: conduit_network_join_call::installation::JOIN_CAPABILITY,
        execution_profile: conduit_net::NETWORK_JOIN_EXECUTION_PROFILE,
        artifact: "artifact/join/fixture",
        base: "base/join",
        base_implementation: "base/join/fixture@1",
        mechanism: conduit_network_join_call::installation::JOIN_BASE_KIND,
        authority: conduit_net::NETWORK_CONFIG_AUTHORITY,
        resource_class: conduit_net::WIFI_STATION_RESOURCE_CLASS,
    };
    let records: Vec<_> = c::AssignedPlanRecords::new(&bytes)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let selected = || SelectedAssignedOperationPlan {
        input: if borrowed {
            AssignedOperationInput::Records {
                header: &bytes[..c::ASSIGNED_PLAN_HEADER_BYTES],
                records: &records,
            }
        } else {
            AssignedOperationInput::Contiguous(&bytes)
        },
        maxima,
        requirements: c::AssignedPlanRequirements {
            host: c::AssignedIdentity::from_text(fragment.host_id.as_str()),
            boot: c::AssignedIdentity::from_text(fragment.boot_id.as_str()),
            host_calls: &calls,
            resources: &resources,
            remote_bindings: &[],
        },
        computations: c::AssignedComputationProfile {
            maximum_text_bytes: 32768,
            maximum_total_text_bytes: 32768,
            requirements: &[],
        },
        fragment_id: &fragment.fragment_id,
        active: &active,
        node: lowered.nodes[0].node,
    };
    assert_eq!(
        bind_selected_assigned_operation(&table, &claim, definition(), native(), selected()),
        Err(SelectedCallRefusal::WrongBinding)
    );
    let (result, allocation) = allocation_probe::observe(|| {
        bind_selected_assigned_operation_for_subject(
            &table,
            &claim,
            definition(),
            &subject,
            native(),
            selected(),
        )
    });
    assert_eq!(result, Ok(lowered.nodes[0].node));
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(
        bind_selected_assigned_operation_for_subject(
            &table,
            &claim,
            definition(),
            &kind.kind_id,
            native(),
            selected()
        ),
        Err(SelectedCallRefusal::WrongBinding)
    );
    let mut noncanonical = active.clone();
    noncanonical.play_sequence += 1;
    assert_eq!(
        bind_selected_assigned_operation_for_subject(
            &table,
            &claim,
            definition(),
            &subject,
            native(),
            SelectedAssignedOperationPlan {
                active: &noncanonical,
                ..selected()
            }
        ),
        Err(SelectedCallRefusal::WrongBinding)
    );
    let mut stale = claim.clone();
    stale.boot_id = "boot/old".into();
    assert_eq!(
        bind_selected_assigned_operation_for_subject(
            &table,
            &stale,
            definition(),
            &subject,
            native(),
            selected()
        ),
        Err(SelectedCallRefusal::WrongBinding)
    );
    assert_eq!(
        bind_selected_assigned_operation_for_subject(
            &table,
            &claim,
            definition(),
            &subject,
            AssignedNativeIdentity {
                authority: "authority/other",
                ..native()
            },
            selected()
        ),
        Err(SelectedCallRefusal::WrongBinding)
    );
    assert_eq!(
        bind_selected_operation_for_subject(
            &table,
            &claim,
            definition(),
            &subject,
            SelectedOperationPlan {
                fragment: &fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placement
            }
        ),
        Ok(lowered.nodes[0].node)
    );
    let contract = conduit_network_join_call::PreparedJoinContract::prepare();
    let (owner, allocation) = allocation_probe::observe(|| {
        ready_base.bind_assigned(table, handle, claim, &contract, selected())
    });
    let mut owner = owner.unwrap();
    drop(records);
    drop(bytes);
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(effects.get(), 0);
    let mut request = [0; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize];
    let length = conduit_net::encode_network_join_request(
        conduit_net::NetworkJoinRequest {
            ssid: b"fixture-net",
            credential: b"fixture-password",
        },
        &mut request,
    )
    .unwrap();
    let (generation, allocation) = allocation_probe::observe(|| {
        let output = ready(owner.invoke(
            lowered.nodes[0].node,
            conduit_kernel::HostCallId(0),
            conduit_kernel::RequestId(0),
            &request[..length],
        ))
        .unwrap();
        conduit_net::decode_network_attachment(output)
            .unwrap()
            .generation
    });
    assert_eq!(generation, 9);
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(effects.get(), 1);
    assert!(owner.cleanup_required());
}

/// A CPU provider establishes no physical attachment truth.
struct Provider<'a>(&'a core::cell::Cell<usize>);
impl conduit_network_join_call::NetworkJoinProvider for Provider<'_> {
    async fn join(
        &mut self,
        _: conduit_net::NetworkJoinRequest<'_>,
    ) -> Result<conduit_network_join_call::AttachmentRecord, conduit_net::StationAttemptFailure>
    {
        self.0.set(self.0.get() + 1);
        conduit_network_join_call::AttachmentRecord::encode(conduit_net::NetworkAttachmentInfo {
            attachment_id: "attachment/fixture",
            host_id: "host/network",
            boot_id: "boot/current",
            resource_pool_id: "pool/radio",
            generation: 9,
        })
        .map_err(|_| conduit_net::StationAttemptFailure {
            phase: conduit_net::StationPhase::ConfigureAddress,
            reason: conduit_net::StationFailure::ProviderUnavailable,
        })
    }
    async fn cleanup(&mut self) -> Result<(), conduit_net::StationFailure> {
        Ok(())
    }
}
fn ready<T>(future: impl core::future::Future<Output = T>) -> T {
    let mut future = core::pin::pin!(future);
    match future.as_mut().poll(&mut core::task::Context::from_waker(
        core::task::Waker::noop(),
    )) {
        core::task::Poll::Ready(value) => value,
        core::task::Poll::Pending => panic!("fixture provider unexpectedly pending"),
    }
}
