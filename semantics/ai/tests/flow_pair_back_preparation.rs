#![cfg(feature = "fixed-numeric-owners")]
use conduit_ai::fixed_numeric_pair_flow::*;
#[path = "../src/numeric_allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn limits() -> FlowPairProfilePreparationLimits {
    FlowPairProfilePreparationLimits {
        maximum_preparation_requested_bytes: FLOW_PAIR_PROFILE_PREPARATION_REQUESTED_BYTES,
        maximum_preparation_peak_bytes: FLOW_PAIR_PROFILE_PREPARATION_PEAK_BYTES,
        maximum_retained_heap_bytes: FLOW_PAIR_PROFILE_RETAINED_BYTES,
    }
}
#[test]
fn all_six_original_profiles_and_backs_are_reserved() {
    for identity in IMMUTABLE_FLOW_PAIR_KINDS {
        for which in 0..3 {
            let mut l = limits();
            match which {
                0 => l.maximum_preparation_requested_bytes -= 1,
                1 => l.maximum_preparation_peak_bytes -= 1,
                _ => l.maximum_retained_heap_bytes -= 1,
            }
            let (r, o) = allocation_probe::observe(|| {
                PreparedFixedFlowPairProfile::prepare_immutable_no_cache(identity, l)
            });
            assert!(matches!(r, Err(FlowPairStorageRefusal::Capacity)));
            assert_eq!((o.allocations, o.reallocations), (0, 0));
        }
        let (r, o) = allocation_probe::observe(|| {
            PreparedFixedFlowPairProfile::prepare_immutable_no_cache(identity, limits())
        });
        let (profile, receipt) = r.unwrap();
        assert!(o.requested_bytes <= receipt.preparation_requested_bytes_bound);
        assert!(o.peak_bytes <= receipt.preparation_peak_bytes_bound);
        assert_eq!(o.live_bytes, profile.owned_heap_bytes().unwrap());
        assert_eq!(o.live_bytes, receipt.retained_accounted_heap_bytes);
        println!(
            "profile {identity} requested={} peak={} retained={}",
            o.requested_bytes, o.peak_bytes, o.live_bytes
        );
        let (r, o) =
            allocation_probe::observe(|| FixedFlowPairBack::storage_reservation(&profile).unwrap());
        assert_eq!((o.allocations, o.reallocations), (0, 0));
        for which in 0..2 {
            let mut requested = r.preparation_requested_bytes_bound;
            let mut retained = r.retained_heap_bytes_bound;
            if which == 0 {
                requested -= 1;
            } else {
                retained -= 1;
            }
            let (result, o) = allocation_probe::observe(|| {
                FixedFlowPairBack::prepare_selected_with_storage_limits(
                    &profile, requested, retained,
                )
            });
            assert!(matches!(result, Err(FlowPairStorageRefusal::Capacity)));
            assert_eq!((o.allocations, o.reallocations), (0, 0));
        }
        let (result, o) = allocation_probe::observe(|| {
            FixedFlowPairBack::prepare_selected_with_storage_limits(
                &profile,
                r.preparation_requested_bytes_bound,
                r.retained_heap_bytes_bound,
            )
        });
        let (back, receipt) = result.unwrap();
        assert!(o.requested_bytes <= receipt.preparation_requested_bytes_bound);
        assert!(o.live_bytes <= receipt.retained_heap_bytes_bound);
        assert_eq!(o.live_bytes, back.local_accounted_heap_bytes());
        println!(
            "back {identity} requested={} peak={} retained={} reservation={r:?}",
            o.requested_bytes, o.peak_bytes, o.live_bytes
        );
    }
}
#[test]
fn foreign_profile_refuses_without_catalog_allocation() {
    let (r, o) = allocation_probe::observe(|| {
        PreparedFixedFlowPairProfile::prepare_immutable_no_cache("foreign", limits())
    });
    assert!(matches!(r, Err(FlowPairStorageRefusal::ForeignKind)));
    assert_eq!((o.allocations, o.reallocations), (0, 0));
}

