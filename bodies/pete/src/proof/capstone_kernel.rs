//! Fixed production-kernel topology for the sealed five-Gear capstone.

use super::capstone_operations::{CurrentSelector, DriveSink};
use conduit_kernel::{
    scheduler::{CordCapacity, CordSpec, FixedScheduler, NodeSpec, OperationDriver},
    BoundedValueRef, CordId, FixedHostOperationBindings, FixedRoutes, FixedSignLog,
    FixedValueStore, HostOperationBinding, HostOperationDisposition, HostOperationId, KernelEvent,
    NodeId, Operation, OperationAction, OperationInput, PortId, RequestId, RouteRange, RouteTarget,
    ValueRef, ValueStorage,
};

pub(super) const OBSERVATION_NODE: NodeId = NodeId(0);
const REQUESTED_NODE: NodeId = NodeId(1);
const STOPPED_NODE: NodeId = NodeId(2);
const SELECT_NODE: NodeId = NodeId(3);
pub(super) const DRIVE_NODE: NodeId = NodeId(4);
pub(super) const OBSERVATION_REQUEST: RequestId = RequestId(1);
const OPERATION: HostOperationId = HostOperationId(0);
const PORTS: usize = 3;
const SIGNS: usize = 256;
const SCALAR_BYTES: u32 = conduit_core::SCALAR_ENCODED_LEN as u32;
const VALUE_BYTES: u32 = 8 * SCALAR_BYTES;

#[derive(Clone, Copy)]
pub(super) struct ObservationSource {
    empty: ValueRef,
    pending: bool,
    emitted: bool,
}

impl Operation for ObservationSource {
    fn start(&mut self) -> OperationAction {
        self.pending = true;
        OperationAction::RequestHostOperation {
            request: OBSERVATION_REQUEST,
            operation: OPERATION,
            input: BoundedValueRef::new(self.empty, 0).expect("empty request is exact"),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted {
                request: OBSERVATION_REQUEST,
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
pub(super) struct VelocitySource {
    linear: ValueRef,
    angular: Option<ValueRef>,
    phase: u8,
}

impl Operation for VelocitySource {
    fn start(&mut self) -> OperationAction {
        self.phase = 1;
        OperationAction::Emit {
            port: PortId(0),
            value: self.linear,
        }
    }

    fn resume(&mut self, _input: OperationInput) -> OperationAction {
        invalid(3)
    }

    fn advance(&mut self) -> OperationAction {
        match (self.phase, self.angular) {
            (1, Some(value)) => {
                self.phase = 2;
                OperationAction::Emit {
                    port: PortId(1),
                    value,
                }
            }
            (1 | 2, _) => {
                self.phase = 0;
                OperationAction::Complete
            }
            _ => invalid(4),
        }
    }

    fn cancel(&mut self) {
        self.phase = 0;
    }
}

#[derive(Clone, Copy)]
pub(super) enum CapstoneOperation {
    Observation(ObservationSource),
    Velocity(VelocitySource),
    Select(CurrentSelector),
    Drive(DriveSink),
}

impl Operation for CapstoneOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Observation(value) => value.start(),
            Self::Velocity(value) => value.start(),
            Self::Select(value) => value.start(),
            Self::Drive(value) => value.start(),
        }
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::Observation(value) => value.resume(input),
            Self::Velocity(value) => value.resume(input),
            Self::Select(value) => value.resume(input),
            Self::Drive(value) => value.resume(input),
        }
    }
    fn resume_value(&mut self, port: PortId, value: ValueRef, bytes: &[u8]) -> OperationAction {
        match self {
            Self::Select(operation) => operation.resume_value(port, value, bytes),
            Self::Drive(operation) => operation.resume_value(port, value, bytes),
            _ => self.resume(OperationInput::Value { port, value }),
        }
    }
    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Observation(value) => value.advance(),
            Self::Velocity(value) => value.advance(),
            Self::Select(_) | Self::Drive(_) => OperationAction::Await,
        }
    }
    fn retains_resumed_value(&self) -> bool {
        match self {
            Self::Drive(value) => value.retains_resumed_value(),
            _ => false,
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Observation(value) => value.cancel(),
            Self::Velocity(value) => value.cancel(),
            Self::Select(value) => value.cancel(),
            Self::Drive(value) => value.cancel(),
        }
    }
}

pub(super) type CapstoneScheduler = FixedScheduler<
    OperationDriver<CapstoneOperation, PORTS>,
    FixedValueStore<8, { SCALAR_BYTES as usize }>,
    FixedSignLog<SIGNS>,
    5,
    5,
    PORTS,
    5,
    15,
    5,
    5,
    2,
>;

