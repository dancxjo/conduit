//! Actual private mailbox under selected possession; Root presence/authority are
//! CPU fixtures. No physical initialized credential Base or Body start is claimed.
extern crate alloc;
use conduit_core as c;
use conduit_environment_http::network_setup::*;
use conduit_kernel::{HostCallId, RequestId};
use conduit_network_join_call::*;
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
    for (acknowledged, revoked) in [(true, false), (false, false), (true, true)] {
        let mut scope = possession::scope(conduit_net::NETWORK_CREDENTIALS_SUBJECT);
        scope.authority_contract_id = conduit_net::NETWORK_CREDENTIALS_AUTHORITY.into();
        scope.operation_contract_id = conduit_net::NETWORK_CREDENTIALS_HOST_CALL.into();
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
        let identity = c::PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        };
        let fragment = c::seal_plan(identity, vec![fragment]).fragments.remove(0);
        let lowered = lower_plan_fragment(&fragment).unwrap();
        let active =
            c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
        scope.plan_id = fragment.plan_id.clone();
        scope.active_play_id = active.active_play_id.clone();
        let (table, handle, claim) = possession::possession(scope);
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
        // SAFETY: Root and private Base identity are CPU fixtures, never physical availability.
        let mut owner = unsafe {
            NetworkCredentialsHostCall::bind_selected(
                table,
                handle,
                claim,
                Source {
                    mailbox,
                    calls: &calls,
                },
                &contract,
                SelectedOperationPlan {
                    fragment: &fragment,
                    lowered: &lowered,
                    active: &active,
                    placement_id: &placement,
                },
            )
        }
        .unwrap();
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
}
