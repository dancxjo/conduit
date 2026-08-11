use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, OperationDriver,
};
use conduit_kernel::{
    BoundedValueRef, CordId, FixedHostOperationBindings, FixedRoutes, HostOperationBinding,
    HostOperationDisposition, HostOperationId, HostOperationOutcome, HostedSignLog,
    HostedValueStore, NodeId, Operation, OperationAction, OperationInput, PortId, RequestId,
    RouteRange, RouteTarget, ValueRef, ValueStorage,
};
use conduit_std_host::{DeadlineHostAdapter, DeadlineWake};

#[derive(Clone, Copy, Debug)]
enum DeadlineOperation {
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

impl Operation for DeadlineOperation {
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
            _ => OperationAction::Fail(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::InvalidInput,
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

#[derive(Clone, Copy, Debug)]
struct VirtualClock(u64);

impl conduit_std_host::DeadlineClock for VirtualClock {
    fn now_ms(&mut self) -> Result<u64, conduit_std_host::DeadlineClockError> {
        Ok(self.0)
    }

    fn wait_until_ms(
        &mut self,
        deadline_ms: u64,
    ) -> Result<(), conduit_std_host::DeadlineClockError> {
        self.0 = self.0.max(deadline_ms);
        Ok(())
    }
}

#[test]
fn production_kernel_arms_cancels_replaces_and_completes_one_deadline() {
    let mut values = HostedValueStore::new(4, 8, 32).unwrap();
    let value_shape = values.allocation_capacities();
    let initial = values.store(&50_u64.to_le_bytes()).unwrap();
    let replacement = values.store(&3_u64.to_le_bytes()).unwrap();
    let mut routes = FixedRoutes::<4, 1>::new(2);
    routes
        .install(
            NodeId(1),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: conduit_kernel::CordEndpoint::local(NodeId(0), PortId(0)),
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
    let sign_charge = u32::try_from(core::mem::size_of::<conduit_kernel::KernelEvent>()).unwrap();
    let signs = HostedSignLog::new(64, sign_charge * 64).unwrap();
    let sign_shape = signs.allocation_capacity();
    let nodes = [
        NodeSpec {
            input_cords: [Some(CordId(0)), None],
            maximum_step_work: 3,
        },
        NodeSpec {
            input_cords: [None, None],
            maximum_step_work: 3,
        },
    ];
    let cords = [CordSpec::local(
        CordId(0),
        (NodeId(1), PortId(0)),
        (NodeId(0), PortId(0)),
        CordCapacity {
            slot_start: 0,
            item_capacity: 1,
            byte_capacity: 8,
        },
    )];
    let mut scheduler =
        FixedScheduler::<_, _, _, 2, 1, 2, 1, 4, 1, 2, 1>::new_with_host_operations(
            nodes,
            cords,
            routes,
            bindings,
            [
                OperationDriver::new(DeadlineOperation::Reset {
                    initial,
                    replacement: None,
                    cancellation: None,
                    phase: 0,
                })
                .unwrap(),
                OperationDriver::new(DeadlineOperation::Source {
                    value: replacement,
                    advanced: false,
                })
                .unwrap(),
            ],
            values,
            signs,
        )
        .unwrap();
    let mut host = DeadlineHostAdapter::<_, 1>::new(VirtualClock(100));

    scheduler.step().unwrap();
    let first = scheduler.next_host_request().unwrap();
    let first_duration = u64::from_le_bytes(
        scheduler
            .host_value(first.input.value)
            .unwrap()
            .try_into()
            .unwrap(),
    );
    host.arm(first, first_duration).unwrap();

    scheduler.step().unwrap();
    scheduler.step().unwrap();
    let cancellation = scheduler.next_host_cancellation().unwrap();
    host.cancel(cancellation).unwrap();
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
    let second_duration = u64::from_le_bytes(
        scheduler
            .host_value(second.input.value)
            .unwrap()
            .try_into()
            .unwrap(),
    );
    host.arm(second, second_duration).unwrap();
    assert_eq!(
        host.wait_next().unwrap(),
        DeadlineWake::Fired(second.into())
    );
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

    assert!(host.is_empty());
    assert_eq!(scheduler.pending_host_operation_count(), 0);
    assert_eq!(scheduler.values().used_items(), 0);
    assert_eq!(scheduler.values().allocation_capacities(), value_shape);
    assert_eq!(scheduler.signs().allocation_capacity(), sign_shape);
}
