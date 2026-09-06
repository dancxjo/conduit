use conduit_human::{KeyEvent, KEY_EVENT_CONFORMANCE_VECTORS};
use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerError, StepInputBytes, StepIo,
    StepOperation, StepOutcome,
};
use conduit_kernel::{
    CordId, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId, PortId, RouteRange,
    RouteTarget, ValueRef, ValueStorage,
};

const PORTS: usize = 1;
const EVENTS: usize = 64;

type KeyboardScheduler =
    FixedScheduler<Driver, FixedValueStore<8, 3>, FixedSignLog<EVENTS>, 2, 1, PORTS, 1, 2, 1>;

#[derive(Clone, Copy)]
enum Role {
    Source {
        values: [Option<ValueRef>; 8],
        next: usize,
        pressure_waits: u16,
        fail: bool,
    },
    Sink {
        seen: [Option<KeyEvent>; 8],
        count: usize,
        consumes: bool,
    },
}

#[derive(Clone, Copy)]
struct Driver {
    role: Role,
    cancelled: bool,
}

impl StepOperation<PORTS> for Driver {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match &mut self.role {
            Role::Source {
                values,
                next,
                pressure_waits,
                fail,
            } => {
                if *fail {
                    return StepOutcome::Fail(conduit_kernel::Failure {
                        code: conduit_kernel::FailureCode::HostOperationFailed,
                        detail: 0x4b44,
                    });
                }
                let Some(value) = values.get(*next).copied().flatten() else {
                    return StepOutcome::Complete;
                };
                if !io.output_ready(PortId(0)) {
                    *pressure_waits += 1;
                    return StepOutcome::Await;
                }
                io.send(PortId(0), value).expect("key output admitted");
                *next += 1;
                StepOutcome::Progress
            }
            Role::Sink {
                seen,
                count,
                consumes,
            } => {
                if io.input(PortId(0)).is_some() {
                    if !*consumes {
                        return StepOutcome::Await;
                    }
                    let bytes = input_bytes
                        .input(PortId(0))
                        .expect("present key value has bytes");
                    seen[*count] = Some(KeyEvent::decode(bytes).expect("portable vector decodes"));
                    *count += 1;
                    io.consume(PortId(0)).expect("key value consumed");
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0))
                        .expect("key flow closure consumed");
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
        }
    }

    fn cancel(&mut self) {
        self.cancelled = true;
    }
}

fn scheduler(fail: bool, sink_consumes: bool) -> KeyboardScheduler {
    let mut store = FixedValueStore::new(24).expect("eight exact key values admitted");
    let mut values = [None; 8];
    for (index, vector) in KEY_EVENT_CONFORMANCE_VECTORS.iter().enumerate() {
        values[index] = Some(
            store
                .store(&vector.encoded)
                .expect("vector storage admitted"),
        );
    }
    let mut routes = FixedRoutes::new(PORTS as u16);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: conduit_kernel::CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let sign_bytes = u32::try_from(EVENTS * core::mem::size_of::<KernelEvent>()).unwrap();
    KeyboardScheduler::new(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_work: 1,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_work: 1,
            },
        ],
        [CordSpec::local(
            CordId(0),
            (NodeId(0), PortId(0)),
            (NodeId(1), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 3,
            },
        )],
        routes,
        [
            Driver {
                role: Role::Source {
                    values,
                    next: 0,
                    pressure_waits: 0,
                    fail,
                },
                cancelled: false,
            },
            Driver {
                role: Role::Sink {
                    seen: [None; 8],
                    count: 0,
                    consumes: sink_consumes,
                },
                cancelled: false,
            },
        ],
        store,
        FixedSignLog::new(sign_bytes).unwrap(),
    )
    .unwrap()
}

#[test]
fn portable_vectors_cross_capacity_one_cord_and_close_exactly() {
    let mut scheduler = scheduler(false, true);
    scheduler.run(128).expect("bounded key source completes");
    let Role::Source { pressure_waits, .. } = scheduler.drivers()[0].role else {
        unreachable!()
    };
    let Role::Sink { seen, count, .. } = scheduler.drivers()[1].role else {
        unreachable!()
    };
    assert_eq!(pressure_waits, 0);
    assert_eq!(count, 8);
    for (actual, expected) in seen.into_iter().zip(KEY_EVENT_CONFORMANCE_VECTORS) {
        assert_eq!(actual.unwrap().encode(), expected.encoded);
    }
}

#[test]
fn full_key_cord_waits_under_pressure_without_dropping_or_overwriting() {
    let mut scheduler = scheduler(false, false);
    scheduler.step().expect("first key is admitted");
    scheduler.step().expect("blocked sink awaits");
    scheduler.step().expect("source waits on the full Cord");
    let Role::Source {
        next,
        pressure_waits,
        ..
    } = scheduler.drivers()[0].role
    else {
        unreachable!()
    };
    assert_eq!(next, 1);
    assert_eq!(pressure_waits, 1);
    assert_eq!(scheduler.cord_usage(CordId(0)), Ok((1, 3)));
}

#[test]
fn cancellation_and_host_input_failure_remain_distinct() {
    let mut cancelled = scheduler(false, true);
    cancelled
        .cancel()
        .expect("source Play cancellation is admitted");
    assert!(cancelled.drivers().iter().all(|driver| driver.cancelled));

    let mut failed = scheduler(true, true);
    assert_eq!(
        failed.step(),
        Err(SchedulerError::OperationFailed(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::HostOperationFailed,
            detail: 0x4b44
        }))
    );
    assert!(!failed.drivers()[0].cancelled);
}
