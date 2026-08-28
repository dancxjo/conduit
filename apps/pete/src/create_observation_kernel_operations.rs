use conduit_kernel::{
    scheduler::{CordCapacity, CordSpec, FixedScheduler, NodeSpec, OperationDriver},
    BoundedValueRef, CordId, FixedHostOperationBindings, FixedRoutes, FixedSignLog,
    FixedValueStore, HostOperationBinding, HostOperationDisposition, HostOperationId, KernelEvent,
    NodeId, Operation, OperationAction, OperationInput, PortId, RequestId, RouteRange, RouteTarget,
    ValueRef, ValueStorage,
};

const SOURCE_NODE: NodeId = NodeId(0);
const SINK_NODE: NodeId = NodeId(1);
const OPERATION: HostOperationId = HostOperationId(0);
const PORTS: usize = 1;
const SIGNS: usize = 64;
const MAXIMUM_VALUE_BYTES: usize = conduit_robotics::ROBOTICS_CHARGING_ENCODED_LEN;

#[derive(Clone, Copy)]
pub(super) struct ObservationSource {
    empty: ValueRef,
    pending: bool,
    emitted: bool,
}

impl Operation for ObservationSource {
    fn start(&mut self) -> OperationAction {
        let input = BoundedValueRef::new(self.empty, 0).expect("empty request is exact");
        self.pending = true;
        OperationAction::RequestHostOperation {
            request: RequestId(1),
            operation: OPERATION,
            input,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted {
                request: RequestId(1),
                outcome,
            } if self.pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.failure.is_none() =>
            {
                self.pending = false;
                match outcome.output {
                    Some(output) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    None => OperationAction::Complete,
                }
            }
            OperationInput::HostOperationCompleted { outcome, .. }
                if self.pending
                    && outcome.disposition == HostOperationDisposition::Failed
                    && outcome.output.is_none()
                    && outcome.failure.is_some() =>
            {
                self.pending = false;
                OperationAction::Fail(outcome.failure.expect("guarded failure"))
            }
            _ => invalid(1),
        }
    }

    fn advance(&mut self) -> OperationAction {
        if self.emitted {
            self.emitted = false;
            OperationAction::Complete
        } else {
            invalid(2)
        }
    }

    fn cancel(&mut self) {
        self.pending = false;
        self.emitted = false;
    }
}

#[derive(Clone, Copy)]
pub(super) struct ObservationSink {
    received: bool,
}

impl Operation for ObservationSink {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0), ..
            } if !self.received => {
                self.received = true;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if self.received => {
                OperationAction::Complete
            }
            _ => invalid(3),
        }
    }

    fn cancel(&mut self) {}
}

const fn invalid(detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

#[derive(Clone, Copy)]
pub(super) enum DriverOperation {
    Source(ObservationSource),
    Sink(ObservationSink),
}

impl Operation for DriverOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source(value) => value.start(),
            Self::Sink(value) => value.start(),
        }
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::Source(value) => value.resume(input),
            Self::Sink(value) => value.resume(input),
        }
    }
    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source(value) => value.advance(),
            Self::Sink(value) => value.advance(),
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Source(value) => value.cancel(),
            Self::Sink(value) => value.cancel(),
        }
    }
}

pub(super) type Scheduler = FixedScheduler<
    OperationDriver<DriverOperation, PORTS>,
    FixedValueStore<3, MAXIMUM_VALUE_BYTES>,
    FixedSignLog<SIGNS>,
    2,
    1,
    PORTS,
    1,
    1,
    1,
    1,
    1,
>;

pub(super) fn prepare_scheduler(maximum_output_bytes: u32) -> Result<Scheduler, &'static str> {
    let mut values = FixedValueStore::<3, MAXIMUM_VALUE_BYTES>::new(MAXIMUM_VALUE_BYTES as u32)
        .map_err(|_| "value admission failed")?;
    let empty = values
        .store(&[])
        .map_err(|_| "empty request admission failed")?;
    let mut routes = FixedRoutes::<1, 1>::new(PORTS as u16);
    routes
        .install(
            SOURCE_NODE,
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: conduit_kernel::CordEndpoint::local(SINK_NODE, PortId(0)),
            }],
        )
        .map_err(|_| "route admission failed")?;
    routes.seal().map_err(|_| "route seal failed")?;
    let mut bindings = FixedHostOperationBindings::<1>::new(1);
    bindings
        .install(
            SOURCE_NODE,
            HostOperationBinding {
                operation: OPERATION,
                maximum_input_bytes: 0,
                maximum_output_bytes,
            },
        )
        .map_err(|_| "host operation admission failed")?;
    bindings.seal().map_err(|_| "host operation seal failed")?;
    let signs = FixedSignLog::new((SIGNS * core::mem::size_of::<KernelEvent>()) as u32)
        .map_err(|_| "sign admission failed")?;
    FixedScheduler::new_with_host_operations(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_work: 2,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_work: 2,
            },
        ],
        [CordSpec::local(
            CordId(0),
            (SOURCE_NODE, PortId(0)),
            (SINK_NODE, PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: maximum_output_bytes,
            },
        )],
        routes,
        bindings,
        [
            OperationDriver::new(DriverOperation::Source(ObservationSource {
                empty,
                pending: false,
                emitted: false,
            }))
            .map_err(|_| "source preparation failed")?,
            OperationDriver::new(DriverOperation::Sink(ObservationSink { received: false }))
                .map_err(|_| "sink preparation failed")?,
        ],
        values,
        signs,
    )
    .map_err(|_| "kernel preparation failed")
}

pub(super) fn sink_received(scheduler: &Scheduler) -> bool {
    matches!(
        scheduler.drivers()[1].operation(),
        DriverOperation::Sink(ObservationSink { received: true })
    )
}
