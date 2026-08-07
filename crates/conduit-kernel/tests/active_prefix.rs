use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerError, SchedulerStatus, StepIo,
    StepOperation, StepOutcome,
};
use conduit_kernel::{
    CordEndpoint, CordId, FixedEvidenceLog, FixedHostOperationBindings, FixedRoutes,
    FixedValueStore, HostOperationBinding, HostOperationId, KernelEvent, NodeId, PortId,
    ProtocolError, RouteRange, RouteTarget, ValueRef, ValueStorage,
};

const MAX_NODES: usize = 4;
const MAX_CORDS: usize = 3;
const PORTS: usize = 2;
const QUEUE_SLOTS: usize = 4;
const ROUTE_SLOTS: usize = MAX_NODES * PORTS;
const ROUTE_TARGETS: usize = MAX_CORDS;
const EVIDENCE_EVENTS: usize = 128;

type TestScheduler = FixedScheduler<
    Driver,
    FixedValueStore<4, 1>,
    FixedEvidenceLog<EVIDENCE_EVENTS>,
    MAX_NODES,
    MAX_CORDS,
    PORTS,
    QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
>;

#[derive(Clone, Copy)]
enum Role {
    Source {
        values: [Option<ValueRef>; 2],
        next: usize,
    },
    Pass,
    Sink {
        seen: u16,
    },
    Inactive,
}

#[derive(Clone, Copy)]
struct Driver {
    role: Role,
    cancelled: bool,
}

impl Driver {
    const fn new(role: Role) -> Self {
        Self {
            role,
            cancelled: false,
        }
    }
}

impl StepOperation<PORTS> for Driver {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        match &mut self.role {
            Role::Source { values, next } => {
                let Some(value) = values.get(*next).copied().flatten() else {
                    return StepOutcome::Complete;
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), value).expect("source output admitted");
                *next += 1;
                StepOutcome::Progress
            }
            Role::Pass => {
                if let Some(value) = io.input(PortId(0)) {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume(PortId(0)).expect("pass input present");
                    io.send(PortId(0), value).expect("pass output admitted");
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).expect("pass closure present");
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Role::Sink { seen } => {
                if io.input(PortId(0)).is_some() {
                    io.consume(PortId(0)).expect("sink input present");
                    *seen += 1;
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).expect("sink closure present");
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Role::Inactive => panic!("inactive capacity slot executed"),
        }
    }

    fn cancel(&mut self) {
        if matches!(self.role, Role::Inactive) {
            panic!("inactive capacity slot cancelled");
        }
        self.cancelled = true;
    }
}

fn node(input: Option<CordId>) -> NodeSpec<PORTS> {
    NodeSpec {
        input_cords: [input, None],
        maximum_step_work: 4,
    }
}

fn inactive_node() -> NodeSpec<PORTS> {
    NodeSpec {
        input_cords: [Some(CordId(u16::MAX)), None],
        maximum_step_work: 0,
    }
}

fn cord(index: u16, source: u16, sink: u16) -> CordSpec {
    CordSpec::local(
        CordId(index),
        (NodeId(source), PortId(0)),
        (NodeId(sink), PortId(0)),
        CordCapacity {
            slot_start: index,
            item_capacity: 1,
            byte_capacity: 1,
        },
    )
}

fn inactive_cord() -> CordSpec {
    CordSpec {
        cord: CordId(u16::MAX),
        source: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
        sink: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
        slot_start: u16::MAX,
        item_capacity: 0,
        byte_capacity: 0,
    }
}

fn evidence() -> FixedEvidenceLog<EVIDENCE_EVENTS> {
    let bytes = u32::try_from(EVIDENCE_EVENTS * core::mem::size_of::<KernelEvent>())
        .expect("test evidence budget fits");
    FixedEvidenceLog::new(bytes).expect("test evidence budget is exact")
}

fn values() -> (FixedValueStore<4, 1>, [Option<ValueRef>; 2]) {
    let mut values = FixedValueStore::new(4).expect("test value budget is exact");
    let first = values.store(&[1]).expect("first value admitted");
    let second = values.store(&[2]).expect("second value admitted");
    (values, [Some(first), Some(second)])
}

fn routes(edges: &[(u16, u16, u16)]) -> FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS> {
    let mut routes = FixedRoutes::new(PORTS as u16);
    for (cord, source, sink) in edges.iter().copied() {
        let target = RouteTarget {
            cord: CordId(cord),
            sink: CordEndpoint::local(NodeId(sink), PortId(0)),
        };
        routes
            .install(
                NodeId(source),
                PortId(0),
                RouteRange {
                    start: cord,
                    len: 1,
                },
                &[target],
            )
            .expect("test route installs");
    }
    routes.seal().expect("test routes seal");
    routes
}

