//! Complete current-Source/Root-issued owner construction and execution.
//! CPU transcript providers do not establish physical radio/SDK/target admission.
use super::{allocation_probe, c, native_authority as root};
use conduit_net::*;
use conduit_network_join_call::{installation::*, *};
use conduit_plot::{
    CheckedConnectionRef, CheckedGearRef, LinkedPlanningFactsRef, PlanningOriginRef,
};
use std::sync::OnceLock;
struct Inert;
impl NetworkJoinProvider for Inert {
    async fn join(
        &mut self,
        _: NetworkJoinRequest<'_>,
    ) -> Result<AttachmentRecord, StationAttemptFailure> {
        Ok(AttachmentRecord::encode(NetworkAttachmentInfo {
            attachment_id: "cpu/attached",
            host_id: "esp32/b0a732286178",
            boot_id: "boot/0123456789abcdef0123456789abcdef",
            resource_pool_id: "pool/esp32/wifi-station",
            generation: u64::MAX,
        })
        .unwrap())
    }
    async fn cleanup(&mut self) -> Result<(), StationFailure> {
        panic!("planning must not poll cleanup")
    }
}
impl CredentialSource for Inert {
    fn take_acknowledged(&mut self, output: &mut [u8]) -> Result<usize, CredentialSourceRefusal> {
        encode_network_join_request(
            NetworkJoinRequest {
                ssid: b"fixture-network",
                credential: b"fixture-only",
            },
            output,
        )
        .map_err(|_| CredentialSourceRefusal::OutputCapacity)
    }
}
fn identity(role: &str) -> NetworkNativeIdentity {
    NetworkNativeIdentity {
        host: "esp32/b0a732286178".into(),
        boot: "boot/0123456789abcdef0123456789abcdef".into(),
        base: format!("base/esp32/{role}").into(),
        instance: format!("instance/esp32/{role}").into(),
        provider_generation: u64::MAX,
        resource: format!("pool/esp32/{role}").into(),
        resource_generation: c::ResourceGenerationId(format!("generation/{role}/{}", u64::MAX)),
        envelope: format!("envelope/esp32/{role}").into(),
        artifact: format!("native/esp32-{role}-root@1").into(),
        implementation: format!("esp32/{role}@1").into(),
        base_implementation: format!("esp32/{role}-base@1").into(),
    }
}

