use core::mem::size_of;

use conduit_kernel::debug_observation::{
    DebugBreakpoint, DebugBreakpointKind, DebugControlRefusal, DebugEventKind,
    DebugExecutionIdentity, DebugNodeBinding, DebugObservationBuffer, DebugObservationInput,
    DebugObservationRecord, DebugObservationRefusal, DebugSubject, ObservedSignSink,
    DEBUG_CONTROL_SCHEMA_VERSION, DEBUG_OBSERVATION_SCHEMA_VERSION, MAX_DEBUG_VALUE_PREVIEW_BYTES,
};
use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerError, StepInputBytes, StepIo,
    StepOperation, StepOutcome,
};
use conduit_kernel::{
    CordId, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId, PortId, RouteRange,
    RouteTarget, ValueRef, ValueStorage,
};

const PORTS: usize = 1;

#[derive(Clone, Copy)]
enum Driver {
    Source { value: ValueRef, sent: bool },
    Sink { received: bool },
    Fault,
}

impl StepOperation<PORTS> for Driver {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Source { value, sent } if !*sent => {
                io.send(PortId(0), *value).unwrap();
                *sent = true;
                StepOutcome::Progress
            }
            Self::Source { .. } => StepOutcome::Complete,
            Self::Sink { received } if !*received => {
                if io.input(PortId(0)).is_some() {
                    io.consume(PortId(0)).unwrap();
                    *received = true;
                    StepOutcome::Progress
                } else {
                    StepOutcome::Await
                }
            }
            Self::Sink { .. } if io.input_closed(PortId(0)) => {
                io.consume_closed(PortId(0)).unwrap();
                StepOutcome::Complete
            }
            Self::Sink { .. } => StepOutcome::Await,
            Self::Fault => StepOutcome::Fail(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::InvalidInput,
                detail: 17,
            }),
        }
    }

    fn cancel(&mut self) {}
}

fn execution_identity(byte: u8) -> DebugExecutionIdentity {
    DebugExecutionIdentity {
        body: [byte; 32],
        plan: [byte.wrapping_add(1); 32],
        play: [byte.wrapping_add(2); 32],
    }
}

fn observation_bytes(records: usize) -> u32 {
    u32::try_from(records * size_of::<DebugObservationRecord>()).unwrap()
}

fn sign_bytes(records: usize) -> u32 {
    u32::try_from(records * size_of::<KernelEvent>()).unwrap()
}

fn buffer<const RECORDS: usize>(
    execution: DebugExecutionIdentity,
    retained: u16,
    preview: u8,
) -> DebugObservationBuffer<RECORDS> {
    DebugObservationBuffer::new(
        execution,
        retained,
        observation_bytes(usize::from(retained)),
        preview,
    )
    .unwrap()
}

