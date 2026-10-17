#![cfg(feature = "alloc")]
//! Physical grouping exercises the production transaction, pressure and debug paths.
use conduit_kernel::debug_observation::*;
use conduit_kernel::scheduler::*;
use conduit_kernel::*;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
const N: usize = 34;
const C: usize = 33;
const SIGNS: usize = 4096;
type Evidence = ObservedSignSink<HostedSignLog, N, 1, 8>;
type Play = FixedScheduler<Driver, FixedValueStore<64, 8>, Evidence, N, C, 1, C, N, C>;

#[derive(Clone, Copy)]
enum Driver {
    Source {
        values: [ValueRef; 4],
        index: usize,
        count: usize,
    },
    Add {
        bytes: [u8; 8],
        fail: bool,
    },
    Sink {
        values: [u64; 4],
        count: usize,
        delay: usize,
    },
}
impl StepBack<1> for Driver {
    fn step(&mut self, io: &mut StepIo<1>, inputs: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Source {
                values,
                index,
                count,
            } => {
                if *index == *count {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), values[*index]).unwrap();
                *index += 1;
                StepOutcome::Progress
            }
            Self::Add { bytes, fail } => {
                if let Some(input) = inputs.input(PortId(0)) {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    if *fail {
                        return StepOutcome::Fail(Failure {
                            code: FailureCode::InvalidInput,
                            detail: 77,
                        });
                    }
                    *bytes = (u64::from_le_bytes(input.try_into().unwrap()) + 1).to_le_bytes();
                    io.send_prepared(PortId(0), 8).unwrap();
                    io.consume(PortId(0)).unwrap();
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::Sink {
                values,
                count,
                delay,
            } => {
                if *delay > 0 {
                    *delay -= 1;
                    io.consume_fuel(io.remaining_fuel()).unwrap();
                    return StepOutcome::Yield;
                }
                if let Some(input) = inputs.input(PortId(0)) {
                    values[*count] = u64::from_le_bytes(input.try_into().unwrap());
                    *count += 1;
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
    fn prepared_output(&self, _: PortId) -> Option<&[u8]> {
        match self {
            Self::Add { bytes, .. } => Some(bytes),
            _ => None,
        }
    }
    fn cancel(&mut self) {}
}
fn execution() -> DebugExecutionIdentity {
    DebugExecutionIdentity {
        body: [1; 32],
        plan: [2; 32],
        play: [3; 32],
    }
}
fn prepare(fused: bool, count: usize, delay: usize, failure: Option<usize>) -> Play {
    let mut values = FixedValueStore::<64, 8>::new(512).unwrap();
    let inputs = core::array::from_fn(|i| values.store(&(i as u64).to_le_bytes()).unwrap());
    let mut routes = FixedRoutes::<N, C>::new(1);
    for i in 0..C {
        routes
            .install(
                NodeId(i as u16),
                PortId(0),
                RouteRange {
                    start: i as u16,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(i as u16),
                    sink: CordEndpoint::local(NodeId(i as u16 + 1), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let signs = HostedSignLog::new(
        SIGNS as u16,
        (SIGNS * core::mem::size_of::<KernelEvent>()) as u32,
    )
    .unwrap();
    let mut evidence = Evidence::detached(
        signs,
        execution(),
        [DebugNodeBinding { host: 1, plot: 1 }; N],
        [[Some(1)]; N],
    );
    evidence
        .attach(
            DebugObservationBuffer::<8>::new(
                execution(),
                8,
                (8 * core::mem::size_of::<DebugObservationRecord>()) as u32,
                0,
            )
            .unwrap(),
        )
        .unwrap();
    let drivers = core::array::from_fn(|i| {
        if i == 0 {
            Driver::Source {
                values: inputs,
                index: 0,
                count,
            }
        } else if i == N - 1 {
            Driver::Sink {
                values: [0; 4],
                count: 0,
                delay,
            }
        } else {
            Driver::Add {
                bytes: [0; 8],
                fail: failure == Some(i),
            }
        }
    });
    let mut play = Play::new(
        core::array::from_fn(|i| NodeSpec {
            input_cords: [(i > 0).then(|| CordId(i as u16 - 1))],
            maximum_step_fuel: 8,
        }),
        core::array::from_fn(|i| {
            CordSpec::local(
                CordId(i as u16),
                (NodeId(i as u16), PortId(0)),
                (NodeId(i as u16 + 1), PortId(0)),
                CordCapacity {
                    slot_start: i as u16,
                    item_capacity: 1,
                    byte_capacity: 8,
                    pressure_policy: Default::default(),
                },
            )
        }),
        routes,
        drivers,
        values,
        evidence,
    )
    .unwrap();
    if fused {
        play.bind_execution_chain(&(1..=32).map(NodeId).collect::<Vec<_>>())
            .unwrap();
    }
    play
}
fn finish(play: &mut Play) {
    for _ in 0..10000 {
        if play.step().unwrap() == SchedulerStatus::Drained {
            return;
        }
    }
    panic!("bounded chain did not drain");
}
#[test]
fn logical_outputs_pressure_terminal_signs_and_allocation_match() {
    for count in [0, 1, 4] {
        for delay in [0, 100] {
            let mut reference = prepare(false, count, delay, None);
            let mut fused = prepare(true, count, delay, None);
            let (_, allocation) = allocation_probe::observe(|| {
                finish(&mut reference);
                finish(&mut fused);
            });
            assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
            match (&reference.drivers()[N - 1], &fused.drivers()[N - 1]) {
                (
                    Driver::Sink {
                        values: a,
                        count: ac,
                        ..
                    },
                    Driver::Sink {
                        values: b,
                        count: bc,
                        ..
                    },
                ) => {
                    assert_eq!(a, b);
                    assert_eq!(ac, bc);
                    assert_eq!(*ac, count);
                }
                _ => unreachable!(),
            }
            // Logical per-node evidence is unchanged even where physical order interleaves differently.
            for node in 0..N {
                for kind in [
                    KernelEventKind::ValueRouted,
                    KernelEventKind::ValueConsumed,
                    KernelEventKind::BackCompleted,
                ] {
                    let events = |p: &Play| {
                        p.signs()
                            .inner()
                            .events()
                            .filter(|e| e.node == NodeId(node as u16) && e.kind == kind)
                            .count()
                    };
                    assert_eq!(events(&reference), events(&fused));
                }
            }
            for play in [&reference, &fused] {
                assert_eq!(
                    play.logical_invocations(),
                    play.signs()
                        .inner()
                        .events()
                        .filter(|event| event.kind == KernelEventKind::StepFuelGranted)
                        .count() as u64
                );
            }
            assert!(fused.decisions() < reference.decisions());
        }
    }
}
#[test]
fn every_logical_failure_point_survives_grouping() {
    for node in 1..=32 {
        for fused in [false, true] {
            let mut play = prepare(fused, 1, 0, Some(node));
            loop {
                match play.step() {
                    Err(SchedulerError::BackFailed(failure)) => {
                        assert_eq!(failure.detail, 77);
                        break;
                    }
                    Ok(_) => {}
                    other => panic!("unexpected {other:?}"),
                }
            }
            assert!(play
                .signs()
                .inner()
                .events()
                .any(|e| e.node == NodeId(node as u16) && e.kind == KernelEventKind::BackFailed));
        }
    }
}
#[test]
fn every_logical_checkpoint_can_suspend_then_cancel_or_resume() {
    for node in 1..=32 {
        for cancel in [false, true] {
            let mut play = prepare(true, 1, 0, None);
            play.request_debug_breakpoint(DebugBreakpoint {
                schema_version: DEBUG_CONTROL_SCHEMA_VERSION,
                execution: execution(),
                subject: DebugSubject::Gear(NodeId(node)),
                kind: DebugBreakpointKind::BeforeGearStart,
            })
            .unwrap();
            loop {
                match play.step() {
                    Err(SchedulerError::DebugSuspended) => break,
                    Ok(_) => {}
                    other => panic!("unexpected {other:?}"),
                }
            }
            let suspension = play.debug_suspension().unwrap();
            assert_eq!(suspension.node, NodeId(node));
            if cancel {
                play.cancel().unwrap();
                assert_eq!(play.step().unwrap(), SchedulerStatus::Cancelled);
            } else {
                play.resume_debug_suspension(suspension).unwrap();
                finish(&mut play);
            }
        }
    }
}

#[test]
fn invalid_overlapping_and_late_chains_refuse_before_mutation() {
    let mut play = prepare(false, 1, 0, None);
    for chain in [
        vec![NodeId(1)],
        vec![NodeId(1), NodeId(1)],
        vec![NodeId(1), NodeId(3)],
        vec![NodeId(2), NodeId(1)],
        vec![NodeId(1), NodeId(99)],
    ] {
        assert_eq!(
            play.bind_execution_chain(&chain),
            Err(SchedulerError::InvalidPlan)
        );
    }
    play.bind_execution_chain(&[NodeId(1), NodeId(2)]).unwrap();
    assert_eq!(
        play.bind_execution_chain(&[NodeId(2), NodeId(3)]),
        Err(SchedulerError::InvalidPlan)
    );
    play.step().unwrap();
    assert_eq!(
        play.bind_execution_chain(&[NodeId(3), NodeId(4)]),
        Err(SchedulerError::InvalidPlan)
    );
    finish(&mut play);
}

#[test]
fn execution_owner_can_stop_before_every_logical_gear_without_later_steps() {
    for node in 1..=32 {
        let mut play = prepare(true, 1, 0, None);
        let mut stopped = false;
        for _ in 0..100 {
            play.step_with_checkpoint(|candidate| {
                assert!(!stopped, "execution continued after the control checkpoint");
                stopped = candidate == NodeId(node);
                !stopped
            })
            .unwrap();
            if stopped {
                break;
            }
        }
        assert!(stopped);
        assert!(!play
            .signs()
            .inner()
            .events()
            .any(|event| event.node == NodeId(node)
                && event.kind == KernelEventKind::StepFuelGranted));
        play.cancel().unwrap();
        assert_eq!(play.step().unwrap(), SchedulerStatus::Cancelled);
    }
}