#[test]
fn network_whole_borrowed_constructor_matches_rich_dispatch_and_retains_refusals() {
    let image = source_image();
    let expected = image.identity().unwrap();
    let expected = (
        expected.source_document_id.as_str(),
        expected.checked_plot_id.as_str(),
        expected.expanded_plot_id.as_str(),
    );
    let mut issuer = root::RootIssuer::fixture(
        "esp32/b0a732286178".into(),
        "boot/0123456789abcdef0123456789abcdef".into(),
    );
    let host: c::HostId = "layout/host".into();
    let boot: c::BootId = "layout/boot".into();
    let instance: c::BaseInstanceId = "layout/instance".into();
    let (table, table_layout) = allocation_probe::observe(|| {
        c::BaseCapabilityTable::new(host, boot, instance, 1, [1; 32], 1).unwrap()
    });
    println!(
        "CPU64 one empty capability entry allocation {}",
        table_layout.peak_bytes
    );
    drop(table);
    let (retained, stats) = allocation_probe::observe(|| {
        let bases = [c::BaseImplementationId::from(
            c::LOCAL_BASE_IMPLEMENTATION_ID,
        )];
        let image = source_image();
        // SAFETY: exclusive inert providers; no SDK initialization/effects claimed.
        let pair = unsafe {
            ReadyNetworkPair::new(
                credential_installation::ReadyCredentialBase::new(
                    identity("private-network-input"),
                    Inert,
                )
                .unwrap(),
                ReadyNetworkJoinBase::new(identity("wifi-station"), u64::MAX, Inert).unwrap(),
            )
        };

        let prepared = pair
            .prepare(|owners| {
                let grants = [
                    issuer
                        .grant_ref(owners.credential_identity(), root::Operation::Credentials)
                        .unwrap(),
                    issuer
                        .grant_ref(owners.join_identity(), root::Operation::Join)
                        .unwrap(),
                ];
                let attachment = PreparedCanonicalNetworkOffer::attachment_local(
                    "conduit/esp32/network-attachment-receipt@1",
                    "esp32/network-attachment-receipt@1",
                    "native/esp32-network-attachment-receipt@1",
                );
                let header = c::HostAdvertisementRef {
                    protocol_version: c::PROTOCOL_VERSION,
                    host_id: owners.join_identity().host.as_str(),
                    boot_id: owners.join_identity().boot.as_str(),
                    offer_generation: c::OfferGeneration(u64::MAX),
                    profile: "environment/esp32-native-root@1",
                    bases: c::BaseProviderAdvertisementRefs::Borrowed(&[]),
                    resources: c::ResourceOfferRefs::Borrowed(&[]),
                    capabilities: c::CapabilityOfferRefs::Borrowed(&[]),
                    planner_capabilities: &[],
                };

                owners
                    .with_advertisement(header, &attachment, |owners, host| {
                        conduit_planner::borrowed_local::with_linked_local_plan(
                            &image,
                            host,
                            &bases,
                            c::AuthorityGrantRefs::Borrowed(&grants),
                            1,
                            MAXIMUM_JOIN_INPUT_BYTES,
                            |scope| {
                                let tables =
                                    NetworkControlFlatDirectTables::prepare(scope.verified())
                                        .unwrap();
                                let roles =
                                    PreparedNetworkKernelParts::check_flat_direct(&tables).unwrap();
                                let view = scope.verified().view();
                                let credential = view
                                    .placements
                                    .iter()
                                    .nth(usize::from(roles.credentials.0))
                                    .unwrap()
                                    .placement_id;
                                let join = view
                                    .placements
                                    .iter()
                                    .nth(usize::from(roles.join.0))
                                    .unwrap()
                                    .placement_id;
                                let active =
                                    issuer.reserve_linked_network_play(scope, expected).unwrap();
                                let (table, handle, claim) = issuer
                                    .issue_linked_network(
                                        owners.credential_identity(),
                                        root::Operation::Credentials,
                                        scope,
                                        expected,
                                        &active,
                                    )
                                    .unwrap();
                                let credential_lease = NetworkOperationLease {
                                    table,
                                    handle,
                                    claim,
                                };

                                let credential_selection = owners
                                    .check_flat_credentials(
                                        credential_lease,
                                        &tables,
                                        &active,
                                        credential,
                                    )
                                    .unwrap_or_else(|_| {
                                        panic!("actual credential selection refused")
                                    });
                                let (table, handle, claim) = issuer
                                    .issue_linked_network(
                                        credential_selection.join_identity(),
                                        root::Operation::Join,
                                        scope,
                                        expected,
                                        &active,
                                    )
                                    .unwrap();
                                let join_lease = NetworkOperationLease {
                                    table,
                                    handle,
                                    claim,
                                };

                                let checked = credential_selection
                                    .check_flat_join(join_lease, &tables, &active, join)
                                    .unwrap_or_else(|_| panic!("actual native bindings refused"));
                                let parts =
                                    PreparedNetworkKernelParts::from_flat_direct(tables).unwrap();
                                Ok::<_, ()>((checked, (parts, active)))
                            },
                        )
                        .unwrap()
                    })
                    .unwrap()
            })
            .unwrap_or_else(|_| panic!("actual owner pair refused"));
        let (credentials, join, (parts, active)) = prepared;
        drop(bases);

        let kernel = Box::new(parts.finish().unwrap());

        (credentials, join, kernel, active)
    });
    println!("WHOLE beforeReady through exact Source/ordinary planner/Rootleases/selected actual owners/full kernel: {:?}; retained {}",stats,stats.retained_bytes());
    assert!(
        stats.peak_bytes < 9_000,
        "complete CPU64 construction unexpectedly grew: {stats:?}"
    );
    assert_eq!(conduit_kernel::SignSink::len(retained.2.kernel.signs()), 0);
    println!(
        "inline kernel {} credential {} join {} table {} claim {}",
        core::mem::size_of::<PreparedNetworkKernel>(),
        core::mem::size_of_val(&retained.0),
        core::mem::size_of_val(&retained.1),
        core::mem::size_of::<c::BaseCapabilityTable>(),
        core::mem::size_of::<c::BaseOperationClaim>()
    );

    let trace = execute(retained);
    let rich = prepare_rich(&image);
    assert_eq!(trace, execute(rich));
    println!("Complete actual Source rich/borrowed owner dispatch bytes and all26 canonical kernel Signs equal");
    for mutation in 0..7 {
        check_partial_refusal(&image, mutation);
    }
    println!("Seven exact partial-issuance/native-claim/bearer refusals retain both Ready owners and issued operation custody");
}

