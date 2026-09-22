//! The source and byte-checking sink are fixtures; the installed pulse Back and
//! every execution decision use the production Step protocol and fixed scheduler.
use super::*;
use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, StepBack, StepInputBytes, StepIo, StepOutcome,
};
use conduit_kernel::{
    BoundedValueRef, FixedHostCallBindings, HostCallBinding, HostCallDisposition, HostCallId,
    HostCallOutcome, RequestId,
};
use conduit_kernel::{
    CordEndpoint, CordId, FixedRoutes, FixedSignLog, NodeId, RouteRange, RouteTarget,
};

enum Driver {
    Source {
        values: Vec<ValueRef>,
        next: usize,
    },
    Pulse(InstalledBack),
    Sink {
        next: u32,
        wait: ValueRef,
        pending: bool,
    },
}
impl StepBack<1> for Driver {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Pulse(operation) => operation.step(io, bytes),
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
            Self::Sink {
                next,
                wait,
                pending,
            } => {
                if *pending {
                    if io.host_completion().is_none() {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion().unwrap();
                    *pending = false;
                    *next = 1;
                    return StepOutcome::Progress;
                }
                if *next == 0 {
                    io.request_host_call(
                        RequestId(0),
                        HostCallId(0),
                        BoundedValueRef::new(*wait, 1).unwrap(),
                    )
                    .unwrap();
                    *pending = true;
                    return StepOutcome::Progress;
                }
                if io.input(PortId(0)).is_some() {
                    let canonical = bytes.input(PortId(0)).unwrap();
                    let pulse = conduit_time::decode_pulse_observation(canonical).unwrap();
                    assert_eq!((pulse.sequence, pulse.period_ms), (*next - 1, 320));
                    io.consume(PortId(0)).unwrap();
                    *next += 1;
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
        }
    }
}

#[test]
fn installed_pulse_stream_runs_in_production_kernel_with_capacity_one_cords() {
    let (pulse, mut values) = prepared();
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
    let mut bindings = FixedHostCallBindings::<3>::new(1);
    bindings
        .install(
            NodeId(2),
            HostCallBinding {
                call: HostCallId(0),
                maximum_input_bytes: 1,
                maximum_output_bytes: 1,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 3, 2, 1, 2, 3, 2, 3, 1>::new_with_host_calls(
        [None, Some(CordId(0)), Some(CordId(1))].map(|input| NodeSpec {
            input_cords: [input],
            maximum_step_fuel: 3,
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
                    pressure_policy: Default::default(),
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
            Driver::Pulse(InstalledBack::PulseObserve(pulse)),
            Driver::Sink {
                next: 0,
                wait,
                pending: false,
            },
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
    let Driver::Pulse(InstalledBack::PulseObserve(observer)) = &scheduler.drivers()[1] else {
        panic!("pulse driver");
    };
    let staged_sequence = observer.next_sequence();
    assert!(staged_sequence > 0 && staged_sequence < 3);
    for _ in 0..16 {
        scheduler.step().unwrap();
    }
    let Driver::Pulse(InstalledBack::PulseObserve(observer)) = &scheduler.drivers()[1] else {
        panic!("pulse driver");
    };
    assert_eq!(
        observer.next_sequence(),
        staged_sequence,
        "pressure cannot consume more input"
    );
    let request = scheduler.next_host_request().unwrap();
    assert_eq!(request.node, NodeId(2));
    scheduler
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .unwrap();
    scheduler.run(128).unwrap();
    let Driver::Sink { next: 4, .. } = &scheduler.drivers()[2] else {
        panic!("sink driver");
    };
    assert_eq!(
        scheduler.values().allocation_capacities(),
        allocation_before
    );
}
