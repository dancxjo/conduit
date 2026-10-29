//! Complete borrowed Source/offer planning and affine provider handoff on CPU.
//! Pinned fixture facts model linked Make ROM; they are not target memory proof.
use super::*;
use conduit_core as c;
use conduit_net::*;
use conduit_network_join_call::scan::*;
use conduit_plan_lowering::selected_operation::SelectedFlatDirectOperationPlan;
use conduit_plot::{
    CheckedConnectionRef, CheckedGearRef, LinkedPlanningFactsRef, PlanningOriginRef,
};
use std::sync::OnceLock;
static FACTS: OnceLock<LinkedPlanningFactsRef<'static>> = OnceLock::new();
fn facts() -> Result<LinkedPlanningFactsRef<'static>, conduit_plot::flat_image::ImageRefusal> {
    FACTS
        .get()
        .copied()
        .ok_or(conduit_plot::flat_image::ImageRefusal::UnsupportedGraph)
}
fn pinned_image() -> Vec<u8> {
    let source = Box::leak(Box::new(
        conduit_environment_make::compile_network_scan().unwrap(),
    ));
    let bytes = conduit_plot::flat_image::indexed::encode(source).unwrap();
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
            .map(|p| PlanningOriginRef {
                gear_id: p.gear_id.as_str(),
                source_span: p.source_span,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    assert!(FACTS
        .set(LinkedPlanningFactsRef {
            source_document_id: source.source_document_id.as_str(),
            checked_plot_id: source.checked_plot_id.as_str(),
            expanded_plot_id: source.expanded_plot_id.as_str(),
            name: &source.name,
            completion: source.completion,
            gears,
            connections,
            provenance,
        })
        .is_ok());
    bytes
}
#[test]
fn borrowed_scan_preparation_moves_provider_only_after_descriptions_retire() {
    let bytes = pinned_image();
    // SAFETY: all callback facts above came from this exact checked Source.
    // This fixture construction is outside allocation observations, like ROM.
    let image = unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&bytes)
            .unwrap()
            .with_make_planning_facts(facts)
    };
    for refuse_after_binding in [false, true] {
        let (expected, ready) = scan_fixture();
        let native = ready.native_identity();
        let active = c::bind_active_play(&expected.plan_id, &native.host, &native.boot, 1);
        let mut lease = Some(possession::possession(issued_scope(&ready, &active)));
        let grants = [c::AuthorityGrant {
            grant_id: "cpu/grant".into(),
            contract_id: NETWORK_SCAN_AUTHORITY.into(),
            host_call_contract_id: NETWORK_SCAN_HOST_CALL.into(),
            subject_kind: NETWORK_SCAN_SUBJECT.into(),
            host_id: native.host.clone(),
            boot_id: native.boot.clone(),
            capability_id: SCAN_CAPABILITY.into(),
        }];
        let bases = ["conduit.base/local@1".into(), "cpu/base@1".into()];
        let (result, memory) = allocation_probe::observe(|| {
            ready.prepare(|view| {
                let native = view.native_identity();
                let header = c::HostAdvertisementRef {
                    protocol_version: c::PROTOCOL_VERSION,
                    host_id: native.host.as_str(),
                    boot_id: native.boot.as_str(),
                    offer_generation: c::OfferGeneration(1),
                    profile: "cpu/scan",
                    bases: c::BaseProviderAdvertisementRefs::Borrowed(&[]),
                    resources: c::ResourceOfferRefs::Borrowed(&[]),
                    capabilities: c::CapabilityOfferRefs::Borrowed(&[]),
                    planner_capabilities: &[],
                };
                let before = allocation_probe::current_observation().unwrap();
                let evidence = PreparedCanonicalNetworkOffer::scan_sign(
                    "cpu/sign",
                    "cpu/sign@1",
                    "cpu/sign-artifact",
                );
                view.with_advertisement(header, &evidence, |owner, host| {
                    let after = allocation_probe::current_observation().unwrap();
                    assert_eq!(after.allocations, before.allocations);
                    assert_eq!(after.reallocations, before.reallocations);
                    assert_eq!(
                        (
                            host.bases.len(),
                            host.resources.len(),
                            host.capabilities.len()
                        ),
                        (1, 1, 2)
                    );
                    conduit_planner::borrowed_local::with_linked_local_plan(
                        &image,
                        host,
                        &bases,
                        c::AuthorityGrantRefs::Owned(&grants),
                        1,
                        MAXIMUM_SCAN_OUTPUT_BYTES,
                        |scope| {
                            let planned = scope.verified().view();
                            assert_eq!(planned.plan_id, expected.plan_id.as_str());
                            assert_eq!(
                                planned.fragment_id,
                                expected.fragments[0].fragment_id.as_str()
                            );
                            let tables = ScanFlatDirectTables::prepare(scope.verified()).unwrap();
                            let roles =
                                PreparedScanKernelParts::check_flat_direct(&tables).unwrap();
                            let placement = planned
                                .placements
                                .iter()
                                .find(|g| g.kind_id == NETWORK_SCAN_OPERATION)
                                .unwrap()
                                .placement_id;
                            let (table, handle, claim) = lease.take().unwrap();
                            let binding = owner
                                .check_flat(
                                    table,
                                    handle,
                                    claim,
                                    SelectedFlatDirectOperationPlan {
                                        tables: &tables,
                                        active: &active,
                                        placement_id: placement,
                                    },
                                )
                                .unwrap_or_else(|_| panic!("same actual initialized owner"));
                            assert_eq!(binding.node(), roles.scan);
                            if refuse_after_binding {
                                return Err(binding.into_operation());
                            }
                            let parts = PreparedScanKernelParts::from_flat_direct(tables).unwrap();
                            Ok((binding, (parts, roles.scan)))
                        },
                    )
                    .unwrap()
                })
                .unwrap()
            })
        });
        assert!(lease.is_none());
        if refuse_after_binding {
            let (returned, mut issued) = result
                .err()
                .unwrap_or_else(|| panic!("requested later refusal"));
            assert_eq!(returned.native_identity().host.as_str(), "cpu/host");
            assert_eq!(
                issued.inspection().scope.active_play_id,
                active.active_play_id
            );
            assert_eq!(
                issued.inspection().lifecycle,
                c::CapabilityLifecycle::Issued
            );
            issued.revoke().unwrap();
            assert_eq!(
                issued.inspection().lifecycle,
                c::CapabilityLifecycle::Revoked
            );
        } else {
            let (mut selected, (parts, node)) =
                result.unwrap_or_else(|_| panic!("complete selected preparation"));
            // No physical scan runs in this proof: revoke the unused selected
            // operation before returning its same inert provider.
            selected
                .revoke(node, conduit_kernel::HostCallId(0))
                .unwrap();
            let mut selected = Some(selected);
            assert!(NetworkScanHostCall::retire_retained(&mut selected).is_ok());
            drop(parts);
        }
        println!("CPU borrowed scan planning peak={} allocations={} (excludes fixture ROM/Ready/Root/issuer/kernel; not target fit)",memory.peak_bytes,memory.allocations);
    }
}