type Constructed = (
    NetworkCredentialsHostCall<Inert>,
    NetworkJoinHostCall<Inert>,
    Box<PreparedNetworkKernel>,
    c::ActivePlayIdentity,
);
type ExecutionTrace = (
    Vec<(u16, Vec<u8>, Vec<u8>)>,
    Vec<conduit_kernel::KernelEvent>,
);
fn execute(retained: Constructed) -> ExecutionTrace {
    let (mut credentials, mut join, mut kernel, _active) = retained;
    let roles = kernel.roles;
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
                let mut out = [0; MAXIMUM_JOIN_INPUT_BYTES as usize];
                let length = credentials
                    .invoke(
                        request.node,
                        request.call,
                        request.request,
                        &input,
                        &mut out,
                    )
                    .unwrap();
                out[..length].to_vec()
            } else if request.node == roles.join {
                let mut future = core::pin::pin!(join.invoke(
                    request.node,
                    request.call,
                    request.request,
                    &input
                ));
                let mut context = std::task::Context::from_waker(std::task::Waker::noop());
                match core::future::Future::poll(future.as_mut(), &mut context) {
                    std::task::Poll::Ready(Ok(value)) => value.to_vec(),
                    _ => panic!("CPU provider did not finish"),
                }
            } else {
                Vec::new()
            };
            let reference = if output.is_empty() {
                None
            } else {
                Some(
                    conduit_kernel::BoundedValueRef::new(
                        kernel.kernel.store_host_value(&output).unwrap(),
                        MAXIMUM_JOIN_OUTPUT_BYTES,
                    )
                    .unwrap(),
                )
            };
            trace.push((request.node.0, input, output));
            kernel
                .kernel
                .complete_host_call(
                    request.node,
                    request.request,
                    conduit_kernel::HostCallOutcome {
                        disposition: conduit_kernel::HostCallDisposition::Completed,
                        output: reference,
                        failure: None,
                    },
                )
                .unwrap();
        }
        if kernel.kernel.step().unwrap() == conduit_kernel::scheduler::SchedulerStatus::Drained {
            drained = true;
            break;
        }
    }
    assert!(drained);
    assert_eq!(trace.len(), 3);
    assert!(!credentials.retirement_ready());
    assert_eq!(join.inspection().completed_operations, 1);
    (trace, kernel.kernel.signs().events().collect())
}

