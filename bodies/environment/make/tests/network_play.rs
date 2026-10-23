//! Real checked Source, planner, lowering, mailbox and kernel. Only radio and
//! Root presence/authority are CPU fixtures; no hardware or Body start is claimed.
extern crate alloc;
use conduit_core as c;
use conduit_environment_http::network_setup::*;
use conduit_environment_make::PreparedNetworkSetup;
use conduit_kernel::{
    self as k,
    scheduler::{FixedScheduler, SchedulerStatus},
    ValueStorage,
};
use conduit_net as net;
use conduit_network_join_call::{installation::*, *};
use conduit_plan_lowering::selected_operation::SelectedOperationPlan;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "network_play_support/preparation_profile.rs"]
mod preparation_profile;
#[path = "network_play_support/projection.rs"]
mod projection;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "../../../../architecture/plan-lowering/tests/common/native_possession.rs"]
mod possession;
struct PrivateSource<'a> {
    mailbox: NetworkSetupMailbox,
    calls: &'a core::cell::Cell<u32>,
}
impl CredentialSource for PrivateSource<'_> {
    fn take_acknowledged(&mut self, output: &mut [u8]) -> Result<usize, CredentialSourceRefusal> {
        self.calls.set(self.calls.get() + 1);
        self.mailbox
            .take_acknowledged(output)
            .map_err(|error| match error {
                NetworkSetupRefusal::NotAcknowledged => CredentialSourceRefusal::Unacknowledged,
                _ => CredentialSourceRefusal::Invalid,
            })
    }
}
struct Provider<'a>(&'a core::cell::Cell<u32>);
impl NetworkJoinProvider for Provider<'_> {
    async fn join(
        &mut self,
        request: net::NetworkJoinRequest<'_>,
    ) -> Result<AttachmentRecord, net::StationAttemptFailure> {
        assert_eq!(request.ssid, b"fixture-net");
        assert_eq!(request.credential, b"fixture-password");
        self.0.set(self.0.get() + 1);
        Ok(AttachmentRecord::encode(net::NetworkAttachmentInfo {
            attachment_id: "attachment/fixture",
            host_id: "host/network",
            boot_id: "boot/current",
            resource_pool_id: "pool/radio",
            generation: 9,
        })
        .unwrap())
    }
    async fn cleanup(&mut self) -> Result<(), net::StationFailure> {
        Ok(())
    }
}
fn run<T>(future: impl core::future::Future<Output = T>) -> T {
    let mut future = core::pin::pin!(future);
    match future.as_mut().poll(&mut core::task::Context::from_waker(
        core::task::Waker::noop(),
    )) {
        core::task::Poll::Ready(value) => value,
        core::task::Poll::Pending => panic!("fixture unexpectedly pending"),
    }
}
#[test]
fn checked_network_plot_consumes_only_acknowledged_private_input_through_real_kernel() {
    for assigned in [false, true] {
        for acknowledged in [true, false] {
            execute(acknowledged, assigned);
        }
    }
}
fn execute(acknowledged: bool, assigned: bool) {
    let mut scope = possession::scope(net::NETWORK_CONFIG_SUBJECT);
    scope.capability_id = JOIN_CAPABILITY.into();
    scope.authority_contract_id = net::NETWORK_CONFIG_AUTHORITY.into();
    scope.operation_contract_id = net::NETWORK_JOIN_HOST_CALL.into();
    scope.maximum_parameter_bytes = net::MAXIMUM_JOIN_INPUT_BYTES;
    scope.maximum_result_bytes = net::MAXIMUM_JOIN_OUTPUT_BYTES;
    let effects = core::cell::Cell::new(0);
    let identity = NetworkNativeIdentity {
        host: scope.host_id.clone(),
        boot: scope.boot_id.clone(),
        base: "base/join".into(),
        instance: scope.base_instance_id.clone(),
        provider_generation: 1,
        resource: scope.resource_pool_id.clone(),
        resource_generation: scope.resource_generation_id.clone(),
        envelope: scope.envelope_id.clone(),
        artifact: "artifact/join/fixture".into(),
        implementation: scope.implementation_id.clone(),
        base_implementation: "base/join/fixture@1".into(),
    };
    // SAFETY: CPU fixture only, never a physical initialized provider.
    let ready = unsafe { ReadyNetworkJoinBase::new(identity, 9, Provider(&effects)) }.unwrap();
    let mut host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/network".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    ready.append_to_advertisement(&mut host).unwrap();
    let mut mailbox = NetworkSetupMailbox::new(1);
    let input = NetworkSetupInput::from_form(
        b"ssid=fixture-net&password=fixture-password",
        NetworkSetupLimits {
            minimum_credential_bytes: 8,
            maximum_credential_bytes: 63,
        },
    )
    .unwrap();
    let submission = mailbox.submit(input).unwrap();
    if acknowledged {
        mailbox.acknowledge(submission).unwrap();
    }
    let credential_calls = core::cell::Cell::new(0);
    let mut credential_scope = possession::scope(net::NETWORK_CREDENTIALS_SUBJECT);
    credential_scope.base_instance_id = "base/credentials".into();
    credential_scope.resource_pool_id = "pool/credentials".into();
    credential_scope.capability_id = credential_installation::CREDENTIAL_CAPABILITY.into();
    credential_scope.implementation_id = "impl/credentials".into();
    credential_scope.authority_contract_id = net::NETWORK_CREDENTIALS_AUTHORITY.into();
    credential_scope.authority_grant_id = "grant/credentials".into();
    credential_scope.operation_contract_id = net::NETWORK_CREDENTIALS_HOST_CALL.into();
    credential_scope.maximum_parameter_bytes = 1;
    credential_scope.maximum_result_bytes = net::MAXIMUM_JOIN_INPUT_BYTES;
    // SAFETY: actual mailbox retained; Root identity/authority remain CPU fixtures.
    let private = unsafe {
        credential_installation::ReadyCredentialBase::new(
            NetworkNativeIdentity {
                host: credential_scope.host_id.clone(),
                boot: credential_scope.boot_id.clone(),
                base: "base/credentials".into(),
                instance: credential_scope.base_instance_id.clone(),
                provider_generation: 1,
                resource: credential_scope.resource_pool_id.clone(),
                resource_generation: credential_scope.resource_generation_id.clone(),
                envelope: credential_scope.envelope_id.clone(),
                artifact: "artifact/credentials".into(),
                implementation: credential_scope.implementation_id.clone(),
                base_implementation: "base/credentials@1".into(),
            },
            PrivateSource {
                mailbox,
                calls: &credential_calls,
            },
        )
    }
    .unwrap();
    let mut stale = host.clone();
    stale.boot_id = "boot/stale".into();
    let original = stale.clone();
    assert_eq!(
        private.append_to_advertisement(&mut stale),
        Err(NetworkInstallationRefusal::WrongBoot)
    );
    assert_eq!(stale, original);
    private.append_to_advertisement(&mut host).unwrap();
    let original = host.clone();
    assert_eq!(
        private.append_to_advertisement(&mut host),
        Err(NetworkInstallationRefusal::Advertisement)
    );
    assert_eq!(host, original);
    host.capabilities.push(net::network_attachment_sign_offer(
        "cap/sign".into(),
        "impl/sign".into(),
        "artifact/sign".into(),
    ));
    let grant =
        |id: &str, authority: &str, call: &str, subject: &str, cap: &str| c::AuthorityGrant {
            grant_id: id.into(),
            contract_id: authority.into(),
            host_call_contract_id: call.into(),
            subject_kind: subject.into(),
            host_id: scope.host_id.clone(),
            boot_id: scope.boot_id.clone(),
            capability_id: cap.into(),
        };
    let grants = [
        grant(
            "grant/configurator",
            net::NETWORK_CONFIG_AUTHORITY,
            net::NETWORK_JOIN_HOST_CALL,
            net::NETWORK_CONFIG_SUBJECT,
            JOIN_CAPABILITY,
        ),
        grant(
            "grant/credentials",
            net::NETWORK_CREDENTIALS_AUTHORITY,
            net::NETWORK_CREDENTIALS_HOST_CALL,
            net::NETWORK_CREDENTIALS_SUBJECT,
            credential_installation::CREDENTIAL_CAPABILITY,
        ),
    ];
    let bases = ["conduit.base/local@1".into()];
    assert!(PreparedNetworkSetup::prepare(&host, &bases, &grants[..1]).is_err());
    let (prepared, preparation) = allocation_probe::observe(|| {
        PreparedNetworkSetup::prepare(&host, &bases, &grants).unwrap()
    });
    assert!(c::verify_plan(prepared.plan()));
    preparation_profile::measure(&host, &bases, &grants, prepared.plan());
    assert_eq!(prepared.body_plot_plan().plan, *prepared.plan());
    let projection = projection::Projection::prepare(prepared);
    let prepared = projection.native.checked();
    let lowered = prepared.lowered();
    let fragment = &prepared.plan().fragments[0];
    const P: usize = conduit_network_join_call::CONTROL_PORTS;
    assert_eq!((lowered.nodes.len(), lowered.cords.len()), (3, 2));
    let find = |kind: &str| {
        lowered
            .nodes
            .iter()
            .find(|n| {
                fragment
                    .placements
                    .iter()
                    .any(|g| g.placement_id == n.placement_id && g.kind_id.as_str() == kind)
            })
            .unwrap()
            .node
    };
    let credential = find(net::NETWORK_CREDENTIALS_OPERATION);
    let join = find(net::NETWORK_JOIN_OPERATION);
    let sign = find(net::NETWORK_ATTACHMENT_SIGN_OPERATION);
    let active = c::bind_active_play(&fragment.plan_id, &host.host_id, &host.boot_id, 3);
    credential_scope.plan_id = fragment.plan_id.clone();
    credential_scope.active_play_id = active.active_play_id.clone();
    let (credential_table, credential_handle, credential_claim) =
        possession::possession(credential_scope);
    let credential_contract = PreparedCredentialContract::prepare();
    let mut credential_owner = if assigned {
        private.bind_assigned(
            credential_table,
            credential_handle,
            credential_claim,
            &credential_contract,
            projection.selected(&active, credential),
        )
    } else {
        private.bind_selected(
            credential_table,
            credential_handle,
            credential_claim,
            &credential_contract,
            SelectedOperationPlan {
                fragment,
                lowered,
                active: &active,
                placement_id: &lowered.nodes[usize::from(credential.0)].placement_id,
            },
        )
    }
    .unwrap();
    scope.plan_id = fragment.plan_id.clone();
    scope.active_play_id = active.active_play_id.clone();
    let (table, handle, claim) = possession::possession(scope);
    let contract = PreparedJoinContract::prepare();
    let mut owner = if assigned {
        ready.bind_assigned(
            table,
            handle,
            claim,
            &contract,
            projection.selected(&active, join),
        )
    } else {
        ready.bind_selected(
            table,
            handle,
            claim,
            &contract,
            SelectedOperationPlan {
                fragment,
                lowered,
                active: &active,
                placement_id: &lowered.nodes[usize::from(join.0)].placement_id,
            },
        )
    }
    .unwrap();
    let mut routes = k::FixedRoutes::<{ 3 * P }, 2>::new(P as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut bindings = k::FixedHostCallBindings::<3>::new(1);
    for call in &lowered.host_calls {
        bindings.install(call.node, call.binding).unwrap();
    }
    bindings.seal().unwrap();
    let mut values = k::FixedValueStore::<8, 167>::new(8 * 167).unwrap();
    let empty = values.store(&[]).unwrap();
    let drivers = core::array::from_fn(|i| {
        if i == usize::from(credential.0) {
            NetworkControlBack::credentials(empty).unwrap()
        } else if i == usize::from(join.0) {
            NetworkControlBack::join()
        } else {
            NetworkControlBack::attachment_sign()
        }
    });
    let (nodes, cords, routes, bindings) = if assigned {
        let (tables, memory) = allocation_probe::observe(|| projection.tables());
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert_eq!(
            tables.nodes,
            core::array::from_fn(|i| k::scheduler::NodeSpec {
                input_cords: [lowered.node_specs[i].input_cords[0]],
                maximum_step_fuel: lowered.node_specs[i].maximum_step_fuel
            })
        );
        assert_eq!(
            tables.cords,
            core::array::from_fn(|i| lowered.cords[i].spec)
        );
        (tables.nodes, tables.cords, tables.routes, tables.calls)
    } else {
        (
            core::array::from_fn(|i| k::scheduler::NodeSpec {
                input_cords: [lowered.node_specs[i].input_cords[0]],
                maximum_step_fuel: lowered.node_specs[i].maximum_step_fuel,
            }),
            core::array::from_fn(|i| lowered.cords[i].spec),
            routes,
            bindings,
        )
    };
    let mut kernel = if assigned {
        FixedScheduler::<_, _, _, 3, 2, P, 2, { 3 * P }, 2, 3, 3>::new_with_host_calls(
            nodes,
            cords,
            routes,
            bindings,
            drivers,
            values,
            k::HostedSignLog::new(128, (128 * core::mem::size_of::<k::KernelEvent>()) as u32)
                .unwrap(),
        )
        .unwrap()
    } else {
        let prepared = PreparedNetworkKernel::prepare(fragment, lowered).unwrap();
        assert_eq!(
            prepared.roles,
            NetworkControlRoles {
                credentials: credential,
                join,
                attachment_sign: sign
            }
        );
        prepared.kernel
    };
    let mut signs = 0;
    let mut finished = false;
    let mut buffer = [0; 167];
    let ((), allocation) = allocation_probe::observe(|| {
        for _ in 0..100 {
            if let Some(request) = kernel.next_host_request() {
                let (output, failure) = if request.node == credential {
                    assert!(kernel.host_value(request.input.value).unwrap().is_empty());
                    match credential_owner.invoke(
                        request.node,
                        request.call,
                        request.request,
                        kernel.host_value(request.input.value).unwrap(),
                        &mut buffer,
                    ) {
                        Ok(length) => (
                            Some(
                                k::BoundedValueRef::new(
                                    kernel.store_host_value(&buffer[..length]).unwrap(),
                                    167,
                                )
                                .unwrap(),
                            ),
                            None,
                        ),
                        Err(CredentialCallRefusal::Source(
                            CredentialSourceRefusal::Unacknowledged,
                        )) => (
                            None,
                            Some(k::Failure {
                                code: k::FailureCode::HostCallDenied,
                                detail: 42,
                            }),
                        ),
                        Err(_) => panic!("unexpected mailbox refusal"),
                    }
                } else if request.node == join {
                    let output = run(owner.invoke(
                        request.node,
                        request.call,
                        request.request,
                        kernel.host_value(request.input.value).unwrap(),
                    ))
                    .unwrap();
                    (
                        Some(
                            k::BoundedValueRef::new(kernel.store_host_value(output).unwrap(), 167)
                                .unwrap(),
                        ),
                        None,
                    )
                } else {
                    assert_eq!(request.node, sign);
                    let attachment = net::decode_network_attachment(
                        kernel.host_value(request.input.value).unwrap(),
                    )
                    .unwrap();
                    assert_eq!(attachment.generation, 9);
                    signs += 1;
                    (None, None)
                };
                kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        k::HostCallOutcome {
                            disposition: if failure.is_some() {
                                k::HostCallDisposition::Denied
                            } else {
                                k::HostCallDisposition::Completed
                            },
                            output,
                            failure,
                        },
                    )
                    .unwrap();
            }
            match kernel.step() {
                Ok(SchedulerStatus::Drained) => {
                    assert!(acknowledged);
                    finished = true;
                    break;
                }
                Err(k::scheduler::SchedulerError::BackFailed(failure)) => {
                    assert!(!acknowledged);
                    assert_eq!(failure.code, k::FailureCode::HostCallDenied);
                    finished = true;
                    break;
                }
                Ok(_) => {}
                Err(error) => panic!("{error:?}"),
            }
        }
    });
    assert!(finished);
    assert_eq!(
        credential_owner.invoke(
            credential,
            k::HostCallId(0),
            k::RequestId(0),
            &[],
            &mut buffer
        ),
        Err(CredentialCallRefusal::WrongRequest)
    );
    buffer.fill(0);
    assert_eq!(credential_calls.get(), 1);
    assert_eq!(effects.get(), u32::from(acknowledged));
    assert_eq!(signs, usize::from(acknowledged));
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    eprintln!(
        "network_control_host_preparation_peak={} fixed_kernel_bytes={}",
        preparation.peak_bytes,
        core::mem::size_of_val(&kernel)
    );
}
