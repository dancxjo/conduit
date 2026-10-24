//! Numeric routing/admission and pin possession are fixtures. The production
//! kernel creates each request; the real owner and wire engine service it.
use super::*;
use conduit_kernel::{self as k, scheduler::*, ValueStorage};
enum Back {
    Source {
        values: [k::ValueRef; 2],
        next: usize,
        staged: bool,
    },
    Display(WriteBack),
}
impl StepBack<1> for Back {
    fn step(&mut self, io: &mut StepIo<1>, input: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Display(back) => back.step(io, input),
            Self::Source {
                values,
                next,
                staged,
            } => {
                *staged = false;
                if !io.output_ready(k::PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(k::PortId(0), values[*next]).unwrap();
                *staged = true;
                if *next == 1 {
                    StepOutcome::Complete
                } else {
                    StepOutcome::Progress
                }
            }
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Display(back) => <WriteBack as StepBack<1>>::step_committed(back),
            Self::Source { next, staged, .. } => {
                if *staged {
                    *next += 1;
                    *staged = false;
                }
            }
        }
    }
    fn cancel(&mut self) {
        if let Self::Display(back) = self {
            <WriteBack as StepBack<1>>::cancel(back);
        }
    }
}
#[test]
fn kernel_output_requests_reach_exact_pin_possession_without_allocation_or_retry() {
    for mode in 0..5 {
        execute(mode);
    }
}
fn execute(mode: u8) {
    let canonical = encoded();
    let state = Rc::new(State::default());
    state.nack.set(mode == 1);
    let (mut owner, node) = fixture::owner(Provider(state.clone()));
    assert_eq!(node, k::NodeId(0));
    let mut values = k::FixedValueStore::<2, 1024>::new(2048).unwrap();
    let inputs = [
        values.store(&canonical).unwrap(),
        values.store(&canonical).unwrap(),
    ];
    let mut routes = k::FixedRoutes::<2, 1>::new(1);
    routes
        .install(
            k::NodeId(1),
            k::PortId(0),
            k::RouteRange { start: 0, len: 1 },
            &[k::RouteTarget {
                cord: k::CordId(0),
                sink: k::CordEndpoint::local(k::NodeId(0), k::PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let mut bindings = k::FixedHostCallBindings::<1>::new(1);
    bindings
        .install(
            node,
            k::HostCallBinding {
                call: k::HostCallId(0),
                maximum_input_bytes: MAXIMUM_BYTES,
                maximum_output_bytes: 0,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    let mut kernel = FixedScheduler::<_, _, _, 2, 1, 1, 1, 2, 1, 1, 1>::new_with_host_calls(
        [
            NodeSpec {
                input_cords: [Some(k::CordId(0))],
                maximum_step_fuel: 4,
            },
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 4,
            },
        ],
        [CordSpec::local(
            k::CordId(0),
            (k::NodeId(1), k::PortId(0)),
            (node, k::PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: MAXIMUM_BYTES,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        bindings,
        [
            Back::Display(WriteBack::new(if mode == 3 { 1 } else { 2 })),
            Back::Source {
                values: inputs,
                next: 0,
                staged: false,
            },
        ],
        values,
        k::FixedSignLog::<128>::new((128 * core::mem::size_of::<k::KernelEvent>()) as u32).unwrap(),
    )
    .unwrap();
    let mut submitted = 0;
    let mut terminal = false;
    let ((), memory) = allocation_probe::observe(|| {
        for _ in 0..100 {
            if let Some(request) = kernel.next_host_request() {
                assert_eq!(request.node, node);
                assert_eq!(request.request, k::RequestId(submitted));
                assert_eq!(kernel.host_value(request.input.value).unwrap(), canonical);
                if mode == 4 {
                    kernel.cancel().unwrap();
                    owner.revoke(node, k::HostCallId(0)).unwrap();
                    assert!(kernel
                        .complete_host_call(
                            node,
                            request.request,
                            k::HostCallOutcome {
                                disposition: k::HostCallDisposition::Completed,
                                output: None,
                                failure: None
                            }
                        )
                        .is_err());
                    assert_eq!(state.edges.get(), 0);
                    terminal = true;
                    break;
                }
                let result = owner.invoke(
                    node,
                    request.call,
                    request.request,
                    kernel.host_value(request.input.value).unwrap(),
                    || mode == 2,
                );
                submitted += 1;
                let failure = result.as_ref().err().map(|_| k::Failure {
                    code: if mode == 2 {
                        k::FailureCode::Cancelled
                    } else {
                        k::FailureCode::HostCallDenied
                    },
                    detail: 31,
                });
                kernel
                    .complete_host_call(
                        node,
                        request.request,
                        k::HostCallOutcome {
                            disposition: if failure.is_some() {
                                k::HostCallDisposition::Failed
                            } else {
                                k::HostCallDisposition::Completed
                            },
                            output: None,
                            failure,
                        },
                    )
                    .unwrap();
            }
            match kernel.step() {
                Ok(SchedulerStatus::Drained) => {
                    assert_eq!(mode, 0);
                    terminal = true;
                    break;
                }
                Err(SchedulerError::BackFailed(reason)) => {
                    assert_eq!(
                        reason.code,
                        match mode {
                            1 => k::FailureCode::HostCallDenied,
                            2 => k::FailureCode::Cancelled,
                            3 => k::FailureCode::StorageExhausted,
                            _ => panic!("unexpected failure: {reason:?}"),
                        }
                    );
                    terminal = true;
                    break;
                }
                Ok(_) => (),
                Err(reason) => panic!("unexpected kernel refusal: {reason:?}"),
            }
        }
    });
    assert!(terminal);
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(
        submitted,
        match mode {
            0 => 2,
            4 => 0,
            _ => 1,
        }
    );
    assert_eq!(owner.inspection().completed_operations, submitted);
    if mode != 4 {
        assert_eq!(state.released.get(), [true; 2]);
    }
    if mode == 0 || mode == 3 {
        owner.revoke(node, k::HostCallId(0)).unwrap();
    }
    assert_eq!(owner.inspection().lifecycle, CapabilityLifecycle::Revoked);
}

#[test]
fn sink_phase_advances_only_with_the_ordinary_step_commit() {
    let value = k::ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 1,
    };
    let input = StepInputBytes::test_frame([None], None);
    let mut back = WriteBack::new(2);
    for _ in 0..2 {
        let mut io = StepIo::test_frame([Some(value)], [false], [None], None, 4);
        assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
        assert_eq!(io.test_host_request().unwrap().0, k::RequestId(0));
        // First frame is discarded, as on a refused kernel transaction. The
        // next frame must still stage request 0, not invent another operation.
    }
    <WriteBack as StepBack<1>>::step_committed(&mut back);
    let completed = Some((
        k::RequestId(0),
        k::HostCallOutcome {
            disposition: k::HostCallDisposition::Completed,
            output: None,
            failure: None,
        },
    ));
    for _ in 0..2 {
        let mut io = StepIo::test_frame([None], [false], [None], completed, 4);
        assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
        assert!(io.test_host_completion_consumed());
    }
    <WriteBack as StepBack<1>>::step_committed(&mut back);
    let mut io = StepIo::test_frame([Some(value)], [false], [None], None, 4);
    assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
    assert_eq!(io.test_host_request().unwrap().0, k::RequestId(1));
    <WriteBack as StepBack<1>>::cancel(&mut back);
    <WriteBack as StepBack<1>>::step_committed(&mut back);
    let mut io = StepIo::test_frame([Some(value)], [false], [None], None, 4);
    assert!(matches!(
        back.step(&mut io, &input),
        StepOutcome::Fail(k::Failure {
            code: k::FailureCode::Cancelled,
            ..
        })
    ));
    assert!(io.test_host_request().is_none());
}

#[test]
fn cancelled_host_completion_preserves_detail_and_rejects_inconsistent_reason() {
    for (failure, expected) in [
        (
            None,
            k::Failure {
                code: k::FailureCode::Cancelled,
                detail: 0,
            },
        ),
        (
            Some(k::Failure {
                code: k::FailureCode::Cancelled,
                detail: 13,
            }),
            k::Failure {
                code: k::FailureCode::Cancelled,
                detail: 13,
            },
        ),
        (
            Some(k::Failure {
                code: k::FailureCode::HostCallFailed,
                detail: 13,
            }),
            k::Failure {
                code: k::FailureCode::InvalidLifecycle,
                detail: 10,
            },
        ),
    ] {
        let value = k::ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 1,
        };
        let input = StepInputBytes::test_frame([None], None);
        let mut back = WriteBack::new(1);
        let mut io = StepIo::test_frame([Some(value)], [false], [None], None, 4);
        assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
        <WriteBack as StepBack<1>>::step_committed(&mut back);
        let outcome = k::HostCallOutcome {
            disposition: k::HostCallDisposition::Cancelled,
            output: None,
            failure,
        };
        let mut io =
            StepIo::test_frame([None], [false], [None], Some((k::RequestId(0), outcome)), 4);
        assert_eq!(back.step(&mut io, &input), StepOutcome::Fail(expected));
        assert!(io.test_host_request().is_none());
    }
}