fn prepare_rich(image: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>) -> Constructed {
    let mut credentials = Some(unsafe {
        credential_installation::ReadyCredentialBase::new(identity("private-network-input"), Inert)
            .unwrap()
    });
    let mut join = Some(unsafe {
        ReadyNetworkJoinBase::new(identity("wifi-station"), u64::MAX, Inert).unwrap()
    });
    let j = join.as_ref().unwrap().native_identity();
    let mut issuer = root::RootIssuer::fixture(j.host.clone(), j.boot.clone());
    let mut host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: j.host.clone(),
        boot_id: j.boot.clone(),
        offer_generation: c::OfferGeneration(u64::MAX),
        profile: "environment/esp32-native-root@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    credentials
        .as_ref()
        .unwrap()
        .append_to_advertisement(&mut host)
        .unwrap();
    host.capabilities.push(network_attachment_sign_local_offer(
        "conduit/esp32/network-attachment-receipt@1".into(),
        "esp32/network-attachment-receipt@1".into(),
        "native/esp32-network-attachment-receipt@1".into(),
    ));
    join.as_ref()
        .unwrap()
        .append_to_advertisement(&mut host)
        .unwrap();
    host.bases.sort_by(|a, b| a.base_id.cmp(&b.base_id));
    host.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    let grants = [
        issuer
            .grant(
                credentials.as_ref().unwrap().native_identity(),
                root::Operation::Credentials,
            )
            .unwrap(),
        issuer
            .grant(
                join.as_ref().unwrap().native_identity(),
                root::Operation::Join,
            )
            .unwrap(),
    ];
    let placements =
        conduit_planner::indexed_source::default_placements(image, &[host.clone()]).unwrap();
    let empty = std::collections::BTreeMap::new();
    let empty_candidates = std::collections::BTreeMap::new();
    let empty_limits = std::collections::BTreeMap::new();
    let options = conduit_planner::PlanningOptions {
        connection_bases: &empty,
        line_candidates: &empty_candidates,
        connection_item_capacity: 1,
        connection_byte_capacity: MAXIMUM_JOIN_INPUT_BYTES,
        authority_grants: &grants,
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let bases = [c::BaseImplementationId::from(
        c::LOCAL_BASE_IMPLEMENTATION_ID,
    )];
    let plan = conduit_planner::indexed_source::plan_with_connection_limits(
        image,
        &[host],
        &placements,
        &bases,
        options,
        &empty_limits,
    )
    .unwrap();
    let fragment = &plan.fragments[0];
    let lower = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    let parts = CheckedNetworkLowering::prepare(fragment)
        .unwrap()
        .kernel_parts(fragment)
        .unwrap();
    let roles = parts.roles();
    let active = issuer.reserve_play(fragment).unwrap();
    let (table, handle, claim) = issuer
        .issue(
            credentials.as_ref().unwrap().native_identity(),
            root::Operation::Credentials,
            fragment,
            &active,
        )
        .unwrap();
    let credential_placement = &fragment.placements[usize::from(roles.credentials.0)].placement_id;
    let credentials = credential_installation::ReadyCredentialBase::bind_selected_retained(
        &mut credentials,
        table,
        handle,
        claim,
        &PreparedCredentialContract::prepare(),
        conduit_plan_lowering::selected_operation::SelectedOperationPlan {
            fragment,
            lowered: &lower,
            active: &active,
            placement_id: credential_placement,
        },
    )
    .unwrap();
    let (table, handle, claim) = issuer
        .issue(
            join.as_ref().unwrap().native_identity(),
            root::Operation::Join,
            fragment,
            &active,
        )
        .unwrap();
    let join_placement = &fragment.placements[usize::from(roles.join.0)].placement_id;
    let join = ReadyNetworkJoinBase::bind_selected_retained(
        &mut join,
        table,
        handle,
        claim,
        &PreparedJoinContract::prepare(),
        conduit_plan_lowering::selected_operation::SelectedOperationPlan {
            fragment,
            lowered: &lower,
            active: &active,
            placement_id: join_placement,
        },
    )
    .unwrap();
    (credentials, join, Box::new(parts.finish().unwrap()), active)
}

struct FailedCustody {
    credential: conduit_core::BoundBaseOperation,
    join: NetworkOperationLease,
    error: NetworkPairRefusal,
}
#[allow(
    clippy::result_large_err,
    reason = "Refusal must retain both actual owners and every issued operation without another allocation."
)]
fn check_partial_refusal(
    image: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>,
    mutation: usize,
) {
    let expected = image.identity().unwrap();
    let expected = (
        expected.source_document_id.as_str(),
        expected.checked_plot_id.as_str(),
        expected.expanded_plot_id.as_str(),
    );
    let mut issuer = root::RootIssuer::fixture(
        "esp32/b0a732286178".into(),
        "boot/0123456789abcdef0123456789abcdef".into(),
    );
    let pair = unsafe {
        ReadyNetworkPair::new(
            credential_installation::ReadyCredentialBase::new(
                identity("private-network-input"),
                Inert,
            )
            .unwrap(),
            ReadyNetworkJoinBase::new(identity("wifi-station"), u64::MAX, Inert).unwrap(),
        )
    };
    let bases = [c::BaseImplementationId::from(
        c::LOCAL_BASE_IMPLEMENTATION_ID,
    )];
    let result = pair.prepare(|owners| {
        let grants = [
            issuer
                .grant_ref(owners.credential_identity(), root::Operation::Credentials)
                .unwrap(),
            issuer
                .grant_ref(owners.join_identity(), root::Operation::Join)
                .unwrap(),
        ];
        let attachment = PreparedCanonicalNetworkOffer::attachment_local(
            "conduit/esp32/network-attachment-receipt@1",
            "esp32/network-attachment-receipt@1",
            "native/esp32-network-attachment-receipt@1",
        );
        let header = c::HostAdvertisementRef {
            protocol_version: c::PROTOCOL_VERSION,
            host_id: owners.join_identity().host.as_str(),
            boot_id: owners.join_identity().boot.as_str(),
            offer_generation: c::OfferGeneration(u64::MAX),
            profile: "environment/esp32-native-root@1",
            bases: c::BaseProviderAdvertisementRefs::Borrowed(&[]),
            resources: c::ResourceOfferRefs::Borrowed(&[]),
            capabilities: c::CapabilityOfferRefs::Borrowed(&[]),
            planner_capabilities: &[],
        };
        owners
            .with_advertisement(header, &attachment, |owners, host| {
                conduit_planner::borrowed_local::with_linked_local_plan(
                    image,
                    host,
                    &bases,
                    c::AuthorityGrantRefs::Borrowed(&grants),
                    1,
                    MAXIMUM_JOIN_INPUT_BYTES,
                    |scope| {
                        let tables =
                            NetworkControlFlatDirectTables::prepare(scope.verified()).unwrap();
                        let roles = PreparedNetworkKernelParts::check_flat_direct(&tables).unwrap();
                        let view = scope.verified().view();
                        let credential = view
                            .placements
                            .iter()
                            .nth(usize::from(roles.credentials.0))
                            .unwrap()
                            .placement_id;
                        let join = view
                            .placements
                            .iter()
                            .nth(usize::from(roles.join.0))
                            .unwrap()
                            .placement_id;
                        let wrong_pins = [
                            ("wrong/source", expected.1, expected.2),
                            (expected.0, "wrong/checked", expected.2),
                            (expected.0, expected.1, "wrong/expanded"),
                        ];
                        for pins in wrong_pins {
                            assert!(issuer.reserve_linked_network_play(scope, pins).is_err());
                        }
                        let active = issuer.reserve_linked_network_play(scope, expected).unwrap();
                        assert_eq!(
                            active.play_sequence, 1,
                            "Source-pin refusals must not consume a sequence"
                        );
                        for pins in wrong_pins {
                            assert!(issuer
                                .issue_linked_network(
                                    owners.credential_identity(),
                                    root::Operation::Credentials,
                                    scope,
                                    pins,
                                    &active
                                )
                                .is_err());
                        }
                        let (table, handle, claim) = issuer
                            .issue_linked_network(
                                owners.credential_identity(),
                                root::Operation::Credentials,
                                scope,
                                expected,
                                &active,
                            )
                            .unwrap();
                        let first = owners
                            .check_flat_credentials(
                                NetworkOperationLease {
                                    table,
                                    handle,
                                    claim,
                                },
                                &tables,
                                &active,
                                credential,
                            )
                            .unwrap_or_else(|_| panic!("credential preflight"));
                        let (table, handle, claim) = issuer
                            .issue_linked_network(
                                first.join_identity(),
                                root::Operation::Join,
                                scope,
                                expected,
                                &active,
                            )
                            .unwrap();
                        let mut join_lease = NetworkOperationLease {
                            table,
                            handle,
                            claim,
                        };
                        let mut wrong_active = active.clone();
                        match mutation {
                            0 => {
                                join_lease.claim.implementation_id = "forged/implementation".into()
                            }
                            1 => join_lease.claim.base_provider_generation -= 1,
                            2 => {
                                join_lease.claim.resource_generation_id =
                                    c::ResourceGenerationId("stale/resource-generation".into())
                            }
                            3 => join_lease.claim.plan_id = "forged/plan".into(),
                            4 => wrong_active.active_play_id = "stale/active".into(),
                            5 => {
                                let mut other = root::RootIssuer::fixture(
                                    active.host_id.clone(),
                                    active.boot_id.clone(),
                                );
                                let other_active =
                                    other.reserve_linked_network_play(scope, expected).unwrap();
                                let (_, handle, _) = other
                                    .issue_linked_network(
                                        first.join_identity(),
                                        root::Operation::Credentials,
                                        scope,
                                        expected,
                                        &other_active,
                                    )
                                    .unwrap();
                                join_lease.handle = handle;
                            }
                            6 => join_lease.table.revoke(&join_lease.handle).unwrap(),
                            _ => unreachable!(),
                        }
                        let actual = if mutation == 4 {
                            &wrong_active
                        } else {
                            &active
                        };
                        match first.check_flat_join(join_lease, &tables, actual, join) {
                            Ok(_) => panic!("forged or stale binding accepted {mutation}"),
                            Err((first, join, error)) => {
                                Err::<(PreparedNetworkPairBindings<'_>, ()), _>(FailedCustody {
                                    credential: first.into_operation(),
                                    join,
                                    error,
                                })
                            }
                        }
                    },
                )
                .unwrap()
            })
            .unwrap()
    });
    let (pair, failure) = match result {
        Err(value) => value,
        Ok(_) => panic!("partial refusal auto-ran"),
    };
    let (credentials, join) = pair.into_ready();
    assert_eq!(
        credentials.native_identity(),
        &identity("private-network-input")
    );
    assert_eq!(join.native_identity(), &identity("wifi-station"));
    assert_eq!(
        failure.credential.inspection().lifecycle,
        c::CapabilityLifecycle::Issued
    );
    assert_eq!(failure.credential.inspection().completed_operations, 0);
    assert_eq!(failure.credential.inspection().in_flight_operations, 0);
    let second = failure.join.table.inspections().next().unwrap();
    assert_eq!(second.completed_operations, 0);
    assert_eq!(second.in_flight_operations, 0);
    assert_eq!(
        second.lifecycle,
        if mutation == 6 {
            c::CapabilityLifecycle::Revoked
        } else {
            c::CapabilityLifecycle::Issued
        }
    );
    match (mutation, failure.error) {
        (
            0..=3,
            NetworkPairRefusal::Join(JoinCallRefusal::WrongBinding | JoinCallRefusal::Binding(_)),
        )
        | (4, NetworkPairRefusal::Join(JoinCallRefusal::WrongBinding))
        | (
            5,
            NetworkPairRefusal::Join(JoinCallRefusal::Capability(
                c::BaseCapabilityRefusal::UnknownCapability,
            )),
        )
        | (6, NetworkPairRefusal::Join(JoinCallRefusal::Binding(_))) => {}
        other => panic!("unexpected exact refusal {other:?}"),
    }
}

