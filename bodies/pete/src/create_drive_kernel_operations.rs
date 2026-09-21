use conduit_kernel::{
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, NodeSpec, StepInputBytes, StepIo, StepOperation,
        StepOutcome,
    },
    BoundedValueRef, CordId, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostCallBinding, HostCallDisposition, HostCallId, KernelEvent, NodeId, PortId, RequestId,
    RouteRange, RouteTarget, ValueRef, ValueStorage,
};

const LINEAR_NODE: NodeId = NodeId(0);
const ANGULAR_NODE: NodeId = NodeId(1);
pub(super) const DRIVE_NODE: NodeId = NodeId(2);
pub(super) const DRIVE_REQUEST: RequestId = RequestId(1);
const DRIVE_OPERATION: HostCallId = HostCallId(0);
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

#[derive(Clone, Copy)]
pub(super) struct CreateDriveOperation {
    request: BoundedValueRef,
    seen: [bool; 2],
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
pub(super) enum DriveKernelOperation {
    Source(ScalarSource),
    Drive(CreateDriveOperation),
}

impl StepOperation<PORTS> for DriveKernelOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source(source) => {
                if source.emitted {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), source.value).expect("ready drive Cord");
                source.emitted = true;
                StepOutcome::Complete
            }
            Self::Drive(operation) => {
                if let Some((request, outcome)) = io.host_completion() {
                    if request != DRIVE_REQUEST || !operation.pending || outcome.output.is_some() {
                        return invalid(3);
                    }
                    io.consume_host_completion()
                        .expect("observed drive Host Call completion");
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
                for index in 0..operation.seen.len() {
                    if !operation.seen[index] && io.input(PortId(index as u16)).is_some() {
                        io.consume(PortId(index as u16))
                            .expect("present drive input");
                        operation.seen[index] = true;
                        if operation.seen.into_iter().all(|seen| seen) {
                            io.request_host_call(DRIVE_REQUEST, DRIVE_OPERATION, operation.request)
                                .expect("planned drive Host Call");
                            operation.pending = true;
                        }
                        return StepOutcome::Progress;
                    }
                }
                StepOutcome::Await
            }
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Source(value) => value.emitted = false,
            Self::Drive(value) => {
                value.pending = false;
                value.admitted = false;
            }
        }
    }
}

pub(super) type DriveScheduler = FixedScheduler<
    DriveKernelOperation,
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

    let mut bindings = FixedHostCallBindings::<3>::new(1);
    bindings
        .install(
            DRIVE_NODE,
            HostCallBinding {
                call: DRIVE_OPERATION,
                maximum_input_bytes: REQUEST_BYTES,
                maximum_output_bytes: 0,
            },
        )
        .map_err(|_| "drive Host Call admission failed")?;
    bindings.seal().map_err(|_| "drive Host Call seal failed")?;
    let signs = FixedSignLog::new((SIGNS * core::mem::size_of::<KernelEvent>()) as u32)
        .map_err(|_| "drive Sign admission failed")?;
    FixedScheduler::new_with_host_calls(
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
                    pressure_policy: Default::default(),
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
                    pressure_policy: Default::default(),
                },
            ),
        ],
        routes,
        bindings,
        [
            DriveKernelOperation::Source(ScalarSource {
                value: linear_value,
                emitted: false,
            }),
            DriveKernelOperation::Source(ScalarSource {
                value: angular_value,
                emitted: false,
            }),
            DriveKernelOperation::Drive(CreateDriveOperation {
                request,
                seen: [false; 2],
                pending: false,
                admitted: false,
            }),
        ],
        values,
        signs,
    )
    .map_err(|_| "drive kernel preparation failed")
}

pub(super) fn drive_is_admitted(scheduler: &DriveScheduler) -> bool {
    matches!(
        &scheduler.drivers()[usize::from(DRIVE_NODE.0)],
        DriveKernelOperation::Drive(CreateDriveOperation { admitted: true, .. })
    )
}