#[test]
fn actual_archived_plan_factory_uses_matching_prepared_profile() {
    use conduit_ai::operation_owners::fixed_numeric_pair_flow::FixedFlowPairOperationFactory;
    let artifact_root = std::env::var("CONDUIT_FARGAN_PREPARATION_ARTIFACT_ROOT").unwrap();
    let bytes = std::fs::read(format!(
        "{artifact_root}/outputs/committed-common-fargan-greeting/sealed-epoch-plan.json"
    ))
    .unwrap();
    let plan: conduit_core::Plan = serde_json::from_slice(&bytes).unwrap();
    let (factory, factory_allocations) =
        allocation_probe::observe(|| FixedFlowPairOperationFactory::for_plan(&plan).unwrap());
    assert_eq!(
        factory_allocations.live_bytes,
        factory.local_owned_payload_bytes().unwrap()
    );
    println!(
        "actual retained factory payload={} selection array={} transient preparation requested={} peak={} (not yet admitted)",
        factory_allocations.live_bytes,
        factory.retained_selection_array_bytes().unwrap(),
        factory_allocations.requested_bytes,
        factory_allocations.peak_bytes
    );
    let profiles: Vec<_> = IMMUTABLE_FLOW_PAIR_KINDS
        .iter()
        .map(|identity| {
            PreparedFixedFlowPairProfile::prepare_immutable_no_cache(identity, limits())
                .unwrap()
                .0
        })
        .collect();
    let refs: Vec<_> = profiles.iter().collect();
    let (reused, reused_allocations) = allocation_probe::observe(|| {
        FixedFlowPairOperationFactory::for_plan_with_prepared_profiles(&plan, &refs).unwrap()
    });
    assert_eq!(
        reused_allocations.live_bytes,
        reused.local_owned_payload_bytes().unwrap()
    );
    assert!(
        reused.local_owned_payload_bytes().unwrap() <= factory.local_owned_payload_bytes().unwrap()
    );
    assert!(reused_allocations.requested_bytes < factory_allocations.requested_bytes);
    for gear in plan.fragments[0]
        .placements
        .iter()
        .filter(|gear| gear.kind_id.as_str().starts_with("numeric/flow-pair"))
    {
        use conduit_composite::KernelOperationFactory;
        let a = factory.budget(gear).unwrap();
        let b = reused.budget(gear).unwrap();
        assert_eq!(
            (
                a.value_items,
                a.value_bytes,
                a.maximum_value_bytes,
                a.host_requests,
                a.sign_items
            ),
            (
                b.value_items,
                b.value_bytes,
                b.maximum_value_bytes,
                b.host_requests,
                b.sign_items
            )
        );
    }
    println!(
        "reused profile factory requested={} peak={} retained={} (original profile owners/Plan verification upstream separately charged)",
        reused_allocations.requested_bytes,
        reused_allocations.peak_bytes,
        reused_allocations.live_bytes
    );
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.kind_id.as_str() == IMMUTABLE_FLOW_PAIR_KINDS[0])
        .unwrap();
    let (profile, _) =
        PreparedFixedFlowPairProfile::prepare_immutable_no_cache(gear.kind_id.as_str(), limits())
            .unwrap();
    let (r, o) = allocation_probe::observe(|| {
        factory
            .preparation_storage_reservation(gear, &profile)
            .unwrap()
    });
    assert_eq!((o.allocations, o.reallocations), (0, 0));
    for which in 0..2 {
        let mut requested = r.preparation_requested_bytes_bound;
        let mut retained = r.retained_heap_bytes_bound;
        if which == 0 {
            requested -= 1;
        } else {
            retained -= 1;
        }
        let (result, o) = allocation_probe::observe(|| {
            factory.prepare_with_storage_limits(gear, &profile, requested, retained)
        });
        assert!(matches!(result, Err(FlowPairStorageRefusal::Capacity)));
        assert_eq!((o.allocations, o.reallocations), (0, 0));
    }
    let (result, o) = allocation_probe::observe(|| {
        factory.prepare_with_storage_limits(
            gear,
            &profile,
            r.preparation_requested_bytes_bound,
            r.retained_heap_bytes_bound,
        )
    });
    let (back, receipt) = result.unwrap();
    let actual_back = o;
    assert!(o.requested_bytes <= receipt.preparation_requested_bytes_bound);
    assert_eq!(
        o.live_bytes,
        back.storage().accounted_retained_bytes().unwrap()
    );
    let (foreign, _) = PreparedFixedFlowPairProfile::prepare_immutable_no_cache(
        IMMUTABLE_FLOW_PAIR_KINDS[1],
        limits(),
    )
    .unwrap();
    let (result, o) =
        allocation_probe::observe(|| factory.preparation_storage_reservation(gear, &foreign));
    assert!(matches!(
        result,
        Err(FlowPairStorageRefusal::ProfileNotPrepared)
    ));
    assert_eq!((o.allocations, o.reallocations), (0, 0));
    println!(
        "actual archived {}: Back requested={} peak={} retained={} profile retained={} (factory/Plan upstream preparation separately charged)",
        gear.placement_id.as_str(),
        actual_back.requested_bytes,
        actual_back.peak_bytes,
        actual_back.live_bytes,
        profile.owned_heap_bytes().unwrap()
    );
}

#[test]
fn prepared_profile_payload_inventories_match_actual_capacities() {
    use conduit_ai::{
        closing_structured_pair::ClosingStructuredPairProfile,
        fixed_numeric_catalog::fixed_numeric_type, fixed_numeric_guard::FixedGuardProfile,
        fixed_numeric_u16_profile::PreparedU16Profile, nominal_weakening::PreparedNominalWeakening,
    };
    use conduit_core::*;
    let ty = fixed_numeric_type("NumericF32Vector20").unwrap();
    let (profile, o) =
        allocation_probe::observe(|| FixedGuardProfile::prepare(ty.clone()).unwrap());
    assert_eq!(o.live_bytes, profile.owned_payload_bytes().unwrap());
    let (profile, o) = allocation_probe::observe(|| {
        ClosingStructuredPairProfile::prepare(
            StructuredInfoType::leaf(kind_id("value/u16")).unwrap(),
            ty.clone(),
        )
        .unwrap()
    });
    assert_eq!(o.live_bytes, profile.owned_payload_bytes().unwrap());
    let (profile, o) =
        allocation_probe::observe(|| PreparedNominalWeakening::prepare(ty.clone()).unwrap());
    assert_eq!(o.live_bytes, profile.owned_payload_bytes().unwrap());
    for source in [
        "type FarganPeriod = U16 in 32..=255\n",
        "type Even = U16 in 32..=255 where . % 2 == 0\n",
    ] {
        let (profile, o) =
            allocation_probe::observe(|| PreparedU16Profile::check_definition(source).unwrap());
        assert_eq!(o.live_bytes, profile.owned_payload_bytes().unwrap());
    }
}
