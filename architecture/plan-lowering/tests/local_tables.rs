#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_plan_lowering::{local_tables::*, lowering::lower_verified_plan_metadata_fragment};
#[path = "common/execution_regions.rs"]
mod fixtures;
type Tables<'a> = VerifiedLocalKernelTables<'a, 2, 1, 4, 8, 1, 2>;
fn local_plan() -> c::Plan {
    let mut fragment = fixtures::transfer_plan().fragments.remove(0);
    fragment.execution_regions.clear();
    fixtures::common::seal(fragment)
}
fn verified(plan: c::Plan) -> c::VerifiedPlanMetadata {
    c::verify_plan_metadata_with_configurations(plan, |_| Ok::<_, ()>(vec![])).unwrap()
}
#[test]
fn local_cord_and_startup_are_projected_without_assigned_identity() {
    let plan = verified(local_plan());
    let fragment = &plan.preparation_metadata().fragments[0];
    let lowered = lower_verified_plan_metadata_fragment(&plan, &fragment.fragment_id).unwrap();
    let tables = Tables::prepare(&plan, &fragment.fragment_id, &lowered).unwrap();
    assert_eq!(
        tables.startup_order(),
        &[conduit_kernel::NodeId(0), conduit_kernel::NodeId(1)]
    );
    assert_eq!(tables.cords[0], lowered.cords[0].spec);
    assert_eq!(
        tables.nodes[1].input_cords[0],
        Some(conduit_kernel::CordId(0))
    );
    assert!(tables.routes.is_sealed());
    assert_eq!(
        tables
            .routes
            .route(conduit_kernel::NodeId(0), conduit_kernel::PortId(0))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(tables.preparation_scope().0.plan_id(), plan.plan_id());
}
#[test]
fn substituted_tables_and_insufficient_capacity_refuse() {
    let plan = verified(local_plan());
    let id = &plan.preparation_metadata().fragments[0].fragment_id;
    let lowered = lower_verified_plan_metadata_fragment(&plan, id).unwrap();
    assert!(matches!(
        Tables::prepare(&plan, &"missing".into(), &lowered),
        Err(LocalTableRefusal::Scope)
    ));
    let mut changed = lowered.clone();
    changed.cords[0].spec.byte_capacity += 1;
    assert!(matches!(
        Tables::prepare(&plan, id, &changed),
        Err(LocalTableRefusal::Lowering)
    ));
    assert!(matches!(
        VerifiedLocalKernelTables::<1, 1, 4, 8, 1, 2>::prepare(&plan, id, &lowered),
        Err(LocalTableRefusal::Capacity)
    ));
    assert!(VerifiedLocalKernelTables::<2, 1, 4, 8, 0, 2>::prepare(&plan, id, &lowered).is_err());
}
#[test]
fn partition_contracts_refuse_instead_of_being_dropped() {
    for rich in [fixtures::transfer_plan(), fixtures::plan(2)] {
        let plan = verified(rich);
        let id = &plan.preparation_metadata().fragments[0].fragment_id;
        match lower_verified_plan_metadata_fragment(&plan, id) {
            Ok(lowered) => {
                let result = if lowered.nodes.len() == 1 {
                    VerifiedLocalKernelTables::<1, 0, 4, 4, 0, 1>::prepare(&plan, id, &lowered)
                        .map(|_| ())
                } else {
                    Tables::prepare(&plan, id, &lowered).map(|_| ())
                };
                assert_eq!(result, Err(LocalTableRefusal::Unsupported));
            }
            Err(conduit_plan_lowering::lowering::LoweringError::UnsupportedExecutionRegion(_)) => {
                let safe = verified(local_plan());
                let safe_id = &safe.preparation_metadata().fragments[0].fragment_id;
                let substituted = lower_verified_plan_metadata_fragment(&safe, safe_id).unwrap();
                assert!(matches!(
                    Tables::prepare(&plan, id, &substituted),
                    Err(LocalTableRefusal::Lowering)
                ));
            }
            Err(error) => panic!("unexpected profile refusal: {error:?}"),
        }
    }
}

#[test]
fn a_state_profile_cannot_replace_the_local_storage_profile() {
    use conduit_plan_lowering::lowering::{
        lower_verified_plan_metadata_fragment_for_profile, FIXED_KERNEL_STORAGE_PROFILE,
    };
    let plan = verified(fixtures::common::seal(fixtures::common::fragment()));
    let id = &plan.preparation_metadata().fragments[0].fragment_id;
    let lowered = lower_verified_plan_metadata_fragment_for_profile(
        &plan,
        id,
        FIXED_KERNEL_STORAGE_PROFILE
            .with_state_storage(1, 1)
            .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        VerifiedLocalKernelTables::<1, 0, 4, 4, 0, 1>::prepare(&plan, id, &lowered),
        Err(LocalTableRefusal::Lowering)
    ));
}

#[test]
fn owned_verified_lowering_avoids_duplicate_preparation_and_preserves_tables() {
    let plan = verified(local_plan());
    let id = &plan.preparation_metadata().fragments[0].fragment_id;
    let lowering = VerifiedLocalLowering::prepare(&plan, id).unwrap();
    let (borrowed, before) =
        allocation_probe::observe(|| Tables::prepare(&plan, id, lowering.lowered()).unwrap());
    let (direct, after) =
        allocation_probe::observe(|| Tables::from_verified_lowering(&lowering).unwrap());
    assert_eq!((after.allocations, after.reallocations), (0, 0));
    assert!(before.peak_bytes > after.peak_bytes);
    assert_eq!(direct.nodes, borrowed.nodes);
    assert_eq!(direct.cords, borrowed.cords);
    assert_eq!(direct.startup_order(), borrowed.startup_order());
    assert_eq!(
        direct.preparation_scope().0.plan_id(),
        borrowed.preparation_scope().0.plan_id()
    );
    assert_eq!(direct.preparation_scope().1, borrowed.preparation_scope().1);
    for node in 0..2 {
        for port in 0..4 {
            let route = |tables: &Tables<'_>| {
                tables
                    .routes
                    .route(conduit_kernel::NodeId(node), conduit_kernel::PortId(port))
                    .map(|targets| targets.collect::<Vec<_>>())
            };
            assert_eq!(route(&direct), route(&borrowed));
        }
    }
    assert!(matches!(
        VerifiedLocalLowering::prepare(&plan, &"missing".into()),
        Err(LocalTableRefusal::Scope)
    ));
    assert!(matches!(
        VerifiedLocalKernelTables::<1, 1, 4, 8, 1, 2>::from_verified_lowering(&lowering),
        Err(LocalTableRefusal::Capacity)
    ));
}
