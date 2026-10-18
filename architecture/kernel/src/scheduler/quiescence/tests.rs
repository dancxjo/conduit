extern crate std;
use super::*;
use crate::{FixedSignLog, FixedValueStore, KernelEvent, RouteRange};

#[derive(Clone, Copy)]
struct Await;
impl StepBack<1> for Await {
    fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        StepOutcome::Await
    }
    fn cancel(&mut self) {}
}

type Scheduler =
    FixedScheduler<Await, FixedValueStore<2, 1>, FixedSignLog<16>, 2, 2, 1, 2, 2, 1, 2, 1>;

fn parts(quiescence: bool) -> ([NodeSpec<1>; 2], [CordSpec; 2], FixedRoutes<2, 1>) {
    let mut routes = FixedRoutes::new(1);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let track = if quiescence {
        AssignedConnectionTrack::Quiescence
    } else {
        AssignedConnectionTrack::Payload
    };
    (
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 8,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_fuel: 8,
            },
        ],
        [
            CordSpec::local(
                CordId(0),
                (NodeId(0), PortId(0)),
                (NodeId(1), PortId(0)),
                CordCapacity {
                    slot_start: 0,
                    item_capacity: 1,
                    byte_capacity: 1,
                    pressure_policy: Default::default(),
                },
            )
            .with_track(track),
            // A quiescence-marked padding entry must remain inert.
            CordSpec::inactive().with_track(AssignedConnectionTrack::Quiescence),
        ],
        routes,
    )
}

fn signs() -> FixedSignLog<16> {
    FixedSignLog::new((16 * core::mem::size_of::<KernelEvent>()) as u32).unwrap()
}

fn assert_prepared(scheduler: &mut Scheduler, quiescence: bool) {
    assert_eq!(scheduler.quiescence_cord_end, usize::from(quiescence));
    for _ in 0..2 {
        assert!(matches!(
            scheduler.step().unwrap(),
            SchedulerStatus::Progress { .. }
        ));
    }
    assert_eq!(scheduler.step().unwrap(), SchedulerStatus::Idle);
    if !quiescence {
        scheduler.arm_quiescence_outputs(0).unwrap();
        assert_eq!(scheduler.scan_counts.get().cord_scans, 0);
    }
}

#[test]
fn ordinary_preparation_uses_only_active_cords() {
    for quiescence in [false, true] {
        let (nodes, cords, routes) = parts(quiescence);
        let mut scheduler = Scheduler::new_with_active_counts(
            2,
            1,
            nodes,
            cords,
            routes,
            [Await; 2],
            FixedValueStore::new(2).unwrap(),
            signs(),
        )
        .unwrap();
        assert_prepared(&mut scheduler, quiescence);
    }
}

#[cfg(feature = "alloc")]
#[test]
fn boxed_preparation_uses_only_active_cords() {
    for quiescence in [false, true] {
        let (nodes, cords, routes) = parts(quiescence);
        let mut bindings = FixedHostCallBindings::new(1);
        bindings.seal().unwrap();
        let mut scheduler = Scheduler::new_boxed_with_active_counts_and_host_calls(
            2,
            1,
            nodes,
            cords,
            routes,
            bindings,
            [Await; 2],
            FixedValueStore::new(2).unwrap(),
            signs(),
        )
        .unwrap();
        assert_prepared(&mut scheduler, quiescence);
    }
}

