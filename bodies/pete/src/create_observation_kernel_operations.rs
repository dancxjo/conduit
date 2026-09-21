use conduit_kernel::{
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, NodeSpec, StepBack, StepInputBytes, StepIo,
        StepOutcome,
    },
    BoundedValueRef, CordId, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostCallBinding, HostCallDisposition, HostCallId, KernelEvent, NodeId, PortId, RequestId,
    RouteRange, RouteTarget, ValueRef, ValueStorage,
};

const SOURCE_NODE: NodeId = NodeId(0);
const SINK_NODE: NodeId = NodeId(1);
const OPERATION: HostCallId = HostCallId(0);
const PORTS: usize = 1;
const SIGNS: usize = 64;
const MAXIMUM_VALUE_BYTES: usize = conduit_robotics::ROBOTICS_CHARGING_ENCODED_LEN;

#[derive(Clone, Copy)]
pub(super) struct ObservationSource {
    empty: ValueRef,
    pending: bool,
    emitted: bool,
}

#[derive(Clone, Copy)]
pub(super) struct ObservationSink {
    received: bool,
}

const fn invalid(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

#[derive(Clone, Copy)]
pub(super) enum DriverOperation {
    Source(ObservationSource),
    Sink(ObservationSink),
}

impl StepBack<PORTS> for DriverOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source(source) => {
                if let Some((request, outcome)) = io.host_completion() {
                    if request != RequestId(1) || !source.pending {
                        return invalid(1);
                    }
                    match (outcome.disposition, outcome.output, outcome.failure) {
                        (HostCallDisposition::Completed, output, None) => {
                            if output.is_some() && !io.output_ready(PortId(0)) {
                                return StepOutcome::Await;
                            }
                            io.consume_host_completion()
                                .expect("observed observation Host Call completion");
                            source.pending = false;
                            if let Some(output) = output {
                                io.send(PortId(0), output.value)
                                    .expect("ready observation Cord");
                                source.emitted = true;
                            }
                            return StepOutcome::Complete;
                        }
                        (HostCallDisposition::Failed, None, Some(failure)) => {
                            io.consume_host_completion()
                                .expect("observed failed observation Host Call");
                            source.pending = false;
                            return StepOutcome::Fail(failure);
                        }
                        _ => return invalid(1),
                    }
                }
                if source.emitted {
                    return StepOutcome::Complete;
                }
                if source.pending {
                    return StepOutcome::Await;
                }
                let input = BoundedValueRef::new(source.empty, 0).expect("empty request is exact");
                io.request_host_call(RequestId(1), OPERATION, input)
                    .expect("planned observation Host Call");
                source.pending = true;
                StepOutcome::Progress
            }
            Self::Sink(sink) => {
                if !sink.received {
                    if io.input(PortId(0)).is_none() {
                        return StepOutcome::Await;
                    }
                    io.consume(PortId(0)).expect("present observation");
                    sink.received = true;
                    return StepOutcome::Progress;
                }
                if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0))
                        .expect("observed observation closure");
                    return StepOutcome::Complete;
                }
                StepOutcome::Await
            }
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Source(value) => {
                value.pending = false;
                value.emitted = false;
            }
            Self::Sink(_) => {}
        }
    }
}

pub(super) type Scheduler = FixedScheduler<
    DriverOperation,
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
    let mut bindings = FixedHostCallBindings::<1>::new(1);
    bindings
        .install(
            SOURCE_NODE,
            HostCallBinding {
                call: OPERATION,
                maximum_input_bytes: 0,
                maximum_output_bytes,
            },
        )
        .map_err(|_| "Host Call admission failed")?;
    bindings.seal().map_err(|_| "Host Call seal failed")?;
    let signs = FixedSignLog::new((SIGNS * core::mem::size_of::<KernelEvent>()) as u32)
        .map_err(|_| "sign admission failed")?;
    FixedScheduler::new_with_host_calls(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 2,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_fuel: 2,
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
                pressure_policy: Default::default(),
            },
        )],
        routes,
        bindings,
        [
            DriverOperation::Source(ObservationSource {
                empty,
                pending: false,
                emitted: false,
            }),
            DriverOperation::Sink(ObservationSink { received: false }),
        ],
        values,
        signs,
    )
    .map_err(|_| "kernel preparation failed")
}

pub(super) fn sink_received(scheduler: &Scheduler) -> bool {
    matches!(
        &scheduler.drivers()[1],
        DriverOperation::Sink(ObservationSink { received: true })
    )
}