struct PinnedSource {
    facts: LinkedPlanningFactsRef<'static>,
    bytes: &'static [u8],
}
static SOURCE: OnceLock<PinnedSource> = OnceLock::new();
fn pinned_source() -> &'static PinnedSource {
    SOURCE.get_or_init(|| {
        // The ordinary Make checker/expander owns every field. Leaking this immutable
        // fixture models ROM; no alleged IDs or hand-authored numeric graph enter it.
        let source = Box::leak(Box::new(
            conduit_environment_make::compile_network_setup().unwrap(),
        ));
        let gears = Box::leak(
            source
                .gears
                .iter()
                .map(CheckedGearRef::from)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let connections = Box::leak(
            source
                .connections
                .iter()
                .map(CheckedConnectionRef::from)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let provenance = Box::leak(
            source
                .provenance
                .iter()
                .map(|v| PlanningOriginRef {
                    gear_id: v.gear_id.as_str(),
                    source_span: v.source_span,
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let bytes = Box::leak(
            conduit_plot::flat_image::indexed::encode(source)
                .unwrap()
                .into_boxed_slice(),
        );
        if let Some(path) = std::env::var_os("CONDUIT_NETWORK_PAIRED_SOURCE") {
            let native = std::fs::read(path).expect("independently selected native Make image");
            assert_eq!(
                native.as_slice(),
                bytes,
                "current ordinary Make Source must equal paired native image"
            );
        }
        PinnedSource {
            facts: LinkedPlanningFactsRef {
                source_document_id: source.source_document_id.as_str(),
                checked_plot_id: source.checked_plot_id.as_str(),
                expanded_plot_id: source.expanded_plot_id.as_str(),
                name: &source.name,
                completion: source.completion,
                gears,
                connections,
                provenance,
            },
            bytes,
        }
    })
}
fn linked_facts() -> Result<LinkedPlanningFactsRef<'static>, conduit_plot::flat_image::ImageRefusal>
{
    Ok(pinned_source().facts)
}
fn source_image() -> conduit_plot::flat_image::indexed::IndexedSourceImage<'static> {
    // SAFETY: ordinary Make checked/expanded and encoded this exact immutable
    // Source; the callback above borrows every complete field from that same value.
    unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(
            pinned_source().bytes,
        )
        .unwrap()
        .with_make_planning_facts(linked_facts)
    }
}
