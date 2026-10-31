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

#[test]
fn direct_verified_projection_matches_ordinary_tables_without_heap_or_identity_copies() {
    use conduit_kernel::{NodeId, PortId};
    let plan = verified(local_plan());
    let id = &plan.preparation_metadata().fragments[0].fragment_id;
    let (lowering, ordinary_memory) =
        allocation_probe::observe(|| VerifiedLocalLowering::prepare(&plan, id).unwrap());
    let ordinary = Tables::from_verified_lowering(&lowering).unwrap();
    let (direct, memory) = allocation_probe::observe(|| {
        DirectLocalKernelTables::<2, 1, 4, 8, 1, 2>::prepare(&plan, id).unwrap()
    });
    assert_eq!(
        (memory.allocations, memory.reallocations, memory.peak_bytes),
        (0, 0, 0)
    );
    assert!(ordinary_memory.peak_bytes > memory.peak_bytes);
    assert_eq!(*direct.nodes(), ordinary.nodes);
    assert_eq!(*direct.cords(), ordinary.cords);
    assert_eq!(direct.startup_order(), ordinary.startup_order());
    assert_eq!(
        direct.cord_value_slots(),
        lowering.lowered().cord_value_slots
    );
    assert_eq!(
        direct.cord_value_bytes(),
        lowering.lowered().cord_value_bytes
    );
    assert_eq!(direct.sign_items(), lowering.lowered().sign_items);
    assert_eq!(direct.sign_bytes(), lowering.lowered().sign_bytes);
    for (index, gear) in plan.preparation_metadata().fragments[0]
        .placements
        .iter()
        .enumerate()
    {
        assert_eq!(
            direct.node_for_placement(&gear.placement_id),
            Some(NodeId(index as u16))
        );
    }
    for node in 0..2 {
        for port in 0..4 {
            assert_eq!(
                direct
                    .routes()
                    .route(NodeId(node), PortId(port))
                    .map(|i| i.collect::<Vec<_>>()),
                ordinary
                    .routes
                    .route(NodeId(node), PortId(port))
                    .map(|i| i.collect::<Vec<_>>())
            );
        }
    }
    assert_eq!(direct.preparation_scope().0.plan_id(), plan.plan_id());
    assert!(core::ptr::eq(
        direct.preparation_scope().1,
        &plan.preparation_metadata().fragments[0]
    ));
}

#[test]
fn direct_tables_preserve_scope_capacity_and_partition_refusals() {
    let plan = verified(local_plan());
    let id = &plan.preparation_metadata().fragments[0].fragment_id;
    assert!(matches!(
        DirectLocalKernelTables::<2, 1, 4, 8, 1, 2>::prepare(&plan, &"missing".into()),
        Err(LocalTableRefusal::Scope)
    ));
    assert!(matches!(
        DirectLocalKernelTables::<1, 1, 4, 8, 1, 2>::prepare(&plan, id),
        Err(LocalTableRefusal::Capacity)
    ));
    assert!(DirectLocalKernelTables::<2, 1, 4, 8, 0, 2>::prepare(&plan, id).is_err());
    for rich in [fixtures::transfer_plan(), fixtures::plan(2)] {
        let verified = verified(rich);
        let id = &verified.preparation_metadata().fragments[0].fragment_id;
        let actual = if verified.preparation_metadata().fragments[0]
            .placements
            .len()
            == 1
        {
            DirectLocalKernelTables::<1, 0, 4, 4, 0, 1>::prepare(&verified, id).map(|_| ())
        } else {
            DirectLocalKernelTables::<2, 1, 4, 8, 1, 2>::prepare(&verified, id).map(|_| ())
        };
        let expected = VerifiedLocalLowering::prepare(&verified, id).and_then(|lowered| {
            if lowered.lowered().nodes.len() == 1 {
                VerifiedLocalKernelTables::<1, 0, 4, 4, 0, 1>::from_verified_lowering(&lowered)
                    .map(|_| ())
            } else {
                Tables::from_verified_lowering(&lowered).map(|_| ())
            }
        });
        assert_eq!(actual, expected);
    }
}

