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
    assert_eq!(scheduler.has_quiescence_cords, quiescence);
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
}