#[test]
fn production_scheduler_emits_exact_bounded_start_value_and_completion_observations() {
    let execution = execution_identity(1);
    let mut values = FixedValueStore::<2, 8>::new(16).unwrap();
    let value = values.store(b"42").unwrap();
    let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: conduit_kernel::CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let signs = FixedSignLog::<32>::new(sign_bytes(32)).unwrap();
    let mut observed = ObservedSignSink::<_, 2, PORTS, 16>::detached(
        signs,
        execution,
        [
            DebugNodeBinding { form: 4, host: 8 },
            DebugNodeBinding { form: 9, host: 8 },
        ],
        [[Some(12)], [Some(12)]],
    );
    observed.attach(buffer(execution, 16, 8)).unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 2, 1, PORTS, 2, 2, 1>::new(
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
            (NodeId(0), PortId(0)),
            (NodeId(1), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 2,
                byte_capacity: 8,
            },
        )],
        routes,
        [
            Driver::Source { value, sent: false },
            Driver::Sink { received: false },
        ],
        values,
        observed,
    )
    .unwrap();

    assert!(matches!(
        scheduler.step().unwrap(),
        conduit_kernel::scheduler::SchedulerStatus::Progress { .. }
    ));
    let live_history = scheduler.detach_debug_observer().unwrap();
    assert!(!live_history.is_empty());
    scheduler.attach_debug_observer(live_history).unwrap();
    scheduler.run(16).unwrap();
    let observations = scheduler.signs().observations().unwrap();
    let records = (0..observations.len())
        .map(|index| *observations.record(index).unwrap())
        .collect::<Vec<_>>();

    assert_eq!(records.first().unwrap().kind, DebugEventKind::GearStarted);
    assert!(records.iter().any(|record| {
        record.kind == DebugEventKind::ValueSent
            && record.subject == DebugSubject::Cord(CordId(0))
            && record.related_subject
                == Some(DebugSubject::Port {
                    gear: NodeId(0),
                    port: PortId(0),
                })
            && record.preview() == b"42"
            && record.type_identity == Some(12)
            && record.form == 4
            && record.host == 8
    }));
    let sent = records
        .iter()
        .find(|record| record.kind == DebugEventKind::ValueSent)
        .unwrap();
    let received = records
        .iter()
        .find(|record| record.kind == DebugEventKind::ValueReceived)
        .unwrap();
    assert_eq!(received.causal_parent_sequence, Some(sent.sequence));
    assert!(sent.invocation_sequence.is_some());
    assert!(received.invocation_sequence.is_some());
    assert!(records.iter().any(|record| {
        record.kind == DebugEventKind::ValueReceived
            && record.subject == DebugSubject::Cord(CordId(0))
            && record.preview() == b"42"
            && record.form == 9
    }));
    assert_eq!(
        records
            .iter()
            .filter(|record| record.kind == DebugEventKind::GearStarted)
            .count(),
        2
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| record.kind == DebugEventKind::GearCompleted)
            .count(),
        2
    );
    assert!(records
        .windows(2)
        .all(|pair| pair[0].sequence < pair[1].sequence));
    assert!(records
        .windows(2)
        .all(|pair| pair[0].host_sequence < pair[1].host_sequence));
    assert!(records.iter().all(|record| record.execution == execution));
    assert!(observations.gap().is_none());
}

#[test]
fn pressure_overwrites_only_debug_history_and_exposes_the_exact_gap() {
    let execution = execution_identity(7);
    let mut observations = buffer::<2>(execution, 2, 4);
    for sequence in 0..3 {
        observations
            .admit(DebugObservationInput {
                execution,
                host_sequence: sequence,
                host: 3,
                form: 5,
                subject: DebugSubject::Port {
                    gear: NodeId(1),
                    port: PortId(0),
                },
                related_subject: Some(DebugSubject::Cord(CordId(0))),
                kind: DebugEventKind::ValueReceived,
                type_identity: Some(12),
                value: Some(b"abcdef"),
                fault_code: None,
                causal_parent_sequence: None,
                invocation_sequence: None,
            })
            .unwrap();
    }

    assert_eq!(observations.len(), 2);
    assert_eq!(observations.record(0).unwrap().sequence, 1);
    assert_eq!(observations.latest().unwrap().sequence, 2);
    assert_eq!(observations.latest().unwrap().preview(), b"abcd");
    assert!(observations.latest().unwrap().preview_truncated);
    assert_eq!(observations.latest().unwrap().value_bytes, 6);
    assert_eq!(
        observations.gap().unwrap(),
        conduit_kernel::debug_observation::DebugObservationGap {
            dropped_records: 1,
            first_retained_sequence: 1,
        }
    );
    assert_eq!(observations.used_bytes(), observation_bytes(2));
}

#[test]
fn stale_unknown_malformed_and_nonmonotonic_inputs_refuse_without_replacing_history() {
    let execution = execution_identity(20);
    let mut observations = buffer::<4>(execution, 4, 8);
    let accepted = observations
        .admit(DebugObservationInput {
            execution,
            host_sequence: 4,
            host: 2,
            form: 1,
            subject: DebugSubject::Gear(NodeId(0)),
            related_subject: None,
            kind: DebugEventKind::GearStarted,
            type_identity: None,
            value: None,
            fault_code: None,
            causal_parent_sequence: None,
            invocation_sequence: None,
        })
        .unwrap();

    let input = |execution, host_sequence, kind| DebugObservationInput {
        execution,
        host_sequence,
        host: 2,
        form: 1,
        subject: DebugSubject::Gear(NodeId(0)),
        related_subject: None,
        kind,
        type_identity: None,
        value: None,
        fault_code: None,
        causal_parent_sequence: None,
        invocation_sequence: None,
    };
    assert_eq!(
        observations.admit(input(
            execution_identity(21),
            5,
            DebugEventKind::GearCompleted,
        )),
        Err(DebugObservationRefusal::StaleExecution)
    );
    assert_eq!(
        observations.admit(input(execution, 4, DebugEventKind::GearCompleted)),
        Err(DebugObservationRefusal::InvalidSequence)
    );
    assert_eq!(
        observations.admit(input(execution, 5, DebugEventKind::Unsupported(99))),
        Err(DebugObservationRefusal::UnsupportedEventKind)
    );
    assert_eq!(observations.len(), 1);

    let mut unknown_schema = accepted;
    unknown_schema.schema_version = DEBUG_OBSERVATION_SCHEMA_VERSION + 1;
    assert_eq!(
        unknown_schema.validate_for(execution, 8),
        Err(DebugObservationRefusal::UnsupportedSchemaVersion)
    );
    let mut unknown_kind = accepted;
    unknown_kind.kind = DebugEventKind::Unsupported(44);
    assert_eq!(
        unknown_kind.validate_for(execution, 8),
        Err(DebugObservationRefusal::UnsupportedEventKind)
    );
}

