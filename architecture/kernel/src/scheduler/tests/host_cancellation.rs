use super::*;

#[derive(Clone, Copy, Debug)]
enum CancellationOperation {
    Reset {
        initial: ValueRef,
        replacement: Option<ValueRef>,
        cancellation: Option<RequestId>,
        phase: u8,
    },
    Source {
        value: ValueRef,
        advanced: bool,
    },
}

impl Operation for CancellationOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Reset { initial, phase, .. } => {
                *phase = 1;
                OperationAction::RequestHostOperation {
                    request: RequestId(21),
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(*initial, 8).unwrap(),
                }
            }
            Self::Source { value, .. } => OperationAction::Emit {
                port: PortId(0),
                value: *value,
            },
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Reset {
                    replacement,
                    cancellation,
                    phase: 1,
                    ..
                },
                OperationInput::Value { value, .. },
            ) => {
                *replacement = Some(value);
                *cancellation = Some(RequestId(21));
                OperationAction::Await
            }
            (
                Self::Reset {
                    replacement, phase, ..
                },
                OperationInput::HostOperationCompleted {
                    request: RequestId(21),
                    outcome,
                },
            ) if outcome.disposition == HostOperationDisposition::Cancelled => {
                *phase = 2;
                OperationAction::RequestHostOperation {
                    request: RequestId(22),
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(replacement.take().unwrap(), 8).unwrap(),
                }
            }
            (
                Self::Reset { phase, .. },
                OperationInput::HostOperationCompleted {
                    request: RequestId(22),
                    outcome,
                },
            ) if outcome.disposition == HostOperationDisposition::Completed => {
                *phase = 3;
                OperationAction::Complete
            }
            _ => OperationAction::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 856,
            }),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source { advanced, .. } if !*advanced => {
                *advanced = true;
                OperationAction::Complete
            }
            _ => OperationAction::Await,
        }
    }

    fn accepts_input_while_host_operation_pending(&self) -> bool {
        matches!(self, Self::Reset { phase: 1, .. })
    }

    fn take_host_operation_cancellation(&mut self) -> Option<RequestId> {
        match self {
            Self::Reset { cancellation, .. } => cancellation.take(),
            Self::Source { .. } => None,
        }
    }

    fn retains_resumed_value(&self) -> bool {
        matches!(
            self,
            Self::Reset {
                replacement: Some(_),
                phase: 1,
                ..
            }
        )
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Normalized {
    first: RequestId,
    replacement: RequestId,
    cancellation: super::super::HostOperationCancellation,
    used_items: u16,
    pending: usize,
    saw_cancellation: bool,
}

type CancellationScheduler<S, E> =
    FixedScheduler<OperationDriver<CancellationOperation, 2>, S, E, 2, 1, 2, 1, 4, 1, 2, 1>;

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
    let mut bindings = FixedHostOperationBindings::<2>::new(1);
    bindings
        .install(
            NodeId(0),
            HostOperationBinding {
                operation: HostOperationId(0),
                maximum_input_bytes: 8,
                maximum_output_bytes: 0,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    FixedScheduler::<_, _, _, 2, 1, 2, 1, 4, 1, 2, 1>::new_with_host_operations(
        [node([Some(CordId(0)), None]), node([None, None])],
        [CordSpec::local(
            CordId(0),
            (NodeId(1), PortId(0)),
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 8,
            },
        )],
        routes,
        bindings,
        [
            OperationDriver::new(CancellationOperation::Reset {
                initial,
                replacement: None,
                cancellation: None,
                phase: 0,
            })
            .unwrap(),
            OperationDriver::new(CancellationOperation::Source {
                value: replacement,
                advanced: false,
            })
            .unwrap(),
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
        .complete_host_operation(
            cancellation.node,
            cancellation.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Cancelled,
                output: None,
                failure: None,
            },
        )
        .unwrap();
    scheduler.step().unwrap();
    let second = scheduler.next_host_request().unwrap();
    scheduler
        .complete_host_operation(
            second.node,
            second.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
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
        pending: scheduler.pending_host_operation_count(),
        saw_cancellation: scheduler
            .signs()
            .contains_kind(KernelEventKind::HostOperationCancellationRequested),
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
        Err(super::super::SchedulerError::HostOperationCancellationUndispatched)
    );
    assert!(scheduler.next_host_cancellation().is_none());
    assert_eq!(scheduler.pending_host_operation_count(), 1);
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
    assert!(scheduler.next_host_cancellation().is_none());
    scheduler.step().unwrap();
    assert_eq!(
        scheduler.step(),
        Err(super::super::SchedulerError::OperationFailed(
            crate::Failure {
                code: crate::FailureCode::InvalidInput,
                detail: 856
            }
        ))
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
    assert_eq!(normalized.cancellation.operation, HostOperationId(0));
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
