//! The source and byte-checking sink are fixtures; the installed pulse operation
//! and every execution decision use the production driver and fixed scheduler.
use super::*;
use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, OperationDriver, StepInputBytes, StepIo,
    StepOperation, StepOutcome,
};
use conduit_kernel::{
    BoundedValueRef, FixedHostOperationBindings, HostOperationBinding, HostOperationDisposition,
    HostOperationId, HostOperationOutcome, RequestId,
};
use conduit_kernel::{
    CordEndpoint, CordId, FixedRoutes, FixedSignLog, NodeId, RouteRange, RouteTarget,
};

enum Fixture {
    Pulse(InstalledOperation),
    Sink { next: u32, wait: ValueRef },
}
impl Operation for Fixture {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Pulse(operation) => operation.start(),
            Self::Sink { wait, .. } => OperationAction::RequestHostOperation {
                request: RequestId(0),
                operation: HostOperationId(0),
                input: BoundedValueRef::new(*wait, 1).unwrap(),
            },
        }
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::Pulse(operation) => operation.resume(input),
            Self::Sink { .. }
                if matches!(
                    input,
                    OperationInput::HostOperationCompleted {
                        request: RequestId(0),
                        ..
                    }
                ) =>
            {
                OperationAction::Await
            }
            Self::Sink { .. } if input == (OperationInput::Closed { port: PortId(0) }) => {
                OperationAction::Complete
            }
            _ => panic!("fixture received unexpected input"),
        }
    }
    fn resume_value(&mut self, port: PortId, value: ValueRef, canonical: &[u8]) -> OperationAction {
        match self {
            Self::Pulse(operation) => operation.resume_value(port, value, canonical),
            Self::Sink { next, .. } => {
                assert_eq!(port, PortId(0));
                let pulse = conduit_time::decode_pulse_observation(canonical).unwrap();
                assert_eq!((pulse.sequence, pulse.period_ms), (*next, 320));
                *next += 1;
                OperationAction::Await
            }
        }
    }
    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Pulse(operation) => operation.advance(),
            Self::Sink { .. } => OperationAction::Await,
        }
    }
    fn cancel(&mut self) {
        if let Self::Pulse(operation) = self {
            operation.cancel();
        }
    }
}

enum Driver {
    Source { values: Vec<ValueRef>, next: usize },
    Installed(Box<OperationDriver<Fixture, 1>>),
}
impl StepOperation<1> for Driver {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Installed(operation) => operation.step(io, bytes),
            Self::Source { values, next } => {
                let Some(value) = values.get(*next).copied() else {
                    return StepOutcome::Complete;
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), value).unwrap();
                *next += 1;
                StepOutcome::Progress
            }
        }
    }
}

#[test]
fn installed_pulse_stream_runs_in_production_kernel_with_capacity_one_cords() {
    let (pulse, mut values) = prepared(3);
    let ticks = (0..3)
        .map(|sequence| values.store(&conduit_time::encode_tick(sequence)).unwrap())
        .collect();
    let wait = values.store(&[0]).unwrap();
    let allocation_before = values.allocation_capacities();
    let mut routes = FixedRoutes::<3, 2>::new(1);
    for index in 0..2 {
        routes
            .install(
                NodeId(index),
                PortId(0),
                RouteRange {
                    start: index,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(index),
                    sink: CordEndpoint::local(NodeId(index + 1), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut bindings = FixedHostOperationBindings::<3>::new(1);
    bindings
        .install(
            NodeId(2),
            HostOperationBinding {
                operation: HostOperationId(0),
                maximum_input_bytes: 1,
                maximum_output_bytes: 1,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    let mut scheduler =
        FixedScheduler::<_, _, _, 3, 2, 1, 2, 3, 2, 3, 1>::new_with_host_operations(
            [None, Some(CordId(0)), Some(CordId(1))].map(|input| NodeSpec {
                input_cords: [input],
                maximum_step_work: 3,
            }),
            [0, 1].map(|index| {
                CordSpec::local(
                    CordId(index),
                    (NodeId(index), PortId(0)),
                    (NodeId(index + 1), PortId(0)),
                    CordCapacity {
                        slot_start: index,
                        item_capacity: 1,
                        byte_capacity: 8,
                    },
                )
            }),
            routes,
            bindings,
            [
                Driver::Source {
                    values: ticks,
                    next: 0,
                },
                Driver::Installed(Box::new(
                    OperationDriver::new(Fixture::Pulse(pulse)).unwrap(),
                )),
                Driver::Installed(Box::new(
                    OperationDriver::new(Fixture::Sink { next: 0, wait }).unwrap(),
                )),
            ],
            values,
            FixedSignLog::<256>::new(
                (core::mem::size_of::<conduit_kernel::KernelEvent>() * 256) as u32,
            )
            .unwrap(),
        )
        .unwrap();
    for _ in 0..32 {
        scheduler.step().unwrap();
    }
    let Driver::Installed(observer) = &scheduler.drivers()[1] else {
        panic!("pulse driver");
    };
    let Fixture::Pulse(InstalledOperation::PulseObserve(observer)) = observer.operation() else {
        panic!("installed observer");
    };
    let staged_sequence = observer.next;
    assert!(staged_sequence > 0 && staged_sequence < 3);
    for _ in 0..16 {
        scheduler.step().unwrap();
    }
    let Driver::Installed(observer) = &scheduler.drivers()[1] else {
        panic!("pulse driver");
    };
    let Fixture::Pulse(InstalledOperation::PulseObserve(observer)) = observer.operation() else {
        panic!("installed observer");
    };
    assert_eq!(
        observer.next, staged_sequence,
        "pressure cannot consume more input"
    );
    let request = scheduler.next_host_request().unwrap();
    assert_eq!(request.node, NodeId(2));
    scheduler
        .complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .unwrap();
    scheduler.run(128).unwrap();
    let Driver::Installed(sink) = &scheduler.drivers()[2] else {
        panic!("sink driver");
    };
    assert!(matches!(sink.operation(), Fixture::Sink { next: 3, .. }));
    assert_eq!(
        scheduler.values().allocation_capacities(),
        allocation_before
    );
}