#[test]
fn attach_detach_and_fault_observation_do_not_change_execution_result() {
    let execution = execution_identity(30);
    let signs = FixedSignLog::<4>::new(sign_bytes(4)).unwrap();
    let mut observed = ObservedSignSink::<_, 2, PORTS, 4>::detached(
        signs,
        execution,
        [
            DebugNodeBinding { form: 2, host: 6 },
            DebugNodeBinding { form: 2, host: 6 },
        ],
        [[None], [None]],
    );
    observed.attach(buffer(execution, 1, 0)).unwrap();
    assert_eq!(
        observed.attach(buffer(execution, 4, 0)),
        Err(DebugObservationRefusal::ObserverAlreadyAttached)
    );
    let detached_history = observed.detach().unwrap();
    assert!(detached_history.is_empty());
    assert!(matches!(
        observed.detach(),
        Err(DebugObservationRefusal::ObserverDetached)
    ));
    observed.attach(buffer(execution, 1, 0)).unwrap();

    let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: conduit_kernel::CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 2, 1, PORTS, 1, 2, 1>::new(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_work: 1,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
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
                byte_capacity: 1,
            },
        )],
        routes,
        [Driver::Fault, Driver::Sink { received: false }],
        FixedValueStore::<1, 1>::new(1).unwrap(),
        observed,
    )
    .unwrap();
    assert_eq!(
        scheduler.step(),
        Err(SchedulerError::OperationFailed(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::InvalidInput,
            detail: 17
        }))
    );
    let observations = scheduler.signs().observations().unwrap();
    assert_eq!(observations.latest().unwrap().kind, DebugEventKind::Fault);
    assert_eq!(observations.latest().unwrap().fault_code, Some(17));
    assert_eq!(observations.gap().unwrap().dropped_records, 1);

    let signs = FixedSignLog::<4>::new(sign_bytes(4)).unwrap();
    let detached = ObservedSignSink::<_, 2, PORTS, 4>::detached(
        signs,
        execution,
        [
            DebugNodeBinding { form: 2, host: 6 },
            DebugNodeBinding { form: 2, host: 6 },
        ],
        [[None], [None]],
    );
    let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: conduit_kernel::CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let mut without_observer = FixedScheduler::<_, _, _, 2, 1, PORTS, 1, 2, 1>::new(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_work: 1,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
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
                byte_capacity: 1,
            },
        )],
        routes,
        [Driver::Fault, Driver::Sink { received: false }],
        FixedValueStore::<1, 1>::new(1).unwrap(),
        detached,
    )
    .unwrap();
    assert_eq!(
        without_observer.step(),
        Err(SchedulerError::OperationFailed(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::InvalidInput,
            detail: 17
        }))
    );
    assert!(without_observer.signs().observations().is_none());
}

#[test]
fn invalid_record_and_preview_budgets_refuse_before_attachment() {
    let execution = execution_identity(50);
    assert!(matches!(
        DebugObservationBuffer::<1>::new(execution, 0, 1, 0),
        Err(DebugObservationRefusal::InvalidBounds)
    ));
    assert!(matches!(
        DebugObservationBuffer::<1>::new(
            execution,
            1,
            u32::try_from(size_of::<DebugObservationRecord>() - 1).unwrap(),
            0,
        ),
        Err(DebugObservationRefusal::InvalidBounds)
    ));
    assert!(matches!(
        DebugObservationBuffer::<1>::new(
            execution,
            1,
            observation_bytes(1),
            u8::try_from(MAX_DEBUG_VALUE_PREVIEW_BYTES + 1).unwrap(),
        ),
        Err(DebugObservationRefusal::InvalidBounds)
    ));
}

