use super::{
    AdapterTransaction, CordCapacity, CordSpec, FixedScheduler, NodeSpec, OperationDriver,
    RemoteIngressOutcome, SchedulerError, SchedulerStatus, StepInputBytes, StepIo, StepOperation,
    StepOutcome,
};
use crate::{
    BoundedValueRef, CanonicalValue, CordId, Failure, FailureCode, FixedHostOperationBindings,
    FixedRoutes, FixedSignLog, FixedValueStore, HostOperationBinding, HostOperationDisposition,
    HostOperationId, HostOperationOutcome, KernelEventKind, NodeId, Operation, OperationAction,
    OperationInput, PortId, ProtocolError, RemoteEndpointId, RequestId, RouteRange, RouteTarget,
    SignQuery, SignSink, ValueRef, ValueStorage,
};

mod host_alias;
mod host_cancellation;

const NODES: usize = 6;
const CORDS: usize = 5;
const PORTS: usize = 2;

#[test]
fn derived_output_staging_is_bounded_independently_of_port_capacity() {
    assert!(core::mem::size_of::<AdapterTransaction<32>>() < 1_024);
}

#[derive(Clone, Copy, Debug)]
enum Driver {
    Source {
        values: [Option<ValueRef>; 4],
        next: usize,
    },
    Tee,
    Filter,
    Latest {
        held: Option<ValueRef>,
    },
    Sink {
        seen: [Option<ValueRef>; 4],
        len: usize,
        stall: bool,
    },
    BlockedSink {
        cancelled: bool,
    },
}