#[test]
fn quiescence_arming_visits_only_the_sources_routed_targets() {
    const N: usize = 128;
    let mut routes = FixedRoutes::<N, 3>::new(1);
    let targets = core::array::from_fn::<_, 3, _>(|cord| RouteTarget {
        cord: CordId(cord as u16),
        sink: CordEndpoint::local(NodeId((cord + 1) as u16), PortId(0)),
    });
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 3 },
            &targets,
        )
        .unwrap();
    routes.seal().unwrap();
    let nodes = core::array::from_fn(|node| NodeSpec {
        input_cords: [if node == 0 {
            None
        } else {
            Some(CordId((node - 1) as u16))
        }],
        maximum_step_fuel: 8,
    });
    let cords = core::array::from_fn(|cord| {
        if cord == N - 1 {
            return CordSpec::inactive().with_track(AssignedConnectionTrack::Quiescence);
        }
        let capacity = CordCapacity {
            slot_start: cord as u16,
            item_capacity: 1,
            byte_capacity: 1,
            pressure_policy: Default::default(),
        };
        if cord < 3 {
            CordSpec::local(
                CordId(cord as u16),
                (NodeId(0), PortId(0)),
                (NodeId((cord + 1) as u16), PortId(0)),
                capacity,
            )
            .with_track(if cord < 2 {
                AssignedConnectionTrack::Quiescence
            } else {
                AssignedConnectionTrack::Payload
            })
        } else {
            CordSpec::remote_ingress(
                CordId(cord as u16),
                RemoteEndpointId(cord as u16),
                (NodeId((cord + 1) as u16), PortId(0)),
                capacity,
            )
        }
    });
    let mut scheduler = FixedScheduler::<_, _, _, N, N, 1, N, N, 3>::new_with_active_counts(
        N,
        N - 1,
        nodes,
        cords,
        routes,
        [Await; N],
        FixedValueStore::<2, 1>::new(2).unwrap(),
        signs(),
    )
    .unwrap();
    scheduler.scan_counts.set(Default::default());
    scheduler.arm_quiescence_outputs(0).unwrap();
    assert_eq!(scheduler.scan_counts.get().cord_scans, 3);
    for (cord, state) in scheduler.cords.iter().enumerate() {
        assert_eq!(state.quiescence_armed, cord < 2);
    }
    scheduler.scan_counts.set(Default::default());
    scheduler.arm_quiescence_outputs(1).unwrap();
    assert_eq!(scheduler.scan_counts.get().cord_scans, 0);
    assert!(!scheduler.cords[N - 1].quiescence_armed);
    assert_eq!(scheduler.quiescence_cord_end, 2);
    scheduler.scan_counts.set(Default::default());
    assert_eq!(
        scheduler.emit_quiescence_transitions().unwrap(),
        Some(NodeId(0))
    );
    assert_eq!(scheduler.scan_counts.get().cord_scans, 4);
    assert_eq!(scheduler.cords[0].len, 1);
    assert_eq!(scheduler.cords[1].len, 1);
    assert_eq!(scheduler.cords[2].len, 0);
    assert!(scheduler.cords.iter().all(|cord| !cord.quiescence_armed));
    let entries = scheduler.signs.entries;
    scheduler.scan_counts.set(Default::default());
    assert_eq!(scheduler.emit_quiescence_transitions().unwrap(), None);
    assert_eq!(scheduler.scan_counts.get().cord_scans, 0);
    assert_eq!(scheduler.signs.entries, entries);
    assert_eq!(scheduler.armed_quiescence_count, 0);
    // Rearming is idempotent, including while the existing queues are full.
    scheduler.arm_quiescence_outputs(0).unwrap();
    scheduler.arm_quiescence_outputs(0).unwrap();
    assert_eq!(scheduler.armed_quiescence_count, 2);
    assert_eq!(scheduler.emit_quiescence_transitions().unwrap(), None);
    assert_eq!(scheduler.armed_quiescence_count, 2);
    assert_eq!(scheduler.signs.entries, entries);
    for cord in 0..2 {
        let value = scheduler.pop(cord).unwrap();
        scheduler.values.release(value).unwrap();
        assert_eq!(
            scheduler.emit_quiescence_transitions().unwrap(),
            Some(NodeId(0))
        );
        assert_eq!(scheduler.armed_quiescence_count, 1 - cord);
        assert!(!scheduler.cords[cord].quiescence_armed);
    }
    scheduler.scan_counts.set(Default::default());
    assert_eq!(scheduler.emit_quiescence_transitions().unwrap(), None);
    assert_eq!(scheduler.scan_counts.get().cord_scans, 0);
    scheduler.arm_quiescence_outputs(0).unwrap();
    assert_eq!(scheduler.armed_quiescence_count, 2);
    scheduler.cancel().unwrap();
    assert_eq!(scheduler.armed_quiescence_count, 0);
    assert!(scheduler.cords.iter().all(|cord| !cord.quiescence_armed));
    assert_eq!(scheduler.values.used_items(), 0);
}

#[test]
fn prepared_quiescence_bound_includes_last_match_and_ignores_empty_prefix() {
    assert_eq!(quiescence_cord_end(&[]), 0);
    for mask in 0u16..256 {
        let cords: [CordSpec; 8] = core::array::from_fn(|i| {
            CordSpec::inactive().with_track(if mask & (1 << i) != 0 {
                AssignedConnectionTrack::Quiescence
            } else {
                AssignedConnectionTrack::Payload
            })
        });
        for active in 0..=8 {
            let expected =
                (0..active).fold(0, |end, i| if mask & (1 << i) != 0 { i + 1 } else { end });
            assert_eq!(quiescence_cord_end(&cords[..active]), expected);
        }
    }
}

