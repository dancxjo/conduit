//! Exact network Source, actual Root issuance and retained initialized CPU owners.
use super::*;
use conduit_network_join_call::{self as net, credential_installation::*, installation::*};
#[allow(
    dead_code,
    reason = "shared planning module retains compatibility rich and split entrances"
)]
#[path = "../../src/network_planning.rs"]
mod planning;
use conduit_plan_lowering::selected_operation::{
    SelectedDirectOperationPlan, SelectedOperationPlan,
};
struct Provider {
    effects: Rc<Cell<usize>>,
}
impl net::CredentialSource for Provider {
    fn take_acknowledged(&mut self, out: &mut [u8]) -> Result<usize, net::CredentialSourceRefusal> {
        self.effects.set(self.effects.get() + 1);
        conduit_net::encode_network_join_request(
            conduit_net::NetworkJoinRequest {
                ssid: b"fixture-network",
                credential: b"fixture-only",
            },
            out,
        )
        .map_err(|_| net::CredentialSourceRefusal::OutputCapacity)
    }
}
impl net::NetworkJoinProvider for Provider {
    async fn join(
        &mut self,
        request: conduit_net::NetworkJoinRequest<'_>,
    ) -> Result<net::AttachmentRecord, conduit_net::StationAttemptFailure> {
        assert_eq!(request.ssid, b"fixture-network");
        self.effects.set(self.effects.get() + 1);
        Ok(
            net::AttachmentRecord::encode(conduit_net::NetworkAttachmentInfo {
                attachment_id: "fixture/attached",
                host_id: "fixture/host",
                boot_id: "fixture/boot",
                resource_pool_id: "fixture/resource/join",
                generation: 1,
            })
            .unwrap(),
        )
    }
    async fn cleanup(&mut self) -> Result<(), conduit_net::StationFailure> {
        Ok(())
    }
}
fn identity(name: &str) -> NetworkNativeIdentity {
    NetworkNativeIdentity {
        host: "fixture/host".into(),
        boot: "fixture/boot".into(),
        base: format!("fixture/base/{name}").into(),
        instance: format!("fixture/instance/{name}").into(),
        provider_generation: 1,
        resource: format!("fixture/resource/{name}").into(),
        resource_generation: c::ResourceGenerationId(format!("fixture/generation/{name}")),
        envelope: format!("fixture/envelope/{name}").into(),
        artifact: format!("fixture/artifact/{name}").into(),
        implementation: format!("fixture/implementation/{name}").into(),
        base_implementation: format!("fixture/base-implementation/{name}").into(),
    }
}
struct NetworkFixture {
    source: conduit_plot::ExpandedCanonicalPlot,
    bytes: Vec<u8>,
    identity: c::PlotIdentity,
    host: c::HostAdvertisement,
    bases: Vec<c::BaseImplementationId>,
    grants: Vec<c::AuthorityGrant>,
    issuer: native_authority::RootIssuer,
    credentials: Option<ReadyCredentialBase<Provider>>,
    join: Option<ReadyNetworkJoinBase<Provider>>,
    effects: Rc<Cell<usize>>,
}
fn fixture() -> NetworkFixture {
    let source = conduit_environment_make::compile_network_setup().unwrap();
    let expected = c::PlotIdentity {
        source_document_id: source.source_document_id.clone(),
        checked_plot_id: source.checked_plot_id.clone(),
        expanded_plot_id: source.expanded_plot_id.clone(),
    };
    let bytes = conduit_plot::flat_image::indexed::encode(&source).unwrap();
    let effects = Rc::new(Cell::new(0));
    // SAFETY: exclusively owned inert CPU transcript providers, no radio or credentials.
    let (credentials, join) = unsafe {
        (
            ReadyCredentialBase::new(
                identity("credentials"),
                Provider {
                    effects: effects.clone(),
                },
            )
            .unwrap(),
            ReadyNetworkJoinBase::new(
                identity("join"),
                1,
                Provider {
                    effects: effects.clone(),
                },
            )
            .unwrap(),
        )
    };
    let mut host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: identity("join").host,
        boot_id: identity("join").boot,
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/network".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    credentials.append_to_advertisement(&mut host).unwrap();
    join.append_to_advertisement(&mut host).unwrap();
    host.capabilities
        .push(conduit_net::network_attachment_sign_local_offer(
            "fixture/sign".into(),
            "fixture/sign-implementation".into(),
            "fixture/sign-artifact".into(),
        ));
    let issuer = native_authority::RootIssuer::fixture(host.host_id.clone(), host.boot_id.clone());
    let grants = vec![
        issuer
            .grant(
                credentials.native_identity(),
                native_authority::Operation::Credentials,
            )
            .unwrap(),
        issuer
            .grant(join.native_identity(), native_authority::Operation::Join)
            .unwrap(),
    ];
    let bases = vec![
        "conduit.base/local@1".into(),
        identity("join").base_implementation,
        identity("credentials").base_implementation,
    ];
    NetworkFixture {
        source,
        bytes,
        identity: expected,
        host,
        bases,
        grants,
        issuer,
        credentials: Some(credentials),
        join: Some(join),
        effects,
    }
}
fn immediate<T>(future: impl core::future::Future<Output = T>) -> T {
    let mut future = Box::pin(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => panic!("bounded CPU provider must complete"),
    }
}
#[test]
fn direct_network_matches_rich_source_selected_owners_and_complete_kernel_trace() {
    let rich = execute(false);
    let direct = execute(true);
    assert_eq!(rich, direct);
}
#[derive(Debug, PartialEq, Eq)]
struct ExecutionTrace {
    calls: Vec<(u16, Vec<u8>, Vec<u8>)>,
    signs: Vec<k::KernelEvent>,
}
fn execute(direct: bool) -> ExecutionTrace {
    let mut f = fixture();
    // SAFETY: freshly checked canonical Source encoded above.
    let image =
        unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&f.bytes) }
            .unwrap();
    let rich = planning::PreparedNetworkSetup::from_indexed_for_identity(
        &f.identity,
        &image,
        &f.host,
        &f.bases,
        &f.grants,
    )
    .unwrap();
    let verified = planning::PreparedVerifiedNetworkSetup::from_indexed_for_identity(
        &f.identity,
        &image,
        f.host.clone(),
        &f.bases,
        &f.grants,
    )
    .unwrap();
    let plan = verified.metadata();
    let fragment = &plan.preparation_metadata().fragments[0];
    let tables = verified.tables().unwrap();
    assert_eq!(
        serde_json::to_value(plan.serialize_with_configurations(|g| {
            Ok::<_, ()>(
                f.source
                    .gears
                    .iter()
                    .find(|s| s.gear_id == g.gear_id)
                    .unwrap()
                    .configuration
                    .clone(),
            )
        }))
        .unwrap(),
        serde_json::to_value(rich.plan()).unwrap()
    );
    let roles = net::PreparedNetworkKernelParts::check_direct(&tables).unwrap();
    let active = if direct {
        f.issuer
            .reserve_verified_network_play(plan, &fragment.fragment_id)
            .unwrap()
    } else {
        f.issuer.reserve_play(&rich.plan().fragments[0]).unwrap()
    };
    let operation = native_authority::Operation::Credentials;
    let (table, handle, claim) = if direct {
        f.issuer.issue_verified_network(
            f.credentials.as_ref().unwrap().native_identity(),
            operation,
            plan,
            &fragment.fragment_id,
            &active,
        )
    } else {
        f.issuer.issue(
            f.credentials.as_ref().unwrap().native_identity(),
            operation,
            &rich.plan().fragments[0],
            &active,
        )
    }
    .unwrap();
    let placement = &fragment.placements[usize::from(roles.credentials.0)].placement_id;
    let mut credentials = if direct {
        ReadyCredentialBase::bind_direct_retained(
            &mut f.credentials,
            table,
            handle,
            claim,
            &net::PreparedCredentialContract::prepare(),
            SelectedDirectOperationPlan {
                tables: &tables,
                active: &active,
                placement_id: placement,
            },
        )
    } else {
        ReadyCredentialBase::bind_selected_retained(
            &mut f.credentials,
            table,
            handle,
            claim,
            &net::PreparedCredentialContract::prepare(),
            SelectedOperationPlan {
                fragment: &rich.plan().fragments[0],
                lowered: rich.lowered(),
                active: &active,
                placement_id: placement,
            },
        )
    }
    .unwrap();
    let operation = native_authority::Operation::Join;
    let (table, handle, claim) = if direct {
        f.issuer.issue_verified_network(
            f.join.as_ref().unwrap().native_identity(),
            operation,
            plan,
            &fragment.fragment_id,
            &active,
        )
    } else {
        f.issuer.issue(
            f.join.as_ref().unwrap().native_identity(),
            operation,
            &rich.plan().fragments[0],
            &active,
        )
    }
    .unwrap();
    let placement = &fragment.placements[usize::from(roles.join.0)].placement_id;
    let mut join = if direct {
        ReadyNetworkJoinBase::bind_direct_retained(
            &mut f.join,
            table,
            handle,
            claim,
            &net::PreparedJoinContract::prepare(),
            SelectedDirectOperationPlan {
                tables: &tables,
                active: &active,
                placement_id: placement,
            },
        )
    } else {
        ReadyNetworkJoinBase::bind_selected_retained(
            &mut f.join,
            table,
            handle,
            claim,
            &net::PreparedJoinContract::prepare(),
            SelectedOperationPlan {
                fragment: &rich.plan().fragments[0],
                lowered: rich.lowered(),
                active: &active,
                placement_id: placement,
            },
        )
    }
    .unwrap();
    assert_eq!(f.effects.get(), 0);
    assert!(f.credentials.is_none() && f.join.is_none());
    let parts = net::PreparedNetworkKernelParts::from_direct(tables).unwrap();
    let mut kernel = if direct {
        parts.finish().unwrap()
    } else {
        net::PreparedNetworkKernel::prepare(&rich.plan().fragments[0], rich.lowered()).unwrap()
    };
    drop(verified);
    drop(rich);
    let mut trace = Vec::new();
    let mut drained = false;
    for _ in 0..100 {
        if let Some(request) = kernel.kernel.next_host_request() {
            let input = kernel
                .kernel
                .host_value(request.input.value)
                .unwrap()
                .to_vec();
            let output = if request.node == roles.credentials {
                let mut output = [0; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize];
                let length = credentials
                    .invoke(
                        request.node,
                        request.call,
                        request.request,
                        &input,
                        &mut output,
                    )
                    .unwrap();
                let value = output[..length].to_vec();
                output.fill(0);
                value
            } else if request.node == roles.join {
                immediate(join.invoke(request.node, request.call, request.request, &input))
                    .unwrap()
                    .to_vec()
            } else {
                Vec::new()
            };
            let reference = if output.is_empty() {
                None
            } else {
                Some(
                    k::BoundedValueRef::new(kernel.kernel.store_host_value(&output).unwrap(), 167)
                        .unwrap(),
                )
            };
            trace.push((request.node.0, input, output));
            kernel
                .kernel
                .complete_host_call(
                    request.node,
                    request.request,
                    k::HostCallOutcome {
                        disposition: k::HostCallDisposition::Completed,
                        output: reference,
                        failure: None,
                    },
                )
                .unwrap();
        }
        if kernel.kernel.step().unwrap() == k::scheduler::SchedulerStatus::Drained {
            drained = true;
            break;
        }
    }
    assert!(drained);
    assert_eq!(trace.len(), 3);
    assert_eq!(f.effects.get(), 2);
    ExecutionTrace {
        calls: trace,
        signs: kernel.kernel.signs().events().collect(),
    }
}

