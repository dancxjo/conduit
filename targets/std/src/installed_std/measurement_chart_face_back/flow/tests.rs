use super::*;
use conduit_kernel::{HostCallDisposition, HostCallOutcome, ValueRef};
fn back() -> MeasurementChartFlowFaceBack {
    MeasurementChartFlowFaceBack {
        inner: MeasurementChartFaceBack {
            gate: FaceCommitGate::prepare("test/chart".into(), HostCallId(0), 2).unwrap(),
            maximum_updates: 2,
            next_request: 0,
            pending: None,
            candidate: None,
            input_bytes: 128,
        },
        phase: None,
        staged: None,
        cancelled: false,
    }
}
fn value() -> ValueRef {
    ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 8,
    }
}
fn frame(completion: Option<u32>) -> StepIo<1> {
    StepIo::test_frame(
        [Some(value())],
        [true],
        [Some(128)],
        completion.map(|r| {
            (
                RequestId(r),
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )
        }),
        32,
    )
}
fn step(back: &mut MeasurementChartFlowFaceBack, io: &mut StepIo<1>) -> StepOutcome {
    back.step(io, &StepInputBytes::test_frame([None], None))
}
fn commit(back: &mut MeasurementChartFlowFaceBack) {
    <MeasurementChartFlowFaceBack as StepBack<1>>::step_committed(back);
}
#[test]
fn exact_two_completions_keep_original_payload_until_observer_custody() {
    let mut back = back();
    let mut start = frame(None);
    assert_eq!(step(&mut back, &mut start), StepOutcome::Progress);
    assert!(start.test_retained(PortId(0)));
    assert!(!start.test_consumed(PortId(0)));
    commit(&mut back);
    let mut built = frame(Some(0));
    assert_eq!(step(&mut back, &mut built), StepOutcome::Progress);
    assert!(built.test_retained(PortId(0)));
    assert!(!built.test_consumed(PortId(0)));
    commit(&mut back);
    assert!(back.phase.unwrap().custody);
    let mut pressured = frame(None);
    assert_eq!(step(&mut back, &mut pressured), StepOutcome::Await);
    assert!(!pressured.test_consumed(PortId(0)));
    let mut accepted = frame(Some(1));
    assert_eq!(step(&mut back, &mut accepted), StepOutcome::Progress);
    assert!(accepted.test_consumed(PortId(0)));
    commit(&mut back);
    assert!(back.phase.is_none());
    let mut closed = StepIo::test_frame([None], [true], [Some(128)], None, 32);
    assert_eq!(step(&mut back, &mut closed), StepOutcome::Complete);
    assert!(closed.test_consumed_closed(PortId(0)));
}
#[test]
fn wrong_late_completion_and_cancellation_cannot_consume_chart() {
    let mut back = back();
    let mut start = frame(None);
    step(&mut back, &mut start);
    commit(&mut back);
    let mut wrong = frame(Some(77));
    assert!(matches!(step(&mut back, &mut wrong), StepOutcome::Fail(_)));
    assert!(!wrong.test_consumed(PortId(0)));
    <MeasurementChartFlowFaceBack as StepBack<1>>::cancel(&mut back);
    let mut late = frame(Some(0));
    assert!(matches!(step(&mut back, &mut late), StepOutcome::Fail(_)));
    assert!(!late.test_consumed(PortId(0)));
}
#[test]
fn original_abnormal_cause_propagates_only_after_owed_payload() {
    let terminal = conduit_kernel::CanonicalValue::new(
        &conduit_core::TerminalInfo::new(conduit_core::TerminalCategory::DomainOwned, [9; 32])
            .encode(),
    )
    .unwrap();
    let mut back = back();
    let mut io = StepIo::test_frame([None], [false], [Some(128)], None, 32)
        .test_with_input_abnormal(PortId(0), terminal);
    assert_eq!(
        step(&mut back, &mut io),
        StepOutcome::Abnormal {
            port: PortId(0),
            terminal
        }
    );
    assert!(io.test_consumed_closed(PortId(0)));
}
#[test]
fn pending_observer_custody_suspends_only_its_own_node() {
    use conduit_kernel::scheduler::{CordCapacity, CordSpec, FixedScheduler, NodeSpec};
    use conduit_kernel::{
        CordId, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore, HostCallBinding,
        NodeId, RemoteEndpointId,
    };
    enum Driver {
        Face(Box<MeasurementChartFlowFaceBack>),
        Other(bool),
    }
    impl StepBack<1> for Driver {
        fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
            match self {
                Self::Face(back) => back.step(io, bytes),
                Self::Other(_) => {
                    if io.input(PortId(0)).is_some() {
                        io.consume(PortId(0)).unwrap();
                        StepOutcome::Complete
                    } else {
                        StepOutcome::Await
                    }
                }
            }
        }
        fn step_committed(&mut self) {
            match self {
                Self::Face(back) => {
                    <MeasurementChartFlowFaceBack as StepBack<1>>::step_committed(back)
                }
                Self::Other(done) => *done = true,
            }
        }
    }
    type Kernel =
        FixedScheduler<Driver, FixedValueStore<8, 256>, FixedSignLog<128>, 2, 2, 1, 2, 0, 0, 4, 1>;
    let nodes = core::array::from_fn(|i| NodeSpec {
        input_cords: [Some(CordId(i as u16))],
        maximum_step_fuel: 32,
    });
    let cords = core::array::from_fn(|i| {
        CordSpec::remote_ingress(
            CordId(i as u16),
            RemoteEndpointId(i as u16),
            (NodeId(i as u16), PortId(0)),
            CordCapacity {
                slot_start: i as u16,
                item_capacity: 1,
                byte_capacity: 128,
                pressure_policy: Default::default(),
            },
        )
    });
    let mut routes = FixedRoutes::<0, 0>::new(1);
    routes.seal().unwrap();
    let mut calls = FixedHostCallBindings::<4>::new(2);
    for call in [0, 1] {
        calls
            .install(
                NodeId(0),
                HostCallBinding {
                    call: HostCallId(call),
                    maximum_input_bytes: 128,
                    maximum_output_bytes: 0,
                },
            )
            .unwrap();
    }
    calls.seal().unwrap();
    let signs = FixedSignLog::<128>::new_with_remote_storage(
        (128 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
        32,
        conduit_kernel::remote_sign_storage_bytes(32).unwrap(),
    )
    .unwrap();
    let mut kernel = Kernel::new_with_active_counts_and_host_calls(
        2,
        2,
        nodes,
        cords,
        routes,
        calls,
        [Driver::Face(Box::new(back())), Driver::Other(false)],
        FixedValueStore::new(256).unwrap(),
        signs,
    )
    .unwrap();
    kernel
        .admit_remote_input(RemoteEndpointId(0), CordId(0), 0, &[0; 8])
        .unwrap();
    let first = (0..8)
        .find_map(|_| {
            kernel.step().unwrap();
            kernel.next_host_request()
        })
        .unwrap();
    kernel
        .complete_host_call(
            first.node,
            first.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .unwrap();
    let custody = (0..8)
        .find_map(|_| {
            kernel.step().unwrap();
            kernel.next_host_request()
        })
        .unwrap();
    assert_eq!(custody.call, HostCallId(1));
    kernel
        .admit_remote_input(RemoteEndpointId(1), CordId(1), 0, &[1])
        .unwrap();
    for _ in 0..4 {
        kernel.step().unwrap();
    }
    assert!(matches!(kernel.drivers()[1], Driver::Other(true)));
    assert!(kernel.host_call_is_pending(custody));
}