fn sparse_quiescence(
) -> FixedScheduler<Await, FixedValueStore<4, 1>, FixedSignLog<16>, 128, 127, 1, 127, 128, 127> {
    let mut routes = FixedRoutes::new(1);
    for cord in 0..127 {
        routes
            .install(
                NodeId(cord),
                PortId(0),
                RouteRange {
                    start: cord,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(cord),
                    sink: CordEndpoint::local(NodeId(cord + 1), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    FixedScheduler::new(
        core::array::from_fn(|node| NodeSpec {
            input_cords: [if node == 0 {
                None
            } else {
                Some(CordId((node - 1) as u16))
            }],
            maximum_step_fuel: 8,
        }),
        core::array::from_fn(|cord| {
            CordSpec::local(
                CordId(cord as u16),
                (NodeId(cord as u16), PortId(0)),
                (NodeId((cord + 1) as u16), PortId(0)),
                CordCapacity {
                    slot_start: cord as u16,
                    item_capacity: 1,
                    byte_capacity: 1,
                    pressure_policy: Default::default(),
                },
            )
            .with_track(AssignedConnectionTrack::Quiescence)
        }),
        routes,
        [Await; 128],
        FixedValueStore::new(4).unwrap(),
        signs(),
    )
    .unwrap()
}

#[test]
fn sparse_armed_quiescence_ignores_unarmed_tail_without_losing_ineligible_edges() {
    let mut scheduler = sparse_quiescence();
    scheduler.arm_quiescence_outputs(0).unwrap();
    scheduler.scan_counts.set(Default::default());
    assert_eq!(
        scheduler.emit_quiescence_transitions().unwrap(),
        Some(NodeId(0))
    );
    std::println!(
        "SPARSE_QUIESCENCE_PROBES {}",
        scheduler.scan_counts.get().cord_scans
    );
    assert_eq!(scheduler.scan_counts.get().cord_scans, 2);
    assert_eq!(scheduler.cords[0].len, 1);
    assert_eq!(scheduler.armed_quiescence_count, 0);
    // A full early edge and a completed source remain armed but cannot emit.
    scheduler.arm_quiescence_outputs(0).unwrap();
    scheduler.arm_quiescence_outputs(63).unwrap();
    scheduler.arm_quiescence_outputs(126).unwrap();
    scheduler.mark_completed(63);
    assert_eq!(scheduler.armed_quiescence_count, 3);
    assert_eq!(
        scheduler.emit_quiescence_transitions().unwrap(),
        Some(NodeId(126))
    );
    assert_eq!(scheduler.armed_quiescence_count, 2);
    assert!(scheduler.cords[0].quiescence_armed);
    assert!(scheduler.cords[63].quiescence_armed);
    assert!(!scheduler.cords[126].quiescence_armed);
    let entries = scheduler.signs.entries;
    assert_eq!(scheduler.emit_quiescence_transitions().unwrap(), None);
    assert_eq!(scheduler.signs.entries, entries);
    let value = scheduler.pop(0).unwrap();
    scheduler.values.release(value).unwrap();
    assert_eq!(
        scheduler.emit_quiescence_transitions().unwrap(),
        Some(NodeId(0))
    );
    assert_eq!(scheduler.armed_quiescence_count, 1);
    assert!(scheduler.cords[63].quiescence_armed);
    scheduler.cancel().unwrap();
    assert_eq!(scheduler.armed_quiescence_count, 0);
    assert_eq!(scheduler.values.used_items(), 0);
}

#[test]
fn sparse_quiescence_matches_full_scan_eligibility_and_sign_order() {
    for layout in 0..64 {
        let mut scheduler = sparse_quiescence();
        for (index, cord) in [0, 63, 126].into_iter().enumerate() {
            let state = (layout >> (index * 2)) & 3;
            if state != 0 {
                scheduler.arm_quiescence_outputs(cord).unwrap();
            }
            if state == 2 {
                let value = scheduler.values.store(&[]).unwrap();
                scheduler.push(cord, value).unwrap();
            }
            if state == 3 {
                scheduler.mark_completed(cord);
            }
        }
        let before = scheduler.armed_quiescence_count;
        let expected: std::vec::Vec<_> = (0..127)
            .filter(|&cord| {
                scheduler.cords[cord].quiescence_armed
                    && scheduler.cords[cord].len == 0
                    && !scheduler.completed.contains(cord)
            })
            .map(|cord| NodeId(cord as u16))
            .collect();
        assert_eq!(
            scheduler.emit_quiescence_transitions().unwrap(),
            expected.first().copied()
        );
        let actual: std::vec::Vec<_> = scheduler
            .signs
            .events()
            .filter(|event| event.kind == KernelEventKind::ValueRouted)
            .map(|event| event.node)
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(scheduler.armed_quiescence_count, before - expected.len());
        for node in expected {
            assert!(!scheduler.cords[usize::from(node.0)].quiescence_armed);
            assert_eq!(scheduler.cords[usize::from(node.0)].len, 1);
        }
        let entries = scheduler.signs.entries;
        assert_eq!(scheduler.emit_quiescence_transitions().unwrap(), None);
        assert_eq!(scheduler.signs.entries, entries);
        scheduler.cancel().unwrap();
        assert_eq!(scheduler.values.used_items(), 0);
    }
}