#[test]
fn one_scheduler_capacity_runs_two_different_active_shapes() {
    let (store, source_values) = values();
    let mut pair = TestScheduler::new_with_active_counts(
        2,
        1,
        [
            node(None),
            node(Some(CordId(0))),
            inactive_node(),
            inactive_node(),
        ],
        [cord(0, 0, 1), inactive_cord(), inactive_cord()],
        routes(&[(0, 0, 1)]),
        [
            Driver::new(Role::Source {
                values: source_values,
                next: 0,
            }),
            Driver::new(Role::Sink { seen: 0 }),
            Driver::new(Role::Inactive),
            Driver::new(Role::Inactive),
        ],
        store,
        evidence(),
    )
    .expect("pair installs in larger fixed capacity");
    pair.run(32)
        .expect("pair completes under capacity-one pressure");
    assert_eq!(pair.step(), Ok(SchedulerStatus::Complete));
    assert!(pair.evidence().events().all(|event| event.node.0 < 2));
    assert!(matches!(pair.drivers()[1].role, Role::Sink { seen: 2 }));

    let (store, source_values) = values();
    let mut chain = TestScheduler::new_with_active_counts(
        3,
        2,
        [
            node(None),
            node(Some(CordId(0))),
            node(Some(CordId(1))),
            inactive_node(),
        ],
        [cord(0, 0, 1), cord(1, 1, 2), inactive_cord()],
        routes(&[(0, 0, 1), (1, 1, 2)]),
        [
            Driver::new(Role::Source {
                values: source_values,
                next: 0,
            }),
            Driver::new(Role::Pass),
            Driver::new(Role::Sink { seen: 0 }),
            Driver::new(Role::Inactive),
        ],
        store,
        evidence(),
    )
    .expect("chain installs in the same fixed capacity type");
    chain
        .run(48)
        .expect("chain completes under capacity-one pressure");
    assert!(chain.evidence().events().all(|event| event.node.0 < 3));
    assert!(matches!(chain.drivers()[2].role, Role::Sink { seen: 2 }));
}

#[test]
fn inactive_capacity_is_rejected_and_never_cancelled() {
    let (store, source_values) = values();
    let result = TestScheduler::new_with_active_counts(
        0,
        1,
        [
            node(None),
            node(Some(CordId(0))),
            inactive_node(),
            inactive_node(),
        ],
        [cord(0, 0, 1), inactive_cord(), inactive_cord()],
        routes(&[(0, 0, 1)]),
        [
            Driver::new(Role::Source {
                values: source_values,
                next: 0,
            }),
            Driver::new(Role::Sink { seen: 0 }),
            Driver::new(Role::Inactive),
            Driver::new(Role::Inactive),
        ],
        store,
        evidence(),
    );
    assert!(matches!(result, Err(SchedulerError::InvalidActiveCapacity)));

    let (store, source_values) = values();
    let mut scheduler = TestScheduler::new_with_active_counts(
        2,
        1,
        [
            node(None),
            node(Some(CordId(0))),
            inactive_node(),
            inactive_node(),
        ],
        [cord(0, 0, 1), inactive_cord(), inactive_cord()],
        routes(&[(0, 0, 1)]),
        [
            Driver::new(Role::Source {
                values: source_values,
                next: 0,
            }),
            Driver::new(Role::Sink { seen: 0 }),
            Driver::new(Role::Inactive),
            Driver::new(Role::Inactive),
        ],
        store,
        evidence(),
    )
    .expect("pair installs");
    assert_eq!(
        scheduler.cord_usage(CordId(1)),
        Err(SchedulerError::InvalidPlan)
    );
    scheduler.cancel().expect("active prefix cancels");
    assert!(scheduler.drivers()[0].cancelled);
    assert!(scheduler.drivers()[1].cancelled);
    assert!(!scheduler.drivers()[2].cancelled);
    assert!(!scheduler.drivers()[3].cancelled);
}

#[test]
fn sealed_tables_cannot_reference_inactive_nodes_or_cords() {
    let (store, source_values) = values();
    let invalid_route = TestScheduler::new_with_active_counts(
        2,
        1,
        [
            node(None),
            node(Some(CordId(0))),
            inactive_node(),
            inactive_node(),
        ],
        [cord(0, 0, 1), inactive_cord(), inactive_cord()],
        routes(&[(0, 2, 1)]),
        [
            Driver::new(Role::Source {
                values: source_values,
                next: 0,
            }),
            Driver::new(Role::Sink { seen: 0 }),
            Driver::new(Role::Inactive),
            Driver::new(Role::Inactive),
        ],
        store,
        evidence(),
    );
    assert!(matches!(
        invalid_route,
        Err(SchedulerError::Routing(ProtocolError::RouteTableInvalid))
    ));

    let (store, source_values) = values();
    let mut bindings = FixedHostOperationBindings::<4>::new(1);
    bindings
        .install(
            NodeId(2),
            HostOperationBinding {
                operation: HostOperationId(0),
                maximum_input_bytes: 1,
                maximum_output_bytes: 0,
            },
        )
        .expect("binding fits physical capacity");
    bindings.seal().expect("binding table seals");
    let invalid_binding = FixedScheduler::<
        Driver,
        _,
        _,
        MAX_NODES,
        MAX_CORDS,
        PORTS,
        QUEUE_SLOTS,
        ROUTE_SLOTS,
        ROUTE_TARGETS,
        4,
        2,
    >::new_with_active_counts_and_host_operations(
        2,
        1,
        [
            node(None),
            node(Some(CordId(0))),
            inactive_node(),
            inactive_node(),
        ],
        [cord(0, 0, 1), inactive_cord(), inactive_cord()],
        routes(&[(0, 0, 1)]),
        bindings,
        [
            Driver::new(Role::Source {
                values: source_values,
                next: 0,
            }),
            Driver::new(Role::Sink { seen: 0 }),
            Driver::new(Role::Inactive),
            Driver::new(Role::Inactive),
        ],
        store,
        evidence(),
    );
    assert!(matches!(
        invalid_binding,
        Err(SchedulerError::Routing(
            ProtocolError::HostOperationTableInvalid
        ))
    ));
}
