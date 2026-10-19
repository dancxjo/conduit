//! Atomic materialization and rollback for distinct prepared output ports.
use super::*;
#[derive(Clone, Copy)]
struct Pair {
    source: bool,
    committed: u8,
}
impl StepBack<PORTS> for Pair {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if !self.source || !io.output_ready(PortId(0)) || !io.output_ready(PortId(1)) {
            return StepOutcome::Await;
        }
        io.send_prepared(PortId(0), 4).unwrap();
        io.send_prepared(PortId(1), 4).unwrap();
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match port {
            PortId(0) => Some(b"abcd"),
            PortId(1) => Some(b"wxyz"),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        self.committed += 1;
    }
}
fn routes() -> FixedRoutes<2, 2> {
    let mut routes = FixedRoutes::new(PORTS as u16);
    for index in 0..2 {
        routes
            .install(
                NodeId(0),
                PortId(index),
                RouteRange {
                    start: index,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(index),
                    sink: crate::CordEndpoint::local(NodeId(1), PortId(index)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    routes
}
fn cords(second_bound: u32) -> [CordSpec; 2] {
    core::array::from_fn(|index| {
        CordSpec::local(
            CordId(index as u16),
            (NodeId(0), PortId(index as u16)),
            (NodeId(1), PortId(index as u16)),
            CordCapacity {
                slot_start: index as u16,
                item_capacity: 1,
                byte_capacity: 4,
                pressure_policy: Default::default(),
            },
        )
        .with_maximum_value_bytes(if index == 1 { second_bound } else { 4 })
    })
}
#[test]
fn prepared_pair_commits_both_routes_and_waits_when_either_queue_is_full() {
    let mut scheduler = FixedScheduler::<_, _, _, 2, 2, PORTS, 2, 2, 2>::new(
        [
            node([None; PORTS]),
            node([Some(CordId(0)), Some(CordId(1))]),
        ],
        cords(4),
        routes(),
        [
            Pair {
                source: true,
                committed: 0,
            },
            Pair {
                source: false,
                committed: 0,
            },
        ],
        FixedValueStore::<2, 8>::new(8).unwrap(),
        FixedSignLog::<32>::new(32 * core::mem::size_of::<crate::KernelEvent>() as u32).unwrap(),
    )
    .unwrap();
    scheduler.step().unwrap();
    assert_eq!(scheduler.drivers[0].committed, 1);
    for index in 0..2 {
        assert_eq!(scheduler.cord_usage(CordId(index)).unwrap(), (1, 4));
    }
    assert_eq!(scheduler.values.used_items(), 2);
    for (index, expected) in [b"abcd", b"wxyz"].into_iter().enumerate() {
        let value = scheduler.queue_slots[index].unwrap();
        assert_eq!(scheduler.values.get(value).unwrap(), expected);
        assert_eq!(scheduler.values.reference_count(value).unwrap(), 1);
    }
    scheduler.step().unwrap();
    scheduler.step().unwrap();
    assert_eq!(scheduler.drivers[0].committed, 1);
    assert_eq!(scheduler.values.used_items(), 2);
}
#[test]
fn partial_storage_failure_and_late_semantic_failure_leave_both_routes_empty() {
    for (second_bound, byte_capacity) in [(3, 9), (3, 12), (4, 9), (4, 12)] {
        let mut values = FixedValueStore::<2, 12>::new(byte_capacity).unwrap();
        let sentinel = values.store(b"held").unwrap();
        let mut scheduler = FixedScheduler::<_, _, _, 2, 2, PORTS, 2, 2, 2>::new(
            [
                node([None; PORTS]),
                node([Some(CordId(0)), Some(CordId(1))]),
            ],
            cords(second_bound),
            routes(),
            [
                Pair {
                    source: true,
                    committed: 0,
                },
                Pair {
                    source: false,
                    committed: 0,
                },
            ],
            values,
            FixedSignLog::<32>::new(32 * core::mem::size_of::<crate::KernelEvent>() as u32)
                .unwrap(),
        )
        .unwrap();
        let failure = scheduler.step().unwrap_err();
        assert_eq!(
            failure,
            if second_bound == 3 {
                SchedulerError::SemanticValueBoundExceeded
            } else if byte_capacity == 9 {
                SchedulerError::Storage(crate::StorageError::ByteCapacityExceeded)
            } else {
                SchedulerError::Storage(crate::StorageError::ItemCapacityExceeded)
            }
        );
        assert_eq!(scheduler.drivers[0].committed, 0);
        assert_eq!(scheduler.values.used_items(), 1);
        assert_eq!(scheduler.values.used_bytes(), 4);
        assert_eq!(scheduler.values.get(sentinel).unwrap(), b"held");
        assert_eq!(scheduler.values.reference_count(sentinel).unwrap(), 1);
        for index in 0..2 {
            assert_eq!(scheduler.cord_usage(CordId(index)).unwrap(), (0, 0));
        }
    }
}
#[test]
fn prepared_pair_is_rolled_back_when_mandatory_signs_cannot_commit() {
    let mut scheduler = FixedScheduler::<_, _, _, 2, 2, PORTS, 2, 2, 2>::new(
        [
            node([None; PORTS]),
            node([Some(CordId(0)), Some(CordId(1))]),
        ],
        cords(4),
        routes(),
        [
            Pair {
                source: true,
                committed: 0,
            },
            Pair {
                source: false,
                committed: 0,
            },
        ],
        FixedValueStore::<2, 8>::new(8).unwrap(),
        FixedSignLog::<1>::new(core::mem::size_of::<crate::KernelEvent>() as u32).unwrap(),
    )
    .unwrap();
    assert!(matches!(scheduler.step(), Err(SchedulerError::Sign(_))));
    assert_eq!(scheduler.drivers[0].committed, 0);
    assert_eq!(scheduler.values.used_items(), 0);
    assert_eq!(scheduler.values.used_bytes(), 0);
    for index in 0..2 {
        assert_eq!(scheduler.cord_usage(CordId(index)).unwrap(), (0, 0));
    }
}
#[test]
fn prepared_ports_reject_duplicates_and_conflicting_canonical_emissions() {
    let frame = || StepIo::test_frame([None; PORTS], [false; PORTS], [Some(8); PORTS], None, 8);
    let mut io = frame();
    io.send_prepared(PortId(0), 4).unwrap();
    assert!(io.send_prepared(PortId(0), 4).is_err());
    let mut io = frame();
    io.send_prepared(PortId(0), 4).unwrap();
    assert!(io
        .send_canonical(PortId(1), CanonicalValue::new(b"data").unwrap())
        .is_err());
    let mut io = frame();
    io.send_canonical(PortId(0), CanonicalValue::new(b"data").unwrap())
        .unwrap();
    assert!(io.send_prepared(PortId(1), 4).is_err());
}
