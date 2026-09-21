use super::{
    AssignedPressurePolicy, CordCapacity, CordSpec, FixedScheduler, NodeSpec, RemoteIngressOutcome,
    SchedulerError, SchedulerStatus, StepInputBytes, StepIo, StepOperation, StepOutcome,
};
use crate::{
    BoundedValueRef, CanonicalValue, CordId, Failure, FailureCode, FixedHostCallBindings,
    FixedRoutes, FixedSignLog, FixedValueStore, HostCallBinding, HostCallDisposition, HostCallId,
    HostCallOutcome, KernelEventKind, NodeId, PortId, ProtocolError, RemoteEndpointId, RequestId,
    RouteRange, RouteTarget, SignQuery, SignSink, ValueRef, ValueStorage,
};

mod host_alias;
mod host_cancellation;

const NODES: usize = 6;
const CORDS: usize = 5;
const PORTS: usize = 2;

#[derive(Clone, Copy, Debug)]
enum Driver {
    Source {
        values: [Option<ValueRef>; 4],
        next: usize,
    },
    Tee,
    Filter,
    Latest {
        held: Option<ValueRef>,
    },
    Sink {
        seen: [Option<ValueRef>; 4],
        len: usize,
        stall: bool,
    },
    BlockedSink {
        cancelled: bool,
    },
}