impl StepOperation<PORTS> for Driver {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Source { values, next } => {
                let Some(value) = values.get(*next).copied().flatten() else {
                    return StepOutcome::Complete;
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), value).unwrap();
                *next += 1;
                StepOutcome::Progress
            }
            Self::Tee => {
                if let Some(value) = io.input(PortId(0)) {
                    if !io.output_ready(PortId(0)) || !io.output_ready(PortId(1)) {
                        return StepOutcome::Await;
                    }
                    io.consume(PortId(0)).unwrap();
                    io.send(PortId(0), value).unwrap();
                    io.send(PortId(1), value).unwrap();
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::Filter => {
                if let Some(value) = io.input(PortId(0)) {
                    if value.slot % 2 == 0 && !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume(PortId(0)).unwrap();
                    if value.slot % 2 == 0 {
                        io.send(PortId(0), value).unwrap();
                    }
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::Latest { held } => {
                if let Some(value) = io.input(PortId(0)) {
                    if let Some(previous) = held.take() {
                        io.discard(previous).unwrap();
                    }
                    io.take_input(PortId(0)).unwrap();
                    *held = Some(value);
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    let Some(latest) = held.take() else {
                        io.consume_closed(PortId(0)).unwrap();
                        return StepOutcome::Complete;
                    };
                    if !io.output_ready(PortId(0)) {
                        *held = Some(latest);
                        return StepOutcome::Await;
                    }
                    io.consume_closed(PortId(0)).unwrap();
                    io.send(PortId(0), latest).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::Sink { seen, len, stall } => {
                if *stall && io.input(PortId(0)).is_some() {
                    *stall = false;
                    io.exhaust_work_budget();
                    return StepOutcome::Yield;
                }
                if let Some(value) = io.input(PortId(0)) {
                    io.consume(PortId(0)).unwrap();
                    seen[*len] = Some(value);
                    *len += 1;
                    *stall = true;
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::BlockedSink { .. } => StepOutcome::Await,
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Latest { held } => *held = None,
            Self::BlockedSink { cancelled } => *cancelled = true,
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum HostDriver {
    Source {
        value: Option<ValueRef>,
    },
    Effect {
        requested: bool,
        cancelled: bool,
        repeat_request: bool,
    },
    Sink {
        seen: Option<ValueRef>,
    },
    Transform {
        input: ValueRef,
        phase: u8,
    },
    DecodedSink {
        seen: Option<[u8; 5]>,
    },
}

impl StepOperation<PORTS> for HostDriver {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Source { value } => {
                let Some(current) = *value else {
                    return StepOutcome::Complete;
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), current).unwrap();
                *value = None;
                StepOutcome::Progress
            }
            Self::Effect { requested, .. } if !*requested => {
                let Some(input) = io.input(PortId(0)) else {
                    return StepOutcome::Await;
                };
                io.consume(PortId(0)).unwrap();
                io.request_host_operation(
                    RequestId(7),
                    HostOperationId(0),
                    BoundedValueRef::new(input, 4).unwrap(),
                )
                .unwrap();
                *requested = true;
                StepOutcome::Progress
            }
            Self::Effect { repeat_request, .. } => {
                let Some((request, outcome)) = io.host_completion() else {
                    return StepOutcome::Await;
                };
                assert_eq!(request, RequestId(7));
                let output = outcome.output.expect("host output").value;
                if *repeat_request {
                    io.consume_host_completion().unwrap();
                    io.request_host_operation(
                        request,
                        HostOperationId(0),
                        BoundedValueRef::new(output, 4).unwrap(),
                    )
                    .unwrap();
                    return StepOutcome::Progress;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.consume_host_completion().unwrap();
                io.send(PortId(0), output).unwrap();
                StepOutcome::Complete
            }
            Self::Sink { seen } => {
                if let Some(value) = io.input(PortId(0)) {
                    io.consume(PortId(0)).unwrap();
                    *seen = Some(value);
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::Transform { input, phase } => match *phase {
                0 => {
                    io.request_host_operation(
                        RequestId(1),
                        HostOperationId(0),
                        BoundedValueRef::new(*input, 0).unwrap(),
                    )
                    .unwrap();
                    *phase = 1;
                    StepOutcome::Progress
                }
                1 => {
                    let Some((RequestId(1), outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    assert_eq!(input_bytes.host_output(), Some([0x90, 60, 100].as_slice()));
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    assert_eq!(outcome.disposition, HostOperationDisposition::Completed);
                    io.consume_host_completion().unwrap();
                    io.send_canonical(PortId(0), CanonicalValue::new(&[9, 8, 7, 6, 5]).unwrap())
                        .unwrap();
                    io.request_host_operation(
                        RequestId(2),
                        HostOperationId(0),
                        BoundedValueRef::new(*input, 0).unwrap(),
                    )
                    .unwrap();
                    *phase = 2;
                    StepOutcome::Progress
                }
                2 => {
                    let Some((RequestId(2), _)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    assert_eq!(input_bytes.host_output(), Some([0x80, 60, 0].as_slice()));
                    io.consume_host_completion().unwrap();
                    io.discard(*input).unwrap();
                    *phase = 3;
                    StepOutcome::Complete
                }
                _ => StepOutcome::Complete,
            },
            Self::DecodedSink { seen } => {
                if io.input(PortId(0)).is_some() {
                    *seen = Some(input_bytes.input(PortId(0)).unwrap().try_into().unwrap());
                    io.consume(PortId(0)).unwrap();
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
        }
    }

    fn cancel(&mut self) {
        if let Self::Effect { cancelled, .. } = self {
            *cancelled = true;
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum JoinDriver {
    Source { value: Option<ValueRef> },
    Join,
    Sink { seen: Option<ValueRef> },
}

impl StepOperation<PORTS> for JoinDriver {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Source { value } => {
                let Some(current) = *value else {
                    return StepOutcome::Complete;
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), current).unwrap();
                *value = None;
                StepOutcome::Progress
            }
            Self::Join => {
                let (Some(left), Some(_right)) = (io.input(PortId(0)), io.input(PortId(1))) else {
                    return if io.input_closed(PortId(0)) && io.input_closed(PortId(1)) {
                        io.consume_closed(PortId(0)).unwrap();
                        io.consume_closed(PortId(1)).unwrap();
                        StepOutcome::Complete
                    } else {
                        StepOutcome::Await
                    };
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.consume(PortId(0)).unwrap();
                io.consume(PortId(1)).unwrap();
                io.send(PortId(0), left).unwrap();
                StepOutcome::Progress
            }
            Self::Sink { seen } => {
                if let Some(value) = io.input(PortId(0)) {
                    io.consume(PortId(0)).unwrap();
                    *seen = Some(value);
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum AdapterOperation {
    Source { value: ValueRef, advanced: bool },
    Tee { value: Option<ValueRef>, phase: u8 },
    HostEffect,
    Sink { seen: Option<ValueRef> },
}

impl Operation for AdapterOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source { value, .. } => OperationAction::Emit {
                port: PortId(0),
                value: *value,
            },
            _ => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Tee { value, phase },
                OperationInput::Value {
                    port: PortId(0),
                    value: input,
                },
            ) => {
                *value = Some(input);
                *phase = 1;
                OperationAction::Emit {
                    port: PortId(0),
                    value: input,
                }
            }
            (Self::Tee { .. }, OperationInput::Closed { port: PortId(0) }) => {
                OperationAction::Complete
            }
            (
                Self::HostEffect,
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) => OperationAction::RequestHostOperation {
                request: RequestId(11),
                operation: HostOperationId(0),
                input: BoundedValueRef::new(value, 4).unwrap(),
            },
            (
                Self::HostEffect,
                OperationInput::HostOperationCompleted {
                    request: RequestId(11),
                    outcome,
                },
            ) => OperationAction::Emit {
                port: PortId(0),
                value: outcome.output.expect("adapter host output").value,
            },
            (Self::HostEffect, OperationInput::Closed { port: PortId(0) }) => {
                OperationAction::Complete
            }
            (
                Self::Sink { seen },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) => {
                *seen = Some(value);
                OperationAction::Await
            }
            (Self::Sink { .. }, OperationInput::Closed { port: PortId(0) }) => {
                OperationAction::Complete
            }
            _ => OperationAction::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 91,
            }),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source { advanced, .. } if !*advanced => {
                *advanced = true;
                OperationAction::Complete
            }
            Self::Tee {
                value: Some(value),
                phase,
            } if *phase == 1 => {
                *phase = 2;
                OperationAction::Emit {
                    port: PortId(1),
                    value: *value,
                }
            }
            Self::Tee { value, phase } if *phase == 2 => {
                *value = None;
                *phase = 0;
                OperationAction::Await
            }
            _ => OperationAction::Await,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ConformanceOperation {
    Tick {
        request: u32,
        input: ValueRef,
        emitted: bool,
    },
    Tee {
        value: Option<ValueRef>,
        phase: u8,
    },
    Filter,
    Latest {
        held: Option<ValueRef>,
        released: Option<ValueRef>,
        retain_resumed: bool,
        closing: bool,
    },
    Show {
        seen: [Option<ValueRef>; 4],
        len: usize,
    },
}

impl Operation for ConformanceOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Tick { request, input, .. } => OperationAction::RequestHostOperation {
                request: RequestId(*request),
                operation: HostOperationId(0),
                input: BoundedValueRef::new(*input, 4).unwrap(),
            },
            _ => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Tick {
                    request,
                    input,
                    emitted,
                },
                OperationInput::HostOperationCompleted {
                    request: completed,
                    outcome,
                },
            ) if completed == RequestId(*request) => {
                let value = outcome.output.expect("tick host output").value;
                *input = value;
                *emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            (
                Self::Tee { value, phase },
                OperationInput::Value {
                    port: PortId(0),
                    value: input,
                },
            ) => {
                *value = Some(input);
                *phase = 1;
                OperationAction::Emit {
                    port: PortId(0),
                    value: input,
                }
            }
            (Self::Tee { .. }, OperationInput::Closed { port: PortId(0) }) => {
                OperationAction::Complete
            }
            (
                Self::Filter,
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) => {
                if value.byte_len == 1 {
                    OperationAction::Emit {
                        port: PortId(0),
                        value,
                    }
                } else {
                    OperationAction::Await
                }
            }
            (Self::Filter, OperationInput::Closed { port: PortId(0) }) => OperationAction::Complete,
            (
                Self::Latest {
                    held,
                    released,
                    retain_resumed,
                    ..
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) => {
                *released = held.replace(value);
                *retain_resumed = true;
                OperationAction::Await
            }
            (
                Self::Latest {
                    held,
                    retain_resumed,
                    closing,
                    ..
                },
                OperationInput::Closed { port: PortId(0) },
            ) => {
                *retain_resumed = false;
                let Some(value) = held.take() else {
                    return OperationAction::Complete;
                };
                *closing = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            (
                Self::Show { seen, len },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) => {
                seen[*len] = Some(value);
                *len += 1;
                OperationAction::Await
            }
            (Self::Show { .. }, OperationInput::Closed { port: PortId(0) }) => {
                OperationAction::Complete
            }
            _ => OperationAction::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 92,
            }),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Tick {
                request,
                input,
                emitted,
            } if *emitted => {
                *emitted = false;
                if *request == 4 {
                    OperationAction::Complete
                } else {
                    *request += 1;
                    OperationAction::RequestHostOperation {
                        request: RequestId(*request),
                        operation: HostOperationId(0),
                        input: BoundedValueRef::new(*input, 4).unwrap(),
                    }
                }
            }
            Self::Tee {
                value: Some(value),
                phase,
            } if *phase == 1 => {
                *phase = 2;
                OperationAction::Emit {
                    port: PortId(1),
                    value: *value,
                }
            }
            Self::Tee { value, phase } if *phase == 2 => {
                *value = None;
                *phase = 0;
                OperationAction::Await
            }
            Self::Latest { closing, .. } if *closing => {
                *closing = false;
                OperationAction::Complete
            }
            _ => OperationAction::Await,
        }
    }

    fn retains_resumed_value(&self) -> bool {
        matches!(
            self,
            Self::Latest {
                retain_resumed: true,
                ..
            }
        )
    }

    fn take_released_value(&mut self) -> Option<ValueRef> {
        match self {
            Self::Latest { released, .. } => released.take(),
            _ => None,
        }
    }

    fn cancel(&mut self) {
        if let Self::Latest {
            held,
            released,
            retain_resumed,
            ..
        } = self
        {
            *held = None;
            *released = None;
            *retain_resumed = false;
        }
    }
}

#[test]
fn multi_value_port_graph_handles_pressure_closure_and_uneven_consumers() {
    let event_charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let normalized = execute(
        FixedValueStore::<8, 4>::new(16).unwrap(),
        FixedSignLog::<128>::new(event_charge * 128).unwrap(),
    );
    assert_eq!(normalized.show_a_len, 2);
    assert_eq!(normalized.show_a[..2], [0, 2]);
    assert_eq!(normalized.show_b_len, 1);
    assert_eq!(normalized.show_b[0], 3);
    assert_eq!(normalized.used_items, 0);
    assert!(normalized.saw_input_closed);
}

#[test]
fn public_operation_state_machine_drives_atomic_tee_scheduler_step() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let normalized = execute_operation_adapter(
        FixedValueStore::<4, 4>::new(16).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    assert_eq!(normalized.left, 0);
    assert_eq!(normalized.right, 0);
    assert_eq!(normalized.used_items, 0);
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_and_fixed_operation_adapter_vectors_match() {
    use crate::{HostedSignLog, HostedValueStore};

    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let fixed = execute_operation_adapter(
        FixedValueStore::<4, 4>::new(16).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    let hosted = execute_operation_adapter(
        HostedValueStore::new(4, 4, 16).unwrap(),
        HostedSignLog::new(64, charge * 64).unwrap(),
    );
    assert_eq!(fixed, hosted);
}

#[test]
fn operation_adapter_routes_correlated_host_completion() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let mut values = FixedValueStore::<4, 4>::new(16).unwrap();
    let input = values.store(&[1]).unwrap();
    let mut routes = FixedRoutes::<6, 2>::new(2);
    for (source, cord_id, sink) in [(0, 0, 1), (1, 1, 2)] {
        routes
            .install(
                NodeId(source),
                PortId(0),
                RouteRange {
                    start: cord_id,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(cord_id),
                    sink: crate::CordEndpoint::local(NodeId(sink), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut bindings = FixedHostOperationBindings::<3>::new(1);
    bindings
        .install(
            NodeId(1),
            HostOperationBinding {
                operation: HostOperationId(0),
                maximum_input_bytes: 4,
                maximum_output_bytes: 4,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    let mut scheduler =
        FixedScheduler::<_, _, _, 3, 2, 2, 2, 6, 2, 3, 1>::new_with_host_operations(
            [
                node([None, None]),
                node([Some(CordId(0)), None]),
                node([Some(CordId(1)), None]),
            ],
            [cord(0, 0, 0, 1, 0), cord(1, 1, 0, 2, 0)],
            routes,
            bindings,
            [
                OperationDriver::new(AdapterOperation::Source {
                    value: input,
                    advanced: false,
                })
                .unwrap(),
                OperationDriver::new(AdapterOperation::HostEffect).unwrap(),
                OperationDriver::new(AdapterOperation::Sink { seen: None }).unwrap(),
            ],
            values,
            FixedSignLog::<64>::new(charge * 64).unwrap(),
        )
        .unwrap();
    scheduler.step().unwrap();
    scheduler.step().unwrap();
    let request = scheduler.next_host_request().unwrap();
    assert_eq!(request.request, RequestId(11));
    let output = scheduler.store_host_value(&[2]).unwrap();
    scheduler
        .complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(BoundedValueRef::new(output, 4).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    scheduler.run(32).unwrap();
    let AdapterOperation::Sink { seen: Some(seen) } = scheduler.drivers()[2].operation() else {
        panic!("adapter host sink");
    };
    assert_eq!(seen.slot, output.slot);
    assert_eq!(scheduler.values().used_items(), 0);
}

#[test]
fn full_multi_value_form_runs_through_public_operation_adapter() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let normalized = execute_full_operation_adapter(
        FixedValueStore::<8, 4>::new(24).unwrap(),
        FixedSignLog::<256>::new(charge * 256).unwrap(),
    );
    assert_eq!(normalized.produced, 4);
    assert_eq!(normalized.show_a_len, 2);
    assert_eq!(normalized.show_a_bytes[..2], [1, 1]);
    assert_eq!(normalized.show_b_len, 1);
    assert_eq!(normalized.show_b_bytes[0], 2);
    assert_eq!(normalized.used_items, 0);
    assert_eq!(normalized.pending, 0);
    assert!(normalized.saw_input_closed);
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_and_fixed_full_operation_adapter_vectors_match() {
    use crate::{HostedSignLog, HostedValueStore};

    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let fixed = execute_full_operation_adapter(
        FixedValueStore::<8, 4>::new(24).unwrap(),
        FixedSignLog::<256>::new(charge * 256).unwrap(),
    );
    let hosted = execute_full_operation_adapter(
        HostedValueStore::new(8, 4, 24).unwrap(),
        HostedSignLog::new(256, charge * 256).unwrap(),
    );
    assert_eq!(fixed, hosted);
}

#[derive(Debug, Eq, PartialEq)]
struct FullAdapterNormalized {
    show_a_bytes: [u32; 4],
    show_a_len: usize,
    show_b_bytes: [u32; 4],
    show_b_len: usize,
    produced: usize,
    decisions: u32,
    sign_len: u16,
    sign_bytes: u32,
    used_items: u16,
    pending: usize,
    saw_input_closed: bool,
}

fn execute_full_operation_adapter<S, E>(mut values: S, signs: E) -> FullAdapterNormalized
where
    S: ValueStorage,
    E: SignSink + SignQuery,
{
    let seed = values.store(&[255]).unwrap();
    let mut routes = FixedRoutes::<12, 5>::new(2);
    for (source, port, cord_id, sink) in [
        (0, 0, 0, 1),
        (1, 0, 1, 2),
        (1, 1, 2, 3),
        (2, 0, 3, 4),
        (3, 0, 4, 5),
    ] {
        routes
            .install(
                NodeId(source),
                PortId(port),
                RouteRange {
                    start: cord_id,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(cord_id),
                    sink: crate::CordEndpoint::local(NodeId(sink), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut bindings = FixedHostOperationBindings::<6>::new(1);
    bindings
        .install(
            NodeId(0),
            HostOperationBinding {
                operation: HostOperationId(0),
                maximum_input_bytes: 4,
                maximum_output_bytes: 4,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    let mut scheduler =
        FixedScheduler::<_, _, _, 6, 5, 2, 5, 12, 5, 6, 2>::new_with_host_operations(
            [
                node([None, None]),
                node([Some(CordId(0)), None]),
                node([Some(CordId(1)), None]),
                node([Some(CordId(2)), None]),
                node([Some(CordId(3)), None]),
                node([Some(CordId(4)), None]),
            ],
            [
                cord(0, 0, 0, 1, 0),
                cord(1, 1, 0, 2, 0),
                cord(2, 1, 1, 3, 0),
                cord(3, 2, 0, 4, 0),
                cord(4, 3, 0, 5, 0),
            ],
            routes,
            bindings,
            [
                OperationDriver::new(ConformanceOperation::Tick {
                    request: 1,
                    input: seed,
                    emitted: false,
                })
                .unwrap(),
                OperationDriver::new(ConformanceOperation::Tee {
                    value: None,
                    phase: 0,
                })
                .unwrap(),
                OperationDriver::new(ConformanceOperation::Filter).unwrap(),
                OperationDriver::new(ConformanceOperation::Latest {
                    held: None,
                    released: None,
                    retain_resumed: false,
                    closing: false,
                })
                .unwrap(),
                OperationDriver::new(ConformanceOperation::Show {
                    seen: [None; 4],
                    len: 0,
                })
                .unwrap(),
                OperationDriver::new(ConformanceOperation::Show {
                    seen: [None; 4],
                    len: 0,
                })
                .unwrap(),
            ],
            values,
            signs,
        )
        .unwrap();

    let mut produced = 0_usize;
    let mut complete = false;
    for _ in 0..512 {
        if let Some(request) = scheduler.next_host_request() {
            assert_eq!(request.node, NodeId(0));
            assert_eq!(request.operation, HostOperationId(0));
            let bytes: &[u8] = match produced {
                0 => &[0],
                1 => &[1, 1],
                2 => &[2],
                3 => &[3, 3],
                _ => panic!("unexpected tick request"),
            };
            let output = scheduler.store_host_value(bytes).unwrap();
            scheduler
                .complete_host_operation(
                    request.node,
                    request.request,
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: Some(BoundedValueRef::new(output, 4).unwrap()),
                        failure: None,
                    },
                )
                .unwrap();
            produced += 1;
            continue;
        }
        match scheduler.step().unwrap() {
            SchedulerStatus::Complete => {
                complete = true;
                break;
            }
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Idle => panic!("adapter form became idle"),
            SchedulerStatus::Cancelled => panic!("adapter form cancelled"),
        }
    }
    assert!(complete, "adapter form exceeded decision bound");
    let ConformanceOperation::Show {
        seen: show_a,
        len: show_a_len,
    } = scheduler.drivers()[4].operation()
    else {
        panic!("show-a adapter");
    };
    let ConformanceOperation::Show {
        seen: show_b,
        len: show_b_len,
    } = scheduler.drivers()[5].operation()
    else {
        panic!("show-b adapter");
    };
    let mut show_a_bytes = [0; 4];
    for (index, value) in show_a[..*show_a_len].iter().enumerate() {
        show_a_bytes[index] = value.expect("show-a value").byte_len;
    }
    let mut show_b_bytes = [0; 4];
    for (index, value) in show_b[..*show_b_len].iter().enumerate() {
        show_b_bytes[index] = value.expect("show-b value").byte_len;
    }
    FullAdapterNormalized {
        show_a_bytes,
        show_a_len: *show_a_len,
        show_b_bytes,
        show_b_len: *show_b_len,
        produced,
        decisions: scheduler.decisions(),
        sign_len: scheduler.signs().len(),
        sign_bytes: scheduler.signs().used_bytes(),
        used_items: scheduler.values().used_items(),
        pending: scheduler.pending_host_operation_count(),
        saw_input_closed: scheduler
            .signs()
            .contains_kind(KernelEventKind::InputClosed),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct AdapterNormalized {
    left: u16,
    right: u16,
    decisions: u32,
    sign_len: u16,
    sign_bytes: u32,
    used_items: u16,
}

fn execute_operation_adapter<S, E>(mut values: S, signs: E) -> AdapterNormalized
where
    S: ValueStorage,
    E: SignSink,
{
    let value = values.store(&[42]).unwrap();
    let mut routes = FixedRoutes::<8, 3>::new(2);
    for (source, port, cord_id, sink) in [(0, 0, 0, 1), (1, 0, 1, 2), (1, 1, 2, 3)] {
        routes
            .install(
                NodeId(source),
                PortId(port),
                RouteRange {
                    start: cord_id,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(cord_id),
                    sink: crate::CordEndpoint::local(NodeId(sink), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 4, 3, 2, 3, 8, 3>::new(
        [
            node([None, None]),
            node([Some(CordId(0)), None]),
            node([Some(CordId(1)), None]),
            node([Some(CordId(2)), None]),
        ],
        [
            cord(0, 0, 0, 1, 0),
            cord(1, 1, 0, 2, 0),
            cord(2, 1, 1, 3, 0),
        ],
        routes,
        [
            OperationDriver::new(AdapterOperation::Source {
                value,
                advanced: false,
            })
            .unwrap(),
            OperationDriver::new(AdapterOperation::Tee {
                value: None,
                phase: 0,
            })
            .unwrap(),
            OperationDriver::new(AdapterOperation::Sink { seen: None }).unwrap(),
            OperationDriver::new(AdapterOperation::Sink { seen: None }).unwrap(),
        ],
        values,
        signs,
    )
    .unwrap();
    scheduler.run(32).unwrap();
    let AdapterOperation::Sink { seen: Some(left) } = scheduler.drivers()[2].operation() else {
        panic!("left adapter sink");
    };
    let AdapterOperation::Sink { seen: Some(right) } = scheduler.drivers()[3].operation() else {
        panic!("right adapter sink");
    };
    AdapterNormalized {
        left: left.slot,
        right: right.slot,
        decisions: scheduler.decisions(),
        sign_len: scheduler.signs().len(),
        sign_bytes: scheduler.signs().used_bytes(),
        used_items: scheduler.values().used_items(),
    }
}

#[test]
fn blocked_join_preserves_every_input_until_atomic_commit() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let normalized = execute_join(
        FixedValueStore::<4, 4>::new(16).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    assert_eq!(normalized.output_slot, 0);
    assert_eq!(normalized.used_items, 0);
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_and_fixed_join_rollback_vectors_match() {
    use crate::{HostedSignLog, HostedValueStore};

    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let fixed = execute_join(
        FixedValueStore::<4, 4>::new(16).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    let hosted = execute_join(
        HostedValueStore::new(4, 4, 16).unwrap(),
        HostedSignLog::new(64, charge * 64).unwrap(),
    );
    assert_eq!(fixed, hosted);
}

#[derive(Debug, Eq, PartialEq)]
struct JoinNormalized {
    output_slot: u16,
    decisions: u32,
    sign_len: u16,
    sign_bytes: u32,
    used_items: u16,
}

fn execute_join<S, E>(mut values: S, signs: E) -> JoinNormalized
where
    S: ValueStorage,
    E: SignSink,
{
    let left = values.store(&[10]).unwrap();
    let right = values.store(&[20]).unwrap();
    let mut routes = FixedRoutes::<8, 3>::new(2);
    for (source, target, sink, sink_port) in [(0, 0, 1, 0), (2, 1, 1, 1), (1, 2, 3, 0)] {
        routes
            .install(
                NodeId(source),
                PortId(0),
                RouteRange {
                    start: target,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(target),
                    sink: crate::CordEndpoint::local(NodeId(sink), PortId(sink_port)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 4, 3, 2, 3, 8, 3>::new(
        [
            node([None, None]),
            node([Some(CordId(0)), Some(CordId(1))]),
            node([None, None]),
            node([Some(CordId(2)), None]),
        ],
        [
            cord(0, 0, 0, 1, 0),
            cord(1, 2, 0, 1, 1),
            cord(2, 1, 0, 3, 0),
        ],
        routes,
        [
            JoinDriver::Source { value: Some(left) },
            JoinDriver::Join,
            JoinDriver::Source { value: Some(right) },
            JoinDriver::Sink { seen: None },
        ],
        values,
        signs,
    )
    .unwrap();
    scheduler.step().unwrap();
    assert_eq!(scheduler.cords[0].len, 1);
    scheduler.step().unwrap();
    assert_eq!(scheduler.cords[0].len, 1);
    assert_eq!(scheduler.cords[1].len, 0);
    scheduler.run(32).unwrap();
    let JoinDriver::Sink { seen: Some(seen) } = scheduler.drivers()[3] else {
        panic!("join sink");
    };
    JoinNormalized {
        output_slot: seen.slot,
        decisions: scheduler.decisions(),
        sign_len: scheduler.signs().len(),
        sign_bytes: scheduler.signs().used_bytes(),
        used_items: scheduler.values().used_items(),
    }
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_and_fixed_schedulers_have_matching_multi_value_vectors() {
    use crate::{HostedSignLog, HostedValueStore};

    let event_charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let fixed = execute(
        FixedValueStore::<8, 4>::new(16).unwrap(),
        FixedSignLog::<128>::new(event_charge * 128).unwrap(),
    );
    let hosted = execute(
        HostedValueStore::new(8, 4, 16).unwrap(),
        HostedSignLog::new(128, event_charge * 128).unwrap(),
    );
    assert_eq!(fixed, hosted);
}

#[test]
fn scheduler_admits_correlates_and_wakes_host_operations() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let normalized = execute_host_operation(
        FixedValueStore::<8, 8>::new(32).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    assert_eq!(normalized.request, RequestId(7));
    assert_eq!(normalized.operation, HostOperationId(0));
    assert_eq!(normalized.input, [3]);
    assert_eq!(normalized.output_slot, 1);
    assert_eq!(normalized.used_items, 0);
    assert_eq!(normalized.pending, 0);
    assert!(normalized.saw_requested);
    assert!(normalized.saw_completed);
}

#[test]
fn scheduler_rejects_unbound_host_operation_before_consumption_commit() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let mut scheduler = host_scheduler_with_binding_node(
        FixedValueStore::<8, 8>::new(32).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
        NodeId(0),
    );
    scheduler.step().unwrap();
    assert_eq!(scheduler.cords[0].len, 1);
    assert_eq!(
        scheduler.step(),
        Err(super::SchedulerError::Routing(
            ProtocolError::HostOperationMissing
        ))
    );
    assert_eq!(scheduler.cords[0].len, 1);
    assert_eq!(scheduler.values().used_items(), 1);
    assert_eq!(scheduler.pending_host_operation_count(), 0);
}

#[test]
fn scheduler_never_reuses_a_retired_request_identity() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let mut scheduler = host_scheduler(
        FixedValueStore::<8, 8>::new(32).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    scheduler.step().unwrap();
    scheduler.step().unwrap();
    let request = scheduler.next_host_request().unwrap();
    let output = scheduler.store_host_value(&[4]).unwrap();
    scheduler
        .complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(BoundedValueRef::new(output, 4).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    let HostDriver::Effect { repeat_request, .. } = &mut scheduler.drivers[1] else {
        panic!("effect driver");
    };
    *repeat_request = true;
    scheduler.step().unwrap();
    scheduler.step().unwrap();
    assert_eq!(
        scheduler.step(),
        Err(super::SchedulerError::HostOperationRequestDuplicate)
    );
    assert_eq!(scheduler.pending_host_operation_count(), 1);
    assert_eq!(scheduler.values().used_items(), 1);
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_and_fixed_host_operation_vectors_match() {
    use crate::{HostedSignLog, HostedValueStore};

    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let fixed = execute_host_operation(
        FixedValueStore::<8, 8>::new(32).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    let hosted = execute_host_operation(
        HostedValueStore::new(8, 8, 32).unwrap(),
        HostedSignLog::new(64, charge * 64).unwrap(),
    );
    assert_eq!(fixed, hosted);
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_executor_keeps_allocation_shape_after_play_start() {
    use crate::{HostedSignLog, HostedValueStore};

    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let values = HostedValueStore::new(8, 8, 32).unwrap();
    let value_shape = values.allocation_capacities();
    let signs = HostedSignLog::new(64, charge * 64).unwrap();
    let sign_shape = signs.allocation_capacity();
    let mut scheduler = host_scheduler(values, signs);
    assert_eq!(scheduler.values.allocation_capacities(), value_shape);
    assert_eq!(scheduler.signs.allocation_capacity(), sign_shape);
    scheduler.step().unwrap();
    scheduler.step().unwrap();
    let request = scheduler.next_host_request().unwrap();
    let output = scheduler.store_host_value(&[4]).unwrap();
    scheduler
        .complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(BoundedValueRef::new(output, 4).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    scheduler.run(32).unwrap();
    assert_eq!(scheduler.values.allocation_capacities(), value_shape);
    assert_eq!(scheduler.signs.allocation_capacity(), sign_shape);
    assert_eq!(scheduler.values.used_items(), 0);
}

#[test]
fn cancellation_rejects_late_host_completion_and_releases_pending_input() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let mut scheduler = host_scheduler(
        FixedValueStore::<8, 8>::new(32).unwrap(),
        FixedSignLog::<64>::new(charge * 64).unwrap(),
    );
    scheduler.step().unwrap();
    scheduler.step().unwrap();
    let request = scheduler.next_host_request().unwrap();
    assert_eq!(request.request, RequestId(7));
    scheduler.cancel().unwrap();
    assert_eq!(scheduler.pending_host_operation_count(), 0);
    assert_eq!(scheduler.values().used_items(), 0);
    assert_eq!(
        scheduler.complete_host_operation(
            NodeId(1),
            RequestId(7),
            HostOperationOutcome {
                disposition: HostOperationDisposition::Cancelled,
                output: None,
                failure: Some(Failure {
                    code: FailureCode::Cancelled,
                    detail: 0,
                }),
            },
        ),
        Err(super::SchedulerError::HostOperationCompletionRejected)
    );
    assert_eq!(scheduler.step().unwrap(), SchedulerStatus::Cancelled);
    let HostDriver::Effect { cancelled, .. } = scheduler.drivers()[1] else {
        panic!("effect driver");
    };
    assert!(cancelled);
    assert!(scheduler
        .signs()
        .contains_kind(KernelEventKind::RunCancelled));
}

#[derive(Debug, Eq, PartialEq)]
struct HostNormalized {
    request: RequestId,
    operation: HostOperationId,
    input: [u8; 1],
    output_slot: u16,
    decisions: u32,
    sign_len: u16,
    sign_bytes: u32,
    used_items: u16,
    pending: usize,
    saw_requested: bool,
    saw_completed: bool,
}

fn execute_host_operation<S, E>(values: S, signs: E) -> HostNormalized
where
    S: ValueStorage,
    E: SignSink + SignQuery,
{
    let mut scheduler = host_scheduler(values, signs);
    assert!(matches!(
        scheduler.step().unwrap(),
        SchedulerStatus::Progress { node: NodeId(0) }
    ));
    assert!(matches!(
        scheduler.step().unwrap(),
        SchedulerStatus::Progress { node: NodeId(1) }
    ));
    assert_eq!(scheduler.pending_host_operation_count(), 1);
    assert_eq!(
        scheduler.complete_host_operation(
            NodeId(1),
            RequestId(7),
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        ),
        Err(super::SchedulerError::HostOperationCompletionRejected)
    );
    let request = scheduler.next_host_request().unwrap();
    let mut input = [0];
    input.copy_from_slice(scheduler.host_value(request.input.value).unwrap());
    assert_eq!(
        scheduler.complete_host_operation(
            NodeId(0),
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        ),
        Err(super::SchedulerError::HostOperationCompletionRejected)
    );
    assert_eq!(
        scheduler.complete_host_operation(
            NodeId(1),
            RequestId(8),
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        ),
        Err(super::SchedulerError::HostOperationCompletionRejected)
    );
    let oversized = scheduler.store_host_value(&[0, 1, 2, 3, 4]).unwrap();
    assert_eq!(
        scheduler.complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(BoundedValueRef::new(oversized, 5).unwrap()),
                failure: None,
            },
        ),
        Err(super::SchedulerError::HostOperationOutputExceeded)
    );
    scheduler.discard_host_value(oversized).unwrap();
    let output = scheduler.store_host_value(&[4]).unwrap();
    scheduler
        .complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(BoundedValueRef::new(output, 4).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    assert_eq!(
        scheduler.complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        ),
        Err(super::SchedulerError::HostOperationCompletionRejected)
    );
    scheduler.run(32).unwrap();
    let HostDriver::Sink { seen: Some(seen) } = scheduler.drivers()[2] else {
        panic!("host sink");
    };
    HostNormalized {
        request: request.request,
        operation: request.operation,
        input,
        output_slot: seen.slot,
        decisions: scheduler.decisions(),
        sign_len: scheduler.signs().len(),
        sign_bytes: scheduler.signs().used_bytes(),
        used_items: scheduler.values().used_items(),
        pending: scheduler.pending_host_operation_count(),
        saw_requested: scheduler
            .signs()
            .contains_kind(KernelEventKind::HostOperationRequested),
        saw_completed: scheduler
            .signs()
            .contains_kind(KernelEventKind::HostOperationCompleted),
    }
}

fn host_scheduler<S, E>(
    values: S,
    signs: E,
) -> FixedScheduler<HostDriver, S, E, 3, 2, 2, 2, 6, 2, 3, 1>
where
    S: ValueStorage,
    E: SignSink,
{
    host_scheduler_with_binding_node(values, signs, NodeId(1))
}

#[test]
fn host_output_bytes_are_borrowed_and_derived_output_uses_admitted_storage() {
    let mut values = FixedValueStore::<4, 64>::new(128).unwrap();
    let empty = values.store(&[]).unwrap();
    let mut routes = FixedRoutes::<2, 1>::new(2);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: crate::CordEndpoint::local(NodeId(1), PortId(0)),
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
                maximum_input_bytes: 0,
                maximum_output_bytes: 3,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    let signs =
        FixedSignLog::<32>::new((32 * core::mem::size_of::<crate::KernelEvent>()) as u32).unwrap();
    let mut scheduler =
        FixedScheduler::<_, _, _, 2, 1, 2, 1, 2, 1, 2, 1>::new_with_host_operations(
            [node([None, None]), node([Some(CordId(0)), None])],
            [CordSpec::local(
                CordId(0),
                (NodeId(0), PortId(0)),
                (NodeId(1), PortId(0)),
                CordCapacity {
                    slot_start: 0,
                    item_capacity: 1,
                    byte_capacity: 8,
                },
            )],
            routes,
            bindings,
            [
                HostDriver::Transform {
                    input: empty,
                    phase: 0,
                },
                HostDriver::DecodedSink { seen: None },
            ],
            values,
            signs,
        )
        .unwrap();
    assert!(matches!(
        scheduler.step().unwrap(),
        SchedulerStatus::Progress { .. }
    ));
    let request = scheduler.next_host_request().unwrap();
    let observation = scheduler.store_host_value(&[0x90, 60, 100]).unwrap();
    scheduler
        .complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(BoundedValueRef::new(observation, 3).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    let second_request = (0..3)
        .find_map(|_| {
            let _ = scheduler.step().unwrap();
            scheduler.next_host_request()
        })
        .expect("derived output and repeated zero-byte request make bounded progress");
    assert_eq!(second_request.request, RequestId(2));
    assert_eq!(second_request.input, request.input);
    let second_observation = scheduler.store_host_value(&[0x80, 60, 0]).unwrap();
    scheduler
        .complete_host_operation(
            second_request.node,
            second_request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(BoundedValueRef::new(second_observation, 3).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    scheduler.run(16).unwrap();
    let HostDriver::DecodedSink { seen } = scheduler.drivers()[1] else {
        panic!("derived sink identity changed");
    };
    assert_eq!(seen, Some([9, 8, 7, 6, 5]));
    assert_eq!(scheduler.values().used_items(), 0);
}

fn host_scheduler_with_binding_node<S, E>(
    mut values: S,
    signs: E,
    binding_node: NodeId,
) -> FixedScheduler<HostDriver, S, E, 3, 2, 2, 2, 6, 2, 3, 1>
where
    S: ValueStorage,
    E: SignSink,
{
    let input = values.store(&[3]).unwrap();
    let mut routes = FixedRoutes::<6, 2>::new(2);
    for (node, cord, sink) in [(0, 0, 1), (1, 1, 2)] {
        routes
            .install(
                NodeId(node),
                PortId(0),
                RouteRange {
                    start: cord,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(cord),
                    sink: crate::CordEndpoint::local(NodeId(sink), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut bindings = FixedHostOperationBindings::<3>::new(1);
    bindings
        .install(
            binding_node,
            HostOperationBinding {
                operation: HostOperationId(0),
                maximum_input_bytes: 4,
                maximum_output_bytes: 4,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    FixedScheduler::new_with_host_operations(
        [
            node([None, None]),
            node([Some(CordId(0)), None]),
            node([Some(CordId(1)), None]),
        ],
        [cord(0, 0, 0, 1, 0), cord(1, 1, 0, 2, 0)],
        routes,
        bindings,
        [
            HostDriver::Source { value: Some(input) },
            HostDriver::Effect {
                requested: false,
                cancelled: false,
                repeat_request: false,
            },
            HostDriver::Sink { seen: None },
        ],
        values,
        signs,
    )
    .unwrap()
}

#[test]
fn cancellation_releases_queued_and_driver_owned_values_and_is_terminal() {
    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let normalized = execute_cancellation(
        FixedValueStore::<4, 4>::new(8).unwrap(),
        FixedSignLog::<16>::new(charge * 16).unwrap(),
    );
    assert_eq!(normalized.used_items, 0);
    assert!(normalized.driver_cancelled);
    assert_eq!(normalized.status, SchedulerStatus::Cancelled);
    assert!(normalized.saw_cancellation_requested);
    assert!(normalized.saw_run_cancelled);
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_and_fixed_cancellation_vectors_match() {
    use crate::{HostedSignLog, HostedValueStore};

    let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
    let fixed = execute_cancellation(
        FixedValueStore::<4, 4>::new(8).unwrap(),
        FixedSignLog::<16>::new(charge * 16).unwrap(),
    );
    let hosted = execute_cancellation(
        HostedValueStore::new(4, 4, 8).unwrap(),
        HostedSignLog::new(16, charge * 16).unwrap(),
    );
    assert_eq!(fixed, hosted);
}

#[derive(Debug, Eq, PartialEq)]
struct CancellationNormalized {
    used_items: u16,
    sign_len: u16,
    sign_bytes: u32,
    driver_cancelled: bool,
    status: SchedulerStatus,
    saw_cancellation_requested: bool,
    saw_run_cancelled: bool,
}

fn execute_cancellation<S, E>(mut values: S, signs: E) -> CancellationNormalized
where
    S: ValueStorage,
    E: SignSink + SignQuery,
{
    let source_values = [
        Some(values.store(&[0]).unwrap()),
        Some(values.store(&[1]).unwrap()),
        None,
        None,
    ];
    let mut routes = FixedRoutes::<2, 1>::new(1);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: crate::CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 2, 1, 2, 1, 2, 1>::new(
        [
            NodeSpec {
                input_cords: [None, None],
                maximum_step_work: 1,
            },
            NodeSpec {
                input_cords: [Some(CordId(0)), None],
                maximum_step_work: 1,
            },
        ],
        [CordSpec::local(
            CordId(0),
            (NodeId(0), PortId(0)),
            (NodeId(1), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 4,
            },
        )],
        routes,
        [
            Driver::Source {
                values: source_values,
                next: 0,
            },
            Driver::BlockedSink { cancelled: false },
        ],
        values,
        signs,
    )
    .unwrap();
    assert!(matches!(
        scheduler.step().unwrap(),
        SchedulerStatus::Progress { node: NodeId(0) }
    ));
    assert!(matches!(
        scheduler.step().unwrap(),
        SchedulerStatus::Progress { node: NodeId(1) }
    ));
    scheduler.cancel().unwrap();
    let Driver::BlockedSink { cancelled } = scheduler.drivers()[1] else {
        panic!("blocked sink");
    };
    CancellationNormalized {
        used_items: scheduler.values().used_items(),
        sign_len: scheduler.signs().len(),
        sign_bytes: scheduler.signs().used_bytes(),
        driver_cancelled: cancelled,
        status: scheduler.step().unwrap(),
        saw_cancellation_requested: scheduler
            .signs()
            .contains_kind(KernelEventKind::CancellationRequested),
        saw_run_cancelled: scheduler
            .signs()
            .contains_kind(KernelEventKind::RunCancelled),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Normalized {
    show_a: [u16; 4],
    show_a_len: usize,
    show_b: [u16; 4],
    show_b_len: usize,
    decisions: u32,
    sign_len: u16,
    sign_bytes: u32,
    used_items: u16,
    saw_input_closed: bool,
}

fn execute<S, E>(mut values: S, signs: E) -> Normalized
where
    S: ValueStorage,
    E: SignSink + SignQuery,
{
    let source_values = [
        Some(values.store(&[0]).unwrap()),
        Some(values.store(&[1]).unwrap()),
        Some(values.store(&[2]).unwrap()),
        Some(values.store(&[3]).unwrap()),
    ];
    let mut routes = FixedRoutes::<{ NODES * PORTS }, CORDS>::new(PORTS as u16);
    for (source_node, source_port, target) in [
        (
            0,
            0,
            RouteTarget {
                cord: CordId(0),
                sink: crate::CordEndpoint::local(NodeId(1), PortId(0)),
            },
        ),
        (
            1,
            0,
            RouteTarget {
                cord: CordId(1),
                sink: crate::CordEndpoint::local(NodeId(2), PortId(0)),
            },
        ),
        (
            1,
            1,
            RouteTarget {
                cord: CordId(2),
                sink: crate::CordEndpoint::local(NodeId(3), PortId(0)),
            },
        ),
        (
            2,
            0,
            RouteTarget {
                cord: CordId(3),
                sink: crate::CordEndpoint::local(NodeId(4), PortId(0)),
            },
        ),
        (
            3,
            0,
            RouteTarget {
                cord: CordId(4),
                sink: crate::CordEndpoint::local(NodeId(5), PortId(0)),
            },
        ),
    ] {
        routes
            .install(
                NodeId(source_node),
                PortId(source_port),
                RouteRange {
                    start: target.cord.0,
                    len: 1,
                },
                &[target],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let cords = [
        cord(0, 0, 0, 1, 0),
        cord(1, 1, 0, 2, 0),
        cord(2, 1, 1, 3, 0),
        cord(3, 2, 0, 4, 0),
        cord(4, 3, 0, 5, 0),
    ];
    let nodes = [
        node([None, None]),
        node([Some(CordId(0)), None]),
        node([Some(CordId(1)), None]),
        node([Some(CordId(2)), None]),
        node([Some(CordId(3)), None]),
        node([Some(CordId(4)), None]),
    ];
    let drivers = [
        Driver::Source {
            values: source_values,
            next: 0,
        },
        Driver::Tee,
        Driver::Filter,
        Driver::Latest { held: None },
        Driver::Sink {
            seen: [None; 4],
            len: 0,
            stall: true,
        },
        Driver::Sink {
            seen: [None; 4],
            len: 0,
            stall: true,
        },
    ];
    let mut scheduler =
        FixedScheduler::<_, _, _, NODES, CORDS, PORTS, CORDS, { NODES * PORTS }, CORDS>::new(
            nodes, cords, routes, drivers, values, signs,
        )
        .unwrap();

    scheduler.run(128).unwrap();
    assert_eq!(scheduler.step().unwrap(), SchedulerStatus::Complete);
    let Driver::Sink { seen, len, .. } = &scheduler.drivers()[4] else {
        panic!("show-a sink");
    };
    let mut show_a = [u16::MAX; 4];
    for (index, value) in seen[..*len].iter().enumerate() {
        show_a[index] = value.unwrap().slot;
    }
    let show_a_len = *len;
    let Driver::Sink { seen, len, .. } = &scheduler.drivers()[5] else {
        panic!("show-b sink");
    };
    let mut show_b = [u16::MAX; 4];
    for (index, value) in seen[..*len].iter().enumerate() {
        show_b[index] = value.unwrap().slot;
    }
    Normalized {
        show_a,
        show_a_len,
        show_b,
        show_b_len: *len,
        decisions: scheduler.decisions(),
        sign_len: scheduler.signs().len(),
        sign_bytes: scheduler.signs().used_bytes(),
        used_items: scheduler.values().used_items(),
        saw_input_closed: scheduler
            .signs()
            .contains_kind(KernelEventKind::InputClosed),
    }
}

fn node(input_cords: [Option<CordId>; PORTS]) -> NodeSpec<PORTS> {
    NodeSpec {
        input_cords,
        maximum_step_work: 3,
    }
}

#[test]
fn remote_cords_keep_values_owned_until_delivery_and_retry_full_without_growth() {
    let endpoint = RemoteEndpointId(0);
    let mut source_values = FixedValueStore::<3, 12>::new(12).unwrap();
    let first = source_values.store(b"abcd").unwrap();
    let second = source_values.store(b"efgh").unwrap();
    let mut source_routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    source_routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: crate::CordEndpoint::Remote(endpoint),
            }],
        )
        .unwrap();
    source_routes.seal().unwrap();
    let source_sign = FixedSignLog::<64>::new_with_remote_storage(
        (64 * core::mem::size_of::<crate::KernelEvent>()) as u32,
        64,
        crate::remote_sign_storage_bytes(64).unwrap(),
    )
    .unwrap();
    let mut source = FixedScheduler::<_, _, _, 1, 1, PORTS, 1, 2, 1>::new(
        [node([None; PORTS])],
        [CordSpec::remote_egress(
            CordId(0),
            (NodeId(0), PortId(0)),
            endpoint,
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 4,
            },
        )],
        source_routes,
        [Driver::Source {
            values: [Some(first), Some(second), None, None],
            next: 0,
        }],
        source_values,
        source_sign,
    )
    .unwrap();

    let mut sink_routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    sink_routes.seal().unwrap();
    let sink_sign = FixedSignLog::<64>::new_with_remote_storage(
        (64 * core::mem::size_of::<crate::KernelEvent>()) as u32,
        64,
        crate::remote_sign_storage_bytes(64).unwrap(),
    )
    .unwrap();
    let mut sink = FixedScheduler::<_, _, _, 1, 1, PORTS, 1, 2, 1>::new(
        [node([Some(CordId(0)), None])],
        [CordSpec::remote_ingress(
            CordId(0),
            endpoint,
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 4,
            },
        )],
        sink_routes,
        [Driver::Sink {
            seen: [None; 4],
            len: 0,
            stall: false,
        }],
        FixedValueStore::<1, 4>::new(4).unwrap(),
        sink_sign,
    )
    .unwrap();

    assert!(matches!(
        source.step().unwrap(),
        SchedulerStatus::Progress { .. }
    ));
    let offer = source
        .remote_egress_offer(endpoint, CordId(0))
        .unwrap()
        .unwrap();
    assert_eq!(offer.sequence, 0);
    assert_eq!(
        source.remote_egress_offer(endpoint, CordId(0)).unwrap(),
        Some(offer)
    );
    assert_eq!(
        source.remote_egress_offer(RemoteEndpointId(1), CordId(0)),
        Err(SchedulerError::InvalidRemoteCordAccess)
    );
    assert_eq!(source.values().reference_count(first).unwrap(), 1);
    assert_eq!(
        source.discard_host_value(first),
        Err(SchedulerError::ValueOwnershipViolation)
    );
    let first_bytes = *source
        .host_value(offer.value)
        .unwrap()
        .first_chunk::<4>()
        .unwrap();
    assert!(matches!(
        sink.admit_remote_input(endpoint, CordId(0), 0, &first_bytes)
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence: 0, .. }
    ));
    assert_eq!(
        sink.admit_remote_input(endpoint, CordId(0), 0, &first_bytes),
        Err(SchedulerError::RemoteSequenceRejected)
    );
    assert_eq!(
        sink.admit_remote_input(endpoint, CordId(0), 2, b"ijkl"),
        Err(SchedulerError::RemoteSequenceRejected)
    );
    assert_eq!(sink.cord_usage(CordId(0)).unwrap(), (1, 4));
    assert_eq!(
        sink.admit_remote_input(endpoint, CordId(0), 1, b"efgh")
            .unwrap(),
        RemoteIngressOutcome::Full { sequence: 1 }
    );
    assert_eq!(sink.cord_usage(CordId(0)).unwrap(), (1, 4));
    assert_eq!(sink.values().used_items(), 1);

    assert_eq!(
        source.remote_egress_accept(endpoint, CordId(0), 1),
        Err(SchedulerError::RemoteSequenceRejected)
    );
    source.remote_egress_accept(endpoint, CordId(0), 0).unwrap();
    source.remote_egress_accept(endpoint, CordId(0), 0).unwrap();
    source
        .remote_egress_delivered(endpoint, CordId(0), 0)
        .unwrap();
    assert_eq!(source.values().used_items(), 1);
    assert_eq!(
        source.remote_egress_delivered(endpoint, CordId(0), 0),
        Err(SchedulerError::RemoteDeliveryRejected)
    );
    sink.step().unwrap();
    assert_eq!(sink.values().used_items(), 0);

    source.step().unwrap();
    let offer = source
        .remote_egress_offer(endpoint, CordId(0))
        .unwrap()
        .unwrap();
    assert_eq!(offer.sequence, 1);
    let second_bytes = *source
        .host_value(offer.value)
        .unwrap()
        .first_chunk::<4>()
        .unwrap();
    assert!(matches!(
        sink.admit_remote_input(endpoint, CordId(0), 1, &second_bytes)
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence: 1, .. }
    ));
    source.remote_egress_accept(endpoint, CordId(0), 1).unwrap();
    source
        .remote_egress_delivered(endpoint, CordId(0), 1)
        .unwrap();
    source.step().unwrap();
    assert!(source.remote_egress_terminal(endpoint, CordId(0)).unwrap());

    sink.close_remote_input(endpoint, CordId(0)).unwrap();
    sink.step().unwrap();
    sink.step().unwrap();
    assert_eq!(sink.step().unwrap(), SchedulerStatus::Complete);
    assert_eq!(sink.values().used_items(), 0);
    assert_eq!(sink.cord_usage(CordId(0)).unwrap(), (0, 0));
    assert!(source
        .signs()
        .contains_kind(KernelEventKind::RemoteValueDelivered));
    assert!(sink
        .signs()
        .contains_kind(KernelEventKind::RemoteInputClosed));
    assert!(source
        .signs()
        .events()
        .filter_map(|event| {
            source
                .signs()
                .remote_identity(event.sequence)
                .map(|remote| (event.kind, remote))
        })
        .eq([
            (
                KernelEventKind::RemoteValueOffered,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Egress,
                    sequence: 0,
                },
            ),
            (
                KernelEventKind::RemoteValueAccepted,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Egress,
                    sequence: 0,
                },
            ),
            (
                KernelEventKind::RemoteValueDelivered,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Egress,
                    sequence: 0,
                },
            ),
            (
                KernelEventKind::RemoteValueOffered,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Egress,
                    sequence: 1,
                },
            ),
            (
                KernelEventKind::RemoteValueAccepted,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Egress,
                    sequence: 1,
                },
            ),
            (
                KernelEventKind::RemoteValueDelivered,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Egress,
                    sequence: 1,
                },
            ),
            (
                KernelEventKind::RemoteOutputClosed,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Egress,
                    sequence: 2,
                },
            ),
        ]));
    assert!(sink
        .signs()
        .events()
        .filter_map(|event| {
            sink.signs()
                .remote_identity(event.sequence)
                .map(|remote| (event.kind, remote))
        })
        .eq([
            (
                KernelEventKind::RemoteInputAdmitted,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Ingress,
                    sequence: 0,
                },
            ),
            (
                KernelEventKind::RemoteInputAdmitted,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Ingress,
                    sequence: 1,
                },
            ),
            (
                KernelEventKind::RemoteInputClosed,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord: CordId(0),
                    direction: crate::RemoteCordDirection::Ingress,
                    sequence: 2,
                },
            ),
        ]));
    let remote_sign_count = source
        .signs()
        .events()
        .filter(|event| source.signs().remote_identity(event.sequence).is_some())
        .count();
    source.cancel().unwrap();
    assert_eq!(
        source.remote_egress_accept(endpoint, CordId(0), 1),
        Err(SchedulerError::Cancelled)
    );
    assert_eq!(
        source.remote_egress_delivered(endpoint, CordId(0), 1),
        Err(SchedulerError::RemoteDeliveryRejected)
    );
    assert_eq!(
        source
            .signs()
            .events()
            .filter(|event| source.signs().remote_identity(event.sequence).is_some())
            .count(),
        remote_sign_count
    );
}

#[test]
fn remote_delivery_sign_exhaustion_preserves_the_in_flight_value() {
    let endpoint = RemoteEndpointId(0);
    let mut values = FixedValueStore::<1, 4>::new(4).unwrap();
    let value = values.store(b"data").unwrap();
    let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: crate::CordEndpoint::Remote(endpoint),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let signs = FixedSignLog::<8>::new_with_remote_storage(
        (8 * core::mem::size_of::<crate::KernelEvent>()) as u32,
        2,
        crate::remote_sign_storage_bytes(2).unwrap(),
    )
    .unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 1, 1, PORTS, 1, 2, 1>::new(
        [node([None; PORTS])],
        [CordSpec::remote_egress(
            CordId(0),
            (NodeId(0), PortId(0)),
            endpoint,
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 4,
            },
        )],
        routes,
        [Driver::Source {
            values: [Some(value), None, None, None],
            next: 0,
        }],
        values,
        signs,
    )
    .unwrap();

    scheduler.step().unwrap();
    let offer = scheduler
        .remote_egress_offer(endpoint, CordId(0))
        .unwrap()
        .unwrap();
    scheduler
        .remote_egress_accept(endpoint, CordId(0), offer.sequence)
        .unwrap();
    assert_eq!(
        scheduler.remote_egress_delivered(endpoint, CordId(0), offer.sequence),
        Err(SchedulerError::Sign(
            crate::SignError::RemoteItemCapacityExceeded
        ))
    );
    assert_eq!(scheduler.cord_usage(CordId(0)).unwrap(), (1, 4));
    assert_eq!(scheduler.values().reference_count(value).unwrap(), 1);
    assert_eq!(
        scheduler.remote_egress_offer(endpoint, CordId(0)).unwrap(),
        Some(offer)
    );
}

#[test]
fn remote_ingress_sign_exhaustion_preserves_queue_sequence_and_open_state() {
    let endpoint = RemoteEndpointId(0);
    let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    routes.seal().unwrap();
    let signs =
        FixedSignLog::<8>::new((8 * core::mem::size_of::<crate::KernelEvent>()) as u32).unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 1, 1, PORTS, 1, 2, 1>::new(
        [node([Some(CordId(0)), None])],
        [CordSpec::remote_ingress(
            CordId(0),
            endpoint,
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 4,
            },
        )],
        routes,
        [Driver::Sink {
            seen: [None; 4],
            len: 0,
            stall: false,
        }],
        FixedValueStore::<1, 4>::new(4).unwrap(),
        signs,
    )
    .unwrap();

    assert_eq!(
        scheduler.admit_remote_input(endpoint, CordId(0), 0, b"data"),
        Err(SchedulerError::Sign(
            crate::SignError::RemoteItemCapacityExceeded
        ))
    );
    assert_eq!(scheduler.cord_usage(CordId(0)).unwrap(), (0, 0));
    assert_eq!(
        scheduler.admit_remote_input(endpoint, CordId(0), 1, b"data"),
        Err(SchedulerError::RemoteSequenceRejected)
    );
    assert_eq!(
        scheduler.close_remote_input(endpoint, CordId(0)),
        Err(SchedulerError::Sign(
            crate::SignError::RemoteItemCapacityExceeded
        ))
    );
    assert_eq!(
        scheduler.admit_remote_input(endpoint, CordId(0), 0, b"data"),
        Err(SchedulerError::Sign(
            crate::SignError::RemoteItemCapacityExceeded
        ))
    );
}

fn cord(id: u16, source_node: u16, source_port: u16, sink_node: u16, sink_port: u16) -> CordSpec {
    CordSpec::local(
        CordId(id),
        (NodeId(source_node), PortId(source_port)),
        (NodeId(sink_node), PortId(sink_port)),
        CordCapacity {
            slot_start: id,
            item_capacity: 1,
            byte_capacity: 4,
        },
    )
}