#[test]
fn exact_gear_breakpoint_suspends_real_execution_and_resume_is_one_shot() {
    let execution = execution_identity(60);
    let mut values = FixedValueStore::<2, 8>::new(16).unwrap();
    let value = values.store(b"42").unwrap();
    let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: conduit_kernel::CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let signs = FixedSignLog::<32>::new(sign_bytes(32)).unwrap();
    let mut observed = ObservedSignSink::<_, 2, PORTS, 16>::detached(
        signs,
        execution,
        [
            DebugNodeBinding { form: 4, host: 8 },
            DebugNodeBinding { form: 9, host: 8 },
        ],
        [[Some(12)], [Some(12)]],
    );
    observed.attach(buffer(execution, 16, 8)).unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 2, 1, PORTS, 2, 2, 1>::new(
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
            (NodeId(0), PortId(0)),
            (NodeId(1), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 2,
                byte_capacity: 8,
            },
        )],
        routes,
        [
            Driver::Source { value, sent: false },
            Driver::Sink { received: false },
        ],
        values,
        observed,
    )
    .unwrap();
    let breakpoint = DebugBreakpoint {
        schema_version: DEBUG_CONTROL_SCHEMA_VERSION,
        execution,
        subject: DebugSubject::Gear(NodeId(0)),
        kind: DebugBreakpointKind::BeforeGearStart,
    };
    scheduler.request_debug_breakpoint(breakpoint).unwrap();

    assert_eq!(scheduler.step(), Err(SchedulerError::DebugSuspended));
    assert_eq!(scheduler.decisions(), 0);
    let suspension = scheduler.debug_suspension().unwrap();
    assert_eq!(suspension.subject, DebugSubject::Gear(NodeId(0)));
    assert_eq!(scheduler.step(), Err(SchedulerError::DebugSuspended));
    assert_eq!(scheduler.decisions(), 0);
    let mut stale = suspension;
    stale.execution = execution_identity(61);
    assert_eq!(
        scheduler.resume_debug_suspension(stale),
        Err(DebugControlRefusal::StaleSuspension)
    );
    scheduler.resume_debug_suspension(suspension).unwrap();
    assert!(matches!(
        scheduler.step().unwrap(),
        conduit_kernel::scheduler::SchedulerStatus::Progress { node: NodeId(0) }
    ));
    assert_eq!(scheduler.decisions(), 1);
    scheduler.run(16).unwrap();
}

#[test]
fn stale_and_distributed_breakpoints_refuse_before_execution_control() {
    let execution = execution_identity(70);
    let signs = FixedSignLog::<4>::new(sign_bytes(4)).unwrap();
    let observed = ObservedSignSink::<_, 2, PORTS, 4>::detached(
        signs,
        execution,
        [
            DebugNodeBinding { form: 1, host: 2 },
            DebugNodeBinding { form: 2, host: 3 },
        ],
        [[None], [None]],
    );
    let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: conduit_kernel::CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 2, 1, PORTS, 1, 2, 1>::new(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_work: 1,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
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
                byte_capacity: 1,
            },
        )],
        routes,
        [Driver::Fault, Driver::Sink { received: false }],
        FixedValueStore::<1, 1>::new(1).unwrap(),
        observed,
    )
    .unwrap();
    let request = |execution| DebugBreakpoint {
        schema_version: DEBUG_CONTROL_SCHEMA_VERSION,
        execution,
        subject: DebugSubject::Gear(NodeId(0)),
        kind: DebugBreakpointKind::BeforeGearStart,
    };
    assert_eq!(
        scheduler.request_debug_breakpoint(request(execution_identity(71))),
        Err(DebugControlRefusal::StaleExecution)
    );
    assert_eq!(
        scheduler.request_debug_breakpoint(request(execution)),
        Err(DebugControlRefusal::DistributedSuspensionUnsupported)
    );
    assert_eq!(scheduler.decisions(), 0);
}