#[test]
fn direct_and_ordinary_tables_emit_identical_kernel_events() {
    use conduit_kernel::{
        self as k,
        scheduler::{
            CordSpec, FixedScheduler, NodeSpec, StepBack, StepInputBytes, StepIo, StepOutcome,
        },
    };
    #[derive(Clone, Copy)]
    struct Back {
        source: bool,
        sent: bool,
    }
    impl StepBack<4> for Back {
        fn step(&mut self, io: &mut StepIo<4>, bytes: &StepInputBytes<'_, 4>) -> StepOutcome {
            if self.source {
                if self.sent || !io.output_ready(k::PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send_canonical(k::PortId(0), k::CanonicalValue::new(&[11]).unwrap())
                    .unwrap();
            } else {
                let Some(value) = bytes.input(k::PortId(0)) else {
                    return StepOutcome::Await;
                };
                assert_eq!(value, [11]);
                io.consume(k::PortId(0)).unwrap();
            }
            StepOutcome::Progress
        }
        fn step_committed(&mut self) {
            if self.source {
                self.sent = true;
            }
        }
    }
    fn run(
        nodes: [NodeSpec<4>; 2],
        cords: [CordSpec; 1],
        routes: k::FixedRoutes<8, 1>,
    ) -> Vec<k::KernelEvent> {
        let mut scheduler = FixedScheduler::<_, _, _, 2, 1, 4, 2, 8, 1>::new(
            nodes,
            cords,
            routes,
            [
                Back {
                    source: true,
                    sent: false,
                },
                Back {
                    source: false,
                    sent: false,
                },
            ],
            k::FixedValueStore::<3, 1>::new(3).unwrap(),
            k::FixedSignLog::<64>::new(64 * core::mem::size_of::<k::KernelEvent>() as u32).unwrap(),
        )
        .unwrap();
        for _ in 0..10 {
            scheduler.step().unwrap();
        }
        scheduler.cancel().unwrap();
        scheduler.signs().events().collect()
    }
    let plan = verified(local_plan());
    let id = &plan.preparation_metadata().fragments[0].fragment_id;
    let lowered = VerifiedLocalLowering::prepare(&plan, id).unwrap();
    let ordinary = Tables::from_verified_lowering(&lowered).unwrap();
    let direct = DirectLocalKernelTables::<2, 1, 4, 8, 1, 2>::prepare(&plan, id).unwrap();
    let before = run(ordinary.nodes, ordinary.cords, ordinary.routes);
    let direct = direct.into_parts();
    let after = run(direct.nodes, direct.cords, direct.routes);
    assert!(!after.is_empty());
    assert_eq!(before, after);
}

#[test]
fn direct_projection_preserves_external_fore_and_native_attachment_fences() {
    fn same<const C: usize>(plan: c::Plan) {
        let plan = verified(plan);
        let id = &plan.preparation_metadata().fragments[0].fragment_id;
        let expected = VerifiedLocalLowering::prepare(&plan, id).and_then(|lowered| {
            VerifiedLocalKernelTables::<1, C, 4, 4, 2, 2>::from_verified_lowering(&lowered)
                .map(|_| ())
        });
        let actual = DirectLocalKernelTables::<1, C, 4, 4, 2, 2>::prepare(&plan, id).map(|_| ());
        assert!(expected.is_err());
        assert_eq!(actual, expected);
    }
    let base = || {
        let mut f = fixtures::common::fragment();
        f.states.clear();
        f.sign_storage_budget = c::mandatory_sign_storage_requirement(&f.expected_sign).unwrap();
        f
    };
    let mut fore = base();
    fore.fore_ports = [
        ("commands", "next", c::PortDirection::Input),
        ("states", "current", c::PortDirection::Output),
    ]
    .into_iter()
    .map(|(front, gear, direction)| c::PlannedForePort {
        front_port_id: c::port_id(front),
        direction,
        placement_id: "placement".into(),
        gear_port_id: c::port_id(gear),
        value_kind: c::kind_id("fixture/byte@1"),
        value_contract: None,
        abnormal_kind: None,
        track: c::ConnectionTrack::Payload,
        temporal: c::PortTemporal::Value,
        pressure_policy: c::DeliveryPressurePolicy::PreserveOrder,
        item_capacity: 1,
        byte_capacity: 1,
        selected_line: None,
    })
    .collect();
    same::<2>(fixtures::common::seal(fore));
    let call = |name: &str, maximum_in_flight| c::HostCallRequirement {
        contract_id: name.into(),
        target_kind: None,
        maximum_in_flight,
        maximum_input_bytes: 1,
        maximum_output_bytes: 1,
    };
    let mut concurrent = base();
    concurrent.placements[0]
        .host_calls
        .push(call("call/one", 2));
    same::<0>(fixtures::common::seal(concurrent));
    let mut multiple = base();
    multiple.placements[0].host_calls = vec![call("call/one", 1), call("call/two", 1)];
    same::<0>(fixtures::common::seal(multiple));
    let mut unowned = base();
    unowned.placements[0].resources.push(c::ResourceBinding {
        pool_id: "resource/pool".into(),
        class_id: "resource/class".into(),
        units: 1,
        protected: None,
        compute: None,
        content: None,
    });
    same::<0>(fixtures::common::seal(unowned));
}
