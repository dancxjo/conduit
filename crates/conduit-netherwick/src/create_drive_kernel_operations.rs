use conduit_kernel::{
    scheduler::{CordCapacity, CordSpec, FixedScheduler, NodeSpec, OperationDriver},
    BoundedValueRef, CordId, FixedHostOperationBindings, FixedRoutes, FixedSignLog,
    FixedValueStore, HostOperationBinding, HostOperationDisposition, HostOperationId, KernelEvent,
    NodeId, Operation, OperationAction, OperationInput, PortId, RequestId, RouteRange, RouteTarget,
    ValueRef, ValueStorage,
};

const LINEAR_NODE: NodeId = NodeId(0);
const ANGULAR_NODE: NodeId = NodeId(1);
pub(super) const DRIVE_NODE: NodeId = NodeId(2);
pub(super) const DRIVE_REQUEST: RequestId = RequestId(1);
const DRIVE_OPERATION: HostOperationId = HostOperationId(0);
const PORTS: usize = 2;
const SIGNS: usize = 64;
const SCALAR_BYTES: u32 = conduit_core::SCALAR_ENCODED_LEN as u32;
pub(super) const REQUEST_BYTES: u32 = (2 * conduit_core::SCALAR_ENCODED_LEN) as u32;
const VALUE_BYTES: u32 = 2 * REQUEST_BYTES;

#[derive(Clone, Copy)]
pub(super) struct ScalarSource {
    value: ValueRef,
    emitted: bool,
}

impl Operation for ScalarSource {
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
pub(super) struct CreateDriveOperation {
    request: BoundedValueRef,
    seen: [bool; 2],
    pending: bool,
    admitted: bool,
}

impl Operation for CreateDriveOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { port, .. }
                if usize::from(port.0) < self.seen.len()
                    && !self.seen[usize::from(port.0)]
                    && !self.pending
                    && !self.admitted =>
            {
                self.seen[usize::from(port.0)] = true;
                if self.seen.into_iter().all(|seen| seen) {
                    self.pending = true;
                    OperationAction::RequestHostOperation {
                        request: DRIVE_REQUEST,
                        operation: DRIVE_OPERATION,
                        input: self.request,
                    }
                } else {
                    OperationAction::Await
                }
            }
            OperationInput::HostOperationCompleted {
                request: DRIVE_REQUEST,
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
pub(super) enum DriveKernelOperation {
    Source(ScalarSource),
    Drive(CreateDriveOperation),
}

impl Operation for DriveKernelOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source(value) => value.start(),
            Self::Drive(value) => value.start(),
        }
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::Source(value) => value.resume(input),
            Self::Drive(value) => value.resume(input),
        }
    }
    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source(value) => value.advance(),
            Self::Drive(value) => value.advance(),
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Source(value) => value.cancel(),
            Self::Drive(value) => value.cancel(),
        }
    }
}

pub(super) type DriveScheduler = FixedScheduler<
    OperationDriver<DriveKernelOperation, PORTS>,
    FixedValueStore<3, { REQUEST_BYTES as usize }>,
    FixedSignLog<SIGNS>,
    3,
    2,
    PORTS,
    2,
    3,
    2,
    3,
    1,
>;

pub(super) fn prepare_drive_scheduler(
    linear: &[u8; conduit_core::SCALAR_ENCODED_LEN],
    angular: &[u8; conduit_core::SCALAR_ENCODED_LEN],
) -> Result<DriveScheduler, &'static str> {
    let mut values = FixedValueStore::<3, { REQUEST_BYTES as usize }>::new(VALUE_BYTES)
        .map_err(|_| "drive value admission failed")?;
    let linear_value = values
        .store(linear)
        .map_err(|_| "linear admission failed")?;
    let angular_value = values
        .store(angular)
        .map_err(|_| "angular admission failed")?;
    let mut packed = [0_u8; REQUEST_BYTES as usize];
    packed[..conduit_core::SCALAR_ENCODED_LEN].copy_from_slice(linear);
    packed[conduit_core::SCALAR_ENCODED_LEN..].copy_from_slice(angular);
    let request_value = values
        .store(&packed)
        .map_err(|_| "request admission failed")?;
    let request = BoundedValueRef::new(request_value, REQUEST_BYTES)
        .map_err(|_| "drive request bound invalid")?;

    let mut routes = FixedRoutes::<3, 2>::new(PORTS as u16);
    for (source, cord, sink_port, slot) in [
        (LINEAR_NODE, CordId(0), PortId(0), 0),
        (ANGULAR_NODE, CordId(1), PortId(1), 1),
    ] {
        routes
            .install(
                source,
                PortId(0),
                RouteRange {
                    start: slot,
                    len: 1,
                },
                &[RouteTarget {
                    cord,
                    sink: conduit_kernel::CordEndpoint::local(DRIVE_NODE, sink_port),
                }],
            )
            .map_err(|_| "drive route admission failed")?;
    }
    routes.seal().map_err(|_| "drive route seal failed")?;

    let mut bindings = FixedHostOperationBindings::<3>::new(1);
    bindings
        .install(
            DRIVE_NODE,
            HostOperationBinding {
                operation: DRIVE_OPERATION,
                maximum_input_bytes: REQUEST_BYTES,
                maximum_output_bytes: 0,
            },
        )
        .map_err(|_| "drive Host operation admission failed")?;
    bindings
        .seal()
        .map_err(|_| "drive Host operation seal failed")?;
    let signs = FixedSignLog::new((SIGNS * core::mem::size_of::<KernelEvent>()) as u32)
        .map_err(|_| "drive Sign admission failed")?;
    FixedScheduler::new_with_host_operations(
        [
            NodeSpec {
                input_cords: [None, None],
                maximum_step_work: 2,
            },
            NodeSpec {
                input_cords: [None, None],
                maximum_step_work: 2,
            },
            NodeSpec {
                input_cords: [Some(CordId(0)), Some(CordId(1))],
                maximum_step_work: 2,
            },
        ],
        [
            CordSpec::local(
                CordId(0),
                (LINEAR_NODE, PortId(0)),
                (DRIVE_NODE, PortId(0)),
                CordCapacity {
                    slot_start: 0,
                    item_capacity: 1,
                    byte_capacity: SCALAR_BYTES,
                },
            ),
            CordSpec::local(
                CordId(1),
                (ANGULAR_NODE, PortId(0)),
                (DRIVE_NODE, PortId(1)),
                CordCapacity {
                    slot_start: 1,
                    item_capacity: 1,
                    byte_capacity: SCALAR_BYTES,
                },
            ),
        ],
        routes,
        bindings,
        [
            OperationDriver::new(DriveKernelOperation::Source(ScalarSource {
                value: linear_value,
                emitted: false,
            }))
            .map_err(|_| "linear source preparation failed")?,
            OperationDriver::new(DriveKernelOperation::Source(ScalarSource {
                value: angular_value,
                emitted: false,
            }))
            .map_err(|_| "angular source preparation failed")?,
            OperationDriver::new(DriveKernelOperation::Drive(CreateDriveOperation {
                request,
                seen: [false; 2],
                pending: false,
                admitted: false,
            }))
            .map_err(|_| "drive operation preparation failed")?,
        ],
        values,
        signs,
    )
    .map_err(|_| "drive kernel preparation failed")
}

pub(super) fn drive_is_admitted(scheduler: &DriveScheduler) -> bool {
    matches!(
        scheduler.drivers()[usize::from(DRIVE_NODE.0)].operation(),
        DriveKernelOperation::Drive(CreateDriveOperation { admitted: true, .. })
    )
}
