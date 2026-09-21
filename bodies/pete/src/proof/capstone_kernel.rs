//! Fixed production-kernel topology for the sealed five-Gear capstone.

use super::capstone_operations::{CurrentSelector, DriveSink};
use conduit_kernel::{
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, NodeSpec, StepInputBytes, StepIo, StepOperation,
        StepOutcome,
    },
    BoundedValueRef, CordId, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostCallBinding, HostCallDisposition, HostCallId, KernelEvent, NodeId, PortId, RequestId,
    RouteRange, RouteTarget, ValueRef, ValueStorage,
};

pub(super) const OBSERVATION_NODE: NodeId = NodeId(0);
const REQUESTED_NODE: NodeId = NodeId(1);
const STOPPED_NODE: NodeId = NodeId(2);
const SELECT_NODE: NodeId = NodeId(3);
pub(super) const DRIVE_NODE: NodeId = NodeId(4);
pub(super) const OBSERVATION_REQUEST: RequestId = RequestId(1);
const OPERATION: HostCallId = HostCallId(0);
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

#[derive(Clone, Copy)]
pub(super) struct VelocitySource {
    linear: ValueRef,
    angular: Option<ValueRef>,
    phase: u8,
}

#[derive(Clone, Copy)]
pub(super) enum CapstoneOperation {
    Observation(ObservationSource),
    Velocity(VelocitySource),
    Select(CurrentSelector),
    Drive(DriveSink),
}

impl StepOperation<PORTS> for CapstoneOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Observation(source) => {
                if let Some((request, outcome)) = io.host_completion() {
                    if request != OBSERVATION_REQUEST || !source.pending {
                        return invalid(1);
                    }
                    match (outcome.disposition, outcome.output, outcome.failure) {
                        (HostCallDisposition::Completed, output, None) => {
                            if output.is_some() && !io.output_ready(PortId(0)) {
                                return StepOutcome::Await;
                            }
                            io.consume_host_completion()
                                .expect("observed capstone observation completion");
                            source.pending = false;
                            if let Some(output) = output {
                                io.send(PortId(0), output.value)
                                    .expect("ready capstone observation Cord");
                                source.emitted = true;
                            }
                            return StepOutcome::Complete;
                        }
                        (HostCallDisposition::Failed, _, Some(failure)) => {
                            io.consume_host_completion()
                                .expect("observed failed capstone observation");
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
                io.request_host_call(
                    OBSERVATION_REQUEST,
                    OPERATION,
                    BoundedValueRef::new(source.empty, 0).expect("empty request is exact"),
                )
                .expect("planned capstone observation Host Call");
                source.pending = true;
                StepOutcome::Progress
            }
            Self::Velocity(source) => match source.phase {
                0 => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.send(PortId(0), source.linear)
                        .expect("ready capstone linear Cord");
                    source.phase = 1;
                    StepOutcome::Progress
                }
                1 => {
                    let Some(value) = source.angular else {
                        return StepOutcome::Complete;
                    };
                    if !io.output_ready(PortId(1)) {
                        return StepOutcome::Await;
                    }
                    io.send(PortId(1), value)
                        .expect("ready capstone angular Cord");
                    source.phase = 2;
                    StepOutcome::Complete
                }
                2 => StepOutcome::Complete,
                _ => invalid(4),
            },
            Self::Select(value) => value.step(io, bytes),
            Self::Drive(value) => value.step(io, bytes),
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Observation(value) => {
                value.pending = false;
                value.emitted = false;
            }
            Self::Velocity(value) => value.phase = 0,
            Self::Select(value) => value.cancel(),
            Self::Drive(value) => value.cancel(),
        }
    }
}

pub(super) type CapstoneScheduler = FixedScheduler<
    CapstoneOperation,
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

    let mut bindings = FixedHostCallBindings::<5>::new(1);
    bindings
        .install(
            OBSERVATION_NODE,
            HostCallBinding {
                operation: OPERATION,
                maximum_input_bytes: 0,
                maximum_output_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
            },
        )
        .map_err(|_| "observation Host Call admission failed")?;
    bindings
        .install(
            DRIVE_NODE,
            HostCallBinding {
                operation: OPERATION,
                maximum_input_bytes: 2 * SCALAR_BYTES,
                maximum_output_bytes: 0,
            },
        )
        .map_err(|_| "drive Host Call admission failed")?;
    bindings.seal().map_err(|_| "Host Call seal failed")?;

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
            maximum_step_work: 3,
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
                pressure_policy: Default::default(),
            },
        )
    });
    FixedScheduler::new_with_host_calls(
        node_specs,
        cords,
        routes,
        bindings,
        [
            CapstoneOperation::Observation(ObservationSource {
                empty,
                pending: false,
                emitted: false,
            }),
            CapstoneOperation::Velocity(VelocitySource {
                linear: requested_linear,
                angular: Some(requested_angular),
                phase: 0,
            }),
            CapstoneOperation::Velocity(VelocitySource {
                linear: stopped_linear,
                angular: None,
                phase: 0,
            }),
            CapstoneOperation::Select(CurrentSelector {
                selector: None,
                candidates: [None; 2],
                closed: [false; 3],
            }),
            CapstoneOperation::Drive(DriveSink {
                angular_is_zero: false,
                closed: [false; 2],
                pending: false,
                completed: false,
            }),
        ],
        values,
        signs,
    )
    .map_err(|_| "capstone kernel preparation failed")
}

const fn invalid(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}
