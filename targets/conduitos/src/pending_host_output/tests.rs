extern crate std;
use std::{vec, vec::Vec};
use super::{PendingHostOutput, PendingOutputRefusal};
use conduit_kernel::scheduler::*;
use conduit_kernel::*;
const PORTS: usize = 2;
enum Driver {
    Source { values: Vec<ValueRef>, next: usize },
    Effect { next: u32 },
    Sink { seen: Vec<Vec<u8>> },
}
impl StepBack<PORTS> for Driver {
    fn step(&mut self, io: &mut StepIo<PORTS>, input: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source { values, next } => {
                if *next == values.len() {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), values[*next]).unwrap();
                *next += 1;
                StepOutcome::Progress
            }
            Self::Effect { next } => {
                if let Some((_, outcome)) = io.host_completion() {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    let v = outcome.output.unwrap().value;
                    io.consume_host_completion().unwrap();
                    io.send(PortId(0), v).unwrap();
                    return StepOutcome::Progress;
                }
                if let Some(v) = io.input(PortId(0)) {
                    io.consume(PortId(0)).unwrap();
                    io.request_host_call(
                        RequestId(*next),
                        HostCallId(0),
                        BoundedValueRef::new(v, 1).unwrap(),
                    )
                    .unwrap();
                    *next += 1;
                    return StepOutcome::Progress;
                }
                if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::Sink { seen } => {
                if io.input(PortId(0)).is_some() {
                    seen.push(input.input(PortId(0)).unwrap().to_vec());
                    io.consume(PortId(0)).unwrap();
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
fn run(pressure: bool) -> (Vec<Vec<u8>>, u32, usize) {
    let mut values = FixedValueStore::<8, 8>::new(64).unwrap();
    let originals = vec![values.store(&[3]).unwrap(), values.store(&[4]).unwrap()];
    let mut routes = FixedRoutes::<6, 2>::new(2);
    for (n, c, s) in [(0, 0, 1), (1, 1, 2)] {
        routes
            .install(
                NodeId(n),
                PortId(0),
                RouteRange { start: c, len: 1 },
                &[RouteTarget {
                    cord: CordId(c),
                    sink: CordEndpoint::local(NodeId(s), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::<3>::new(1);
    bindings
        .install(
            NodeId(1),
            HostCallBinding {
                call: HostCallId(0),
                maximum_input_bytes: 1,
                maximum_output_bytes: 1,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    let nodes = [None, Some(CordId(0)), Some(CordId(1))].map(|x| NodeSpec {
        input_cords: [x, None],
        maximum_step_fuel: 3,
    });
    let cords = [(0, 0, 1), (1, 1, 2)].map(|(c, n, s)| {
        CordSpec::local(
            CordId(c),
            (NodeId(n), PortId(0)),
            (NodeId(s), PortId(0)),
            CordCapacity {
                slot_start: c,
                item_capacity: 1,
                byte_capacity: 8,
                pressure_policy: Default::default(),
            },
        )
    });
    let signs =
        FixedSignLog::<256>::new((256 * std::mem::size_of::<KernelEvent>()) as u32).unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 3, 2, 2, 2, 6, 2, 3, 1>::new_with_host_calls(
        nodes,
        cords,
        routes,
        bindings,
        [
            Driver::Source {
                values: originals,
                next: 0,
            },
            Driver::Effect { next: 0 },
            Driver::Sink { seen: vec![] },
        ],
        values,
        signs,
    )
    .unwrap();
    let mut retained = PendingHostOutput::<8>::new();
    let mut invocations = 0;
    for _ in 0..64 {
        if matches!(scheduler.step().unwrap(), SchedulerStatus::Drained) {
            break;
        }
        while let Some(call) = scheduler.next_host_request() {
            let input = scheduler.host_value(call.input.value).unwrap();
            let output = [input[0] + 1];
            invocations += 1;
            retained.retain(call.node, call.request, &output).unwrap();
            if pressure {
                let before = scheduler.decisions();
                let mut leases = vec![];
                while scheduler.values().used_items() < scheduler.values().item_capacity() {
                    leases.push(scheduler.store_host_value(&[]).unwrap());
                }
                assert!(
                    retained
                        .try_store(|b| scheduler.store_host_value(b))
                        .is_err()
                );
                assert_eq!(scheduler.decisions(), before);
                assert_eq!(scheduler.pending_host_call_count(), 1);
                assert_eq!(retained.original().unwrap().2, &output);
                for lease in leases {
                    scheduler.discard_host_value(lease).unwrap();
                }
            }
            let first = retained
                .try_store(|b| scheduler.store_host_value(b))
                .unwrap();
            assert_eq!(
                retained
                    .try_store(|_| -> Result<ValueRef, ()> { panic!("must not store twice") })
                    .unwrap(),
                first
            );
            retained
                .try_complete(|node, request, value, len| {
                    scheduler.complete_host_call(
                        node,
                        request,
                        HostCallOutcome {
                            disposition: HostCallDisposition::Completed,
                            output: Some(BoundedValueRef::new(value, len).unwrap()),
                            failure: None,
                        },
                    )
                })
                .unwrap();
        }
    }
    assert_eq!(scheduler.pending_host_call_count(), 0);
    assert_eq!(scheduler.values().used_items(), 0);
    let Driver::Sink { seen } = &scheduler.drivers()[2] else {
        panic!()
    };
    (seen.clone(), scheduler.decisions(), invocations)
}
#[test]
fn actual_scheduler_two_epoch_backpressure_resume_equivalence() {
    let uninterrupted = run(false);
    let resumed = run(true);
    assert_eq!(uninterrupted, resumed);
    assert_eq!(resumed.0, vec![vec![4], vec![5]]);
    assert_eq!(resumed.2, 2);
}

#[test]
fn original_output_survives_refusals_and_explicit_retirement() {
    let mut retained = PendingHostOutput::<2>::new();
    assert_eq!(
        retained.retain(NodeId(1), RequestId(1), &[1, 2, 3]),
        Err(PendingOutputRefusal::Capacity)
    );
    retained.retain(NodeId(1), RequestId(1), &[1, 2]).unwrap();
    assert_eq!(
        retained.retain(NodeId(2), RequestId(2), &[]),
        Err(PendingOutputRefusal::Occupied)
    );
    let value = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 2,
    };
    retained.try_store(|_| Ok::<_, ()>(value)).unwrap();
    assert_eq!(
        retained.try_complete(|_, _, _, _| Err("actual completion refusal")),
        Err("actual completion refusal")
    );
    assert_eq!(
        retained.original().unwrap(),
        (NodeId(1), RequestId(1), &[1, 2][..])
    );
    assert_eq!(
        retained.retire_cancelled(|_| Err("lease still owned")),
        Err("lease still owned")
    );
    assert!(retained.original().is_some());
    retained
        .retire_cancelled(|v| {
            assert_eq!(v, value);
            Ok::<_, ()>(())
        })
        .unwrap();
    assert!(retained.original().is_none());
}