#[test]
fn direct_refusals_match_rich_and_preserve_actual_slots_without_effects() {
    for credentials in [false, true] {
        for failure in [
            "host",
            "boot",
            "plan",
            "play",
            "placement",
            "generation",
            "resource",
            "envelope",
            "parameter",
            "work",
            "revoked",
            "foreign-ready",
        ] {
            assert_eq!(
                refused(false, credentials, failure),
                refused(true, credentials, failure),
                "credentials={credentials} failure={failure}"
            );
        }
    }
}
fn refused(direct: bool, credentials: bool, failure: &str) -> String {
    let mut f = fixture();
    // SAFETY: exact freshly checked Source generated above.
    let image =
        unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&f.bytes) }
            .unwrap();
    let rich = planning::PreparedNetworkSetup::from_indexed_for_identity(
        &f.identity,
        &image,
        &f.host,
        &f.bases,
        &f.grants,
    )
    .unwrap();
    let verified = planning::PreparedVerifiedNetworkSetup::from_indexed_for_identity(
        &f.identity,
        &image,
        f.host.clone(),
        &f.bases,
        &f.grants,
    )
    .unwrap();
    let plan = verified.metadata();
    let fragment = &plan.preparation_metadata().fragments[0];
    let tables = verified.tables().unwrap();
    let operation = if credentials {
        native_authority::Operation::Credentials
    } else {
        native_authority::Operation::Join
    };
    let active = f
        .issuer
        .reserve_verified_network_play(plan, &fragment.fragment_id)
        .unwrap();
    let native = if credentials {
        f.credentials.as_ref().unwrap().native_identity()
    } else {
        f.join.as_ref().unwrap().native_identity()
    };
    let (mut table, handle, mut claim) = f
        .issuer
        .issue_verified_network(native, operation, plan, &fragment.fragment_id, &active)
        .unwrap();
    let roles = net::PreparedNetworkKernelParts::check_direct(&tables).unwrap();
    let mut placement = fragment.placements[usize::from(if credentials {
        roles.credentials.0
    } else {
        roles.join.0
    })]
    .placement_id
    .clone();
    let mut selected_active = active.clone();
    match failure {
        "host" => claim.host_id = "foreign/host".into(),
        "boot" => claim.boot_id = "foreign/boot".into(),
        "plan" => claim.plan_id = "foreign/plan".into(),
        "play" => selected_active.play_sequence += 1,
        "placement" => placement = "foreign/placement".into(),
        "generation" => claim.base_provider_generation += 1,
        "resource" => {
            claim.resource_generation_id = c::ResourceGenerationId("foreign/resource".into())
        }
        "envelope" => claim.envelope_id = "foreign/envelope".into(),
        "parameter" => claim.parameter_bytes += 1,
        "work" => claim.work_units += 1,
        "revoked" => table.revoke(&handle).unwrap(),
        "foreign-ready" => {
            let mut native = identity(if credentials { "credentials" } else { "join" });
            native.artifact = "foreign/artifact".into();
            // SAFETY: independent exclusively owned inert provider, deliberately not selected.
            unsafe {
                if credentials {
                    f.credentials = Some(
                        ReadyCredentialBase::new(
                            native,
                            Provider {
                                effects: f.effects.clone(),
                            },
                        )
                        .unwrap(),
                    );
                } else {
                    f.join = Some(
                        ReadyNetworkJoinBase::new(
                            native,
                            1,
                            Provider {
                                effects: f.effects.clone(),
                            },
                        )
                        .unwrap(),
                    );
                }
            }
        }
        _ => unreachable!(),
    }
    let error = if credentials {
        let contract = net::PreparedCredentialContract::prepare();
        let result = if direct {
            ReadyCredentialBase::bind_direct_retained(
                &mut f.credentials,
                table,
                handle,
                claim,
                &contract,
                SelectedDirectOperationPlan {
                    tables: &tables,
                    active: &selected_active,
                    placement_id: &placement,
                },
            )
        } else {
            ReadyCredentialBase::bind_selected_retained(
                &mut f.credentials,
                table,
                handle,
                claim,
                &contract,
                SelectedOperationPlan {
                    fragment: &rich.plan().fragments[0],
                    lowered: rich.lowered(),
                    active: &selected_active,
                    placement_id: &placement,
                },
            )
        };
        match result {
            Err(error) => format!("{error:?}"),
            Ok(_) => panic!("bad credential selection accepted"),
        }
    } else {
        let contract = net::PreparedJoinContract::prepare();
        let result = if direct {
            ReadyNetworkJoinBase::bind_direct_retained(
                &mut f.join,
                table,
                handle,
                claim,
                &contract,
                SelectedDirectOperationPlan {
                    tables: &tables,
                    active: &selected_active,
                    placement_id: &placement,
                },
            )
        } else {
            ReadyNetworkJoinBase::bind_selected_retained(
                &mut f.join,
                table,
                handle,
                claim,
                &contract,
                SelectedOperationPlan {
                    fragment: &rich.plan().fragments[0],
                    lowered: rich.lowered(),
                    active: &selected_active,
                    placement_id: &placement,
                },
            )
        };
        match result {
            Err(error) => format!("{error:?}"),
            Ok(_) => panic!("bad join selection accepted"),
        }
    };
    assert!(f.credentials.is_some() && f.join.is_some());
    assert_eq!(f.effects.get(), 0);
    error
}
#[test]
fn verified_root_scope_refuses_foreign_boot_fragment_and_unreserved_play() {
    let mut f = fixture();
    // SAFETY: exact freshly checked linked Source.
    let image =
        unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&f.bytes) }
            .unwrap();
    let verified = planning::PreparedVerifiedNetworkSetup::from_indexed_for_identity(
        &f.identity,
        &image,
        f.host.clone(),
        &f.bases,
        &f.grants,
    )
    .unwrap();
    let plan = verified.metadata();
    let id = &plan.preparation_metadata().fragments[0].fragment_id;
    let active = c::bind_active_play(plan.plan_id(), &f.host.host_id, &f.host.boot_id, 1);
    assert!(f
        .issuer
        .issue_verified_network(
            f.join.as_ref().unwrap().native_identity(),
            native_authority::Operation::Join,
            plan,
            id,
            &active
        )
        .is_err());
    assert!(f
        .issuer
        .reserve_verified_network_play(plan, &"foreign/fragment".into())
        .is_err());
    let mut foreign =
        native_authority::RootIssuer::fixture(f.host.host_id.clone(), "foreign/boot".into());
    assert!(foreign.reserve_verified_network_play(plan, id).is_err());
    let actual = f.issuer.reserve_verified_network_play(plan, id).unwrap();
    assert_eq!(actual, active);
    assert!(f
        .issuer
        .issue_verified_network(
            f.join.as_ref().unwrap().native_identity(),
            native_authority::Operation::Join,
            plan,
            &"foreign/fragment".into(),
            &active
        )
        .is_err());
    f.issuer
        .issue_verified_network(
            f.join.as_ref().unwrap().native_identity(),
            native_authority::Operation::Join,
            plan,
            id,
            &active,
        )
        .unwrap();
    assert!(f
        .issuer
        .issue_verified_network(
            f.join.as_ref().unwrap().native_identity(),
            native_authority::Operation::Join,
            plan,
            id,
            &active
        )
        .is_err());
    assert_eq!(f.effects.get(), 0);
}
