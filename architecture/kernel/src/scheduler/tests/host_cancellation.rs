use super::*;

#[derive(Clone, Copy, Debug)]
enum CancellationBack {
    Reset {
        initial: ValueRef,
        replacement: Option<ValueRef>,
        phase: u8,
    },
    Source {
        value: ValueRef,
        emitted: bool,
    },
}

impl StepBack<2> for CancellationBack {
    fn step(&mut self, io: &mut StepIo<2>, _input_bytes: &StepInputBytes<'_, 2>) -> StepOutcome {
        match self {
            Self::Reset {
                initial,
                replacement,
                phase,
            } => match *phase {
                0 => {
                    io.request_host_call(
                        RequestId(21),
                        HostCallId(0),
                        BoundedValueRef::new(*initial, 8).unwrap(),
                    )
                    .unwrap();
                    *phase = 1;
                    StepOutcome::Progress
                }
                1 => {
                    if let Some((request, outcome)) = io.host_completion() {
                        if request != RequestId(21)
                            || outcome.disposition != HostCallDisposition::Cancelled
                            || io.consume_host_completion().is_err()
                        {
                            return invalid_cancellation();
                        }
                        let Some(value) = replacement.take() else {
                            return invalid_cancellation();
                        };
                        io.request_host_call(
                            RequestId(22),
                            HostCallId(0),
                            BoundedValueRef::new(value, 8).unwrap(),
                        )
                        .unwrap();
                        *phase = 2;
                        return StepOutcome::Progress;
                    }
                    let Some(value) = io.input(PortId(0)) else {
                        return StepOutcome::Await;
                    };
                    io.take_input(PortId(0)).unwrap();
                    io.cancel_host_call(RequestId(21)).unwrap();
                    *replacement = Some(value);
                    StepOutcome::Progress
                }
                2 => {
                    let Some((request, outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    if request != RequestId(22)
                        || outcome.disposition != HostCallDisposition::Completed
                        || io.consume_host_completion().is_err()
                    {
                        return invalid_cancellation();
                    }
                    *phase = 3;
                    StepOutcome::Complete
                }
                _ => StepOutcome::Complete,
            },
            Self::Source { value, emitted } => {
                if *emitted {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), *value).unwrap();
                *emitted = true;
                StepOutcome::Complete
            }
        }
    }

    fn accepts_input_while_host_call_pending(&self) -> bool {
        matches!(self, Self::Reset { phase: 1, .. })
    }
}

fn invalid_cancellation() -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail: 856,
    })
}

#[derive(Debug, Eq, PartialEq)]
struct Normalized {
    first: RequestId,
    replacement: RequestId,
    cancellation: super::super::HostCallCancellation,
    used_items: u16,
    pending: usize,
    saw_cancellation: bool,
}

type CancellationScheduler<S, E> = FixedScheduler<CancellationBack, S, E, 2, 1, 2, 1, 4, 1, 2, 1>;

fn scheduler<S: ValueStorage, E: SignSink>(mut values: S, signs: E) -> CancellationScheduler<S, E> {
    let initial = values.store(&10_u64.to_le_bytes()).unwrap();
    let replacement = values.store(&[2]).unwrap();
    let mut routes = FixedRoutes::<4, 1>::new(2);
    routes
        .install(
            NodeId(1),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: crate::CordEndpoint::local(NodeId(0), PortId(0)),
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
                maximum_input_bytes: 8,
                maximum_output_bytes: 0,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    FixedScheduler::<_, _, _, 2, 1, 2, 1, 4, 1, 2, 1>::new_with_host_calls(
        [node([Some(CordId(0)), None]), node([None, None])],
        [CordSpec::local(
            CordId(0),
            (NodeId(1), PortId(0)),
            (NodeId(0), PortId(0)),
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
            CancellationBack::Reset {
                initial,
                replacement: None,
                phase: 0,
            },
            CancellationBack::Source {
                value: replacement,
                emitted: false,
            },
        ],
        values,
        signs,
    )
    .unwrap()
}

fn execute<S: ValueStorage, E: SignSink + SignQuery>(values: S, signs: E) -> Normalized {
    let mut scheduler = scheduler(values, signs);
    scheduler.step().unwrap();
    assert!(scheduler.next_host_cancellation().is_none());
    let first = scheduler.next_host_request().unwrap();
    scheduler.step().unwrap();
    scheduler.step().unwrap();
    let cancellation = scheduler.next_host_cancellation().unwrap();
    assert!(scheduler.next_host_cancellation().is_none());
    scheduler
        .complete_host_call(
            cancellation.node,
            cancellation.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Cancelled,
                output: None,
                failure: None,
            },
        )
        .unwrap();
    scheduler.step().unwrap();
    let second = scheduler.next_host_request().unwrap();
    scheduler
        .complete_host_call(
            second.node,
            second.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .unwrap();
    scheduler.run(16).unwrap();
    Normalized {
        first: first.request,
        replacement: second.request,
        cancellation,
        used_items: scheduler.values().used_items(),
        pending: scheduler.pending_host_call_count(),
        saw_cancellation: scheduler
            .signs()
            .contains_kind(KernelEventKind::HostCallCancellationRequested),
    }
}

#[test]
fn refuses_cancellation_until_the_exact_request_is_dispatched() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let mut scheduler = scheduler(
        FixedValueStore::<4, 8>::new(32).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    scheduler.step().unwrap();
    scheduler.step().unwrap();
    assert_eq!(
        scheduler.step(),
        Err(super::super::SchedulerError::HostCallCancellationUndispatched)
    );
    assert!(scheduler.next_host_cancellation().is_none());
    assert_eq!(scheduler.pending_host_call_count(), 1);
}

#[test]
fn accepted_completion_wins_before_cancellation() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let mut scheduler = scheduler(
        FixedValueStore::<4, 8>::new(32).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    scheduler.step().unwrap();
    let request = scheduler.next_host_request().unwrap();
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
    assert!(scheduler.next_host_cancellation().is_none());
    scheduler.step().unwrap();
    assert_eq!(
        scheduler.step(),
        Err(super::super::SchedulerError::BackFailed(crate::Failure {
            code: crate::FailureCode::InvalidInput,
            detail: 856
        }))
    );
    assert!(scheduler.next_host_cancellation().is_none());
}

#[test]
fn cancels_and_replaces_one_dispatched_request() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let normalized = execute(
        FixedValueStore::<4, 8>::new(32).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    assert_eq!(normalized.first, RequestId(21));
    assert_eq!(normalized.replacement, RequestId(22));
    assert_eq!(normalized.cancellation.node, NodeId(0));
    assert_eq!(normalized.cancellation.request, normalized.first);
    assert_eq!(normalized.cancellation.call, HostCallId(0));
    assert_eq!(normalized.used_items, 0);
    assert_eq!(normalized.pending, 0);
    assert!(normalized.saw_cancellation);
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_and_fixed_vectors_match() {
    use crate::{HostedSignLog, HostedValueStore};

    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let fixed = execute(
        FixedValueStore::<4, 8>::new(32).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    let hosted = execute(
        HostedValueStore::new(4, 8, 32).unwrap(),
        HostedSignLog::new(64, charge * 64).unwrap(),
    );
    assert_eq!(fixed, hosted);
}