impl StepOperation<PORTS> for Driver {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Source { values, next } => {
                let Some(value) = values.get(*next).copied().flatten() else {
                    return StepOutcome::Complete;
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), value).unwrap();
                *next += 1;
                StepOutcome::Progress
            }
            Self::Tee => {
                if let Some(value) = io.input(PortId(0)) {
                    if !io.output_ready(PortId(0)) || !io.output_ready(PortId(1)) {
                        return StepOutcome::Await;
                    }
                    io.consume(PortId(0)).unwrap();
                    io.send(PortId(0), value).unwrap();
                    io.send(PortId(1), value).unwrap();
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::Filter => {
                if let Some(value) = io.input(PortId(0)) {
                    if value.slot % 2 == 0 && !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume(PortId(0)).unwrap();
                    if value.slot % 2 == 0 {
                        io.send(PortId(0), value).unwrap();
                    }
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::Latest { held } => {
                if let Some(value) = io.input(PortId(0)) {
                    if let Some(previous) = held.take() {
                        io.discard(previous).unwrap();
                    }
                    io.take_input(PortId(0)).unwrap();
                    *held = Some(value);
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    let Some(latest) = held.take() else {
                        io.consume_closed(PortId(0)).unwrap();
                        return StepOutcome::Complete;
                    };
                    if !io.output_ready(PortId(0)) {
                        *held = Some(latest);
                        return StepOutcome::Await;
                    }
                    io.consume_closed(PortId(0)).unwrap();
                    io.send(PortId(0), latest).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::Sink { seen, len, stall } => {
                if *stall && io.input(PortId(0)).is_some() {
                    *stall = false;
                    io.exhaust_work_budget();
                    return StepOutcome::Yield;
                }
                if let Some(value) = io.input(PortId(0)) {
                    io.consume(PortId(0)).unwrap();
                    seen[*len] = Some(value);
                    *len += 1;
                    *stall = true;
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::BlockedSink { .. } => StepOutcome::Await,
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Latest { held } => *held = None,
            Self::BlockedSink { cancelled } => *cancelled = true,
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum HostDriver {
    Source {
        value: Option<ValueRef>,
    },
    Effect {
        requested: bool,
        cancelled: bool,
        repeat_request: bool,
    },
    Sink {
        seen: Option<ValueRef>,
    },
    Transform {
        input: ValueRef,
        phase: u8,
    },
    DecodedSink {
        seen: Option<[u8; 5]>,
    },
}

impl StepOperation<PORTS> for HostDriver {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Source { value } => {
                let Some(current) = *value else {
                    return StepOutcome::Complete;
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), current).unwrap();
                *value = None;
                StepOutcome::Progress
            }
            Self::Effect { requested, .. } if !*requested => {
                let Some(input) = io.input(PortId(0)) else {
                    return StepOutcome::Await;
                };
                io.consume(PortId(0)).unwrap();
                io.request_host_call(
                    RequestId(7),
                    HostCallId(0),
                    BoundedValueRef::new(input, 4).unwrap(),
                )
                .unwrap();
                *requested = true;
                StepOutcome::Progress
            }
            Self::Effect { repeat_request, .. } => {
                let Some((request, outcome)) = io.host_completion() else {
                    return StepOutcome::Await;
                };
                assert_eq!(request, RequestId(7));
                let output = outcome.output.expect("host output").value;
                if *repeat_request {
                    io.consume_host_completion().unwrap();
                    io.request_host_call(
                        request,
                        HostCallId(0),
                        BoundedValueRef::new(output, 4).unwrap(),
                    )
                    .unwrap();
                    return StepOutcome::Progress;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.consume_host_completion().unwrap();
                io.send(PortId(0), output).unwrap();
                StepOutcome::Complete
            }
            Self::Sink { seen } => {
                if let Some(value) = io.input(PortId(0)) {
                    io.consume(PortId(0)).unwrap();
                    *seen = Some(value);
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::Transform { input, phase } => match *phase {
                0 => {
                    io.request_host_call(
                        RequestId(1),
                        HostCallId(0),
                        BoundedValueRef::new(*input, 0).unwrap(),
                    )
                    .unwrap();
                    *phase = 1;
                    StepOutcome::Progress
                }
                1 => {
                    let Some((RequestId(1), outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    assert_eq!(input_bytes.host_output(), Some([0x90, 60, 100].as_slice()));
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    assert_eq!(outcome.disposition, HostCallDisposition::Completed);
                    io.consume_host_completion().unwrap();
                    io.send_canonical(PortId(0), CanonicalValue::new(&[9, 8, 7, 6, 5]).unwrap())
                        .unwrap();
                    io.request_host_call(
                        RequestId(2),
                        HostCallId(0),
                        BoundedValueRef::new(*input, 0).unwrap(),
                    )
                    .unwrap();
                    *phase = 2;
                    StepOutcome::Progress
                }
                2 => {
                    let Some((RequestId(2), _)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    assert_eq!(input_bytes.host_output(), Some([0x80, 60, 0].as_slice()));
                    io.consume_host_completion().unwrap();
                    io.discard(*input).unwrap();
                    *phase = 3;
                    StepOutcome::Complete
                }
                _ => StepOutcome::Complete,
            },
            Self::DecodedSink { seen } => {
                if io.input(PortId(0)).is_some() {
                    *seen = Some(input_bytes.input(PortId(0)).unwrap().try_into().unwrap());
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

    fn cancel(&mut self) {
        if let Self::Effect { cancelled, .. } = self {
            *cancelled = true;
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum JoinDriver {
    Source { value: Option<ValueRef> },
    Join,
    Sink { seen: Option<ValueRef> },
}

impl StepOperation<PORTS> for JoinDriver {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Source { value } => {
                let Some(current) = *value else {
                    return StepOutcome::Complete;
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), current).unwrap();
                *value = None;
                StepOutcome::Progress
            }
            Self::Join => {
                let (Some(left), Some(_right)) = (io.input(PortId(0)), io.input(PortId(1))) else {
                    return if io.input_closed(PortId(0)) && io.input_closed(PortId(1)) {
                        io.consume_closed(PortId(0)).unwrap();
                        io.consume_closed(PortId(1)).unwrap();
                        StepOutcome::Complete
                    } else {
                        StepOutcome::Await
                    };
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.consume(PortId(0)).unwrap();
                io.consume(PortId(1)).unwrap();
                io.send(PortId(0), left).unwrap();
                StepOutcome::Progress
            }
            Self::Sink { seen } => {
                if let Some(value) = io.input(PortId(0)) {
                    io.consume(PortId(0)).unwrap();
                    *seen = Some(value);
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
fn multi_value_port_graph_handles_pressure_closure_and_uneven_consumers() {
    let event_charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let normalized = execute(
        FixedValueStore::<8, 4>::new(16).unwrap(),
        FixedSignLog::<128>::new(event_charge * 128).unwrap(),
    );
    assert_eq!(normalized.show_a_len, 2);
    assert_eq!(normalized.show_a[..2], [0, 2]);
    assert_eq!(normalized.show_b_len, 1);
    assert_eq!(normalized.show_b[0], 3);
    assert_eq!(normalized.used_items, 0);
    assert!(normalized.saw_input_closed);
}

#[test]
fn blocked_join_preserves_every_input_until_atomic_commit() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let normalized = execute_join(
        FixedValueStore::<4, 4>::new(16).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    assert_eq!(normalized.output_slot, 0);
    assert_eq!(normalized.used_items, 0);
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_and_fixed_join_rollback_vectors_match() {
    use crate::{HostedSignLog, HostedValueStore};

    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let fixed = execute_join(
        FixedValueStore::<4, 4>::new(16).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    let hosted = execute_join(
        HostedValueStore::new(4, 4, 16).unwrap(),
        HostedSignLog::new(64, charge * 64).unwrap(),
    );
    assert_eq!(fixed, hosted);
}

#[derive(Debug, Eq, PartialEq)]
struct JoinNormalized {
    output_slot: u16,
    decisions: u32,
    sign_len: u16,
    sign_bytes: u32,
    used_items: u16,
}

fn execute_join<S, E>(mut values: S, signs: E) -> JoinNormalized
where
    S: ValueStorage,
    E: SignSink,
{
    let left = values.store(&[10]).unwrap();
    let right = values.store(&[20]).unwrap();
    let mut routes = FixedRoutes::<8, 3>::new(2);
    for (source, target, sink, sink_port) in [(0, 0, 1, 0), (2, 1, 1, 1), (1, 2, 3, 0)] {
        routes
            .install(
                NodeId(source),
                PortId(0),
                RouteRange {
                    start: target,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(target),
                    sink: crate::CordEndpoint::local(NodeId(sink), PortId(sink_port)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 4, 3, 2, 3, 8, 3>::new(
        [
            node([None, None]),
            node([Some(CordId(0)), Some(CordId(1))]),
            node([None, None]),
            node([Some(CordId(2)), None]),
        ],
        [
            cord(0, 0, 0, 1, 0),
            cord(1, 2, 0, 1, 1),
            cord(2, 1, 0, 3, 0),
        ],
        routes,
        [
            JoinDriver::Source { value: Some(left) },
            JoinDriver::Join,
            JoinDriver::Source { value: Some(right) },
            JoinDriver::Sink { seen: None },
        ],
        values,
        signs,
    )
    .unwrap();
    scheduler.step().unwrap();
    assert_eq!(scheduler.cords[0].len, 1);
    scheduler.step().unwrap();
    assert_eq!(scheduler.cords[0].len, 1);
    assert_eq!(scheduler.cords[1].len, 0);
    scheduler.run(32).unwrap();
    let JoinDriver::Sink { seen: Some(seen) } = scheduler.drivers()[3] else {
        panic!("join sink");
    };
    JoinNormalized {
        output_slot: seen.slot,
        decisions: scheduler.decisions(),
        sign_len: scheduler.signs().len(),
        sign_bytes: scheduler.signs().used_bytes(),
        used_items: scheduler.values().used_items(),
    }
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_and_fixed_schedulers_have_matching_multi_value_vectors() {
    use crate::{HostedSignLog, HostedValueStore};

    let event_charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let fixed = execute(
        FixedValueStore::<8, 4>::new(16).unwrap(),
        FixedSignLog::<128>::new(event_charge * 128).unwrap(),
    );
    let hosted = execute(
        HostedValueStore::new(8, 4, 16).unwrap(),
        HostedSignLog::new(128, event_charge * 128).unwrap(),
    );
    assert_eq!(fixed, hosted);
}

#[test]
fn scheduler_admits_correlates_and_wakes_host_calls() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let normalized = execute_host_call(
        FixedValueStore::<8, 8>::new(32).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    assert_eq!(normalized.request, RequestId(7));
    assert_eq!(normalized.call, HostCallId(0));
    assert_eq!(normalized.input, [3]);
    assert_eq!(normalized.output_slot, 1);
    assert_eq!(normalized.used_items, 0);
    assert_eq!(normalized.pending, 0);
    assert!(normalized.saw_requested);
    assert!(normalized.saw_completed);
}

#[test]
fn scheduler_rejects_unbound_host_call_before_consumption_commit() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let mut scheduler = host_scheduler_with_binding_node(
        FixedValueStore::<8, 8>::new(32).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
        NodeId(0),
    );
    scheduler.step().unwrap();
    assert_eq!(scheduler.cords[0].len, 1);
    assert_eq!(
        scheduler.step(),
        Err(super::SchedulerError::Routing(
            ProtocolError::HostCallMissing
        ))
    );
    assert_eq!(scheduler.cords[0].len, 1);
    assert_eq!(scheduler.values().used_items(), 1);
    assert_eq!(scheduler.pending_host_call_count(), 0);
}

#[test]
fn scheduler_never_reuses_a_retired_request_identity() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let mut scheduler = host_scheduler(
        FixedValueStore::<8, 8>::new(32).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    scheduler.step().unwrap();
    scheduler.step().unwrap();
    let request = scheduler.next_host_request().unwrap();
    let output = scheduler.store_host_value(&[4]).unwrap();
    scheduler
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(output, 4).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    let HostDriver::Effect { repeat_request, .. } = &mut scheduler.drivers[1] else {
        panic!("effect driver");
    };
    *repeat_request = true;
    scheduler.step().unwrap();
    scheduler.step().unwrap();
    assert_eq!(
        scheduler.step(),
        Err(super::SchedulerError::HostCallRequestDuplicate)
    );
    assert_eq!(scheduler.pending_host_call_count(), 1);
    assert_eq!(scheduler.values().used_items(), 2);
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_and_fixed_host_call_vectors_match() {
    use crate::{HostedSignLog, HostedValueStore};

    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let fixed = execute_host_call(
        FixedValueStore::<8, 8>::new(32).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    let hosted = execute_host_call(
        HostedValueStore::new(8, 8, 32).unwrap(),
        HostedSignLog::new(64, charge * 64).unwrap(),
    );
    assert_eq!(fixed, hosted);
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_executor_keeps_allocation_shape_after_play_start() {
    use crate::{HostedSignLog, HostedValueStore};

    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let values = HostedValueStore::new(8, 8, 32).unwrap();
    let value_shape = values.allocation_capacities();
    let signs = HostedSignLog::new(64, charge * 64).unwrap();
    let sign_shape = signs.allocation_capacity();
    let mut scheduler = host_scheduler(values, signs);
    assert_eq!(scheduler.values.allocation_capacities(), value_shape);
    assert_eq!(scheduler.signs.allocation_capacity(), sign_shape);
    scheduler.step().unwrap();
    scheduler.step().unwrap();
    let request = scheduler.next_host_request().unwrap();
    let output = scheduler.store_host_value(&[4]).unwrap();
    scheduler
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(output, 4).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    scheduler.run(32).unwrap();
    assert_eq!(scheduler.values.allocation_capacities(), value_shape);
    assert_eq!(scheduler.signs.allocation_capacity(), sign_shape);
    assert_eq!(scheduler.values.used_items(), 0);
}

#[test]
fn cancellation_rejects_late_host_completion_and_releases_pending_input() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let mut scheduler = host_scheduler(
        FixedValueStore::<8, 8>::new(32).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    scheduler.step().unwrap();
    scheduler.step().unwrap();
    let request = scheduler.next_host_request().unwrap();
    assert_eq!(request.request, RequestId(7));
    scheduler.cancel().unwrap();
    assert_eq!(scheduler.pending_host_call_count(), 0);
    assert_eq!(scheduler.values().used_items(), 0);
    assert_eq!(
        scheduler.complete_host_call(
            NodeId(1),
            RequestId(7),
            HostCallOutcome {
                disposition: HostCallDisposition::Cancelled,
                output: None,
                failure: Some(Failure {
                    code: FailureCode::Cancelled,
                    detail: 0,
                }),
            },
        ),
        Err(super::SchedulerError::HostCallCompletionRejected)
    );
    assert_eq!(scheduler.step().unwrap(), SchedulerStatus::Cancelled);
    let HostDriver::Effect { cancelled, .. } = scheduler.drivers()[1] else {
        panic!("effect driver");
    };
    assert!(cancelled);
    assert!(scheduler
        .signs()
        .contains_kind(KernelEventKind::RunCancelled));
}

#[derive(Debug, Eq, PartialEq)]
struct HostNormalized {
    request: RequestId,
    call: HostCallId,
    input: [u8; 1],
    output_slot: u16,
    decisions: u32,
    sign_len: u16,
    sign_bytes: u32,
    used_items: u16,
    pending: usize,
    saw_requested: bool,
    saw_completed: bool,
}

fn execute_host_call<S, E>(values: S, signs: E) -> HostNormalized
where
    S: ValueStorage,
    E: SignSink + SignQuery,
{
    let mut scheduler = host_scheduler(values, signs);
    assert!(matches!(
        scheduler.step().unwrap(),
        SchedulerStatus::Progress { node: NodeId(0) }
    ));
    assert!(matches!(
        scheduler.step().unwrap(),
        SchedulerStatus::Progress { node: NodeId(1) }
    ));
    assert_eq!(scheduler.pending_host_call_count(), 1);
    assert_eq!(
        scheduler.complete_host_call(
            NodeId(1),
            RequestId(7),
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        ),
        Err(super::SchedulerError::HostCallCompletionRejected)
    );
    let request = scheduler.next_host_request().unwrap();
    let mut input = [0];
    input.copy_from_slice(scheduler.host_value(request.input.value).unwrap());
    assert_eq!(
        scheduler.complete_host_call(
            NodeId(0),
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        ),
        Err(super::SchedulerError::HostCallCompletionRejected)
    );
    assert_eq!(
        scheduler.complete_host_call(
            NodeId(1),
            RequestId(8),
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        ),
        Err(super::SchedulerError::HostCallCompletionRejected)
    );
    let oversized = scheduler.store_host_value(&[0, 1, 2, 3, 4]).unwrap();
    assert_eq!(
        scheduler.complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(oversized, 5).unwrap()),
                failure: None,
            },
        ),
        Err(super::SchedulerError::HostCallOutputExceeded)
    );
    scheduler.discard_host_value(oversized).unwrap();
    let output = scheduler.store_host_value(&[4]).unwrap();
    scheduler
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(output, 4).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    assert_eq!(
        scheduler.complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        ),
        Err(super::SchedulerError::HostCallCompletionRejected)
    );
    scheduler.run(32).unwrap();
    let HostDriver::Sink { seen: Some(seen) } = scheduler.drivers()[2] else {
        panic!("host sink");
    };
    HostNormalized {
        request: request.request,
        call: request.call,
        input,
        output_slot: seen.slot,
        decisions: scheduler.decisions(),
        sign_len: scheduler.signs().len(),
        sign_bytes: scheduler.signs().used_bytes(),
        used_items: scheduler.values().used_items(),
        pending: scheduler.pending_host_call_count(),
        saw_requested: scheduler
            .signs()
            .contains_kind(KernelEventKind::HostCallRequested),
        saw_completed: scheduler
            .signs()
            .contains_kind(KernelEventKind::HostCallCompleted),
    }
}

fn host_scheduler<S, E>(
    values: S,
    signs: E,
) -> FixedScheduler<HostDriver, S, E, 3, 2, 2, 2, 6, 2, 3, 1>
where
    S: ValueStorage,
    E: SignSink,
{
    host_scheduler_with_binding_node(values, signs, NodeId(1))
}

#[test]
fn host_output_bytes_are_borrowed_and_derived_output_uses_admitted_storage() {
    let mut values = FixedValueStore::<4, 64>::new(128).unwrap();
    let empty = values.store(&[]).unwrap();
    let mut routes = FixedRoutes::<2, 1>::new(2);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: crate::CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::<2>::new(1);
    bindings
        .install(
            NodeId(0),
            HostCallBinding {
                call: HostCallId(0),
                maximum_input_bytes: 0,
                maximum_output_bytes: 3,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    let signs =
        FixedSignLog::<32>::new((32 * core::mem::size_of::<crate::KernelEvent>()) as u32).unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 2, 1, 2, 1, 2, 1, 2, 1>::new_with_host_calls(
        [node([None, None]), node([Some(CordId(0)), None])],
        [CordSpec::local(
            CordId(0),
            (NodeId(0), PortId(0)),
            (NodeId(1), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 8,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        bindings,
        [
            HostDriver::Transform {
                input: empty,
                phase: 0,
            },
            HostDriver::DecodedSink { seen: None },
        ],
        values,
        signs,
    )
    .unwrap();
    assert!(matches!(
        scheduler.step().unwrap(),
        SchedulerStatus::Progress { .. }
    ));
    let request = scheduler.next_host_request().unwrap();
    let observation = scheduler.store_host_value(&[0x90, 60, 100]).unwrap();
    scheduler
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(observation, 3).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    let second_request = (0..3)
        .find_map(|_| {
            let _ = scheduler.step().unwrap();
            scheduler.next_host_request()
        })
        .expect("derived output and repeated zero-byte request make bounded progress");
    assert_eq!(second_request.request, RequestId(2));
    assert_eq!(second_request.input, request.input);
    let second_observation = scheduler.store_host_value(&[0x80, 60, 0]).unwrap();
    scheduler
        .complete_host_call(
            second_request.node,
            second_request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(second_observation, 3).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    scheduler.run(16).unwrap();
    let HostDriver::DecodedSink { seen } = scheduler.drivers()[1] else {
        panic!("derived sink identity changed");
    };
    assert_eq!(seen, Some([9, 8, 7, 6, 5]));
    assert_eq!(scheduler.values().used_items(), 0);
}

fn host_scheduler_with_binding_node<S, E>(
    mut values: S,
    signs: E,
    binding_node: NodeId,
) -> FixedScheduler<HostDriver, S, E, 3, 2, 2, 2, 6, 2, 3, 1>
where
    S: ValueStorage,
    E: SignSink,
{
    let input = values.store(&[3]).unwrap();
    let mut routes = FixedRoutes::<6, 2>::new(2);
    for (node, cord, sink) in [(0, 0, 1), (1, 1, 2)] {
        routes
            .install(
                NodeId(node),
                PortId(0),
                RouteRange {
                    start: cord,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(cord),
                    sink: crate::CordEndpoint::local(NodeId(sink), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::<3>::new(1);
    bindings
        .install(
            binding_node,
            HostCallBinding {
                call: HostCallId(0),
                maximum_input_bytes: 4,
                maximum_output_bytes: 4,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    FixedScheduler::new_with_host_calls(
        [
            node([None, None]),
            node([Some(CordId(0)), None]),
            node([Some(CordId(1)), None]),
        ],
        [cord(0, 0, 0, 1, 0), cord(1, 1, 0, 2, 0)],
        routes,
        bindings,
        [
            HostDriver::Source { value: Some(input) },
            HostDriver::Effect {
                requested: false,
                cancelled: false,
                repeat_request: false,
            },
            HostDriver::Sink { seen: None },
        ],
        values,
        signs,
    )
    .unwrap()
}

#[test]
fn cancellation_releases_queued_and_driver_owned_values_and_is_terminal() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let normalized = execute_cancellation(
        FixedValueStore::<4, 4>::new(8).unwrap(),
        FixedSignLog::<16>::new(charge * 16).unwrap(),
    );
    assert_eq!(normalized.used_items, 0);
    assert!(normalized.driver_cancelled);
    assert_eq!(normalized.status, SchedulerStatus::Cancelled);
    assert!(normalized.saw_cancellation_requested);
    assert!(normalized.saw_run_cancelled);
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_and_fixed_cancellation_vectors_match() {
    use crate::{HostedSignLog, HostedValueStore};

    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let fixed = execute_cancellation(
        FixedValueStore::<4, 4>::new(8).unwrap(),
        FixedSignLog::<16>::new(charge * 16).unwrap(),
    );
    let hosted = execute_cancellation(
        HostedValueStore::new(4, 4, 8).unwrap(),
        HostedSignLog::new(16, charge * 16).unwrap(),
    );
    assert_eq!(fixed, hosted);
}

#[derive(Debug, Eq, PartialEq)]
struct CancellationNormalized {
    used_items: u16,
    sign_len: u16,
    sign_bytes: u32,
    driver_cancelled: bool,
    status: SchedulerStatus,
    saw_cancellation_requested: bool,
    saw_run_cancelled: bool,
}

fn execute_cancellation<S, E>(mut values: S, signs: E) -> CancellationNormalized
where
    S: ValueStorage,
    E: SignSink + SignQuery,
{
    let source_values = [
        Some(values.store(&[0]).unwrap()),
        Some(values.store(&[1]).unwrap()),
        None,
        None,
    ];
    let mut routes = FixedRoutes::<2, 1>::new(1);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: crate::CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 2, 1, 2, 1, 2, 1>::new(
        [
            NodeSpec {
                input_cords: [None, None],
                maximum_step_work: 1,
            },
            NodeSpec {
                input_cords: [Some(CordId(0)), None],
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
                byte_capacity: 4,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        [
            Driver::Source {
                values: source_values,
                next: 0,
            },
            Driver::BlockedSink { cancelled: false },
        ],
        values,
        signs,
    )
    .unwrap();
    assert!(matches!(
        scheduler.step().unwrap(),
        SchedulerStatus::Progress { node: NodeId(0) }
    ));
    assert!(matches!(
        scheduler.step().unwrap(),
        SchedulerStatus::Progress { node: NodeId(1) }
    ));
    scheduler.cancel().unwrap();
    let Driver::BlockedSink { cancelled } = scheduler.drivers()[1] else {
        panic!("blocked sink");
    };
    CancellationNormalized {
        used_items: scheduler.values().used_items(),
        sign_len: scheduler.signs().len(),
        sign_bytes: scheduler.signs().used_bytes(),
        driver_cancelled: cancelled,
        status: scheduler.step().unwrap(),
        saw_cancellation_requested: scheduler
            .signs()
            .contains_kind(KernelEventKind::CancellationRequested),
        saw_run_cancelled: scheduler
            .signs()
            .contains_kind(KernelEventKind::RunCancelled),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Normalized {
    show_a: [u16; 4],
    show_a_len: usize,
    show_b: [u16; 4],
    show_b_len: usize,
    decisions: u32,
    sign_len: u16,
    sign_bytes: u32,
    used_items: u16,
    saw_input_closed: bool,
}

fn execute<S, E>(mut values: S, signs: E) -> Normalized
where
    S: ValueStorage,
    E: SignSink + SignQuery,
{
    let source_values = [
        Some(values.store(&[0]).unwrap()),
        Some(values.store(&[1]).unwrap()),
        Some(values.store(&[2]).unwrap()),
        Some(values.store(&[3]).unwrap()),
    ];
    let mut routes = FixedRoutes::<{ NODES * PORTS }, CORDS>::new(PORTS as u16);
    for (source_node, source_port, target) in [
        (
            0,
            0,
            RouteTarget {
                cord: CordId(0),
                sink: crate::CordEndpoint::local(NodeId(1), PortId(0)),
            },
        ),
        (
            1,
            0,
            RouteTarget {
                cord: CordId(1),
                sink: crate::CordEndpoint::local(NodeId(2), PortId(0)),
            },
        ),
        (
            1,
            1,
            RouteTarget {
                cord: CordId(2),
                sink: crate::CordEndpoint::local(NodeId(3), PortId(0)),
            },
        ),
        (
            2,
            0,
            RouteTarget {
                cord: CordId(3),
                sink: crate::CordEndpoint::local(NodeId(4), PortId(0)),
            },
        ),
        (
            3,
            0,
            RouteTarget {
                cord: CordId(4),
                sink: crate::CordEndpoint::local(NodeId(5), PortId(0)),
            },
        ),
    ] {
        routes
            .install(
                NodeId(source_node),
                PortId(source_port),
                RouteRange {
                    start: target.cord.0,
                    len: 1,
                },
                &[target],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let cords = [
        cord(0, 0, 0, 1, 0),
        cord(1, 1, 0, 2, 0),
        cord(2, 1, 1, 3, 0),
        cord(3, 2, 0, 4, 0),
        cord(4, 3, 0, 5, 0),
    ];
    let nodes = [
        node([None, None]),
        node([Some(CordId(0)), None]),
        node([Some(CordId(1)), None]),
        node([Some(CordId(2)), None]),
        node([Some(CordId(3)), None]),
        node([Some(CordId(4)), None]),
    ];
    let drivers = [
        Driver::Source {
            values: source_values,
            next: 0,
        },
        Driver::Tee,
        Driver::Filter,
        Driver::Latest { held: None },
        Driver::Sink {
            seen: [None; 4],
            len: 0,
            stall: true,
        },
        Driver::Sink {
            seen: [None; 4],
            len: 0,
            stall: true,
        },
    ];
    let mut scheduler =
        FixedScheduler::<_, _, _, NODES, CORDS, PORTS, CORDS, { NODES * PORTS }, CORDS>::new(
            nodes, cords, routes, drivers, values, signs,
        )
        .unwrap();

    scheduler.run(128).unwrap();
    assert_eq!(scheduler.step().unwrap(), SchedulerStatus::Drained);
    let Driver::Sink { seen, len, .. } = &scheduler.drivers()[4] else {
        panic!("show-a sink");
    };
    let mut show_a = [u16::MAX; 4];
    for (index, value) in seen[..*len].iter().enumerate() {
        show_a[index] = value.unwrap().slot;
    }
    let show_a_len = *len;
    let Driver::Sink { seen, len, .. } = &scheduler.drivers()[5] else {
        panic!("show-b sink");
    };
    let mut show_b = [u16::MAX; 4];
    for (index, value) in seen[..*len].iter().enumerate() {
        show_b[index] = value.unwrap().slot;
    }
    Normalized {
        show_a,
        show_a_len,
        show_b,
        show_b_len: *len,
        decisions: scheduler.decisions(),
        sign_len: scheduler.signs().len(),
        sign_bytes: scheduler.signs().used_bytes(),
        used_items: scheduler.values().used_items(),
        saw_input_closed: scheduler
            .signs()
            .contains_kind(KernelEventKind::InputClosed),
    }
}

fn node(input_cords: [Option<CordId>; PORTS]) -> NodeSpec<PORTS> {
    NodeSpec {
        input_cords,
        maximum_step_work: 3,
    }
}

#[test]
fn remote_cords_keep_values_owned_until_delivery_and_retry_full_without_growth() {
    let endpoint = RemoteEndpointId(0);
    let mut source_values = FixedValueStore::<3, 12>::new(12).unwrap();
    let first = source_values.store(b"abcd").unwrap();
    let second = source_values.store(b"efgh").unwrap();
    let mut source_routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    source_routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: crate::CordEndpoint::Remote(endpoint),
            }],
        )
        .unwrap();
    source_routes.seal().unwrap();
    let source_sign = FixedSignLog::<64>::new_with_remote_storage(
        (64 * core::mem::size_of::<crate::KernelEvent>()) as u32,
        64,
        crate::remote_sign_storage_bytes(64).unwrap(),
    )
    .unwrap();
    let mut source = FixedScheduler::<_, _, _, 1, 1, PORTS, 1, 2, 1>::new(
        [node([None; PORTS])],
        [CordSpec::remote_egress(
            CordId(0),
            (NodeId(0), PortId(0)),
            endpoint,
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 4,
                pressure_policy: Default::default(),
            },
        )],
        source_routes,
        [Driver::Source {
            values: [Some(first), Some(second), None, None],
            next: 0,
        }],
        source_values,
        source_sign,
    )
    .unwrap();

    let mut sink_routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    sink_routes.seal().unwrap();
    let sink_sign = FixedSignLog::<64>::new_with_remote_storage(
        (64 * core::mem::size_of::<crate::KernelEvent>()) as u32,
        64,
        crate::remote_sign_storage_bytes(64).unwrap(),
    )
    .unwrap();
    let mut sink = FixedScheduler::<_, _, _, 1, 1, PORTS, 1, 2, 1>::new(
        [node([Some(CordId(0)), None])],
        [CordSpec::remote_ingress(
            CordId(0),
            endpoint,
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 4,
                pressure_policy: Default::default(),
            },
        )],
        sink_routes,
        [Driver::Sink {
            seen: [None; 4],
            len: 0,
            stall: false,
        }],
        FixedValueStore::<1, 4>::new(4).unwrap(),
        sink_sign,
    )
    .unwrap();

    assert!(matches!(
        source.step().unwrap(),
        SchedulerStatus::Progress { .. }
    ));
    let offer = source
        .remote_egress_offer(endpoint, CordId(0))
        .unwrap()
        .unwrap();
    assert_eq!(offer.sequence, 0);
    assert_eq!(
        source.remote_egress_offer(endpoint, CordId(0)).unwrap(),
        Some(offer)
    );
    assert_eq!(
        source.remote_egress_offer(RemoteEndpointId(1), CordId(0)),
        Err(SchedulerError::InvalidRemoteCordAccess)
    );
    assert_eq!(source.values().reference_count(first).unwrap(), 1);
    assert_eq!(
        source.discard_host_value(first),
        Err(SchedulerError::ValueOwnershipViolation)
    );
    let first_bytes = *source
        .host_value(offer.value)
        .unwrap()
        .first_chunk::<4>()
        .unwrap();
    assert!(matches!(
        sink.admit_remote_input(endpoint, CordId(0), 0, &first_bytes)
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence: 0, .. }
    ));
    assert_eq!(
        sink.admit_remote_input(endpoint, CordId(0), 0, &first_bytes),
        Err(SchedulerError::RemoteSequenceRejected)
    );
    assert_eq!(
        sink.admit_remote_input(endpoint, CordId(0), 2, b"ijkl"),
        Err(SchedulerError::RemoteSequenceRejected)
    );
    assert_eq!(sink.cord_usage(CordId(0)).unwrap(), (1, 4));
    assert_eq!(
        sink.admit_remote_input(endpoint, CordId(0), 1, b"efgh")
            .unwrap(),
        RemoteIngressOutcome::Full { sequence: 1 }
    );
    assert_eq!(sink.cord_usage(CordId(0)).unwrap(), (1, 4));
    assert_eq!(sink.values().used_items(), 1);

    assert_eq!(
        source.remote_egress_accept(endpoint, CordId(0), 1),
        Err(SchedulerError::RemoteSequenceRejected)
    );
    source.remote_egress_accept(endpoint, CordId(0), 0).unwrap();
    source.remote_egress_accept(endpoint, CordId(0), 0).unwrap();
    source
        .remote_egress_delivered(endpoint, CordId(0), 0)
        .unwrap();
    assert_eq!(source.values().used_items(), 1);
    assert_eq!(
        source.remote_egress_delivered(endpoint, CordId(0), 0),
        Err(SchedulerError::RemoteDeliveryRejected)
    );
    sink.step().unwrap();
    assert_eq!(sink.values().used_items(), 0);

    source.step().unwrap();
    let offer = source
        .remote_egress_offer(endpoint, CordId(0))
        .unwrap()
        .unwrap();
    assert_eq!(offer.sequence, 1);
    let second_bytes = *source
        .host_value(offer.value)
        .unwrap()
        .first_chunk::<4>()
        .unwrap();
    assert!(matches!(
        sink.admit_remote_input(endpoint, CordId(0), 1, &second_bytes)
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence: 1, .. }
    ));
    source.remote_egress_accept(endpoint, CordId(0), 1).unwrap();
    source
        .remote_egress_delivered(endpoint, CordId(0), 1)
        .unwrap();
    source.step().unwrap();
    assert!(source.remote_egress_terminal(endpoint, CordId(0)).unwrap());

    sink.close_remote_input(endpoint, CordId(0)).unwrap();
    sink.step().unwrap();
    sink.step().unwrap();
    assert_eq!(sink.step().unwrap(), SchedulerStatus::Drained);
    assert_eq!(sink.values().used_items(), 0);
    assert_eq!(sink.cord_usage(CordId(0)).unwrap(), (0, 0));
    assert!(source
        .signs()
        .contains_kind(KernelEventKind::RemoteValueDelivered));
    assert!(sink
        .signs()
        .contains_kind(KernelEventKind::RemoteInputClosed));
    assert!(source
        .signs()
        .events()
        .filter_map(|event| {
            source
                .signs()
                .remote_identity(event.sequence)
                .map(|remote| (event.kind, remote))
        })
        .eq([
            (
                KernelEventKind::RemoteValueOffered,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Egress,
                    sequence: 0,
                },
            ),
            (
                KernelEventKind::RemoteValueAccepted,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Egress,
                    sequence: 0,
                },
            ),
            (
                KernelEventKind::RemoteValueDelivered,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Egress,
                    sequence: 0,
                },
            ),
            (
                KernelEventKind::RemoteValueOffered,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Egress,
                    sequence: 1,
                },
            ),
            (
                KernelEventKind::RemoteValueAccepted,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Egress,
                    sequence: 1,
                },
            ),
            (
                KernelEventKind::RemoteValueDelivered,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Egress,
                    sequence: 1,
                },
            ),
            (
                KernelEventKind::RemoteOutputClosed,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Egress,
                    sequence: 2,
                },
            ),
        ]));
    assert!(sink
        .signs()
        .events()
        .filter_map(|event| {
            sink.signs()
                .remote_identity(event.sequence)
                .map(|remote| (event.kind, remote))
        })
        .eq([
            (
                KernelEventKind::RemoteInputAdmitted,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Ingress,
                    sequence: 0,
                },
            ),
            (
                KernelEventKind::RemoteInputAdmitted,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Ingress,
                    sequence: 1,
                },
            ),
            (
                KernelEventKind::RemoteInputClosed,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Ingress,
                    sequence: 2,
                },
            ),
        ]));
    let remote_sign_count = source
        .signs()
        .events()
        .filter(|event| source.signs().remote_identity(event.sequence).is_some())
        .count();
    source.cancel().unwrap();
    assert_eq!(
        source.remote_egress_accept(endpoint, CordId(0), 1),
        Err(SchedulerError::Cancelled)
    );
    assert_eq!(
        source.remote_egress_delivered(endpoint, CordId(0), 1),
        Err(SchedulerError::RemoteDeliveryRejected)
    );
    assert_eq!(
        source
            .signs()
            .events()
            .filter(|event| source.signs().remote_identity(event.sequence).is_some())
            .count(),
        remote_sign_count
    );
}

#[test]
fn remote_delivery_sign_exhaustion_preserves_the_in_flight_value() {
    let endpoint = RemoteEndpointId(0);
    let mut values = FixedValueStore::<1, 4>::new(4).unwrap();
    let value = values.store(b"data").unwrap();
    let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: crate::CordEndpoint::Remote(endpoint),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let signs = FixedSignLog::<8>::new_with_remote_storage(
        (8 * core::mem::size_of::<crate::KernelEvent>()) as u32,
        2,
        crate::remote_sign_storage_bytes(2).unwrap(),
    )
    .unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 1, 1, PORTS, 1, 2, 1>::new(
        [node([None; PORTS])],
        [CordSpec::remote_egress(
            CordId(0),
            (NodeId(0), PortId(0)),
            endpoint,
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 4,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        [Driver::Source {
            values: [Some(value), None, None, None],
            next: 0,
        }],
        values,
        signs,
    )
    .unwrap();

    scheduler.step().unwrap();
    let offer = scheduler
        .remote_egress_offer(endpoint, CordId(0))
        .unwrap()
        .unwrap();
    scheduler
        .remote_egress_accept(endpoint, CordId(0), offer.sequence)
        .unwrap();
    assert_eq!(
        scheduler.remote_egress_delivered(endpoint, CordId(0), offer.sequence),
        Err(SchedulerError::Sign(
            crate::SignError::RemoteItemCapacityExceeded
        ))
    );
    assert_eq!(scheduler.cord_usage(CordId(0)).unwrap(), (1, 4));
    assert_eq!(scheduler.values().reference_count(value).unwrap(), 1);
    assert_eq!(
        scheduler.remote_egress_offer(endpoint, CordId(0)).unwrap(),
        Some(offer)
    );
}

#[test]
fn remote_ingress_sign_exhaustion_preserves_queue_sequence_and_open_state() {
    let endpoint = RemoteEndpointId(0);
    let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    routes.seal().unwrap();
    let signs =
        FixedSignLog::<8>::new((8 * core::mem::size_of::<crate::KernelEvent>()) as u32).unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 1, 1, PORTS, 1, 2, 1>::new(
        [node([Some(CordId(0)), None])],
        [CordSpec::remote_ingress(
            CordId(0),
            endpoint,
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 4,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        [Driver::Sink {
            seen: [None; 4],
            len: 0,
            stall: false,
        }],
        FixedValueStore::<1, 4>::new(4).unwrap(),
        signs,
    )
    .unwrap();

    assert_eq!(
        scheduler.admit_remote_input(endpoint, CordId(0), 0, b"data"),
        Err(SchedulerError::Sign(
            crate::SignError::RemoteItemCapacityExceeded
        ))
    );
    assert_eq!(scheduler.cord_usage(CordId(0)).unwrap(), (0, 0));
    assert_eq!(
        scheduler.admit_remote_input(endpoint, CordId(0), 1, b"data"),
        Err(SchedulerError::RemoteSequenceRejected)
    );
    assert_eq!(
        scheduler.close_remote_input(endpoint, CordId(0)),
        Err(SchedulerError::Sign(
            crate::SignError::RemoteItemCapacityExceeded
        ))
    );
    assert_eq!(
        scheduler.admit_remote_input(endpoint, CordId(0), 0, b"data"),
        Err(SchedulerError::Sign(
            crate::SignError::RemoteItemCapacityExceeded
        ))
    );
}

#[test]
fn coalescing_cords_supersede_the_newest_pending_item_without_growth() {
    let endpoint = RemoteEndpointId(0);
    let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    routes.seal().unwrap();
    let signs = FixedSignLog::<16>::new_with_remote_storage(
        (16 * core::mem::size_of::<crate::KernelEvent>()) as u32,
        16,
        crate::remote_sign_storage_bytes(16).unwrap(),
    )
    .unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 1, 1, PORTS, 1, 2, 1>::new(
        [node([Some(CordId(0)), None])],
        [CordSpec::remote_ingress(
            CordId(0),
            endpoint,
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 4,
                pressure_policy: AssignedPressurePolicy::CoalesceLatest,
            },
        )],
        routes,
        [Driver::BlockedSink { cancelled: false }],
        FixedValueStore::<2, 8>::new(8).unwrap(),
        signs,
    )
    .unwrap();

    assert_eq!(
        scheduler.admit_remote_input(endpoint, CordId(0), 0, b"old!"),
        Ok(RemoteIngressOutcome::Accepted { sequence: 0 })
    );
    assert_eq!(scheduler.values().used_items(), 1);
    assert_eq!(
        scheduler.admit_remote_input(endpoint, CordId(0), 1, b"new!"),
        Ok(RemoteIngressOutcome::Accepted { sequence: 1 })
    );
    assert_eq!(scheduler.cord_usage(CordId(0)).unwrap(), (1, 4));
    assert_eq!(scheduler.values().used_items(), 1);

    scheduler.cancel().unwrap();
    let Driver::BlockedSink { cancelled } = scheduler.drivers()[0] else {
        panic!("blocked sink");
    };
    assert!(cancelled);
    assert_eq!(scheduler.values().used_items(), 0);
}

fn cord(id: u16, source_node: u16, source_port: u16, sink_node: u16, sink_port: u16) -> CordSpec {
    CordSpec::local(
        CordId(id),
        (NodeId(source_node), PortId(source_port)),
        (NodeId(sink_node), PortId(sink_port)),
        CordCapacity {
            slot_start: id,
            item_capacity: 1,
            byte_capacity: 4,
            pressure_policy: Default::default(),
        },
    )
}
