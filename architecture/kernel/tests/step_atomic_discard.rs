//! Atomic bounded Step disposal under pressure and invalid identities.

use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerError, SchedulerStatus, StepBack,
    StepInputBytes, StepIo, StepOutcome,
};
use conduit_kernel::{
    CordEndpoint, CordId, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId, PortId,
    RemoteEndpointId, RouteRange, RouteTarget, ValueRef, ValueStorage,
};

const PORTS: usize = 2;
const ENDPOINT: RemoteEndpointId = RemoteEndpointId(7);

struct ReleaseBack {
    releases: [Option<ValueRef>; 3],
}

impl StepBack<PORTS> for ReleaseBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        for value in self.releases.into_iter().flatten() {
            if io.discard(value).is_err() {
                // StepIo retains the exact machine-readable protocol/storage fault.
                return StepOutcome::Await;
            }
        }
        StepOutcome::Complete
    }
}

enum PressureBack {
    Source {
        trigger: ValueRef,
        sent: bool,
    },
    Releaser {
        filler: ValueRef,
        output: ValueRef,
        releases: [ValueRef; PORTS],
        started: bool,
    },
}

impl StepBack<PORTS> for PressureBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source {
                trigger,
                sent: false,
            } if io.output_ready(PortId(0)) => {
                io.send(PortId(0), *trigger).unwrap();
                if let Self::Source { sent, .. } = self {
                    *sent = true;
                }
                StepOutcome::Progress
            }
            Self::Source { sent: true, .. } => StepOutcome::Complete,
            Self::Source { .. } => StepOutcome::Await,
            Self::Releaser {
                filler,
                started: false,
                ..
            } if io.output_ready(PortId(0)) => {
                io.send(PortId(0), *filler).unwrap();
                if let Self::Releaser { started, .. } = self {
                    *started = true;
                }
                StepOutcome::Progress
            }
            Self::Releaser {
                output,
                releases,
                started: true,
                ..
            } if io.input(PortId(0)).is_some() && io.output_ready(PortId(0)) => {
                io.consume(PortId(0)).unwrap();
                io.send(PortId(0), *output).unwrap();
                for value in *releases {
                    io.discard(value).unwrap();
                }
                StepOutcome::Progress
            }
            Self::Releaser { .. } => StepOutcome::Await,
        }
    }
}

#[test]
fn two_distinct_values_discard_in_one_terminal_step() {
    let mut values = FixedValueStore::<2, 1>::new(2).unwrap();
    let first = values.store(&[1]).unwrap();
    let second = values.store(&[2]).unwrap();
    let mut scheduler = scheduler_without_cords(
        ReleaseBack {
            releases: [Some(first), Some(second), None],
        },
        values,
    )
    .unwrap();
    assert_eq!(scheduler.step(), Ok(SchedulerStatus::Drained));
    assert_eq!(scheduler.values().used_items(), 0);
}

#[test]
fn duplicate_and_stale_discard_fail_before_partial_commit() {
    let mut duplicate_values = FixedValueStore::<2, 1>::new(1).unwrap();
    let duplicate = duplicate_values.store(&[1]).unwrap();
    let mut duplicate_scheduler = scheduler_without_cords(
        ReleaseBack {
            releases: [Some(duplicate), Some(duplicate), None],
        },
        duplicate_values,
    )
    .unwrap();
    assert_eq!(
        duplicate_scheduler.step(),
        Err(SchedulerError::InvalidPortAccess)
    );
    assert_eq!(duplicate_scheduler.values().used_items(), 1);

    let mut stale_values = FixedValueStore::<2, 1>::new(2).unwrap();
    let stale = stale_values.store(&[1]).unwrap();
    let live = stale_values.store(&[2]).unwrap();
    stale_values.release(stale).unwrap();
    let mut stale_scheduler = scheduler_without_cords(
        ReleaseBack {
            releases: [Some(stale), Some(live), None],
        },
        stale_values,
    )
    .unwrap();
    assert_eq!(
        stale_scheduler.step(),
        Err(SchedulerError::Storage(
            conduit_kernel::StorageError::StaleReference
        ))
    );
    assert_eq!(stale_scheduler.values().used_items(), 1);
}

#[test]
fn discard_overflow_is_a_step_protocol_refusal() {
    let value = |slot| ValueRef {
        slot,
        generation: 1,
        byte_len: 1,
    };
    let values = FixedValueStore::<2, 1>::new(2).unwrap();
    let mut scheduler = scheduler_without_cords(
        ReleaseBack {
            releases: [Some(value(0)), Some(value(1)), Some(value(2))],
        },
        values,
    )
    .unwrap();
    assert_eq!(scheduler.step(), Err(SchedulerError::InvalidPortAccess));
}

#[test]
fn blocked_output_preserves_both_discards_until_atomic_commit() {
    let mut values = FixedValueStore::<5, 1>::new(5).unwrap();
    let trigger = values.store(&[1]).unwrap();
    let filler = values.store(&[2]).unwrap();
    let output = values.store(&[3]).unwrap();
    let first_release = values.store(&[4]).unwrap();
    let second_release = values.store(&[5]).unwrap();
    let node_specs = [
        NodeSpec {
            input_cords: [None; PORTS],
            maximum_step_fuel: 4,
        },
        NodeSpec {
            input_cords: [Some(CordId(1)), None],
            maximum_step_fuel: 6,
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
                pressure_policy: Default::default(),
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
                pressure_policy: Default::default(),
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
        PressureBack::Source {
            trigger,
            sent: false,
        },
        PressureBack::Releaser {
            filler,
            output,
            releases: [first_release, second_release],
            started: false,
        },
    ];
    let mut scheduler = FixedScheduler::<_, _, _, 2, 2, PORTS, 2, 4, 2>::new(
        node_specs,
        cord_specs,
        routes,
        drivers,
        values,
        sign::<96>(),
    )
    .unwrap();

    let mut reached_pressure = false;
    for _ in 0..8 {
        match scheduler.step() {
            Ok(SchedulerStatus::Progress { .. }) => {}
            Ok(SchedulerStatus::Idle) => {
                reached_pressure = true;
                break;
            }
            outcome => panic!("unexpected pre-pressure outcome: {outcome:?}"),
        }
    }
    assert!(reached_pressure);
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
    driver: ReleaseBack,
    values: FixedValueStore<2, 1>,
) -> Result<
    FixedScheduler<ReleaseBack, FixedValueStore<2, 1>, FixedSignLog<32>, 1, 1, PORTS, 1, 2, 1>,
    SchedulerError,
> {
    let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    routes.seal()?;
    FixedScheduler::new_with_active_counts(
        1,
        0,
        [NodeSpec {
            input_cords: [None; PORTS],
            maximum_step_fuel: 4,
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
        pressure_policy: Default::default(),
    }
}

fn sign<const EVENTS: usize>() -> FixedSignLog<EVENTS> {
    let bytes = u32::try_from(EVENTS * core::mem::size_of::<KernelEvent>()).unwrap();
    let remote_bytes =
        conduit_kernel::remote_sign_storage_bytes(u16::try_from(EVENTS).unwrap()).unwrap();
    FixedSignLog::new_with_remote_storage(bytes, u16::try_from(EVENTS).unwrap(), remote_bytes)
        .unwrap()
}
