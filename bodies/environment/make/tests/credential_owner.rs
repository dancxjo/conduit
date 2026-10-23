//! Actual private mailbox under selected possession; Root presence/authority are
//! CPU fixtures. No physical initialized credential Base or Body start is claimed.
extern crate alloc;
use conduit_core as c;
use conduit_embedded_build as e;
use conduit_environment_http::network_setup::*;
use conduit_kernel::{HostCallId, RequestId};
use conduit_network_join_call::credential_installation::*;
use conduit_network_join_call::*;
use conduit_plan_lowering::selected_assigned_operation::*;
use conduit_plan_lowering::{lowering::lower_plan_fragment, selected_operation::*};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../../../../targets/conduitos/src/machine_membrane/selection_fixture.rs"]
mod fixture;
#[path = "../../../../architecture/plan-lowering/tests/common/native_possession.rs"]
mod possession;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
struct Source<'a> {
    mailbox: NetworkSetupMailbox,
    calls: &'a core::cell::Cell<u32>,
}
impl CredentialSource for Source<'_> {
    fn take_acknowledged(&mut self, output: &mut [u8]) -> Result<usize, CredentialSourceRefusal> {
        self.calls.set(self.calls.get() + 1);
        self.mailbox
            .take_acknowledged(output)
            .map_err(|error| match error {
                NetworkSetupRefusal::NotAcknowledged => CredentialSourceRefusal::Unacknowledged,
                NetworkSetupRefusal::NoSubmission => CredentialSourceRefusal::Unavailable,
                _ => CredentialSourceRefusal::Invalid,
            })
    }
}
#[test]
fn actual_private_mailbox_requires_current_selected_possession_and_response_ack() {
    for assigned in [false, true] {
        for (acknowledged, revoked, substituted) in [
            (true, false, false),
            (false, false, false),
            (true, true, false),
            (true, false, true),
        ] {
            check(acknowledged, revoked, assigned, substituted);
        }
    }
}
fn check(acknowledged: bool, revoked: bool, assigned: bool, substituted: bool) {
    let mut scope = possession::scope(conduit_net::NETWORK_CREDENTIALS_SUBJECT);
    scope.authority_contract_id = conduit_net::NETWORK_CREDENTIALS_AUTHORITY.into();
    scope.operation_contract_id = conduit_net::NETWORK_CREDENTIALS_HOST_CALL.into();
    scope.capability_id = CREDENTIAL_CAPABILITY.into();
    scope.maximum_parameter_bytes = 1;
    scope.maximum_result_bytes = conduit_net::MAXIMUM_JOIN_INPUT_BYTES;
    let kind = conduit_net::network_credentials_contract();
    let (mut fragment, _, _, placement) = fixture::selected(
        &scope,
        &kind,
        conduit_net::NETWORK_CREDENTIALS_HOST_CALL,
        conduit_net::MAXIMUM_JOIN_INPUT_BYTES,
        "credentials",
        "credentials",
    );
    let gear = &mut fragment.placements[0];
    gear.host_calls[0].maximum_input_bytes = 1;
    gear.host_calls[0].target_kind = Some(conduit_net::NETWORK_CREDENTIALS_SUBJECT.into());
    gear.resources[0].class_id = CREDENTIAL_RESOURCE_CLASS.into();
    gear.execution_profile_id = "conduit.network/credentials-hosted@1".into();
    gear.base.as_mut().unwrap().mechanism_family = CREDENTIAL_BASE_KIND.into();
    if substituted {
        gear.artifact_id = "artifact/substituted".into();
    }
    let identity = c::PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let fragment = c::seal_plan(identity, vec![fragment]).fragments.remove(0);
    let lowered = lower_plan_fragment(&fragment).unwrap();
    let active = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
    scope.plan_id = fragment.plan_id.clone();
    scope.active_play_id = active.active_play_id.clone();
    let native_identity = conduit_network_join_call::installation::NetworkNativeIdentity {
        host: scope.host_id.clone(),
        boot: scope.boot_id.clone(),
        base: "base/credentials".into(),
        instance: scope.base_instance_id.clone(),
        provider_generation: scope.base_provider_generation,
        resource: scope.resource_pool_id.clone(),
        resource_generation: scope.resource_generation_id.clone(),
        envelope: scope.envelope_id.clone(),
        artifact: "artifact/credentials/fixture".into(),
        implementation: scope.implementation_id.clone(),
        base_implementation: "base/credentials/fixture@1".into(),
    };
    let (table, handle, claim) = possession::possession(scope.clone());
    let mut mailbox = NetworkSetupMailbox::new(1);
    let input = NetworkSetupInput::from_form(
        b"ssid=fixture-network&password=fixture-password",
        NetworkSetupLimits {
            minimum_credential_bytes: 8,
            maximum_credential_bytes: 64,
        },
    )
    .unwrap();
    let submission = mailbox.submit(input).unwrap();
    if acknowledged {
        mailbox.acknowledge(submission).unwrap();
    }
    let calls = core::cell::Cell::new(0);
    let contract = PreparedCredentialContract::prepare();
    // SAFETY: Root/native identity are CPU fixtures, never physical availability.
    let ready = unsafe {
        ReadyCredentialBase::new(
            native_identity,
            Source {
                mailbox,
                calls: &calls,
            },
        )
    }
    .unwrap();
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
    let host_calls = [c::AssignedIdentity::from_text(
        conduit_net::NETWORK_CREDENTIALS_HOST_CALL,
    )];
    let resources: Vec<_> = image.resources.iter().map(|r| r.resource).collect();

    let selected = || SelectedAssignedOperationPlan {
        bytes: &bytes,
        maxima,
        requirements: c::AssignedPlanRequirements {
            host: c::AssignedIdentity::from_text(fragment.host_id.as_str()),
            boot: c::AssignedIdentity::from_text(fragment.boot_id.as_str()),
            host_calls: &host_calls,
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
    let mut retained = Some(ready);
    let (result, binding_memory) = allocation_probe::observe(|| {
        if assigned {
            retained
                .take()
                .unwrap()
                .bind_assigned(table, handle, claim, &contract, selected())
        } else {
            ReadyCredentialBase::bind_selected_retained(
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
        }
    });
    if assigned {
        assert_eq!(
            (binding_memory.allocations, binding_memory.reallocations),
            (0, 0)
        );
    }
    if substituted {
        if !assigned {
            assert!(
                retained.is_some(),
                "refusal must retain acknowledged private input"
            );
        }
        assert!(matches!(
            result,
            Err(CredentialCallRefusal::WrongRequest
                | CredentialCallRefusal::Binding(SelectedCallRefusal::WrongBinding))
        ));
        assert_eq!(calls.get(), 0);
        return;
    }
    let mut owner = result.unwrap();
    assert!(retained.is_none());
    if !assigned {
        let (table, handle, claim) = possession::possession(scope);
        assert!(matches!(
            ReadyCredentialBase::bind_selected_retained(
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
            Err(CredentialCallRefusal::OwnerUnavailable)
        ));
    }
    let node = lowered.nodes[0].node;
    let mut output = [0; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize];
    let ((), allocation) = allocation_probe::observe(|| {
        assert_eq!(
            owner.invoke(node, HostCallId(0), RequestId(1), &[], &mut output),
            Err(CredentialCallRefusal::WrongRequest)
        );
        assert_eq!(
            owner.invoke(node, HostCallId(0), RequestId(0), &[], &mut output[..1]),
            Err(CredentialCallRefusal::OutputCapacity)
        );
        assert_eq!(calls.get(), 0);
        if revoked {
            owner.revoke().unwrap();
        }
        let result = owner.invoke(node, HostCallId(0), RequestId(0), &[], &mut output);
        if revoked {
            assert!(matches!(result, Err(CredentialCallRefusal::Capability(_))));
            assert_eq!(calls.get(), 0);
        } else if acknowledged {
            let request =
                conduit_net::decode_network_join_request(&output[..result.unwrap()]).unwrap();
            assert_eq!(request.ssid, b"fixture-network");
            assert_eq!(request.credential, b"fixture-password");
        } else {
            assert_eq!(
                result,
                Err(CredentialCallRefusal::Source(
                    CredentialSourceRefusal::Unacknowledged
                ))
            );
        }
        if !revoked {
            assert_eq!(calls.get(), 1);
            assert_eq!(
                owner.invoke(node, HostCallId(0), RequestId(0), &[], &mut output),
                Err(CredentialCallRefusal::WrongRequest)
            );
        }
        output.fill(0);
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
}

#[path = "../../../../targets/esp32/proof/tm1637/src/native_command.rs"]
mod native_command;
