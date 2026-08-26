use conduit_kernel::{
    scheduler::{CordCapacity, CordSpec, FixedScheduler, NodeSpec, OperationDriver},
    BoundedValueRef, CordId, FixedHostOperationBindings, FixedRoutes, FixedSignLog,
    FixedValueStore, HostOperationBinding, HostOperationDisposition, HostOperationId, KernelEvent,
    NodeId, Operation, OperationAction, OperationInput, PortId, RequestId, RouteRange, RouteTarget,
    ValueStorage,
};

const REQUEST_NODE: NodeId = NodeId(0);
pub(super) const DOCK_NODE: NodeId = NodeId(1);
pub(super) const DOCK_REQUEST: RequestId = RequestId(1);
const DOCK_OPERATION: HostOperationId = HostOperationId(0);
const PORTS: usize = 1;
const SIGNS: usize = 32;
pub(super) const REQUEST_BYTES: u32 = conduit_core::BOOL_ENCODED_LEN as u32;

#[derive(Clone, Copy)]
pub(super) struct BooleanSource {
    value: conduit_kernel::ValueRef,
    emitted: bool,
}

impl Operation for BooleanSource {
    fn start(&mut self) -> OperationAction {
        self.emitted = true;
        OperationAction::Emit {
            port: PortId(0),
            value: self.value,
        }
    }

    fn resume(&mut self, _input: OperationInput) -> OperationAction {
        invalid(1)
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
        self.emitted = false;
    }
}

#[derive(Clone, Copy)]
pub(super) struct CreateDockOperation {
    request: BoundedValueRef,
    pending: bool,
    admitted: bool,
}

impl Operation for CreateDockOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0), ..
            } if !self.pending && !self.admitted => {
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: DOCK_REQUEST,
                    operation: DOCK_OPERATION,
                    input: self.request,
                }
            }
            OperationInput::HostOperationCompleted {
                request: DOCK_REQUEST,
                outcome,
            } if self.pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                self.pending = false;
                self.admitted = true;
                OperationAction::Await
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
            _ => invalid(3),
        }
    }

    fn advance(&mut self) -> OperationAction {
        invalid(4)
    }

    fn cancel(&mut self) {
        self.pending = false;
        self.admitted = false;
    }
}

const fn invalid(detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

#[derive(Clone, Copy)]
pub(super) enum DockKernelOperation {
    Source(BooleanSource),
    Dock(CreateDockOperation),
}

impl Operation for DockKernelOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source(value) => value.start(),
            Self::Dock(value) => value.start(),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::Source(value) => value.resume(input),
            Self::Dock(value) => value.resume(input),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source(value) => value.advance(),
            Self::Dock(value) => value.advance(),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Source(value) => value.cancel(),
            Self::Dock(value) => value.cancel(),
        }
    }
}

pub(super) type DockScheduler = FixedScheduler<
    OperationDriver<DockKernelOperation, PORTS>,
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
    let mut bindings = FixedHostOperationBindings::<2>::new(1);
    bindings
        .install(
            DOCK_NODE,
            HostOperationBinding {
                operation: DOCK_OPERATION,
                maximum_input_bytes: REQUEST_BYTES,
                maximum_output_bytes: 0,
            },
        )
        .map_err(|_| "dock Host operation admission failed")?;
    bindings
        .seal()
        .map_err(|_| "dock Host operation seal failed")?;
    let signs = FixedSignLog::new((SIGNS * core::mem::size_of::<KernelEvent>()) as u32)
        .map_err(|_| "dock Sign admission failed")?;
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
            (REQUEST_NODE, PortId(0)),
            (DOCK_NODE, PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: REQUEST_BYTES,
            },
        )],
        routes,
        bindings,
        [
            OperationDriver::new(DockKernelOperation::Source(BooleanSource {
                value,
                emitted: false,
            }))
            .map_err(|_| "dock request source preparation failed")?,
            OperationDriver::new(DockKernelOperation::Dock(CreateDockOperation {
                request: bounded,
                pending: false,
                admitted: false,
            }))
            .map_err(|_| "dock operation preparation failed")?,
        ],
        values,
        signs,
    )
    .map_err(|_| "dock kernel preparation failed")
}

pub(super) fn dock_is_admitted(scheduler: &DockScheduler) -> bool {
    matches!(
        scheduler.drivers()[usize::from(DOCK_NODE.0)].operation(),
        DockKernelOperation::Dock(CreateDockOperation { admitted: true, .. })
    )
}