pub(super) fn prepare_scheduler(
    requested_linear: &[u8; conduit_core::SCALAR_ENCODED_LEN],
    requested_angular: &[u8; conduit_core::SCALAR_ENCODED_LEN],
    stopped_linear: &[u8; conduit_core::SCALAR_ENCODED_LEN],
) -> Result<CapstoneScheduler, &'static str> {
    let mut values = FixedValueStore::<8, { SCALAR_BYTES as usize }>::new(VALUE_BYTES)
        .map_err(|_| "capstone value admission failed")?;
    let empty = values.store(&[]).map_err(|_| "empty admission failed")?;
    let requested_linear = values
        .store(requested_linear)
        .map_err(|_| "requested linear admission failed")?;
    let requested_angular = values
        .store(requested_angular)
        .map_err(|_| "requested angular admission failed")?;
    let stopped_linear = values
        .store(stopped_linear)
        .map_err(|_| "stopped linear admission failed")?;

    let route_specs = [
        (OBSERVATION_NODE, PortId(0), SELECT_NODE, PortId(0)),
        (REQUESTED_NODE, PortId(0), SELECT_NODE, PortId(1)),
        (STOPPED_NODE, PortId(0), SELECT_NODE, PortId(2)),
        (SELECT_NODE, PortId(0), DRIVE_NODE, PortId(0)),
        (REQUESTED_NODE, PortId(1), DRIVE_NODE, PortId(1)),
    ];
    let mut routes = FixedRoutes::<15, 5>::new(PORTS as u16);
    for (index, (source, source_port, sink, sink_port)) in route_specs.into_iter().enumerate() {
        routes
            .install(
                source,
                source_port,
                RouteRange {
                    start: index as u16,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(index as u16),
                    sink: conduit_kernel::CordEndpoint::local(sink, sink_port),
                }],
            )
            .map_err(|_| "capstone route admission failed")?;
    }
    routes.seal().map_err(|_| "capstone route seal failed")?;

    let mut bindings = FixedHostOperationBindings::<5>::new(1);
    bindings
        .install(
            OBSERVATION_NODE,
            HostOperationBinding {
                operation: OPERATION,
                maximum_input_bytes: 0,
                maximum_output_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
            },
        )
        .map_err(|_| "observation Host operation admission failed")?;
    bindings
        .install(
            DRIVE_NODE,
            HostOperationBinding {
                operation: OPERATION,
                maximum_input_bytes: 2 * SCALAR_BYTES,
                maximum_output_bytes: 0,
            },
        )
        .map_err(|_| "drive Host operation admission failed")?;
    bindings.seal().map_err(|_| "Host operation seal failed")?;

    let signs = FixedSignLog::new((SIGNS * core::mem::size_of::<KernelEvent>()) as u32)
        .map_err(|_| "capstone Sign admission failed")?;
    let node_specs = [
        NodeSpec {
            input_cords: [None; PORTS],
            maximum_step_work: 2,
        },
        NodeSpec {
            input_cords: [None; PORTS],
            maximum_step_work: 2,
        },
        NodeSpec {
            input_cords: [None; PORTS],
            maximum_step_work: 2,
        },
        NodeSpec {
            input_cords: [Some(CordId(0)), Some(CordId(1)), Some(CordId(2))],
            maximum_step_work: 3,
        },
        NodeSpec {
            input_cords: [Some(CordId(3)), Some(CordId(4)), None],
            maximum_step_work: 2,
        },
    ];
    let cords = route_specs.map(|(source, source_port, sink, sink_port)| {
        let cord = if source == OBSERVATION_NODE { 0 } else { 1 };
        CordSpec::local(
            CordId(
                route_specs
                    .iter()
                    .position(|candidate| candidate == &(source, source_port, sink, sink_port))
                    .expect("route is in fixed table") as u16,
            ),
            (source, source_port),
            (sink, sink_port),
            CordCapacity {
                slot_start: route_specs
                    .iter()
                    .position(|candidate| candidate == &(source, source_port, sink, sink_port))
                    .expect("route is in fixed table") as u16,
                item_capacity: 1,
                byte_capacity: if cord == 0 { 1 } else { SCALAR_BYTES },
            },
        )
    });
    FixedScheduler::new_with_host_operations(
        node_specs,
        cords,
        routes,
        bindings,
        [
            OperationDriver::new(CapstoneOperation::Observation(ObservationSource {
                empty,
                pending: false,
                emitted: false,
            }))
            .map_err(|_| "observation operation preparation failed")?,
            OperationDriver::new(CapstoneOperation::Velocity(VelocitySource {
                linear: requested_linear,
                angular: Some(requested_angular),
                phase: 0,
            }))
            .map_err(|_| "requested operation preparation failed")?,
            OperationDriver::new(CapstoneOperation::Velocity(VelocitySource {
                linear: stopped_linear,
                angular: None,
                phase: 0,
            }))
            .map_err(|_| "stopped operation preparation failed")?,
            OperationDriver::new(CapstoneOperation::Select(CurrentSelector {
                selector: None,
                candidates: [None; 2],
                closed: [false; 3],
            }))
            .map_err(|_| "selector operation preparation failed")?,
            OperationDriver::new(CapstoneOperation::Drive(DriveSink {
                linear: None,
                angular_is_zero: false,
                closed: [false; 2],
                pending: false,
                completed: false,
                retain_resumed: false,
            }))
            .map_err(|_| "drive operation preparation failed")?,
        ],
        values,
        signs,
    )
    .map_err(|_| "capstone kernel preparation failed")
}

const fn invalid(detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}
