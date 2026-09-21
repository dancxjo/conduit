use conduit_kernel::{
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, NodeSpec, StepInputBytes, StepIo, StepOperation,
        StepOutcome,
    },
    BoundedValueRef, CordId, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostCallBinding, HostCallDisposition, HostCallId, KernelEvent, NodeId, PortId, RequestId,
    RouteRange, RouteTarget, ValueStorage,
};

const REQUEST_NODE: NodeId = NodeId(0);
pub(super) const DOCK_NODE: NodeId = NodeId(1);
pub(super) const DOCK_REQUEST: RequestId = RequestId(1);
const DOCK_OPERATION: HostCallId = HostCallId(0);
const PORTS: usize = 1;
const SIGNS: usize = 32;
pub(super) const REQUEST_BYTES: u32 = conduit_core::BOOL_ENCODED_LEN as u32;

#[derive(Clone, Copy)]
pub(super) struct BooleanSource {
    value: conduit_kernel::ValueRef,
    emitted: bool,
}

#[derive(Clone, Copy)]
pub(super) struct CreateDockOperation {
    request: BoundedValueRef,
    pending: bool,
    admitted: bool,
}

const fn invalid(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

#[derive(Clone, Copy)]
pub(super) enum DockKernelOperation {
    Source(BooleanSource),
    Dock(CreateDockOperation),
}

impl StepOperation<PORTS> for DockKernelOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source(source) => {
                if source.emitted {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), source.value).expect("ready dock Cord");
                source.emitted = true;
                StepOutcome::Complete
            }
            Self::Dock(operation) => {
                if let Some((request, outcome)) = io.host_completion() {
                    if request != DOCK_REQUEST || !operation.pending || outcome.output.is_some() {
                        return invalid(3);
                    }
                    io.consume_host_completion()
                        .expect("observed dock Host Call completion");
                    operation.pending = false;
                    return match (outcome.disposition, outcome.failure) {
                        (HostCallDisposition::Completed, None) => {
                            operation.admitted = true;
                            StepOutcome::Progress
                        }
                        (HostCallDisposition::Failed, Some(failure)) => StepOutcome::Fail(failure),
                        _ => invalid(3),
                    };
                }
                if operation.pending || operation.admitted {
                    return StepOutcome::Await;
                }
                if io.input(PortId(0)).is_none() {
                    return StepOutcome::Await;
                }
                io.consume(PortId(0)).expect("present dock request");
                io.request_host_call(DOCK_REQUEST, DOCK_OPERATION, operation.request)
                    .expect("planned dock Host Call");
                operation.pending = true;
                StepOutcome::Progress
            }
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Source(value) => value.emitted = false,
            Self::Dock(value) => {
                value.pending = false;
                value.admitted = false;
            }
        }
    }
}

pub(super) type DockScheduler = FixedScheduler<
    DockKernelOperation,
    FixedValueStore<1, { REQUEST_BYTES as usize }>,
    FixedSignLog<SIGNS>,
    2,
    1,
    PORTS,
    1,
    2,
    1,
    2,
    1,
>;

pub(super) fn prepare_dock_scheduler(request: [u8; 1]) -> Result<DockScheduler, &'static str> {
    let mut values = FixedValueStore::<1, { REQUEST_BYTES as usize }>::new(REQUEST_BYTES)
        .map_err(|_| "dock value admission failed")?;
    let value = values
        .store(&request)
        .map_err(|_| "dock request admission failed")?;
    let bounded =
        BoundedValueRef::new(value, REQUEST_BYTES).map_err(|_| "dock request bound invalid")?;
    let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    routes
        .install(
            REQUEST_NODE,
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: conduit_kernel::CordEndpoint::local(DOCK_NODE, PortId(0)),
            }],
        )
        .map_err(|_| "dock route admission failed")?;
    routes.seal().map_err(|_| "dock route seal failed")?;
    let mut bindings = FixedHostCallBindings::<2>::new(1);
    bindings
        .install(
            DOCK_NODE,
            HostCallBinding {
                call: DOCK_OPERATION,
                maximum_input_bytes: REQUEST_BYTES,
                maximum_output_bytes: 0,
            },
        )
        .map_err(|_| "dock Host Call admission failed")?;
    bindings.seal().map_err(|_| "dock Host Call seal failed")?;
    let signs = FixedSignLog::new((SIGNS * core::mem::size_of::<KernelEvent>()) as u32)
        .map_err(|_| "dock Sign admission failed")?;
    FixedScheduler::new_with_host_calls(
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
            (REQUEST_NODE, PortId(0)),
            (DOCK_NODE, PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: REQUEST_BYTES,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        bindings,
        [
            DockKernelOperation::Source(BooleanSource {
                value,
                emitted: false,
            }),
            DockKernelOperation::Dock(CreateDockOperation {
                request: bounded,
                pending: false,
                admitted: false,
            }),
        ],
        values,
        signs,
    )
    .map_err(|_| "dock kernel preparation failed")
}

pub(super) fn dock_is_admitted(scheduler: &DockScheduler) -> bool {
    matches!(
        &scheduler.drivers()[usize::from(DOCK_NODE.0)],
        DockKernelOperation::Dock(CreateDockOperation { admitted: true, .. })
    )
}
