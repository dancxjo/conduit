use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, OperationDriver, SchedulerError,
    SchedulerStatus,
};
use conduit_kernel::{
    CordEndpoint, CordId, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId,
    Operation, OperationAction, OperationInput, PortId, RemoteEndpointId, RouteRange, RouteTarget,
    ValueRef, ValueStorage,
};

const PORTS: usize = 2;
const ENDPOINT: RemoteEndpointId = RemoteEndpointId(7);

struct ReleaseOperation {
    action: OperationAction,
    releases: [Option<ValueRef>; 3],
    next_release: usize,
}

impl Operation for ReleaseOperation {
    fn start(&mut self) -> OperationAction {
        self.action
    }

    fn resume(&mut self, _input: OperationInput) -> OperationAction {
        OperationAction::Await
    }

    fn take_released_value(&mut self) -> Option<ValueRef> {
        let value = self.releases.get(self.next_release).copied().flatten();
        if value.is_some() {
            self.next_release += 1;
        }
        value
    }
}

enum PressureOperation {
    Source {
        trigger: ValueRef,
    },
    Releaser {
        filler: ValueRef,
        output: ValueRef,
        releases: [Option<ValueRef>; PORTS],
        next_release: usize,
        started: bool,
        release_ready: bool,
    },
}

impl Operation for PressureOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source { trigger } => OperationAction::Emit {
                port: PortId(0),
                value: *trigger,
            },
            Self::Releaser { filler, .. } => OperationAction::Emit {
                port: PortId(0),
                value: *filler,
            },
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Releaser {
                    output,
                    release_ready,
                    ..
                },
                OperationInput::Value {
                    port: PortId(0), ..
                },
            ) => {
                *release_ready = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: *output,
                }
            }
            _ => OperationAction::Await,
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source { .. } => OperationAction::Complete,
            Self::Releaser { started, .. } if !*started => {
                *started = true;
                OperationAction::Await
            }
            Self::Releaser { .. } => OperationAction::Await,
        }
    }

    fn take_released_value(&mut self) -> Option<ValueRef> {
        let Self::Releaser {
            releases,
            next_release,
            release_ready,
            ..
        } = self
        else {
            return None;
        };
        if !*release_ready {
            return None;
        }
        let value = releases.get(*next_release).copied().flatten();
        if value.is_some() {
            *next_release += 1;
        }
        value
    }
}

#[test]
fn two_distinct_values_release_in_one_terminal_transition() {
    let mut values = FixedValueStore::<2, 1>::new(2).unwrap();
    let first = values.store(&[1]).unwrap();
    let second = values.store(&[2]).unwrap();
    let driver = OperationDriver::new(ReleaseOperation {
        action: OperationAction::Complete,
        releases: [Some(first), Some(second), None],
        next_release: 0,
    })
    .unwrap();
    let mut scheduler = scheduler_without_cords(driver, values).unwrap();

    assert_eq!(scheduler.step(), Ok(SchedulerStatus::Complete));
    assert_eq!(scheduler.values().used_items(), 0);
}

#[test]
fn duplicate_and_stale_release_fail_before_partial_commit() {
    let mut duplicate_values = FixedValueStore::<2, 1>::new(1).unwrap();
    let duplicate = duplicate_values.store(&[1]).unwrap();
    let duplicate_driver = OperationDriver::new(ReleaseOperation {
        action: OperationAction::Complete,
        releases: [Some(duplicate), Some(duplicate), None],
        next_release: 0,
    })
    .unwrap();
    let mut duplicate_scheduler =
        scheduler_without_cords(duplicate_driver, duplicate_values).unwrap();
    assert_eq!(
        duplicate_scheduler.step(),
        Err(SchedulerError::InvalidPortAccess)
    );
    assert_eq!(duplicate_scheduler.values().used_items(), 1);

    let mut stale_values = FixedValueStore::<2, 1>::new(2).unwrap();
    let stale = stale_values.store(&[1]).unwrap();
    let live = stale_values.store(&[2]).unwrap();
    stale_values.release(stale).unwrap();
    let stale_driver = OperationDriver::new(ReleaseOperation {
        action: OperationAction::Complete,
        releases: [Some(stale), Some(live), None],
        next_release: 0,
    })
    .unwrap();
    let mut stale_scheduler = scheduler_without_cords(stale_driver, stale_values).unwrap();
    assert_eq!(
        stale_scheduler.step(),
        Err(SchedulerError::Storage(
            conduit_kernel::StorageError::StaleReference
        ))
    );
    assert_eq!(stale_scheduler.values().used_items(), 1);
}

#[test]
fn release_overflow_is_a_protocol_violation_before_scheduling() {
    let value = |slot| ValueRef {
        slot,
        generation: 1,
        byte_len: 1,
    };
    assert!(matches!(
        OperationDriver::<_, PORTS>::new(ReleaseOperation {
            action: OperationAction::Complete,
            releases: [Some(value(0)), Some(value(1)), Some(value(2))],
            next_release: 0,
        }),
        Err(SchedulerError::OperationProtocolViolation)
    ));
}

