use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, StepInputBytes, StepIo, StepOperation,
    StepOutcome,
};
use conduit_kernel::{
    BoundedValueRef, CordId, FixedHostCallBindings, FixedRoutes, HostCallBinding,
    HostCallDisposition, HostCallId, HostCallOutcome, HostedSignLog, HostedValueStore, NodeId,
    PortId, RequestId, RouteRange, RouteTarget, ValueRef, ValueStorage,
};
use conduit_std_host::{DeadlineHostAdapter, DeadlineWake};

#[derive(Clone, Copy, Debug)]
enum DeadlineOperation {
    Reset {
        initial: ValueRef,
        replacement: Option<ValueRef>,
        phase: u8,
    },
    Source {
        value: ValueRef,
        advanced: bool,
    },
}

impl StepOperation<2> for DeadlineOperation {
    fn step(&mut self, io: &mut StepIo<2>, _: &StepInputBytes<'_, 2>) -> StepOutcome {
        match self {
            Self::Reset {
                initial,
                replacement,
                phase,
                ..
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
                            || outcome.output.is_some()
                            || outcome.failure.is_some()
                            || io.consume_host_completion().is_err()
                        {
                            return invalid_deadline_lifecycle();
                        }
                        let Some(value) = replacement.take() else {
                            return invalid_deadline_lifecycle();
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
                        || outcome.output.is_some()
                        || outcome.failure.is_some()
                        || io.consume_host_completion().is_err()
                    {
                        return invalid_deadline_lifecycle();
                    }
                    *phase = 3;
                    StepOutcome::Complete
                }
                _ => StepOutcome::Complete,
            },
            Self::Source { value, advanced } => {
                if *advanced {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), *value).unwrap();
                *advanced = true;
                StepOutcome::Complete
            }
        }
    }

    fn accepts_input_while_host_call_pending(&self) -> bool {
        matches!(self, Self::Reset { phase: 1, .. })
    }
}

fn invalid_deadline_lifecycle() -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidInput,
        detail: 856,
    })
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
            pressure_policy: Default::default(),
        },
    )];
    let mut scheduler = FixedScheduler::<_, _, _, 2, 1, 2, 1, 4, 1, 2, 1>::new_with_host_calls(
        nodes,
        cords,
        routes,
        bindings,
        [
            DeadlineOperation::Reset {
                initial,
                replacement: None,
                phase: 0,
            },
            DeadlineOperation::Source {
                value: replacement,
                advanced: false,
            },
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

    assert!(host.is_empty());
    assert_eq!(scheduler.pending_host_call_count(), 0);
    assert_eq!(scheduler.values().used_items(), 0);
    assert_eq!(scheduler.values().allocation_capacities(), value_shape);
    assert_eq!(scheduler.signs().allocation_capacity(), sign_shape);
}
