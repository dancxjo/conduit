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