#[test]
fn blocked_output_preserves_both_releases_until_atomic_commit() {
    let mut values = FixedValueStore::<5, 1>::new(5).unwrap();
    let trigger = values.store(&[1]).unwrap();
    let filler = values.store(&[2]).unwrap();
    let output = values.store(&[3]).unwrap();
    let first_release = values.store(&[4]).unwrap();
    let second_release = values.store(&[5]).unwrap();

    let node_specs = [
        NodeSpec {
            input_cords: [None; PORTS],
            maximum_step_work: 4,
        },
        NodeSpec {
            input_cords: [Some(CordId(1)), None],
            maximum_step_work: 6,
        },
    ];
    let cord_specs = [
        CordSpec::remote_egress(
            CordId(0),
            (NodeId(1), PortId(0)),
            ENDPOINT,
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 1,
            },
        ),
        CordSpec::local(
            CordId(1),
            (NodeId(0), PortId(0)),
            (NodeId(1), PortId(0)),
            CordCapacity {
                slot_start: 1,
                item_capacity: 1,
                byte_capacity: 1,
            },
        ),
    ];
    let mut routes = FixedRoutes::<4, 2>::new(PORTS as u16);
    install_route(
        &mut routes,
        NodeId(0),
        CordId(1),
        CordEndpoint::local(NodeId(1), PortId(0)),
    );
    install_route(
        &mut routes,
        NodeId(1),
        CordId(0),
        CordEndpoint::Remote(ENDPOINT),
    );
    routes.seal().unwrap();
    let drivers = [
        OperationDriver::new(PressureOperation::Source { trigger }).unwrap(),
        OperationDriver::new(PressureOperation::Releaser {
            filler,
            output,
            releases: [Some(first_release), Some(second_release)],
            next_release: 0,
            started: false,
            release_ready: false,
        })
        .unwrap(),
    ];
    let signs = sign::<96>();
    let mut scheduler = FixedScheduler::<_, _, _, 2, 2, PORTS, 2, 4, 2>::new(
        node_specs, cord_specs, routes, drivers, values, signs,
    )
    .unwrap();

    assert!(matches!(
        scheduler.step(),
        Ok(SchedulerStatus::Progress { .. })
    ));
    assert!(matches!(
        scheduler.step(),
        Ok(SchedulerStatus::Progress { .. })
    ));
    assert!(matches!(
        scheduler.step(),
        Ok(SchedulerStatus::Progress { .. })
    ));
    assert_eq!(scheduler.values().used_items(), 5);
    assert_eq!(scheduler.step(), Ok(SchedulerStatus::Idle));
    assert_eq!(scheduler.values().used_items(), 5);
    assert_eq!(scheduler.values().reference_count(first_release), Ok(1));
    assert_eq!(scheduler.values().reference_count(second_release), Ok(1));

    let filler_offer = scheduler
        .remote_egress_offer(ENDPOINT, CordId(0))
        .unwrap()
        .unwrap();
    scheduler
        .remote_egress_accept(ENDPOINT, CordId(0), filler_offer.sequence)
        .unwrap();
    scheduler
        .remote_egress_delivered(ENDPOINT, CordId(0), filler_offer.sequence)
        .unwrap();
    assert_eq!(scheduler.values().used_items(), 4);

    assert!(matches!(
        scheduler.step(),
        Ok(SchedulerStatus::Progress { .. })
    ));
    assert_eq!(scheduler.values().used_items(), 1);
    assert_eq!(
        scheduler.values().reference_count(first_release),
        Err(conduit_kernel::StorageError::StaleReference)
    );
    assert_eq!(
        scheduler.values().reference_count(second_release),
        Err(conduit_kernel::StorageError::StaleReference)
    );
}

fn scheduler_without_cords(
    driver: OperationDriver<ReleaseOperation, PORTS>,
    values: FixedValueStore<2, 1>,
) -> Result<
    FixedScheduler<
        OperationDriver<ReleaseOperation, PORTS>,
        FixedValueStore<2, 1>,
        FixedSignLog<32>,
        1,
        1,
        PORTS,
        1,
        2,
        1,
    >,
    SchedulerError,
> {
    let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    routes.seal()?;
    FixedScheduler::new_with_active_counts(
        1,
        0,
        [NodeSpec {
            input_cords: [None; PORTS],
            maximum_step_work: 4,
        }],
        [inactive_cord()],
        routes,
        [driver],
        values,
        sign::<32>(),
    )
}

fn install_route<const ROUTES: usize, const TARGETS: usize>(
    routes: &mut FixedRoutes<ROUTES, TARGETS>,
    source: NodeId,
    cord: CordId,
    sink: CordEndpoint,
) {
    routes
        .install(
            source,
            PortId(0),
            RouteRange {
                start: cord.0,
                len: 1,
            },
            &[RouteTarget { cord, sink }],
        )
        .unwrap();
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

fn sign<const EVENTS: usize>() -> FixedSignLog<EVENTS> {
    let bytes = u32::try_from(EVENTS * core::mem::size_of::<KernelEvent>()).unwrap();
    FixedSignLog::new(bytes).unwrap()
}
